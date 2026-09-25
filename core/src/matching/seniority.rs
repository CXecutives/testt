//! Target profile of an ad against the profile's minimum years (`zielprofil_min_jahre`),
//! for interim and permanent roles alike. Only the requirement lines count.
//!
//! Too junior (decided): a closed range whose upper bound is below the minimum
//! (`3-5 Jahre`), or a minimum below it (`mindestens 5 Jahre`, `3 Jahre Power BI`) without
//! a senior title: the ad's highest years are its target, whatever topic they name. An
//! open minimum with a senior title (`7+ years`, Senior/Lead/Head) never excludes; it is
//! only over-qualification (partial). A junior title without numbers is a check. Any
//! statement at or above the minimum makes the target senior enough.

use std::ops::Range;

use serde_json::json;

use super::atoms::{self, fold};
use super::facts::Finding;
use super::job::{JobDoc, contains_word};
use super::lexicon::engine as lex;
use super::types::{CriterionKey, ReasonCode, ReasonKind};

/// Years of experience in one requirement: lower bound and, for a closed range, the
/// upper bound.
pub(crate) fn experience_years(folded: &str) -> Option<(u32, Option<u32>)> {
    // `Min. 5 years in Regulatory Affairs` and `5+ years in internal audit` state years
    // without the word experience.
    if !lex::EXPERIENCE_WORDS.iter().any(|w| folded.contains(w))
        && !lex::MIN_MARKERS.iter().any(|w| folded.contains(w))
    {
        return None;
    }
    let text = folded.replace(['–', '—'], "-");
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | ':'))
        .filter(|w| !w.is_empty())
        .collect();
    // Years right after `davon` (`of which`) are a part of the total before them
    // (`Mehrjährige Erfahrung, davon mindestens drei Jahre in ...`): without a number in the
    // total the line states no minimum.
    let part = |i: usize| {
        (i.saturating_sub(3)..i).any(|k| {
            lex::YEARS_SUBSPAN.contains(&words[k])
                || (words[k] == "which" && k > 0 && words[k - 1] == "of")
        })
    };
    let number = |w: &str| -> Option<u32> {
        let w = w.trim_end_matches('+');
        w.parse::<u32>()
            .ok()
            .or_else(|| {
                lex::NUMBER_WORDS
                    .iter()
                    .find(|(word, _)| *word == w)
                    .map(|&(_, n)| n)
            })
            .filter(|n| (1..=40).contains(n))
    };
    let unit = |i: usize| {
        words
            .get(i)
            .is_some_and(|u| lex::YEAR_UNITS.iter().any(|y| u.starts_with(y)))
    };
    for (i, word) in words.iter().enumerate() {
        if let Some((a, b)) = word.split_once('-')
            && let (Some(a), Some(b)) = (number(a), number(b))
            && unit(i + 1)
        {
            return (!part(i)).then_some((a.min(b), Some(a.max(b))));
        }
        let Some(n) = number(word) else { continue };
        if let (Some(sep), Some(m)) = (words.get(i + 1), words.get(i + 2).and_then(|w| number(w)))
            && ["-", "bis", "to", "and"].contains(sep)
            && unit(i + 3)
        {
            return (!part(i)).then_some((n.min(m), Some(n.max(m))));
        }
        if unit(i + 1) {
            return (!part(i)).then_some((n, None));
        }
    }
    None
}

/// Senior wording in the title (whole words, or `...leiter`, `...leitung`).
pub(crate) fn senior_title(title: &str) -> bool {
    let folded = fold(title);
    atoms::raw_tokens(&folded).any(|t| {
        lex::SENIOR_TITLES.contains(&t)
            || ["leiter", "leiterin", "leitung"]
                .iter()
                .any(|end| t.ends_with(end))
    })
}

pub(crate) fn junior_title(title: &str) -> bool {
    let folded = fold(title);
    lex::JUNIOR_TITLES.iter().any(|w| contains_word(&folded, w))
}

