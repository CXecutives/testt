//! One click: mailbox -> new jobs -> job details -> export.
//!
//! The run kinds pick the steps (`RunKind`); the portals come from the settings, never from
//! the page. The export always runs at the end - after a cancellation, a portal stop or an
//! error too (it is purely local). Every run starts with exactly one `Started` (its kind: the
//! page also follows runs it did not start) and ends with exactly one `Finished`; the summary
//! of a mailbox run (fetch, whole mailbox) is also stored as `last_run_summary` - "the last
//! fetch" for the page, which a rescore or a details run never replaces. Events carry codes
//! and data, never prose; the log gets English lines with the run id (never content,
//! addresses or passwords).

pub mod demo;
pub mod local;
pub mod rescore;
pub mod score;

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{ErrorInfo, InvalidInput};
use crate::export::{self, RESULT_DIR, TXT_DIR, texts, write_job_txt, write_xlsx};
use crate::fetch::policy::Policy;
use crate::fetch::{
    FetchEvent, FetchSummary, PageFetcher, PortalHealth, Prescore, Selection, fetch_all,
    neutral_prescore,
};
use crate::mail::imap::{MailError, MailSource};
use crate::mail::scan::{ScanError, ScanEvent, ScanSummary, Scope, scan};
use crate::portal::{FetchPath, JobKey, Portal};
use crate::store::{JobFilter, JobRow, Store};
use crate::text::truncate_chars;
use crate::time;
use crate::view::{EmptyAlert, JobView, MAX_SUBJECT_CHARS};
pub use local::LocalMatcher;
pub use score::Matcher;
use score::Tally;

/// What the interface starts. The JSON is flat: `{ "kind": "details", "keys": [...] }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RunRequest {
    #[serde(flatten)]
    #[cfg_attr(test, ts(flatten))]
    pub kind: RunKind,
}

/// The kinds of run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum RunKind {
    /// New alert mails of the enabled portals, their job details, export.
    Fetch,
    /// Job details of exactly these jobs (older ones too), export.
    Details { keys: Vec<JobKey> },
    /// Score again with the current profile, export.
    Rescore,
    /// Like `fetch`, but the whole inbox.
    FullMailbox,
}

/// The kind of a run without its data (summary, snapshot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum RunKindName {
    Fetch,
    Details,
    Rescore,
    FullMailbox,
}

impl RunKindName {
    /// A mailbox run (fetch or the whole mailbox): the kind "the last fetch" means.
    pub fn reads_mail(self) -> bool {
        matches!(self, RunKindName::Fetch | RunKindName::FullMailbox)
    }
}

impl RunKind {
    pub fn name(&self) -> RunKindName {
        match self {
            RunKind::Fetch => RunKindName::Fetch,
            RunKind::Details { .. } => RunKindName::Details,
            RunKind::Rescore => RunKindName::Rescore,
            RunKind::FullMailbox => RunKindName::FullMailbox,
        }
    }
}

/// Run settings that do not come from the page.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub workspace: PathBuf,
    /// Dry run: nothing is written.
    pub dry_run: bool,
    /// Portals whose alert mails are read (settings: enabled).
    pub portals: Vec<Portal>,
    /// Portals whose job pages may be fetched (settings: enabled and details on).
    pub fetch_portals: Vec<Portal>,
    /// Of those, the portals read in the session window (sign-in switched on); the others
    /// go as a guest. A run never opens a session window for any other portal.
    pub sign_in: Vec<Portal>,
}

impl RunContext {
    /// The fetch path of a portal in this run.
    pub fn path(&self, portal: Portal) -> FetchPath {
        if self.sign_in.contains(&portal) {
            FetchPath::Session
        } else {
            FetchPath::Guest
        }
    }
}

/// Mailbox and fetch routes of a run (dummies in tests and in the dry run).
pub trait Backends {
    type Mail: MailSource + Send;
    type Pages: PageFetcher + Send;
    fn connect_mail(
        &mut self,
        cancel: &CancellationToken,
    ) -> impl Future<Output = Result<Self::Mail, MailError>> + Send;
    /// Fetch path of **one** portal - only the one `path` names: own HTTP session or own
    /// window. The portals run side by side and therefore share none.
    fn pages(&mut self, portal: Portal, path: FetchPath) -> Result<Self::Pages, String>;
    /// The matcher of the run; `None` = nothing is scored (no usable profile or engine).
    fn matcher(&self) -> Option<Arc<dyn Matcher>> {
        None
    }
    /// The pre-score that orders the fetch queue of a portal (`matching::prescore` with the
    /// profile); neutral by default - then the newest mail comes first.
    fn prescore(&self) -> Prescore {
        neutral_prescore()
    }
    /// The fetch path of every portal as the settings say right now, asked before every
    /// request: a portal switched off (or to another path) during the run gets no further
    /// request. `None` by default - the paths stay as the run started.
    fn live_paths(&self) -> Option<LivePaths> {
        None
    }
}

/// The current fetch path of a portal (`None` = switched off), see [`Backends::live_paths`].
pub type LivePaths = Arc<dyn Fn(Portal) -> Option<FetchPath> + Send + Sync>;

