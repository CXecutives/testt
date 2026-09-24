//! IPC v3: what the interface receives and sends - defined once here (and next to the run
//! types in `pipeline`), TypeScript types are generated from it (`view::ts`, tests only).
//!
//! Rules: camelCase, `null` instead of missing fields, data enums tagged with `kind`, never
//! prose - notices, states and errors are codes with data; the words live in the UI catalog.
//! Company and location are cleaned here (the database holds the raw mail values).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use jiff::Timestamp;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::ErrorInfo;
use crate::fetch::policy::{Policy, limits};
use crate::fetch::{PortalHealth, RETRY_AFTER};
use crate::matching::{self, Assessment, ProfileSummary};
use crate::model::{
    AppStatus, Band, DescStatus, KeyFacts, MatchRecord, MatchStatus, Notice, band, gmail_url,
    is_usable_title,
};
use crate::pipeline::{LocalMatcher, Matcher, RunSnapshot, RunSummary, local};
use crate::portal::{JobKey, Portal};
pub use crate::profile::{
    LanguageLevel, ProfileAvailability, ProfileCompetence, ProfileCriteria, ProfileForm,
    ProfileLanguage, ProfileWishes, RemoteWish,
};
use crate::settings::{PortalSwitches, Settings};
use crate::store::{AlertMailRow, JobRow, ListFacet, PageQuery, Store};
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
    /// Rate, start, duration, remote share and contract type of the ad.
    pub facts: KeyFacts,
}

/// One row of the job list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "flat facts of a list row, one JSON field each (IPC contract)"
)]
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
    /// Saved ("Gemerkt", the star): the stage `saved`.
    pub pinned: bool,
    pub detail: DetailState,
    /// The full text is short (verified, but under 100 characters).
    pub short: bool,
    #[serde(rename = "match")]
    #[cfg_attr(test, ts(rename = "match"))]
    pub match_: Option<JobMatch>,
    /// The same job was also announced by these portals.
    pub also_on: Vec<Portal>,
    /// The stage in the user's pipeline (`null` = none; `saved` is the star).
    pub app_status: Option<AppStatus>,
    /// When the stage was set last.
    pub status_at: Option<Timestamp>,
    /// The day to follow up (while applied or in talks).
    #[cfg_attr(test, ts(type = "string | null"))]
    pub follow_up_on: Option<jiff::civil::Date>,
    /// The job is archived (by the user or by age).
    pub archived: bool,
    /// The user marked the job as fitting although the engine excludes it ("Trotzdem
    /// passend"): it counts as scored with its fit score, its note is `userOverride`.
    pub overridden: bool,
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
            pinned: job.app_status == Some(AppStatus::Saved),
            detail: DetailState::of(job),
            short: job.desc_status == DescStatus::Ok && job.desc_short,
            match_: job.match_.as_ref().map(|record| {
                let mut shown = JobMatch::from(record);
                if job.override_include {
                    shown.status = MatchStatus::Scored;
                    shown.note = Some(Notice {
                        code: crate::store::marks::USER_OVERRIDE.to_owned(),
                        params: serde_json::Map::new(),
                    });
                }
                shown
            }),
            also_on: Vec::new(),
            app_status: job.app_status,
            status_at: job.app_status_at,
            follow_up_on: job.follow_up_on,
            archived: job.archived_at.is_some(),
            overridden: job.override_include,
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
            facts: record.facts.clone(),
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
    /// The user's note (`null` = none).
    pub note: Option<String>,
}

/// Most reasons in the reader.
pub const MAX_REASONS: usize = 40;
/// Most highlighted passages in the reader.
pub const MAX_HIGHLIGHTS: usize = 200;

/// The reader data of a job; `None` if the job does not exist (any more).
///
/// Reasons are not stored: with a usable `matcher` the job is assessed again from its stored
/// text. If its stored score has another revision, the fresh one is saved - only with `save`
/// (no run active); otherwise the run's catch-up takes care of it. Either way the reader
/// shows the fresh result.
pub fn job_detail(
    store: &Store,
    key: &JobKey,
    matcher: Option<&LocalMatcher>,
    save: bool,
    now: Timestamp,
) -> crate::Result<Option<JobDetail>> {
    let Some(mut job) = store.job(key)? else {
        return Ok(None);
    };
    let text = store.description(key)?;
    // An engine panic leaves the reader without a match instead of failing the command.
    let assessed = matcher.filter(|m| m.usable()).and_then(|m| {
        let assessment =
            crate::pipeline::score::guarded(key, || m.assessment(&job, text.as_deref()))??;
        Some((m, assessment))
    });
    let match_ = match assessed {
        None => None,
        Some((matcher, assessment)) => {
            let at = if job.match_rev.as_deref() == Some(matcher.rev()) {
                store.match_at(key)?.unwrap_or(now)
            } else {
                let record = local::record(&assessment);
                // Only over the score read above: a run that started meanwhile may have
                // stored its own (compare and set).
                if save
                    && let Err(e) = store.save_match_if(
                        key,
                        &record,
                        matcher.rev(),
                        job.match_rev.as_deref(),
                        now,
                    )
                {
                    log::warn!("fresh score of {key} not stored: {e}");
                }
                job.match_ = Some(record);
                job.match_rev = Some(matcher.rev().to_owned());
                now
            };
            let mut detail = match_detail(&assessment, matcher, at);
            if job.override_include {
                overridden(&mut detail);
                if let Some(record) = job.match_.as_mut() {
                    record.status = MatchStatus::Scored;
                }
            }
            Some(detail)
        }
    };
    Ok(Some(JobDetail {
        note: store.note(key)?,
        text,
        url: job.url.to_string(),
        fetched_at: job.desc_fetched_at,
        mail: JobMail {
            subject: job.mail_subject.clone(),
            gmail_url: job.gmail_id.and_then(gmail_url).map(|u| u.to_string()),
        },
        match_,
        job: job_views(store, std::slice::from_ref(&job))?
            .pop()
            .unwrap_or_else(|| JobView::from(&job)),
    }))
}

