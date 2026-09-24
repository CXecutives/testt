//! IPC v3: what the interface receives and sends - defined once here (and next to the run
//! types in `pipeline`), TypeScript types are generated from it (`view::ts`, tests only).
//!
//! Rules: camelCase, `null` instead of missing fields, data enums tagged with `kind`, never
//! prose - notices, states and errors are codes with data; the words live in the UI catalog.
//! Company and location are cleaned here (the database holds the raw mail values).

use std::path::PathBuf;
use std::sync::LazyLock;

use jiff::Timestamp;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::ErrorInfo;
use crate::fetch::policy::{Allowance, Policy, limits};
use crate::fetch::{PortalHealth, RETRY_AFTER};
use crate::model::{
    Band, DescStatus, MatchRecord, MatchStatus, Notice, band, gmail_url, is_usable_title,
};
use crate::pipeline::{RunSnapshot, RunSummary};
use crate::portal::{JobKey, Portal};
use crate::settings::{PortalSwitches, Settings};
use crate::store::{AlertMailRow, JobRow, PageQuery, Store};
use crate::text::split_company_location;

#[cfg(test)]
mod ts;

/// Most jobs one page of the list carries.
pub const MAX_PAGE: u32 = 500;

// ---------------------------------------------------------------------- Jobs

/// How the job is done, as far as the location field says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum WorkMode {
    Remote,
    Hybrid,
    Onsite,
}

/// Work mode words in a location ("Berlin (Remote)", "Hybrid", "Vor Ort") - the same words
/// the mail heuristics keep (`text::page_location`). German mail patterns, do not translate.
static REMOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:remote|home ?office)\b").unwrap());
static HYBRID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bhybrid\b").unwrap());
static ONSITE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:vor ort|on-site|onsite)\b").unwrap());

/// Work mode of a raw location value; remote and on-site together count as hybrid.
pub fn work_mode(location: &str) -> Option<WorkMode> {
    let (remote, onsite) = (REMOTE.is_match(location), ONSITE.is_match(location));
    if HYBRID.is_match(location) || (remote && onsite) {
        Some(WorkMode::Hybrid)
    } else if remote {
        Some(WorkMode::Remote)
    } else if onsite {
        Some(WorkMode::Onsite)
    } else {
        None
    }
}

/// State of the job details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum DetailState {
    Ok,
    /// Not fetched yet; `retryAt: null` = with the next run.
    Pending {
        retry_at: Option<Timestamp>,
    },
    /// Only the teaser a guest sees (freelance.de without sign-in).
    Teaser,
    /// The last attempts found no description; retried from `retryAt`.
    Failed {
        attempts: u32,
        retry_at: Option<Timestamp>,
    },
    /// The ad no longer exists.
    Gone,
    /// Given up after several failed attempts.
    Unfetchable,
}

impl DetailState {
    pub fn of(job: &JobRow) -> DetailState {
        match job.desc_status {
            DescStatus::Ok => DetailState::Ok,
            DescStatus::Missing => DetailState::Pending { retry_at: None },
            DescStatus::Failed => DetailState::Failed {
                attempts: u32::try_from(job.desc_attempts).unwrap_or(0),
                retry_at: job
                    .desc_attempted_at
                    .and_then(|at| at.checked_add(RETRY_AFTER).ok()),
            },
            DescStatus::Teaser => DetailState::Teaser,
            DescStatus::Gone => DetailState::Gone,
            DescStatus::Unfetchable => DetailState::Unfetchable,
        }
    }
}

/// The match of a job in the list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobMatch {
    /// 0-100; also kept for excluded jobs.
    pub score: u8,
    pub band: Band,
    pub status: MatchStatus,
    pub note: Option<Notice>,
    pub must_met: u16,
    pub must_total: u16,
    /// At most two met requirements, quoted from the ad.
    pub top: Vec<String>,
}

/// One row of the job list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobView {
    pub key: JobKey,
    pub portal: Portal,
    /// Empty when neither the mail nor the link carries a usable title.
    pub title: String,
    pub company: String,
    pub location: String,
    pub work_mode: Option<WorkMode>,
    pub mail_date: Option<Timestamp>,
    pub first_seen_at: Timestamp,
    pub unread: bool,
    pub pinned: bool,
    pub detail: DetailState,
    /// The full text is short (verified, but under 100 characters).
    pub short: bool,
    #[serde(rename = "match")]
    #[cfg_attr(test, ts(rename = "match"))]
    pub match_: Option<JobMatch>,
    /// The same job was also announced by these portals.
    pub also_on: Vec<Portal>,
}

