//! Outcome matrix, pace, caps and pauses - one test per row. Time is simulated
//! (`start_paused`): waits cost no real time.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tokio::time::Instant;

use super::*;
use crate::fetch::policy::limits;
use crate::model::Posting;
use crate::portal::job_link;
use crate::store::MailRef;

/// One request: who, when it started, when it finished - the basis of the pace and overlap
/// checks.
#[derive(Clone, Debug)]
struct Call {
    id: String,
    portal: Portal,
    route: Route,
    start: Instant,
    end: Instant,
}

/// Fake fetcher: a sequence of outcomes per job id, otherwise a complete text.
///
/// Every portal gets its own fetch route; the copies share their memory so that a test sees
/// all requests in one place.
#[derive(Clone, Default)]
struct Fake {
    script: Arc<Mutex<HashMap<String, VecDeque<PageOutcome>>>>,
    calls: Arc<Mutex<Vec<Call>>>,
    /// Result of a sign-in (`None` = no sign-in possible).
    login: Option<Login>,
    /// Start and end of every sign-in.
    logins: Arc<Mutex<Vec<(Instant, Instant)>>>,
    /// Duration of every request (slow answers) or sign-in.
    delay: Duration,
    login_delay: Duration,
}

impl Fake {
    fn with(self, id: &str, outcomes: impl IntoIterator<Item = PageOutcome>) -> Fake {
        lock_test(&self.script).insert(id.into(), outcomes.into_iter().collect());
        self
    }

    /// All requests in the order they started.
    fn calls(&self) -> Vec<Call> {
        let mut calls = lock_test(&self.calls).clone();
        calls.sort_by_key(|call| call.start);
        calls
    }

    fn ids(&self) -> Vec<String> {
        self.calls().into_iter().map(|call| call.id).collect()
    }

    fn logins(&self) -> Vec<(Instant, Instant)> {
        lock_test(&self.logins).clone()
    }
}

fn lock_test<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl PageFetcher for Fake {
    async fn fetch(&mut self, link: &JobLink, route: Route, _: &CancellationToken) -> PageOutcome {
        let start = Instant::now();
        tokio::time::sleep(self.delay).await;
        let outcome = lock_test(&self.script)
            .get_mut(&link.key.id)
            .and_then(VecDeque::pop_front);
        lock_test(&self.calls).push(Call {
            id: link.key.id.clone(),
            portal: link.key.portal,
            route,
            start,
            end: Instant::now(),
        });
        outcome.unwrap_or_else(|| text(&"Vollständige Beschreibung. ".repeat(10)))
    }

    async fn login(&mut self, _: Portal, _: &CancellationToken) -> Login {
        let start = Instant::now();
        tokio::time::sleep(self.login_delay).await;
        lock_test(&self.logins).push((start, Instant::now()));
        self.login.unwrap_or(Login::NotSignedIn)
    }
}

fn text(t: &str) -> PageOutcome {
    PageOutcome::Text {
        text: t.into(),
        short: t.chars().count() < MIN_TEXT_CHARS,
        closed: false,
        fields: None,
    }
}

fn suspicious() -> PageOutcome {
    PageOutcome::Suspicious("no description".into())
}

fn base() -> Timestamp {
    "2026-09-19T08:00:00Z".parse().unwrap()
}

/// Clock that runs with the simulated tokio time.
fn clock() -> impl Fn() -> Timestamp {
    let start = Instant::now();
    move || base() + SignedDuration::try_from(start.elapsed()).unwrap()
}

fn url(portal: Portal, id: u64) -> String {
    match portal {
        Portal::LinkedIn => format!("https://www.linkedin.com/jobs/view/{id}/"),
        Portal::Freelancermap => format!("https://www.freelancermap.de/nproj/{id}.html"),
        Portal::FreelanceDe => format!("https://www.freelance.de/project/index.php?id={id}"),
    }
}

/// Creates jobs (mail `days_ago` days before `base()`).
fn store_with(jobs: &[(Portal, u64, i64)]) -> Store {
    let store = Store::in_memory().unwrap();
    let run = store.begin_run().unwrap();
    for &(portal, id, days_ago) in jobs {
        let link = job_link(&url(portal, id)).unwrap();
        let posting = Posting::new(link.key, link.url, "Titel aus der Mail", "Mailfirma", "");
        let date = base() - SignedDuration::from_hours(days_ago * 24);
        let mail = MailRef {
            subject: "Neue Jobs",
            date: Some(date),
            gmail_id: None,
        };
        store.upsert_posting(run, &posting, mail, date).unwrap();
    }
    store
}

