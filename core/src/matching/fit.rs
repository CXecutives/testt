//! The profile the new engine matches against, and the match ladder for one item:
//! exact/stem/synonym = 1.0, profile more specific = 1.0, profile more general = 0.5,
//! two thirds of a long phrase = 0.5, generic atoms never alone (V1-V4, V15), language
//! levels (V9), years (V8), degrees (field, relatedness, level) and licences.

use std::collections::BTreeMap;

use serde_json::Value;

use super::atoms::{self, Fit, Vocab, fold};
use super::job::{Class, Item, contains_word, level_in};
use super::legacy::LegacyProfile;
use super::lexicon::{self, engine as lex};
use super::params::{E_FULL, E_HALF, E_NONE, SENTENCE_ATOMS};
use super::types::Via;

/// A profile entry (core competence, tool, certificate, ...), or an alternative term of
/// one (`auch`), which counts like a synonym of that entry.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub text: String,
    pub path: String,
    pub atoms: Vec<String>,
    pub years: Option<u32>,
    /// The competence this alias belongs to.
    pub alias_of: Option<String>,
}

/// What the new engine uses from the profile.
#[derive(Debug, Clone)]
pub(crate) struct Skills {
    pub entries: Vec<Entry>,
    /// Language stem and CEFR level (1-7).
    pub languages: Vec<(String, u8)>,
    /// Fields of the profile's degrees; `None` = no degree stated.
    pub degree_fields: Option<Vec<&'static str>>,
    /// Highest degree level (1 bachelor, 2 master or diploma, 3 doctorate; 0 unknown).
    pub degree_level: u8,
    /// Degrees as written.
    pub degrees: Vec<String>,
    /// All profile texts, folded (licences are looked up here).
    pub folded: Vec<String>,
    pub total_years: Option<u32>,
    /// Core plus the domain packs the competences switch on.
    pub vocab: Vocab,
}

fn years_of(value: &Value) -> Option<u32> {
    lex::KEYS_YEARS
        .iter()
        .find_map(|k| value.get(*k).and_then(Value::as_u64))
        .and_then(|y| u32::try_from(y).ok())
}

/// Total years of experience stated at the top level (`berufserfahrung_jahre`).
fn total_years_of(data: &Value) -> Option<u32> {
    lexicon::KEYS_TOTAL_YEARS
        .iter()
        .find_map(|k| data.get(*k).and_then(Value::as_u64))
        .and_then(|y| u32::try_from(y).ok())
}

/// Degree fields named in a folded text; a match inside a longer match does not count.
pub(crate) fn degree_fields_in(folded: &str) -> Vec<&'static str> {
    let found: Vec<(usize, usize, &'static str)> = lex::DEGREE_FIELDS
        .iter()
        .flat_map(|&(word, field)| {
            folded
                .match_indices(word)
                .map(move |(at, _)| (at, at + word.len(), field))
        })
        .collect();
    let mut fields: Vec<&'static str> = found
        .iter()
        .filter(|(start, end, _)| {
            !found
                .iter()
                .any(|(s, e, _)| s <= start && end <= e && e - s > end - start)
        })
        .map(|(_, _, field)| *field)
        .collect();
    fields.sort_unstable();
    fields.dedup();
    fields
}

/// Degree level named in a folded text (0 = none).
pub(crate) fn degree_level_in(folded: &str) -> u8 {
    let level = lex::DEGREE_LEVELS
        .iter()
        .filter(|(word, _)| folded.contains(word))
        .map(|&(_, level)| level)
        .max()
        .unwrap_or(0);
    let applied = lex::DEGREE_APPLIED.iter().any(|w| folded.contains(w));
    if level == 2 && applied && !folded.contains("master") {
        1
    } else {
        level
    }
}