impl From<&JobRow> for JobView {
    fn from(job: &JobRow) -> JobView {
        let (company, location) = split_company_location(&job.company, &job.location);
        JobView {
            key: job.key.clone(),
            portal: job.key.portal,
            title: display_title(job),
            company,
            location,
            work_mode: work_mode(&job.location),
            mail_date: job.mail_date,
            first_seen_at: job.first_seen_at,
            unread: job.read_at.is_none(),
            pinned: job.pinned_at.is_some(),
            detail: DetailState::of(job),
            short: job.desc_status == DescStatus::Ok && job.desc_short,
            match_: job.match_.as_ref().map(JobMatch::from),
            also_on: Vec::new(),
        }
    }
}

impl From<&MatchRecord> for JobMatch {
    fn from(record: &MatchRecord) -> JobMatch {
        JobMatch {
            score: record.score,
            band: band(record.score),
            status: record.status,
            note: record.note.clone(),
            must_met: record.must_met,
            must_total: record.must_total,
            top: record.top.clone(),
        }
    }
}

/// The stored title, or - if it is unusable - one read from the slug of a link.
fn display_title(job: &JobRow) -> String {
    if is_usable_title(&job.title) {
        return job.title.clone();
    }
    [job.title.as_str(), job.url.as_str()]
        .into_iter()
        .find_map(slug_title)
        .unwrap_or_default()
}

/// A readable title from the last path segment of a link
/// (`/projekt/sap-berater-m-w-d-80331` -> "Sap berater (m/w/d)"); `None` without words.
pub fn slug_title(url: &str) -> Option<String> {
    let url = url::Url::parse(url.trim()).ok()?;
    let segment = url.path_segments()?.rfind(|s| !s.is_empty())?;
    let segment = segment.strip_suffix(".html").unwrap_or(segment);
    let decoded = percent_decode(segment);
    let words: Vec<&str> = decoded
        .split(['-', '_', '+'])
        .filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_digit()))
        .filter(|w| !w.eq_ignore_ascii_case("projekt"))
        .collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let gender = words.get(i..i + 3).is_some_and(|w| {
            let lower: Vec<String> = w.iter().map(|s| s.to_lowercase()).collect();
            lower.iter().all(|s| ["m", "w", "d"].contains(&s.as_str())) && lower.len() == 3
        });
        if gender {
            out.push(format!("({})", words[i..i + 3].join("/").to_lowercase()));
            i += 3;
        } else {
            out.push(words[i].to_string());
            i += 1;
        }
    }
    let letters = out
        .iter()
        .filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 2)
        .count();
    if letters < 2 {
        return None;
    }
    let title = out.join(" ");
    let mut chars = title.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(h), Some(l)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Kind of a reason in the match explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ReasonKind {
    Met,
    Partial,
    Open,
    Violation,
    Check,
}

/// Weight of a reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ReasonWeight {
    Must,
    Nice,
    Hard,
    Info,
}

/// Where the profile backs a reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Evidence {
    /// The profile phrase.
    pub profile: String,
    /// JSON path in the profile file.
    pub path: String,
    /// Code of the match ladder step (exact, stem, synonym ...).
    pub via: String,
    /// Words of the ad.
    pub quote: String,
}

/// A range in the full text, in UTF-16 code units (as the browser counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TextRange {
    pub start: u32,
    pub end: u32,
}

/// One reason of the match explanation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Reason {
    pub id: String,
    pub kind: ReasonKind,
    pub weight: ReasonWeight,
    pub code: String,
    /// The requirement as the ad words it (data, not prose of the app).
    pub label: String,
    pub evidence: Option<Evidence>,
    #[cfg_attr(test, ts(type = "Record<string, string | number | boolean | null>"))]
    pub params: serde_json::Map<String, serde_json::Value>,
    pub ranges: Vec<TextRange>,
}

