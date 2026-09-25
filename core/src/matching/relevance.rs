//! Relevance of a job for the profile, independent of explicit requirements:
//! title fit (V17) and a BM25F-like lexical score over title x3, requirement lines x2 and
//! the rest x1 with static specificity and a fixed document length (V18).

use std::collections::BTreeMap;
use std::ops::Range;

use super::atoms::{self, Fit, Vocab, fold};
use super::fit::Skills;
use super::lexicon::engine::{self as lex, KEY_USP};
use super::params::{
    BM25_B, BM25_K1, BM25_LENGTH, FIELD_REQUIREMENTS, FIELD_REST, FIELD_TITLE, GENERIC_WEIGHT,
    RELEVANCE_HALF, SPECIFIC_WEIGHT,
};

fn counts(text: &str, vocab: &Vocab) -> BTreeMap<String, u64> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        for atom in atoms::atoms(line, vocab) {
            *map.entry(atom).or_insert(0) += 1;
        }
    }
    map
}

fn occurrences(counts: &BTreeMap<String, u64>, profile_atom: &str) -> u64 {
    counts
        .iter()
        .filter(|(job, _)| matches!(atoms::fit(job, profile_atom), Fit::Equal | Fit::Specific))
        .map(|(_, n)| *n)
        .sum()
}

/// Distinct profile atoms with their static weight: specific 1000, generic 200; an atom
/// only named in a USP sentence counts half (free text is weaker evidence).
pub(crate) fn query(skills: &Skills) -> Vec<(String, u64)> {
    let mut atoms: Vec<(String, bool)> = skills
        .entries
        .iter()
        .flat_map(|e| {
            let usp = e.path.starts_with(KEY_USP);
            e.atoms.iter().map(move |a| (a.clone(), usp))
        })
        .collect();
    // Competence occurrences (`false`) sort first and win the dedup.
    atoms.sort();
    atoms.dedup_by(|b, a| a.0 == b.0);
    atoms
        .into_iter()
        .map(|(a, usp)| {
            let weight = if atoms::is_generic(&a) {
                GENERIC_WEIGHT
            } else if usp {
                SPECIFIC_WEIGHT / 2
            } else {
                SPECIFIC_WEIGHT
            };
            (a, weight)
        })
        .collect()
}

/// Title fit in per-mille: share of the title's content atoms the profile covers, scaled
/// by the static weight of the profile atom (specific 1000, USP-only 500).
pub(crate) fn title_fit(query: &[(String, u64)], title: &str, vocab: &Vocab) -> u64 {
    // Contract words (`Interim`, `Freelance`, `befristet`) say nothing about the field: they
    // count in the title but never match the profile (`Interim Management` in a profile
    // does not make every interim title fit).
    let contract = |a: &str| {
        lex::TITLE_CONTRACT_WORDS
            .iter()
            .any(|w| *w == a || atoms::stem(w) == a)
    };
    let title_atoms: Vec<String> = atoms::atoms(title, vocab)
        .into_iter()
        .filter(|a| !atoms::is_generic(a))
        .collect();
    if title_atoms.is_empty() {
        return 0;
    }
    let score: u64 = title_atoms
        .iter()
        .filter(|t| !contract(t))
        .map(|t| {
            query
                .iter()
                .map(|(p, weight)| {
                    let full = (*weight).clamp(SPECIFIC_WEIGHT / 2, SPECIFIC_WEIGHT);
                    match atoms::fit(t, p) {
                        Fit::Equal | Fit::Specific => full,
                        Fit::General => full / 2,
                        Fit::None => 0,
                    }
                })
                .max()
                .unwrap_or(0)
        })
        .sum();
    score / title_atoms.len() as u64
}

/// The title query: the profile's query plus the atoms of its target roles (a title that
/// names a target role fits).
pub(crate) fn title_query(query: &[(String, u64)], roles: &[String]) -> Vec<(String, u64)> {
    let mut out = query.to_vec();
    for atom in roles {
        if !out.iter().any(|(a, _)| a == atom) {
            out.push((atom.clone(), SPECIFIC_WEIGHT));
        }
    }
    out
}

