//! Rules for permanent roles (only when the profile sets them): minimum annual salary and
//! the region (places plus a minimum remote share). Decided only for a stated permanent
//! role with clear wording; an inferred permanent role, an unclear contract (an agency
//! without details) or unclear wording gives a check.
//!
//! Salary: EUR per year, the upper bound counts (`110.000 bis 130.000 €` -> 130,000),
//! `ab 100.000` is only a lower bound, monthly or foreign amounts are checks.
//! Region: a profile place in the location field or a location line (`Standort: Frankfurt
//! oder München`) is inside; a stated remote share of at least the minimum (`80 % remote`,
//! `fully remote`) accepts any place; hybrid wording or office days are no proof; a
//! location that names only the country is unclear.

use std::ops::Range;

use serde_json::{Value, json};

use super::atoms::fold;
use super::contract::{Contract, ContractKind};
use super::facts::{Finding, HardCriteria, JobFacts, Segment, fact};
use super::job::contains_word;
use super::lexicon::engine as lex;
use super::types::{CriterionKey, ReasonCode};

/// Does a rule for permanent roles apply, and may it decide?
fn scope(contract: &Contract) -> Option<bool> {
    match contract.kind {
        ContractKind::Permanent => Some(!contract.inferred),
        ContractKind::Unclear if contract.agency || contract.stated_permanent => Some(false),
        _ => None,
    }
}

/// A salary statement of one sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Salary {
    /// Highest amount; `None` when only a lower bound is given (`ab 100.000 €`).
    pub upper: Option<u64>,
    pub lower: u64,
    pub monthly: bool,
    pub currency: Option<&'static str>,
}

/// Amounts in a folded sentence: thousands separators, `k`/`TEUR` suffixes, no percentages.
fn amounts(folded: &str) -> Vec<(usize, u64)> {
    let bytes = folded.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() || (i > 0 && bytes[i - 1].is_ascii_alphabetic()) {
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
                i += 1;
            } else {
                break;
            }
        }
        let rest = folded[i..].trim_start();
        if rest.starts_with('%') {
            continue;
        }
        let thousands = lex::THOUSAND_SUFFIXES.iter().any(|s| rest.starts_with(s))
            || rest == "k"
            || rest.starts_with("k-");
        let value = if thousands {
            value.saturating_mul(1000)
        } else {
            value
        };
        out.push((start, value));
    }
    out
}

/// Parses a salary statement (a salary word and an amount of at least 10,000 per year or
/// 1,000 per month).
pub(crate) fn parse_salary(folded: &str) -> Option<Salary> {
    if !lex::SALARY_CUES.iter().any(|w| folded.contains(w)) {
        return None;
    }
    let monthly = lex::MONTHLY_WORDS.iter().any(|w| folded.contains(w));
    let floor = if monthly {
        lex::SALARY_MIN_AMOUNT / 10
    } else {
        lex::SALARY_MIN_AMOUNT
    };
    let found: Vec<(usize, u64)> = amounts(folded)
        .into_iter()
        .filter(|(_, v)| *v >= floor)
        .collect();
    let upper = found.iter().map(|(_, v)| *v).max()?;
    let lower = found.iter().map(|(_, v)| *v).min().unwrap_or(upper);
    let lower_only = found.len() == 1 && {
        let before = &folded[..found[0].0];
        let tail: String = before
            .chars()
            .rev()
            .take(24)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        lex::LOWER_BOUND_WORDS.iter().any(|w| tail.contains(w))
    };
    let currency = lex::OTHER_CURRENCIES
        .iter()
        .find(|w| folded.contains(**w))
        .copied();
    Some(Salary {
        upper: (!lower_only).then_some(upper),
        lower,
        monthly,
        currency,
    })
}

/// Minimum salary rule.
pub(crate) fn salary(
    criteria: &HardCriteria,
    contract: &Contract,
    segments: &[Segment],
) -> Vec<Finding> {
    let (Some(min), Some(may_decide)) = (criteria.min_salary, scope(contract)) else {
        return Vec::new();
    };
    let key = Some(CriterionKey::MinSalary);
    let Some((salary, span)) = segments
        .iter()
        .find_map(|(range, f)| parse_salary(f).map(|s| (s, range.clone())))
    else {
        return vec![Finding::new(
            ReasonCode::SalaryUnknown,
            false,
            key,
            json!({ "min": min }),
            Vec::new(),
        )];
    };
    let per_year = |v: u64| if salary.monthly { v * 12 } else { v };
    let shown = per_year(salary.upper.unwrap_or(salary.lower));
    if shown >= min {
        return Vec::new();
    }
    let decided =
        may_decide && salary.currency.is_none() && !salary.monthly && salary.upper.is_some();
    let mut params = json!({ "salary": shown, "min": min });
    if salary.upper.is_none() {
        params["lowerBound"] = json!(true);
    }
    if salary.monthly {
        params["monthly"] = json!(true);
    }
    if let Some(currency) = salary.currency {
        params["currency"] = json!(currency.to_uppercase());
    }
    vec![Finding::new(
        ReasonCode::Salary,
        decided,
        key,
        params,
        vec![span],
    )]
}

