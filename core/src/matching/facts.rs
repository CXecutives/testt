//! Hard criteria of the new engine (V10-V13), structured facts first. A violation is
//! decided only on clear wording; everything unclear becomes a check:
//! ANÜ (named, not negated, not optional) · work country (location, on-site sentence, not
//! fully remote) · day rate (EUR, upper bound, hourly x 8, not for permanent roles) ·
//! availability (never decided: a gap or a vague start is a check).

use std::ops::Range;

use jiff::civil::Date;
use serde_json::{Value, json};

use super::atoms::fold;
use super::contract::{Contract, ContractKind};
use super::job::{contains_word, sentences};
use super::lexicon::{self, engine as lex};
use super::normalize::splitlines;
use super::params::HOURS_PER_DAY;
use super::profile::Criteria;
use super::types::{CriterionKey, ReasonCode, ReasonKind};
use crate::portal::Portal;

/// When the consultant is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
    Unset,
    Now,
    From(Date),
}

/// Hard criteria as the new engine reads them from the profile.
#[derive(Debug, Clone)]
pub(crate) struct HardCriteria {
    pub min_rate: Option<i128>,
    pub countries: Option<Vec<String>>,
    pub remote_outside: Option<bool>,
    pub anue_excluded: bool,
    pub available: Availability,
    /// Minimum annual salary (EUR) for permanent roles.
    pub min_salary: Option<u64>,
    /// Places of the region for permanent roles (as written; matched folded).
    pub places: Option<Vec<String>>,
    /// Remote share (percent) that makes a permanent role outside the region acceptable.
    pub remote_min: Option<u64>,
    /// Minimum years the target profile of an ad must ask for.
    pub target_years: Option<u32>,
    /// Keys present with a value that cannot be read: (key, value).
    pub not_understood: Vec<(&'static str, String)>,
}

/// A profile value of the new criteria: first key found in `harte_kriterien` (or the
/// English section), German key first.
fn criterion<'a>(data: &'a Value, keys: &[&'static str]) -> Option<(&'static str, &'a Value)> {
    lexicon::KEY_CRITERIA_ALIASES.iter().find_map(|section| {
        let section = data.get(*section)?;
        keys.iter()
            .find_map(|k| section.get(*k).filter(|v| !v.is_null()).map(|v| (*k, v)))
    })
}

/// A whole number from a JSON number or a string (`150000`, `150.000`, `150k`, `60 %`).
fn number(value: &Value) -> Option<u64> {
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    if let Some(f) = value.as_f64() {
        // Profile values are small positive numbers; fractions are cut.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        return (f >= 0.0).then_some(f as u64);
    }
    let text = fold(value.as_str()?);
    let text = text
        .trim()
        .trim_end_matches('%')
        .trim_end_matches('€')
        .trim();
    let (digits, factor) = match text.strip_suffix('k') {
        Some(rest) => (rest.trim(), 1000),
        None => (text, 1),
    };
    let digits: String = digits
        .chars()
        .filter(|c| !matches!(c, '.' | ',' | ' '))
        .collect();
    digits.parse::<u64>().ok().map(|n| n * factor)
}

impl HardCriteria {
    pub(crate) fn new(legacy: &Criteria, data: &Value) -> Self {
        let available = match availability_text(data) {
            None => Availability::Unset,
            Some(text) => match parse_start(text) {
                Some(Start::Date(from)) => Availability::From(from),
                _ if fold(text).contains(lex::NOW_WORD) => Availability::Now,
                _ => Availability::Unset,
            },
        };
        let mut not_understood = Vec::new();
        let mut read = |keys: &[&'static str]| {
            let (key, value) = criterion(data, keys)?;
            let n = number(value).filter(|n| *n > 0);
            if n.is_none() {
                not_understood.push((key, value.to_string()));
            }
            n
        };
        let min_salary = read(lexicon::KEYS_MIN_SALARY);
        let remote_min = read(lexicon::KEYS_PERMANENT_REMOTE).map(|p| p.min(100));
        let target_years = read(lexicon::KEYS_TARGET_YEARS).and_then(|y| u32::try_from(y).ok());
        let places = criterion(data, lexicon::KEYS_PERMANENT_PLACES).and_then(|(key, value)| {
            let list: Vec<String> = value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect();
            if list.is_empty() {
                not_understood.push((key, value.to_string()));
                None
            } else {
                Some(list)
            }
        });
        Self {
            min_rate: legacy.min_day_rate.filter(|m| *m > 0),
            countries: legacy.countries.clone().filter(|c| !c.is_empty()),
            remote_outside: legacy.remote_outside_allowed,
            anue_excluded: legacy.anue_excluded == Some(true),
            available,
            min_salary,
            places,
            remote_min,
            target_years,
            not_understood,
        }
    }
}

/// The profile's availability text (hard criteria first, then preferences).
pub(crate) fn availability_text(data: &Value) -> Option<&str> {
    [lexicon::KEY_CRITERIA, lexicon::KEY_PREFERENCES]
        .iter()
        .filter_map(|section| data.get(*section)?.get(lexicon::KEY_AVAILABLE)?.as_str())
        .find(|s| !s.trim().is_empty())
}

/// A decided violation, a check, or a frame row (met or partial).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Finding {
    pub code: ReasonCode,
    pub decided: bool,
    pub kind: ReasonKind,
    pub key: Option<CriterionKey>,
    pub params: Value,
    pub spans: Vec<Range<usize>>,
}

