//! Input and output types of the matching engine. Self-contained: the integrator maps
//! them onto the IPC and store types.

use serde::Serialize;
use serde_json::{Map, Value};

pub use crate::model::KeyFacts;
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
    /// Hiring company or agency as the portal or mail gave it (industry wish).
    pub company: &'a str,
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
    /// Rate, start, duration, remote share and contract type as read from the ad.
    pub facts: KeyFacts,
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
    /// A formal requirement (degree field, licence) the profile does not hold: a check
    /// with a score cap, decided only when the ad makes it mandatory.
    FormalOpen,
    LowEvidence,
    ShortText,
    /// Stated annual salary (`salary`, `min`): decided below the minimum for a stated
    /// permanent role in EUR per year, otherwise a check.
    Salary,
    /// A permanent role without a salary statement.
    SalaryUnknown,
    /// A permanent role outside the profile's region without enough remote share
    /// (`location`): decided for a stated permanent role, a check when inferred.
    PermanentRegion,
    /// The work location of a permanent role is unclear (country only, none given, or an
    /// unclear contract type).
    PermanentRegionUnclear,
    /// The target profile asks for fewer years than the profile's minimum (`years`, `max`,
    /// `target`).
    TooJunior,
    /// Years or level of the target profile are unclear (topic-specific years, junior title).
    SeniorityUnclear,
    /// The profile is clearly more senior than the target profile (`years`, `target`).
    Overqualified,
    /// Contract type inferred from the ad (`type`: interim, permanent, anue, unclear;
    /// `inferred` when only indirect cues were found).
    ContractType,
    /// A staffing agency without contract details: temporary agency work is possible.
    AnueRisk,
    /// A Schwerpunkt of the profile the ad demands (`focus`, `met`, `partial`, `inTitle`,
    /// `relevance`): its requirements met in full count double.
    Focus,
    /// The title matches a target role (`role`, `fit` full or half, `points`).
    TargetRole,
    /// Day rate wish (`state` met, near, missed or unknown, `points`, `wish`, `rate`,
    /// `hourly`, `currency`).
    DayRateWish,
    /// Remote wish (`state`, `points`, `min` or `onsite`, `level` full, mostly, partly or
    /// onSite, the ad's share `share` or `from`/`to`).
    RemoteWish,
    /// Region wish (`state`, `points`, `location`, `remote`).
    RegionWish,
    /// Industry wish (`state`, `points`, `industry`, `wish`).
    IndustryWish,
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
    /// Minimum annual salary for permanent roles.
    MinSalary,
    /// Places (and minimum remote share) for permanent roles.
    PermanentRegion,
    /// Minimum years the target profile of an ad must ask for.
    TargetYears,
}

/// State of one hard criterion for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CriterionStatus {
    /// Not set in the profile, or not for this kind of job (a salary for a freelance role).
    Inactive,
    /// Set, and the ad says nothing that shows whether it is met.
    NotMentioned,
    /// Met, with the ad's value as evidence.
    Ok,
    Check,
    Violated,
}

/// A hard criterion for one job, with the reason that decided it and the ad's value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionState {
    pub key: CriterionKey,
    pub status: CriterionStatus,
    pub reason: Option<u16>,
    /// The ad's value: `rate`, `hourly`, `currency`, `rateOpen` (day rate); `start` (`now`,
    /// `vague` or an ISO date); `location` or `remote` (countries, region); `contract`
    /// (ANUE); `salary`; `years` (target years).
    pub params: Map<String, Value>,
    /// Where the ad states it, in UTF-16 offsets (start, end).
    pub range: Option<(u32, u32)>,
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
    /// A criterion key is present but its value cannot be read (`key`, `value`).
    CriterionNotUnderstood,
    /// A remote minimum for permanent roles without places: the region rule stays off.
    RegionWithoutPlaces,
    /// More Schwerpunkte than count (`count`, `max`): the first ones are used.
    FocusTrimmed,
    /// Keys of a criteria section the engine does not read (`keys`).
    IgnoredKeys,
}

/// A wish of the profile (`einsatzpraeferenzen`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WishKey {
    /// Schwerpunkte (`schwerpunkte`).
    Focus,
    /// Target roles (`wunschrollen`).
    TargetRoles,
    /// Wished day rate (`tagessatz_wunsch`).
    DayRate,
    /// Wished remote share (`remote`).
    Remote,
    /// Wished regions (`regionen`).
    Regions,
    /// Wished industries (`branchen`).
    Industries,
}

/// A wish as understood from the profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WishInfo {
    pub key: WishKey,
    pub set: bool,
    pub params: Map<String, Value>,
}

/// An alternative term of a profile competence (`auch` / `aliases`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasInfo {
    /// The competence as written.
    pub competence: String,
    /// The alternative term.
    pub alias: String,
    /// JSON path of the alias, e.g. `kernkompetenzen[2].auch[0]`.
    pub path: String,
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
    /// Alternative terms of competences.
    pub aliases: Vec<AliasInfo>,
    /// Domain packs switched on by the profile's competences (e.g. `finance`, `sap`).
    pub packs: Vec<String>,
    /// Total years of professional experience, if stated.
    pub years: Option<u32>,
    /// Degrees as written in the profile.
    pub degrees: Vec<String>,
    /// Schwerpunkte, target roles and the wishes, each with `set`.
    pub wishes: Vec<WishInfo>,
}
