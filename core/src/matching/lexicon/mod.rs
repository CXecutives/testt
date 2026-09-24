//! Words and patterns the engine looks for in job ads and profiles.
//!
//! external contract - do not translate: everything in this module is German or English
//! wording of job ads, consultant profiles (German JSON keys shared with the external
//! `job-matching` skill) and the old engine's output texts. The legacy parts are copied
//! verbatim from `ca9a2cd^:matcher.py`; `tables.rs` is generated from
//! `core/tests/fixtures/matching/legacy_lexicon.json`.

pub(crate) mod engine;
mod tables;

pub(crate) use tables::*;

/// Heading prefixes checked after the exact heading lists, in this order.
pub(crate) const HEADING_PREFIXES: &[(&str, HeadingKind)] = &[
    ("anforderung", HeadingKind::Must),
    ("ihr profil", HeadingKind::Must),
    ("dein profil", HeadingKind::Must),
    ("qualifikation", HeadingKind::Must),
    ("voraussetzung", HeadingKind::Must),
    ("wünschenswert", HeadingKind::Nice),
    ("wuenschenswert", HeadingKind::Nice),
    ("von vorteil", HeadingKind::Nice),
];

/// What a heading line opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadingKind {
    Must,
    Nice,
    Task,
    Neutral,
}

/// Characters that start a bullet line.
pub(crate) const BULLETS: &[char] = &['-', '*', '•', '·', '▸', '►'];

/// Characters removed from a requirement item before tokenising.
pub(crate) const ITEM_STRIP: &[char] = &['(', ')', '[', ']', '{', '}', '„', '“', '"', '\''];

/// Suffixes that turn a language token into the language (`deutschkenntnisse` -> `deutsch`).
pub(crate) const LANGUAGE_SUFFIXES: &[&str] = &[
    "kenntnisse",
    "kenntnis",
    "sprache",
    "sprachen",
    "sprachkenntnisse",
];

/// Inline headings (`Anforderungen: ...`). `{S}` is Python's whitespace class.
pub(crate) const INLINE_HEADING: &str = r"^{S}*(ihr profil|dein profil|anforderung(?:en)?|qualifikation(?:en)?|voraussetzung(?:en)?|wünschenswert|wuenschenswert|ihre aufgaben|deine aufgaben|aufgaben|wir bieten|über uns){S}*[:：]{S}+(.+)$";

/// Requirement cues in a sentence (fallback without requirement sections).
pub(crate) const CUE_HARD: &str = r"\b(?:vorausgesetzt|erforderlich|benötigt|benoetigt|setzen{S}+wir{S}+voraus|setzt{S}+{NS}+{S}+voraus|voraussetzung{S}+ist|erwarten{S}+wir|abgeschlossenes{S}+studium|studium|hochschulabschluss|abschluss|abgeschlossene{S}+ausbildung|ausbildung|zertifizierung|zertifikat|zertifiziert|mehrjährig|mehrjaehrig|langjährig|langjaehrig|verhandlungssicher|fließend|fliessend|muttersprachlich|sprachkenntnisse|sicherer{S}+umgang|fundierte{S}+kenntnisse|umfassende{S}+kenntnisse|sehr{S}+gute{S}+kenntnisse|gute{S}+kenntnisse|tiefgehende{S}+kenntnisse|praxiserfahrung|berufserfahrung)\b";
/// `Erfahrung ... in/mit/im/von` within 40 characters.
pub(crate) const CUE_EXPERIENCE: &str =
    r"\b(?:erfahrung|erfahrungen|kenntnis|kenntnisse)\b.{0,40}\b(?:in|mit|im|von)\b";
/// Marks a cue sentence as nice-to-have.
pub(crate) const CUE_NICE: &str = r"\b(?:von{S}+vorteil|nice{S}+to{S}+have|nice-to-have|wünschenswert|wuenschenswert|idealerweise|vorteilhaft|bonus)\b";

/// Separators of requirement items (case-sensitive, like the old engine).
pub(crate) const ITEM_SPLIT: &str =
    r"{S}+(?:sowie|und|oder|bzw\.|beziehungsweise){S}+|,{S}+|;{S}+|{S}+&{S}+";

/// Job signal patterns, applied to the case-folded text.
pub(crate) const REMOTE_PERCENT: &str = r"({D}{1,3}){S}*%{S}*(?:remote|remote-anteil)";
pub(crate) const FULLY_REMOTE: &str =
    r"(?:100{S}*%{S}*remote|voll(?:ständig)?{S}*remote|fully{S}+remote)";