/// A highlighted passage of the full text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Highlight {
    pub id: String,
    /// UTF-16 offsets.
    pub start: u32,
    pub end: u32,
    pub kind: ReasonKind,
    /// Id of the reason it belongs to.
    pub reason: String,
}

/// The full match explanation of one job (reader).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct MatchDetail {
    pub score: u8,
    pub status: MatchStatus,
    pub band: Band,
    /// Revision of engine, profile and model the score was made with.
    pub rev: String,
    pub at: Timestamp,
    pub summary: Option<Notice>,
    /// At most 40.
    pub reasons: Vec<Reason>,
    /// At most 200.
    pub highlights: Vec<Highlight>,
    /// The hard criteria strip.
    pub criteria: Vec<Reason>,
}

/// The alert mail a job came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobMail {
    pub subject: String,
    pub gmail_url: Option<String>,
}

/// One job in the reader.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobDetail {
    pub job: JobView,
    /// The full text (only with details `ok`).
    pub text: Option<String>,
    pub url: String,
    pub fetched_at: Option<Timestamp>,
    pub mail: JobMail,
    #[serde(rename = "match")]
    #[cfg_attr(test, ts(rename = "match"))]
    pub match_: Option<MatchDetail>,
}

/// The reader data of a job; `None` if the job does not exist (any more).
pub fn job_detail(store: &Store, key: &JobKey) -> crate::Result<Option<JobDetail>> {
    let Some(job) = store.job(key)? else {
        return Ok(None);
    };
    Ok(Some(JobDetail {
        text: store.description(key)?,
        url: job.url.to_string(),
        fetched_at: job.desc_fetched_at,
        mail: JobMail {
            subject: job.mail_subject.clone(),
            gmail_url: job.gmail_id.and_then(gmail_url).map(|u| u.to_string()),
        },
        match_: None,
        job: JobView::from(&job),
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum JobFacet {
    New,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum JobSort {
    /// Best match first (excluded jobs behind the others).
    Match,
    Newest,
}

/// Which jobs the list shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobQuery {
    pub facet: JobFacet,
    pub sort: JobSort,
    pub search: Option<String>,
    /// At most [`MAX_PAGE`]; 0 = counts only.
    pub limit: u32,
    pub offset: u32,
}

/// Counts of the list (with the search applied, whatever the facet).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobCounts {
    pub new: u32,
    pub all: u32,
    pub excluded: u32,
    pub high: u32,
    pub no_detail: u32,
}

/// One page of the job list with its counts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobPage {
    pub jobs: Vec<JobView>,
    pub counts: JobCounts,
}

/// List and counts from one store query. "New" means unread and not excluded.
pub fn job_page(store: &Store, query: &JobQuery) -> crate::Result<JobPage> {
    let (rows, counts) = store.job_page(&PageQuery {
        only_new: query.facet == JobFacet::New,
        by_match: query.sort == JobSort::Match,
        search: query.search.clone(),
        limit: query.limit.min(MAX_PAGE),
        offset: query.offset,
    })?;
    Ok(JobPage {
        jobs: rows.iter().map(JobView::from).collect(),
        counts: JobCounts {
            new: counts.new,
            all: counts.all,
            excluded: counts.excluded,
            high: counts.high,
            no_detail: counts.no_detail,
        },
    })
}

/// An alert mail of a run without recognised jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct EmptyAlert {
    pub portal: Portal,
    pub subject: String,
    pub date: Option<Timestamp>,
    /// Gmail message id (hexadecimal) - opened through `open_target`.
    pub gmail_id: Option<String>,
}

/// Longest subject an event or summary carries (in characters).
pub const MAX_SUBJECT_CHARS: usize = 160;

impl From<&AlertMailRow> for EmptyAlert {
    fn from(row: &AlertMailRow) -> EmptyAlert {
        EmptyAlert {
            portal: row.portal,
            subject: crate::text::truncate_chars(&row.subject, MAX_SUBJECT_CHARS),
            date: row.mail_date,
            gmail_id: row.gmail_id.map(|id| format!("{id:x}")),
        }
    }
}

// ---------------------------------------------------------------------- App state

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Platform {
    Windows,
    Macos,
}

/// Where the Gmail app password is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum VaultKind {
    WindowsCredentialManager,
    MacosKeychain,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Mailbox {
    /// The stored Gmail address; `null` = no mailbox connected.
    pub user: Option<String>,
    pub vault: VaultKind,
    /// The vault could not be read.
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SettingsView {
    /// Effective workspace (chosen or default).
    pub workspace: PathBuf,
    pub workspace_is_default: bool,
    /// Number of the app's text files - exactly what "delete text files" removes.
    pub txt_files: usize,
    pub excel_exists: bool,
}

/// Changes of the settings. `null` = unchanged; the workspace only changes through the
/// folder dialog (`pick_workspace`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SettingsPatch {
    pub portals: Vec<PortalPatch>,
    pub auto_fetch_on_start: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PortalPatch {
    pub portal: Portal,
    pub enabled: Option<bool>,
    pub fetch_details: Option<bool>,
    pub login_enabled: Option<bool>,
}

impl SettingsPatch {
    pub fn apply(&self, settings: &mut Settings) {
        for patch in &self.portals {
            let switches: &mut PortalSwitches = settings.portals.entry(patch.portal).or_default();
            if let Some(on) = patch.enabled {
                switches.enabled = on;
            }
            if let Some(on) = patch.fetch_details {
                switches.fetch_details = on;
            }
            if let Some(on) = patch.login_enabled {
                switches.login_enabled = on;
            }
        }
        if let Some(on) = self.auto_fetch_on_start {
            settings.auto_fetch_on_start = on;
        }
    }
}

/// Whether a portal offers a sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum PortalLogin {
    None,
    Optional,
}

/// Risk of the portal switches: low (public pages), grey (guest access, no account
/// affected), account (signed in - the user's account is at stake).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Risk {
    Low,
    Grey,
    Account,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Quota {
    pub used_hour: usize,
    pub cap_hour: usize,
    pub used_day: usize,
    pub cap_day: usize,
}

/// A portal in the settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PortalState {
    pub portal: Portal,
    pub enabled: bool,
    pub fetch_details: bool,
    pub login: PortalLogin,
    pub login_enabled: bool,
    /// `null` = unknown (or no sign-in), `false` = sign-in needed.
    pub signed_in: Option<bool>,
    pub risk: Risk,
    pub health: PortalHealth,
    pub quota: Option<Quota>,
}