/// The reader of a job the user marked as fitting anyway: it counts as scored, its summary
/// is `userOverride`, and a reason with that code comes first (the engine's violations stay
/// listed: the user sees what the engine found).
fn overridden(detail: &mut MatchDetail) {
    let code = crate::store::marks::USER_OVERRIDE;
    detail.status = MatchStatus::Scored;
    detail.summary = Some(Notice {
        code: code.to_owned(),
        params: serde_json::Map::new(),
    });
    detail.reasons.insert(
        0,
        Reason {
            id: code.to_owned(),
            kind: ReasonKind::Met,
            weight: ReasonWeight::Info,
            code: code.to_owned(),
            label: String::new(),
            evidence: None,
            params: serde_json::Map::new(),
            ranges: Vec::new(),
        },
    );
    detail.reasons.truncate(MAX_REASONS);
}

/// The reader's explanation of an assessment: at most [`MAX_REASONS`] reasons (violations,
/// checks and musts before nice-to-haves and info), the highlights of those reasons (at most
/// [`MAX_HIGHLIGHTS`]) and the hard-criteria strip with every criterion the profile sets.
pub fn match_detail(assessment: &Assessment, matcher: &LocalMatcher, at: Timestamp) -> MatchDetail {
    let rank = |r: &matching::Reason| match (r.kind, r.weight) {
        (matching::ReasonKind::Violation, _) => 0,
        (matching::ReasonKind::Check, _) => 1,
        (_, matching::Weight::Must | matching::Weight::Hard) => 2,
        (_, matching::Weight::Nice) => 3,
        (_, matching::Weight::Info) => 4,
    };
    let mut ranked: Vec<&matching::Reason> = assessment.reasons.iter().collect();
    ranked.sort_by_key(|r| rank(r));
    let kept: BTreeSet<u16> = ranked.iter().take(MAX_REASONS).map(|r| r.id).collect();
    let highlights: Vec<&matching::Highlight> = assessment
        .highlights
        .iter()
        .filter(|h| kept.contains(&h.reason))
        .take(MAX_HIGHLIGHTS)
        .collect();
    let ranges = |reason: &matching::Reason| -> Vec<TextRange> {
        highlights
            .iter()
            .filter(|h| reason.ranges.contains(&h.id))
            .map(|h| TextRange {
                start: h.start,
                end: h.end,
            })
            .collect()
    };
    let reasons = assessment
        .reasons
        .iter()
        .filter(|r| kept.contains(&r.id))
        .map(|r| Reason {
            id: reason_id(r.id),
            kind: reason_kind(r.kind),
            weight: reason_weight(r.weight),
            code: local::code_name(&r.code),
            label: r.label.clone().unwrap_or_default(),
            evidence: r.evidence.as_ref().map(|e| Evidence {
                profile: e.profile.clone(),
                path: e.path.clone(),
                via: local::code_name(&e.via),
                quote: e.quote.clone(),
            }),
            params: local::flat_params(&r.params),
            ranges: ranges(r),
        })
        .collect();
    let criteria = criteria_strip(assessment, matcher, &ranges);
    let record = local::record(assessment);
    MatchDetail {
        score: record.score,
        status: record.status,
        band: band(record.score),
        rev: matcher.rev().to_owned(),
        at,
        summary: Some(summary_notice(&assessment.summary)),
        reasons,
        highlights: highlights
            .iter()
            .map(|h| Highlight {
                id: format!("h{}", h.id),
                start: h.start,
                end: h.end,
                kind: reason_kind(h.kind),
                reason: reason_id(h.reason),
            })
            .collect(),
        criteria,
    }
}

