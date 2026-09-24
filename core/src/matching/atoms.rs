//! Skill atoms of the new engine: case-folded, umlauts folded, fillers removed, light
//! stemming (V1), bilingual concepts (V3), and the fit of two atoms including compounds
//! (V2) and specific/general relations (V4).

use std::sync::LazyLock;

use super::lexicon::domains::{DOMAINS, Domain};
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

/// The concept table of one profile: the general core plus the domain packs its
/// competences switch on. Stemmed keys and values, longest keys first.
#[derive(Debug, Clone)]
pub(crate) struct Vocab {
    concepts: Vec<(Vec<String>, String)>,
    packs: Vec<&'static str>,
}

impl Vocab {
    fn with(packs: &[&'static Domain]) -> Self {
        let mut concepts: Vec<(Vec<String>, String)> = lex::CORE_CONCEPTS
            .iter()
            .chain(packs.iter().flat_map(|d| d.concepts.iter()))
            .map(|(key, value)| (key.split(' ').map(stem).collect(), stem(value)))
            .collect();
        concepts.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
        Self {
            concepts,
            packs: packs.iter().map(|d| d.name).collect(),
        }
    }

    /// The general core only (tests).
    #[cfg(test)]
    pub(crate) fn core() -> Self {
        Self::with(&[])
    }

    /// Core plus every pack a token of `texts` triggers.
    pub(crate) fn for_texts<'a>(texts: impl IntoIterator<Item = &'a str>) -> Self {
        let folded: Vec<String> = texts.into_iter().map(fold).collect();
        let packs: Vec<&'static Domain> = DOMAINS
            .iter()
            .filter(|domain| {
                folded.iter().any(|text| {
                    raw_tokens(text).any(|t| domain.triggers.iter().any(|p| t.starts_with(p)))
                })
            })
            .copied()
            .collect();
        Self::with(&packs)
    }

    /// Every pack (tests).
    #[cfg(test)]
    pub(crate) fn all() -> Self {
        Self::with(DOMAINS)
    }

    /// Names of the switched-on packs.
    pub(crate) fn packs(&self) -> &[&'static str] {
        &self.packs
    }

    fn concept_at(&self, stems: &[String], at: usize) -> Option<(usize, String)> {
        self.concepts.iter().find_map(|(key, value)| {
            let end = at + key.len();
            (end <= stems.len() && stems[at..end] == key[..]).then(|| (key.len(), value.clone()))
        })
    }
}

/// Is `word` a stopword or filler (folded form)?
pub(crate) fn is_filler(word: &str) -> bool {
    STOP.binary_search_by(|w| w.as_str().cmp(word)).is_ok()
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '.' | '/' | '-' | '&' | 'ß')
}

/// Raw folded tokens (the old token class plus `&` inside words such as `M&A`), trimmed
/// of leading and trailing `.`, `-`, `/` and `&`.
pub(crate) fn raw_tokens(folded: &str) -> impl Iterator<Item = &str> {
    folded
        .split(|c: char| !is_token_char(c))
        .map(|t| t.trim_matches(|c| c == '.' || c == '-' || c == '/' || c == '&'))
        .filter(|t| !t.is_empty())
}

