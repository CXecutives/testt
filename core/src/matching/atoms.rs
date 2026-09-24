//! Skill atoms of the new engine: case-folded, umlauts folded, fillers removed, light
//! stemming (V1), bilingual concepts (V3), and the fit of two atoms including compounds
//! (V2) and specific/general relations (V4).

use std::sync::LazyLock;

use super::lexicon::{self, engine as lex};
use super::normalize::casefold;

/// Case-folded, umlauts folded (`Übersicht` -> `ubersicht`, `ß` -> `ss`).
pub(crate) fn fold(text: &str) -> String {
    casefold(text)
        .chars()
        .map(|c| match c {
            'ä' => 'a',
            'ö' => 'o',
            'ü' => 'u',
            other => other,
        })
        .collect()
}

static STOP: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut words: Vec<String> = lexicon::STOPWORDS.iter().map(|w| fold(w)).collect();
    words.extend(lex::FILLERS.iter().map(|w| (*w).to_owned()));
    words.sort();
    words.dedup();
    words
});

/// Concept table with stemmed keys and values; longest keys first.
static CONCEPTS: LazyLock<Vec<(Vec<String>, String)>> = LazyLock::new(|| {
    let mut table: Vec<(Vec<String>, String)> = lex::CONCEPTS
        .iter()
        .map(|(key, value)| (key.split(' ').map(stem).collect(), stem(value)))
        .collect();
    table.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    table
});

/// Is `word` a stopword or filler (folded form)?
pub(crate) fn is_filler(word: &str) -> bool {
    STOP.binary_search_by(|w| w.as_str().cmp(word)).is_ok()
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '.' | '/' | '-' | 'ß')
}

/// Raw folded tokens (the old token class), trimmed of trailing `.` and `-`.
pub(crate) fn raw_tokens(folded: &str) -> impl Iterator<Item = &str> {
    folded
        .split(|c: char| !is_token_char(c))
        .map(|t| t.trim_matches(|c| c == '.' || c == '-' || c == '/'))
        .filter(|t| !t.is_empty())
}

/// Light, symmetric stemmer for German and English (V1). Hyphenated words are stemmed
/// part by part.
pub(crate) fn stem(word: &str) -> String {
    if word.contains('-') {
        return word.split('-').map(stem_word).collect::<Vec<_>>().join("-");
    }
    stem_word(word)
}

/// Two passes, so `reportings` and `reporting` meet in `report`.
fn stem_word(word: &str) -> String {
    let once = stem_once(word);
    stem_once(&once)
}

fn stem_once(word: &str) -> String {
    const RULES: &[(&str, &str)] = &[
        ("ungen", "ung"),
        ("heiten", "heit"),
        ("keiten", "keit"),
        ("ies", "y"),
        ("ing", ""),
        ("ern", ""),
        ("en", ""),
        ("er", ""),
        ("es", ""),
        ("ed", ""),
        ("e", ""),
        ("s", ""),
    ];
    if word.len() < 5 || !word.is_ascii() {
        return word.to_owned();
    }
    for (suffix, replacement) in RULES {
        if let Some(base) = word.strip_suffix(suffix) {
            if *suffix == "s" && (base.ends_with('s') || base.ends_with('i') || base.ends_with('u'))
            {
                continue;
            }
            if base.len() >= 4 {
                return format!("{base}{replacement}");
            }
        }
    }
    word.to_owned()
}

/// Atoms of a text: tokens without fillers, stemmed, concepts applied.
pub(crate) fn atoms(text: &str) -> Vec<String> {
    let folded = fold(text);
    let stems: Vec<String> = raw_tokens(&folded)
        .filter(|t| t.len() >= 3 || lexicon::synonym(t).is_some())
        .filter_map(|t| match lexicon::synonym(t) {
            Some(Some(mapped)) => Some(mapped.to_owned()),
            Some(None) => None,
            None => Some(t.to_owned()),
        })
        .filter(|t| !is_filler(t))
        .filter(|t| t.chars().any(|c| c.is_ascii_alphabetic()))
        .flat_map(|t| {
            let t = t.as_str();
            // Hyphenated words that form a known concept count as that concept.
            let spaced: Vec<String> = t.split('-').map(stem).collect();
            if spaced.len() > 1 && concept_at(&spaced, 0).is_some_and(|(n, _)| n == spaced.len()) {
                spaced
            } else {
                vec![stem(t)]
            }
        })
        .collect();
    let mut result = Vec::with_capacity(stems.len());
    let mut i = 0;
    while i < stems.len() {
        if let Some((len, concept)) = concept_at(&stems, i) {
            result.push(concept);
            i += len;
        } else {
            result.push(stems[i].clone());
            i += 1;
        }
    }
    result.dedup();
    result
}