fn key(portal: Portal, id: u64) -> JobKey {
    job_link(&url(portal, id)).unwrap().key
}

struct Run {
    summary: FetchSummary,
    completed: bool,
    stops: Vec<(Portal, StopReason, usize)>,
    /// Waits that were announced beforehand (portal, end).
    waits: Vec<(Portal, Timestamp)>,
}

async fn run(
    fake: &Fake,
    store: &Store,
    policy: &mut Policy,
    selection: Selection<'_>,
    clock: &impl Fn() -> Timestamp,
) -> Run {
    run_with(
        fake,
        store,
        policy,
        selection,
        clock,
        &CancellationToken::new(),
    )
    .await
}

async fn run_with(
    fake: &Fake,
    store: &Store,
    policy: &mut Policy,
    selection: Selection<'_>,
    clock: &impl Fn() -> Timestamp,
    cancel: &CancellationToken,
) -> Run {
    run_inner(fake, store, policy, selection, clock, cancel).await
}

async fn run_inner(
    fake: &Fake,
    store: &Store,
    policy: &mut Policy,
    selection: Selection<'_>,
    clock: &impl Fn() -> Timestamp,
    cancel: &CancellationToken,
) -> Run {
    let mut summary = FetchSummary::default();
    let mut stops = Vec::new();
    let mut waits = Vec::new();
    let shared = Mutex::new(std::mem::take(policy));
    let completed = fetch_all(
        |_portal| Ok(fake.clone()),
        store,
        &shared,
        selection,
        cancel,
        clock,
        &mut summary,
        |e| match e {
            FetchEvent::PortalStopped {
                portal,
                reason,
                skipped,
            } => {
                stops.push((portal, reason, skipped));
            }
            FetchEvent::Waiting { portal, until } => waits.push((portal, until)),
            _ => {}
        },
    )
    .await
    .unwrap();
    *policy = shared.into_inner().unwrap_or_else(PoisonError::into_inner);
    // The stop is in the summary too.
    for (portal, reason, _) in &stops {
        let counts = &summary.per_portal[portal];
        assert_eq!(counts.stop.as_ref(), Some(reason));
    }
    Run {
        summary,
        completed,
        stops,
        waits,
    }
}

/// Compare ids regardless of order (portals run side by side).
fn sorted(ids: &[String]) -> Vec<String> {
    let mut ids = ids.to_vec();
    ids.sort();
    ids
}

const FM: Portal = Portal::Freelancermap;
const LI: Portal = Portal::LinkedIn;
const FL: Portal = Portal::FreelanceDe;

/// The router has no choice any more: a session window exists only where nothing can be
/// read without a sign-in. freelancermap signed in is character-identical to the guest
/// view (measured), LinkedIn too - both go as a guest.
#[test]
fn only_a_portal_that_needs_an_account_uses_a_window() {
    assert_eq!(route(LI), Route::Http);
    assert_eq!(route(FM), Route::Http);
    assert_eq!(route(FL), Route::Session);
}

#[tokio::test(start_paused = true)]
async fn matrix_text_short_closed_gone_suspicious() {
    let c = clock();
    let store = store_with(&[
        (FM, 10_001, 1),
        (FM, 10_002, 1),
        (FM, 10_003, 1),
        (FM, 10_004, 1),
        (FM, 10_005, 1),
    ]);
    let fake = Fake::default()
        .with("10002", [text("Kurz, aber echt.")])
        .with(
            "10003",
            [PageOutcome::Text {
                text: "Geschlossen, Text bleibt. ".repeat(5),
                short: false,
                closed: true,
                fields: Some(PageFields {
                    title: "Titel der Seite".into(),
                    company: "Seitenfirma".into(),
                    location: "Köln".into(),
                }),
            }],
        )
        .with("10004", [PageOutcome::Gone])
        .with("10005", [suspicious()]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert!(r.completed);
    let n = &r.summary.per_portal[&FM];
    assert_eq!((n.ok, n.short, n.closed, n.gone, n.failed), (3, 1, 1, 1, 1));
    let job = |id| store.job(&key(FM, id)).unwrap().unwrap();
    assert_eq!(job(10_001).desc_status, DescStatus::Ok);
    assert!(job(10_002).desc_short);
    assert!(job(10_003).desc_closed);
    // Page fields replace the mail heuristics.
    assert_eq!(
        (
            job(10_003).title.as_str(),
            job(10_003).company.as_str(),
            job(10_003).location.as_str()
        ),
        ("Titel der Seite", "Seitenfirma", "Köln")
    );
    assert_eq!(job(10_004).desc_status, DescStatus::Gone);
    assert_eq!(
        (job(10_005).desc_status, job(10_005).desc_attempts),
        (DescStatus::Failed, 1)
    );
    // Every request counts.
    assert_eq!(policy.state(FM).accesses.len(), 5);
    // Second run: success is never fetched again, failures only after 12 h.
    let calls = fake.calls().len();
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.calls().len(), calls);
}

