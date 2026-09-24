//! The old Python engine (`ca9a2cd^:matcher.py`, "v3") end to end, for parity tests.
//!
//! [`legacy_percent`] reproduces the old percentage and its explanation lists exactly,
//! measured against `core/tests/fixtures/matching/legacy.json`. The single documented
//! deviation is [`int_rate`]: a trailing decimal comma is a decimal comma
//! (`1.200,50` -> 1200; the old engine read 120050).

use serde::Serialize;
use serde_json::Value;

use super::criteria::{self, Violation};
use super::ladder::{self, Hit, Phrase};
use super::lexicon::{self, HeadingKind};
use super::normalize::{char_len, rstrip, splitlines, strip};
use super::profile::{self, Criteria, Signals};
use super::requirements::{self, Crash, Requirement};
use super::score::{self, LegacyBasis, LegacyCounts};
use super::sections::{self, ReqKind};
use super::signals;

pub use super::profile::Term;
pub use super::signals::{JobSignals, int_rate};

/// Texts shorter than this (code points, after stripping) were skipped by the old reader.
pub const MIN_TEXT_CHARS: usize = 100;
/// Longest label of a requirement in the old output.
const LABEL_MAX_CHARS: usize = 140;

/// The old result row of one job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyScore {
    pub pct: u8,
    pub coverage: u8,
    pub mode: LegacyMode,
    /// Met requirements (labels as the old engine printed them).
    pub matched: Vec<String>,
    /// Open must requirements (or missing vocabulary terms).
    pub missing: Vec<String>,
    /// Violated hard criteria (old German texts).
    pub violations: Vec<String>,
    /// Number of (term, value) hits; the old overview counted a job as matched if > 0.
    pub term_hits: usize,
    pub must_total: usize,
    pub nice_total: usize,
    pub nice_covered: usize,
}

/// Which list the old score came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyMode {
    Requirements,
    Vocab,
}

/// What the old engine did with one job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyOutcome {
    Scored(LegacyScore),
    /// Too little text or nothing assessable: the job did not appear.
    Absent,
    /// `IndexError: no such group` on an inline heading (the whole old run failed).
    Crashed,
    /// The profile is not a JSON object (`ProfileError`).
    InvalidProfile,
}

/// The old percentage and lists of `text` (a job body) against `profile`; `None` when
/// the old engine showed no row for it (absent, crash or invalid profile).
pub fn legacy_percent(profile: &Value, text: &str) -> Option<LegacyScore> {
    match legacy_outcome(profile, text) {
        LegacyOutcome::Scored(score) => Some(score),
        _ => None,
    }
}

/// Like [`legacy_percent`], but tells the reasons for a missing row apart.
pub fn legacy_outcome(profile: &Value, text: &str) -> LegacyOutcome {
    if !profile.is_object() {
        return LegacyOutcome::InvalidProfile;
    }
    let body = file_body(text);
    if char_len(&body) < MIN_TEXT_CHARS {
        return LegacyOutcome::Absent;
    }
    let compiled = LegacyProfile::new(profile);
    match evaluate(&compiled, &body, true) {
        Err(Crash) => LegacyOutcome::Crashed,
        Ok(evaluation) => evaluation
            .legacy_score()
            .map_or(LegacyOutcome::Absent, LegacyOutcome::Scored),
    }
}

/// The body as the old reader passed it on: lines re-joined with `\n`, stripped.
pub(crate) fn file_body(text: &str) -> String {
    strip(&splitlines(text).join("\n")).to_owned()
}

/// A job description file in the TXT contract format, read like the old engine did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobFile {
    pub title: String,
    pub company: String,
    pub location: String,
    pub url: String,
    pub source: String,
    pub text: String,
}

