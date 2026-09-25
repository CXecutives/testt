//! Hard criteria of the new engine (V10-V13), structured facts first. A violation is
//! decided only on clear wording; everything unclear becomes a check:
//! ANÜ (named, not negated, not optional) · permanent employment (excluded by the profile:
//! a stated permanent role, not an inferred one or one that offers interim too) · work
//! country (location, on-site sentence, not fully remote) · day rate (EUR, upper bound,
//! hourly x 8, not for permanent roles) · availability (never decided: a gap or a vague
//! start is a check).

use std::collections::HashMap;
use std::ops::Range;

use std::sync::LazyLock;

use jiff::civil::Date;
use regex::Regex;
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
    /// `ausgeschlossene_vertragsarten` names permanent employment (`festanstellung`).
    pub permanent_excluded: bool,
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
    let entries: Vec<&str> = match value {
        Value::Array(items) => items
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()?,
        Value::String(text) => text
            .split([',', ';', '/'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect(),
        _ => return None,
    };
    let list = entries
        .iter()
        .map(|entry| country_code(entry))
        .collect::<Option<Vec<String>>>()?;
    (!list.is_empty()).then_some(list)
}

/// A country of the profile as its ISO code: a two-letter code as written ("UK" is GB), or a
/// country name the engine knows ("Deutschland", "Österreich", "Switzerland"); `None` for
/// anything else, so an unknown name is reported instead of excluding every country.
fn country_code(entry: &str) -> Option<String> {
    let text = entry.trim();
    if text.len() == 2 && text.chars().all(|c| c.is_ascii_alphabetic()) {
        let code = text.to_ascii_uppercase();
        return Some(if code == "UK" { "GB".to_owned() } else { code });
    }
    let name = fold(text);
    lex::COUNTRIES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, code)| (*code).to_owned())
}