impl Finding {
    /// A violation when `decided`, else a check.
    pub(crate) fn new(
        code: ReasonCode,
        decided: bool,
        key: Option<CriterionKey>,
        params: Value,
        spans: Vec<Range<usize>>,
    ) -> Self {
        let kind = if decided {
            ReasonKind::Violation
        } else {
            ReasonKind::Check
        };
        Self {
            code,
            decided,
            kind,
            key,
            params,
            spans,
        }
    }

    /// A frame row that never excludes (met, partial or check).
    pub(crate) fn row(
        code: ReasonCode,
        kind: ReasonKind,
        key: Option<CriterionKey>,
        params: Value,
        spans: Vec<Range<usize>>,
    ) -> Self {
        Self {
            code,
            decided: false,
            kind,
            key,
            params,
            spans,
        }
    }
}

/// Job facts used for the criteria.
pub(crate) struct JobFacts<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub location: &'a str,
    pub portal: Portal,
    pub facts: Option<&'a Value>,
    pub posted: Option<Date>,
}

/// A sentence of the text: byte range and folded text.
pub(crate) type Segment = (Range<usize>, String);

/// Sentences of the text (also split at ` // `), as byte ranges with their folded text.
pub(crate) fn segments(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    for line in splitlines(text) {
        for part in line.split(" // ") {
            for sentence in sentences(part) {
                let start = (sentence.as_ptr() as usize).saturating_sub(text.as_ptr() as usize);
                out.push((start..start + sentence.len(), fold(sentence)));
            }
        }
    }
    out
}

pub(crate) fn fact<'a>(facts: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    facts.and_then(|f| f.get(key)).filter(|v| !v.is_null())
}

/// Findings of the contract type, ANÜ, country, day rate and availability.
pub(crate) fn check(
    criteria: &HardCriteria,
    job: &JobFacts<'_>,
    segments: &[Segment],
    folded: &str,
    contract: &Contract,
    anue_findings: Vec<Finding>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let (row, inferred) = match contract.kind {
        ContractKind::Interim => (ReasonKind::Met, false),
        ContractKind::Permanent => (ReasonKind::Partial, contract.inferred),
        ContractKind::Anue => (ReasonKind::Partial, false),
        ContractKind::Unclear => (ReasonKind::Check, false),
    };
    findings.push(Finding::row(
        ReasonCode::ContractType,
        row,
        None,
        json!({ "type": contract.kind.name(), "inferred": inferred }),
        contract.spans.clone(),
    ));
    if contract.kind == ContractKind::Permanent || contract.stated_permanent {
        findings.push(Finding::new(
            ReasonCode::Permanent,
            false,
            None,
            json!({}),
            Vec::new(),
        ));
    }
    if criteria.anue_excluded {
        if anue_findings.is_empty() && contract.kind == ContractKind::Unclear && contract.agency {
            findings.push(Finding::new(
                ReasonCode::AnueRisk,
                false,
                Some(CriterionKey::NoAnue),
                json!({}),
                contract.spans.clone(),
            ));
        }
        findings.extend(anue_findings);
    }
    if let Some(allowed) = &criteria.countries {
        findings.extend(country(criteria, allowed, job, segments, folded));
    }
    if contract.kind != ContractKind::Permanent {
        findings.extend(day_rate(criteria, job, segments));
    }
    if let Availability::From(date) = criteria.available {
        findings.extend(availability(date, job, segments));
    }
    findings
}

