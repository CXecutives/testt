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
pub(crate) fn number(value: &Value) -> Option<u64> {
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
        .filter(|c| !matches!(c, '.' | ',' | ' ' | '\''))
        .collect();
    digits.parse::<u64>().ok()?.checked_mul(factor)
}

/// Countries of a criteria value: a list, or one text (`DE` or `DE, AT`).
fn countries_of(value: &Value) -> Option<Vec<String>> {
    let list: Vec<String> = match value {
        Value::Array(items) => items
            .iter()
            .map(|v| v.as_str().map(|s| s.trim().to_uppercase()))
            .collect::<Option<Vec<String>>>()?,
        Value::String(text) => text
            .split([',', ';', '/'])
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => return None,
    };
    (!list.is_empty() && list.iter().all(|c| c.len() == 2)).then_some(list)
}

/// Does a criteria value exclude temporary agency work (a list or a text naming ANÜ)?
fn excludes_anue(value: &Value) -> Option<bool> {
    let texts: Vec<String> = match value {
        Value::Array(items) => items.iter().filter_map(Value::as_str).map(fold).collect(),
        Value::String(text) => vec![fold(text)],
        _ => return None,
    };
    Some(texts.iter().any(|t| {
        t.trim() == lexicon::CONTRACT_ANUE
            || lex::ANUE_PARTS.iter().any(|p| t.contains(p))
            || contains_word(t, "anu")
    }))
}