/// Languages of the profile with their CEFR level (4 when the level is not readable).
fn languages_of(data: &Value) -> Vec<(String, u8)> {
    data.get(lex::KEY_LANGUAGES)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = fold(item.get(lex::KEY_LANGUAGE)?.as_str()?);
            let language = lex::LANGUAGES.iter().find(|l| name.starts_with(**l))?;
            let level = item
                .get(lex::KEY_LEVEL)
                .and_then(Value::as_str)
                .and_then(level_in)
                .unwrap_or(4);
            Some(((*language).to_owned(), level))
        })
        .collect()
}

/// Alternative terms of profile competences: (competence, alias, path, years).
fn aliases(data: &Value, core: &[String]) -> Vec<(String, String, String, Option<u32>)> {
    let mut out = Vec::new();
    let Value::Object(map) = data else { return out };
    for (list, value) in map {
        for (index, item) in value.as_array().into_iter().flatten().enumerate() {
            let Value::Object(fields) = item else {
                continue;
            };
            let Some(competence) = fields
                .values()
                .filter_map(Value::as_str)
                .map(str::trim)
                .find(|text| core.iter().any(|c| c == text))
            else {
                continue;
            };
            for key in lexicon::KEYS_ALIASES {
                let terms = fields.get(*key).and_then(Value::as_array);
                for (j, term) in terms.into_iter().flatten().enumerate() {
                    let Some(term) = term.as_str().map(str::trim).filter(|t| !t.is_empty()) else {
                        continue;
                    };
                    out.push((
                        competence.to_owned(),
                        term.to_owned(),
                        format!("{list}[{index}].{key}[{j}]"),
                        years_of(item),
                    ));
                }
            }
        }
    }
    out
}

impl Skills {
    /// `extra`: competences (text, JSON path) that must be entries although the profile
    /// walk did not find them (Schwerpunkte under an English key or in objects).
    pub(crate) fn new(legacy: &LegacyProfile, data: &Value, extra: &[(String, String)]) -> Self {
        // Years per competence text: list items with a `jahre` number.
        let mut years: BTreeMap<String, u32> = BTreeMap::new();
        if let Value::Object(map) = data {
            for value in map.values() {
                for item in value.as_array().into_iter().flatten() {
                    let Some(y) = years_of(item) else { continue };
                    for text in item
                        .as_object()
                        .into_iter()
                        .flat_map(|o| o.values())
                        .filter_map(Value::as_str)
                    {
                        years.insert(text.trim().to_owned(), y);
                    }
                }
            }
        }
        let core_texts: Vec<String> = legacy.signals.core.iter().map(|c| c.text.clone()).collect();
        let aliases = aliases(data, &core_texts);
        let folded_core: Vec<String> = core_texts.iter().map(|t| fold(t)).collect();
        let extra: Vec<&(String, String)> = extra
            .iter()
            .filter(|(text, _)| !folded_core.contains(&fold(text)))
            .collect();
        let vocab = Vocab::for_texts(
            core_texts
                .iter()
                .map(String::as_str)
                .chain(aliases.iter().map(|(_, alias, _, _)| alias.as_str()))
                .chain(extra.iter().map(|(text, _)| text.as_str())),
        );
        let mut entries: Vec<Entry> = legacy
            .signals
            .core
            .iter()
            .map(|core| Entry {
                atoms: atoms::atoms(&core.text, &vocab),
                years: years.get(&core.text).copied(),
                text: core.text.clone(),
                path: core.path.clone(),
                alias_of: None,
            })
            .collect();
        entries.extend(
            aliases
                .into_iter()
                .map(|(competence, alias, path, alias_years)| Entry {
                    atoms: atoms::atoms(&alias, &vocab),
                    years: alias_years.or_else(|| years.get(&competence).copied()),
                    text: alias,
                    path,
                    alias_of: Some(competence),
                }),
        );
        entries.extend(extra.iter().map(|(text, path)| Entry {
            atoms: atoms::atoms(text, &vocab),
            years: None,
            text: text.clone(),
            path: path.clone(),
            alias_of: None,
        }));
        entries.retain(|e| !e.atoms.is_empty());
        let languages = languages_of(data);
        let mut degree_fields: Option<Vec<&'static str>> = None;
        let (mut degree_level, mut degrees) = (0, Vec::new());
        for core in &legacy.signals.core {
            let folded = fold(&core.text);
            let is_degree = core.path.ends_with("abschluss")
                || lex::DEGREE_WORDS.iter().any(|w| folded.contains(w));
            if is_degree {
                let fields = degree_fields.get_or_insert_with(Vec::new);
                fields.extend(degree_fields_in(&folded));
                degree_level = degree_level.max(degree_level_in(&folded));
                degrees.push(core.text.clone());
            }
        }
        let total_years = total_years_of(data)
            .or_else(|| years_of(data))
            .or_else(|| years.values().copied().max());
        Self {
            entries,
            languages,
            degree_fields,
            degree_level,
            degrees,
            folded: core_texts.iter().map(|t| fold(t)).collect(),
            total_years,
            vocab,
        }
    }

