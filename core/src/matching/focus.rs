//! Schwerpunkte (`schwerpunkte`, `ENGINE_VERSION` 4): the three to five core competences the
//! consultant wants to be booked for. Off while the key is missing.
//!
//! A Schwerpunkt is the profile entry with its text plus that competence's `auch` terms. A
//! requirement (section or sentence stage, must or nice, met in full) that names such an
//! entry in full weighs `FOCUS_FACTOR` times in `M` and `K`. A Schwerpunkt named in full by
//! a requirement or by the title is demanded and adds `FOCUS_RELEVANCE` to the relevance
//! (at most `FOCUS_RELEVANCE_MAX` of them); a narrower requirement names it in half (shown,
//! no effect). An ad that demands none gets nothing, never less.

use serde_json::Value;

use super::atoms::fold;
use super::fit::{self, ItemFit, Skills};
use super::job::{Class, Item, Stage};
use super::lexicon;
use super::params::{E_FULL, E_NONE, FOCUS_MAX};
use super::types::Via;

/// A Schwerpunkt and the profile entries that are it.
#[derive(Debug, Clone)]
pub(crate) struct Focus {
    /// As written in the profile.
    pub text: String,
    /// JSON path, e.g. `schwerpunkte[1]`.
    pub path: String,
    /// Indices into [`Skills::entries`]: the competence and its alternative terms.
    pub entries: Vec<usize>,
}

/// Texts of a profile list: strings, or objects with a text field (`kompetenz`, `name`).
pub(crate) fn texts(value: &Value) -> Option<Vec<String>> {
    let items = value.as_array()?;
    Some(
        items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.as_str()),
                Value::Object(fields) => lexicon::KEYS_ITEM_TEXT
                    .iter()
                    .find_map(|k| fields.get(*k).and_then(Value::as_str)),
                _ => None,
            })
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// The Schwerpunkte of a profile: (text, path) of the first `FOCUS_MAX`, the number the
/// profile names, and the key with its value when it names none that can be read.
pub(crate) struct Declared {
    pub items: Vec<(String, String)>,
    pub count: usize,
    pub unreadable: Option<(&'static str, String)>,
}

pub(crate) fn declared(data: &Value) -> Declared {
    let found = lexicon::KEYS_FOCUS
        .iter()
        .find_map(|key| data.get(*key).filter(|v| !v.is_null()).map(|v| (*key, v)));
    let Some((key, value)) = found else {
        return Declared {
            items: Vec::new(),
            count: 0,
            unreadable: None,
        };
    };
    let list = texts(value).unwrap_or_default();
    let unreadable = list.is_empty().then(|| (key, value.to_string()));
    Declared {
        count: list.len(),
        items: list
            .into_iter()
            .take(FOCUS_MAX)
            .enumerate()
            .map(|(i, text)| (text, format!("{key}[{i}]")))
            .collect(),
        unreadable,
    }
}

/// Maps the declared Schwerpunkte onto the profile entries; a text no entry carries is
/// returned as not understood (key, text).
pub(crate) fn resolve(
    skills: &Skills,
    declared: &[(String, String)],
) -> (Vec<Focus>, Vec<(&'static str, String)>) {
    let mut focus = Vec::new();
    let mut unreadable = Vec::new();
    for (text, path) in declared {
        let folded = fold(text);
        let entries: Vec<usize> = skills
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                fold(&e.text) == folded || e.alias_of.as_deref().is_some_and(|c| fold(c) == folded)
            })
            .map(|(i, _)| i)
            .collect();
        if entries.is_empty() {
            let key = lexicon::KEYS_FOCUS
                .iter()
                .find(|k| path.starts_with(**k))
                .copied()
                .unwrap_or(lexicon::KEYS_FOCUS[0]);
            unreadable.push((key, text.clone()));
            continue;
        }
        focus.push(Focus {
            text: text.clone(),
            path: path.clone(),
            entries,
        });
    }
    (focus, unreadable)
}

/// The Schwerpunkt a requirement names (the best, the first on a tie) and whether in full:
/// every word of the Schwerpunkt in the requirement (exact, stem, synonym, light compound)
/// while the requirement itself is met in full (`item_value`); a narrower requirement
/// (`Tableau im Reporting` for `Reporting`) names it in half. A broader requirement that
/// only lies inside the Schwerpunkt (`SAP S/4HANA` for `SAP S/4HANA Finance`) does not
/// name it: the ad asks for less than the Schwerpunkt.
pub(crate) fn of_item(
    skills: &Skills,
    focus: &[Focus],
    item: &Item,
    item_value: u16,
) -> Option<(usize, bool)> {
    if item.class != Class::Skill || item.stage == Stage::Vocabulary {
        return None;
    }
    let (index, fit) = focus
        .iter()
        .enumerate()
        .map(|(i, f)| (i, fit::focus_fit(skills, item, &f.entries)))
        .filter(|(_, fit)| fit.value > E_NONE && fit.via != Via::Specific)
        .fold(
            None,
            |best: Option<(usize, ItemFit)>, (i, fit)| match best {
                Some((_, b)) if b.value >= fit.value => best,
                _ => Some((i, fit)),
            },
        )?;
    Some((index, fit.value == E_FULL && item_value == E_FULL))
}

/// Which Schwerpunkte the title names.
pub(crate) fn in_title(skills: &Skills, focus: &[Focus], title: &str) -> Vec<bool> {
    focus
        .iter()
        .map(|f| fit::title_names(skills, title, &f.entries))
        .collect()
}