/// The fetch paths as the stored settings say right now (the app's
/// [`Backends::live_paths`]). Unreadable settings count as switched off: no request without
/// a readable switch.
pub fn stored_paths(store: Arc<Store>) -> LivePaths {
    Arc::new(move |portal| {
        crate::settings::Settings::load(&store)
            .ok()
            .and_then(|settings| settings.fetch_path(portal))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Step {
    Scan,
    Fetch,
    Score,
    Export,
}

/// What is happening right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum StatusCode {
    ConnectingMail,
    SearchingMail,
    ReadingMails,
    FetchingDetails,
    SigningIn,
    /// Gap before the next request of a portal (`until` for a countdown).
    Waiting,
    Scoring,
    WritingFiles,
}

/// Events to the interface. Each stays small (< 8 KB; bigger messages bypass the ACL of the
/// Tauri channel): full texts and job rows are fetched by the page itself (`list_jobs`,
/// `job_detail`). Struct variants only - with `tag = "type"` a newtype variant would merge
/// into the tag.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum RunEvent {
    /// The first event of every run: its kind (the auto fetch and the rescores the app starts
    /// by itself included).
    Started { kind: RunKindName },
    Progress {
        step: Step,
        portal: Option<Portal>,
        done: usize,
        total: usize,
    },
    /// What is happening; `until`: end of a wait (the interface shows a countdown).
    Status {
        code: StatusCode,
        portal: Option<Portal>,
        until: Option<Timestamp>,
    },
    Alert {
        portal: Portal,
        subject: String,
        date: Option<Timestamp>,
        postings: usize,
        /// Gmail message id (hexadecimal) - opened through `open_target`.
        gmail_id: Option<String>,
    },
    /// A job has a new state - the finished list row. `fresh`: first seen in this run (a
    /// new job, not one the page may already list further down).
    JobUpdated { job: Box<JobView>, fresh: bool },
    /// A portal stopped for the rest of the run, or its health changed.
    PortalHealth {
        portal: Portal,
        health: PortalHealth,
    },
    /// Sign-in needed: the session window is open (`waiting`) or closed again.
    LoginNeeded { portal: Portal, waiting: bool },
    /// The end. A named field, no newtype (see above).
    Finished { summary: Box<RunSummary> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Outcome {
    Completed,
    Cancelled,
    Failed { error: ErrorInfo },
}

/// Counters of the mailbox step. Invariant: `postings = new + known + dup`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ScanCounts {
    pub mails_found: usize,
    pub mails_checked: usize,
    /// Unreadable mails (counted instead of dropped silently).
    pub mails_defective: usize,
    pub alert_mails: usize,
    /// Alert mails without a recognised job (layout changed?).
    pub empty_alerts: usize,
    pub postings: usize,
    pub new: usize,
    pub known: usize,
    pub dup: usize,
}

impl From<&ScanSummary> for ScanCounts {
    fn from(s: &ScanSummary) -> ScanCounts {
        ScanCounts {
            mails_found: s.mails_found,
            mails_checked: s.mails_checked,
            mails_defective: s.mails_defective,
            alert_mails: s.alert_mails,
            empty_alerts: s.zero_posting_mails,
            postings: s.postings_total,
            new: s.new,
            known: s.known_before,
            dup: s.dup_in_run,
        }
    }
}

/// One portal in the run summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PortalSummary {
    pub portal: Portal,
    pub new: usize,
    pub known: usize,
    pub dup: usize,
    /// Job details fetched (full text found).
    pub fetched: usize,
    /// Pages without a description.
    pub failed: usize,
    /// Ads that no longer exist.
    pub gone: usize,
    /// Jobs left for later (pause, cap, sign-in, breaker, network).
    pub skipped: usize,
    /// Why the portal stopped in this run.
    pub stopped: Option<PortalHealth>,
}

/// Counters of the score step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ScoreSummary {
    pub scored: usize,
    pub excluded: usize,
    pub unscorable: usize,
    /// Jobs still waiting for a score.
    pub pending: usize,
    pub best: Option<u8>,
}

