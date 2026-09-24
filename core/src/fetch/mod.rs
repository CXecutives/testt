//! Fetching job details: queue, safety rules, outcome matrix.
//!
//! The portals run side by side - **within** a portal, however, strictly one after the
//! other: same gaps, same caps, same breaker. That is a safety property, not a speed-up.
//! Every description is stored right away. After a block signal there is **no** automatic
//! fallback route - the portal pauses.
//!
//! This module knows no single portal: which portals exist, how they are read and how fast
//! comes from the registry (`crate::portal`).
//!
//! The safety state sits behind a short lock; it is never held across a sleep or a request,
//! and `policy.json` therefore has exactly one writer.

pub mod http;
pub mod policy;
pub mod site;

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::task::Poll;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use jiff::{SignedDuration, Timestamp};
use tokio_util::sync::CancellationToken;

use crate::model::DescStatus;
use crate::portal::{Access, Facts, JobKey, JobLink, PORTALS, Portal, PortalAdapter};
use crate::store::{JobRow, Store};
use http::HttpFetcher;
use policy::{Allowance, NET_RETRY, PauseKind, PauseReason, Policy};
pub use policy::{MAX_AGE, RETRY_AFTER};

/// From this length on a text counts as complete without further checks.
const MIN_TEXT_CHARS: usize = 100;
/// Longer waits are announced beforehand (the interface shows a countdown).
const WAIT_NOTICE: Duration = Duration::from_secs(1);
/// So many suspicious pages in a row stop a portal for the run.
const SUSPICIOUS_STREAK: u32 = 2;

/// Why a page gave no full text, or why a portal stopped - a code, never prose. Stored as
/// its [`Display`](fmt::Display) form (`noDescription`, `http429`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cause {
    /// No description container on the page (layout changed?).
    NoDescription,
    /// The description container is empty.
    EmptyDescription,
    /// The page shows another job than the one from the mail.
    WrongPage,
    /// Without its data island the page cannot be recognised as the job.
    PageNotRecognised,
    /// Redirected to a page that is no job page.
    NotAProjectPage,
    /// Bigger than any ad.
    PageTooLarge,
    /// Redirected to a sign-in or security check page.
    LoginWall,
    /// Redirected somewhere unexpected.
    UnexpectedRedirect,
    /// A redirect the client did not follow (foreign host, too many).
    RedirectNotFollowed,
    /// The same page redirected twice.
    RepeatedRedirect,
    /// Redirected right after the sign-in (the page is requested again).
    PostLoginRedirect,
    /// An HTTP status that decides by itself.
    Http(u16),
    /// No answer within the time limit.
    Timeout,
    /// No connection to the portal.
    NoConnection,
    /// The connection broke off.
    ConnectionLost,
    /// No answer, twice in a row.
    NoAnswerTwice,
    /// The session window did not load a page of the portal.
    NotLoaded,
    /// The probe script failed on the page.
    ProbeFailed,
    /// The page did not answer the probe.
    NoProbeAnswer,
    /// The probe answer was unreadable.
    ProbeUnreadable,
    /// A security check (captcha).
    Captcha,
    /// The sign-in page.
    LoginPage,
    /// No sign-out link: not signed in.
    NoLogoutLink,
    /// Only the teaser although signed in.
    TeaserDespiteSession,
    /// The session window is gone or could not be created or navigated.
    NoWindow,
    /// The user closed the session window.
    WindowClosed,
    /// Two pages without a description in a row.
    Breaker,
    /// A sample answer of the dry run.
    DrySample,
}

impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cause::Http(code) => write!(f, "http{code}"),
            other => {
                let code = serde_json::to_value(other).unwrap_or_default();
                f.write_str(code.as_str().unwrap_or("unknown"))
            }
        }
    }
}

/// Structured data of a page (more reliable than the mail heuristics).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageFields {
    pub title: String,
    pub company: String,
    pub location: String,
}