/// ANÜ named (decided), optional or hidden; empty when not mentioned.
pub(crate) fn anue(job: &JobFacts<'_>, segments: &[Segment]) -> Vec<Finding> {
    let any = |f: &str, words: &[&str], parts: &[&str]| {
        words.iter().any(|w| contains_word(f, w)) || parts.iter().any(|p| f.contains(p))
    };
    let named = |f: &str| any(f, lex::ANUE_WORDS, lex::ANUE_PARTS);
    let negated = |f: &str| any(f, lex::ANUE_NEGATION, lex::ANUE_NEGATION_PARTS);
    let optional = |f: &str| any(f, lex::ANUE_OPTION, lex::ANUE_OPTION_PARTS);
    let (mut decided, mut option, mut hidden) = (Vec::new(), Vec::new(), Vec::new());
    let contract = fact(job.facts, "contract")
        .and_then(Value::as_str)
        .map(fold);
    if let Some(f) = contract.as_deref().filter(|f| named(f) && !negated(f)) {
        if optional(f) {
            option.push(0..0);
        } else {
            decided.push(0..0);
        }
    }
    for (range, f) in segments {
        if named(f) {
            if negated(f) {
                continue;
            }
            if optional(f) {
                &mut option
            } else {
                &mut decided
            }
            .push(range.clone());
        } else if lex::ANUE_HIDDEN.iter().any(|w| contains_word(f, w)) {
            hidden.push(range.clone());
        }
    }
    let spans = |v: Vec<Range<usize>>| v.into_iter().filter(|r| !r.is_empty()).collect();
    if !decided.is_empty() {
        vec![Finding::new(
            ReasonCode::Anue,
            true,
            Some(CriterionKey::NoAnue),
            json!({}),
            spans(decided),
        )]
    } else if !option.is_empty() {
        vec![Finding::new(
            ReasonCode::AnueOptional,
            false,
            Some(CriterionKey::NoAnue),
            json!({}),
            spans(option),
        )]
    } else if !hidden.is_empty() {
        vec![Finding::new(
            ReasonCode::AnueHidden,
            false,
            Some(CriterionKey::NoAnue),
            json!({}),
            spans(hidden),
        )]
    } else {
        Vec::new()
    }
}

/// Countries named in a folded text (names and cities).
fn countries_in(folded: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = lex::COUNTRIES
        .iter()
        .chain(lex::CITIES)
        .filter(|(name, _)| contains_word(folded, name))
        .map(|&(_, code)| code)
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

fn location_countries(location: &str) -> Vec<&'static str> {
    let folded = fold(location);
    let named = countries_in(&folded);
    if !named.is_empty() {
        return named;
    }
    let german_code = folded
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .any(|w| {
            let digits = w.strip_prefix("d-").unwrap_or(w);
            digits.len() == 5 && digits.chars().all(|c| c.is_ascii_digit())
        });
    if german_code { vec!["DE"] } else { Vec::new() }
}