/// The jobs a mailbox run brought - the "neue Jobs" of the run card: first seen in the run
/// (a job several portals announce counts once, as its original), excluded ones left out;
/// `high`: how many of them are scored in the high band.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct NewJobs {
    pub count: usize,
    pub high: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ExportSummary {
    /// Written Excel overview (if written in this run).
    pub overview_xlsx: Option<PathBuf>,
    /// Written HTML overview (if written in this run).
    pub overview_html: Option<PathBuf>,
    /// A foreign overview at the same path was backed up here.
    pub backup: Option<PathBuf>,
    pub txt_written: usize,
    /// Number of text files that could not be written - the number for every display.
    pub txt_failed: usize,
    /// Examples for the log (the first at most 20 keys), never for counting.
    #[serde(skip)]
    pub txt_failed_keys: Vec<String>,
    /// The first error (closest to the cause); `params.target` names what failed.
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RunSummary {
    pub run: i64,
    pub kind: RunKindName,
    pub outcome: Outcome,
    pub dry_run: bool,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub scan: Option<ScanCounts>,
    pub per_portal: Vec<PortalSummary>,
    /// Set by a mailbox run (summaries of earlier versions have none).
    #[serde(default)]
    pub new_jobs: Option<NewJobs>,
    pub score: Option<ScoreSummary>,
    pub export: Option<ExportSummary>,
    /// Alert mails of this run without recognised jobs (at most [`MAX_EMPTY_ALERTS`]).
    pub empty_alerts: Vec<EmptyAlert>,
    /// Counters of the fetch step (tests and log only).
    #[serde(skip)]
    pub fetch: Option<FetchSummary>,
}

impl RunSummary {
    pub fn new(kind: RunKindName, dry_run: bool, started_at: Timestamp) -> RunSummary {
        RunSummary {
            run: 0,
            kind,
            outcome: Outcome::Completed,
            dry_run,
            started_at,
            finished_at: started_at,
            scan: None,
            per_portal: Vec::new(),
            new_jobs: None,
            score: None,
            export: None,
            empty_alerts: Vec::new(),
            fetch: None,
        }
    }

    /// The summary as the `Finished` event, below [`MAX_EVENT_BYTES`] whatever subjects,
    /// paths and error params it holds: empty alert mails go from the end first, then error
    /// params, then the file paths (the stored summary keeps everything).
    pub fn finished_event(&self) -> RunEvent {
        // `{"type":"finished","summary":...}` around the summary.
        const ENVELOPE: usize = 64;
        let mut summary = self.clone();
        let fits = |s: &RunSummary| {
            serde_json::to_vec(s).is_ok_and(|json| json.len() + ENVELOPE <= MAX_EVENT_BYTES)
        };
        while !fits(&summary) {
            if summary.empty_alerts.pop().is_some() {
                continue;
            }
            if let Outcome::Failed { error } = &mut summary.outcome
                && !error.params.is_empty()
            {
                error.params.clear();
                continue;
            }
            let Some(export) = summary.export.as_mut() else {
                break;
            };
            if let Some(error) = export.error.as_mut()
                && !error.params.is_empty()
            {
                error.params.clear();
            } else if export.backup.take().is_none()
                && export.overview_html.take().is_none()
                && export.overview_xlsx.take().is_none()
            {
                break;
            }
        }
        RunEvent::Finished {
            summary: Box::new(summary),
        }
    }
}

/// A run in progress, for a page that attaches again (reload): what it is and the events
/// that describe its current state (last status and progress, portal health, alerts).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RunSnapshot {
    pub kind: RunKindName,
    pub started_at: Timestamp,
    pub replay: Vec<RunEvent>,
}

/// Summary of the last mailbox run, "the last fetch" (JSON).
pub const LAST_RUN: &str = "last_run_summary";
/// Number of the last run with a mailbox scan.
const LAST_SCAN_RUN: &str = "last_scan_run";
/// State of the overview per path (key = prefix + path).
const EXPORT_STAMP: &str = "export:";
/// Info sheet rows of the last successful mailbox scan.
const LAST_SCAN_INFO: &str = "last_scan_info";
/// So many keys of unwritten text files the log names.
const MAX_FAILED_NAMES: usize = 20;
/// So many empty alert mails a summary carries (the event must stay small).
pub const MAX_EMPTY_ALERTS: usize = 10;
/// Largest `Finished` event in bytes of JSON: Tauri channel messages above 8 KB bypass the
/// ACL (a margin for the channel's own framing).
pub const MAX_EVENT_BYTES: usize = 7 * 1024;
/// Start of the last successful mailbox scan (Unix seconds).
const LAST_FETCH_AT: &str = "last_fetch_at";
/// Label of the mail address row that earlier versions stored - do not translate.
const LEGACY_ACCOUNT_LABEL: &str = "Gmail-Konto";
/// Info sheet labels and values earlier versions stored with the last mailbox scan, and
/// today's words for them (until the next scan stores its own) - do not translate.
const LEGACY_INFO: [(&str, &str); 5] = [
    ("Umfang des letzten Laufs", texts::INFO_SCOPE),
    ("Neu (letzter Lauf)", texts::INFO_NEW),
    ("Schon bekannt (letzter Lauf)", texts::INFO_KNOWN),
    ("Doppelt in mehreren Mails (letzter Lauf)", texts::INFO_DUP),
    ("Neu seit letztem Lauf", texts::SCOPE_NEW),
];
/// The app fetches by itself at the start when the last fetch is older than this.
pub const AUTO_FETCH_AFTER: jiff::SignedDuration = jiff::SignedDuration::from_hours(6);

/// What a run kind does.
struct Plan<'a> {
    scan: Option<Scope>,
    fetch: Option<Selection<'a>>,
}

impl<'a> Plan<'a> {
    fn of(kind: &'a RunKind, ctx: &'a RunContext) -> Plan<'a> {
        let queue = Some(Selection::Queue(&ctx.fetch_portals));
        match kind {
            RunKind::Fetch => Plan {
                scan: Some(Scope::New),
                fetch: queue,
            },
            RunKind::FullMailbox => Plan {
                scan: Some(Scope::All),
                fetch: queue,
            },
            RunKind::Details { keys } => Plan {
                scan: None,
                fetch: Some(Selection::Jobs(keys, &ctx.fetch_portals)),
            },
            RunKind::Rescore => Plan {
                scan: None,
                fetch: None,
            },
        }
    }
}

