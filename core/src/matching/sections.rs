//! Lines, headings and sentences of a job text (old engine semantics).

use std::sync::LazyLock;

use regex::Regex;

use super::lexicon::{self, HeadingKind};
use super::normalize::{casefold, char_len, decimal, is_space, split_whitespace, strip};
use super::pyre;

/// Headings longer than this are ordinary lines.
const HEADING_MAX_CHARS: usize = 64;
/// Cue sentences longer than this are ignored.
const CUE_MAX_CHARS: usize = 400;

static INLINE_HEADING: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::INLINE_HEADING));
static CUE_HARD: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::CUE_HARD));
static CUE_EXPERIENCE: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::CUE_EXPERIENCE));
static CUE_NICE: LazyLock<Regex> = LazyLock::new(|| pyre::compile(lexicon::CUE_NICE));

/// Kind of a requirement: must or nice-to-have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ReqKind {
    Must,
    Nice,
}

/// Does the (stripped) line start with a bullet character?
pub(crate) fn is_bullet(line: &str) -> bool {
    line.starts_with(lexicon::BULLETS)
}

/// `^[bullets]+\s*` removed (only if at least one bullet character leads).
fn strip_bullets(text: &str) -> &str {
    let rest = text.trim_start_matches(lexicon::BULLETS);
    if rest.len() == text.len() {
        text
    } else {
        rest.trim_start_matches(is_space)
    }
}

/// `^\d+[.)]\s*` removed (Python `\d`: any decimal digit).
fn strip_number(text: &str) -> &str {
    let rest = text.trim_start_matches(|c| decimal(c).is_some());
    if rest.len() == text.len() {
        return text;
    }
    match rest.strip_prefix(['.', ')']) {
        Some(after) => after.trim_start_matches(is_space),
        None => text,
    }
}

/// Python `_norm_heading`.
pub(crate) fn norm_heading(line: &str) -> String {
    let text = strip(strip_bullets(strip(line)));
    let text = strip(strip_number(text));
    let text = text.trim_end_matches([':', '：']);
    let kept: String = casefold(text)
        .chars()
        .filter(|&c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, 'ä' | 'ö' | 'ü' | 'ß' | ' ' | '&' | '/' | '-')
        })
        .collect();
    split_whitespace(&kept).collect::<Vec<_>>().join(" ")
}

/// Python `_section_kind`: what a heading line opens, `None` for ordinary lines.
pub(crate) fn section_kind(line: &str) -> Option<HeadingKind> {
    let norm = norm_heading(line);
    if norm.is_empty() || char_len(&norm) > HEADING_MAX_CHARS {
        return None;
    }
    for (table, kind) in [
        (lexicon::NEUTRAL_HEADINGS, HeadingKind::Neutral),
        (lexicon::NICE_HEADINGS, HeadingKind::Nice),
        (lexicon::MUST_HEADINGS, HeadingKind::Must),
        (lexicon::TASK_HEADINGS, HeadingKind::Task),
    ] {
        if lexicon::contains(table, &norm) {
            return Some(kind);
        }
    }
    lexicon::HEADING_PREFIXES
        .iter()
        .find(|(prefix, _)| norm.starts_with(prefix))
        .map(|&(_, kind)| kind)
}

/// Result of the inline-heading check (`Anforderungen: ...` on one line).
pub(crate) enum Inline<'a> {
    /// No inline heading.
    None,
    /// A must/nice heading with the rest of the line as a requirement.
    Section(ReqKind, &'a str),
    /// Another inline heading (tasks, offer, about us): the line stays an ordinary line.
    Other,
}

/// Does the line match the old inline-heading pattern? The old engine crashed here
/// (`IndexError: no such group`) because the pattern had one group, not two.
pub(crate) fn is_inline_heading(line: &str) -> bool {
    INLINE_HEADING.is_match(&pyre::view(line))
}