fn country(
    criteria: &HardCriteria,
    allowed: &[String],
    job: &JobFacts<'_>,
    segments: &[(Range<usize>, String)],
    folded: &str,
) -> Vec<Finding> {
    let outside = |codes: &[&'static str]| -> Vec<String> {
        codes
            .iter()
            .filter(|c| !allowed.iter().any(|a| a == *c))
            .map(|c| (*c).to_owned())
            .collect()
    };
    let facts_remote = fact(job.facts, "remotePercent").and_then(Value::as_u64) == Some(100);
    let location = fact(job.facts, "location")
        .and_then(Value::as_str)
        .unwrap_or(job.location);
    let remote_full = facts_remote
        || lex::FULL_REMOTE.iter().any(|w| folded.contains(w))
        || fold(location).starts_with("remote");
    let home_allowed = segments.iter().any(|(_, f)| {
        lex::REMOTE_FROM.iter().any(|w| f.contains(w))
            && countries_in(f)
                .iter()
                .any(|c| allowed.iter().any(|a| a == c))
    });
    let mut onsite: Vec<(&'static str, Range<usize>)> = Vec::new();
    let mut travel: Vec<(&'static str, Range<usize>)> = Vec::new();
    for (range, f) in segments {
        let codes = countries_in(f);
        if codes.is_empty() {
            continue;
        }
        if lex::ONSITE_WORDS.iter().any(|w| f.contains(w)) {
            onsite.extend(codes.iter().map(|c| (*c, range.clone())));
        } else if lex::TRAVEL_WORDS.iter().any(|w| contains_word(f, w)) {
            travel.extend(codes.iter().map(|c| (*c, range.clone())));
        }
    }
    let located = outside(&location_countries(location));
    let onsite_codes: Vec<&'static str> = onsite.iter().map(|(c, _)| *c).collect();
    let onsite_out = outside(&onsite_codes);
    let travel_codes: Vec<&'static str> = travel.iter().map(|(c, _)| *c).collect();
    let travel_out = outside(&travel_codes);
    let spans_of = |list: &[(&'static str, Range<usize>)], codes: &[String]| -> Vec<Range<usize>> {
        let mut spans: Vec<Range<usize>> = list
            .iter()
            .filter(|(c, _)| codes.iter().any(|o| o == c))
            .map(|(_, r)| r.clone())
            .collect();
        spans.dedup();
        spans
    };
    let unclear = |codes: Vec<String>, spans: Vec<Range<usize>>| {
        Finding::new(
            ReasonCode::CountryUnclear,
            false,
            Some(CriterionKey::Countries),
            json!({ "countries": codes }),
            spans,
        )
    };
    if !remote_full {
        let mut decided: Vec<String> = located.iter().chain(&onsite_out).cloned().collect();
        decided.sort();
        decided.dedup();
        if !decided.is_empty() {
            let params = json!({ "outside": decided, "allowed": allowed });
            return vec![Finding::new(
                ReasonCode::Country,
                true,
                Some(CriterionKey::Countries),
                params,
                spans_of(&onsite, &decided),
            )];
        }
        if !travel_out.is_empty() {
            return vec![unclear(travel_out.clone(), spans_of(&travel, &travel_out))];
        }
        return Vec::new();
    }
    if !onsite_out.is_empty() {
        return vec![unclear(onsite_out.clone(), spans_of(&onsite, &onsite_out))];
    }
    if !located.is_empty() && criteria.remote_outside == Some(false) && !home_allowed {
        return vec![unclear(located, Vec::new())];
    }
    if !travel_out.is_empty() {
        return vec![unclear(travel_out.clone(), spans_of(&travel, &travel_out))];
    }
    Vec::new()
}

/// A rate statement: highest amount, hourly or daily, EUR or not.
pub(crate) struct Rate {
    upper: u64,
    hourly: bool,
    currency: Option<&'static str>,
}

pub(crate) fn parse_rate(folded: &str) -> Option<Rate> {
    if !lex::RATE_WORDS.iter().any(|w| folded.contains(w))
        || lex::SALARY_WORDS.iter().any(|w| folded.contains(w))
    {
        return None;
    }
    let mut amounts = Vec::new();
    let bytes = folded.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut value: u64 = 0;
        while i < bytes.len() {
            let c = bytes[i];
            if c.is_ascii_digit() {
                value = value.saturating_mul(10).saturating_add(u64::from(c - b'0'));
                i += 1;
            } else if (c == b'.' || c == b',')
                && bytes
                    .get(i + 1..i + 4)
                    .is_some_and(|d| d.iter().all(u8::is_ascii_digit))
                && bytes.get(i + 4).is_none_or(|d| !d.is_ascii_digit())
            {
                i += 1; // thousands separator
            } else {
                break;
            }
        }
        // Decimals (",50", ",-") and percentages are no separate amounts.
        let percent = folded[i..].trim_start().starts_with('%');
        if !percent && value >= 20 {
            amounts.push(value);
        }
        if i < bytes.len() && (bytes[i] == b',' || bytes[i] == b'.') {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'-') {
                j += 1;
            }
            if j > i + 1 {
                i = j;
            }
        }
    }
    let upper = amounts.into_iter().max()?;
    let hourly = lex::HOURLY_WORDS.iter().any(|w| folded.contains(w));
    let currency = lex::OTHER_CURRENCIES
        .iter()
        .find(|w| folded.contains(**w))
        .copied();
    Some(Rate {
        upper,
        hourly,
        currency,
    })
}

fn day_rate(
    criteria: &HardCriteria,
    job: &JobFacts<'_>,
    segments: &[(Range<usize>, String)],
) -> Vec<Finding> {
    let from_facts = fact(job.facts, "rate")
        .and_then(Value::as_str)
        .and_then(|s| parse_rate(&fold(s)))
        .map(|r| (r, None));
    let rate = from_facts.or_else(|| {
        segments
            .iter()
            .find_map(|(range, f)| parse_rate(f).map(|r| (r, Some(range.clone()))))
    });
    let Some((rate, span)) = rate else {
        return Vec::new();
    };
    let spans: Vec<Range<usize>> = span.into_iter().collect();
    if let Some(currency) = rate.currency {
        let params = json!({ "currency": currency.to_uppercase(), "amount": rate.upper });
        return vec![Finding::new(
            ReasonCode::DayRateCurrency,
            false,
            Some(CriterionKey::MinDayRate),
            params,
            spans,
        )];
    }
    // `parse_rate` saturates absurd digit runs at `u64::MAX`: such an amount stays far
    // above any minimum instead of wrapping below it.
    let per_day = if rate.hourly {
        rate.upper.saturating_mul(HOURS_PER_DAY)
    } else {
        rate.upper
    };
    match criteria.min_rate {
        Some(min) if i128::from(per_day) < min => {
            let params = json!({ "rate": per_day, "min": min.to_string(), "hourly": rate.hourly });
            vec![Finding::new(
                ReasonCode::DayRate,
                true,
                Some(CriterionKey::MinDayRate),
                params,
                spans,
            )]
        }
        _ => Vec::new(),
    }
}

/// A job start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    Now,
    Date(Date),
    Vague,
}