/// Runs a run. Database errors end it as `Failed`; the export still runs - unless the run
/// could not even be created.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "store, rules, request, cancellation, clock and events separately (replaceable in \
              tests); the steps of a run read best in one place"
)]
pub async fn run<B: Backends>(
    backends: &mut B,
    store: &Store,
    policy: &Mutex<Policy>,
    request: &RunRequest,
    ctx: &RunContext,
    cancel: &CancellationToken,
    clock: impl Fn() -> Timestamp,
    mut emit: impl FnMut(RunEvent),
) -> RunSummary {
    let started_at = clock();
    let mut summary = RunSummary::new(request.kind.name(), ctx.dry_run, started_at);
    emit(RunEvent::Started { kind: summary.kind });
    // Without a run number (database locked or broken) nothing can be assigned: then
    // neither mailbox nor fetch nor export.
    let run = match store.begin_run() {
        Ok(run) => run,
        Err(e) => {
            log::error!("run could not begin: {e}");
            summary.outcome = failed(ErrorInfo::from(&e));
            summary.finished_at = clock();
            emit(summary.finished_event());
            return summary;
        }
    };
    summary.run = run;
    log::info!("run {run}: {:?} started", summary.kind);
    let plan = Plan::of(&request.kind, ctx);
    let mut postings: BTreeMap<Portal, usize> = BTreeMap::new();

    if let Some(scope) = plan.scan {
        let before_scan = last_scan_run(store).unwrap_or(0);
        if ctx.portals.is_empty() {
            summary.outcome = failed(ErrorInfo::from(&InvalidInput::NoPortal));
        } else if let Err(e) = store.kv_set(LAST_SCAN_RUN, &run.to_string()) {
            // "New in this run" shows the jobs of the last run with a mailbox scan.
            summary.outcome = failed(ErrorInfo::from(&e));
        } else {
            // Remember the state of the last real scan: whoever never reaches the mailbox
            // (wrong app password, no network, instant cancel) must not empty it.
            let mut scanned = ScanSummary::default();
            let result = scan_step(
                backends,
                store,
                (run, scope, started_at),
                &ctx.portals,
                cancel,
                &mut scanned,
                &mut postings,
                &mut emit,
            )
            .await;
            summary.outcome = match result {
                Ok(()) => {
                    remember_scan(store, scope, &scanned, started_at);
                    Outcome::Completed
                }
                Err(ScanError::Mail(MailError::Cancelled)) => Outcome::Cancelled,
                Err(ScanError::Mail(e)) => failed(ErrorInfo::from(&e)),
                Err(ScanError::Store(e)) => failed(ErrorInfo::from(&e)),
            };
            if scanned.mails_checked == 0 {
                let _ = store.kv_set(LAST_SCAN_RUN, &before_scan.to_string());
            }
            summary.scan = Some(ScanCounts::from(&scanned));
            summary.empty_alerts = store
                .zero_posting_mails(run)
                .unwrap_or_default()
                .iter()
                .take(MAX_EMPTY_ALERTS)
                .map(EmptyAlert::from)
                .collect();
        }
    }

    let matcher = backends.matcher();
    let mut tally = Tally::default();
    if let Some(selection) = plan.fetch
        && summary.outcome == Outcome::Completed
    {
        let mut fetched = FetchSummary::default();
        summary.outcome = fetch_step(
            backends,
            ctx,
            (store, policy, matcher.as_deref()),
            run,
            selection,
            cancel,
            &clock,
            (&mut fetched, &mut tally),
            &mut emit,
        )
        .await;
        summary.fetch = Some(fetched);
    }
    summary.per_portal = per_portal(store, run, &postings, summary.fetch.as_ref());
    if let Some(matcher) = &matcher {
        score_step(
            store,
            &**matcher,
            cancel,
            &clock,
            &mut tally,
            &mut summary,
            &mut emit,
        );
    }

    if summary.scan.is_some() {
        match store.new_jobs(run) {
            Ok((count, high)) => summary.new_jobs = Some(NewJobs { count, high }),
            Err(e) => log::warn!("run {run}: new jobs not counted: {e}"),
        }
    }

    summary.finished_at = clock();
    if !ctx.dry_run {
        emit(status(StatusCode::WritingFiles, None, None));
        let info = info_rows(store, started_at);
        let exported = export_all(store, &ctx.workspace, &info, run, summary.finished_at);
        write_top_matches(
            store,
            &ctx.workspace,
            matcher.as_deref(),
            summary.finished_at,
        );
        log_export(run, &exported);
        summary.export = Some(exported);
    }
    // "The last fetch" of the page: a rescore or a details run never replaces it.
    if !ctx.dry_run
        && summary.kind.reads_mail()
        && let Ok(json) = serde_json::to_string(&summary)
        && let Err(e) = store.kv_set(LAST_RUN, &json)
    {
        log::warn!("run {run}: summary not stored: {e}");
    }
    log::info!("run {run}: finished {:?}", summary.outcome);
    emit(summary.finished_event());
    summary
}