    /// Competences without their aliases.
    pub(crate) fn competences(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.alias_of.is_none())
    }
}

/// How well the profile meets one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ItemFit {
    /// Evidence in per-mille: 0, 500 or 1000.
    pub value: u16,
    pub entry: Option<usize>,
    pub via: Via,
}

const NONE: ItemFit = ItemFit {
    value: E_NONE,
    entry: None,
    via: Via::Exact,
};

/// Job atoms plus the parts of hyphenated compounds.
fn expand(atoms: &[String]) -> Vec<String> {
    let mut out = atoms.to_vec();
    for atom in atoms {
        if atom.contains('-') {
            out.extend(atom.split('-').filter(|p| p.len() >= 2).map(str::to_owned));
        }
    }
    out
}

/// A free-text entry (a USP sentence, a long description): containing a requirement is
/// weaker evidence than a competence entry.
fn sentence_like(entry: &Entry) -> bool {
    entry.path.starts_with(lex::KEY_USP)
        || entry.atoms.iter().filter(|a| !atoms::is_generic(a)).count() >= SENTENCE_ATOMS
}

/// One entry against the atoms of one alternative.
fn entry_fit(job: &[String], expanded: &[String], whole: &Entry) -> (u16, Via) {
    let entry = &whole.atoms;
    let best = |p: &String| {
        expanded
            .iter()
            .map(|j| atoms::fit(j, p))
            .max()
            .unwrap_or(Fit::None)
    };
    let fits: Vec<Fit> = entry.iter().map(best).collect();
    let matched = fits.iter().filter(|f| **f != Fit::None).count();
    let generic_only = entry
        .iter()
        .zip(&fits)
        .filter(|(_, f)| **f != Fit::None)
        .all(|(p, _)| atoms::is_generic(p));
    let whole_item = job
        .iter()
        .all(|j| entry.iter().any(|p| atoms::fit(j, p) != Fit::None));
    if matched == entry.len() && (!generic_only || whole_item) {
        return if fits.contains(&Fit::General) {
            (E_HALF, Via::General)
        } else if fits.contains(&Fit::Specific) {
            (E_FULL, Via::Specific)
        } else {
            (E_FULL, Via::Exact)
        };
    }
    // The item lies inside the entry: the profile is more specific.
    let content: Vec<&String> = job.iter().filter(|j| !atoms::is_generic(j)).collect();
    if !content.is_empty()
        && content.iter().all(|j| {
            entry
                .iter()
                .any(|p| matches!(atoms::fit(j, p), Fit::Equal | Fit::Specific))
        })
    {
        // Free text that mentions a skill (a USP sentence, a long description such as
        // `Integration FI/CO mit MM und SD`) does not prove it.
        return if sentence_like(whole) {
            (E_NONE, Via::Exact)
        } else {
            (E_FULL, Via::Specific)
        };
    }
    if entry.len() >= 3 && 3 * matched >= 2 * entry.len() && !generic_only {
        return (E_HALF, Via::General);
    }
    (E_NONE, Via::Exact)
}