/// Result of a page request (outcome matrix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageOutcome {
    /// Description found. `short`: below 100 characters but verified (container present,
    /// no sign-in wall, the right page).
    Text {
        text: String,
        short: bool,
        closed: bool,
        fields: Option<PageFields>,
        facts: Facts,
    },
    /// Only the teaser a guest sees (freelance.de without sign-in): short, but the right
    /// page - stored for matching and marked, never as a text file.
    Teaser {
        text: String,
        fields: Option<PageFields>,
        facts: Facts,
    },
    /// The ad no longer exists.
    Gone,
    /// Page loaded but without a recognisable description.
    Suspicious(Cause),
    /// Only readable with a (new) sign-in.
    LoginRequired(Cause),
    /// Rate limited; `retry_after`: the portal's own `Retry-After`, the shortest pause.
    Throttled {
        cause: Cause,
        retry_after: Option<Duration>,
    },
    Blocked(Cause),
    NetError {
        timeout: bool,
        cause: Cause,
    },
    /// The portal redirected once (after the sign-in) - request the same page again, as a
    /// new, counted request.
    Retry(Cause),
    Cancelled,
}

/// What a page parser found.
#[derive(Debug, Default)]
pub(crate) struct Parsed {
    /// `None`: no description container on the page.
    pub text: Option<String>,
    pub closed: bool,
    pub fields: PageFields,
    pub facts: Facts,
}

/// Parser result -> outcome matrix.
pub(crate) fn judge(parsed: Parsed) -> PageOutcome {
    match parsed.text {
        None => PageOutcome::Suspicious(Cause::NoDescription),
        Some(text) if text.trim().is_empty() => PageOutcome::Suspicious(Cause::EmptyDescription),
        Some(text) => {
            let short = text.chars().count() < MIN_TEXT_CHARS;
            let fields = Some(parsed.fields).filter(|f| *f != PageFields::default());
            PageOutcome::Text {
                text,
                short,
                closed: parsed.closed,
                fields,
                facts: parsed.facts,
            }
        }
    }
}

/// Result of a sign-in by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Login {
    SignedIn,
    /// Signed in, but the portal showed a security check: the session counts, the portal
    /// rests until the next run (the app never solves a check itself).
    Challenged,
    /// Not signed in (window closed, time up, cancelled).
    NotSignedIn,
}

/// The fetch path of one portal in a run - a guest client or a session window.
pub trait PageFetcher {
    fn fetch(
        &mut self,
        link: &JobLink,
        cancel: &CancellationToken,
    ) -> impl Future<Output = PageOutcome> + Send;

    /// Lets the user sign in (session window visible). Without a sign-in: not signed in.
    fn login(
        &mut self,
        portal: Portal,
        cancel: &CancellationToken,
    ) -> impl Future<Output = Login> + Send {
        let _ = (portal, cancel);
        async { Login::NotSignedIn }
    }

    /// Whether this path reads with the user's sign-in (a session window).
    fn session(&self) -> bool {
        false
    }
}

/// The fetch path the app builds for a portal: only the one it needs.
pub enum Fetchers<S> {
    Guest(HttpFetcher),
    Session(S),
}

impl<S: PageFetcher + Send> PageFetcher for Fetchers<S> {
    async fn fetch(&mut self, link: &JobLink, cancel: &CancellationToken) -> PageOutcome {
        match self {
            Fetchers::Guest(http) => http.fetch(link, cancel).await,
            Fetchers::Session(session) => session.fetch(link, cancel).await,
        }
    }

    async fn login(&mut self, portal: Portal, cancel: &CancellationToken) -> Login {
        match self {
            Fetchers::Guest(_) => Login::NotSignedIn,
            Fetchers::Session(session) => session.login(portal, cancel).await,
        }
    }

    fn session(&self) -> bool {
        matches!(self, Fetchers::Session(_))
    }
}

/// Health of a portal for the interface: why it rests or what looks wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum PortalHealth {
    Ok,
    /// The portal rests. `until: null` = until the next run (network trouble, security
    /// check at the sign-in).
    Paused {
        until: Option<Timestamp>,
        reason: PauseReason,
    },
    /// Hourly or daily cap reached; requests are possible again from `until`.
    QuotaReached {
        until: Timestamp,
    },
    /// Alert mails without recognised jobs (`emptyMails`) or pages without a description
    /// in a row (`pages`) - the portal probably changed its layout.
    LayoutSuspect {
        empty_mails: usize,
        pages: u32,
    },
    /// A sign-in is needed to read the portal.
    LoginRequired,
}