#[tokio::test(start_paused = true)]
async fn two_suspicious_pages_in_a_row_trip_the_breaker() {
    let c = clock();
    let store = store_with(&[
        (LI, 4_000_000_001, 1),
        (LI, 4_000_000_002, 2),
        (LI, 4_000_000_003, 3),
        (LI, 4_000_000_004, 4),
    ]);
    let fake = Fake::default()
        .with("4000000001", [suspicious()])
        .with("4000000002", [suspicious()]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.ids(), ["4000000001", "4000000002"]);
    assert_eq!(r.summary.per_portal[&LI].skipped, 2);
    assert!(matches!(
        r.stops.as_slice(),
        [(LI, StopReason::Breaker { .. }, 2)]
    ));
    // The breaker pauses the portal for an hour - clicking again circumvents nothing.
    let paused = policy.state(LI);
    assert_eq!(paused.pause_kind, Some(PauseKind::Throttled));
    assert_eq!(
        paused.paused_until.unwrap().duration_since(c()).as_hours(),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn the_breaker_counts_across_runs() {
    let c = clock();
    let store = store_with(&[(LI, 4_000_000_001, 1), (LI, 4_000_000_002, 2)]);
    let fake = Fake::default()
        .with("4000000001", [suspicious()])
        .with("4000000002", [suspicious()]);
    let mut policy = Policy::in_memory();
    // "Fetch details" for one job each: two clicks, two suspicious pages - then stop.
    for (id, stopped) in [(4_000_000_001, false), (4_000_000_002, true)] {
        let keys = [key(LI, id)];
        let r = run(
            &fake,
            &store,
            &mut policy,
            Selection::Jobs(&keys, &Portal::ALL),
            &c,
        )
        .await;
        let breaker = r
            .stops
            .iter()
            .any(|s| matches!(s.1, StopReason::Breaker { .. }));
        assert_eq!(breaker, stopped);
    }
    assert!(matches!(
        policy.allowance(LI, c()),
        Allowance::Paused { .. }
    ));
    // A complete text resets the counter.
    tokio::time::advance(Duration::from_secs(2 * 3600)).await;
    let store = store_with(&[(LI, 4_000_000_003, 1)]);
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(policy.state(LI).suspicious_streak, 0);
}

#[tokio::test(start_paused = true)]
async fn a_full_text_between_resets_the_breaker_but_a_short_one_does_not() {
    let c = clock();
    let store = store_with(&[
        (LI, 4_000_000_001, 1),
        (LI, 4_000_000_002, 2),
        (LI, 4_000_000_003, 3),
        (LI, 4_000_000_004, 4),
    ]);
    let fake = Fake::default()
        .with("4000000001", [suspicious()])
        .with("4000000003", [suspicious()]);
    let mut policy = Policy::in_memory();
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.calls().len(), 4);

    let store = store_with(&[
        (LI, 4_000_000_001, 1),
        (LI, 4_000_000_002, 2),
        (LI, 4_000_000_003, 3),
    ]);
    let fake = Fake::default()
        .with("4000000001", [suspicious()])
        .with("4000000002", [text("kurz")])
        .with("4000000003", [suspicious()]);
    let r = run(
        &fake,
        &store,
        &mut Policy::in_memory(),
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert!(matches!(
        r.stops.as_slice(),
        [(LI, StopReason::Breaker { .. }, 0)]
    ));
}

#[tokio::test(start_paused = true)]
async fn throttle_pauses_the_portal_across_runs_without_costing_attempts() {
    let c = clock();
    let store = store_with(&[(FM, 10_001, 1), (FM, 10_002, 2), (LI, 4_000_000_001, 1)]);
    let fake = Fake::default().with("10001", [PageOutcome::Throttled("HTTP 429".into())]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    let until = base() + SignedDuration::from_hours(1);
    assert!(matches!(&r.stops[0], (FM, StopReason::Paused { until: u, .. }, 2) if *u >= until));
    // LinkedIn continues independently.
    assert_eq!(sorted(&fake.ids()), ["10001", "4000000001"]);
    let job = store.job(&key(FM, 10_001)).unwrap().unwrap();
    assert_eq!(
        (job.desc_status, job.desc_attempts),
        (DescStatus::Missing, 0)
    );
    // New run right after: not a single request to the paused portal.
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.calls().len(), 2);
    assert_eq!(r.summary.per_portal[&FM].skipped, 2);
}

#[tokio::test(start_paused = true)]
async fn block_pauses_a_day_and_a_second_block_a_week() {
    let c = clock();
    let store = store_with(&[(LI, 4_000_000_001, 1)]);
    let fake = Fake::default().with(
        "4000000001",
        [
            PageOutcome::Blocked("HTTP 999".into()),
            PageOutcome::Blocked("HTTP 999".into()),
        ],
    );
    let mut policy = Policy::in_memory();
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    let first = policy.state(LI).paused_until.unwrap();
    assert_eq!(first.duration_since(base()).as_hours(), 24);
    // After expiry (simulated 25 h later) the second block signal: 7 days.
    tokio::time::advance(Duration::from_secs(25 * 3600)).await;
    let later = c();
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    let second = policy.state(LI).paused_until.unwrap();
    assert_eq!(second.duration_since(later).as_hours(), 7 * 24);
}

/// After a 7-day pause the next block signal must not count as the first again (otherwise
/// 24 h, 7 d, 24 h, 7 d ...).
#[test]
fn a_block_right_after_the_week_long_pause_stays_a_week() {
    let mut policy = Policy::in_memory();
    let mut now = base();
    let mut lengths = Vec::new();
    for _ in 0..4 {
        let until = policy.pause(LI, PauseKind::Blocked, "HTTP 999", now);
        lengths.push(until.duration_since(now).as_hours());
        // The first request after the pause is blocked again.
        now = until + SignedDuration::from_mins(1);
    }
    assert_eq!(lengths, [24, 7 * 24, 7 * 24, 7 * 24]);
    // Seven quiet days after the end of the pause: the short pause again.
    let quiet = now + SignedDuration::from_hours(8 * 24);
    let until = policy.pause(LI, PauseKind::Blocked, "HTTP 999", quiet);
    assert_eq!(until.duration_since(quiet).as_hours(), 24);
}

#[tokio::test(start_paused = true)]
async fn network_error_is_retried_once_after_30_seconds() {
    let c = clock();
    let store = store_with(&[(FM, 10_001, 1), (FM, 10_002, 1)]);
    let fake = Fake::default()
        .with(
            "10001",
            [PageOutcome::NetError {
                timeout: false,
                detail: "no connection".into(),
            }],
        )
        .with(
            "10002",
            [
                PageOutcome::NetError {
                    timeout: true,
                    detail: "no answer".into(),
                },
                PageOutcome::NetError {
                    timeout: true,
                    detail: "no answer".into(),
                },
            ],
        );
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.ids(), ["10001", "10001", "10002", "10002"]);
    let calls = fake.calls();
    assert!(calls[1].start - calls[0].start >= NET_RETRY_DELAY);
    assert_eq!(
        store.job(&key(FM, 10_001)).unwrap().unwrap().desc_status,
        DescStatus::Ok
    );
    // No answer twice = throttle: 60 minutes pause, job unchanged.
    assert!(matches!(r.stops[..], [(FM, StopReason::Paused { .. }, 1)]));
    assert_eq!(
        store.job(&key(FM, 10_002)).unwrap().unwrap().desc_attempts,
        0
    );
    assert_eq!(policy.state(FM).accesses.len(), 4);
}

#[tokio::test(start_paused = true)]
async fn pace_is_kept_within_and_across_runs() {
    let c = clock();
    let store = store_with(&[
        (LI, 4_000_000_001, 1),
        (LI, 4_000_000_002, 2),
        (LI, 4_000_000_003, 3),
    ]);
    let fake = Fake::default();
    let mut policy = Policy::in_memory();
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    for pair in fake.calls().windows(2) {
        let gap = pair[1].start - pair[0].start;
        assert!(
            (Duration::from_secs(4)..=Duration::from_secs(7)).contains(&gap),
            "{gap:?}"
        );
    }
    // Clicked again right away: the first request of the new run keeps the gap too.
    let store2 = store_with(&[(LI, 4_000_000_009, 1)]);
    let last = fake.calls().last().unwrap().start;
    run(
        &fake,
        &store2,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert!(fake.calls().last().unwrap().start - last >= Duration::from_secs(4));
}

#[tokio::test(start_paused = true)]
async fn hourly_cap_stops_with_the_next_possible_time() {
    let c = clock();
    let jobs: Vec<(Portal, u64, i64)> = (1..=21).map(|i| (LI, 4_000_000_000 + i, 1)).collect();
    let store = store_with(&jobs);
    let fake = Fake::default();
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.calls().len(), 20);
    assert_eq!(r.summary.per_portal[&LI].skipped, 1);
    assert!(
        matches!(&r.stops[..], [(LI, StopReason::Quota { next_at }, 1)] if *next_at > base() + SignedDuration::from_mins(59))
    );
}

#[tokio::test(start_paused = true)]
async fn login_required_stops_freelance_and_marks_the_session() {
    let c = clock();
    let store = store_with(&[(FL, 1_255_067, 1), (FL, 1_255_068, 1)]);
    let fake = Fake::default().with("1255067", [PageOutcome::LoginRequired("Teaser".into())]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert!(matches!(
        r.stops.as_slice(),
        [(FL, StopReason::LoginRequired, 2)]
    ));
    assert_eq!(r.summary.per_portal[&FL].skipped, 2);
    assert!(policy.state(FL).login_needed);
    assert_eq!(
        store
            .job(&key(FL, 1_255_067))
            .unwrap()
            .unwrap()
            .desc_attempts,
        0
    );
    // Success confirms the session.
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert!(!policy.state(FL).login_needed);
    assert!(policy.state(FL).session_confirmed_at.is_some());
}

/// All chosen portals get their turn (side by side, hence without a fixed order); portals
/// not chosen are not touched.
#[tokio::test(start_paused = true)]
async fn every_chosen_portal_is_fetched_and_no_other() {
    let c = clock();
    let store = store_with(&[(FL, 1_255_067, 1), (LI, 4_000_000_001, 1), (FM, 10_001, 1)]);
    let fake = Fake::default();
    let r = run(
        &fake,
        &store,
        &mut Policy::in_memory(),
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(sorted(&fake.ids()), ["10001", "1255067", "4000000001"]);
    assert!(r.stops.is_empty());
    // Portals not chosen are not touched.
    let store = store_with(&[(LI, 4_000_000_002, 1), (FM, 10_002, 1)]);
    let fake = Fake::default();
    run(
        &fake,
        &store,
        &mut Policy::in_memory(),
        Selection::Queue(&[LI]),
        &c,
    )
    .await;
    assert_eq!(fake.ids(), ["4000000002"]);
}

#[tokio::test(start_paused = true)]
async fn only_recent_mails_automatically_but_any_job_on_request() {
    let c = clock();
    let store = store_with(&[(FM, 10_001, 40), (FM, 10_002, 1)]);
    let fake = Fake::default();
    let mut policy = Policy::in_memory();
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.ids(), ["10002"]);
    let keys = [key(FM, 10_001), key(FM, 10_002), key(FM, 10_001)];
    run(
        &fake,
        &store,
        &mut policy,
        Selection::Jobs(&keys, &Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(
        fake.ids(),
        ["10002", "10001"],
        "an older job on request, a fetched one never again, a duplicate once"
    );
}

/// A cancellation in the middle of a request is no page finding: the job stays open (no
/// failed attempt, no error) and the breaker does not count.
#[tokio::test(start_paused = true)]
async fn a_cancelled_page_is_no_failed_job() {
    let c = clock();
    let store = store_with(&[(LI, 4_000_000_001, 1), (LI, 4_000_000_002, 1)]);
    let fake = Fake::default().with("4000000001", [PageOutcome::Cancelled]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert!(!r.completed);
    assert!(r.stops.is_empty());
    let n = &r.summary.per_portal[&LI];
    assert_eq!((n.ok, n.failed, n.skipped), (0, 0, 0));
    let job = store.job(&key(LI, 4_000_000_001)).unwrap().unwrap();
    assert_eq!(job.desc_status, DescStatus::Missing);
    assert_eq!((job.desc_attempts, job.desc_error), (0, None));
    assert_eq!(policy.state(LI).suspicious_streak, 0);
}

#[tokio::test(start_paused = true)]
async fn cancel_during_the_pause_ends_at_once() {
    let c = clock();
    let store = store_with(&[(LI, 4_000_000_001, 1), (LI, 4_000_000_002, 1)]);
    let fake = Fake::default();
    let cancel = CancellationToken::new();
    let started = Instant::now();
    let mut policy = Policy::in_memory();
    let (r, ()) = tokio::join!(
        run_with(
            &fake,
            &store,
            &mut policy,
            Selection::Queue(&Portal::ALL),
            &c,
            &cancel
        ),
        async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            cancel.cancel();
        }
    );
    assert!(!r.completed);
    assert_eq!(fake.calls().len(), 1);
    assert!(started.elapsed() < Duration::from_secs(2));
    // The cancelled request was not counted.
    assert_eq!(policy.state(LI).accesses.len(), 1);
}

/// The rules check pause and cap before the gap; a cancellation during the wait counts
/// nothing.
#[tokio::test(start_paused = true)]
async fn admit_checks_the_pause_first_and_counts_only_real_accesses() {
    let c = clock();
    let policy = Mutex::new(Policy::in_memory());
    lock(&policy).record_access(LI, c());
    lock(&policy).pause(LI, PauseKind::Throttled, "HTTP 429", c());
    let started = Instant::now();
    let admission = admit(&policy, LI, &CancellationToken::new(), &c, |_| {
        panic!("a paused portal does not wait")
    })
    .await
    .unwrap();
    assert!(matches!(
        admission,
        Admission::Stop(StopReason::Paused { .. })
    ));
    assert_eq!(started.elapsed(), Duration::ZERO);
    assert_eq!(lock(&policy).state(LI).accesses.len(), 1, "nothing counted");

    let policy = Mutex::new(Policy::in_memory());
    lock(&policy).record_access(FM, c());
    let cancel = CancellationToken::new();
    let (admission, ()) = tokio::join!(admit(&policy, FM, &cancel, &c, |_| {}), async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        cancel.cancel();
    });
    assert_eq!(admission.unwrap(), Admission::Cancelled);
    assert_eq!(lock(&policy).state(FM).accesses.len(), 1);
}

/// Sign-in during the run: if it succeeds, the same job is fetched again and the run goes
/// on; at most one sign-in per run. Sign-in page and retry keep the gap.
#[tokio::test(start_paused = true)]
async fn login_during_the_run_retries_the_same_job() {
    let c = clock();
    let store = store_with(&[(FL, 1_255_067, 1), (FL, 1_255_068, 1)]);
    let fake = Fake {
        login: Some(Login::SignedIn),
        login_delay: Duration::from_secs(60),
        ..Fake::default()
    }
    .with("1255067", [PageOutcome::LoginRequired("Teaser".into())]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.logins().len(), 1);
    assert_eq!(fake.ids(), ["1255067", "1255067", "1255068"]);
    assert_eq!(r.summary.per_portal[&FL].ok, 2);
    assert!(!policy.state(FL).login_needed);
    // Every request counts - the first one that led to the sign-in and the sign-in page too.
    assert_eq!(policy.state(FL).accesses.len(), 4);
    let (login_start, login_end) = fake.logins()[0];
    let calls = fake.calls();
    let before = login_start - calls[0].start;
    let after = calls[1].start - login_end;
    assert!(
        before >= Duration::from_secs(10),
        "before the sign-in only {before:?}"
    );
    assert!(
        after >= Duration::from_secs(10),
        "after the sign-in only {after:?}"
    );

    // Refused/cancelled: the portal stops, no second sign-in.
    let store = store_with(&[(FL, 1_255_067, 1), (FL, 1_255_068, 1)]);
    let fake = Fake {
        login: Some(Login::NotSignedIn),
        ..Fake::default()
    }
    .with("1255067", [PageOutcome::LoginRequired("Teaser".into())]);
    let r = run(
        &fake,
        &store,
        &mut Policy::in_memory(),
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!((fake.logins().len(), fake.calls().len()), (1, 1));
    assert_eq!(r.summary.per_portal[&FL].skipped, 2);
}

/// Security check at the sign-in: the session counts, but the portal rests for this run -
/// with its own reason, not "sign-in needed".
#[tokio::test(start_paused = true)]
async fn a_challenged_login_keeps_the_session_but_rests_the_portal() {
    let c = clock();
    let store = store_with(&[(FL, 1_255_067, 1), (FL, 1_255_068, 1)]);
    let fake = Fake {
        login: Some(Login::Challenged),
        ..Fake::default()
    }
    .with("1255067", [PageOutcome::LoginRequired("Teaser".into())]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.ids(), ["1255067"], "no further request in this run");
    assert!(matches!(
        r.stops.as_slice(),
        [(FL, StopReason::Challenged, 2)]
    ));
    let state = policy.state(FL);
    assert!(!state.login_needed && state.session_confirmed_at.is_some());
    assert_eq!(state.accesses.len(), 2, "page and sign-in page");
    assert_eq!(
        r.summary.per_portal[&FL]
            .stop
            .as_ref()
            .map(StopReason::health),
        Some(crate::fetch::PortalHealth::Paused {
            until: None,
            reason: crate::fetch::policy::PauseReason::Challenged
        })
    );
}

/// The gap counts from the answer and across runs - after a slow answer the first request
/// of the next run does not go out right away.
#[tokio::test(start_paused = true)]
async fn the_pace_counts_from_the_answer_across_runs() {
    let c = clock();
    let fake = Fake {
        delay: Duration::from_secs(30),
        ..Fake::default()
    };
    let mut policy = Policy::in_memory();
    for id in [4_000_000_001, 4_000_000_002] {
        let store = store_with(&[(LI, id, 1)]);
        run(
            &fake,
            &store,
            &mut policy,
            Selection::Queue(&Portal::ALL),
            &c,
        )
        .await;
    }
    let calls = fake.calls();
    let gap = calls[1].start - calls[0].start;
    assert!(gap >= Duration::from_secs(34), "gap only {gap:?}");
}

/// The sign-in page during the run is a counted request and keeps the cap.
#[tokio::test(start_paused = true)]
async fn the_login_page_counts_and_respects_the_cap() {
    let c = clock();
    let store = store_with(&[(FL, 1_255_067, 1)]);
    let mut policy = Policy::in_memory();
    // 14 of 15 requests of the hour are used; the page request is the 15th.
    for minutes in 1..=14 {
        policy.record_access(FL, base() - SignedDuration::from_mins(minutes));
    }
    let fake = Fake {
        login: Some(Login::SignedIn),
        ..Fake::default()
    }
    .with("1255067", [PageOutcome::LoginRequired("Teaser".into())]);
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert!(fake.logins().is_empty(), "no sign-in page above the cap");
    assert!(matches!(
        r.stops.as_slice(),
        [(FL, StopReason::Quota { .. }, _)]
    ));
    assert_eq!(policy.state(FL).accesses.len(), 15);
}

/// The redirect after the sign-in repeats the page as its own counted request - at most
/// once per job.
#[tokio::test(start_paused = true)]
async fn a_retry_is_a_counted_access_and_happens_once() {
    let c = clock();
    let store = store_with(&[(FL, 1_255_067, 1), (FL, 1_255_068, 1)]);
    let retry = || PageOutcome::Retry("Weiterleitung nach der Anmeldung".into());
    let fake = Fake::default()
        .with("1255067", [retry()])
        .with("1255068", [retry(), retry()]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.ids(), ["1255067", "1255067", "1255068", "1255068"]);
    assert_eq!(policy.state(FL).accesses.len(), 4);
    assert_eq!(r.summary.per_portal[&FL].ok, 1);
    assert_eq!(
        r.summary.per_portal[&FL].failed, 1,
        "second redirect: suspicious"
    );
    assert_eq!(policy.state(FL).suspicious_streak, 1);
    // The retry keeps the gap too.
    let calls = fake.calls();
    let gap = calls[1].start - calls[0].start;
    assert!(gap >= Duration::from_secs(10), "gap only {gap:?}");
}

/// If every page redirects twice (e.g. a forced interstitial), the breaker applies after two
/// jobs - instead of using up the quota.
#[tokio::test(start_paused = true)]
async fn repeated_redirects_trip_the_breaker() {
    let c = clock();
    let store = store_with(&[(FL, 1_255_067, 1), (FL, 1_255_068, 1), (FL, 1_255_069, 1)]);
    let retry = || PageOutcome::Retry("Weiterleitung nach der Anmeldung".into());
    let fake = Fake::default()
        .with("1255067", [retry(), retry()])
        .with("1255068", [retry(), retry()])
        .with("1255069", [retry(), retry()]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    assert_eq!(fake.calls().len(), 4);
    assert_eq!(policy.state(FL).accesses.len(), 4);
    assert!(matches!(
        r.stops.as_slice(),
        [(FL, StopReason::Breaker { .. }, 1)]
    ));
    assert!(matches!(
        policy.allowance(FL, c()),
        Allowance::Paused { .. }
    ));
}

/// Longer waits are announced beforehand (the interface shows a countdown).
#[tokio::test(start_paused = true)]
async fn long_waits_are_announced() {
    let c = clock();
    let store = store_with(&[(LI, 4_000_000_001, 1), (LI, 4_000_000_002, 2)]);
    let fake = Fake::default().with(
        "4000000001",
        [PageOutcome::NetError {
            timeout: false,
            detail: "no connection".into(),
        }],
    );
    let r = run(
        &fake,
        &store,
        &mut Policy::in_memory(),
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    // Retry after the network error (30 s), then the gap to the second job.
    assert_eq!(r.waits.len(), 2, "{:?}", r.waits);
    assert!(r.waits.iter().all(|(portal, _)| *portal == LI));
    let retry_at = r.waits[0].1;
    assert_eq!(retry_at, base() + SignedDuration::from_secs(30));
    let pace = r.waits[1].1.duration_since(retry_at);
    assert!(
        (SignedDuration::from_secs(4)..=SignedDuration::from_secs(7)).contains(&pace),
        "{pace:?}"
    );
}

/// Safety invariant: within a portal two requests never overlap and the gap never drops
/// below the pace - two different portals, however, really run side by side.
#[tokio::test(start_paused = true)]
async fn portals_run_side_by_side_but_never_overlap_within_one() {
    let c = clock();
    let jobs: Vec<(Portal, u64, i64)> = (1..=3)
        .flat_map(|i| {
            [
                (FM, 10_000 + i, 1),
                (LI, 4_000_000_000 + i, 1),
                (FL, 1_255_000 + i, 1),
            ]
        })
        .collect();
    let store = store_with(&jobs);
    let fake = Fake {
        delay: Duration::from_secs(3),
        ..Fake::default()
    };
    run(
        &fake,
        &store,
        &mut Policy::in_memory(),
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    let calls = fake.calls();
    assert_eq!(calls.len(), 9);
    for portal in Portal::ALL {
        let own: Vec<&Call> = calls.iter().filter(|call| call.portal == portal).collect();
        assert_eq!(own.len(), 3, "{portal}");
        let pace = Duration::from_millis(*limits(portal).pace_ms.start());
        for pair in own.windows(2) {
            assert!(
                pair[0].end <= pair[1].start,
                "{portal}: two requests at once"
            );
            let gap = pair[1].start - pair[0].end;
            assert!(gap >= pace, "{portal}: gap only {gap:?}");
        }
    }
    // Different portals, however, at the same time.
    let first = |portal: Portal| {
        calls
            .iter()
            .find(|call| call.portal == portal)
            .expect("request")
            .clone()
    };
    let (fm, li) = (first(FM), first(LI));
    assert!(
        li.start < fm.end && fm.start < li.end,
        "portals run one after the other instead of side by side"
    );
}

/// Safety invariant: caps, pauses and breaker also apply when running in parallel and on
/// the session route - here one of the three brakes per portal at the same time.
#[tokio::test(start_paused = true)]
async fn limits_pauses_and_the_breaker_hold_in_parallel_and_via_a_session() {
    let c = clock();
    let mut jobs: Vec<(Portal, u64, i64)> = (1..=21).map(|i| (LI, 4_000_000_000 + i, 1)).collect();
    jobs.extend([(FM, 10_001, 1), (FM, 10_002, 1), (FM, 10_003, 1)]);
    jobs.extend([(FL, 1_255_001, 1), (FL, 1_255_002, 1)]);
    let store = store_with(&jobs);
    let fake = Fake::default()
        .with("10001", [suspicious()])
        .with("10002", [suspicious()])
        .with("1255001", [PageOutcome::Blocked("HTTP 403".into())]);
    let mut policy = Policy::in_memory();
    let r = run(
        &fake,
        &store,
        &mut policy,
        Selection::Queue(&Portal::ALL),
        &c,
    )
    .await;
    let calls = fake.calls();
    let of = |portal: Portal| -> Vec<&Call> {
        calls.iter().filter(|call| call.portal == portal).collect()
    };
    // Guest route for LinkedIn and freelancermap, session route only for freelance.de.
    assert!(of(LI).iter().all(|call| call.route == Route::Http));
    assert!(of(FM).iter().all(|call| call.route == Route::Http));
    assert!(of(FL).iter().all(|call| call.route == Route::Session));
    // Hourly cap LinkedIn: 20 requests, the rest waits.
    assert_eq!(of(LI).len(), 20);
    // Breaker freelancermap: stop after two pages without a description.
    assert_eq!(of(FM).len(), 2);
    assert!(matches!(
        policy.allowance(FM, c()),
        Allowance::Paused { .. }
    ));
    // Block signal freelance.de: pause instead of retry.
    assert_eq!(of(FL).len(), 1);
    assert!(matches!(
        policy.allowance(FL, c()),
        Allowance::Paused { .. }
    ));
    let mut stopped: Vec<Portal> = r.stops.iter().map(|(portal, ..)| *portal).collect();
    stopped.sort_unstable();
    assert_eq!(stopped, Portal::ALL);
}
