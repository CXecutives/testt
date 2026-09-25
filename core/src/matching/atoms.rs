//! Skill atoms of the new engine: case-folded, umlauts folded, fillers removed, light
//! stemming (V1), bilingual concepts (V3), and the fit of two atoms including compounds
//! (V2) and specific/general relations (V4).

use std::collections::HashMap;
use std::sync::LazyLock;

use super::lexicon::domains::{DOMAINS, Domain};
use super::lexicon::{self, engine as lex};
use super::normalize::casefold;
use super::params::PACK_HITS;

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
    /// Indices into `concepts` by the first stem of the key, in `concepts` order.
    by_first: HashMap<String, Vec<usize>>,
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
        let mut by_first: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, (key, _)) in concepts.iter().enumerate() {
            if let Some(first) = key.first() {
                by_first.entry(first.clone()).or_default().push(index);
            }
        }
        Self {
            concepts,
            by_first,
            packs: packs.iter().map(|d| d.name).collect(),
        }
    }

    /// The general core only (tests).
    #[cfg(test)]
    pub(crate) fn core() -> Self {
        Self::with(&[])
    }

    /// Core plus every pack that at least `PACK_HITS` tokens of `texts` trigger (one stray
    /// word such as `Budget` in an HR profile switches on no finance pack).
    pub(crate) fn for_texts<'a>(texts: impl IntoIterator<Item = &'a str>) -> Self {
        let folded: Vec<String> = texts.into_iter().map(fold).collect();
        let packs: Vec<&'static Domain> = DOMAINS
            .iter()
            .filter(|domain| {
                let hits = folded
                    .iter()
                    .flat_map(|text| raw_tokens(text))
                    .filter(|t| domain.triggers.iter().any(|p| t.starts_with(p)))
                    .count();
                hits >= PACK_HITS
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

    /// The longest concept whose key starts at `at` (the first in `concepts` order).
    fn concept_at(&self, stems: &[String], at: usize) -> Option<(usize, String)> {
        let candidates = self.by_first.get(stems.get(at)?)?;
        candidates.iter().find_map(|&index| {
            let (key, value) = &self.concepts[index];
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
    // Short codes (`FI/CO`) and short words (`HGB/IFRS`); `S/4HANA` and `m/w/d` stay.
    let codes = parts.len() > 1
        && parts.iter().all(|p| {
            (p.len() <= 3 && lexicon::synonym(p).is_some())
                || ((2..=5).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphabetic()))
        });
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

/// Gender forms end a word (`Personalleiter:in`, `Berater*innen`, `Leiter/in`,
/// `Controller(in)`): the base word counts.
fn strip_gender(folded: &str) -> String {
    let mut out = String::with_capacity(folded.len());
    let mut rest = folded;
    while let Some(c) = rest.chars().next() {
        let form = lex::GENDER_FORMS.iter().find(|form| {
            rest.strip_prefix(**form).is_some_and(|after| {
                after.chars().next().is_none_or(|n| !n.is_alphanumeric())
                    && out.chars().next_back().is_some_and(char::is_alphanumeric)
            })
        });
        if let Some(form) = form {
            rest = &rest[form.len()..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// A hyphenated skill without a part that only says "knowledge of" (`SQL-Kenntnisse`).
fn without_knowledge(token: &str) -> &str {
    lex::KNOWLEDGE_SUFFIXES
        .iter()
        .find_map(|s| token.strip_suffix(s).filter(|rest| !rest.is_empty()))
        .unwrap_or(token)
}

/// A short token that names a skill (`QP`, `R`, `8D`)?
fn short_skill(token: &str) -> bool {
    lex::SHORT_TOKENS.binary_search(&token).is_ok()
}

/// Raw tokens with codes completed by their number (`iso 9001` -> `iso-9001`).
fn numbered(raw: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let t = raw[i];
        let number = raw
            .get(i + 1)
            .filter(|n| n.len() <= 6 && n.chars().all(|c| c.is_ascii_digit()));
        if let Some(n) = number.filter(|_| lex::NUMBERED_CODES.binary_search(&t).is_ok()) {
            out.push(format!("{t}-{n}"));
            i += 2;
        } else {
            out.push(t.to_owned());
            i += 1;
        }
    }
    out
}

/// A token the atoms keep: long enough (or a known short skill or synonym), no filler,
/// with a letter.
fn kept(token: &str) -> Option<String> {
    let mapped = match lexicon::synonym(token) {
        Some(Some(mapped)) => mapped.to_owned(),
        Some(None) => return None,
        None if token.len() < 3 && !short_skill(token) => return None,
        None => token.to_owned(),
    };
    let skill = short_skill(&mapped)
        || (!is_filler(&mapped) && mapped.chars().any(|c| c.is_ascii_alphabetic()));
    skill.then_some(mapped)
}

/// Atoms of a text: tokens without fillers, stemmed, concepts of `vocab` applied. A
/// concept whose words include a filler, a stopword or a short word (`US GAAP`, `Order to
/// Cash`, `Year End Closing`) is found before those words go.
pub(crate) fn atoms(text: &str, vocab: &Vocab) -> Vec<String> {
    let folded = strip_gender(&fold(text));
    let split: Vec<&str> = raw_tokens(&folded)
        .map(without_knowledge)
        .flat_map(split_codes)
        .collect();
    let raw: Vec<String> = numbered(
        &split
            .iter()
            .enumerate()
            // `GmbH & Co. KG` is a company form, not SAP CO.
            .filter(|&(i, t)| !(*t == "co" && split.get(i + 1) == Some(&"kg")))
            .map(|(_, t)| *t)
            .collect::<Vec<_>>(),
    );
    // Concepts across dropped words: the whole stream, stemmed.
    let full: Vec<String> = raw.iter().map(|t| stem(t)).collect();
    let mut pieces: Vec<(String, bool)> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if let Some((n, concept)) = vocab.concept_at(&full, i)
            && n > 1
            && raw[i..i + n].iter().any(|t| kept(t).is_none())
        {
            pieces.push((concept, true));
            i += n;
            continue;
        }
        if let Some(t) = kept(&raw[i]) {
            // Hyphenated words that form a known concept count as that concept.
            let spaced: Vec<String> = t.split('-').map(stem).collect();
            if spaced.len() > 1
                && vocab
                    .concept_at(&spaced, 0)
                    .is_some_and(|(n, _)| n == spaced.len())
            {
                pieces.extend(spaced.into_iter().map(|s| (s, false)));
            } else {
                pieces.push((stem(&t), false));
            }
        }
        i += 1;
    }
    let stems: Vec<String> = pieces.iter().map(|(s, _)| s.clone()).collect();
    let mut result = Vec::with_capacity(stems.len());
    let mut i = 0;
    while i < stems.len() {
        if pieces[i].1 {
            result.push(stems[i].clone());
            i += 1;
        } else if let Some((len, concept)) = vocab
            .concept_at(&stems, i)
            .filter(|(len, _)| !pieces[i..i + len].iter().any(|(_, done)| *done))
        {
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

/// The core's generic atoms plus the broad words of every domain pack (as written and
/// stemmed): a broad word is broad whoever reads it. `longest` lets the hot path skip the
/// long compounds that make up most atoms.
struct Generic {
    words: Vec<String>,
    longest: usize,
}

static GENERIC: LazyLock<Generic> = LazyLock::new(|| {
    let mut words: Vec<String> = lex::GENERIC_ATOMS
        .iter()
        .map(|w| (*w).to_owned())
        .chain(
            DOMAINS
                .iter()
                .flat_map(|d| d.generic.iter())
                .flat_map(|w| [(*w).to_owned(), stem(w)]),
        )
        .collect();
    words.sort();
    words.dedup();
    let longest = words.iter().map(String::len).max().unwrap_or(0);
    Generic { words, longest }
});

/// Is the atom too generic to meet a requirement on its own?
pub(crate) fn is_generic(atom: &str) -> bool {
    let generic = &*GENERIC;
    atom.len() <= generic.longest
        && generic
            .words
            .binary_search_by(|w| w.as_str().cmp(atom))
            .is_ok()
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

/// How a job atom relates to a profile atom (V2 compounds, V4 specific/general). A head
/// the lexicon treats as a near equivalent (`Projektleitung` for `Projektmanagement`, also
/// in `Projektleitungserfahrung`) meets it in half: close, but leading a project and
/// managing projects are not always the same skill.
pub(crate) fn fit(job: &str, profile: &str) -> Fit {
    let direct = fit_direct(job, profile);
    if direct != Fit::None {
        return direct;
    }
    for &(a, b) in lex::EQUIVALENT_HEADS {
        for (from, to) in [(a, b), (b, a)] {
            if let Some(rest) = job.strip_prefix(from)
                && fit_direct(&format!("{to}{rest}"), profile) != Fit::None
            {
                return Fit::General;
            }
        }
    }
    Fit::None
}

fn fit_direct(job: &str, profile: &str) -> Fit {
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
    // A compound needs a real modifier: `h` + `erstellung` is `Herstellung`, no compound.
    // `IT-Carve-out` narrows `Carve-out` (a hyphen marks the compound); `Einführung` is no
    // `Führung` (a verbal particle is no modifier).
    let modifier_ok = |m: &str| {
        let bare = m.trim_end_matches('-');
        (m.ends_with('-') || bare.len() >= lex::MIN_COMPOUND_MODIFIER)
            && !lex::PARTICLE_MODIFIERS.contains(&bare)
    };
    if profile.len() >= 5 {
        if let Some(modifier) = job.strip_suffix(profile).filter(|m| modifier_ok(m)) {
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
            && modifier_ok(modifier)
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
        let sap = Vocab::for_texts(["SAP FI", "SAP S/4HANA", "Datenmigration", "Jira"]);
        assert_eq!(sap.packs(), ["sap", "itProject"]);
        // One stray trigger switches on no pack: the HR profile gets no finance pack.
        let hr = Vocab::for_texts(["Recruiting", "Arbeitsrecht", "Budgetverantwortung"]);
        assert_eq!(hr.packs(), ["hr"]);
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
    fn codes_gender_forms_and_near_equivalents() {
        assert_eq!(all("HGB/IFRS"), all("HGB IFRS"));
        assert_eq!(all("S/4HANA"), all("S/4HANA"));
        assert_eq!(all("Personalleiter:in"), all("Personalleiter"));
        assert_eq!(all("Berater*innen"), all("Berater"));
        assert_eq!(all("Controller/in (m/w/d)"), all("Controller"));
        let a = |s: &str| all(s).remove(0);
        assert_eq!(
            fit(&a("Projektleitungserfahrung"), &a("Projektmanagement")),
            Fit::General
        );
        assert_eq!(
            fit(&a("Projektmanagement"), &a("Projektleitung")),
            Fit::General
        );
    }

    /// A compound needs a real modifier: `Herstellung` is no `Erstellung`.
    #[test]
    fn leading_roles_are_one_concept() {
        let has = |text: &str, atom: &str| all(text).iter().any(|a| a == atom);
        assert!(has("Head of Finance", "cfo"));
        assert!(has("VP Finance", "cfo"));
        assert!(has("Leitung Finanzen", "cfo"));
        assert!(has("Chief Commercial Officer", "vertriebsleitung"));
        assert!(has("Commercial Director", "vertriebsleitung"));
        assert!(has("Leiter IT", "it-leitung"));
        // The rank words alone name no field.
        for word in ["Head", "Chief", "Officer", "Director"] {
            assert!(all(word).iter().all(|a| is_generic(a)), "{word}");
        }
    }

    #[test]
    fn compound_boundaries() {
        let a = |s: &str| all(s).remove(0);
        // A hyphen marks a compound however short its modifier; a verbal particle is none.
        assert_eq!(fit(&a("IT-Carve-out"), &a("Carve-out")), Fit::General);
        assert_eq!(fit(&a("Führung"), &a("Einführung")), Fit::None);
        assert_eq!(fit(&a("Führung"), &a("Durchführung")), Fit::None);
        assert_eq!(fit(&a("Führung"), &a("Buchführung")), Fit::None);
        assert_eq!(all("Konzernrechnung"), all("Konzernrechnungslegung"));
        // `Unternehmen` and `Partner` are too broad to meet anything alone.
        assert!(is_generic(&a("Unternehmen")));
        assert!(is_generic(&a("Partner")));
        assert_eq!(fit(&a("Herstellung"), &a("Erstellung")), Fit::None);
        assert_eq!(fit(&a("Erstellung"), &a("Herstellung")), Fit::None);
        assert_eq!(fit(&a("Berichterstellung"), &a("Erstellung")), Fit::General);
    }

    /// `SQL-Kenntnisse` is SQL; `ISO 9001` and `ISO 13485` differ; short skills stay.
    #[test]
    fn codes_numbers_and_short_skills() {
        assert_eq!(all("SQL-Kenntnisse"), all("SQL"));
        assert_eq!(all("IFRS-Kenntnisse"), all("IFRS"));
        assert_eq!(all("CAPA-Erfahrung"), all("CAPA"));
        assert_eq!(all("SAP-Know-how"), all("SAP"));
        assert_eq!(all("ISO 9001"), ["iso-9001"]);
        assert_ne!(all("ISO 13485"), all("ISO 9001"));
        assert_eq!(all("ISO-9001"), all("ISO 9001:2015"));
        assert!(all("EU GMP Annex 1").contains(&"annex-1".to_owned()));
        assert_ne!(all("Annex 11"), all("Annex 1"));
        for short in ["R", "Go", "5S", "8D", "ML", "VP"] {
            assert_eq!(all(short).len(), 1, "{short}");
        }
        assert_eq!(all("IQ/OQ/PQ").len(), 3);
        assert_eq!(all("CI/CD").len(), 2);
    }

    /// Lexicon terms with a stopword, filler or short word match with spaces too.
    #[test]
    fn multiword_terms_across_dropped_words() {
        assert_eq!(all("US GAAP"), all("US-GAAP"));
        assert_eq!(all("Order to Cash"), all("Order-to-Cash"));
        assert_eq!(all("Working Capital"), all("Working-Capital-Management"));
        assert_eq!(all("Year End Closing"), all("Jahresabschluss"));
        assert_eq!(all("React Native"), all("React-Native"));
        assert_eq!(all("React Native").len(), 1);
        assert_eq!(all("Customer Experience"), all("Customer-Experience"));
        assert_eq!(all("Customer Experience").len(), 1);
        assert_eq!(all("Job Evaluation"), all("Stellenbewertung"));
        assert_eq!(all("General Counsel"), ["leitung-recht"]);
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
