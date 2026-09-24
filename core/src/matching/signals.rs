//! Checkable facts of a job text: remote share, day rate, start, ANÜ, countries
//! (old engine semantics, Python `analyze_job`).

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use super::lexicon;
use super::normalize::{casefold, decimal};
use super::pyre;

static FULLY_REMOTE: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::FULLY_REMOTE));
static REMOTE_PERCENT: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::REMOTE_PERCENT));
static ONSITE: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::ONSITE));
static RATE: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::RATE));
static RATE_SIMPLE: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::RATE_SIMPLE));
static START_NOW: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::START_NOW));
static FUTURE_START: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::FUTURE_START));
static ANUE: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::ANUE));
static ANUE_NEGATED: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::ANUE_NEGATED));
static COUNTRY: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::COUNTRY));

/// Python refuses to convert integer strings longer than this (`ValueError`, read as 0).
const PYTHON_INT_MAX_DIGITS: usize = 4300;

/// Job signals. Ranges are byte ranges in the case-folded text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobSignals {
    /// Remote share in percent: 100 fully remote, 0 on site, 60 "remote" mentioned.
    pub remote: Option<u32>,
    /// Day rate as a Python integer in decimal digits (`"0"` counts as no rate).
    pub rate: Option<String>,
    pub start_now: bool,
    pub start_future: bool,
    /// ANÜ named and not negated anywhere in the text.
    pub anue: bool,
    /// ISO codes of the countries named anywhere, with their first mention.
    pub countries: BTreeMap<&'static str, Range<usize>>,
    pub rate_at: Option<Range<usize>>,
    pub future_at: Option<Range<usize>>,
    pub anue_at: Option<Range<usize>>,
}

/// Python `analyze_job`.
pub(crate) fn analyze(text: &str) -> JobSignals {
    let lowered = casefold(text);
    let view = pyre::view(&lowered);
    let back = |start: usize, end: usize| {
        let (a, b) = pyre::original_range(&lowered, &view, start, end);
        a..b
    };
    let remote = if FULLY_REMOTE.is_match(&view) {
        Some(100)
    } else if let Some(caps) = REMOTE_PERCENT.captures(&view) {
        Some(python_digits(&caps[1]).parse().unwrap_or(0))
    } else if ONSITE.is_match(&view) {
        Some(0)
    } else if lowered.contains("remote") {
        Some(60)
    } else {
        None
    };
    let rate_caps = RATE.captures(&view).or_else(|| RATE_SIMPLE.captures(&view));
    let rate = rate_caps.as_ref().map(|caps| int_rate(&caps[1]));
    let rate_at = rate_caps
        .as_ref()
        .and_then(|caps| caps.get(0))
        .map(|m| back(m.start(), m.end()));
    let future = FUTURE_START.find(&view);
    let anue_match = ANUE.find(&view);
    let anue = anue_match.is_some() && !ANUE_NEGATED.is_match(&view);
    let mut countries = BTreeMap::new();
    for m in COUNTRY.find_iter(&view) {
        let range = back(m.start(), m.end());
        if let Some(code) = lexicon::country_code(&casefold(&lowered[range.clone()])) {
            countries.entry(code).or_insert(range);
        }
    }
    JobSignals {
        remote,
        rate,
        start_now: START_NOW.is_match(&view),
        start_future: future.is_some(),
        anue,
        countries,
        rate_at,
        future_at: future.map(|m| back(m.start(), m.end())),
        anue_at: anue_match
            .filter(|_| anue)
            .map(|m| back(m.start(), m.end())),
    }
}

/// Decimal digits of any script as ASCII (Python `int()` accepts them all).
fn python_digits(text: &str) -> String {
    text.chars()
        .filter_map(decimal)
        .filter_map(|d| char::from_digit(d, 10))
        .collect()
}

/// Python `_int_rate`: separators removed, the rest read as an integer (decimal digits,
/// no leading zeros).
///
/// The one documented deviation from the old engine: a decimal comma followed by one or
/// two digits at the end ends the number, so `1.200,50` is 1200 (the old engine glued the
/// cents on and read 120050).
pub fn int_rate(value: &str) -> String {
    let mut number = value;
    if let Some((integer, fraction)) = value.rsplit_once(',') {
        let fraction_len = fraction.chars().count();
        if (1..=2).contains(&fraction_len) && fraction.chars().all(|c| decimal(c).is_some()) {
            number = integer;
        }
    }
    let digits = python_digits(&number.replace(['.', ','], ""));
    if digits.len() > PYTHON_INT_MAX_DIGITS {
        return "0".to_owned();
    }
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// `rate < min` for a Python integer in decimal digits.
pub(crate) fn rate_below(rate: &str, min: i128) -> bool {
    if min <= 0 || rate.len() > 38 {
        return false;
    }
    rate.parse::<i128>().is_ok_and(|r| r < min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_parsing() {
        assert_eq!(int_rate("1.200"), "1200");
        assert_eq!(int_rate("1.200,50"), "1200");
        assert_eq!(int_rate("1,200"), "1200");
        assert_eq!(int_rate("\u{661}\u{662}\u{660}\u{660}"), "1200");
        assert_eq!(int_rate("0.050"), "50");
        let s = analyze("Tagessatz: 800 € pro Tag, 100 % Remote, ab sofort.");
        assert_eq!(
            (s.rate.as_deref(), s.remote, s.start_now),
            (Some("800"), Some(100), true)
        );
    }

    #[test]
    fn range_rate_reads_the_upper_part() {
        // Old behaviour kept on purpose: "900 - 1.100 €" is read as 100 by the simple pattern.
        assert_eq!(
            analyze("Honorar: 900 - 1.100 € pro Tag").rate.as_deref(),
            Some("100")
        );
    }

    #[test]
    fn anue_negation_is_global() {
        assert!(analyze("Einsatz über ANÜ.").anue);
        assert!(!analyze("Keine ANÜ. Später doch ANÜ.").anue);
        assert!(analyze("nicht im Rahmen der Arbeitnehmerüberlassung").anue);
    }
}