/// A title without gender markers (`(m/w/d)`, `(all genders)`, `(f/m/d)`) and without a
/// marketing tail after a dash or bar that names no skill (`– remote`, `– Scale-up`).
pub(crate) fn clean_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut rest = title;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')').map(|c| open + c) else {
            break;
        };
        let inner = fold(&rest[open + 1..close]);
        let gender = lex::GENDER_MARKERS.iter().any(|m| inner.trim() == *m)
            || (!inner.trim().is_empty()
                && inner
                    .split(['/', '|', ',', ' ', '-'])
                    .filter(|p| !p.is_empty())
                    .all(|p| lex::GENDER_LETTERS.contains(&p)));
        out.push_str(&rest[..open]);
        if !gender {
            out.push_str(&rest[open..=close]);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    for sep in lex::TITLE_TAIL_SEPARATORS {
        if let Some((head, tail)) = out.split_once(sep)
            && !head.trim().is_empty()
            && !super::job::known_skill(tail)
        {
            out = head.to_owned();
        }
    }
    out.trim().to_owned()
}

/// Relevance `R = min(1000, R_lex + T/2)` in per-mille (`T` over the title query).
pub(crate) fn relevance(
    query: &[(String, u64)],
    title_query: &[(String, u64)],
    vocab: &Vocab,
    title: &str,
    text: &str,
    requirement_lines: &[Range<usize>],
) -> u64 {
    let title_counts = counts(title, vocab);
    let all = counts(text, vocab);
    let mut requirement_text = String::new();
    for range in requirement_lines {
        requirement_text.push_str(&text[range.clone()]);
        requirement_text.push('\n');
    }
    let requirement_counts = counts(&requirement_text, vocab);
    let length = text.chars().count() as u64;
    // Length normalisation in per-mille: 1 - b + b * len / avg.
    let norm = (1000 - BM25_B) + BM25_B * length / BM25_LENGTH;
    let mut mass: u64 = 0;
    for (atom, weight) in query {
        let in_title = occurrences(&title_counts, atom);
        let in_requirements = occurrences(&requirement_counts, atom);
        let in_rest = occurrences(&all, atom).saturating_sub(in_requirements);
        let tf =
            FIELD_TITLE * in_title + FIELD_REQUIREMENTS * in_requirements + FIELD_REST * in_rest;
        if tf == 0 {
            continue;
        }
        // BM25 saturation in per-mille: tf * (k1 + 1) / (tf + k1 * norm).
        let saturation = tf * (BM25_K1 + 1000) * 1_000_000 / (tf * 1_000_000 + BM25_K1 * norm);
        mass += weight * saturation / 1000;
    }
    let lexical = 1000 * mass / (mass + RELEVANCE_HALF);
    let title = title_fit(title_query, title, vocab);
    (lexical + title / 2).min(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A contract word in the title (`Interim`) never matches the profile, not even its
    /// `Interim Management`.
    #[test]
    fn title_contract_words_never_match() {
        let vocab = Vocab::all();
        let query = vec![("interim".to_owned(), 1000), ("controll".to_owned(), 1000)];
        assert_eq!(title_fit(&query, "Interim Controller", &vocab), 500);
        assert_eq!(title_fit(&query, "Controller", &vocab), 1000);
        assert_eq!(title_fit(&query, "Freelance Interim", &vocab), 0);
    }

    /// Gender markers and a marketing tail without a skill leave the title; a tail naming a
    /// skill stays.
    #[test]
    fn titles_without_markers_and_tails() {
        assert_eq!(clean_title("Head of IT (all genders)"), "Head of IT");
        assert_eq!(clean_title("Controller (f/m/d)"), "Controller");
        assert_eq!(
            clean_title("Leiter Controlling (m/w/d) – Festanstellung"),
            "Leiter Controlling"
        );
        assert_eq!(
            clean_title("Interim CFO – Scale-up E-Mobility"),
            "Interim CFO"
        );
        assert_eq!(
            clean_title("Senior Consultant – SAP FI/CO"),
            "Senior Consultant – SAP FI/CO"
        );
        assert_eq!(clean_title("Controller (Konzern)"), "Controller (Konzern)");
    }

    /// A title that names a target role fits, even without the role's words in the query.
    #[test]
    fn target_roles_join_the_title_query() {
        let vocab = Vocab::all();
        let query = vec![("controll".to_owned(), 1000)];
        let with_role = title_query(&query, &["werksleit".to_owned()]);
        assert_eq!(title_fit(&query, "Werksleiter", &vocab), 0);
        assert!(title_fit(&with_role, "Werksleiter", &vocab) > 0);
    }
}