/// Catch-up scoring after a completed run and the score summary.
fn score_step(
    store: &Store,
    matcher: &dyn Matcher,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    tally: &mut Tally,
    summary: &mut RunSummary,
    emit: &mut impl FnMut(RunEvent),
) {
    let run = summary.run;
    if summary.outcome == Outcome::Completed {
        match score::catch_up(store, matcher, cancel, clock, tally, emit) {
            Ok(true) => {}
            Ok(false) => summary.outcome = Outcome::Cancelled,
            Err(e) => {
                log::warn!("run {run}: scoring failed: {e}");
                summary.outcome = failed(ErrorInfo::from(&e));
            }
        }
    }
    let pending = store.match_pending(matcher.rev()).unwrap_or(0);
    summary.score = Some(tally.summary(usize::try_from(pending).unwrap_or(0)));
    log::info!("run {run}: score {:?}", summary.score);
}

fn failed(error: ErrorInfo) -> Outcome {
    Outcome::Failed { error }
}

/// Number of the last run with a mailbox scan ("new in this run"); 0 = none yet.
pub fn last_scan_run(store: &Store) -> crate::Result<i64> {
    Ok(store
        .kv_get(LAST_SCAN_RUN)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

/// The summary of the last mailbox run (fetch or whole mailbox), if one is stored (and
/// readable).
pub fn last_run(store: &Store) -> crate::Result<Option<RunSummary>> {
    Ok(store
        .kv_get(LAST_RUN)?
        .and_then(|json| serde_json::from_str(&json).ok()))
}

fn status(code: StatusCode, portal: Option<Portal>, until: Option<Timestamp>) -> RunEvent {
    RunEvent::Status {
        code,
        portal,
        until,
    }
}

/// Reports an activity unless the status line already shows it.
fn announce(
    activity: &mut Option<(StatusCode, Portal)>,
    code: StatusCode,
    portal: Portal,
    emit: &mut impl FnMut(RunEvent),
) {
    if *activity != Some((code, portal)) {
        emit(status(code, Some(portal), None));
        *activity = Some((code, portal));
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "store, request, cancellation, counters and events separately (replaceable in tests)"
)]
async fn scan_step<B: Backends>(
    backends: &mut B,
    store: &Store,
    (run, scope, started_at): (i64, Scope, Timestamp),
    portals: &[Portal],
    cancel: &CancellationToken,
    scanned: &mut ScanSummary,
    postings: &mut BTreeMap<Portal, usize>,
    emit: &mut impl FnMut(RunEvent),
) -> Result<(), ScanError> {
    emit(status(StatusCode::ConnectingMail, None, None));
    let mut mail = backends.connect_mail(cancel).await?;
    log::info!("run {run}: mailbox scan {scope:?} for {portals:?}");
    emit(status(StatusCode::SearchingMail, None, None));
    let result = scan(
        &mut mail,
        store,
        run,
        scope,
        portals,
        started_at,
        cancel,
        scanned,
        |event| match event {
            ScanEvent::Found { total } => {
                if total > 0 {
                    emit(status(StatusCode::ReadingMails, None, None));
                }
            }
            ScanEvent::Alert(alert) => {
                for posting in &alert.postings {
                    *postings.entry(posting.key.portal).or_default() += 1;
                }
                emit(RunEvent::Alert {
                    portal: alert.portal,
                    subject: truncate_chars(&alert.subject, MAX_SUBJECT_CHARS),
                    date: alert.date,
                    postings: alert.postings.len(),
                    gmail_id: alert.gmail_id.map(|id| format!("{id:x}")),
                });
            }
            ScanEvent::Progress { done, total } => emit(RunEvent::Progress {
                step: Step::Scan,
                portal: None,
                done,
                total,
            }),
        },
    )
    .await;
    mail.logout().await;
    let s = &*scanned;
    // Per portal: as soon as all of a portal's alert mails came without a job, its mail
    // layout probably changed - even while the other portals are fine.
    for (portal, mails) in s.empty_portals() {
        log::warn!(
            "run {run}: {}: {mails} alert mails but no jobs recognised - mail layout changed?",
            portal.key()
        );
        emit(RunEvent::PortalHealth {
            portal,
            health: PortalHealth::LayoutSuspect {
                empty_mails: mails,
                pages: 0,
            },
        });
    }
    match &result {
        Ok(()) => log::info!(
            "run {run}: mailbox checked: {} mails, {} alert mails, {} new, {} known, {} duplicates",
            s.mails_checked,
            s.alert_mails,
            s.new,
            s.known_before,
            s.dup_in_run
        ),
        Err(ScanError::Mail(MailError::Cancelled)) => log::info!("run {run}: mailbox cancelled"),
        Err(e) => log::warn!("run {run}: mailbox failed: {e}"),
    }
    if s.mails_defective > 0 {
        log::warn!("run {run}: {} unreadable mails skipped", s.mails_defective);
    }
    result
}

#[expect(
    clippy::too_many_arguments,
    reason = "store, rules, selection, cancellation, clock and events separately (replaceable in tests)"
)]
async fn fetch_step<B: Backends>(
    backends: &mut B,
    ctx: &RunContext,
    (store, policy, matcher): (&Store, &Mutex<Policy>, Option<&dyn Matcher>),
    run: i64,
    selection: Selection<'_>,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    (fetched, tally): (&mut FetchSummary, &mut Tally),
    emit: &mut impl FnMut(RunEvent),
) -> Outcome {
    // Activity in the status line - not anew for every job, again after a wait.
    let mut activity: Option<(StatusCode, Portal)> = None;
    let prescore = backends.prescore();
    // A portal stays on while the settings still name the path the run started with.
    let live = backends.live_paths();
    let on = |portal: Portal| {
        live.as_ref()
            .is_none_or(|now| now(portal) == Some(ctx.path(portal)))
    };
    let result = fetch_all(
        |portal| backends.pages(portal, ctx.path(portal)),
        store,
        policy,
        (selection, &*prescore, &on),
        cancel,
        clock,
        fetched,
        |event| match event {
            FetchEvent::Requeued { portal, count } => log::info!(
                "run {run}: {}: {count} failed jobs open again after a parser update",
                portal.key()
            ),
            FetchEvent::Queued { total } => log::info!("run {run}: job details: {total} open"),
            FetchEvent::Fetching { portal } => {
                announce(&mut activity, StatusCode::FetchingDetails, portal, emit);
            }
            FetchEvent::SigningIn { portal } => {
                announce(&mut activity, StatusCode::SigningIn, portal, emit);
            }
            FetchEvent::Waiting { portal, until } => {
                activity = None;
                emit(status(StatusCode::Waiting, Some(portal), Some(until)));
            }
            FetchEvent::JobUpdated { key, .. } => {
                // A duplicate of another portal's job shows as that job's row (with
                // `alsoOn`) and is scored with it; any other job is scored before its row
                // goes out - the ring appears with the details.
                let shown = if let Ok(Some(original)) = store.dup_of(&key) {
                    original
                } else {
                    if let Some(matcher) = matcher {
                        score::score_one(store, matcher, &key, clock(), tally);
                    }
                    key
                };
                if let Ok(Some(job)) = store.job(&shown)
                    && let Ok(Some(view)) =
                        crate::view::job_views(store, std::slice::from_ref(&job))
                            .map(|mut views| views.pop())
                {
                    emit(RunEvent::JobUpdated {
                        job: Box::new(view),
                        fresh: job.first_seen_run == run,
                    });
                }
            }
            FetchEvent::PortalStopped {
                portal,
                reason,
                skipped,
            } => {
                log::info!("run {run}: {}", reason.log_line(portal, skipped));
                emit(RunEvent::PortalHealth {
                    portal,
                    health: reason.health(),
                });
            }
            FetchEvent::Progress { done, total } => emit(RunEvent::Progress {
                step: Step::Fetch,
                portal: None,
                done,
                total,
            }),
        },
    )
    .await;
    match result {
        Ok(true) => {
            let ok: usize = fetched.per_portal.values().map(|c| c.ok).sum();
            let open = fetched.queued.saturating_sub(ok);
            log::info!("run {run}: job details: {ok} fetched, {open} without details");
            Outcome::Completed
        }
        Ok(false) => {
            log::info!("run {run}: job details cancelled - what was fetched is stored");
            Outcome::Cancelled
        }
        Err(e) => {
            log::warn!("run {run}: job details failed: {e}");
            failed(ErrorInfo::from(&e))
        }
    }
}

