//! Fetching job details: queue, safety rules, outcome matrix.
//!
//! The three portals run side by side - **within** a portal, however, strictly one after
//! the other: same gaps, same caps, same breaker. That is a safety property, not a speed-up.
//! Every description is stored right away. After a block signal there is **no** automatic
//! fallback route - the portal pauses.
//!
//! The safety state sits behind a short lock; it is never held across a sleep or a request,
//! and `policy.json` therefore has exactly one writer.

mod freelance_de;
mod freelancermap;
pub mod http;
mod linkedin;
pub mod policy;
pub mod site;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use jiff::{SignedDuration, Timestamp};
use tokio_util::sync::CancellationToken;

use crate::model::DescStatus;
use crate::portal::{JobKey, JobLink, LoginMode, Portal};
use crate::store::{JobRow, Store};
use http::HttpFetcher;
use policy::{Allowance, PauseKind, PauseReason, Policy};

/// From this length on a text counts as complete without further checks.
const MIN_TEXT_CHARS: usize = 100;
/// Only jobs from mails of the last 30 days are fetched automatically (older ones per click).
pub const MAX_AGE: SignedDuration = SignedDuration::from_hours(30 * 24);
/// A failed fetch is retried after 12 hours at the earliest.
pub const RETRY_AFTER: SignedDuration = SignedDuration::from_hours(12);
/// After a network error: retry once, after this wait.
const NET_RETRY_DELAY: Duration = Duration::from_secs(30);
/// Longer waits are announced beforehand (the interface shows a countdown).
const WAIT_NOTICE: Duration = Duration::from_secs(1);
/// So many suspicious pages in a row stop a portal for the run.
const SUSPICIOUS_STREAK: u32 = 2;
/// Pause detail after the breaker (log only).
const BREAKER_DETAIL: &str = "two pages without a description in a row";
/// Order of the portals when fetching: public first, the session last.
const FETCH_ORDER: [Portal; 3] = [Portal::Freelancermap, Portal::LinkedIn, Portal::FreelanceDe];

/// Structured data of a page (more reliable than the mail heuristics).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageFields {
    pub title: String,
    pub company: String,
    pub location: String,
}

/// Result of a page request (outcome matrix). Texts are details for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageOutcome {
    /// Description found. `short`: below 100 characters but verified (container present,
    /// no sign-in wall, the right page).
    Text {
        text: String,
        short: bool,
        closed: bool,
        fields: Option<PageFields>,
    },
    /// The ad no longer exists.
    Gone,
    /// Page loaded but without a recognisable description.
    Suspicious(String),
    /// Only readable with a (new) sign-in; the text names the sign (for the log).
    LoginRequired(String),
    Throttled(String),
    Blocked(String),
    NetError {
        timeout: bool,
        detail: String,
    },
    /// The portal redirected once (after the sign-in) - request the same page again, as a
    /// new, counted request.
    Retry(String),
    Cancelled,
}

/// What a page parser found.
#[derive(Debug, Default)]
pub(crate) struct Parsed {
    /// `None`: no description container on the page.
    pub text: Option<String>,
    pub closed: bool,
    pub fields: PageFields,
}