/// Python `parse_job_file`: header lines up to the first blank line, then the text;
/// `None` below [`MIN_TEXT_CHARS`].
pub fn parse_job_file(content: &str) -> Option<JobFile> {
    let lines = splitlines(content);
    let mut file = JobFile {
        title: String::new(),
        company: String::new(),
        location: String::new(),
        url: String::new(),
        source: String::new(),
        text: String::new(),
    };
    let mut index = 0;
    for (i, line) in lines.iter().enumerate() {
        index = i;
        if strip(line).is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = strip(value).to_owned();
            match strip(key) {
                "Titel" => file.title = value,
                "Unternehmen" => file.company = value,
                "Ort" => file.location = value,
                "Link" => file.url = value,
                "Quelle" => file.source = value,
                _ => {}
            }
        }
    }
    strip(&lines.get(index + 1..).unwrap_or_default().join("\n")).clone_into(&mut file.text);
    (char_len(&file.text) >= MIN_TEXT_CHARS).then_some(file)
}

/// The profile as the old engine used it.
pub(crate) struct LegacyProfile {
    pub signals: Signals,
    /// Phrase terms in term order.
    pub phrases: Vec<Phrase>,
    pub criteria: Criteria,
}

impl LegacyProfile {
    pub(crate) fn new(data: &Value) -> Self {
        let signals = profile::signals(data);
        let phrases = profile::build_terms(&signals)
            .iter()
            .filter(|t| t.phrase)
            .map(|t| Phrase::new(t.match_key()))
            .collect();
        Self {
            criteria: profile::criteria(data),
            signals,
            phrases,
        }
    }
}

/// One assessed requirement item.
#[derive(Debug, Clone)]
pub(crate) struct ItemResult<'a> {
    pub requirement: Requirement<'a>,
    /// The item as a slice of the job text.
    pub item: &'a str,
    pub hits: Vec<Hit>,
}

/// Everything the old engine computed for one job.
pub(crate) struct Evaluation<'a> {
    pub items: Vec<ItemResult<'a>>,
    pub vocab: Vec<(String, Vec<Hit>)>,
    pub violations: Vec<Violation>,
    pub counts: LegacyCounts,
}

/// Python `score_jobs` for one job (`legacy` keeps the inline-heading crash).
pub(crate) fn evaluate<'a>(
    profile: &LegacyProfile,
    text: &'a str,
    legacy: bool,
) -> Result<Evaluation<'a>, Crash> {
    let (requirements, skills) = requirements::extract(text, legacy)?;
    let mut counts = LegacyCounts::default();
    let mut items = Vec::new();
    for requirement in requirements {
        for item in requirements::split_items(requirement.phrase) {
            let kern = requirements::core_tokens(item);
            if kern.is_empty() {
                continue;
            }
            let hits = ladder::match_tokens(&kern, &profile.phrases);
            match requirement.kind {
                ReqKind::Must => {
                    counts.must_total += 1;
                    counts.must_covered += usize::from(!hits.is_empty());
                }
                ReqKind::Nice => {
                    counts.nice_total += 1;
                    counts.nice_covered += usize::from(!hits.is_empty());
                }
            }
            items.push(ItemResult {
                requirement,
                item,
                hits,
            });
        }
    }
    let vocab: Vec<(String, Vec<Hit>)> = skills
        .into_iter()
        .map(|skill| {
            let hits = ladder::match_item(&skill, &profile.phrases);
            (skill, hits)
        })
        .collect();
    counts.vocab_total = vocab.len();
    counts.vocab_matched = vocab.iter().filter(|(_, hits)| !hits.is_empty()).count();
    let signals = signals::analyze(text);
    let violations = criteria::check(&profile.criteria, &signals);
    counts.violations = violations.len();
    Ok(Evaluation {
        items,
        vocab,
        violations,
        counts,
    })
}