/// The portals of the summary: scan counts (new and known from the store, duplicates as the
/// rest of the postings) and fetch counts, in the order of `Portal::ALL`.
fn per_portal(
    store: &Store,
    run: i64,
    postings: &BTreeMap<Portal, usize>,
    fetch: Option<&FetchSummary>,
) -> Vec<PortalSummary> {
    let seen = if postings.is_empty() {
        Vec::new()
    } else {
        store.scan_counts(run).unwrap_or_default()
    };
    Portal::ALL
        .into_iter()
        .filter_map(|portal| {
            let total = postings.get(&portal).copied();
            let counts = fetch.and_then(|f| f.per_portal.get(&portal));
            if total.is_none() && counts.is_none() {
                return None;
            }
            let (new, known) = seen
                .iter()
                .find(|(p, ..)| *p == portal)
                .map_or((0, 0), |&(_, new, known)| (new, known));
            Some(PortalSummary {
                portal,
                new,
                known,
                dup: total.unwrap_or(0).saturating_sub(new + known),
                fetched: counts.map_or(0, |c| c.ok),
                failed: counts.map_or(0, |c| c.failed),
                gone: counts.map_or(0, |c| c.gone),
                skipped: counts.map_or(0, |c| c.skipped),
                stopped: counts
                    .and_then(|c| c.stop.as_ref())
                    .map(crate::fetch::StopReason::health),
            })
        })
        .collect()
}

/// What failed in an export step (`params.target` of the error).
#[derive(Clone, Copy)]
enum Target {
    /// The folder of the text files.
    TxtFolder,
    /// One text file or its mark in the database.
    Txt,
    /// The Excel overview (writing it or reading its export stamp).
    Overview,
    /// Backing up a foreign overview.
    Backup,
    /// The HTML overview.
    OverviewHtml,
}

impl Target {
    const fn code(self) -> &'static str {
        match self {
            Target::TxtFolder => "txtFolder",
            Target::Txt => "txt",
            Target::Overview => "overview",
            Target::Backup => "backup",
            Target::OverviewHtml => "overviewHtml",
        }
    }
}