/// The state of every portal. `empty_mails`: alert mails without recognised jobs of the
/// last mailbox run.
pub fn portal_states(
    policy: &Policy,
    settings: &Settings,
    empty_mails: &[AlertMailRow],
    now: Timestamp,
) -> Vec<PortalState> {
    Portal::ALL
        .into_iter()
        .map(|portal| {
            let state = policy.state(portal);
            let switches = settings.portal(portal);
            let limits = limits(portal);
            let (used_hour, used_day) = policy.usage(portal, now);
            let login = if portal.access().can_sign_in() {
                PortalLogin::Optional
            } else {
                PortalLogin::None
            };
            // Nothing remembered means "unknown": only a sign-in or a page that asks for one
            // turns it into a statement.
            let signed_in = match (login, state.login_needed, state.session_confirmed_at) {
                (PortalLogin::Optional, true, _) => Some(false),
                (PortalLogin::Optional, false, Some(_)) => Some(true),
                _ => None,
            };
            let risk = match (portal, login) {
                (Portal::Freelancermap, _) => Risk::Low,
                (_, PortalLogin::Optional) if switches.login_enabled || signed_in == Some(true) => {
                    Risk::Account
                }
                _ => Risk::Grey,
            };
            let empty = empty_mails.iter().filter(|m| m.portal == portal).count();
            let health = match policy.allowance(portal, now) {
                Allowance::Paused { until, reason, .. } => PortalHealth::Paused {
                    until: Some(until),
                    reason,
                },
                Allowance::Quota { next_at } => PortalHealth::QuotaReached { until: next_at },
                Allowance::Go if signed_in == Some(false) => PortalHealth::LoginRequired,
                Allowance::Go if empty > 0 || state.suspicious_streak > 0 => {
                    PortalHealth::LayoutSuspect {
                        empty_mails: empty,
                        pages: state.suspicious_streak,
                    }
                }
                Allowance::Go => PortalHealth::Ok,
            };
            PortalState {
                portal,
                enabled: switches.enabled,
                fetch_details: switches.fetch_details,
                login,
                login_enabled: switches.login_enabled,
                signed_in,
                risk,
                health,
                quota: Some(Quota {
                    used_hour,
                    cap_hour: limits.per_hour,
                    used_day,
                    cap_day: limits.per_day,
                }),
            }
        })
        .collect()
}

