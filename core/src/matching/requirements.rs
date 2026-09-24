//! Requirements of a job text, independent of the profile (old engine semantics).
//!
//! Sources in this order: requirement and nice-to-have sections, requirement sentences
//! (cues) when no must was found, and the vocabulary when still no must was found.

use std::cmp::Reverse;
use std::sync::LazyLock;

use regex::Regex;

use super::lexicon::{self, HeadingKind};
use super::normalize::{canonical_stream, char_len, raw_canonical, splitlines, strip, tokens};
use super::pyre;
use super::sections::{self, Inline, ReqKind};

static ITEM_SPLIT: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::ITEM_SPLIT));

static VOCAB_KEYS: LazyLock<Vec<String>> = LazyLock::new(|| {
    lexicon::JOB_SKILL_VOCAB
        .iter()
        .map(|entry| {
            canonical_stream(entry.split_whitespace().map(str::to_owned).collect()).join(" ")
        })
        .collect()
});

/// Where a requirement came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Section,
    Inline,
    Cue,
}

/// One requirement phrase: a slice of the job text.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Requirement<'a> {
    pub phrase: &'a str,
    pub kind: ReqKind,
    pub origin: Origin,
}

/// The old engine raised `IndexError` on an inline heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Crash;

/// Python `_collect_requirements`. `legacy` reproduces the crash on inline headings;
/// otherwise inline headings work as the old code intended.
pub(crate) fn collect_sections(text: &str, legacy: bool) -> Result<Vec<Requirement<'_>>, Crash> {
    let mut requirements = Vec::new();
    let mut current: Option<HeadingKind> = None;
    for line in splitlines(text) {
        let stripped = strip(line);
        if stripped.is_empty() {
            continue;
        }
        if legacy {
            if sections::is_inline_heading(stripped) {
                return Err(Crash);
            }
        } else if let Inline::Section(kind, rest) = sections::inline_heading(stripped) {
            current = Some(match kind {
                ReqKind::Must => HeadingKind::Must,
                ReqKind::Nice => HeadingKind::Nice,
            });
            let phrase = sections::clean_phrase(rest);
            if !phrase.is_empty() {
                requirements.push(Requirement {
                    phrase,
                    kind,
                    origin: Origin::Inline,
                });
            }
            continue;
        }
        let heading = if sections::is_bullet(stripped) {
            None
        } else {
            sections::section_kind(stripped)
        };
        if heading.is_some() {
            current = heading;
            continue;
        }
        let kind = match current {
            Some(HeadingKind::Must) => ReqKind::Must,
            Some(HeadingKind::Nice) => ReqKind::Nice,
            _ => continue,
        };
        for sentence in sections::split_sentences(stripped) {
            let phrase = sections::clean_phrase(sentence);
            if !phrase.is_empty() {
                requirements.push(Requirement {
                    phrase,
                    kind,
                    origin: Origin::Section,
                });
            }
        }
    }
    Ok(requirements)
}

/// Python `_collect_cue_requirements`: requirement sentences anywhere in the text.
pub(crate) fn collect_cues(text: &str) -> Vec<Requirement<'_>> {
    let mut requirements = Vec::new();
    for line in splitlines(text) {
        let stripped = strip(line);
        if stripped.is_empty() || sections::section_kind(stripped).is_some() {
            continue;
        }
        for sentence in sections::split_sentences(stripped) {
            let phrase = sections::clean_phrase(sentence);
            if let Some(kind) = sections::cue_kind(phrase) {
                requirements.push(Requirement {
                    phrase,
                    kind,
                    origin: Origin::Cue,
                });
            }
        }
    }
    requirements
}

/// Python `_extract_job_skills`: vocabulary terms in the text, longest first, without
/// terms contained in a longer found term.
pub(crate) fn extract_job_skills(text: &str) -> Vec<String> {
    let joined = format!(" {} ", raw_canonical(text).join(" "));
    let mut found: Vec<&String> = Vec::new();
    for key in VOCAB_KEYS.iter() {
        if !key.is_empty() && !found.contains(&key) && joined.contains(&format!(" {key} ")) {
            found.push(key);
        }
    }
    found.sort_by_key(|key| Reverse(char_len(key)));
    let mut result: Vec<String> = Vec::new();
    for key in found {
        let needle = format!(" {key} ");
        if !result
            .iter()
            .any(|longer| format!(" {longer} ").contains(&needle))
        {
            result.push(key.clone());
        }
    }
    result
}

/// Python `_extract_requirements`: (requirements, vocabulary terms).
pub(crate) fn extract(
    text: &str,
    legacy: bool,
) -> Result<(Vec<Requirement<'_>>, Vec<String>), Crash> {
    let mut requirements = collect_sections(text, legacy)?;
    let has_must = |reqs: &[Requirement<'_>]| reqs.iter().any(|r| r.kind == ReqKind::Must);
    if !has_must(&requirements) {
        requirements.extend(collect_cues(text));
    }
    if has_must(&requirements) {
        return Ok((requirements, Vec::new()));
    }
    let skills = extract_job_skills(text);
    Ok((requirements, skills))
}

/// Python `_split_requirement_items`: items of an enumeration.
pub(crate) fn split_items(phrase: &str) -> Vec<&str> {
    ITEM_SPLIT
        .split(phrase)
        .map(strip)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Python `_skill_core_tokens`: canonical tokens of an item plus language stems
/// (`deutschkenntnisse` adds `deutsch`). Empty = nothing assessable.
pub(crate) fn core_tokens(item: &str) -> Vec<String> {
    let cleaned: String = item
        .chars()
        .filter(|c| !lexicon::ITEM_STRIP.contains(c))
        .collect();
    let mut result = canonical_stream(tokens(&cleaned));
    for token in result.clone() {
        if let Some(stem) = language_stem(&token)
            && char_len(stem) >= 3
            && !result.iter().any(|t| t == stem)
        {
            result.push(stem.to_owned());
        }
    }
    result
}

/// `^(.+?)(?:kenntnisse|kenntnis|sprachen?|sprachkenntnisse)$`: the shortest prefix of at
/// least three characters, right-stripped of ` -/`.
fn language_stem(token: &str) -> Option<&str> {
    let (at, _) = token
        .char_indices()
        .skip(1)
        .find(|&(at, _)| lexicon::LANGUAGE_SUFFIXES.contains(&&token[at..]))?;
    let head = &token[..at];
    (char_len(head) >= 3).then(|| head.trim_end_matches([' ', '-', '/']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_and_core_tokens() {
        assert_eq!(
            split_items("Python, R, VBA sowie SQL bzw. Excel; Word & Access oder X"),
            ["Python", "R", "VBA", "SQL", "Excel", "Word", "Access", "X"]
        );
        assert_eq!(
            core_tokens("Verhandlungssichere Deutschkenntnisse"),
            ["deutschkenntnisse", "deutsch"]
        );
        assert_eq!(
            core_tokens("Fremdsprachkenntnisse"),
            ["fremdsprachkenntnisse", "fremd"]
        );
        assert!(core_tokens("u. a.").is_empty());
    }

    #[test]
    fn vocabulary_prefers_longer_terms() {
        let skills =
            extract_job_skills("Unser Kunde betreibt SAP S/4HANA und Power BI im Reporting.");
        assert_eq!(skills, ["sap s/4hana", "reporting", "power bi"]);
    }
}