impl PortalHealth {
    /// The one backend truth about a portal, from its safety state: pause, cap, sign-in
    /// needed (only with the sign-in switched on - otherwise the portal goes as a guest),
    /// layout suspect (`empty_mails`: alert mails without jobs; or pages without a
    /// description in a row) - in this order.
    pub fn of(
        policy: &Policy,
        portal: Portal,
        now: Timestamp,
        login_enabled: bool,
        empty_mails: usize,
    ) -> PortalHealth {
        let state = policy.state(portal);
        match policy.allowance(portal, now) {
            Allowance::Paused { until, reason, .. } => PortalHealth::Paused {
                until: Some(until),
                reason,
            },
            Allowance::Quota { next_at } => PortalHealth::QuotaReached { until: next_at },
            Allowance::Go if login_enabled && state.login_needed => PortalHealth::LoginRequired,
            Allowance::Go if empty_mails > 0 || state.suspicious_streak > 0 => {
                PortalHealth::LayoutSuspect {
                    empty_mails,
                    pages: state.suspicious_streak,
                }
            }
            Allowance::Go => PortalHealth::Ok,
        }
    }
}

/// Why a portal is not fetched (any further) in this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Pause from an earlier run or imposed just now. `detail` is the stored code (log
    /// only).
    Paused {
        until: Timestamp,
        reason: PauseReason,
        detail: String,
    },
    /// Cap reached - the rest is possible from `next_at`.
    Quota { next_at: Timestamp },
    /// Two suspicious pages in a row (breaker) - the portal pauses until `until`.
    Breaker { until: Timestamp },
    /// Sign-in needed and it did not happen during the run.
    LoginRequired,
    /// Signed in, but with a security check - the portal rests until the next run.
    Challenged,
    /// Network trouble (second failure after a retry).
    Network { cause: Cause },
}

impl StopReason {
    /// The stop as the interface sees it.
    pub fn health(&self) -> PortalHealth {
        match self {
            StopReason::Paused { until, reason, .. } => PortalHealth::Paused {
                until: Some(*until),
                reason: *reason,
            },
            StopReason::Quota { next_at } => PortalHealth::QuotaReached { until: *next_at },
            StopReason::Breaker { until } => PortalHealth::Paused {
                until: Some(*until),
                reason: PauseReason::LayoutChanged,
            },
            StopReason::LoginRequired => PortalHealth::LoginRequired,
            StopReason::Challenged => PortalHealth::Paused {
                until: None,
                reason: PauseReason::Challenged,
            },
            StopReason::Network { .. } => PortalHealth::Paused {
                until: None,
                reason: PauseReason::Network,
            },
        }
    }

    /// One English line for the log.
    pub fn log_line(&self, portal: Portal, skipped: usize) -> String {
        let key = portal.key();
        match self {
            StopReason::Paused {
                until,
                reason,
                detail,
            } => format!("{key}: paused until {until} ({reason:?}: {detail}), {skipped} left"),
            StopReason::Quota { next_at } => {
                format!("{key}: cap reached, {skipped} left until {next_at}")
            }
            StopReason::Breaker { until } => {
                format!("{key}: breaker (layout changed?), paused until {until}, {skipped} left")
            }
            StopReason::LoginRequired => format!("{key}: sign-in needed, {skipped} left"),
            StopReason::Challenged => {
                format!("{key}: security check at the sign-in, {skipped} left until the next run")
            }
            StopReason::Network { cause } => {
                format!("{key}: network trouble ({cause}), {skipped} left")
            }
        }
    }
}