/// Does any profile atom relate to this job atom (a hyphenated atom through its parts)?
fn covered(skills: &Skills, atom: &str) -> bool {
    let related = |a: &str| {
        skills
            .entries
            .iter()
            .flat_map(|e| &e.atoms)
            .any(|p| atoms::fit(a, p) != Fit::None || atoms::fit(p, a) != Fit::None)
    };
    related(atom)
        || (atom.contains('-')
            && atom
                .split('-')
                .filter(|p| p.len() >= 3 && !atoms::is_generic(p))
                .all(related))
}

fn skill_fit(skills: &Skills, text: &str) -> ItemFit {
    skill_fit_in(skills, text, None)
}

/// The ladder over all entries, or only over `only` (entry indices).
fn skill_fit_in(skills: &Skills, text: &str, only: Option<&[usize]>) -> ItemFit {
    let job = atoms::atoms(text, &skills.vocab);
    if job.is_empty() {
        return NONE;
    }
    let expanded = expand(&job);
    let mut best = NONE;
    for (index, entry) in skills.entries.iter().enumerate() {
        if only.is_some_and(|only| !only.contains(&index)) {
            continue;
        }
        let (value, via) = entry_fit(&job, &expanded, entry);
        if value > best.value {
            best = ItemFit {
                value,
                entry: Some(index),
                via,
            };
        }
    }
    // The ad asks for more than the profile names (`Tableau im Reporting` against
    // `Reporting`): the profile is more general, half.
    if best.value == E_FULL
        && job
            .iter()
            .any(|j| !atoms::is_generic(j) && !covered(skills, j))
    {
        best.value = E_HALF;
        best.via = Via::General;
    }
    best
}

fn language_fit(skills: &Skills, language: &str, level: Option<u8>) -> ItemFit {
    let Some(&(_, have)) = skills.languages.iter().find(|(l, _)| l == language) else {
        return NONE;
    };
    // A level below the requirement (B2 for "fluent") stays open.
    let need = level.unwrap_or(3);
    let value = if have >= need { E_FULL } else { E_NONE };
    ItemFit {
        value,
        entry: None,
        via: Via::Exact,
    }
}

/// A degree requirement: the field (any, comparable, same, neighbouring or other) and the
/// level (a bachelor for a master's requirement is half). `Diplom-Kauffrau (Univ.)` is a
/// business degree at master level.
fn degree_fit(skills: &Skills, text: &str) -> ItemFit {
    let Some(fields) = &skills.degree_fields else {
        return NONE;
    };
    let folded = fold(text);
    let wanted = degree_fields_in(&folded);
    let comparable = lex::COMPARABLE_WORDS.iter().any(|w| folded.contains(w));
    let related = |w: &&str| {
        fields.iter().any(|f| {
            lex::DEGREE_RELATED
                .iter()
                .any(|&(a, b)| (a == *w && b == *f) || (a == *f && b == *w))
        })
    };
    let field = if wanted.is_empty() || comparable || wanted.iter().any(|w| fields.contains(w)) {
        E_FULL
    } else if wanted.iter().any(related) || fields.is_empty() {
        E_HALF
    } else {
        E_NONE
    };
    // The lowest level the ad accepts (`Bachelor or Master` = bachelor).
    let needed = lex::DEGREE_LEVELS
        .iter()
        .filter(|(word, _)| folded.contains(word))
        .map(|&(_, level)| level)
        .min()
        .unwrap_or(0);
    let level_ok = needed == 0 || skills.degree_level == 0 || skills.degree_level >= needed;
    let value = if level_ok { field } else { field.min(E_HALF) };
    ItemFit {
        value,
        entry: None,
        via: Via::Exact,
    }
}

