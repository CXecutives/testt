//! A job text as the new engine reads it: sections with inline headings, frame and
//! English headings (V5), three stages - sections, requirement sentences, vocabulary (V16),
//! items with AND parts, OR alternatives and examples (V6) and the kind of every item (V7).

use std::ops::Range;

use super::atoms::{self, Vocab, fold};
use super::lexicon::{HeadingKind, engine as lex};
use super::normalize::{splitlines, strip};
use super::requirements::extract_job_skills;
use super::sections::{self, Inline, ReqKind};

/// Where an item came from (V16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Section,
    Sentence,
    Vocabulary,
}

/// What an item asks for (V7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Class {
    Skill,
    /// A language (stem) with the required CEFR level (1-7), if stated.
    Language(String, Option<u8>),
    Degree,
    /// A licence or admission (the lexicon word, e.g. `steuerberater`).
    Licence(&'static str),
    Soft,
    Frame,
}

/// One requirement item: met when one of its alternatives is met.
#[derive(Debug, Clone)]
pub(crate) struct Item {
    /// Byte range of the whole item in the text (vocabulary terms have none).
    pub span: Option<Range<usize>>,
    pub text: String,
    pub alternatives: Vec<String>,
    pub kind: ReqKind,
    pub class: Class,
    /// Required years of experience, if stated.
    pub years: Option<u32>,
    pub stage: Stage,
}

/// The requirements of a job.
#[derive(Debug, Clone, Default)]
pub(crate) struct JobDoc {
    pub items: Vec<Item>,
    /// Byte ranges of requirement lines (the "requirements" field of the relevance).
    pub requirement_lines: Vec<Range<usize>>,
}

use super::lexicon::engine::{
    ABBREVIATIONS, AND, AND_WORDS, EN_CUES, EXAMPLE_SEPARATORS, EXAMPLES, MUST_PREFIXES, NICE_CUES,
    NICE_PREFIXES, NUMBER_WORDS, OR, OTHER_PREFIXES, YEAR_UNITS,
};

fn heading(line: &str) -> Option<HeadingKind> {
    if sections::is_bullet(line) {
        return None;
    }
    let norm = fold(&sections::norm_heading(line));
    if norm.is_empty() || norm.len() > 64 {
        return None;
    }
    let starts = |list: &[&str]| list.iter().any(|p| norm.starts_with(p));
    if starts(OTHER_PREFIXES) {
        return Some(HeadingKind::Neutral);
    }
    if let Some(kind) = sections::section_kind(line) {
        return Some(kind);
    }
    if starts(NICE_PREFIXES) {
        Some(HeadingKind::Nice)
    } else if starts(MUST_PREFIXES) {
        Some(HeadingKind::Must)
    } else {
        None
    }
}

fn offset(text: &str, part: &str) -> usize {
    (part.as_ptr() as usize).saturating_sub(text.as_ptr() as usize)
}

fn nice_cue(line: &str) -> bool {
    let folded = fold(line);
    NICE_CUES.iter().any(|c| folded.contains(c))
}

type Phrase<'a> = (&'a str, ReqKind, Stage);

/// Stage 1: requirement and nice-to-have sections; also the offsets of nice lines.
fn section_phrases(text: &str) -> (Vec<Phrase<'_>>, Vec<usize>) {
    let mut current: Option<HeadingKind> = None;
    let mut phrases: Vec<Phrase<'_>> = Vec::new();
    let mut in_nice = Vec::new();
    for line in splitlines(text) {
        let stripped = strip(line);
        if stripped.is_empty() {
            continue;
        }
        match sections::inline_heading(stripped) {
            Inline::Section(kind, rest) => {
                current = Some(if kind == ReqKind::Must {
                    HeadingKind::Must
                } else {
                    HeadingKind::Nice
                });
                let phrase = sections::clean_phrase(rest);
                if !phrase.is_empty() {
                    phrases.push((phrase, kind, Stage::Section));
                }
                continue;
            }
            Inline::Other => {
                current = Some(HeadingKind::Neutral);
                continue;
            }
            Inline::None => {}
        }
        if let Some(kind) = heading(stripped) {
            current = Some(kind);
            continue;
        }
        // `Interessiert? Dann freuen wir uns auf Ihre Bewerbung.` closes the ad.
        if lex::CLOSING_WORDS
            .iter()
            .any(|w| fold(stripped).contains(w))
        {
            current = Some(HeadingKind::Neutral);
            continue;
        }
        let kind = match current {
            Some(HeadingKind::Must) if nice_cue(stripped) => ReqKind::Nice,
            Some(HeadingKind::Must) => ReqKind::Must,
            Some(HeadingKind::Nice) => {
                in_nice.push(offset(text, stripped));
                ReqKind::Nice
            }
            _ => continue,
        };
        for sentence in sentences(stripped) {
            let phrase = sections::clean_phrase(sentence);
            if !phrase.is_empty() {
                phrases.push((phrase, kind, Stage::Section));
            }
        }
    }
    (phrases, in_nice)
}