/// The hard-criteria strip: every criterion the profile sets that applies to the job, with
/// the profile's values, the ad's value, the reason that decided it (`params.reason`) and
/// the passages. Kind `met` only with the ad's value as evidence; `open` when the ad does
/// not mention it.
fn criteria_strip(
    assessment: &Assessment,
    matcher: &LocalMatcher,
    ranges: &dyn Fn(&matching::Reason) -> Vec<TextRange>,
) -> Vec<Reason> {
    let set = &matcher.profile().summary().criteria;
    assessment
        .criteria
        .iter()
        .filter(|c| c.status != matching::CriterionStatus::Inactive)
        .map(|c| {
            let linked = c
                .reason
                .and_then(|id| assessment.reasons.iter().find(|r| r.id == id));
            let mut params = set
                .iter()
                .find(|info| info.key == c.key)
                .map(|info| local::flat_params(&info.params))
                .unwrap_or_default();
            params.extend(local::flat_params(&c.params));
            if let Some(reason) = linked {
                params.extend(local::flat_params(&reason.params));
                params.insert("reason".into(), reason_id(reason.id).into());
            }
            let mut passages = linked.map(ranges).unwrap_or_default();
            if passages.is_empty()
                && let Some((start, end)) = c.range
            {
                passages.push(TextRange { start, end });
            }
            let code = local::code_name(&c.key);
            Reason {
                id: format!("c:{code}"),
                kind: match c.status {
                    matching::CriterionStatus::Violated => ReasonKind::Violation,
                    matching::CriterionStatus::Check => ReasonKind::Check,
                    matching::CriterionStatus::NotMentioned => ReasonKind::Open,
                    _ => ReasonKind::Met,
                },
                weight: ReasonWeight::Hard,
                code,
                label: linked.and_then(|r| r.label.clone()).unwrap_or_default(),
                evidence: None,
                params,
                ranges: passages,
            }
        })
        .collect()
}

/// The counts behind the score (`n of m must`) and how much text there was.
fn summary_notice(s: &matching::Summary) -> Notice {
    Notice {
        code: "summary".into(),
        params: serde_json::Map::from_iter([
            ("mustMet".into(), s.must_met.into()),
            ("mustPartial".into(), s.must_partial.into()),
            ("mustOpen".into(), s.must_open.into()),
            ("mustTotal".into(), s.must_total.into()),
            ("niceMet".into(), s.nice_met.into()),
            ("niceTotal".into(), s.nice_total.into()),
            ("evidence".into(), local::code_name(&s.evidence).into()),
        ]),
    }
}

fn reason_id(id: u16) -> String {
    format!("r{id}")
}

fn reason_kind(kind: matching::ReasonKind) -> ReasonKind {
    match kind {
        matching::ReasonKind::Met => ReasonKind::Met,
        matching::ReasonKind::Partial => ReasonKind::Partial,
        matching::ReasonKind::Open => ReasonKind::Open,
        matching::ReasonKind::Violation => ReasonKind::Violation,
        matching::ReasonKind::Check => ReasonKind::Check,
    }
}

fn reason_weight(weight: matching::Weight) -> ReasonWeight {
    match weight {
        matching::Weight::Must => ReasonWeight::Must,
        matching::Weight::Nice => ReasonWeight::Nice,
        matching::Weight::Hard => ReasonWeight::Hard,
        matching::Weight::Info => ReasonWeight::Info,
    }
}