/// Text files (exactly once per job) and overview. The overview is only regenerated if
/// something changed - data, run, folder - or it is missing; an Excel file open elsewhere
/// is then not disturbed needlessly.
pub fn export_all(
    store: &Store,
    workspace: &Path,
    info: &[(String, String)],
    run: i64,
    now: Timestamp,
) -> ExportSummary {
    let result_dir = workspace.join(RESULT_DIR);
    let mut summary = ExportSummary::default();
    match store.txt_jobs(false) {
        Ok(jobs) => write_txts(store, &result_dir, jobs, now, &mut summary),
        Err(e) => note_error(&mut summary, &e, Target::Txt),
    }
    write_overview(
        store,
        &export::overview_path(&result_dir),
        info,
        run,
        now,
        &mut summary,
    );
    // The HTML overview is small and never locked by a browser: written on every export.
    let path = export::overview_html_path(&result_dir);
    let written = last_scan_run(store)
        .and_then(|new_run| store.overview_jobs(new_run))
        .and_then(|(jobs, pinned)| export::write_overview_html(&path, &jobs, pinned, now));
    match written {
        Ok(()) => summary.overview_html = Some(path),
        Err(e) => note_error(&mut summary, &e, Target::OverviewHtml),
    }
    summary
}

/// `top_matches.json` for the matching skill; a failure only goes to the log (the file is an
/// extra for the skill, the run's own results are complete without it).
pub fn write_top_matches(
    store: &Store,
    workspace: &Path,
    matcher: Option<&dyn Matcher>,
    now: Timestamp,
) {
    let path = workspace.join(RESULT_DIR).join(export::TOP_MATCHES_NAME);
    let written = last_scan_run(store)
        .and_then(|run| export::top_matches(store, matcher, run, now))
        .and_then(|top| {
            let json = serde_json::to_vec_pretty(&top).unwrap_or_default();
            export::write_atomic(&path, &json)
        });
    if let Err(e) = written {
        log::warn!("{} not written: {e}", export::TOP_MATCHES_NAME);
    }
}

/// "Rewrite text files" (e.g. after a change of folder): all jobs with a full text, the
/// names stay. What cannot be written keeps its mark - the next run therefore does not
/// recreate a file deleted on purpose by itself.
pub fn rewrite_txt(store: &Store, workspace: &Path, now: Timestamp) -> ExportSummary {
    let mut summary = ExportSummary::default();
    match store.txt_jobs(true) {
        Ok(jobs) => write_txts(store, &workspace.join(RESULT_DIR), jobs, now, &mut summary),
        Err(e) => note_error(&mut summary, &e, Target::Txt),
    }
    summary
}

/// The first error stays: it is closest to the cause (the folder is unreachable); later
/// consequential errors only go to the log. Only one is reported anyway.
fn note_error(summary: &mut ExportSummary, error: &crate::Error, target: Target) {
    if summary.error.is_some() {
        log::warn!("export: another error ({}): {error}", target.code());
    } else {
        log::warn!("export: {}: {error}", target.code());
        summary.error = Some(ErrorInfo::from(error).with("target", target.code()));
    }
}

/// Remembers an example of an unwritten text file. Counting only happens in `txt_failed`:
/// this list is capped and only names the first keys.
fn note_failed(summary: &mut ExportSummary, key: &JobKey) {
    if summary.txt_failed_keys.len() < MAX_FAILED_NAMES {
        summary.txt_failed_keys.push(key.to_string());
    }
}

/// Writes text files and marks only what is written. The folder is created once: if it is
/// unusable (drive disconnected, no permission), there is one clear error instead of a
/// futile attempt per file.
fn write_txts(
    store: &Store,
    result_dir: &Path,
    jobs: Vec<(JobRow, String)>,
    now: Timestamp,
    summary: &mut ExportSummary,
) {
    if jobs.is_empty() {
        return;
    }
    let dir = result_dir.join(TXT_DIR);
    if let Err(e) = export::ensure_dir(&dir) {
        summary.txt_failed = jobs.len();
        for (job, _) in &jobs {
            note_failed(summary, &job.key);
        }
        note_error(summary, &e, Target::TxtFolder);
        return;
    }
    let total = jobs.len();
    for (done, (job, text)) in jobs.into_iter().enumerate() {
        match write_job_txt(result_dir, &job, &text) {
            Ok(name) => match store.mark_txt_written(&job.key, &name, now) {
                Ok(()) => summary.txt_written += 1,
                Err(e) => {
                    // Without the database nothing can be marked any more: this file and
                    // all remaining ones count as open (the next run catches up).
                    note_error(summary, &e, Target::Txt);
                    summary.txt_failed += total - done;
                    note_failed(summary, &job.key);
                    return;
                }
            },
            Err(e) => {
                log::warn!("text file for {} not written: {e}", job.key);
                summary.txt_failed += 1;
                note_failed(summary, &job.key);
            }
        }
    }
}