/// Counters per portal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortalCounts {
    pub ok: usize,
    /// Only the teaser (guest path).
    pub teaser: usize,
    pub short: usize,
    pub closed: usize,
    pub gone: usize,
    pub failed: usize,
    /// Skipped or waiting jobs (pause, cap, sign-in, breaker, network).
    pub skipped: usize,
    /// Why the portal stopped in this run.
    pub stop: Option<StopReason>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchSummary {
    pub queued: usize,
    pub per_portal: BTreeMap<Portal, PortalCounts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchEvent {
    /// After a parser update: so many failed jobs of the portal are open again.
    Requeued {
        portal: Portal,
        count: usize,
    },
    Queued {
        total: usize,
    },
    /// A page of the portal is about to be requested.
    Fetching {
        portal: Portal,
    },
    /// The sign-in at the portal begins.
    SigningIn {
        portal: Portal,
    },
    /// Waiting until `until` (gap or retry after a network error).
    Waiting {
        portal: Portal,
        until: Timestamp,
    },
    /// A job has a new state (refresh the list).
    JobUpdated {
        key: JobKey,
        status: DescStatus,
    },
    PortalStopped {
        portal: Portal,
        reason: StopReason,
        skipped: usize,
    },
    Progress {
        done: usize,
        total: usize,
    },
}

/// What should be fetched.
#[derive(Debug, Clone, Copy)]
pub enum Selection<'a> {
    /// Open jobs of these portals (mails of the last 30 days).
    Queue(&'a [Portal]),
    /// Exactly these jobs ("fetch details"), older ones too - but only of these portals.
    Jobs(&'a [JobKey], &'a [Portal]),
}

/// Result of [`admit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Request counted and saved - the portal may be contacted now.
    Go,
    /// Pause or cap: no request.
    Stop(StopReason),
    /// Cancelled before anything was counted.
    Cancelled,
}

/// Every portal request passes here - pages, the sign-in during a run and signing in/out by
/// hand: first check pause and caps (a paused portal does not wait first), then wait for
/// the gap (cancellable; `on_wait` learns the end of longer waits beforehand), then count and
/// save durably - before the request, so that a crash in between cannot defeat the cap.
/// No request without a saved state.
pub async fn admit(
    policy: &Mutex<Policy>,
    portal: Portal,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    on_wait: impl FnOnce(Timestamp),
) -> crate::Result<Admission> {
    if cancel.is_cancelled() {
        return Ok(Admission::Cancelled);
    }
    // The lock only covers checking and computing, never the sleep.
    let wait = {
        let policy = lock(policy);
        match policy.allowance(portal, clock()) {
            Allowance::Paused {
                until,
                reason,
                detail,
            } => {
                return Ok(Admission::Stop(StopReason::Paused {
                    until,
                    reason,
                    detail,
                }));
            }
            Allowance::Quota { next_at } => {
                return Ok(Admission::Stop(StopReason::Quota { next_at }));
            }
            Allowance::Go => {}
        }
        policy.pace_wait(portal, clock())
    };
    if let Some(wait) = wait {
        if wait > WAIT_NOTICE {
            on_wait(until(clock(), wait));
        }
        if !sleep_for(wait, cancel).await {
            return Ok(Admission::Cancelled);
        }
    }
    let mut policy = lock(policy);
    policy.record_access(portal, clock());
    policy.save()?;
    Ok(Admission::Go)
}

/// Short access to the safety state. A poisoned state is no reason to abort the run: its
/// counters are valid, and without them there would be no cap at all.
fn lock(policy: &Mutex<Policy>) -> MutexGuard<'_, Policy> {
    policy.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What all portal loops share.
struct Shared<'a, C: Fn() -> Timestamp> {
    store: &'a Store,
    policy: &'a Mutex<Policy>,
    /// Cancellation of **this** fetch - a database error in one portal stops the others this
    /// way, instead of letting them continue without a saved state.
    cancel: &'a CancellationToken,
    clock: &'a C,
}

/// What a portal loop leaves behind.
struct PortalRun {
    portal: Portal,
    counts: PortalCounts,
    completed: bool,
}

/// Message of a portal loop to the one event collector.
enum Note {
    Event(FetchEvent),
    /// A job is judged - progress counts across all portals.
    Done,
}

fn note(notes: &mpsc::UnboundedSender<Note>, event: FetchEvent) {
    let _ = notes.send(Note::Event(event));
}

