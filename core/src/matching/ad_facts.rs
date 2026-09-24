//! The key facts of an ad (rate, start, duration, remote share, place, contract type,
//! salary, years), each with the passage that states it: the evidence of the hard-criteria
//! strip and the facts of the list row. The page facts come first, then the text.

use std::ops::Range;

use serde_json::Value;

use super::atoms::fold;
use super::contract::{Contract, ContractKind};
use super::facts::{self, JobFacts, Rate, Segment, Start, fact, parse_start, stated_rate};
use super::job::{JobDoc, contains_word};
use super::lexicon::engine as lex;
use super::permanent::parse_salary;
use super::seniority::experience_years;
use super::types::KeyFacts;
use super::wishes::remote_share;

/// Longest duration read (ten years).
const MAX_MONTHS: u64 = 120;

/// A value the ad states, with the byte range of the sentence (`None` for a page fact).
#[derive(Debug, Clone)]
pub(crate) struct Stated<T> {
    pub value: T,
    pub span: Option<Range<usize>>,
}

fn stated<T>(value: T, span: Option<Range<usize>>) -> Stated<T> {
    Stated { value, span }
}

/// What the ad states about the hard criteria and the key facts.
#[derive(Debug, Clone)]
pub(crate) struct AdFacts {
    pub rate: Option<Stated<Rate>>,
    /// A rate to be agreed (`Tagessatz nach Absprache`), without an amount.
    pub rate_open: Option<Stated<()>>,
    pub start: Option<Stated<Start>>,
    pub months: Option<Stated<u16>>,
    /// Remote share in percent (from, to).
    pub remote: Option<(u64, u64)>,
    /// The work location as the page states it (trimmed, may be empty).
    pub location: String,
    pub contract: ContractKind,
    /// The contract type rests on a statement (a page fact or a sentence), not on hints.
    pub contract_stated: bool,
    pub contract_span: Option<Range<usize>>,
    /// Stated annual salary (a monthly one times twelve).
    pub salary: Option<Stated<u64>>,
    /// The most years of experience a requirement line asks for.
    pub years: Option<Stated<u32>>,
}

/// Reads the key facts of an ad.
pub(crate) fn read(
    job: &JobFacts<'_>,
    segments: &[Segment],
    folded: &str,
    contract: &Contract,
    doc: &JobDoc,
) -> AdFacts {
    let rate = stated_rate(job, segments).map(|(rate, span)| stated(rate, span));
    let rate_open = if rate.is_none() {
        segments
            .iter()
            .find(|(_, f)| {
                lex::RATE_WORDS.iter().any(|w| f.contains(w))
                    && lex::RATE_OPEN.iter().any(|w| f.contains(w))
            })
            .map(|(range, _)| stated((), Some(range.clone())))
    } else {
        None
    };
    let location = fact(job.facts, super::fact_key::LOCATION)
        .and_then(Value::as_str)
        .unwrap_or(job.location)
        .trim()
        .to_owned();
    let contract_fact = fact(job.facts, super::fact_key::CONTRACT).is_some();
    let contract_span = contract.spans.first().cloned();
    AdFacts {
        rate,
        rate_open,
        start: start(job, segments),
        months: months(job, segments),
        remote: remote_share(job, segments, folded),
        location,
        contract: contract.kind,
        contract_stated: !contract.inferred && (contract_fact || contract_span.is_some()),
        contract_span,
        salary: segments.iter().find_map(|(range, f)| {
            let salary = parse_salary(f)?;
            let shown = salary.upper.unwrap_or(salary.lower);
            let per_year = if salary.monthly {
                shown.saturating_mul(12)
            } else {
                shown
            };
            Some(stated(per_year, Some(range.clone())))
        }),
        years: years(job.text, doc),
    }
}

/// The start: the page fact, else the first sentence about the start.
fn start(job: &JobFacts<'_>, segments: &[Segment]) -> Option<Stated<Start>> {
    let from_fact = fact(job.facts, super::fact_key::START)
        .and_then(Value::as_str)
        .and_then(parse_start)
        .map(|s| stated(s, None));
    from_fact.or_else(|| {
        segments
            .iter()
            .filter(|(_, f)| lex::START_WORDS.iter().any(|w| f.contains(w)))
            .find_map(|(range, f)| parse_start(f).map(|s| stated(s, Some(range.clone()))))
    })
}

/// The duration: the page fact, else the first sentence about duration or start that
/// names months, weeks or years.
fn months(job: &JobFacts<'_>, segments: &[Segment]) -> Option<Stated<u16>> {
    let from_fact = fact(job.facts, super::fact_key::DURATION)
        .and_then(Value::as_str)
        .and_then(|s| parse_months(&fold(s)))
        .map(|m| stated(m, None));
    from_fact.or_else(|| {
        segments
            .iter()
            .filter(|(_, f)| {
                lex::DURATION_WORDS
                    .iter()
                    .chain(lex::START_WORDS)
                    .any(|w| f.contains(w))
            })
            .find_map(|(range, f)| parse_months(f).map(|m| stated(m, Some(range.clone()))))
    })
}

