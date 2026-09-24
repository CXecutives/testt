//! Input and output types of the matching engine. Self-contained: the integrator maps
//! them onto the IPC and store types.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::portal::Portal;

/// How complete the job text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TextKind {
    /// The full description.
    Full,
    /// Only a teaser (for example freelance.de without sign-in).
    Teaser,
}

/// One job to assess.
#[derive(Debug, Clone, Copy)]
pub struct JobInput<'a> {
    pub title: &'a str,
    /// Location as the portal or mail gave it.
    pub location: &'a str,
    pub portal: Portal,
    /// Description body (the TXT text after the header lines).
    pub text: &'a str,
    /// Structured facts of the portal (remote share, start, rate, ...), if any.
    pub facts: Option<&'a Value>,
    /// Date of the alert mail, the reference for start dates.
    pub posted: Option<jiff::civil::Date>,
    pub kind: TextKind,
}

/// Optional semantic similarity (feature experiment; not used yet).
pub trait Embedder: Send + Sync {
    /// Stable model id; part of the match revision.
    fn model_id(&self) -> &str;
    /// Similarity of two short texts in per-mille (0..=1000).
    fn similarity(&self, a: &str, b: &str) -> u16;
}

/// Overall outcome for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Scored,
    /// At least one decided hard-criterion violation; the score is kept for reference.
    Excluded,
    /// Too little text to assess.
    Unscorable,
}

/// How much the text allowed to assess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceLevel {
    Full,
    Low,
    Teaser,
}

/// Counts behind the score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub must_met: u16,
    pub must_partial: u16,
    pub must_open: u16,
    pub must_total: u16,
    pub nice_met: u16,
    pub nice_total: u16,
    pub evidence: EvidenceLevel,
}

/// Result of [`super::assess`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub verdict: Verdict,
    /// 0-100; also set for excluded jobs, 0 for unscorable ones.
    pub score: u8,
    pub summary: Summary,
    pub reasons: Vec<Reason>,
    pub highlights: Vec<Highlight>,
    pub criteria: Vec<CriterionState>,
}

/// Kind of a reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReasonKind {
    Met,
    Partial,
    Open,
    Violation,
    Check,
}

/// Weight class of a reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Weight {
    Must,
    Nice,
    Hard,
    Info,
}

/// What a reason is about; the UI catalog turns code + params into text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReasonCode {
    /// A requirement of the ad (label = quote).
    Requirement,
    /// A vocabulary term found in the ad (no explicit requirements).
    Term,
    Anue,
    DayRate,
    Availability,
    Country,
    AnueOptional,
    AnueHidden,
    CountryUnclear,
    DayRateCurrency,
    /// The job starts before the profile's availability (`days`); never decided.
    AvailabilityGap,
    StartVague,
    Permanent,
    FormalOpen,
    LowEvidence,
    ShortText,
}

/// How a profile entry met a requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Via {
    Exact,
    Stem,
    Synonym,
    /// The profile entry is more specific than the requirement.
    Specific,
    /// The profile entry is more general (counts half).
    General,
    Semantic,
}

/// The profile side of a met requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    /// Profile entry as written.
    pub profile: String,
    /// JSON path of the entry, e.g. `kernkompetenzen[3].kompetenz`.
    pub path: String,
    pub via: Via,
    /// The requirement text it met (at most 120 characters).
    pub quote: String,
}

/// One explained finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reason {
    pub id: u16,
    pub kind: ReasonKind,
    pub weight: Weight,
    pub code: ReasonCode,
    /// Quote from the ad (at most 120 characters).
    pub label: Option<String>,
    pub evidence: Option<Evidence>,
    pub params: Map<String, Value>,
    /// Ids of the highlights that belong to this reason.
    pub ranges: Vec<u16>,
}

/// A passage of the job text, in UTF-16 offsets (JavaScript string indices).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub id: u16,
    pub start: u32,
    pub end: u32,
    pub kind: ReasonKind,
    pub reason: u16,
}

/// The hard criteria of the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CriterionKey {
    MinDayRate,
    Countries,
    NoAnue,
    Availability,
}

/// State of one hard criterion for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CriterionStatus {
    /// Not set in the profile.
    Inactive,
    Ok,
    Check,
    Violated,
}

/// A hard criterion for one job, with the reason that decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionState {
    pub key: CriterionKey,
    pub status: CriterionStatus,
    pub reason: Option<u16>,
}

/// How usable the profile is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileQuality {
    Good,
    /// Few competences; scores are rough.
    Thin,
    /// No competences: nothing is scored.
    Empty,
}

/// Where competences were found in the profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    /// Path pattern, e.g. `kernkompetenzen[].kompetenz`.
    pub path: String,
    pub count: u16,
    /// Found by key name only (not a known profile structure).
    pub guessed: bool,
}

/// A hard criterion as understood from the profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionInfo {
    pub key: CriterionKey,
    pub set: bool,
    pub params: Map<String, Value>,
}

/// Something the user should fix or know about the profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileWarning {
    pub code: ProfileWarningCode,
    pub params: Map<String, Value>,
}

/// Profile warning codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileWarningCode {
    NoCompetences,
    FewCompetences,
    NoCriteria,
    AvailabilityNotUnderstood,
}

/// "What the app understood" of the profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub competence_count: u16,
    /// Up to 40 competences in profile order.
    pub competences: Vec<String>,
    pub sources: Vec<SourceInfo>,
    pub criteria: Vec<CriterionInfo>,
    pub warnings: Vec<ProfileWarning>,
}