/// How much the app understood of the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ProfileQuality {
    Good,
    Thin,
    /// Nothing usable - jobs are not scored.
    Empty,
}

/// "What the app understood" of the profile.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileUnderstanding {
    pub competence_count: u32,
    pub competences: Vec<String>,
    pub sources: Vec<String>,
    pub criteria: Vec<Notice>,
    pub warnings: Vec<Notice>,
}

/// The stored consultant profile.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileInfo {
    /// Name of the file the user chose (else the stored file name).
    pub file_name: String,
    pub bytes: u64,
    pub saved_at: Option<Timestamp>,
    pub quality: Option<ProfileQuality>,
    pub understood: Option<ProfileUnderstanding>,
    pub scored_at: Option<Timestamp>,
    /// Jobs still waiting for a score with this profile.
    pub pending: u32,
    /// The file is no valid JSON object any more (edited by hand).
    pub parse_error: Option<ErrorInfo>,
}

impl ProfileInfo {
    pub fn of(info: &crate::profile::ProfileInfo, file_name: Option<String>) -> ProfileInfo {
        ProfileInfo {
            file_name: file_name.unwrap_or_else(|| crate::profile::PROFILE_FILE.to_string()),
            bytes: info.bytes,
            saved_at: info.saved_at,
            quality: None,
            understood: None,
            scored_at: None,
            pending: 0,
            parse_error: info.parse_error.as_ref().map(ErrorInfo::from),
        }
    }
}

/// Result of "reset everything" after the restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ResetSummary {
    pub removed: usize,
    /// Entries that could not be deleted (details in the log).
    pub failed: usize,
}

/// Everything the interface needs at the start (and after a reload).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AppState {
    pub platform: Platform,
    pub dry_run: bool,
    /// No run has finished yet and no job is known.
    pub first_run: bool,
    /// The run in progress (after a reload the interface picks up from here).
    pub running: Option<RunSnapshot>,
    pub settings: SettingsView,
    pub mailbox: Mailbox,
    pub profile: Option<ProfileInfo>,
    pub portals: Vec<PortalState>,
    pub auto_fetch_on_start: bool,
    pub last_run: Option<RunSummary>,
    pub counts: JobCounts,
    /// The best matches of the last mailbox run (at most five).
    pub top_matches: Vec<JobView>,
    pub match_pending: u32,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub reset_report: Option<ResetSummary>,
}

// ---------------------------------------------------------------------- Commands

/// What `open_target` may open - never an arbitrary path or link from the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum OpenTarget {
    JobUrl {
        key: JobKey,
    },
    /// The alert mail a job came from.
    Gmail {
        key: JobKey,
    },
    /// An alert mail by its Gmail id (e.g. one without recognised jobs).
    AlertMail {
        gmail_id: String,
    },
    /// Home page of a portal ("open in your own browser" after a block).
    PortalHome {
        portal: Portal,
    },
    /// Google page to create an app password.
    AppPasswordPage,
    Workspace,
    Excel,
    Overview,
    LogDir,
}