/// Reads the requirements of a job text.
pub(crate) fn read(text: &str, vocab: &Vocab) -> JobDoc {
    let mut doc = JobDoc::default();
    let (mut phrases, in_nice) = section_phrases(text);
    if !phrases.iter().any(|(_, kind, _)| *kind == ReqKind::Must) {
        for line in splitlines(text) {
            let stripped = strip(line);
            if stripped.is_empty()
                || heading(stripped).is_some()
                || in_nice.contains(&offset(text, stripped))
            {
                continue;
            }
            for sentence in sentences(stripped) {
                let phrase = sections::clean_phrase(sentence);
                let english = EN_CUES.iter().any(|c| fold(phrase).contains(c));
                let kind = sections::cue_kind(phrase).or(english.then_some(ReqKind::Must));
                if let Some(kind) = kind {
                    let kind = if nice_cue(phrase) {
                        ReqKind::Nice
                    } else {
                        kind
                    };
                    phrases.push((phrase, kind, Stage::Sentence));
                }
            }
        }
    }
    for (phrase, kind, stage) in phrases {
        let start = offset(text, phrase);
        doc.requirement_lines.push(start..start + phrase.len());
        let level = level_in(phrase);
        for (span, alternatives) in split(phrase) {
            let whole = &phrase[span.clone()];
            if atoms::atoms(whole, vocab).is_empty() {
                continue;
            }
            let class = classify(whole, level, vocab);
            doc.items.push(Item {
                span: Some(start + span.start..start + span.end),
                text: whole.to_owned(),
                alternatives: alternatives
                    .into_iter()
                    .map(|r| phrase[r].to_owned())
                    .collect(),
                kind,
                class,
                years: years_in(whole),
                stage,
            });
        }
    }
    if doc.items.is_empty() {
        for term in extract_job_skills(text) {
            doc.items.push(Item {
                span: None,
                alternatives: vec![term.clone()],
                text: term,
                kind: ReqKind::Must,
                class: Class::Skill,
                years: None,
                stage: Stage::Vocabulary,
            });
        }
    }
    doc
}

/// Sentences of a line: split after `.!?;` and whitespace before an uppercase letter or a
/// digit, but not after abbreviations (`z. B.`, `u. a.`, `inkl.`).
pub(crate) fn sentences(line: &str) -> Vec<&str> {
    let text = strip(line);
    let mut parts = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (i, &(at, c)) in chars.iter().enumerate() {
        if !matches!(c, '.' | '!' | '?' | ';') {
            continue;
        }
        let Some(&(_, next)) = chars.get(i + 1) else {
            continue;
        };
        if !next.is_whitespace() {
            continue;
        }
        let Some(&(word_at, first)) = chars[i + 1..].iter().find(|(_, c)| !c.is_whitespace())
        else {
            continue;
        };
        if !(first.is_uppercase() || first.is_ascii_digit()) {
            continue;
        }
        let word = text[..at]
            .rsplit(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or("");
        if c == '.'
            && (word.chars().count() <= 2 || ABBREVIATIONS.contains(&word.to_lowercase().as_str()))
        {
            continue;
        }
        parts.push(&text[start..=at]);
        start = word_at;
    }
    parts.push(&text[start..]);
    parts
        .into_iter()
        .map(strip)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Byte ranges of (ASCII, case-insensitive) separators at parenthesis depth 0.
fn separators(text: &str, words: &[&str]) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            _ => {}
        }
        if depth == 0
            && let Some(word) = words.iter().find(|w| {
                bytes
                    .get(i..i + w.len())
                    .is_some_and(|b| b.eq_ignore_ascii_case(w.as_bytes()))
            })
        {
            found.push(i..i + word.len());
            i += word.len();
            continue;
        }
        i += 1;
    }
    found
}