/// `FI/CO` and `UI/UX`: short codes joined by `/` are separate tokens.
fn split_codes(token: &str) -> Vec<&str> {
    let parts: Vec<&str> = token.split('/').collect();
    let codes = parts.len() > 1
        && parts
            .iter()
            .all(|p| p.len() <= 3 && lexicon::synonym(p).is_some());
    if codes { parts } else { vec![token] }
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

/// Atoms of a text: tokens without fillers, stemmed, concepts of `vocab` applied.
pub(crate) fn atoms(text: &str, vocab: &Vocab) -> Vec<String> {
    let folded = fold(text);
    let raw: Vec<&str> = raw_tokens(&folded).flat_map(split_codes).collect();
    let stems: Vec<String> = raw
        .iter()
        .enumerate()
        // `GmbH & Co. KG` is a company form, not SAP CO.
        .filter(|&(i, t)| !(*t == "co" && raw.get(i + 1) == Some(&"kg")))
        .map(|(_, t)| *t)
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
            if spaced.len() > 1
                && vocab
                    .concept_at(&spaced, 0)
                    .is_some_and(|(n, _)| n == spaced.len())
            {
                spaced
            } else {
                vec![stem(t)]
            }
        })
        .collect();
    let mut result = Vec::with_capacity(stems.len());
    let mut i = 0;
    while i < stems.len() {
        if let Some((len, concept)) = vocab.concept_at(&stems, i) {
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
    // A generic profile atom (`Management`) never reaches a compound
    // (`Projektmanagement`).
    if is_generic(profile) {
        return Fit::None;
    }
    // Four letters are too short to be a compound part (`steu` of `Steuern` is not the
    // head of `Steuerung`).
    if profile.len() >= 5 {
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
        // `cash` is not the head of `Order-to-Cash`: a short atom never ends a hyphenated
        // name.
        if let Some(modifier) = profile.strip_suffix(job)
            && !(modifier.ends_with('-') && job.len() < 5)
        {
            return Fit::Specific;
        }
        if profile.strip_prefix(job).is_some_and(|head| {
            (head.starts_with('-') && job.len() >= 5) || light(head, lex::LIGHT_HEADS)
        }) {
            return Fit::Specific;
        }
    }
    Fit::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(text: &str) -> Vec<String> {
        atoms(text, &Vocab::all())
    }

    #[test]
    fn stemming_and_concepts() {
        assert_eq!(all("Mehrjährige Erfahrung im Controlling"), ["controll"]);
        assert_eq!(
            all("Several years of experience in controlling"),
            ["controll"]
        );
        assert_eq!(
            all("Erfahrung mit Monatsabschlüssen"),
            all("Monatsabschlüsse")
        );
        assert_eq!(
            all("Solid knowledge of IFRS group accounting"),
            ["ifrs", "konzernrechnungslegung"]
        );
        assert_eq!(all("Restrukturierungen"), all("Restrukturierung"));
        assert_eq!(all("Shared Service Centern"), all("Shared Service Center"));
        assert_eq!(all("SAP FI/CO"), ["sap", "fi", "co"]);
        assert_eq!(all("M&A-Transaktionen"), ["m&a-transaktion"]);
        assert!(!all("Müller GmbH & Co. KG").contains(&"co".to_owned()));
    }

    /// Bilingual pairs and paraphrases seen in finance ads meet in one concept.
    #[test]
    fn finance_paraphrases() {
        let same = [
            ("Month-end closing", "Monatsabschluss"),
            ("monthly close", "Monatsabschlüsse"),
            ("Year-end closing", "Jahresabschluss"),
            ("Consolidated financial statements", "Konzernabschluss"),
            ("Group controlling", "Konzerncontrolling"),
            ("Group reporting", "Konzernreporting"),
            ("Financial Planning & Analysis", "FP&A"),
            ("Mergers & Acquisitions", "M&A"),
            ("Liquidity planning", "Liquiditätsplanung"),
            ("Cash Management", "Liquiditätssteuerung"),
            ("Variance analysis", "Abweichungsanalyse"),
            ("Soll-Ist-Vergleich", "Abweichungsanalyse"),
            ("Cost center accounting", "Kostenstellenrechnung"),
            ("General Ledger", "Hauptbuchhaltung"),
            ("Accounts payable", "Kreditorenbuchhaltung"),
            ("Asset accounting", "Anlagenbuchhaltung"),
            ("FI-AA", "Anlagenbuchhaltung"),
            ("CO-PA", "Ergebnisrechnung"),
            ("Berichtswesen", "Reporting"),
            ("Hochrechnung", "Forecast"),
            ("Carve out", "Carve-out"),
            ("Post Merger Integration", "PMI"),
            ("Restructuring", "Restrukturierung"),
            ("Finance Business Partner", "Finance Business Partnering"),
            ("German GAAP", "HGB"),
            ("Interim Manager", "Interim Management"),
            ("Interim-Mandat", "Interim Management"),
            ("Accruals", "Abgrenzungen"),
        ];
        for (a, b) in same {
            assert_eq!(all(a), all(b), "{a} / {b}");
        }
    }

    /// False friends: these must not meet each other fully.
    #[test]
    fn anti_pairs() {
        let full = |job: &str, profile: &str| {
            let profile = all(profile);
            all(job).iter().any(|j| {
                profile
                    .iter()
                    .any(|p| matches!(fit(j, p), Fit::Equal | Fit::Specific))
            })
        };
        let pairs = [
            ("Steuerung", "Steuern"),
            ("SAP Business Partner", "Finance Business Partnering"),
            ("Standortkonsolidierung", "Konsolidierung"),
            ("Produktionsplanung", "Unternehmensplanung"),
            ("Change Requests", "Changemanagement"),
            ("Hochschulabschluss", "Jahresabschlüsse"),
            ("Projektmanagement", "Management Accounting"),
            ("Rechnungsprüfung", "Rechnungslegung"),
        ];
        for (job, profile) in pairs {
            assert!(!full(job, profile), "{job} met by {profile}");
        }
    }

    /// Packs switch on by the profile's competences; the core works alone.
    #[test]
    fn packs_follow_the_profile() {
        let finance = Vocab::for_texts(["Controlling", "Konzernrechnungslegung nach IFRS"]);
        assert_eq!(finance.packs(), ["finance"]);
        let sap = Vocab::for_texts(["SAP FI", "Datenmigration"]);
        assert_eq!(sap.packs(), ["sap", "itProject"]);
        let clinical = Vocab::for_texts(["Klinische Studien", "Clinical Trial Management"]);
        assert!(clinical.packs().is_empty());
        assert_ne!(
            atoms("group accounting", &clinical),
            atoms("group accounting", &finance)
        );
        assert_eq!(
            atoms("project management", &clinical),
            atoms("Projektmanagement", &clinical)
        );
    }

    #[test]
    fn compounds() {
        let a = |s: &str| all(s).remove(0);
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
        assert_eq!(fit(&a("Projektmanagement"), &a("Management")), Fit::None);
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
            lex::LOCATION_NOISE,
            lex::GERMAN_CITIES,
        ] {
            assert!(table.windows(2).all(|w| w[0] < w[1]), "{table:?}");
            assert!(table.iter().all(|w| fold(w) == *w), "{table:?}");
        }
        for domain in DOMAINS {
            assert!(
                domain.triggers.windows(2).all(|w| w[0] < w[1]),
                "{}",
                domain.name
            );
            for (key, value) in domain.concepts {
                assert_eq!(fold(key), *key, "{}", domain.name);
                assert_eq!(fold(value), *value, "{}", domain.name);
            }
        }
    }
}