/// Months of a duration statement (`6 Monate`, `3-6 months`, `12+ Monate`, `1 Jahr`,
/// `8 Wochen`): the largest amount before a unit; weeks round up to months.
pub(crate) fn parse_months(folded: &str) -> Option<u16> {
    let words: Vec<&str> = folded
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mut best: Option<u64> = None;
    for pair in words.windows(2) {
        let Ok(amount) = pair[0].parse::<u64>() else {
            continue;
        };
        let unit = pair[1];
        let is = |units: &[&str]| units.iter().any(|u| unit.starts_with(u));
        let months = if is(lex::MONTH_UNITS) {
            amount
        } else if is(lex::YEAR_UNITS) {
            amount.saturating_mul(12)
        } else if is(lex::WEEK_UNITS) {
            amount.div_ceil(4)
        } else {
            continue;
        };
        if (1..=MAX_MONTHS).contains(&months) {
            best = best.max(Some(months));
        }
    }
    best.and_then(|m| u16::try_from(m).ok())
}

/// The most years a requirement line asks for, with the line.
fn years(text: &str, doc: &JobDoc) -> Option<Stated<u32>> {
    doc.requirement_lines
        .iter()
        .filter_map(|range| {
            let line = text.get(range.clone())?;
            let (min, _) = experience_years(&fold(line))?;
            Some(stated(min, Some(range.clone())))
        })
        .max_by_key(|s| s.value)
}

impl AdFacts {
    /// The compact facts for the list row and the reader.
    pub(crate) fn key_facts(&self) -> KeyFacts {
        let percent = |p: u64| u8::try_from(p.min(100)).unwrap_or(100);
        KeyFacts {
            rate: self
                .rate
                .as_ref()
                .map(|r| u32::try_from(r.value.upper).unwrap_or(u32::MAX)),
            hourly: self.rate.as_ref().map(|r| r.value.hourly),
            currency: self
                .rate
                .as_ref()
                .and_then(|r| r.value.currency)
                .map(currency_code),
            rate_open: self.rate_open.as_ref().map(|_| true),
            start: self.start.as_ref().map(|s| start_code(s.value)),
            months: self.months.as_ref().map(|m| m.value),
            remote_from: self.remote.map(|(from, _)| percent(from)),
            remote_to: self.remote.map(|(_, to)| percent(to)),
            contract: match self.contract {
                ContractKind::Interim if self.contract_stated => Some("interim".into()),
                ContractKind::Permanent if self.contract_stated => Some("permanent".into()),
                ContractKind::Anue => Some("anue".into()),
                _ => None,
            },
        }
    }
}

/// `now`, `vague` or the ISO date of a start.
pub(crate) fn start_code(start: Start) -> String {
    match start {
        Start::Now => "now".into(),
        Start::Vague => "vague".into(),
        Start::Date(date) => date.to_string(),
    }
}

/// The ISO code of a currency word (`$` is USD, `£` GBP).
pub(crate) fn currency_code(word: &str) -> String {
    match word {
        "$" => "USD".into(),
        "£" => "GBP".into(),
        other => other.to_uppercase(),
    }
}

/// Is the ad's work location in one of `allowed` (country codes)?
pub(crate) fn location_allowed(location: &str, allowed: &[String]) -> bool {
    let mut codes = facts::location_countries(location);
    // A German city without a country (`Hamburg`) is in Germany.
    let folded = fold(location);
    if codes.is_empty() && lex::GERMAN_CITIES.iter().any(|c| contains_word(&folded, c)) {
        codes.push("DE");
    }
    !codes.is_empty() && codes.iter().all(|c| allowed.iter().any(|a| a == c))
}

/// Does the location name remote work?
pub(crate) fn location_remote(location: &str) -> bool {
    contains_word(&fold(location), "remote")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_in_months() {
        let m = |s: &str| parse_months(&fold(s));
        assert_eq!(m("Laufzeit: 6 Monate"), Some(6));
        assert_eq!(m("Duration 3-6 months, extension possible"), Some(6));
        assert_eq!(m("12+ Monate"), Some(12));
        assert_eq!(m("Projektdauer 1 Jahr"), Some(12));
        assert_eq!(m("8 Wochen"), Some(2));
        assert_eq!(m("Start ab sofort"), None);
        assert_eq!(m("Laufzeit bis 31.12.2026"), None);
        assert_eq!(m("Laufzeit 999 Monate"), None);
    }
}