fn cut(text: &str, seps: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut parts = Vec::new();
    let mut start = 0;
    for sep in seps {
        // Overlapping separators (", " inside ", oder ") merge into one cut.
        if sep.start < start {
            start = start.max(sep.end);
            continue;
        }
        parts.push(start..sep.start);
        start = sep.end;
    }
    parts.push(start..text.len());
    parts
        .into_iter()
        .map(|r| trim_range(text, r))
        .filter(|r| !r.is_empty())
        .collect()
}

fn trim_range(text: &str, range: Range<usize>) -> Range<usize> {
    let part = &text[range.clone()];
    let trimmed =
        part.trim_matches(|c: char| c.is_whitespace() || c == '.' || c == ',' || c == ';');
    let start = range.start + (trimmed.as_ptr() as usize - part.as_ptr() as usize);
    start..start + trimmed.len()
}

/// First example marker in a text: (start, end) byte offsets.
fn example_marker(text: &str) -> Option<(usize, usize)> {
    let lower = text.to_lowercase();
    if lower.len() != text.len() {
        return None;
    }
    EXAMPLES
        .iter()
        .filter_map(|m| lower.find(m).map(|at| (at, at + m.len())))
        // Same total length does not mean same offsets ("İẞ" folds to "i̇ß").
        .filter(|&(at, end)| text.is_char_boundary(at) && text.is_char_boundary(end))
        .min()
}

/// Items of a requirement phrase: (item range, alternative ranges), relative to `phrase`.
pub(crate) fn split(phrase: &str) -> Vec<(Range<usize>, Vec<Range<usize>>)> {
    let lower = fold(phrase);
    let has_or = OR.iter().any(|w| lower.contains(w));
    let has_and = AND_WORDS.iter().any(|w| lower.contains(w));
    let and_seps: Vec<Range<usize>> = if has_or && !has_and {
        separators(phrase, &["; "])
    } else {
        separators(phrase, AND)
    };
    let mut items: Vec<(Range<usize>, Vec<Range<usize>>)> = Vec::new();
    for part in cut(phrase, &and_seps) {
        let text = &phrase[part.clone()];
        let marker = example_marker(text);
        // "z. B. LucaNet" after a comma: examples of the previous item.
        if let Some((0, after)) = marker
            && let Some((_, alternatives)) = items.last_mut()
        {
            alternatives.extend(example_list(phrase, part.start + after..part.end));
            continue;
        }
        let mut alternatives = Vec::new();
        let close = |open: usize| text[open..].find(')').map_or(text.len(), |c| open + c);
        let (head, examples) = match (text.find('('), marker) {
            // "(z. B. LucaNet)": only a marker inside the brackets opens them; "(SAP) z. B."
            // is a marker after a closed bracket.
            (Some(open), Some((start, after))) if start > open && close(open) >= after => (
                part.start..part.start + open,
                Some(part.start + after..part.start + close(open)),
            ),
            (_, Some((start, after))) => (
                part.start..part.start + start,
                Some(part.start + after..part.end),
            ),
            _ => (part.clone(), None),
        };
        let head = trim_range(phrase, head);
        let or_seps: Vec<Range<usize>> = separators(&phrase[head.clone()], OR)
            .into_iter()
            .map(|r| head.start + r.start..head.start + r.end)
            .collect();
        if has_or && !has_and {
            let comma: Vec<Range<usize>> = separators(&phrase[head.clone()], &[", "])
                .into_iter()
                .map(|r| head.start + r.start..head.start + r.end)
                .collect();
            let mut all: Vec<Range<usize>> = or_seps.into_iter().chain(comma).collect();
            all.sort_by_key(|r| r.start);
            alternatives.extend(cut_abs(phrase, &head, &all));
        } else {
            alternatives.extend(cut_abs(phrase, &head, &or_seps));
        }
        if let Some(list) = examples {
            alternatives.extend(example_list(phrase, list));
        }
        if !head.is_empty() {
            items.push((part, alternatives));
        }
    }
    items
}