/// The seniority rule; empty when the profile sets no minimum. `page_levels`: the career
/// level and employment type the page states in its own fields (folded) - an internship or
/// an entry-level role is too junior, an assistant or associate level a check, unless the
/// ad asks for the target's years somewhere.
pub(crate) fn check(
    target: Option<u32>,
    title: &str,
    text: &str,
    doc: &JobDoc,
    page_levels: &[String],
) -> Vec<Finding> {
    let Some(target) = target else {
        return Vec::new();
    };
    let key = Some(CriterionKey::TargetYears);
    let level_is = |values: &[&str]| {
        page_levels
            .iter()
            .find(|l| values.contains(&l.trim()))
            .cloned()
    };
    let statements: Vec<(u32, Option<u32>, Range<usize>)> = doc
        .requirement_lines
        .iter()
        .filter_map(|range| {
            let (min, max) = experience_years(&fold(text.get(range.clone())?))?;
            Some((min, max, range.clone()))
        })
        .collect();
    if statements.iter().any(|(min, ..)| *min >= target) {
        return Vec::new();
    }
    // The page's own career level: an internship or an entry-level role is decided.
    if let Some(level) = level_is(lex::ENTRY_LEVEL_VALUES) {
        return vec![Finding::new(
            ReasonCode::TooJunior,
            true,
            key,
            json!({ "target": target, "level": level }),
            Vec::new(),
        )];
    }
    let Some((min, max, range)) = statements.into_iter().max_by_key(|(min, ..)| *min) else {
        if let Some(level) = level_is(lex::LOW_LEVEL_VALUES) {
            return vec![Finding::new(
                ReasonCode::SeniorityUnclear,
                false,
                key,
                json!({ "target": target, "level": level }),
                Vec::new(),
            )];
        }
        return if junior_title(title) {
            vec![Finding::new(
                ReasonCode::SeniorityUnclear,
                false,
                key,
                json!({ "target": target, "junior": true }),
                Vec::new(),
            )]
        } else {
            Vec::new()
        };
    };
    let params = json!({ "years": min, "max": max, "target": target });
    match max {
        Some(max) if max >= target => Vec::new(),
        None if senior_title(title) => vec![Finding::row(
            ReasonCode::Overqualified,
            ReasonKind::Partial,
            key,
            params,
            vec![range],
        )],
        _ => vec![Finding::new(
            ReasonCode::TooJunior,
            true,
            key,
            params,
            vec![range],
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years_and_titles() {
        let y = |t: &str| experience_years(&fold(t));
        assert_eq!(
            y("3-5 Jahre Berufserfahrung im Controlling"),
            Some((3, Some(5)))
        );
        assert_eq!(y("6 – 8 Jahre Erfahrung"), Some((6, Some(8))));
        assert_eq!(y("7+ years of experience in FP&A"), Some((7, None)));
        assert_eq!(
            y("At least 5 years of professional experience"),
            Some((5, None))
        );
        assert_eq!(y("Mindestens zehn Jahre Berufserfahrung"), Some((10, None)));
        assert_eq!(y("between 3 and 5 years of experience"), Some((3, Some(5))));
        assert_eq!(y("Laufzeit 2 Jahre"), None);
        assert_eq!(y("Min. 5 years in Regulatory Affairs CMC"), Some((5, None)));
        assert_eq!(y("5+ years in internal audit"), Some((5, None)));
        assert_eq!(y("8+ Jahre im Controlling"), Some((8, None)));
        assert!(senior_title("Interim Senior Finance Manager FP&A (m/w/d)"));
        // A part of the total (`davon`) is no minimum of its own.
        assert_eq!(
            y("Langjährige Praxis im Einkauf, davon mindestens zwei Jahre in leitender Rolle"),
            None
        );
        assert_eq!(
            y("Mindestens 9 Jahre Berufserfahrung, davon 4 Jahre im Treasury"),
            Some((9, None))
        );
        assert_eq!(
            y("Several years of experience in tax, of which at least 3 years in transfer pricing"),
            None
        );
        assert_eq!(
            y("Solid experience in audit, including at least 2 years at a Big Four firm"),
            None
        );
        assert!(senior_title("Leiter Konzerncontrolling (Interim)"));
        assert!(senior_title(
            "Interim Leitung Projektcontrolling Anlagenbau"
        ));
        assert!(!senior_title("Finance Manager (m/f/d)"));
        assert!(!senior_title("Controller (m/w/d)"));
        assert!(junior_title("Junior Controller (m/w/d)"));
    }
}