/// List rows with the other portals that announced the same job (`alsoOn`).
pub fn job_views(store: &Store, rows: &[JobRow]) -> crate::Result<Vec<JobView>> {
    let keys: Vec<&JobKey> = rows.iter().map(|row| &row.key).collect();
    let mut also = store.also_on(&keys)?;
    Ok(rows
        .iter()
        .map(|row| JobView {
            also_on: also.remove(&row.key).unwrap_or_default(),
            ..JobView::from(row)
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum JobFacet {
    /// Unread of the last 14 days (the excluded ones last, uncounted).
    New,
    All,
    /// Saved (the star), the latest saved first.
    Saved,
    /// In an application stage: a due follow-up first, then the latest change, the rejected
    /// ones last.
    Applications,
    /// Archived, the latest archived first; in no other list or count.
    Archived,
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

/// Counts of the list (with the search applied, whatever the facet). Every number of the
/// page comes from here: the facets, the tiles, the sidebar and the new jobs per portal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobCounts {
    /// Unread and not excluded ("Neu").
    pub new: u32,
    pub all: u32,
    pub excluded: u32,
    /// Scored in the high band.
    pub high: u32,
    /// Without a full text.
    pub no_detail: u32,
    /// Saved ("Gemerkt", the star).
    pub saved: u32,
    /// In an application stage (applied, interview, offer, rejected).
    pub applications: u32,
    /// Archived - the only count an archived job is in.
    pub archived: u32,
    /// `new` per portal: every portal, in the order of `Portal::ALL`.
    pub new_by_portal: Vec<PortalNew>,
}

/// The new jobs ("Neu") of one portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PortalNew {
    pub portal: Portal,
    pub new: u32,
}

/// One page of the job list with its counts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobPage {
    pub jobs: Vec<JobView>,
    pub counts: JobCounts,
}

/// List and counts from one store query. "New" lists the unread jobs, the excluded ones last;
/// its count leaves the excluded ones out. An archived job is only in "Archived".
pub fn job_page(store: &Store, query: &JobQuery) -> crate::Result<JobPage> {
    let (rows, counts) = store.job_page(&PageQuery {
        facet: match query.facet {
            JobFacet::New => ListFacet::New,
            JobFacet::All => ListFacet::All,
            JobFacet::Saved => ListFacet::Saved,
            JobFacet::Applications => ListFacet::Applications,
            JobFacet::Archived => ListFacet::Archived,
        },
        new_since: crate::store::new_since(Timestamp::now()),
        by_match: query.sort == JobSort::Match,
        search: query.search.clone(),
        limit: query.limit.min(MAX_PAGE),
        offset: query.offset,
    })?;
    Ok(JobPage {
        jobs: job_views(store, &rows)?,
        counts: JobCounts {
            new: counts.new,
            all: counts.all,
            excluded: counts.excluded,
            high: counts.high,
            no_detail: counts.no_detail,
            saved: counts.saved,
            applications: counts.applications,
            archived: counts.archived,
            new_by_portal: counts
                .new_by_portal
                .into_iter()
                .map(|(portal, new)| PortalNew { portal, new })
                .collect(),
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
    /// The Excel file of the overview, where it is or will be written.
    pub excel_path: PathBuf,
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
    /// Days after which old jobs archive themselves; 0 = never (`null` = unchanged).
    pub auto_archive_days: Option<u32>,
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
        if let Some(days) = self.auto_archive_days {
            settings.auto_archive_days = days;
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches and states of the portal, as the settings show them"
)]
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
    /// The user has to act on the health ([`PortalHealth::action_needed`]).
    pub action_needed: bool,
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
            let health = PortalHealth::of(policy, portal, now, switches.login_enabled, empty);
            PortalState {
                portal,
                enabled: switches.enabled,
                fetch_details: switches.fetch_details,
                login,
                login_enabled: switches.login_enabled,
                signed_in,
                risk,
                action_needed: health.action_needed(),
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

impl From<matching::ProfileQuality> for ProfileQuality {
    fn from(quality: matching::ProfileQuality) -> ProfileQuality {
        match quality {
            matching::ProfileQuality::Good => ProfileQuality::Good,
            matching::ProfileQuality::Thin => ProfileQuality::Thin,
            matching::ProfileQuality::Empty => ProfileQuality::Empty,
        }
    }
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
    /// Domain packs the profile switched on (`finance`, `sap`, `itProject`, ...).
    pub packs: Vec<String>,
    /// Total years of professional experience, if the profile states them.
    pub years: Option<u32>,
    /// Degrees as written in the profile.
    pub degrees: Vec<String>,
    /// `schwerpunkte`: the competences that matter most.
    pub focus: Vec<String>,
    /// `wunschrollen`: the roles the consultant is looking for.
    pub roles: Vec<String>,
    /// `einsatzpraeferenzen`: wishes, they nudge the score and never exclude.
    pub wishes: ProfileWishes,
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
    /// The profile as the editor shows it (`null` while the file does not read).
    pub form: Option<ProfileForm>,
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
            form: None,
        }
    }

    /// Adds the form of the editor.
    #[must_use]
    pub fn with_form(mut self, form: Option<ProfileForm>) -> Self {
        self.form = form;
        self
    }

    /// Adds what the engine understood of the profile and how far the jobs are scored with
    /// it (an empty profile scores nothing, so nothing is pending).
    pub fn understood_by(mut self, matcher: &LocalMatcher, store: &Store) -> crate::Result<Self> {
        let profile = matcher.profile();
        self.quality = Some(profile.quality().into());
        let mut understood = understanding(profile.summary());
        // The newer profile inputs, as the form reads them.
        if let Some(form) = &self.form {
            understood.focus.clone_from(&form.focus);
            understood.roles.clone_from(&form.roles);
            understood.wishes = form.wishes.clone();
        }
        self.understood = Some(understood);
        if matcher.usable() {
            self.scored_at = store.scored_at(matcher.rev())?;
            self.pending = store.match_pending(matcher.rev())?;
        }
        Ok(self)
    }
}

/// The "understood" card: competences, where they were found (path patterns), every hard
/// criterion as `{code, params}` with `set`, and the warnings.
pub fn understanding(summary: &ProfileSummary) -> ProfileUnderstanding {
    ProfileUnderstanding {
        competence_count: u32::from(summary.competence_count),
        competences: summary.competences.clone(),
        sources: summary.sources.iter().map(|s| s.path.clone()).collect(),
        criteria: summary
            .criteria
            .iter()
            .map(|c| {
                let mut params = local::flat_params(&c.params);
                params.insert("set".into(), c.set.into());
                Notice {
                    code: local::code_name(&c.key),
                    params,
                }
            })
            .collect(),
        warnings: summary
            .warnings
            .iter()
            .map(|w| Notice {
                code: local::code_name(&w.code),
                params: local::flat_params(&w.params),
            })
            .collect(),
        packs: summary.packs.clone(),
        years: summary.years,
        degrees: summary.degrees.clone(),
        focus: Vec::new(),
        roles: Vec::new(),
        wishes: ProfileWishes::default(),
    }
}

/// A profile read for the editor from a chosen file or a pasted answer: nothing is stored
/// until the user saves it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileDraft {
    pub form: ProfileForm,
    /// The JSON the form came from; saving merges the form into it.
    pub source: String,
    pub quality: ProfileQuality,
}

impl From<crate::profile::Draft> for ProfileDraft {
    fn from(draft: crate::profile::Draft) -> ProfileDraft {
        ProfileDraft {
            form: draft.form,
            source: draft.source,
            quality: draft.quality.into(),
        }
    }
}

/// Saving the editor: the form as it was handed out (`before`) and as the user left it;
/// only what differs is written. `source` is the JSON of a draft (`{}` for a new profile),
/// `null` for the stored profile.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileSave {
    pub before: ProfileForm,
    pub after: ProfileForm,
    pub source: Option<String>,
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
    /// Days after which old jobs without a stage archive themselves; 0 = never.
    pub auto_archive_days: u32,
    /// The last fetch (fetch or whole mailbox) - a rescore or a details run is none.
    pub last_run: Option<RunSummary>,
    pub counts: JobCounts,
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
    /// Google page to turn on 2-step verification (an app password needs it).
    TwoStepPage,
    Workspace,
    Excel,
    Overview,
    LogDir,
}

/// Result of a permanent delete of jobs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Deleted {
    /// Jobs deleted (with the duplicates that stood for them).
    pub count: u32,
    /// The overview could not be written again (e.g. open in Excel); `params.target` names
    /// what failed. The jobs are deleted anyway.
    pub export_error: Option<ErrorInfo>,
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
        let detail = job_detail(&store, &key, None, true, Timestamp::now())
            .unwrap()
            .unwrap();
        assert_eq!(detail.match_, None, "no profile, no match");
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
            facts: crate::model::KeyFacts::default(),
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
            saved: 0,
            applications: 0,
            archived: 0,
            new_by_portal: vec![
                PortalNew {
                    portal: Portal::LinkedIn,
                    new: 2,
                },
                PortalNew {
                    portal: Portal::FreelanceDe,
                    new: 0,
                },
                PortalNew {
                    portal: Portal::Freelancermap,
                    new: 0,
                },
            ],
        };
        // New lists every unread job: the excluded one behind the others (grey in the list),
        // unscored after scored. Its count leaves the excluded one out.
        let new = job_page(&store, &query(JobFacet::New, JobSort::Match, 50, 0)).unwrap();
        assert_eq!(titles(&new), ["B", "D", "C"]);
        assert_eq!(&new.counts, &expected);
        assert!(new.jobs[0].unread && new.jobs[0].match_.is_some());
        let excluded = new.jobs[2].match_.as_ref().unwrap();
        assert!(new.jobs[2].unread && excluded.status == MatchStatus::Excluded);
        let newest = job_page(&store, &query(JobFacet::New, JobSort::Newest, 50, 0)).unwrap();
        assert_eq!(titles(&newest), ["D", "B", "C"]);
        let by_match = job_page(&store, &query(JobFacet::All, JobSort::Match, 50, 0)).unwrap();
        assert_eq!(titles(&by_match), ["B", "A", "D", "C"]);
        let newest = job_page(&store, &query(JobFacet::All, JobSort::Newest, 50, 0)).unwrap();
        assert_eq!(titles(&newest), ["D", "B", "A", "C"]);
        assert!(!newest.jobs[2].unread);
        // Past the end or counts only: no rows, the same counts.
        let past = job_page(&store, &query(JobFacet::All, JobSort::Match, 50, 10)).unwrap();
        assert!(past.jobs.is_empty());
        assert_eq!(&past.counts, &expected);
        let counts_only = job_page(&store, &query(JobFacet::All, JobSort::Match, 0, 0)).unwrap();
        assert_eq!(
            (counts_only.jobs.len(), &counts_only.counts),
            (0, &expected)
        );
        // The search narrows list and counts alike.
        let mut search = query(JobFacet::All, JobSort::Match, 50, 0);
        search.search = Some("volltext".into());
        let found = job_page(&store, &search).unwrap();
        assert_eq!((found.jobs.len(), found.counts.all), (1, 1));
    }

    /// Applications and archived jobs: an archived job leaves "New" and "All" and every count but
    /// "archived"; "applications" lists the jobs with a status, the latest change first. List
    /// and counts still agree for every facet.
    #[test]
    fn applications_and_archived_jobs_have_their_own_lists() {
        let store = four_jobs();
        let key = |i: u8| {
            job_link(&format!("https://www.linkedin.com/jobs/view/400000000{i}/"))
                .unwrap()
                .key
        };
        let at = Timestamp::now();
        let later = at + jiff::SignedDuration::from_mins(5);
        // B (high, unread) applied, D (unscored, unread) in talks later, C (excluded) archived.
        store
            .set_app_status(&key(2), Some(AppStatus::Applied), at)
            .unwrap();
        store
            .set_app_status(&key(4), Some(AppStatus::Interview), later)
            .unwrap();
        store.set_pinned(&key(3), true, at).unwrap();
        store.set_archived(&key(3), true, at).unwrap();
        let page = |facet| {
            job_page(
                &store,
                &JobQuery {
                    facet,
                    sort: JobSort::Match,
                    search: None,
                    limit: 50,
                    offset: 0,
                },
            )
            .unwrap()
        };
        let all = page(JobFacet::All);
        assert_eq!(titles(&all), ["B", "A", "D"], "the archived job is gone");
        let counts = &all.counts;
        assert_eq!(
            (counts.all, counts.new, counts.excluded, counts.saved),
            (3, 2, 0, 0),
            "archived in no count but its own"
        );
        assert_eq!((counts.applications, counts.archived), (2, 1));
        assert_eq!(titles(&page(JobFacet::New)), ["B", "D"]);
        let applications = page(JobFacet::Applications);
        assert_eq!(titles(&applications), ["D", "B"], "latest change first");
        assert_eq!(applications.jobs[0].app_status, Some(AppStatus::Interview));
        assert_eq!(
            applications.counts, all.counts,
            "the counts ignore the facet"
        );
        let archived = page(JobFacet::Archived);
        assert_eq!(titles(&archived), ["C"]);
        assert!(archived.jobs[0].archived);
        // The facet lists exactly as many jobs as its count says.
        assert_eq!(u32::try_from(all.jobs.len()).unwrap(), counts.all);
        assert_eq!(
            u32::try_from(applications.jobs.len()).unwrap(),
            counts.applications
        );
        assert_eq!(u32::try_from(archived.jobs.len()).unwrap(), counts.archived);
        // An archived application is in "archived" only.
        store.set_archived(&key(2), true, later).unwrap();
        let after = page(JobFacet::Applications);
        assert_eq!(titles(&after), ["D"]);
        assert_eq!((after.counts.applications, after.counts.archived), (1, 2));
        assert_eq!(
            titles(&page(JobFacet::Archived)),
            ["B", "C"],
            "latest archived first"
        );
    }

    /// The reader has the note and the time of the status.
    #[test]
    fn the_reader_has_the_note_and_the_status_time() {
        let store = four_jobs();
        let key = job_link("https://www.linkedin.com/jobs/view/4000000002/")
            .unwrap()
            .key;
        // Whole seconds, as the store keeps them.
        let at = Timestamp::from_second(Timestamp::now().as_second()).unwrap();
        store
            .set_app_status(&key, Some(AppStatus::Offer), at)
            .unwrap();
        store.set_note(&key, "Zusage per Mail").unwrap();
        let detail = job_detail(&store, &key, None, false, at).unwrap().unwrap();
        assert_eq!(detail.note.as_deref(), Some("Zusage per Mail"));
        assert_eq!(detail.job.status_at, Some(at));
        assert_eq!(detail.job.app_status, Some(AppStatus::Offer));
        let json = serde_json::to_value(&detail).unwrap();
        assert_eq!(json["job"]["appStatus"], "offer");
        assert_eq!(json["job"]["archived"], false);
    }

    /// The new jobs per portal and the pinned ones come with every page, from the same
    /// statement: every portal in the order of `Portal::ALL` (one order on every screen), read
    /// or excluded jobs are no new ones, the search narrows them like the other counts.
    #[test]
    fn new_per_portal_and_pinned_come_with_the_counts() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let add = |url: &str, title: &str| {
            let link = job_link(url).unwrap();
            let posting = Posting::new(link.key.clone(), link.url, title, "", "");
            let mail = MailRef {
                subject: "x",
                date: None,
                gmail_id: None,
            };
            store
                .upsert_posting(run, &posting, mail, Timestamp::now())
                .unwrap();
            link.key
        };
        let map_new = add(
            "https://www.freelancermap.de/nproj/12345.html",
            "Controlling",
        );
        let map_out = add(
            "https://www.freelancermap.de/nproj/12346.html",
            "Buchhaltung",
        );
        add(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "Interim CFO",
        );
        let li_read = add(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "Controller",
        );
        let now = Timestamp::now();
        store.mark_read(&li_read, now).unwrap();
        store
            .save_matches(&[(map_out, record(MatchStatus::Excluded, 90))], "r", now)
            .unwrap();
        store.set_pinned(&map_new, true, now).unwrap();
        store.set_pinned(&li_read, true, now).unwrap();
        let counts = |search: Option<&str>| {
            job_page(
                &store,
                &JobQuery {
                    facet: JobFacet::New,
                    sort: JobSort::Match,
                    search: search.map(str::to_owned),
                    limit: 0,
                    offset: 0,
                },
            )
            .unwrap()
            .counts
        };
        let per_portal = |counts: &JobCounts| -> Vec<(Portal, u32)> {
            counts
                .new_by_portal
                .iter()
                .map(|p| (p.portal, p.new))
                .collect()
        };
        let all = counts(None);
        assert_eq!(
            per_portal(&all),
            [
                (Portal::LinkedIn, 1),
                (Portal::FreelanceDe, 0),
                (Portal::Freelancermap, 1)
            ]
        );
        assert_eq!(
            all.new_by_portal.iter().map(|p| p.new).sum::<u32>(),
            all.new
        );
        assert_eq!(all.saved, 2);
        let found = counts(Some("interim"));
        assert_eq!(
            per_portal(&found),
            [
                (Portal::LinkedIn, 1),
                (Portal::FreelanceDe, 0),
                (Portal::Freelancermap, 0)
            ]
        );
        assert_eq!(found.saved, 0);
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
        // Whether to act comes with the health: alert mails without jobs ask her to look,
        // a pause and a cap resolve themselves.
        assert_eq!(
            (li.action_needed, fm.action_needed, fl.action_needed),
            (false, false, true)
        );
        assert!(PortalHealth::LoginRequired.action_needed());
        assert!(
            !PortalHealth::LayoutSuspect {
                empty_mails: 0,
                pages: 5
            }
            .action_needed()
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
        // Sign-in switched off: the portal goes as a guest, nothing is wrong.
        assert_eq!(state(&policy).health, PortalHealth::Ok);
        let mut settings = settings.clone();
        settings
            .portals
            .get_mut(&Portal::FreelanceDe)
            .unwrap()
            .login_enabled = true;
        let health = portal_states(&policy, &settings, &[], now)
            .into_iter()
            .find(|s| s.portal == Portal::FreelanceDe)
            .unwrap()
            .health;
        assert_eq!(health, PortalHealth::LoginRequired);
    }

    const AD: &str = "Wir suchen einen Interim CFO (m/w/d).