impl Evaluation<'_> {
    /// The old result row; `None` when nothing was assessable.
    pub(crate) fn legacy_score(&self) -> Option<LegacyScore> {
        let (pct, coverage, basis) = score::legacy(&self.counts)?;
        let term_hits = self.items.iter().map(|i| i.hits.len()).sum::<usize>()
            + self.vocab.iter().map(|(_, hits)| hits.len()).sum::<usize>();
        let (mode, matched, missing) = if basis == LegacyBasis::Vocab {
            let matched = self
                .vocab
                .iter()
                .filter(|(_, hits)| !hits.is_empty())
                .map(|(skill, _)| format!("{skill}{}", lexicon::LABEL_VOCAB))
                .collect();
            let missing = self
                .vocab
                .iter()
                .filter(|(_, hits)| hits.is_empty())
                .map(|(s, _)| s.clone())
                .collect();
            (LegacyMode::Vocab, matched, missing)
        } else {
            let mut matched = Vec::new();
            let mut missing = Vec::new();
            for item in &self.items {
                let label = label(item.item, item.requirement.kind);
                if !item.hits.is_empty() {
                    matched.push(label);
                } else if item.requirement.kind == ReqKind::Must {
                    missing.push(label);
                }
            }
            (LegacyMode::Requirements, matched, missing)
        };
        Some(LegacyScore {
            pct,
            coverage,
            mode,
            matched,
            missing,
            violations: self.violations.iter().map(Violation::legacy_text).collect(),
            term_hits,
            must_total: self.counts.must_total,
            nice_total: self.counts.nice_total,
            nice_covered: self.counts.nice_covered,
        })
    }
}

/// Python label: parentheses become spaces, stripped, at most 140 characters.
pub(crate) fn label(item: &str, kind: ReqKind) -> String {
    let text = item.replace(['(', ')'], " ");
    let text = strip(&text);
    let mut label = if char_len(text) <= LABEL_MAX_CHARS {
        text.to_owned()
    } else {
        let cut: String = text.chars().take(LABEL_MAX_CHARS - 1).collect();
        format!("{}…", rstrip(&cut))
    };
    if kind == ReqKind::Nice {
        label.push_str(lexicon::LABEL_NICE);
    }
    label
}

/// Python `build_terms` for a profile.
pub fn build_terms(profile: &Value) -> Vec<Term> {
    profile::build_terms(&profile::signals(profile))
}

/// Python `canonical_stream(_tokens(text))`.
pub fn canonical_tokens(text: &str) -> Vec<String> {
    super::normalize::canonical_stream(super::normalize::tokens(text))
}

/// Python `canonical_phrase`.
pub fn canonical_phrase(text: &str) -> String {
    super::normalize::canonical_phrase(text)
}

/// Python `_term_hits`: how often a term occurs in a canonical token stream.
pub fn term_hits(tokens: &[String], term: &Term) -> usize {
    let key = term.match_key();
    if term.phrase {
        usize::from(format!(" {} ", tokens.join(" ")).contains(&format!(" {key} ")))
    } else {
        tokens.iter().filter(|t| **t == key).count()
    }
}

/// Python `_section_kind` as the old strings (`must`, `nice`, `task`, `neutral`).
pub fn section_kind(line: &str) -> Option<&'static str> {
    sections::section_kind(line).map(|kind| match kind {
        HeadingKind::Must => "must",
        HeadingKind::Nice => "nice",
        HeadingKind::Task => "task",
        HeadingKind::Neutral => "neutral",
    })
}

/// A requirement phrase and its kind (`must` or `nice`).
pub type LegacyRequirement = (String, &'static str);

/// Python `_extract_requirements`: requirement phrases and vocabulary terms; `Err` for the
/// old crash on inline headings.
pub fn extract_requirements(
    text: &str,
) -> Result<(Vec<LegacyRequirement>, Vec<String>), LegacyOutcome> {
    let (requirements, skills) =
        requirements::extract(text, true).map_err(|Crash| LegacyOutcome::Crashed)?;
    let phrases = requirements
        .into_iter()
        .map(|r| {
            (
                r.phrase.to_owned(),
                if r.kind == ReqKind::Must {
                    "must"
                } else {
                    "nice"
                },
            )
        })
        .collect();
    Ok((phrases, skills))
}

/// Python `analyze_job`.
pub fn analyze_job(text: &str) -> JobSignals {
    signals::analyze(text)
}