fn cut_abs(phrase: &str, within: &Range<usize>, seps: &[Range<usize>]) -> Vec<Range<usize>> {
    let local: Vec<Range<usize>> = seps
        .iter()
        .map(|r| r.start - within.start..r.end - within.start)
        .collect();
    cut(&phrase[within.clone()], &local)
        .into_iter()
        .map(|r| within.start + r.start..within.start + r.end)
        .collect()
}

fn example_list(phrase: &str, list: Range<usize>) -> Vec<Range<usize>> {
    let list = trim_range(phrase, list);
    let seps: Vec<Range<usize>> = separators(&phrase[list.clone()], EXAMPLE_SEPARATORS)
        .into_iter()
        .map(|r| list.start + r.start..list.start + r.end)
        .collect();
    cut_abs(phrase, &list, &seps)
}

/// Highest language level word in a text.
pub(crate) fn level_in(text: &str) -> Option<u8> {
    let folded = fold(text);
    lex::LEVEL_WORDS
        .iter()
        .filter(|(word, _)| contains_word(&folded, word))
        .map(|&(_, level)| level)
        .max()
}

/// Whole-word (or whole-phrase) containment in a folded text.
pub(crate) fn contains_word(folded: &str, word: &str) -> bool {
    let mut from = 0;
    while let Some(at) = folded[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = folded[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after = folded[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

fn starts_with_any(atom: &str, stems: &[&str]) -> bool {
    stems.iter().any(|s| atom.starts_with(s))
}

fn classify(text: &str, phrase_level: Option<u8>, vocab: &Vocab) -> Class {
    let folded = fold(text);
    let tokens: Vec<&str> = atoms::raw_tokens(&folded).collect();
    if tokens.iter().any(|t| starts_with_any(t, lex::FRAME_WORDS)) {
        return Class::Frame;
    }
    if let Some(word) = lex::LICENCE_WORDS.iter().find(|w| folded.contains(**w))
        && lex::LICENCE_CONTEXT.iter().any(|c| folded.contains(c))
    {
        return Class::Licence(word);
    }
    let content = atoms::atoms(text, vocab);
    let soft = tokens
        .iter()
        .filter(|t| starts_with_any(t, lex::SOFT_SKILLS))
        .count();
    if soft > 0 && 2 * soft >= content.len() {
        return Class::Soft;
    }
    if let Some(language) = tokens
        .iter()
        .map(|t| (*t).to_owned())
        .chain(content.iter().cloned())
        .find_map(|t| lex::LANGUAGES.iter().find(|l| t.starts_with(**l)))
    {
        return Class::Language((*language).to_owned(), level_in(text).or(phrase_level));
    }
    if tokens.iter().any(|t| starts_with_any(t, lex::DEGREE_WORDS)) {
        return Class::Degree;
    }
    Class::Skill
}

/// Required years: `mindestens 10 Jahre`, `10+ years`, `zehn Jahre`.
pub(crate) fn years_in(text: &str) -> Option<u32> {
    let folded = fold(text);
    let words: Vec<&str> = folded
        .split(|c: char| !c.is_alphanumeric() && c != '+')
        .filter(|w| !w.is_empty())
        .collect();
    words.windows(2).find_map(|pair| {
        let unit = pair[1];
        if !YEAR_UNITS.iter().any(|u| unit.starts_with(u)) {
            return None;
        }
        let number = pair[0].trim_end_matches('+');
        number
            .parse::<u32>()
            .ok()
            .or_else(|| {
                NUMBER_WORDS
                    .iter()
                    .find(|(w, _)| *w == number)
                    .map(|&(_, n)| n)
            })
            .filter(|n| (1..=40).contains(n))
    })
}

/// Does the lexicon know this heading text? (for tests and the relevance fields)
#[cfg(test)]
pub(crate) fn heading_kind(line: &str) -> Option<HeadingKind> {
    heading(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(phrase: &str) -> Vec<(String, Vec<String>)> {
        split(phrase)
            .into_iter()
            .map(|(r, alts)| {
                (
                    phrase[r].to_owned(),
                    alts.into_iter().map(|a| phrase[a].to_owned()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn and_or_and_examples() {
        let got = items("Kenntnisse in LucaNet oder IBM Cognos");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].1, ["Kenntnisse in LucaNet", "IBM Cognos"]);
        let got = items("Fundierte Kenntnisse in Konsolidierungswerkzeugen, z. B. LucaNet");
        assert_eq!(got.len(), 1);
        assert_eq!(
            got[0].1,
            [
                "Fundierte Kenntnisse in Konsolidierungswerkzeugen",
                "LucaNet"
            ]
        );
        let got = items("Mehrjährige Erfahrung im Controlling, inkl. Budgetierung");
        assert_eq!(got.len(), 2);
        let got = items("Sicherer Umgang mit Power BI / Tableau");
        assert_eq!(got[0].1, ["Sicherer Umgang mit Power BI", "Tableau"]);
        let got = items(
            "Kenntnisse in Analysewerkzeugen (u. a. Python, R, VBA, Power BI) und praktische Erfahrung mit KI",
        );
        assert_eq!(got.len(), 2);
        assert!(got[0].1.contains(&"Power BI".to_owned()), "{got:?}");
    }

    #[test]
    fn overlapping_separators_and_a_marker_after_brackets() {
        // ", " and " oder " overlap: one cut, no inverted range.
        let got =
            items("Studium der Wirtschaftswissenschaften, oder eine vergleichbare Qualifikation");
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(
            got[0].1,
            [
                "Studium der Wirtschaftswissenschaften",
                "eine vergleichbare Qualifikation"
            ]
        );
        // The bracket closes before "z. B.": the examples follow the marker.
        let got = items("Erfahrung mit einem ERP-System (SAP) z. B. S/4HANA");
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(
            got[0].1,
            ["Erfahrung mit einem ERP-System (SAP)", "S/4HANA"],
            "{got:?}"
        );
        // Folding that keeps the length but moves the offsets ("İ" grows, "ẞ" shrinks).
        let got = items("İẞ Kenntnisse z. B. SAP");
        assert!(!got.is_empty(), "{got:?}");
    }

    #[test]
    fn kinds_years_levels() {
        assert_eq!(
            classify("Ausgeprägte Kommunikationsstärke", None, &Vocab::core()),
            Class::Soft
        );
        assert_eq!(
            classify("Reisebereitschaft", None, &Vocab::core()),
            Class::Frame
        );
        assert_eq!(
            classify("Englischkenntnisse auf Niveau C1", None, &Vocab::core()),
            Class::Language("englisch".into(), Some(5))
        );
        assert_eq!(
            classify(
                "Abgeschlossenes Studium der Wirtschaftswissenschaften",
                None,
                &Vocab::core()
            ),
            Class::Degree
        );
        assert_eq!(
            classify(
                "Zulassung als Steuerberater:in zwingend erforderlich",
                None,
                &Vocab::core()
            ),
            Class::Licence("steuerberater")
        );
        assert_eq!(
            classify(
                "Enge Zusammenarbeit mit Steuerberatern",
                None,
                &Vocab::core()
            ),
            Class::Skill
        );
        assert_eq!(
            years_in("Mindestens 15 Jahre Erfahrung in der Konsolidierung"),
            Some(15)
        );
        assert_eq!(years_in("Mindestens zehn Jahre Berufserfahrung"), Some(10));
        assert_eq!(years_in("At least 8 years of experience"), Some(8));
        assert_eq!(heading_kind("Ihre Qualifikation"), Some(HeadingKind::Must));
        assert_eq!(
            heading_kind("Rahmenbedingungen"),
            Some(HeadingKind::Neutral)
        );
    }
}