/// Result of "delete text files".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ClearedTxt {
    pub removed: usize,
    /// File names that could not be deleted (open right now).
    pub failed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::policy::PauseKind;
    use crate::model::Posting;
    use crate::portal::job_link;
    use crate::store::MailRef;

    fn store_with(url: &str, title: &str, company: &str, location: &str) -> (Store, JobKey) {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let link = job_link(url).unwrap();
        let posting = Posting::new(link.key.clone(), link.url, title, company, location);
        let mail = MailRef {
            subject: "Neue Projekte",
            date: None,
            gmail_id: Some(0x1a2b),
        };
        store
            .upsert_posting(run, &posting, mail, Timestamp::now())
            .unwrap();
        (store, link.key)
    }

    #[test]
    fn job_row_is_cleaned_for_display() {
        let (store, key) = store_with(
            "https://www.freelancermap.de/nproj/12345.html",
            "Rolle",
            "von: Muster GmbH",
            "Am Mühlenweg 68, 27356 Rotenburg Wümme (Remote)",
        );
        let view = JobView::from(&store.job(&key).unwrap().unwrap());
        assert_eq!(view.company, "Muster GmbH");
        assert_eq!(view.work_mode, Some(WorkMode::Remote));
        assert_eq!(view.detail, DetailState::Pending { retry_at: None });
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["match"], serde_json::Value::Null, "null, not missing");
        assert_eq!(json["detail"]["kind"], "pending");
        assert_eq!(json["portal"], "freelancermap");
        let detail = job_detail(&store, &key).unwrap().unwrap();
        assert_eq!(
            detail.mail.gmail_url.as_deref(),
            Some("https://mail.google.com/mail/u/0/#all/1a2b")
        );
    }

    #[test]
    fn work_modes_from_the_location() {
        assert_eq!(work_mode("Berlin (Remote)"), Some(WorkMode::Remote));
        assert_eq!(work_mode("Hamburg (Hybrid)"), Some(WorkMode::Hybrid));
        assert_eq!(work_mode("Vor Ort in Köln"), Some(WorkMode::Onsite));
        assert_eq!(work_mode("Remote oder vor Ort"), Some(WorkMode::Hybrid));
        assert_eq!(work_mode("Remotely-Str. 5, München"), None);
        assert_eq!(work_mode("Köln"), None);
    }

    #[test]
    fn an_unusable_title_is_read_from_the_link() {
        assert_eq!(
            slug_title("https://www.freelancermap.de/projekt/sap-berater-m-w-d-80331-muenchen")
                .as_deref(),
            Some("Sap berater (m/w/d) muenchen")
        );
        assert_eq!(
            slug_title(
                "https://www.linkedin.com/jobs/view/sap-production-support-at-siemens-4456653430"
            )
            .as_deref(),
            Some("Sap production support at siemens")
        );
        assert_eq!(
            slug_title("https://www.linkedin.com/jobs/view/4123456789/"),
            None
        );
        assert_eq!(slug_title("kein Link"), None);
        let (store, key) = store_with(
            "https://www.freelancermap.de/projekt/interim-controller-remote",
            "",
            "",
            "",
        );
        let view = JobView::from(&store.job(&key).unwrap().unwrap());
        assert_eq!(view.title, "Interim controller remote");
    }

    fn record(status: MatchStatus, score: u8) -> MatchRecord {
        MatchRecord {
            status,
            score,
            note: None,
            must_met: 1,
            must_total: 2,
            top: Vec::new(),
        }
    }

    /// Four jobs: A read, B high, C excluded with a higher score, D unscored.
    fn four_jobs() -> Store {
        let (store, a) = store_with(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        let run = store.begin_run().unwrap();
        let mut keys = Vec::new();
        for (i, title) in [(2, "B"), (3, "C"), (4, "D")] {
            let link =
                job_link(&format!("https://www.linkedin.com/jobs/view/400000000{i}/")).unwrap();
            let posting = Posting::new(link.key.clone(), link.url, title, "", "");
            let mail = MailRef {
                subject: "x",
                date: None,
                gmail_id: None,
            };
            let at = Timestamp::now() + jiff::SignedDuration::from_mins(i);
            store.upsert_posting(run, &posting, mail, at).unwrap();
            keys.push(link.key);
        }
        store
            .record_text(&keys[0], "Volltext", false, false, Timestamp::now())
            .unwrap();
        store.mark_read(&a, Timestamp::now()).unwrap();
        store
            .save_matches(
                &[
                    (a, record(MatchStatus::Scored, 50)),
                    (keys[0].clone(), record(MatchStatus::Scored, 85)),
                    (keys[1].clone(), record(MatchStatus::Excluded, 95)),
                ],
                "r",
                Timestamp::now(),
            )
            .unwrap();
        store
    }

    fn titles(page: &JobPage) -> Vec<&str> {
        page.jobs.iter().map(|j| j.title.as_str()).collect()
    }

    #[test]
    fn a_page_and_its_counts_come_together() {
        let store = four_jobs();
        let query = |facet, sort, limit, offset| JobQuery {
            facet,
            sort,
            search: None,
            limit,
            offset,
        };
        let expected = JobCounts {
            new: 2,
            all: 4,
            excluded: 1,
            high: 1,
            no_detail: 3,
        };
        // New = unread and not excluded; excluded behind the others, unscored after scored.
        let new = job_page(&store, &query(JobFacet::New, JobSort::Match, 50, 0)).unwrap();
        assert_eq!(titles(&new), ["B", "D"]);
        assert_eq!(new.counts, expected);
        assert!(new.jobs[0].unread && new.jobs[0].match_.is_some());
        let by_match = job_page(&store, &query(JobFacet::All, JobSort::Match, 50, 0)).unwrap();
        assert_eq!(titles(&by_match), ["B", "A", "D", "C"]);
        let newest = job_page(&store, &query(JobFacet::All, JobSort::Newest, 50, 0)).unwrap();
        assert_eq!(titles(&newest), ["D", "B", "A", "C"]);
        assert!(!newest.jobs[2].unread);
        // Past the end or counts only: no rows, the same counts.
        let past = job_page(&store, &query(JobFacet::All, JobSort::Match, 50, 10)).unwrap();
        assert!(past.jobs.is_empty());
        assert_eq!(past.counts, expected);
        let counts_only = job_page(&store, &query(JobFacet::All, JobSort::Match, 0, 0)).unwrap();
        assert_eq!((counts_only.jobs.len(), counts_only.counts), (0, expected));
        // The search narrows list and counts alike.
        let mut search = query(JobFacet::All, JobSort::Match, 50, 0);
        search.search = Some("volltext".into());
        let found = job_page(&store, &search).unwrap();
        assert_eq!((found.jobs.len(), found.counts.all), (1, 1));
    }

    #[test]
    fn portal_state_shows_pause_quota_risk_and_layout() {
        let now = Timestamp::now();
        let mut policy = Policy::in_memory();
        policy.pause(Portal::LinkedIn, PauseKind::Blocked, "HTTP 999", now);
        for _ in 0..25 {
            policy.record_access(Portal::Freelancermap, now);
        }
        let mut settings = Settings::default();
        settings
            .portals
            .get_mut(&Portal::FreelanceDe)
            .unwrap()
            .login_enabled = true;
        let empty = [AlertMailRow {
            portal: Portal::FreelanceDe,
            subject: "Projektvorschläge".into(),
            mail_date: None,
            gmail_id: None,
        }];
        let states = portal_states(&policy, &settings, &empty, now);
        let of = |portal| states.iter().find(|s| s.portal == portal).unwrap();
        let li = of(Portal::LinkedIn);
        assert!(matches!(
            li.health,
            PortalHealth::Paused {
                reason: crate::fetch::policy::PauseReason::Blocked,
                ..
            }
        ));
        assert_eq!(
            (li.login, li.risk, li.signed_in),
            (PortalLogin::None, Risk::Grey, None)
        );
        let fm = of(Portal::Freelancermap);
        assert!(matches!(fm.health, PortalHealth::QuotaReached { .. }));
        assert_eq!(fm.risk, Risk::Low);
        assert_eq!(fm.quota.unwrap().used_hour, 25);
        let fl = of(Portal::FreelanceDe);
        assert_eq!((fl.login, fl.risk), (PortalLogin::Optional, Risk::Account));
        assert_eq!(
            fl.health,
            PortalHealth::LayoutSuspect {
                empty_mails: 1,
                pages: 0
            }
        );
    }

    /// The sign-in state has three values: nothing remembered = unknown.
    #[test]
    fn the_sign_in_state_is_unknown_until_something_happened() {
        let now = Timestamp::now();
        let settings = Settings::default();
        let mut policy = Policy::in_memory();
        let state = |policy: &Policy| {
            portal_states(policy, &settings, &[], now)
                .into_iter()
                .find(|s| s.portal == Portal::FreelanceDe)
                .unwrap()
        };
        assert_eq!(state(&policy).signed_in, None);
        assert_eq!(state(&policy).risk, Risk::Grey);
        policy.set_session(Portal::FreelanceDe, true, now);
        assert_eq!(state(&policy).signed_in, Some(true));
        assert_eq!(state(&policy).risk, Risk::Account);
        policy.set_session(Portal::FreelanceDe, false, now);
        assert_eq!(state(&policy).signed_in, Some(false));
        assert_eq!(state(&policy).health, PortalHealth::LoginRequired);
    }
}