/// Does a criteria value exclude temporary agency work (a list or a text naming ANÜ)?
pub(crate) fn excludes_anue(value: &Value) -> Option<bool> {
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

/// Words of `ausgeschlossene_vertragsarten` that exclude permanent employment (external
/// contract - do not translate: values of the profile, German and English).
const CONTRACT_PERMANENT: &[&str] = &["festanstellung", "permanent"];

/// Does a criteria value exclude permanent employment (a list or a text naming it)?
pub(crate) fn excludes_permanent(value: &Value) -> bool {
    let texts: Vec<String> = match value {
        Value::Array(items) => items.iter().filter_map(Value::as_str).map(fold).collect(),
        Value::String(text) => vec![fold(text)],
        _ => return false,
    };
    texts
        .iter()
        .any(|t| CONTRACT_PERMANENT.iter().any(|word| t.contains(word)))
}

/// The excluded contract types: temporary agency work and permanent employment. A value
/// that names neither in a readable way is reported as not understood.
fn excluded_contracts(
    legacy: &Criteria,
    data: &Value,
    not_understood: &mut Vec<(&'static str, String)>,
) -> (bool, bool) {
    let found = criterion(data, lexicon::KEYS_EXCLUDED_CONTRACTS);
    let anue = legacy.anue_excluded == Some(true)
        || found.is_some_and(|(key, value)| {
            let excluded = excludes_anue(value);
            if excluded.is_none() {
                not_understood.push((key, value.to_string()));
            }
            excluded == Some(true)
        });
    let permanent = found.is_some_and(|(_, value)| excludes_permanent(value));
    (anue, permanent)
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
        // The old reader keeps the entries as written (upper case): names become codes
        // here; a list with an unknown name goes the checked way below and is reported.
        let countries = legacy
            .countries
            .clone()
            .filter(|c| !c.is_empty())
            .and_then(|list| {
                list.iter()
                    .map(|entry| country_code(entry))
                    .collect::<Option<Vec<String>>>()
            })
            .or_else(|| {
                let (key, value) = criterion(data, lexicon::KEYS_COUNTRIES)?;
                let list = countries_of(value);
                if list.is_none() {
                    not_understood.push((key, value.to_string()));
                }
                list
            });
        let (anue_excluded, permanent_excluded) =
            excluded_contracts(legacy, data, &mut not_understood);
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
            permanent_excluded,
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

/// The ad without the other listings a portal shows under it ("Ähnliche Projekte (12)",
/// "Similar jobs"): the text up to the first line that is such a heading. The hard criteria
/// read only this part; the ad text itself stays whole.
pub(crate) fn own_text(text: &str) -> &str {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let folded = fold(line.trim());
        let heading = folded.chars().count() <= 48 && is_listings_heading(&folded);
        if heading && offset > 0 {
            return &text[..offset];
        }
        offset += line.len();
    }
    text
}

/// A folded line that heads the other listings of a portal: a heading of
/// `lexicon::OTHER_LISTINGS` as whole words, then nothing, a count (`(12)`), a colon or a
/// known tail (`anzeigen`, `dieses Anbieters`). A line that goes on is a sentence of the ad
/// (`Ähnliche Projekterfahrung von Vorteil`, `Weitere Projekte sind geplant.`).
pub(crate) fn is_listings_heading(folded: &str) -> bool {
    lex::OTHER_LISTINGS.iter().any(|heading| {
        let Some(tail) = folded.strip_prefix(heading) else {
            return false;
        };
        if tail.chars().next().is_some_and(char::is_alphanumeric) {
            return false;
        }
        let tail = tail.trim().trim_end_matches(':').trim_end();
        let count = tail.trim_start_matches('(').trim_end_matches(')');
        tail.is_empty()
            || (!count.is_empty() && count.chars().all(|c| c.is_ascii_digit()))
            || lex::LISTING_TAILS.contains(&tail)
    })
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
        findings.push(if criteria.permanent_excluded {
            // Excluded by the profile: decided only for a stated permanent role (not one
            // inferred from benefits or a title, not one that offers interim work too).
            let stated = contract.kind == ContractKind::Permanent && !contract.inferred;
            Finding::new(
                ReasonCode::Permanent,
                stated,
                Some(CriterionKey::NoPermanent),
                json!({ "excluded": true, "stated": stated }),
                contract.spans.clone(),
            )
        } else {
            Finding::new(ReasonCode::Permanent, false, None, json!({}), Vec::new())
        });
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
/// A country or city name with its country code.
type Place = (&'static str, &'static str);

/// Country and city names by their first word. A name stands in a text only where the
/// text has that first word as a word of its own (the names start with a letter, and a
/// match needs word boundaries), so a text is looked up word by word.
static PLACES_BY_FIRST_WORD: LazyLock<HashMap<&'static str, Vec<Place>>> = LazyLock::new(|| {
    let mut index: HashMap<&'static str, Vec<Place>> = HashMap::new();
    for &(name, code) in lex::COUNTRIES.iter().chain(lex::CITIES) {
        let first = name
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or(name);
        index.entry(first).or_default().push((name, code));
    }
    index
});

/// Every country or city a text names, with the byte offset of the name.
fn countries_at(folded: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for word in folded.split(|c: char| !c.is_alphanumeric()) {
        let Some(names) = PLACES_BY_FIRST_WORD.get(word) else {
            continue;
        };
        let at = word.as_ptr() as usize - folded.as_ptr() as usize;
        let rest = &folded[at..];
        for &(name, code) in names {
            let whole = rest
                .strip_prefix(name)
                .is_some_and(|after| after.chars().next().is_none_or(|c| !c.is_alphanumeric()));
            if whole {
                found.push((at, code));
            }
        }
    }
    found
}

fn countries_in(folded: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = countries_at(folded).into_iter().map(|(_, c)| c).collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// Where a sentence says on-site (`true`) or travel (`false`), by byte offset.
fn place_cues(folded: &str) -> Vec<(usize, bool)> {
    let mut cues: Vec<(usize, bool)> = lex::ONSITE_WORDS
        .iter()
        .flat_map(|w| folded.match_indices(w).map(|(at, _)| (at, true)))
        .chain(lex::TRAVEL_WORDS.iter().flat_map(|w| {
            folded
                .match_indices(w)
                .filter(|(at, _)| {
                    let before = folded[..*at].chars().next_back();
                    let after = folded[at + w.len()..].chars().next();
                    before.is_none_or(|c| !c.is_alphanumeric())
                        && after.is_none_or(|c| !c.is_alphanumeric())
                })
                .map(|(at, _)| (at, false))
        }))
        .collect();
    cues.sort_unstable();
    cues
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

/// A country with the sentence that names it.
type Named = (&'static str, Range<usize>);

/// The countries of on-site statements and of travel. A country belongs to the on-site or
/// travel statement before it (else the first one after it): `vor Ort in Düsseldorf,
/// gelegentlich Reisen nach Polen` works in Germany and travels to Poland. A frame line
/// naming the place (`Ort: 3199 Rotterdam, Niederlande`) is decided like the job location.
fn places_of_work(segments: &[(Range<usize>, String)]) -> (Vec<Named>, Vec<Named>) {
    let mut onsite: Vec<Named> = Vec::new();
    let mut travel: Vec<Named> = Vec::new();
    for (range, f) in segments {
        let named = countries_at(f);
        if named.is_empty() {
            continue;
        }
        let place = lex::PLACE_LABELS
            .iter()
            .any(|l| f.trim_start().starts_with(l));
        let cues = place_cues(f);
        let (mut here, mut away): (Vec<&'static str>, Vec<&'static str>) = (Vec::new(), Vec::new());
        for (at, code) in named {
            let cue = cues
                .iter()
                .rev()
                .find(|(c, _)| *c < at)
                .or_else(|| cues.iter().find(|(c, _)| *c > at))
                .map(|&(_, onsite)| onsite);
            match (place, cue) {
                (true, _) | (false, Some(true)) => here.push(code),
                (false, Some(false)) => away.push(code),
                (false, None) => {}
            }
        }
        for list in [&mut here, &mut away] {
            list.sort_unstable();
            list.dedup();
        }
        away.retain(|c| !here.contains(c));
        onsite.extend(here.into_iter().map(|c| (c, range.clone())));
        travel.extend(away.into_iter().map(|c| (c, range.clone())));
    }
    (onsite, travel)
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
    let (onsite, travel) = places_of_work(segments);
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
            .find_map(|(range, f)| rate_in(f).map(|r| (r, Some(range.clone()))))
    })
}

/// The rate of a sentence, clause by clause (`Freelance mit 90 € pro Stunde oder befristet
/// (Gehaltsband 72-84 T€ p.a.)`: the salary clause does not hide the rate); the highest per
/// day when clauses name several.
pub(crate) fn rate_in(folded: &str) -> Option<Rate> {
    folded
        .split([';', '(', ')'])
        .flat_map(|part| part.split(" oder "))
        .flat_map(|part| part.split(" or "))
        .filter_map(parse_rate)
        .max_by_key(Rate::per_day)
}

/// A currency next to a time unit, also with the amount between them (`110 EUR/h`, `EUR pro
/// Stunde`, `CHF/Tag`, LinkedIn's `€420/day`). Only a text
/// with one of `CURRENCY_MARKS` can match (the regex is checked after them).
static CURRENCY_PER_TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:€|\beur\b|\beuro\b|\bchf\b|\busd\b|\bgbp\b)\s*(?:\d[\d.,]*\s*)?(?:/|\bpro\b|\bper\b|\bje\b)\s*(?:h\b|hr\b|std\b|stunde|tag\b|day\b|hour|pt\b|mt\b)")
        .expect("currency per time")
});