/// The inline heading as the old code intended it (used by the new engine skeleton).
pub(crate) fn inline_heading(line: &str) -> Inline<'_> {
    let view = pyre::view(line);
    let Some(caps) = INLINE_HEADING.captures(&view) else {
        return Inline::None;
    };
    let (Some(head), Some(rest)) = (caps.get(1), caps.get(2)) else {
        return Inline::None;
    };
    let (from, to) = pyre::original_range(line, &view, head.start(), head.end());
    let head = casefold(&line[from..to]);
    let (from, to) = pyre::original_range(line, &view, rest.start(), rest.end());
    let rest = strip(&line[from..to]);
    let kind = if head == "ihr profil"
        || head == "dein profil"
        || head.starts_with("anforderung")
        || head.starts_with("qualifikation")
        || head.starts_with("voraussetzung")
    {
        ReqKind::Must
    } else if head.starts_with("wünschenswert") || head.starts_with("wuenschenswert") {
        ReqKind::Nice
    } else {
        return Inline::Other;
    };
    Inline::Section(kind, rest)
}

/// Python `_split_sentences`: split after `.!?;` + whitespace before an uppercase letter
/// or digit; parts stripped, empty parts dropped.
pub(crate) fn split_sentences(text: &str) -> Vec<&str> {
    let text = strip(text);
    let mut parts = Vec::new();
    let mut start = 0;
    let mut prev: Option<char> = None;
    let mut iter = text.char_indices().peekable();
    while let Some((at, c)) = iter.next() {
        if !is_space(c) {
            prev = Some(c);
            continue;
        }
        let mut end = at + c.len_utf8();
        while let Some(&(next_at, next)) = iter.peek() {
            if !is_space(next) {
                break;
            }
            end = next_at + next.len_utf8();
            iter.next();
        }
        let before = prev.is_some_and(|p| matches!(p, '.' | '!' | '?' | ';'));
        let after = text[end..].chars().next().is_some_and(|n| {
            n.is_ascii_uppercase() || n.is_ascii_digit() || matches!(n, 'Ä' | 'Ö' | 'Ü')
        });
        if before && after {
            parts.push(&text[start..at]);
            start = end;
        }
        prev = text[at..end].chars().last();
    }
    parts.push(&text[start..]);
    parts
        .into_iter()
        .map(strip)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Python `_clean_phrase`: leading bullets and numbering removed.
pub(crate) fn clean_phrase(sentence: &str) -> &str {
    let text = strip(strip_bullets(strip(sentence)));
    strip(strip_number(text))
}

/// Python `_cue_kind`: requirement kind of a cue sentence, `None` if it is none.
pub(crate) fn cue_kind(sentence: &str) -> Option<ReqKind> {
    if sentence.is_empty() || char_len(sentence) > CUE_MAX_CHARS {
        return None;
    }
    let view = pyre::view(sentence);
    if !(CUE_HARD.is_match(&view) || CUE_EXPERIENCE.is_match(&view)) {
        return None;
    }
    Some(if CUE_NICE.is_match(&view) {
        ReqKind::Nice
    } else {
        ReqKind::Must
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings() {
        assert_eq!(section_kind("Anforderungen:"), Some(HeadingKind::Must));
        assert_eq!(section_kind("- 2) Wünschenswert"), Some(HeadingKind::Nice));
        assert_eq!(section_kind("Ihre Qualifikation"), None);
        assert_eq!(section_kind("Über uns"), Some(HeadingKind::Neutral));
        assert_eq!(norm_heading("• 1. Ihr Profil："), "ihr profil");
    }

    #[test]
    fn sentences() {
        assert_eq!(
            split_sentences("A b. C d; e f. 3 g"),
            ["A b.", "C d; e f.", "3 g"]
        );
        assert_eq!(split_sentences("x.\u{a0} Y"), ["x.", "Y"]);
        assert_eq!(split_sentences("x. \u{200b}Y"), ["x. \u{200b}Y"]);
    }

    #[test]
    fn inline_headings() {
        assert!(is_inline_heading("Anforderungen: SAP"));
        assert!(!is_inline_heading("Anforderungen:"));
        assert!(matches!(
            inline_heading("Ihr Profil: SAP FI"),
            Inline::Section(ReqKind::Must, "SAP FI")
        ));
        assert!(matches!(inline_heading("Wir bieten: Geld"), Inline::Other));
    }
}