/// Parses a start statement (`ab sofort`, `01.11.2026`, `11/2026`, `ab Januar 2027`, `Q1 2027`).
pub(crate) fn parse_start(text: &str) -> Option<Start> {
    let folded = fold(text);
    let words: Vec<&str> = folded
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '/')
        .map(|w| w.trim_matches('.'))
        .filter(|w| !w.is_empty())
        .collect();
    for (i, word) in words.iter().enumerate() {
        let parts: Vec<&str> = word.split(['.', '/']).collect();
        let number = |s: &str| s.parse::<i16>().ok();
        match parts.as_slice() {
            [d, m, y] if y.len() == 4 => {
                if let (Some(d), Some(m), Some(y)) = (number(d), number(m), number(y))
                    && let Ok(date) = Date::new(y, i8::try_from(m).ok()?, i8::try_from(d).ok()?)
                {
                    return Some(Start::Date(date));
                }
            }
            [m, y] if y.len() == 4 && m.len() <= 2 => {
                if let (Some(m), Some(y)) = (number(m), number(y))
                    && let Ok(date) = Date::new(y, i8::try_from(m).ok()?, 1)
                {
                    return Some(Start::Date(date));
                }
            }
            _ => {}
        }
        let year = words
            .get(i + 1)
            .and_then(|w| w.parse::<i16>().ok())
            .filter(|y| *y > 2000);
        if let Some(&(_, month)) = lex::MONTHS.iter().find(|(name, _)| name == word)
            && let Some(year) = year
            && let Ok(date) = Date::new(year, month, 1)
        {
            return Some(Start::Date(date));
        }
        if let Some(quarter) = word
            .strip_prefix('q')
            .and_then(|q| q.parse::<i8>().ok())
            .filter(|q| (1..=4).contains(q))
            && let Some(year) = year
            && let Ok(date) = Date::new(year, 3 * quarter - 2, 1)
        {
            return Some(Start::Date(date));
        }
    }
    if lex::START_NOW.iter().any(|w| contains_word(&folded, w)) {
        return Some(Start::Now);
    }
    lex::START_VAGUE
        .iter()
        .any(|w| folded.contains(w))
        .then_some(Start::Vague)
}