/// Parser result -> outcome matrix.
pub(crate) fn judge(parsed: Parsed) -> PageOutcome {
    match parsed.text {
        None => PageOutcome::Suspicious("no description found (page layout changed?)".into()),
        Some(text) if text.trim().is_empty() => PageOutcome::Suspicious("empty description".into()),
        Some(text) => {
            let short = text.chars().count() < MIN_TEXT_CHARS;
            let fields = Some(parsed.fields).filter(|f| *f != PageFields::default());
            PageOutcome::Text {
                text,
                short,
                closed: parsed.closed,
                fields,
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

/// Which route a page is fetched on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Without an account, as a guest.
    Http,
    /// In the session window, with the user's sign-in.
    Session,
}

/// The only router: the session window exactly when the portal gives nothing without a
/// sign-in. Everything else goes as a guest.
pub fn route(portal: Portal) -> Route {
    match portal.login_mode() {
        LoginMode::None => Route::Http,
        LoginMode::Required => Route::Session,
    }
}

/// Fetches a page - as a guest (HTTP) or in the session window; [`route`] decides.
pub trait PageFetcher {
    fn fetch(
        &mut self,
        link: &JobLink,
        route: Route,
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
}

/// The two fetch routes of the app side by side.
pub struct Fetchers<S> {
    pub http: HttpFetcher,
    pub session: S,
}

impl<S: PageFetcher + Send> PageFetcher for Fetchers<S> {
    async fn fetch(
        &mut self,
        link: &JobLink,
        route: Route,
        cancel: &CancellationToken,
    ) -> PageOutcome {
        match route {
            Route::Session => self.session.fetch(link, route, cancel).await,
            Route::Http => self.http.fetch(link, route, cancel).await,
        }
    }

    async fn login(&mut self, portal: Portal, cancel: &CancellationToken) -> Login {
        self.session.login(portal, cancel).await
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

/// Why a portal is not fetched (any further) in this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Pause from an earlier run or imposed just now. `detail` is for the log.
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
    Network { detail: String },
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
            StopReason::Network { detail } => {
                format!("{key}: network trouble ({detail}), {skipped} left")
            }
        }
    }
}

/// Counters per portal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortalCounts {
    pub ok: usize,
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
    pub per_portal: std::collections::BTreeMap<Portal, PortalCounts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchEvent {
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
    /// `None`: the portal had nothing to fetch in this run.
    counts: Option<PortalCounts>,
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

/// Fetches the job details. `pages` returns the fetch route of a portal - every portal gets
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
    let queue = queue(store, selection, clock())?;
    let total = queue.len();
    summary.queued = total;
    on_event(FetchEvent::Queued { total });

    let mut by_portal: BTreeMap<Portal, Vec<JobRow>> = BTreeMap::new();
    for job in queue {
        by_portal.entry(job.key.portal).or_default().push(job);
    }
    // The fetch route is created before the start: if it fails, no portal begins.
    let mut prepare = |portal: Portal| -> crate::Result<(Portal, Vec<JobRow>, Option<F>)> {
        let jobs = by_portal.remove(&portal).unwrap_or_default();
        let fetcher = if jobs.is_empty() {
            None
        } else {
            Some(
                pages(portal)
                    .map_err(|detail| crate::Error::FetchUnavailable { portal, detail })?,
            )
        };
        Ok((portal, jobs, fetcher))
    };
    let first = prepare(FETCH_ORDER[0])?;
    let second = prepare(FETCH_ORDER[1])?;
    let third = prepare(FETCH_ORDER[2])?;

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
    // One sender per loop; when all are done, the collector ends by itself.
    let (n1, n2, n3) = (notes.clone(), notes.clone(), notes);
    let (a, b, c, ()) = tokio::join!(
        fetch_portal(first, &shared, n1),
        fetch_portal(second, &shared, n2),
        fetch_portal(third, &shared, n3),
        async {
            let mut done = 0;
            while let Some(note) = incoming.recv().await {
                match note {
                    Note::Event(event) => on_event(event),
                    Note::Done => {
                        done += 1;
                        on_event(FetchEvent::Progress { done, total });
                    }
                }
            }
        }
    );

    let mut completed = true;
    for result in [a, b, c] {
        let run = result?;
        completed &= run.completed;
        if let Some(counts) = run.counts {
            summary.per_portal.insert(run.portal, counts);
        }
    }
    Ok(completed)
}

/// One portal loop: strictly sequential, with all rules. An error stops the other portals
/// too - without a saved safety state nobody fetches any further.
async fn fetch_portal<F: PageFetcher, C: Fn() -> Timestamp>(
    work: (Portal, Vec<JobRow>, Option<F>),
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
    (portal, jobs, fetcher): (Portal, Vec<JobRow>, Option<F>),
    shared: &Shared<'_, C>,
    notes: &mpsc::UnboundedSender<Note>,
) -> crate::Result<(Option<PortalCounts>, bool)> {
    let Some(mut fetcher) = fetcher else {
        return Ok((None, true));
    };
    let (store, policy, cancel, clock) = (shared.store, shared.policy, shared.cancel, shared.clock);
    let mut counts = PortalCounts::default();
    let route = route(portal);
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
        let mut outcome =
            match access(&mut fetcher, policy, &link, route, cancel, clock, notes).await? {
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
                    until: until(clock(), NET_RETRY_DELAY),
                },
            );
            if !sleep_for(NET_RETRY_DELAY, cancel).await {
                return Ok((Some(counts), false));
            }
            outcome = match access(&mut fetcher, policy, &link, route, cancel, clock, notes).await?
            {
                Ok(PageOutcome::NetError { timeout: true, .. }) => {
                    PageOutcome::Throttled("no answer twice".into())
                }
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
            PageOutcome::Retry(reason) if retried == Some(index) => PageOutcome::Suspicious(reason),
            other => other,
        };

        let now = clock();
        lock(policy).record_done(portal, now);
        let stop_reason = match outcome {
            PageOutcome::Cancelled => {
                // The answer time counts for the next gap even after a cancellation.
                lock(policy).save()?;
                return Ok((Some(counts), false));
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
            } => {
                store.record_text(&job.key, &text, short, closed, now)?;
                // Only non-empty fields overwrite the mail heuristics: what the page hides
                // ("visible for EXPERT members") arrives empty.
                if let Some(f) = fields {
                    store.record_page_fields(&job.key, &f.title, &f.company, &f.location)?;
                }
                {
                    let mut policy = lock(policy);
                    // A page read in the session window confirms the sign-in; the guest
                    // route says nothing about it.
                    if route == Route::Session {
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
            PageOutcome::Suspicious(reason) => {
                let status = store.record_failed(&job.key, &reason, now)?;
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
                let mut policy = lock(policy);
                (policy.count_suspicious(portal) >= SUSPICIOUS_STREAK).then(|| {
                    StopReason::Breaker {
                        until: policy.pause_for(
                            portal,
                            PauseKind::Throttled,
                            PauseReason::LayoutChanged,
                            BREAKER_DETAIL,
                            now,
                        ),
                    }
                })
            }
            PageOutcome::LoginRequired(sign) => {
                log::info!("{}: sign-in needed ({sign})", portal.key());
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
                            return Ok((Some(counts), false));
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
                                    return Ok((Some(counts), false));
                                }
                                Login::NotSignedIn => Some(StopReason::LoginRequired),
                            }
                        }
                    }
                }
            }
            PageOutcome::Throttled(detail) => {
                let until = lock(policy).pause(portal, PauseKind::Throttled, &detail, now);
                Some(StopReason::Paused {
                    until,
                    reason: PauseReason::Throttled,
                    detail,
                })
            }
            PageOutcome::Blocked(detail) => {
                let until = lock(policy).pause(portal, PauseKind::Blocked, &detail, now);
                Some(StopReason::Paused {
                    until,
                    reason: PauseReason::Blocked,
                    detail,
                })
            }
            PageOutcome::NetError { detail, .. } => Some(StopReason::Network { detail }),
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
    Ok((Some(counts), true))
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
    route: Route,
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
            Ok(Ok(fetcher.fetch(link, route, cancel).await))
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
    log::info!("{}", reason.log_line(portal, skipped));
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
