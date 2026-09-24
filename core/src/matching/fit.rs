//! The profile the new engine matches against, and the match ladder for one item:
//! exact/stem/synonym = 1.0, profile more specific = 1.0, profile more general = 0.5,
//! two thirds of a long phrase = 0.5, generic atoms never alone (V1-V4, V15), language
//! levels (V9), years (V8) and degrees.

use std::collections::BTreeMap;

use serde_json::Value;

use super::atoms::{self, Fit, fold};
use super::job::{Class, Item, contains_word, level_in};
use super::legacy::LegacyProfile;
use super::lexicon::engine as lex;
use super::params::{E_FULL, E_HALF, E_NONE};
use super::types::Via;

/// A profile entry (core competence, tool, certificate, ...).
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub text: String,
    pub path: String,
    pub atoms: Vec<String>,
    pub years: Option<u32>,
}

/// What the new engine uses from the profile.
#[derive(Debug, Clone, Default)]
pub(crate) struct Skills {
    pub entries: Vec<Entry>,
    /// Language stem and CEFR level (1-7).
    pub languages: Vec<(String, u8)>,
    /// Fields of the profile's degrees; `None` = no degree stated.
    pub degree_fields: Option<Vec<&'static str>>,
    pub total_years: Option<u32>,
}

fn years_of(value: &Value) -> Option<u32> {
    lex::KEYS_YEARS
        .iter()
        .find_map(|k| value.get(*k).and_then(Value::as_u64))
        .and_then(|y| u32::try_from(y).ok())
}

impl Skills {
    pub(crate) fn new(legacy: &LegacyProfile, data: &Value) -> Self {
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
        let entries: Vec<Entry> = legacy
            .signals
            .core
            .iter()
            .map(|core| Entry {
                atoms: atoms::atoms(&core.text),
                years: years.get(&core.text).copied(),
                text: core.text.clone(),
                path: core.path.clone(),
            })
            .filter(|e| !e.atoms.is_empty())
            .collect();
        let languages = data
            .get(lex::KEY_LANGUAGES)
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
            .collect();
        let mut degree_fields: Option<Vec<&'static str>> = None;
        for core in &legacy.signals.core {
            let folded = fold(&core.text);
            let is_degree = core.path.ends_with("abschluss")
                || lex::DEGREE_WORDS.iter().any(|w| folded.contains(w));
            if is_degree {
                let fields = degree_fields.get_or_insert_with(Vec::new);
                fields.extend(
                    lex::DEGREE_FIELDS
                        .iter()
                        .filter(|(w, _)| folded.contains(w))
                        .map(|&(_, f)| f),
                );
            }
        }
        let total_years = years_of(data).or_else(|| years.values().copied().max());
        Self {
            entries,
            languages,
            degree_fields,
            total_years,
        }
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

/// One entry against the atoms of one alternative.
fn entry_fit(job: &[String], expanded: &[String], entry: &[String]) -> (u16, Via) {
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
        return (E_FULL, Via::Specific);
    }
    if entry.len() >= 3 && 3 * matched >= 2 * entry.len() && !generic_only {
        return (E_HALF, Via::General);
    }
    (E_NONE, Via::Exact)
}

fn skill_fit(skills: &Skills, text: &str) -> ItemFit {
    let job = atoms::atoms(text);
    if job.is_empty() {
        return NONE;
    }
    let expanded = expand(&job);
    let mut best = NONE;
    for (index, entry) in skills.entries.iter().enumerate() {
        let (value, via) = entry_fit(&job, &expanded, &entry.atoms);
        if value > best.value {
            best = ItemFit {
                value,
                entry: Some(index),
                via,
            };
        }
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

fn degree_fit(skills: &Skills, text: &str) -> ItemFit {
    let Some(fields) = &skills.degree_fields else {
        return NONE;
    };
    let folded = fold(text);
    let wanted: Vec<&str> = lex::DEGREE_FIELDS
        .iter()
        .filter(|(w, _)| folded.contains(w))
        .map(|&(_, f)| f)
        .collect();
    let comparable = lex::COMPARABLE_WORDS.iter().any(|w| folded.contains(w));
    let value = if wanted.is_empty() || comparable || wanted.iter().any(|w| fields.contains(w)) {
        E_FULL
    } else {
        E_HALF
    };
    ItemFit {
        value,
        entry: None,
        via: Via::Exact,
    }
}

/// The ladder for one item (best alternative), then the years requirement.
pub(crate) fn item_fit(skills: &Skills, item: &Item) -> ItemFit {
    let mut fit = match &item.class {
        Class::Frame => NONE,
        Class::Language(language, level) => language_fit(skills, language, *level),
        Class::Degree => degree_fit(skills, &item.text),
        Class::Soft | Class::Skill => {
            let mut alternatives: Vec<&str> =
                item.alternatives.iter().map(String::as_str).collect();
            if alternatives.is_empty() {
                alternatives.push(&item.text);
            }
            alternatives
                .iter()
                .map(|alt| skill_fit(skills, alt))
                .max_by_key(|f| f.value)
                .unwrap_or(NONE)
        }
    };
    if let Some(needed) = item.years {
        let folded = fold(&item.text);
        let general = lex::GENERAL_EXPERIENCE
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