/// Writes the overview if it is missing or something changed since the last time at this
/// path. The state is remembered per path: what the app wrote there stays its own - also
/// after switching the folder and back.
fn write_overview(
    store: &Store,
    path: &Path,
    info: &[(String, String)],
    run: i64,
    now: Timestamp,
    summary: &mut ExportSummary,
) {
    // The run number belongs to the sheet "Info" and changes the file on every run.
    let stamp = serde_json::json!({
        "rev": store.data_rev().unwrap_or(-1),
        "run": run,
    })
    .to_string();
    let key = format!("{EXPORT_STAMP}{}", path.display());
    // Without a readable state the ownership of the file is unknown - then it is neither
    // backed up nor replaced. A database error must not back up the app's own overview.
    let last = match store.kv_get(&key) {
        Ok(last) => last,
        Err(e) => {
            note_error(summary, &e, Target::Overview);
            return;
        }
    };
    if path.exists() && last.as_deref() == Some(stamp.as_str()) {
        return;
    }
    // An overview that does not come from this app (e.g. from the old program in the same
    // folder) is backed up before the first write - never replaced silently. The name is
    // part of the user's workspace - do not translate.
    if path.exists() && last.is_none() {
        let backup = path.with_file_name(format!(
            "JobAlerts.alt-{}.{}",
            now.strftime("%Y%m%d-%H%M%S"),
            path.extension().and_then(|e| e.to_str()).unwrap_or("xlsx")
        ));
        if let Err(e) = std::fs::rename(path, &backup) {
            note_error(summary, &crate::Error::io(path, e), Target::Backup);
            return;
        }
        summary.backup = Some(backup);
    }
    let written = store
        .jobs(&JobFilter::default())
        .and_then(|jobs| write_xlsx(path, &jobs, info));
    match written {
        Ok(()) => {
            if let Err(e) = store.kv_set(&key, &stamp) {
                log::warn!("export stamp not stored: {e}");
            }
            summary.overview_xlsx = Some(path.to_path_buf());
        }
        Err(e) => note_error(summary, &e, Target::Overview),
    }
}

/// Remembers the numbers of a successful mailbox scan for the sheet "Info" - only then: a
/// failed scan does not overwrite the last good state. The mail address stays out of the
/// file - it may be passed on.
fn remember_scan(store: &Store, scope: Scope, scan: &ScanSummary, at: Timestamp) {
    let rows = [
        (texts::INFO_LAST_SCAN, time::display(at)),
        (texts::INFO_SCOPE, scope_text(scope).to_string()),
        (texts::INFO_NEW, scan.new.to_string()),
        (texts::INFO_KNOWN, scan.known_before.to_string()),
        (texts::INFO_DUP, scan.dup_in_run.to_string()),
    ];
    let saved = serde_json::to_string(&rows)
        .map_err(|e| e.to_string())
        .and_then(|json| {
            store
                .kv_set(LAST_SCAN_INFO, &json)
                .map_err(|e| e.to_string())
        });
    if let Err(e) = saved {
        log::warn!("mailbox scan details not stored: {e}");
    }
    if let Err(e) = store.kv_set(LAST_FETCH_AT, &time::to_db(at).to_string()) {
        log::warn!("time of the mailbox scan not stored: {e}");
    }
}

/// Start of the last successful mailbox scan.
pub fn last_fetch_at(store: &Store) -> Option<Timestamp> {
    store
        .kv_get(LAST_FETCH_AT)
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .and_then(time::from_db)
}

/// Should the app start a fetch run by itself? Only when switched on, with a mailbox, and
/// when the last successful mailbox scan is older than [`AUTO_FETCH_AFTER`] (or never was).
pub fn auto_fetch_due(
    store: &Store,
    switched_on: bool,
    mailbox_connected: bool,
    now: Timestamp,
) -> bool {
    switched_on
        && mailbox_connected
        && last_fetch_at(store).is_none_or(|at| now.duration_since(at) > AUTO_FETCH_AFTER)
}

/// Sheet "Info" of the Excel file (last scan, last run, counters, program). The numbers
/// come from the last successful mailbox scan - after pure detail runs too.
fn info_rows(store: &Store, started_at: Timestamp) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = store
        .kv_get(LAST_SCAN_INFO)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    // Earlier versions stored the mail address among the rows; it stays out now.
    rows.retain(|(label, _)| label != LEGACY_ACCOUNT_LABEL);
    // ... and their own words, read in today's until the next scan stores its rows.
    let today = |text: &mut String| {
        if let Some((_, new)) = LEGACY_INFO.iter().find(|(old, _)| old == text) {
            *text = (*new).to_owned();
        }
    };
    for (label, value) in &mut rows {
        today(label);
        today(value);
    }
    rows.push((texts::INFO_LAST_RUN.into(), time::display(started_at)));
    rows.push((
        texts::INFO_JOBS_TOTAL.into(),
        store.job_count().unwrap_or(0).to_string(),
    ));
    rows.push((texts::INFO_PROGRAM.into(), texts::PROGRAM_NAME.into()));
    rows
}

fn log_export(run: i64, exported: &ExportSummary) {
    log::info!(
        "run {run}: export: {} text files written, {} failed{}{}",
        exported.txt_written,
        exported.txt_failed,
        if exported.overview_xlsx.is_some() {
            ", overview written"
        } else {
            ""
        },
        if exported.backup.is_some() {
            ", a foreign overview was backed up"
        } else {
            ""
        }
    );
    if !exported.txt_failed_keys.is_empty() {
        log::warn!(
            "run {run}: text files not written for {}",
            exported.txt_failed_keys.join(", ")
        );
    }
}

fn scope_text(scope: Scope) -> &'static str {
    match scope {
        Scope::New => texts::SCOPE_NEW,
        Scope::All => texts::SCOPE_ALL,
    }
}

#[cfg(test)]
mod tests;