/// Every currency of `CURRENCY_PER_TIME` contains one of these.
const CURRENCY_MARKS: &[&str] = &["€", "eur", "chf", "usd", "gbp"];

/// Can the engine read a rate from this text (a page's rate field)? A bare number is none:
/// without a unit it is no day or hourly rate.
pub(crate) fn readable_rate(text: &str) -> bool {
    parse_rate(&fold(text)).is_some()
}

pub(crate) fn parse_rate(folded: &str) -> Option<Rate> {
    let rate_word = lex::RATE_WORDS.iter().any(|w| folded.contains(w));
    let currency_per_time =
        || CURRENCY_MARKS.iter().any(|m| folded.contains(m)) && CURRENCY_PER_TIME.is_match(folded);
    if !(rate_word || currency_per_time()) || lex::SALARY_WORDS.iter().any(|w| folded.contains(w)) {
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
    let names = |f: &str, words: &[&str]| words.iter().any(|w| f.contains(w));
    // The value beats its label: `Stundensatz: Tagessatz 1.100 - 1.250 €` is a day rate.
    let value = folded.split_once(':').map_or(folded, |(_, v)| v);
    let hourly = names(folded, lex::HOURLY_WORDS)
        && (names(value, lex::HOURLY_WORDS) || !names(value, lex::DAILY_WORDS));
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
            // `EUR 1,100 to 1,250`: past the lower end of the range to its unit.
            b.trim_end_matches(|c: char| {
                c.is_whitespace() || c.is_ascii_digit() || matches!(c, ':' | '(' | '.' | ',')
            })
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
    fn country_names_in_a_profile_become_codes_and_unknown_ones_are_no_rule() {
        assert_eq!(
            countries_of(&json!(["Deutschland", "Österreich", "UK", "ch"])),
            Some(vec!["DE".into(), "AT".into(), "GB".into(), "CH".into()])
        );
        assert_eq!(
            countries_of(&json!("Germany, Switzerland")),
            Some(vec!["DE".into(), "CH".into()])
        );
        // An unknown name never turns into a code that excludes every job.
        assert_eq!(countries_of(&json!(["Deutschland", "Atlantis"])), None);
        assert_eq!(country_code("DEUTSCHLAND").as_deref(), Some("DE"));
    }

    #[test]
    fn the_other_listings_under_an_ad_are_no_part_of_it() {
        let text = "Vertragsart: Freiberuflich\nSAP FI/CO Berater, remote.\n\n\
                    Ähnliche Projekte (12)\nSAP CO Berater (m/w/d), Arbeitnehmerüberlassung\n\
                    Werkstudent Controlling";
        let own = own_text(text);
        assert_eq!(
            own,
            "Vertragsart: Freiberuflich\nSAP FI/CO Berater, remote.\n\n"
        );
        // The ANÜ of another listing excludes nothing.
        assert!(anue_codes(own).is_empty());
        assert_eq!(anue_codes(text), [(ReasonCode::Anue, true)]);
        // A requirement that names similar projects is no heading; the text stays whole.
        let whole = "Ähnliche Projekte im Mittelstand erfolgreich umgesetzt und begleitet, \
                     idealerweise mehrere davon";
        assert_eq!(own_text(whole), whole);
        // Nor is a short line that goes on after the heading's words, or a sentence.
        for line in [
            "Ähnliche Projekterfahrung von Vorteil",
            "Weitere Projekterfahrung wünschenswert",
            "Weitere Projekte sind bereits geplant.",
            "Andere Projekte im Konzern laufen parallel",
        ] {
            let text = format!("Ihr Profil\n{line}\nEinsatz über Arbeitnehmerüberlassung");
            assert_eq!(own_text(&text), text, "{line}");
        }
        // A heading with a count, a colon or a known tail is one.
        for heading in [
            "Ähnliche Projekte (12)",
            "Similar jobs:",
            "Ähnliche Projekte anzeigen",
            "Weitere Projekte dieses Anbieters",
        ] {
            let text = format!("Ihr Profil\n{heading}\nx");
            assert_eq!(own_text(&text), "Ihr Profil\n", "{heading}");
        }
        let english = "Contract: freelance\n\nSimilar jobs\nPayroll clerk (temporary agency work)";
        assert_eq!(own_text(english), "Contract: freelance\n\n");
        // A text that starts with such a heading keeps it (nothing before it).
        assert_eq!(own_text("Weitere Projekte\nx"), "Weitere Projekte\nx");
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
    fn a_denied_anue_never_excludes_in_english_either() {
        for text in [
            "We contract freelancers only; Arbeitnehmerüberlassung (ANÜ) is excluded.",
            "No temporary agency work.",
            "The engagement is not via ANÜ but under a service contract.",
            "A service contract without temporary agency work.",
            "Temporary agency work is ruled out for this mandate.",
        ] {
            assert!(anue_codes(text).is_empty(), "{text}");
        }
        assert_eq!(
            anue_codes("The assignment is temporary agency work via our staffing unit."),
            [(ReasonCode::Anue, true)]
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
        // A currency with a time unit is a rate in every spelling, without a rate word.
        assert_eq!(
            rate("Start: 15.10.2026 | 110 EUR/h | Remote: 90 %"),
            Some((110, true))
        );
        assert_eq!(
            parse_rate(&fold("110 EUR/h")).map(|r| (r.upper, r.hourly)),
            Some((110, true))
        );
        assert_eq!(parse_rate(&fold("950 CHF/Tag")).map(|r| r.upper), Some(950));
        // The salary clause of an either-or does not hide the rate.
        let r = rate_in(&fold(
            "Freelance mit 80–90 € pro Stunde (ca. 32 Std./Woche) oder befristete Anstellung \
             (Gehaltsband 72–84 T€ p.a. bei 40 h)",
        ))
        .unwrap();
        assert_eq!((r.upper, r.hourly), (90, true));
    }

    #[test]
    fn a_rate_range_reads_its_upper_end_in_every_spelling() {
        let rate = |s: &str| rate_in(&fold(s)).map(|r| (r.upper, r.hourly));
        // LinkedIn: the currency before each amount, the unit after it.
        assert_eq!(rate("€610/day - €680/day"), Some((680, false)));
        assert_eq!(rate("€85/hr - €95/hr"), Some((95, true)));
        // English range words between two amounts after one currency.
        assert_eq!(
            rate("Daily rate: EUR 1,050 to 1,180 plus expenses"),
            Some((1180, false))
        );
        assert_eq!(rate("Tagessatz 950 bis 1.050 €"), Some((1050, false)));
        // A portal's rate label is overruled by the value it holds.
        assert_eq!(
            rate("Stundensatz: Tagessatz 980 - 1.120 €"),
            Some((1120, false))
        );
        assert_eq!(rate("Stundensatz: 120 €"), Some((120, true)));
        // A salary chip stays no rate.
        assert_eq!(rate("€95,000/yr - €110,000/yr"), None);
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

    /// Country findings of an ad in Cologne for a profile that works in Germany.
    fn country_codes(text: &str) -> Vec<(ReasonCode, bool)> {
        let data = serde_json::json!({ "harte_kriterien": { "laender": ["DE"] } });
        let criteria = HardCriteria::new(&super::super::profile::criteria(&data), &data);
        let allowed = criteria.countries.clone().expect("countries");
        let job = JobFacts {
            title: "Projektleitung",
            text,
            location: "Köln",
            portal: Portal::LinkedIn,
            facts: None,
            posted: None,
        };
        country(&criteria, &allowed, &job, &segments(text), &fold(text))
            .into_iter()
            .map(|f| (f.code, f.decided))
            .collect()
    }

    /// Only the countries of the on-site statement are places of work; a business trip in
    /// the same sentence is at most a check.
    #[test]
    fn only_the_place_of_work_decides_the_country() {
        assert_eq!(
            country_codes(
                "Vier Tage pro Woche vor Ort in Düsseldorf, gelegentlich Reisen zu Standorten \
                 in Belgien, Polen und Spanien."
            ),
            [(ReasonCode::CountryUnclear, false)]
        );
        assert_eq!(
            country_codes("Einsatz vor Ort in Wien, gelegentlich Reisen nach Köln."),
            [(ReasonCode::Country, true)]
        );
        assert_eq!(
            country_codes("In Wien vor Ort."),
            [(ReasonCode::Country, true)]
        );
    }
}