pub(crate) const ONSITE: &str = r"\b(?:vor{S}+ort|onsite|on-site|präsenz|prasenz)\b";
pub(crate) const RATE: &str = r"(?:tagessatz|day{S}*rate|honorar|stundensatz)[^0-9]{0,40}?({D}[{DB}.,]*){S}*(?:€|eur){S}*(?:pro{S}*tag|/tag|per{S}*day)?";
pub(crate) const RATE_SIMPLE: &str = r"({D}{3,4}){S}*(?:€|eur){S}*(?:/|pro){S}*tag";
pub(crate) const START_NOW: &str =
    r"\b(?:start|beginn|verfügbar|verfugbar|ab){S}*[:：]?{S}*(sofort|asap|ab{S}+sofort)";
pub(crate) const FUTURE_START: &str = r"\bab{S}+(januar|februar|märz|april|mai|juni|juli|august|september|oktober|november|dezember|q[1-4]|monat{S}*{D}+)\b";
pub(crate) const ANUE: &str = r"\b(?:anü|anue|arbeitnehmerüberlassung)\b";
pub(crate) const ANUE_NEGATED: &str =
    r"\b(?:kein|keine|nicht|ohne){S}+(?:anü|anue|arbeitnehmerüberlassung)\b";
pub(crate) const COUNTRY: &str = r"\b(?:deutschland|germany|österreich|oesterreich|austria|schweiz|switzerland|niederlande|netherlands|frankreich|france|italien|italy|spanien|spain|polen|poland|tschechien|czech|usa|uk|england|indien|india)\b";

/// Suffix of a nice-to-have requirement label in the old output.
pub(crate) const LABEL_NICE: &str = " (wünschenswert)";
/// Suffix of a matched vocabulary term in the old output.
pub(crate) const LABEL_VOCAB: &str = " (Job-Fachbegriff)";

/// Old violation texts (`check_criteria`).
pub(crate) const VIOLATION_ANUE: &str = "Arbeitnehmerüberlassung (ANÜ) ist ausgeschlossen";
pub(crate) const VIOLATION_START: &str = "Start liegt in der Zukunft (Verfügbarkeit: sofort)";

pub(crate) fn violation_day_rate(rate: &str, min: &str) -> String {
    format!("Tagessatz {rate} € unter {min} €")
}

pub(crate) fn violation_country(outside: &[String], allowed: &[String]) -> String {
    format!(
        "Einsatzland {} außerhalb {}",
        outside.join(", "),
        allowed.join("/")
    )
}

/// Profile keys of the explicit list pass: top-level list and the item keys read from it.
pub(crate) const PROFILE_LISTS: &[(&str, &[&str])] = &[
    ("methoden_tools", &["name", "kompetenz"]),
    ("zertifizierungen", &["name", "zertifizierung"]),
    ("branchen", &["branche"]),
    ("sprachen", &["sprache"]),
    ("kernkompetenzen", &["kompetenz"]),
    ("alleinstellungsmerkmale", &[]),
    ("keywords", &[]),
];

/// Profile keys of the hard criteria.
pub(crate) const KEY_CRITERIA: &str = "harte_kriterien";
pub(crate) const KEY_PREFERENCES: &str = "einsatzpraeferenzen";
pub(crate) const KEY_MIN_RATE: &str = "min_tagessatz";
pub(crate) const KEY_RATE_FROM: &str = "tagessatz_ab";
pub(crate) const KEY_COUNTRIES: &str = "laender";
pub(crate) const KEY_EXCLUDED_CONTRACTS: &str = "ausgeschlossene_vertragsarten";
pub(crate) const KEY_AVAILABLE: &str = "verfuegbar_ab";
pub(crate) const KEY_REMOTE_OUTSIDE: &str = "remote_ausserhalb_erlaubt";
/// Contract type value meaning temporary agency work (ANÜ).
pub(crate) const CONTRACT_ANUE: &str = "anue";
/// Availability value meaning "immediately".
pub(crate) const AVAILABLE_NOW: &str = "sofort";

/// Membership test in a sorted table.
pub(crate) fn contains(table: &[&str], word: &str) -> bool {
    table.binary_search(&word).is_ok()
}

/// Canonical form of a token: `Some(None)` drops it, `None` keeps it unchanged.
#[allow(clippy::option_option)] // Mirrors the old table, where `None` is a value.
pub(crate) fn synonym(token: &str) -> Option<Option<&'static str>> {
    SYNONYM_WORDS
        .binary_search_by(|(k, _)| k.cmp(&token))
        .ok()
        .map(|i| SYNONYM_WORDS[i].1)
}

/// Merged token of an adjacent pair.
pub(crate) fn synonym_pair(first: &str, second: &str) -> Option<&'static str> {
    SYNONYM_PAIRS
        .binary_search_by(|(a, b, _)| (*a, *b).cmp(&(first, second)))
        .ok()
        .map(|i| SYNONYM_PAIRS[i].2)
}

/// ISO code of a country name.
pub(crate) fn country_code(name: &str) -> Option<&'static str> {
    COUNTRY_NAMES
        .binary_search_by(|(k, _)| k.cmp(&name))
        .ok()
        .map(|i| COUNTRY_NAMES[i].1)
}