fn availability(
    available: Date,
    job: &JobFacts<'_>,
    segments: &[(Range<usize>, String)],
) -> Vec<Finding> {
    let from_facts = fact(job.facts, "start")
        .and_then(Value::as_str)
        .and_then(parse_start);
    let from_text: Vec<(Start, Range<usize>)> = segments
        .iter()
        .filter(|(_, f)| lex::START_WORDS.iter().any(|w| f.contains(w)))
        .filter_map(|(range, f)| parse_start(f).map(|s| (s, range.clone())))
        .collect();
    let key = Some(CriterionKey::Availability);
    let vague = |spans: Vec<Range<usize>>| {
        vec![Finding::new(
            ReasonCode::StartVague,
            false,
            key,
            json!({}),
            spans,
        )]
    };
    // Statements that disagree (facts "sofort", text "01.12.2026") make the start vague.
    let mut all: Vec<Start> = from_facts
        .into_iter()
        .chain(from_text.iter().map(|(s, _)| *s))
        .collect();
    all.dedup();
    if all.len() > 1 {
        return vague(from_text.into_iter().map(|(_, r)| r).collect());
    }
    let (start, spans) = match (from_facts, from_text.into_iter().next()) {
        (Some(f), _) => (f, Vec::new()),
        (None, Some((t, span))) => (t, vec![span]),
        (None, None) => return Vec::new(),
    };
    let start = match start {
        Start::Vague => return vague(spans),
        Start::Now => match job.posted {
            Some(posted) => posted,
            None => return Vec::new(),
        },
        Start::Date(date) => date,
    };
    let days = (available - start).get_days();
    if days > 0 {
        vec![Finding::new(
            ReasonCode::AvailabilityGap,
            false,
            key,
            json!({ "days": days }),
            spans,
        )]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates() {
        let r = parse_rate(&fold("Honorar: 900 - 1.100 € pro Tag")).unwrap();
        assert_eq!((r.upper, r.hourly), (1100, false));
        let r = parse_rate(&fold("Honorar: 95,- € pro Stunde")).unwrap();
        assert_eq!((r.upper, r.hourly), (95, true));
        assert_eq!(
            parse_rate(&fold("Day rate: 1,150 EUR")).unwrap().upper,
            1150
        );
        assert_eq!(
            parse_rate(&fold("Tagessatz: CHF 950 pro Tag"))
                .unwrap()
                .currency,
            Some("chf")
        );
        assert!(parse_rate(&fold("Jahresgehalt von 110.000 € pro Jahr")).is_none());
    }

    #[test]
    fn starts() {
        let d = |y, m, d| Some(Start::Date(Date::new(y, m, d).unwrap()));
        assert_eq!(parse_start("Start: 01.11.2026"), d(2026, 11, 1));
        assert_eq!(parse_start("Start: 11/2026"), d(2026, 11, 1));
        assert_eq!(parse_start("Start: ab Januar 2027"), d(2027, 1, 1));
        assert_eq!(parse_start("Start ab sofort"), Some(Start::Now));
        assert_eq!(parse_start("Start: nach Abstimmung"), Some(Start::Vague));
    }
}