fn concept_at(stems: &[String], at: usize) -> Option<(usize, String)> {
    CONCEPTS.iter().find_map(|(key, value)| {
        let end = at + key.len();
        (end <= stems.len() && stems[at..end] == key[..]).then(|| (key.len(), value.clone()))
    })
}

/// Is the atom too generic to meet a requirement on its own?
pub(crate) fn is_generic(atom: &str) -> bool {
    lexicon::contains(lex::GENERIC_ATOMS, atom)
}

/// Relation of a job atom to a profile atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Fit {
    None,
    /// The job atom narrows the profile atom (`Finanzcontrolling` vs `Controlling`).
    General,
    /// The profile atom narrows the job atom (`SAP-Projektleitung` vs `Projektleitung`).
    Specific,
    /// Same skill, also through a light compound part (`Konzernkonsolidierung`).
    Equal,
}

/// A compound part without the linking `s` that does not change the skill.
fn light(part: &str, table: &[&str]) -> bool {
    let part = part.trim_matches('-');
    let candidates = [
        part,
        part.strip_suffix('s').unwrap_or(part),
        part.strip_prefix('s').unwrap_or(part),
    ];
    part.is_empty()
        || candidates
            .iter()
            .any(|p| lexicon::contains(table, p) || lexicon::contains(table, &stem(p)))
}

/// How a job atom relates to a profile atom (V2 compounds, V4 specific/general).
pub(crate) fn fit(job: &str, profile: &str) -> Fit {
    if job == profile {
        return Fit::Equal;
    }
    if profile.len() >= 4 {
        if let Some(modifier) = job.strip_suffix(profile) {
            return if light(modifier, lex::LIGHT_MODIFIERS) {
                Fit::Equal
            } else {
                Fit::General
            };
        }
        if let Some(head) = job.strip_prefix(profile) {
            return if light(head, lex::LIGHT_HEADS) {
                Fit::Equal
            } else {
                Fit::General
            };
        }
    }
    if job.len() >= 4 && !is_generic(job) {
        if let Some(modifier) = profile.strip_suffix(job) {
            let _ = modifier;
            return Fit::Specific;
        }
        if profile
            .strip_prefix(job)
            .is_some_and(|head| head.starts_with('-') || light(head, lex::LIGHT_HEADS))
        {
            return Fit::Specific;
        }
    }
    Fit::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stemming_and_concepts() {
        assert_eq!(atoms("Mehrjährige Erfahrung im Controlling"), ["controll"]);
        assert_eq!(
            atoms("Several years of experience in controlling"),
            ["controll"]
        );
        assert_eq!(
            atoms("Erfahrung mit Monatsabschlüssen"),
            atoms("Monatsabschlüsse")
        );
        assert_eq!(
            atoms("Solid knowledge of IFRS group accounting"),
            ["ifrs", "konzernrechnungslegung"]
        );
        assert_eq!(atoms("Restrukturierungen"), atoms("Restrukturierung"));
        assert_eq!(
            atoms("Shared Service Centern"),
            atoms("Shared Service Center")
        );
    }

    #[test]
    fn compounds() {
        let a = |s: &str| atoms(s).remove(0);
        assert_eq!(
            fit(&a("Konzernkonsolidierung"), &a("Konsolidierung")),
            Fit::Equal
        );
        assert_eq!(
            fit(&a("Restrukturierungsumfeld"), &a("Restrukturierung")),
            Fit::Equal
        );
        assert_eq!(fit(&a("Carve-out-Projekten"), &a("Carve-out")), Fit::Equal);
        assert_eq!(
            fit(&a("Finanztransformation"), &a("Transformation")),
            Fit::Equal
        );
        assert_eq!(
            fit(&a("Beteiligungscontrolling"), &a("Controlling")),
            Fit::General
        );
        assert_eq!(
            fit(&a("Projektleitung"), &a("SAP-Projektleitung")),
            Fit::Specific
        );
        assert_eq!(fit(&a("Management"), &a("Interim-Management")), Fit::None);
    }

    #[test]
    fn tables_are_sorted_and_folded() {
        for table in [
            lex::FILLERS,
            lex::GENERIC_ATOMS,
            lex::LIGHT_MODIFIERS,
            lex::LIGHT_HEADS,
            lex::SOFT_SKILLS,
            lex::FRAME_WORDS,
        ] {
            assert!(table.windows(2).all(|w| w[0] < w[1]), "{table:?}");
            assert!(table.iter().all(|w| fold(w) == *w), "{table:?}");
        }
    }
}