/// A licence requirement (`Zulassung als Steuerberater`): met only when a profile text
/// names the same licence.
fn licence_fit(skills: &Skills, word: &str) -> ItemFit {
    let value = if skills.folded.iter().any(|t| t.contains(word)) {
        E_FULL
    } else {
        E_NONE
    };
    ItemFit {
        value,
        entry: None,
        via: Via::Exact,
    }
}

/// The alternatives of a skill item (the item itself when it has none).
fn alternatives(item: &Item) -> Vec<&str> {
    let mut alternatives: Vec<&str> = item.alternatives.iter().map(String::as_str).collect();
    if alternatives.is_empty() {
        alternatives.push(&item.text);
    }
    alternatives
}

/// How well only the entries `only` meet a skill item (best alternative, no years). `via`
/// tells whether the item names the entry (exact), something narrower (general, half) or
/// only a broader term inside the entry (specific).
pub(crate) fn focus_fit(skills: &Skills, item: &Item, only: &[usize]) -> ItemFit {
    // An entry that names the item beats one it only lies inside at the same value.
    let rank = |fit: &ItemFit| (fit.value, fit.via != Via::Specific);
    alternatives(item)
        .iter()
        .flat_map(|alt| {
            only.iter()
                .map(move |&index| skill_fit_in(skills, alt, Some(&[index])))
        })
        .fold(
            NONE,
            |best, fit| {
                if rank(&fit) > rank(&best) { fit } else { best }
            },
        )
}

/// Does the title name one of the entries `only` (every atom of the entry, equal)?
pub(crate) fn title_names(skills: &Skills, title: &str, only: &[usize]) -> bool {
    let job = atoms::atoms(title, &skills.vocab);
    if job.is_empty() {
        return false;
    }
    let expanded = expand(&job);
    only.iter()
        .filter_map(|&index| skills.entries.get(index))
        .any(|entry| entry_fit(&job, &expanded, entry) == (E_FULL, Via::Exact))
}

/// The ladder for one item (best alternative), then the years requirement.
pub(crate) fn item_fit(skills: &Skills, item: &Item) -> ItemFit {
    let mut fit = match &item.class {
        Class::Frame => NONE,
        Class::Language(language, level) => language_fit(skills, language, *level),
        Class::Degree => degree_fit(skills, &item.text),
        Class::Licence(word) => licence_fit(skills, word),
        Class::Soft | Class::Skill => alternatives(item)
            .iter()
            .map(|alt| skill_fit(skills, alt))
            .max_by_key(|f| f.value)
            .unwrap_or(NONE),
    };
    if let Some(needed) = item.years {
        let folded = fold(&item.text);
        // General experience only without a topic: `10 Jahre Berufserfahrung`, not
        // `3-5 Jahre Berufserfahrung im Controlling`.
        let topic = atoms::atoms(&item.text, &skills.vocab)
            .into_iter()
            .any(|a| {
                !atoms::is_generic(&a)
                    && !lex::GENERAL_EXPERIENCE
                        .iter()
                        .chain(lex::EXPERIENCE_WORDS)
                        .any(|w| a.starts_with(&atoms::stem(w)))
            });
        let general = !topic
            && lex::GENERAL_EXPERIENCE
                .iter()
                .any(|w| contains_word(&folded, w));
        let have =
            fit.entry
                .and_then(|e| skills.entries[e].years)
                .or(if general || fit.value > 0 {
                    skills.total_years
                } else {
                    None
                });
        if general && fit.value == E_NONE && have.is_some_and(|h| h >= needed) {
            fit = ItemFit {
                value: E_FULL,
                entry: None,
                via: Via::Exact,
            };
        } else if fit.value > E_NONE
            && let Some(have) = have
            && have < needed
        {
            fit.value = if 5 * have >= 4 * needed {
                E_HALF.min(fit.value)
            } else {
                E_NONE
            };
        }
    }
    fit
}