/// Percentages in a folded sentence.
fn percents(folded: &str) -> Vec<u64> {
    let bytes = folded.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut value: u64 = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            value = value
                .saturating_mul(10)
                .saturating_add(u64::from(bytes[i] - b'0'));
            i += 1;
        }
        if folded[i..].trim_start().starts_with('%') && value <= 100 {
            out.push(value);
        }
    }
    out
}

/// Words of a location that name a place (no country, state, postal code or work mode).
fn place_words(folded: &str) -> Vec<&str> {
    folded
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| w.len() > 1)
        .filter(|w| !w.chars().any(|c| c.is_ascii_digit()))
        .filter(|w| *w != "remote" && lex::LOCATION_NOISE.binary_search(w).is_err())
        .collect()
}

/// Region rule for permanent roles.
pub(crate) fn region(
    criteria: &HardCriteria,
    contract: &Contract,
    job: &JobFacts<'_>,
    segments: &[Segment],
    folded: &str,
) -> Vec<Finding> {
    let (Some(places), Some(may_decide)) = (&criteria.places, scope(contract)) else {
        return Vec::new();
    };
    let remote_min = criteria.remote_min.unwrap_or(100);
    let raw_location = fact(job.facts, "location")
        .and_then(Value::as_str)
        .unwrap_or(job.location)
        .trim();
    let location = fold(raw_location);
    // Proof of enough remote work.
    let facts_remote = fact(job.facts, "remotePercent").and_then(Value::as_u64);
    let remote_full = lex::FULL_REMOTE.iter().any(|w| folded.contains(w))
        || lex::REMOTE_FULL_EXTRA.iter().any(|w| folded.contains(w))
        || contains_word(&location, "remote");
    let remote_share = segments
        .iter()
        .filter(|(_, f)| lex::REMOTE_WORDS.iter().any(|w| f.contains(w)))
        .flat_map(|(_, f)| percents(f))
        .chain(facts_remote)
        .max();
    if remote_full || remote_share.is_some_and(|p| p >= remote_min) {
        return Vec::new();
    }
    let folded_places: Vec<String> = places.iter().map(|p| fold(p)).collect();
    let inside = |text: &str| folded_places.iter().any(|p| contains_word(text, p));
    let lines: Vec<&Segment> = segments
        .iter()
        .filter(|(_, f)| lex::LOCATION_LINES.iter().any(|w| f.starts_with(w)))
        .collect();
    if inside(&location) || lines.iter().any(|(_, f)| inside(f)) {
        return Vec::new();
    }
    let onsite: Vec<&Segment> = segments
        .iter()
        .filter(|(_, f)| lex::ONSITE_WORDS.iter().any(|w| f.contains(w)))
        .collect();
    if onsite.iter().any(|(_, f)| inside(f)) {
        return Vec::new();
    }
    // A named place outside the region: the location field, a location line, or a larger
    // city in an on-site sentence.
    let line_place = lines.iter().find(|(_, f)| {
        let rest = f.split_once(':').map_or("", |(_, r)| r);
        !place_words(rest).is_empty()
    });
    let city = onsite.iter().find_map(|(r, f)| {
        lex::GERMAN_CITIES
            .iter()
            .find(|c| contains_word(f, c))
            .map(|c| ((*c).to_owned(), r.clone()))
    });
    let quote = |range: &Range<usize>| job.text.get(range.clone()).unwrap_or("").trim().to_owned();
    let outside: Option<(String, Vec<Range<usize>>)> = if !place_words(&location).is_empty() {
        Some((raw_location.to_owned(), Vec::new()))
    } else if let Some((range, _)) = line_place {
        Some((quote(range), vec![range.clone()]))
    } else {
        city.map(|(_, r)| (quote(&r), vec![r]))
    };
    let key = Some(CriterionKey::PermanentRegion);
    match outside {
        Some((place, spans)) if contract.kind == ContractKind::Permanent => vec![Finding::new(
            ReasonCode::PermanentRegion,
            may_decide,
            key,
            json!({ "location": place, "remoteMin": remote_min, "remote": remote_share }),
            spans,
        )],
        Some((place, spans)) => vec![Finding::new(
            ReasonCode::PermanentRegionUnclear,
            false,
            key,
            json!({ "location": place }),
            spans,
        )],
        None => vec![Finding::new(
            ReasonCode::PermanentRegionUnclear,
            false,
            key,
            json!({ "location": raw_location }),
            Vec::new(),
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::contract::infer;
    use crate::matching::facts::{anue, segments};
    use crate::matching::profile;
    use crate::portal::Portal;

    /// Region findings of a LinkedIn ad for a profile with Munich and 60 % remote.
    fn region_of(location: &str, text: &str) -> Vec<(ReasonCode, bool)> {
        let data = serde_json::json!({ "harte_kriterien": {
            "festanstellung_orte": ["München", "Freising"],
            "festanstellung_remote_min": 60,
            "min_jahresgehalt": 150_000
        }});
        let criteria = HardCriteria::new(&profile::criteria(&data), &data);
        let job = JobFacts {
            title: "Head of Controlling (m/w/d)",
            text,
            location,
            portal: Portal::LinkedIn,
            facts: None,
            posted: None,
        };
        let segments = segments(text);
        let contract = infer(&job, &segments, &anue(&job, &segments));
        region(&criteria, &contract, &job, &segments, &fold(text))
            .into_iter()
            .chain(salary(&criteria, &contract, &segments))
            .map(|f| (f.code, f.decided))
            .collect()
    }

    #[test]
    fn permanent_region_and_salary() {
        let permanent = "Wir bieten eine unbefristete Festanstellung.";
        let with = |extra: &str| format!("{permanent}\n{extra}");
        assert_eq!(
            region_of("Frankfurt am Main, Hessen, Deutschland", permanent),
            [
                (ReasonCode::PermanentRegion, true),
                (ReasonCode::SalaryUnknown, false)
            ]
        );
        let hybrid = with("Hybrides Arbeiten mit bis zu drei Tagen mobilem Arbeiten pro Woche.");
        assert_eq!(
            region_of("Frankfurt am Main", &hybrid)[0],
            (ReasonCode::PermanentRegion, true)
        );
        let remote = with("Die Position ist zu 80 % remote möglich. Jahresgehalt 160.000 €.");
        assert!(region_of("Hamburg", &remote).is_empty());
        let or_munich = with("Standort: Frankfurt am Main oder München. Jahresgehalt 170.000 €.");
        assert!(region_of("Frankfurt am Main", &or_munich).is_empty());
        assert_eq!(
            region_of("Deutschland", permanent)[0],
            (ReasonCode::PermanentRegionUnclear, false)
        );
        let low = with("Jahresgehalt von 110.000 bis 130.000 € zuzüglich Bonus.");
        assert_eq!(region_of("Freising", &low), [(ReasonCode::Salary, true)]);
        let from = with("Gehalt ab 100.000 € brutto.");
        assert_eq!(region_of("München", &from), [(ReasonCode::Salary, false)]);
        let interim = "Interim-Mandat, Tagessatz 1.200 € pro Tag.";
        assert!(region_of("Hamburg", interim).is_empty());
    }

    #[test]
    fn salaries() {
        let s = |t: &str| parse_salary(&fold(t));
        let upper = |t: &str| s(t).and_then(|s| s.upper);
        assert_eq!(
            upper("Ein Jahresgehalt von 110.000 bis 130.000 € zuzüglich Bonus"),
            Some(130_000)
        );
        assert_eq!(
            upper("Zielgehalt von 160.000 – 190.000 € p. a."),
            Some(190_000)
        );
        assert_eq!(upper("Salary: 120k - 140k EUR"), Some(140_000));
        assert_eq!(upper("Vergütung bis zu 95 TEUR"), Some(95_000));
        let from = s("Gehalt ab 100.000 € brutto").unwrap();
        assert_eq!((from.upper, from.lower), (None, 100_000));
        assert!(s("Monatsgehalt 8.500 € brutto").unwrap().monthly);
        assert_eq!(s("Salary CHF 180,000").unwrap().currency, Some("chf"));
        assert!(s("30 Tage Urlaub und betriebliche Altersvorsorge").is_none());
        assert!(s("Eine Festanstellung in Vollzeit mit attraktiver Vergütung").is_none());
    }

    #[test]
    fn remote_shares() {
        assert_eq!(percents("die position ist zu 80 % remote moglich"), [80]);
        assert!(percents("bis zu drei tage mobiles arbeiten pro woche").is_empty());
        assert_eq!(
            place_words("frankfurt am main, hessen, deutschland"),
            ["frankfurt", "am", "main"]
        );
        assert!(place_words("deutschland").is_empty());
        assert!(place_words("80333 munchen").contains(&"munchen"));
    }
}
