//! Target profile of an ad against the profile's minimum years (`zielprofil_min_jahre`),
//! for interim and permanent roles alike. Only the requirement lines count.
//!
//! Too junior (decided): a closed range whose upper bound is below the minimum
//! (`3-5 Jahre`), or a minimum below it (`mindestens 5 Jahre`) without a senior title.
//! An open minimum with a senior title (`7+ years`, Senior/Lead/Head) never excludes; it
//! is only over-qualification (partial). Years of one topic (`3 Jahre Power BI`) that is
//! not the role itself, or a junior title without numbers, are a check. Any statement at
//! or above the minimum makes the target senior enough.

use std::ops::Range;

use serde_json::json;

use super::atoms::{self, Vocab, fold};
use super::facts::Finding;
use super::job::{JobDoc, contains_word};
use super::lexicon::engine as lex;
use super::types::{CriterionKey, ReasonCode, ReasonKind};

/// Years of experience in one requirement: lower bound and, for a closed range, the
/// upper bound.
pub(crate) fn experience_years(folded: &str) -> Option<(u32, Option<u32>)> {
    if !lex::EXPERIENCE_WORDS.iter().any(|w| folded.contains(w)) {
        return None;
    }
    let text = folded.replace(['–', '—'], "-");
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | ':'))
        .filter(|w| !w.is_empty())
        .collect();
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
            return Some((a.min(b), Some(a.max(b))));
        }
        let Some(n) = number(word) else { continue };
        if let (Some(sep), Some(m)) = (words.get(i + 1), words.get(i + 2).and_then(|w| number(w)))
            && ["-", "bis", "to", "and"].contains(sep)
            && unit(i + 3)
        {
            return Some((n.min(m), Some(n.max(m))));
        }
        if unit(i + 1) {
            return Some((n, None));
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

fn junior_title(title: &str) -> bool {
    let folded = fold(title);
    lex::JUNIOR_TITLES.iter().any(|w| contains_word(&folded, w))
}

/// The seniority rule; empty when the profile sets no minimum.
pub(crate) fn check(
    target: Option<u32>,
    title: &str,
    text: &str,
    doc: &JobDoc,
    vocab: &Vocab,
) -> Vec<Finding> {
    let Some(target) = target else {
        return Vec::new();
    };
    let key = Some(CriterionKey::TargetYears);
    let title_atoms: Vec<String> = atoms::atoms(title, vocab)
        .into_iter()
        .filter(|a| !atoms::is_generic(a))
        .collect();
    let statements: Vec<(u32, Option<u32>, bool, Range<usize>)> = doc
        .requirement_lines
        .iter()
        .filter_map(|range| {
            let line = text.get(range.clone())?;
            let folded = fold(line);
            let (min, max) = experience_years(&folded)?;
            let career = lex::CAREER_WORDS.iter().any(|w| folded.contains(w))
                || atoms::atoms(line, vocab).iter().any(|a| {
                    title_atoms.iter().any(|t| {
                        matches!(atoms::fit(a, t), atoms::Fit::Equal | atoms::Fit::Specific)
                    })
                });
            Some((min, max, career, range.clone()))
        })
        .collect();
    if statements.iter().any(|(min, ..)| *min >= target) {
        return Vec::new();
    }
    let Some((min, max, career, range)) = statements.into_iter().max_by_key(|(min, ..)| *min)
    else {
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
        _ if career => vec![Finding::new(
            ReasonCode::TooJunior,
            true,
            key,
            params,
            vec![range],
        )],
        _ => vec![Finding::new(
            ReasonCode::SeniorityUnclear,
            false,
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
        assert!(senior_title("Interim Senior Finance Manager FP&A (m/w/d)"));
        assert!(senior_title("Leiter Konzerncontrolling (Interim)"));
        assert!(senior_title(
            "Interim Leitung Projektcontrolling Anlagenbau"
        ));
        assert!(!senior_title("Finance Manager (m/f/d)"));
        assert!(!senior_title("Controller (m/w/d)"));
        assert!(junior_title("Junior Controller (m/w/d)"));
    }
}