/// A yes or no of a criteria value (`true`, `"ja"`, `"yes"`).
fn yes_no(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::String(text) => match fold(text).trim() {
            "ja" | "yes" | "true" => Some(true),
            "nein" | "no" | "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

impl HardCriteria {
    pub(crate) fn new(legacy: &Criteria, data: &Value) -> Self {
        let mut not_understood = Vec::new();
        let available = match availability_text(data) {
            None => Availability::Unset,
            Some(text) => match parse_start(text) {
                Some(Start::Date(from)) => Availability::From(from),
                _ if fold(text).contains(lex::NOW_WORD) || fold(text).contains("now") => {
                    Availability::Now
                }
                _ => Availability::Unset,
            },
        };
        // The old keys also under `hard_criteria`, with their English names and values of
        // another type. The minimum rate, else (also for a minimum of 0) the old fallback
        // `einsatzpraeferenzen.tagessatz_ab`.
        let fallback = || {
            lexicon::KEY_PREFERENCES_ALIASES
                .iter()
                .find_map(|s| data.get(*s)?.get(lexicon::KEY_RATE_FROM))
                .and_then(number)
                .filter(|n| *n > 0)
                .map(i128::from)
        };
        let min_rate = match criterion(data, lexicon::KEYS_MIN_RATE) {
            None => legacy.min_day_rate.filter(|m| *m > 0).or_else(fallback),
            Some((key, value)) => match number(value) {
                Some(n) if n > 0 => Some(i128::from(n)),
                Some(_) => fallback(),
                None => {
                    if value.as_str().is_none_or(|s| !s.trim().is_empty()) {
                        not_understood.push((key, value.to_string()));
                    }
                    fallback()
                }
            },
        };
        let countries = legacy
            .countries
            .clone()
            .filter(|c| !c.is_empty())
            .or_else(|| {
                let (key, value) = criterion(data, lexicon::KEYS_COUNTRIES)?;
                let list = countries_of(value);
                if list.is_none() {
                    not_understood.push((key, value.to_string()));
                }
                list
            });
        let anue_excluded = legacy.anue_excluded == Some(true)
            || criterion(data, lexicon::KEYS_EXCLUDED_CONTRACTS).is_some_and(|(key, value)| {
                let excluded = excludes_anue(value);
                if excluded.is_none() {
                    not_understood.push((key, value.to_string()));
                }
                excluded == Some(true)
            });
        let remote_outside = legacy.remote_outside_allowed.or_else(|| {
            let (key, value) = criterion(data, lexicon::KEYS_REMOTE_OUTSIDE)?;
            let allowed = yes_no(value);
            if allowed.is_none() {
                not_understood.push((key, value.to_string()));
            }
            allowed
        });
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
            min_rate,
            countries,
            remote_outside,
            anue_excluded,
            available,
            min_salary,
            places,
            remote_min,
            target_years,
            not_understood,
        }
    }
}

/// Keys of the criteria sections the engine does not read (a typo, an unknown rule).
pub(crate) fn ignored_criteria_keys(data: &Value) -> Vec<String> {
    lexicon::KEY_CRITERIA_ALIASES
        .iter()
        .filter_map(|section| data.get(*section)?.as_object())
        .flat_map(|map| map.keys())
        .filter(|key| {
            !lexicon::KEYS_ALL_CRITERIA
                .iter()
                .any(|keys| keys.contains(&key.as_str()))
        })
        .cloned()
        .collect()
}

/// The profile's availability text (hard criteria first, then preferences; German keys
/// and sections first, English after).
pub(crate) fn availability_text(data: &Value) -> Option<&str> {
    lexicon::KEY_CRITERIA_ALIASES
        .iter()
        .chain(lexicon::KEY_PREFERENCES_ALIASES)
        .flat_map(|section| {
            lexicon::KEYS_AVAILABLE
                .iter()
                .map(move |key| (section, key))
        })
        .filter_map(|(section, key)| data.get(*section)?.get(*key)?.as_str())
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

/// Parts of a line between the separators of `SEGMENT_SEPARATORS`.
fn line_parts(line: &str) -> Vec<&str> {
    let mut parts = vec![line];
    for sep in lex::SEGMENT_SEPARATORS {
        parts = parts.into_iter().flat_map(|p| p.split(sep)).collect();
    }
    parts
}

/// Sentences of the text (also split at ` // `, ` · `, ` | `, ` • `), as byte ranges with
/// their folded text.
pub(crate) fn segments(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    for line in splitlines(text) {
        for part in line_parts(line) {
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
    let contract = fact(job.facts, super::fact_key::CONTRACT)
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
    // An ad that offers ANÜ as one option somewhere is optional, even where another
    // sentence names it plainly (`bei ANÜ entsprechender Stundenlohn`).
    if !decided.is_empty() && option.is_empty() {
        vec![Finding::new(
            ReasonCode::Anue,
            true,
            Some(CriterionKey::NoAnue),
            json!({}),
            spans(decided),
        )]
    } else if !option.is_empty() {
        option.extend(decided);
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

pub(crate) fn location_countries(location: &str) -> Vec<&'static str> {
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
    let facts_remote =
        fact(job.facts, super::fact_key::REMOTE_PERCENT).and_then(Value::as_u64) == Some(100);
    let location = fact(job.facts, super::fact_key::LOCATION)
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
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rate {
    pub upper: u64,
    pub hourly: bool,
    pub currency: Option<&'static str>,
}

impl Rate {
    /// The rate per day (an hourly rate times eight); absurd digit runs saturate.
    pub(crate) fn per_day(&self) -> u64 {
        if self.hourly {
            self.upper.saturating_mul(HOURS_PER_DAY)
        } else {
            self.upper
        }
    }
}

/// The rate the ad states: the page facts first, then the first sentence with a rate (and
/// its range).
pub(crate) fn stated_rate(
    job: &JobFacts<'_>,
    segments: &[Segment],
) -> Option<(Rate, Option<Range<usize>>)> {
    let from_facts = fact(job.facts, super::fact_key::RATE)
        .and_then(Value::as_str)
        .and_then(|s| parse_rate(&fold(s)))
        .map(|r| (r, None));
    from_facts.or_else(|| {
        segments
            .iter()
            .find_map(|(range, f)| parse_rate(f).map(|r| (r, Some(range.clone()))))
    })
}

/// Can the engine read a rate from this text (a page's rate field)? A bare number is none:
/// without a unit it is no day or hourly rate.
pub(crate) fn readable_rate(text: &str) -> bool {
    parse_rate(&fold(text)).is_some()
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
        let start = i;
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
        let date = date_part(bytes, start, i);
        // Decimals (",50", ",-") and percentages are no separate amounts.
        if i < bytes.len() && (bytes[i] == b',' || bytes[i] == b'.') && !date {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'-') {
                j += 1;
            }
            if j > i + 1 {
                i = j;
            }
        }
        let percent = folded[i..].trim_start().starts_with('%');
        // Only an amount next to a currency or a rate word is a rate (`Start: 02/2027 ·
        // 78 €/h` is 78, a postcode or a year is none).
        // Next to a currency even a student's wage counts (`16,50 € pro Stunde`).
        if !percent && !date && value >= MIN_RATE_AMOUNT && rate_context(folded, start, i) {
            amounts.push(value);
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

/// Smallest amount read as a rate (a rate is always next to a currency or rate word).
const MIN_RATE_AMOUNT: u64 = 5;

/// Is the number at `start..end` part of a date (`02/2027`, `01.11.2026`)?
fn date_part(bytes: &[u8], start: usize, end: usize) -> bool {
    let digit_at = |i: usize| bytes.get(i).is_some_and(u8::is_ascii_digit);
    // Digits joined by `/` or `.` before the number: `11/2026`, `01.11.2026`.
    let joined_before =
        start >= 2 && matches!(bytes[start - 1], b'/' | b'.') && digit_at(start - 2);
    // `02/2027`, `01.11.`: digits after `/`, or two digits and a dot after `.` (no
    // thousands group such as `1.100`, no decimals such as `95.50`).
    let joined_after = match bytes.get(end) {
        Some(b'/') => digit_at(end + 1),
        Some(b'.') => digit_at(end + 1) && digit_at(end + 2) && bytes.get(end + 3) == Some(&b'.'),
        _ => false,
    };
    joined_before || joined_after
}

/// Does a currency or rate unit follow the amount (after a range such as `- 1.100`), or a
/// currency or rate word precede it (`Tagessatz: bis`, `EUR`)?
fn rate_context(folded: &str, start: usize, end: usize) -> bool {
    let after = folded.get(end..).unwrap_or("");
    let rest = after.trim_start_matches(|c: char| {
        c.is_whitespace() || c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '–')
    });
    let rest = lex::RATE_RANGE_WORDS
        .iter()
        .find_map(|w| rest.strip_prefix(w))
        .map_or(rest, |r| {
            r.trim_start_matches(|c: char| {
                c.is_whitespace() || c.is_ascii_digit() || matches!(c, '.' | ',' | '-')
            })
        });
    if lex::RATE_UNITS.iter().any(|u| rest.starts_with(u)) {
        return true;
    }
    // The upper end of a range looks past the lower one (`EUR 950–1,100`).
    let before = folded
        .get(..start)
        .unwrap_or("")
        .trim_end_matches(|c: char| {
            c.is_whitespace()
                || c.is_ascii_digit()
                || matches!(c, ':' | '(' | '~' | '-' | '–' | '.' | ',')
        });
    let before = lex::RATE_RANGE_WORDS
        .iter()
        .find_map(|w| before.strip_suffix(w))
        .map_or(before, |b| {
            b.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '('))
        });
    lex::RATE_UNITS
        .iter()
        .chain(lex::RATE_WORDS)
        .any(|w| before.ends_with(w))
}

fn day_rate(
    criteria: &HardCriteria,
    job: &JobFacts<'_>,
    segments: &[(Range<usize>, String)],
) -> Vec<Finding> {
    let Some((rate, span)) = stated_rate(job, segments) else {
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
    let per_day = rate.per_day();
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
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '/' && c != '-')
        .map(|w| w.trim_matches(['.', '-']))
        .filter(|w| !w.is_empty())
        .collect();
    for (i, word) in words.iter().enumerate() {
        let parts: Vec<&str> = word.split(['.', '/', '-']).collect();
        let year_of = |s: &str| s.parse::<i16>().ok().filter(|_| s.len() == 4);
        let small = |s: &str| s.parse::<i8>().ok().filter(|_| s.len() <= 2);
        // A part that is no date (a phone number `0170.1234.5678`) is skipped, it does not
        // end the search.
        let date = match parts.as_slice() {
            // ISO: 2026-11-01.
            [y, m, d] if word.contains('-') => year_of(y)
                .zip(small(m))
                .zip(small(d))
                .and_then(|((y, m), d)| Date::new(y, m, d).ok()),
            [d, m, y] => year_of(y)
                .zip(small(m))
                .zip(small(d))
                .and_then(|((y, m), d)| Date::new(y, m, d).ok()),
            [m, y] => year_of(y)
                .zip(small(m))
                .and_then(|(y, m)| Date::new(y, m, 1).ok()),
            _ => None,
        };
        if let Some(date) = date {
            return Some(Start::Date(date));
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
    let from_facts = fact(job.facts, super::fact_key::START)
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
        // A start before the ad was posted means now.
        Start::Date(date) => match job.posted {
            Some(posted) if date < posted => posted,
            _ => date,
        },
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

    fn anue_codes(text: &str) -> Vec<(ReasonCode, bool)> {
        let job = JobFacts {
            title: "Controller",
            text,
            location: "",
            portal: Portal::LinkedIn,
            facts: None,
            posted: None,
        };
        anue(&job, &segments(text))
            .into_iter()
            .map(|f| (f.code, f.decided))
            .collect()
    }

    #[test]
    fn anue_decided_only_without_an_option_or_a_distinction() {
        assert_eq!(
            anue_codes("Die Besetzung erfolgt im Rahmen der Arbeitnehmerüberlassung."),
            [(ReasonCode::Anue, true)]
        );
        assert!(
            anue_codes("Wir achten auf eine saubere Abgrenzung zur Arbeitnehmerüberlassung.")
                .is_empty()
        );
        assert_eq!(
            anue_codes(
                "Vertragsart: Freiberuflich oder Arbeitnehmerüberlassung.\n\
                 Bei ANÜ gilt ein entsprechender Stundenlohn."
            ),
            [(ReasonCode::AnueOptional, false)]
        );
    }

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
        // Only amounts next to a currency or a rate word: no year, date or postcode.
        let rate = |s: &str| parse_rate(&fold(s)).map(|r| (r.upper, r.hourly));
        assert_eq!(
            rate("Start: 02/2027 · Dauer: 10 Monate · 80 % · 78 €/h zzgl. MwSt."),
            Some((78, true))
        );
        assert_eq!(rate("Tagessatz: bis 1.100"), Some((1100, false)));
        assert_eq!(rate("Tagessatz ab 01.11.2026: 950 EUR"), Some((950, false)));
        assert_eq!(rate("80331 München, Tagessatz 900 €"), Some((900, false)));
        assert_eq!(rate("Stundensatz: 95,50 €"), Some((95, true)));
        assert_eq!(
            rate("Day rate: EUR 950–1,100 depending on experience"),
            Some((1100, false))
        );
        assert_eq!(rate("Honorar nach Absprache, Laufzeit bis 2027"), None);
        assert_eq!(rate("16,50 € pro Stunde"), Some((16, true)));
    }

    #[test]
    fn starts() {
        let d = |y, m, d| Some(Start::Date(Date::new(y, m, d).unwrap()));
        assert_eq!(parse_start("Start: 01.11.2026"), d(2026, 11, 1));
        assert_eq!(parse_start("Start: 11/2026"), d(2026, 11, 1));
        assert_eq!(parse_start("Start: ab Januar 2027"), d(2027, 1, 1));
        assert_eq!(parse_start("Start ab sofort"), Some(Start::Now));
        assert_eq!(parse_start("Start: nach Abstimmung"), Some(Start::Vague));
        assert_eq!(parse_start("2026-11-01"), d(2026, 11, 1));
        // A number that is no date does not end the search.
        assert_eq!(
            parse_start("Start: ab sofort, Rückfragen unter 0170.1234.5678"),
            Some(Start::Now)
        );
        assert_eq!(parse_start("Start: 31.02.2026"), None);
    }

    #[test]
    fn criteria_under_english_keys_and_other_types() {
        let read =
            |data: Value| HardCriteria::new(&crate::matching::profile::criteria(&data), &data);
        let english = read(serde_json::json!({ "hard_criteria": {
            "min_day_rate": "1.050 €",
            "countries": "DE, CH",
            "excluded_contract_types": ["ANÜ"],
            "remote_outside_allowed": "nein",
            "available_from": "2026-11-01"
        }}));
        assert_eq!(english.min_rate, Some(1050));
        assert_eq!(
            english.countries,
            Some(vec!["DE".to_owned(), "CH".to_owned()])
        );
        assert!(english.anue_excluded);
        assert_eq!(english.remote_outside, Some(false));
        assert_eq!(
            english.available,
            Availability::From(Date::new(2026, 11, 1).unwrap())
        );
        let german = read(serde_json::json!({ "hard_criteria": {
            "min_tagessatz": 1050, "laender": ["DE"], "ausgeschlossene_vertragsarten": ["anue"]
        }}));
        assert_eq!(german.min_rate, Some(1050));
        assert!(german.anue_excluded);
        let odd = read(serde_json::json!({ "harte_kriterien": {
            "min_tagessatz": "viel", "min_jahresgehalt": "99999999999999999k", "tagessatz_max": 2
        }}));
        let keys: Vec<&str> = odd.not_understood.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, ["min_tagessatz", "min_jahresgehalt"]);
        assert_eq!(
            ignored_criteria_keys(
                &serde_json::json!({ "harte_kriterien": { "tagessatz_max": 2 } })
            ),
            ["tagessatz_max"]
        );
    }
}