Anforderungen:
- Erfahrung im Controlling
- Konzernrechnungslegung nach IFRS
- Kenntnisse in Zollabwicklung

Rahmenbedingungen:
- Tagessatz bis 700 €
- Einsatzort Hamburg";

    fn job_with_text(text: &str) -> (Store, JobKey) {
        let (store, key) = store_with(
            "https://www.linkedin.com/jobs/view/4000000009/",
            "Interim CFO (m/w/d)",
            "Nordlicht AG",
            "Hamburg",
        );
        store
            .record_text(&key, text, false, false, Timestamp::now())
            .unwrap();
        (store, key)
    }

    /// The reader recomputes the reasons; a stale stored score is replaced only when no run
    /// is active (`save`), and a current one keeps its time.
    #[test]
    fn the_reader_recomputes_and_heals_a_stale_score() {
        let matcher = crate::pipeline::demo::matcher();
        let (store, key) = job_with_text(AD);
        let t1: Timestamp = "2026-09-20T08:00:00Z".parse().unwrap();
        let busy = job_detail(&store, &key, Some(&matcher), false, t1)
            .unwrap()
            .unwrap();
        let fresh = busy.match_.as_ref().unwrap();
        assert_eq!(fresh.status, MatchStatus::Excluded);
        assert_eq!((fresh.rev.as_str(), fresh.at), (matcher.rev(), t1));
        assert_eq!(
            busy.job.match_.as_ref().unwrap().status,
            MatchStatus::Excluded,
            "the header shows the fresh result"
        );
        assert_eq!(
            store.match_rev(&key).unwrap(),
            None,
            "a run is active: not saved"
        );
        let idle = job_detail(&store, &key, Some(&matcher), true, t1)
            .unwrap()
            .unwrap();
        assert_eq!(idle.match_, busy.match_);
        assert_eq!(
            store.match_rev(&key).unwrap().as_deref(),
            Some(matcher.rev())
        );
        let stored = store.job(&key).unwrap().unwrap().match_.unwrap();
        assert_eq!(stored.note.unwrap().code, "dayRate");
        let t2: Timestamp = "2026-09-21T08:00:00Z".parse().unwrap();
        let later = job_detail(&store, &key, Some(&matcher), true, t2)
            .unwrap()
            .unwrap();
        assert_eq!(later.match_.unwrap().at, t1, "current score: stored time");
        // Without a usable profile there is no match to explain.
        let empty = LocalMatcher::from_json(&serde_json::json!({}));
        let none = job_detail(&store, &key, Some(&empty), true, t2)
            .unwrap()
            .unwrap();
        assert_eq!(none.match_, None);
    }

    /// "Fits anyway": the excluded job shows as scored with the user's word first (the
    /// engine's findings stay listed); taken back, the engine's verdict is stored again.
    #[test]
    fn an_override_shows_the_users_word_and_can_be_taken_back() {
        let matcher = crate::pipeline::demo::matcher();
        let (store, key) = job_with_text(AD);
        let now = Timestamp::now();
        job_detail(&store, &key, Some(&matcher), true, now).unwrap();
        assert!(store.set_override(&key, true).unwrap());
        let detail = job_detail(&store, &key, Some(&matcher), true, now)
            .unwrap()
            .unwrap();
        assert!(detail.job.overridden);
        let m = detail.match_.unwrap();
        assert_eq!(m.status, MatchStatus::Scored);
        assert_eq!(m.summary.unwrap().code, "userOverride");
        assert_eq!(m.reasons[0].code, "userOverride");
        assert!(m.reasons.iter().any(|r| r.kind == ReasonKind::Violation));
        let row = JobView::from(&store.job(&key).unwrap().unwrap());
        let shown = row.match_.unwrap();
        assert_eq!(shown.status, MatchStatus::Scored);
        assert_eq!(shown.note.unwrap().code, "userOverride");
        assert!(store.set_override(&key, false).unwrap());
        let back = job_detail(&store, &key, Some(&matcher), true, now)
            .unwrap()
            .unwrap();
        assert!(!back.job.overridden);
        assert_eq!(back.match_.unwrap().status, MatchStatus::Excluded);
        let stored = store.job(&key).unwrap().unwrap().match_.unwrap();
        assert_eq!(stored.status, MatchStatus::Excluded);
    }

    /// Reasons, highlights and the criteria strip reference each other consistently.
    #[test]
    fn the_explanation_is_bounded_and_linked() {
        let matcher = crate::pipeline::demo::matcher();
        let many = (0..60).fold(String::new(), |mut all, i| {
            all.push_str("- Kenntnisse in Spezialthema Nummer");
            all.push_str(&i.to_string());
            all.push('\n');
            all
        });
        let text = format!("{AD}\n\nAnforderungen:\n{many}");
        let (store, key) = job_with_text(&text);
        let detail = job_detail(&store, &key, Some(&matcher), true, Timestamp::now())
            .unwrap()
            .unwrap();
        let m = detail.match_.unwrap();
        assert_eq!(m.reasons.len(), MAX_REASONS);
        assert!(m.highlights.len() <= MAX_HIGHLIGHTS);
        assert!(m.reasons.iter().any(|r| r.kind == ReasonKind::Violation));
        let ids: BTreeSet<&str> = m.reasons.iter().map(|r| r.id.as_str()).collect();
        assert!(m.highlights.iter().all(|h| ids.contains(h.reason.as_str())));
        let text16: Vec<u16> = text.encode_utf16().collect();
        for h in &m.highlights {
            assert!(h.start < h.end && h.end as usize <= text16.len(), "{h:?}");
        }
        let quoted = m
            .reasons
            .iter()
            .find(|r| r.label == "Erfahrung im Controlling");
        let quoted = quoted.expect("a met requirement");
        assert_eq!(quoted.kind, ReasonKind::Met);
        assert_eq!(quoted.evidence.as_ref().unwrap().via, "exact");
        let range = quoted.ranges[0];
        let passage = String::from_utf16(&text16[range.start as usize..range.end as usize]);
        assert!(passage.unwrap().contains("Controlling"));
        // The strip: every criterion the sample profile sets, the day rate violated and
        // linked to its reason.
        let codes: Vec<&str> = m.criteria.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(codes, ["minDayRate", "countries", "noAnue", "availability"]);
        let rate = &m.criteria[0];
        assert_eq!(
            (rate.kind, rate.weight),
            (ReasonKind::Violation, ReasonWeight::Hard)
        );
        let linked = rate.params["reason"].as_str().unwrap();
        assert!(ids.contains(linked) && !rate.ranges.is_empty(), "{rate:?}");
        assert_eq!(m.criteria[1].kind, ReasonKind::Met);
        assert_eq!(m.criteria[1].params["countries"], "DE, AT, CH");
        // Met with the ad's value as evidence; the start is not mentioned (open).
        assert_eq!(m.criteria[1].params["location"], "Hamburg");
        assert_eq!(m.criteria[2].params["contract"], "interim");
        assert_eq!(m.criteria[3].kind, ReasonKind::Open);
        let summary = m.summary.unwrap();
        assert_eq!(summary.params["evidence"], "full");
        assert!(serde_json::to_vec(&m.reasons).unwrap().len() < 64 * 1024);
    }

    /// "What the app understood": quality, competences, sources, criteria and warnings as
    /// codes; the pending count follows the stored revisions.
    #[test]
    fn the_profile_summary_reaches_the_interface() {
        let matcher = crate::pipeline::demo::matcher();
        let (store, key) = job_with_text(AD);
        let file = crate::profile::ProfileInfo {
            path: PathBuf::from("beraterprofil.json"),
            bytes: 10,
            saved_at: None,
            parse_error: None,
        };
        let info = ProfileInfo::of(&file, None)
            .understood_by(&matcher, &store)
            .unwrap();
        assert_eq!(info.quality, Some(ProfileQuality::Good));
        assert_eq!((info.pending, info.scored_at), (1, None));
        let understood = info.understood.unwrap();
        assert!(understood.competences.len() <= 40);
        assert!(understood.competence_count as usize >= understood.competences.len());
        assert!(understood.competences.iter().any(|c| c == "Controlling"));
        assert!(
            understood
                .sources
                .iter()
                .any(|s| s == "kernkompetenzen[].kompetenz")
        );
        let criteria: Vec<(&str, bool)> = understood
            .criteria
            .iter()
            .map(|c| (c.code.as_str(), c.params["set"] == true))
            .collect();
        assert_eq!(
            criteria,
            [
                ("minDayRate", true),
                ("countries", true),
                ("noAnue", true),
                ("availability", true),
                ("minSalary", false),
                ("permanentRegion", false),
                ("targetYears", false),
            ]
        );
        assert!(understood.warnings.is_empty());
        let now = Timestamp::now();
        job_detail(&store, &key, Some(&matcher), true, now).unwrap();
        let info = ProfileInfo::of(&file, None)
            .understood_by(&matcher, &store)
            .unwrap();
        assert_eq!(info.pending, 0);
        assert_eq!(
            info.scored_at.map(Timestamp::as_second),
            Some(now.as_second())
        );
        let thin = LocalMatcher::from_json(&serde_json::json!({"keywords": ["SAP FI"]}));
        let info = ProfileInfo::of(&file, None)
            .understood_by(&thin, &store)
            .unwrap();
        assert_eq!(info.quality, Some(ProfileQuality::Thin));
        let warnings = info.understood.unwrap().warnings;
        let codes: Vec<&str> = warnings.iter().map(|w| w.code.as_str()).collect();
        assert_eq!(codes, ["fewCompetences", "noCriteria"]);
        let empty = LocalMatcher::from_json(&serde_json::json!({}));
        let info = ProfileInfo::of(&file, None)
            .understood_by(&empty, &store)
            .unwrap();
        assert_eq!(
            (info.quality, info.pending),
            (Some(ProfileQuality::Empty), 0)
        );
    }
}