/// The registered portals in fetch order: guest portals first, portals with a session
/// window last.
fn fetch_order() -> impl Iterator<Item = &'static dyn PortalAdapter> {
    let guest = |a: &&&dyn PortalAdapter| a.access() == Access::Guest;
    let first = PORTALS.iter().filter(guest);
    let last = PORTALS.iter().filter(move |a| !guest(a));
    first.chain(last).copied()
}

/// Fetches the job details. `pages` returns the fetch path of a portal - every portal gets
/// its own (own HTTP session, own window) so the portals can run side by side. `clock`
/// returns the current time (controllable in tests).
///
/// Errors of the database or while saving `policy.json` abort - without a durable safety
/// state no portal is fetched any further.
#[expect(
    clippy::too_many_arguments,
    reason = "run context: store, rules, selection, clock and events come separately (tests)"
)]
pub async fn fetch_all<F: PageFetcher>(
    mut pages: impl FnMut(Portal) -> Result<F, String>,
    store: &Store,
    policy: &Mutex<Policy>,
    selection: Selection<'_>,
    cancel: &CancellationToken,
    clock: impl Fn() -> Timestamp,
    summary: &mut FetchSummary,
    mut on_event: impl FnMut(FetchEvent),
) -> crate::Result<bool> {
    // After a parser update the portal's failed jobs get a fresh chance.
    if let Selection::Queue(portals) = selection {
        for adapter in PORTALS.iter().filter(|a| portals.contains(&a.portal())) {
            let portal = adapter.portal();
            let count = store.requeue_older_parses(portal, adapter.parser_version())?;
            if count > 0 {
                on_event(FetchEvent::Requeued { portal, count });
            }
        }
    }
    let queue = queue(store, selection, clock())?;
    let mut by_portal: BTreeMap<Portal, Vec<JobRow>> = BTreeMap::new();
    for job in queue {
        by_portal.entry(job.key.portal).or_default().push(job);
    }
    // Only portals with work get a fetch path - built before the start: if one fails, no
    // portal begins.
    let mut work = Vec::new();
    for adapter in fetch_order() {
        let portal = adapter.portal();
        if let Some(mut jobs) = by_portal.remove(&portal) {
            let fetcher = pages(portal)
                .map_err(|detail| crate::Error::FetchUnavailable { portal, detail })?;
            // A teaser is only worth another request where the full text can come: in the
            // session window.
            if !fetcher.session() {
                jobs.retain(|job| job.desc_status != DescStatus::Teaser);
            }
            if !jobs.is_empty() {
                work.push((portal, jobs, fetcher));
            }
        }
    }
    let total = work.iter().map(|(_, jobs, _)| jobs.len()).sum();
    summary.queued = total;
    on_event(FetchEvent::Queued { total });

    // Own cancellation: the user cancels through the given token, an error in a portal
    // through this one.
    let inner = cancel.child_token();
    let shared = Shared {
        store,
        policy,
        cancel: &inner,
        clock: &clock,
    };
    let (notes, mut incoming) = mpsc::unbounded_channel::<Note>();
    let mut loops: Vec<Pin<Box<_>>> = work
        .into_iter()
        .map(|work| Box::pin(fetch_portal(work, &shared, notes.clone())))
        .collect();
    let mut runs: Vec<Option<crate::Result<PortalRun>>> = loops.iter().map(|_| None).collect();
    let mut done = 0;
    let mut deliver = |note: Note| match note {
        Note::Event(event) => on_event(event),
        Note::Done => {
            done += 1;
            on_event(FetchEvent::Progress { done, total });
        }
    };
    // All portal loops side by side in this one task. What a loop reports is delivered
    // right after its step - before the next loop runs, so a cancel in an event handler
    // reaches the other portals before their next request.
    std::future::poll_fn(|cx| {
        let mut pending = false;
        for (future, run) in loops.iter_mut().zip(runs.iter_mut()) {
            if run.is_none() {
                match future.as_mut().poll(cx) {
                    Poll::Ready(result) => *run = Some(result),
                    Poll::Pending => pending = true,
                }
            }
            while let Ok(note) = incoming.try_recv() {
                deliver(note);
            }
        }
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;

    let mut completed = true;
    for result in runs.into_iter().flatten() {
        let run = result?;
        completed &= run.completed;
        summary.per_portal.insert(run.portal, run.counts);
    }
    Ok(completed)
}

/// One portal loop: strictly sequential, with all rules. An error stops the other portals
/// too - without a saved safety state nobody fetches any further.
async fn fetch_portal<F: PageFetcher, C: Fn() -> Timestamp>(
    work: (Portal, Vec<JobRow>, F),
    shared: &Shared<'_, C>,
    notes: mpsc::UnboundedSender<Note>,
) -> crate::Result<PortalRun> {
    let portal = work.0;
    let result = portal_loop(work, shared, &notes).await;
    if result.is_err() {
        shared.cancel.cancel();
    }
    result.map(|(counts, completed)| PortalRun {
        portal,
        counts,
        completed,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "the outcome matrix reads best in one piece"
)]
async fn portal_loop<F: PageFetcher, C: Fn() -> Timestamp>(
    (portal, jobs, mut fetcher): (Portal, Vec<JobRow>, F),
    shared: &Shared<'_, C>,
    notes: &mpsc::UnboundedSender<Note>,
) -> crate::Result<(PortalCounts, bool)> {
    let (store, policy, cancel, clock) = (shared.store, shared.policy, shared.cancel, shared.clock);
    let mut counts = PortalCounts::default();
    let session = fetcher.session();
    let parser_version = portal.adapter().parser_version();
    // One sign-in per portal and run; afterwards the same job is tried again.
    let mut login_tried = false;
    // Job whose page was already retried once (at most one retry).
    let mut retried: Option<usize> = None;
    let mut index = 0;
    while index < jobs.len() {
        let job = &jobs[index];
        let remaining = jobs.len() - index;
        let link = JobLink {
            key: job.key.clone(),
            url: job.url.clone(),
        };
        let mut outcome = match access(&mut fetcher, policy, &link, cancel, clock, notes).await? {
            Ok(outcome) => outcome,
            Err(reason) => {
                stop(&mut counts, &reason, remaining, portal, notes);
                break;
            }
        };
        if let PageOutcome::NetError { .. } = outcome {
            // Retry once - after 30 s, again with all rules.
            note(
                notes,
                FetchEvent::Waiting {
                    portal,
                    until: until(clock(), NET_RETRY),
                },
            );
            if !sleep_for(NET_RETRY, cancel).await {
                return Ok((counts, false));
            }
            outcome = match access(&mut fetcher, policy, &link, cancel, clock, notes).await? {
                Ok(PageOutcome::NetError { timeout: true, .. }) => PageOutcome::Throttled {
                    cause: Cause::NoAnswerTwice,
                    retry_after: None,
                },
                Ok(outcome) => outcome,
                Err(reason) => {
                    stop(&mut counts, &reason, remaining, portal, notes);
                    break;
                }
            };
        }
        // If the same page redirects a second time, something is wrong: suspicious, so the
        // breaker applies.
        let outcome = match outcome {
            PageOutcome::Retry(_) if retried == Some(index) => {
                PageOutcome::Suspicious(Cause::RepeatedRedirect)
            }
            other => other,
        };

        let now = clock();
        lock(policy).record_done(portal, now);
        let stop_reason = match outcome {
            PageOutcome::Cancelled => {
                // The answer time counts for the next gap even after a cancellation.
                lock(policy).save()?;
                return Ok((counts, false));
            }
            PageOutcome::Retry(_) => {
                retried = Some(index);
                lock(policy).save()?;
                continue;
            }
            PageOutcome::Text {
                text,
                short,
                closed,
                fields,
                facts,
            } => {
                store.record_text(&job.key, &text, short, closed, now)?;
                store.record_parse(&job.key, parser_version, Some(&facts))?;
                // Only non-empty fields overwrite the mail heuristics: what the page hides
                // ("visible for EXPERT members") arrives empty.
                if let Some(f) = fields {
                    store.record_page_fields(&job.key, &f.title, &f.company, &f.location)?;
                }
                {
                    let mut policy = lock(policy);
                    // A page read in the session window confirms the sign-in; the guest
                    // path says nothing about it.
                    if session {
                        policy.set_session(portal, true, now);
                    }
                    // Only an undoubtedly complete text resets the breaker.
                    if !short {
                        policy.clear_suspicious(portal);
                    }
                }
                counts.ok += 1;
                counts.short += usize::from(short);
                counts.closed += usize::from(closed);
                note(
                    notes,
                    FetchEvent::JobUpdated {
                        key: job.key.clone(),
                        status: DescStatus::Ok,
                    },
                );
                None
            }
            PageOutcome::Teaser {
                text,
                fields,
                facts,
            } => {
                store.record_teaser(&job.key, &text, now)?;
                store.record_parse(&job.key, parser_version, Some(&facts))?;
                if let Some(f) = fields {
                    store.record_page_fields(&job.key, &f.title, &f.company, &f.location)?;
                }
                // The right page, read as far as a guest can: the layout is fine.
                lock(policy).clear_suspicious(portal);
                counts.teaser += 1;
                note(
                    notes,
                    FetchEvent::JobUpdated {
                        key: job.key.clone(),
                        status: DescStatus::Teaser,
                    },
                );
                None
            }
            PageOutcome::Gone => {
                store.record_gone(&job.key, now)?;
                counts.gone += 1;
                note(
                    notes,
                    FetchEvent::JobUpdated {
                        key: job.key.clone(),
                        status: DescStatus::Gone,
                    },
                );
                None
            }
            PageOutcome::Suspicious(cause) => {
                // A series of suspicious pages in a row (layout changed?) is the portal's
                // fault, not the jobs': the whole series costs ONE attempt - its first page.
                let streak = lock(policy).count_suspicious(portal);
                let status =
                    store.record_failure(&job.key, &cause.to_string(), now, streak <= 1)?;
                store.record_parse(&job.key, parser_version, None)?;
                counts.failed += 1;
                note(
                    notes,
                    FetchEvent::JobUpdated {
                        key: job.key.clone(),
                        status,
                    },
                );
                // Two in a row - across runs too - stop the portal and pause it for an hour:
                // the page layout has probably changed.
                (streak >= SUSPICIOUS_STREAK).then(|| StopReason::Breaker {
                    until: lock(policy).pause_for(
                        portal,
                        PauseKind::Throttled,
                        PauseReason::LayoutChanged,
                        &Cause::Breaker.to_string(),
                        now,
                    ),
                })
            }
            // The guest path has no sign-in: the portal waits for the next run.
            PageOutcome::LoginRequired(_) if !session => Some(StopReason::LoginRequired),
            PageOutcome::LoginRequired(_) => {
                {
                    let mut policy = lock(policy);
                    policy.set_session(portal, false, now);
                    policy.save()?;
                }
                if login_tried {
                    Some(StopReason::LoginRequired)
                } else {
                    login_tried = true;
                    // The sign-in page is a portal request like any other.
                    let admission = admit(policy, portal, cancel, clock, |until| {
                        note(notes, FetchEvent::Waiting { portal, until });
                    })
                    .await?;
                    match admission {
                        Admission::Stop(reason) => Some(reason),
                        Admission::Cancelled => {
                            lock(policy).save()?;
                            return Ok((counts, false));
                        }
                        Admission::Go => {
                            note(notes, FetchEvent::SigningIn { portal });
                            let login = fetcher.login(portal, cancel).await;
                            // The gap to the next page applies after the sign-in too.
                            lock(policy).record_done(portal, clock());
                            match login {
                                Login::SignedIn => {
                                    let mut policy = lock(policy);
                                    policy.set_session(portal, true, clock());
                                    policy.save()?;
                                    continue;
                                }
                                Login::Challenged => {
                                    lock(policy).set_session(portal, true, clock());
                                    Some(StopReason::Challenged)
                                }
                                Login::NotSignedIn if cancel.is_cancelled() => {
                                    lock(policy).save()?;
                                    return Ok((counts, false));
                                }
                                Login::NotSignedIn => Some(StopReason::LoginRequired),
                            }
                        }
                    }
                }
            }
            PageOutcome::Throttled { cause, retry_after } => {
                // The portal's own Retry-After is the shortest pause.
                let detail = cause.to_string();
                let until = lock(policy).pause_at_least(
                    portal,
                    PauseKind::Throttled,
                    &detail,
                    now,
                    retry_after,
                );
                Some(StopReason::Paused {
                    until,
                    reason: PauseReason::Throttled,
                    detail,
                })
            }
            PageOutcome::Blocked(cause) => {
                let detail = cause.to_string();
                let until = lock(policy).pause(portal, PauseKind::Blocked, &detail, now);
                Some(StopReason::Paused {
                    until,
                    reason: PauseReason::Blocked,
                    detail,
                })
            }
            PageOutcome::NetError { cause, .. } => Some(StopReason::Network { cause }),
        };
        lock(policy).save()?;
        let _ = notes.send(Note::Done);
        if let Some(reason) = stop_reason {
            // The current job counts along if it was not judged.
            let skipped = if matches!(reason, StopReason::Breaker { .. }) {
                remaining - 1
            } else {
                remaining
            };
            stop(&mut counts, &reason, skipped, portal, notes);
            break;
        }
        index += 1;
    }
    Ok((counts, true))
}

/// Open jobs: automatically only from the last 30 days; explicitly chosen jobs older ones
/// too - but never one already fetched successfully (success is never fetched again).
fn queue(store: &Store, selection: Selection<'_>, now: Timestamp) -> crate::Result<Vec<JobRow>> {
    Ok(match selection {
        Selection::Queue(portals) => store
            .fetch_queue(now, MAX_AGE, RETRY_AFTER)?
            .into_iter()
            .filter(|j| portals.contains(&j.key.portal))
            .collect(),
        Selection::Jobs(keys, portals) => {
            let mut jobs = Vec::new();
            for key in keys.iter().filter(|k| portals.contains(&k.portal)) {
                if let Some(job) = store.job(key)?
                    && job.desc_status != DescStatus::Ok
                    && !jobs.iter().any(|j: &JobRow| j.key == job.key)
                {
                    jobs.push(job);
                }
            }
            jobs
        }
    })
}

/// Requests a page with all rules. `Err` = the portal may not be requested right now; a
/// cancellation before the request comes as `PageOutcome::Cancelled`.
async fn access<F: PageFetcher>(
    fetcher: &mut F,
    policy: &Mutex<Policy>,
    link: &JobLink,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    notes: &mpsc::UnboundedSender<Note>,
) -> crate::Result<Result<PageOutcome, StopReason>> {
    let portal = link.key.portal;
    let admission = admit(policy, portal, cancel, clock, |until| {
        note(notes, FetchEvent::Waiting { portal, until });
    })
    .await?;
    match admission {
        Admission::Go => {
            note(notes, FetchEvent::Fetching { portal });
            Ok(Ok(fetcher.fetch(link, cancel).await))
        }
        Admission::Stop(reason) => Ok(Err(reason)),
        Admission::Cancelled => Ok(Ok(PageOutcome::Cancelled)),
    }
}

fn stop(
    counts: &mut PortalCounts,
    reason: &StopReason,
    skipped: usize,
    portal: Portal,
    notes: &mpsc::UnboundedSender<Note>,
) {
    counts.skipped += skipped;
    note(
        notes,
        FetchEvent::PortalStopped {
            portal,
            reason: reason.clone(),
            skipped,
        },
    );
    counts.stop = Some(reason.clone());
}

/// End of a wait from `now`.
fn until(now: Timestamp, wait: Duration) -> Timestamp {
    SignedDuration::try_from(wait)
        .ok()
        .and_then(|wait| now.checked_add(wait).ok())
        .unwrap_or(Timestamp::MAX)
}

/// Waits `wait`; `false` if cancelled before.
async fn sleep_for(wait: Duration, cancel: &CancellationToken) -> bool {
    tokio::select! {
        biased;
        () = cancel.cancelled() => false,
        () = tokio::time::sleep(wait) => true,
    }
}

#[cfg(test)]
mod tests;
