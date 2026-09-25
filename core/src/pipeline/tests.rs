//! Runs with the dry-run dummies; simulated time (pace costs nothing).

use std::path::Path;
use std::sync::Mutex;

use jiff::SignedDuration;
use tokio::time::Instant;

use super::demo::{DemoBackends, DemoMail, DemoPages};
use super::*;
use crate::error::ErrorKind;
use crate::export::TXT_DIR;
use crate::fetch::policy::PauseReason;
use crate::model::{DescStatus, Place};

fn clock() -> impl Fn() -> Timestamp {
    let base = Timestamp::now();
    let start = Instant::now();
    move || base + SignedDuration::try_from(start.elapsed()).unwrap()
}

fn request() -> RunRequest {
    RunRequest {
        kind: RunKind::Fetch,
    }
}

fn ctx(workspace: &Path, dry_run: bool) -> RunContext {
    RunContext {
        workspace: workspace.to_path_buf(),
        dry_run,
        portals: Portal::ALL.to_vec(),
        fetch_portals: Portal::ALL.to_vec(),
        sign_in: vec![Portal::FreelanceDe],
        auto_archive_days: 0,
        auto_empty_trash_days: 0,
        language: Language::De,
    }
}

/// Mailbox only: no portal may fetch details.
fn scan_only(workspace: &Path) -> RunContext {
    RunContext {
        fetch_portals: Vec::new(),
        ..ctx(workspace, false)
    }
}

fn txt_files(workspace: &Path) -> usize {
    std::fs::read_dir(workspace.join(RESULT_DIR).join(TXT_DIR)).map_or(0, Iterator::count)
}

async fn go<B: Backends>(
    backends: &mut B,
    store: &Store,
    request: &RunRequest,
    ctx: &RunContext,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
) -> (RunSummary, Vec<RunEvent>) {
    let mut events = Vec::new();
    let policy = Mutex::new(Policy::in_memory());
    let summary = run(backends, store, &policy, request, ctx, cancel, clock, |e| {
        events.push(e);
    })
    .await;
    (summary, events)
}

fn finished(events: &[RunEvent]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, RunEvent::Finished { .. }))
        .count()
}

/// Every event stays below 8 KB (bigger ones the Tauri channel parks in a queue that any
/// page could fetch).
fn assert_small(events: &[RunEvent]) {
    for event in events {
        let size = serde_json::to_vec(event).unwrap().len();
        assert!(size < 8 * 1024, "{size} bytes: {event:?}");
    }
}

#[tokio::test(start_paused = true)]
#[expect(clippy::too_many_lines, reason = "one run, checked from every side")]
async fn one_click_run_writes_everything_and_finishes_once() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, events) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.outcome, Outcome::Completed);
    assert_eq!(s.kind, RunKindName::Fetch);
    let scan = s.scan.as_ref().unwrap();
    assert_eq!((scan.alert_mails, scan.new), (3, 5));
    let fetch = s.fetch.as_ref().unwrap();
    let ok: usize = fetch.per_portal.values().map(|p| p.ok).sum();
    assert_eq!(ok, 4, "freelance.de shows a portal stop in the dry run");
    let freelance = &fetch.per_portal[&Portal::FreelanceDe];
    assert_eq!(freelance.skipped, 1);
    let paused = PortalHealth::Paused {
        until: None,
        reason: PauseReason::Throttled,
    };
    let health = freelance.stop.as_ref().unwrap().health();
    assert!(
        matches!(
            health,
            PortalHealth::Paused {
                until: Some(_),
                reason: PauseReason::Throttled
            }
        ),
        "{health:?} vs {paused:?}"
    );
    // The per-portal summary: scan and fetch counts side by side.
    let of = |portal| s.per_portal.iter().find(|p| p.portal == portal).unwrap();
    let li = of(Portal::LinkedIn);
    assert_eq!((li.new, li.known, li.dup, li.fetched), (2, 0, 0, 2));
    assert_eq!(li.stopped, None);
    let fl = of(Portal::FreelanceDe);
    assert_eq!((fl.new, fl.fetched, fl.skipped), (1, 0, 1));
    assert_eq!(fl.stopped, Some(health));
    assert!(s.empty_alerts.is_empty());
    // The real engine with the sample profile: scored at the details (high, mid, low,
    // excluded), the job without details in the catch-up.
    assert_eq!(
        s.score,
        Some(ScoreSummary {
            scored: 3,
            excluded: 1,
            unscorable: 1,
            pending: 0,
            best: Some(100)
        })
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            RunEvent::JobUpdated { job, .. } if job.match_.as_ref().is_some_and(|m| m.score == 100)
        )),
        "the ring appears with the details"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        RunEvent::Progress {
            step: Step::Score,
            done: 1,
            total: 1,
            ..
        }
    )));
    let export = s.export.as_ref().unwrap();
    assert_eq!(export.txt_written, 4);
    assert!(export.overview_xlsx.as_ref().unwrap().exists());
    // The HTML overview: the unread scored jobs, the excluded one not.
    let html = std::fs::read_to_string(export.overview_html.as_ref().unwrap()).unwrap();
    assert!(html.contains("Interim CFO") && !html.contains("Projektleiter S/4HANA"));
    assert!(!html.contains("Beispielanzeige"), "never the full text");
    assert_eq!(txt_files(dir.path()), 4);
    assert_eq!(finished(&events), 1);
    assert_small(&events);
    assert!(last_run(&store).unwrap().is_some_and(|l| l.run == s.run));
    // The interface sees waits (countdown), every phase and the portal stop as codes.
    let statuses: Vec<(StatusCode, Option<Portal>, bool)> = events
        .iter()
        .filter_map(|e| match e {
            RunEvent::Status {
                code,
                portal,
                until,
            } => Some((*code, *portal, until.is_some())),
            _ => None,
        })
        .collect();
    for phase in [
        (StatusCode::ConnectingMail, None),
        (StatusCode::SearchingMail, None),
        (StatusCode::ReadingMails, None),
        (StatusCode::FetchingDetails, Some(Portal::Freelancermap)),
        (StatusCode::FetchingDetails, Some(Portal::LinkedIn)),
        (StatusCode::FetchingDetails, Some(Portal::FreelanceDe)),
        (StatusCode::WritingFiles, None),
    ] {
        assert!(
            statuses.contains(&(phase.0, phase.1, false)),
            "{phase:?}: {statuses:?}"
        );
    }
    let waits: Vec<_> = statuses.iter().filter(|(.., until)| *until).collect();
    assert!(!waits.is_empty());
    assert!(
        waits
            .iter()
            .all(|(code, portal, _)| *code == StatusCode::Waiting && portal.is_some())
    );
    // After a wait an activity follows again - in between other portals may wait too, they
    // run side by side. The status line never stays on the pause.
    for (i, _) in statuses.iter().enumerate().filter(|(_, (.., wait))| *wait) {
        let next = statuses[i + 1..].iter().find(|(.., wait)| !wait);
        assert!(
            next.is_some_and(|(code, ..)| matches!(
                code,
                StatusCode::FetchingDetails | StatusCode::SigningIn | StatusCode::WritingFiles
            )),
            "{statuses:?}"
        );
    }
    // `null` instead of a missing field.
    let status_json = serde_json::to_value(status(StatusCode::Scoring, None, None)).unwrap();
    assert_eq!(
        status_json,
        serde_json::json!({"type": "status", "code": "scoring", "portal": null, "until": null})
    );
    assert!(events.iter().any(|e| matches!(
        e,
        RunEvent::PortalHealth { portal: Portal::FreelanceDe, health: h, action_needed }
            if *h == health && *action_needed == health.action_needed()
    )));

    // Second run: nothing new, no text file twice; overview anew (new run).
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.scan.as_ref().unwrap().new, 0);
    assert_eq!(s.export.as_ref().unwrap().txt_written, 0);
    assert_eq!(txt_files(dir.path()), 4);
    // Without a change the overview is not touched (an open Excel file is not disturbed).
    let again = export_all(&store, dir.path(), &[], s.run, c(), Language::De);
    assert_eq!(again.overview_xlsx, None);
}

#[tokio::test(start_paused = true)]
async fn cancel_during_fetch_keeps_work_and_still_exports() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let cancel = CancellationToken::new();
    let mut events = Vec::new();
    let policy = Mutex::new(Policy::in_memory());
    let s = run(
        &mut DemoBackends,
        &store,
        &policy,
        &request(),
        &ctx(dir.path(), false),
        &cancel,
        &c,
        |e| {
            if matches!(e, RunEvent::JobUpdated { .. }) {
                cancel.cancel();
            }
            events.push(e);
        },
    )
    .await;
    assert_eq!(s.outcome, Outcome::Cancelled);
    assert_eq!(
        s.export.as_ref().unwrap().txt_written,
        1,
        "what was fetched is stored and exported"
    );
    assert_eq!(finished(&events), 1);
}

/// The mailbox session ends with a logout once the scan is done.
#[tokio::test(start_paused = true)]
async fn the_scan_logs_out() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::mail::imap::MailSource;
    use crate::mail::{RawHead, RawMail};

    static LOGGED_OUT: AtomicBool = AtomicBool::new(false);
    struct Tracked(DemoMail);
    impl MailSource for Tracked {
        async fn search(
            &mut self,
            since: Option<jiff::civil::Date>,
            portals: &[Portal],
        ) -> Result<Vec<u32>, MailError> {
            self.0.search(since, portals).await
        }
        async fn heads(&mut self, uids: &[u32]) -> Result<Vec<RawHead>, MailError> {
            self.0.heads(uids).await
        }
        async fn fetch(&mut self, uids: &[u32]) -> Result<Vec<RawMail>, MailError> {
            self.0.fetch(uids).await
        }
        async fn logout(self) {
            LOGGED_OUT.store(true, Ordering::SeqCst);
        }
    }
    struct WithTracked;
    impl Backends for WithTracked {
        type Mail = Tracked;
        type Pages = DemoPages;
        async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<Tracked, MailError> {
            Ok(Tracked(DemoBackends.connect_mail(cancel).await?))
        }
        fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
            Ok(DemoPages)
        }
    }
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, _) = go(
        &mut WithTracked,
        &store,
        &request(),
        &scan_only(dir.path()),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.outcome, Outcome::Completed);
    assert!(LOGGED_OUT.load(Ordering::SeqCst));
}

/// If the mailbox fails, nothing is fetched - the export still runs.
#[tokio::test(start_paused = true)]
async fn mail_failure_skips_fetch_but_exports() {
    struct Failing;
    impl Backends for Failing {
        type Mail = DemoMail;
        type Pages = DemoPages;
        async fn connect_mail(&mut self, _: &CancellationToken) -> Result<DemoMail, MailError> {
            Err(MailError::Auth(
                "[AUTHENTICATIONFAILED] Invalid credentials".into(),
            ))
        }
        fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
            Ok(DemoPages)
        }
    }
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, events) = go(
        &mut Failing,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(&s.outcome, Outcome::Failed { error } if error.kind == ErrorKind::MailAuth));
    assert!(s.fetch.is_none());
    assert!(s.export.is_some());
    assert_eq!(finished(&events), 1);
    // The Gmail reply is for the log only, never in the summary.
    let json = serde_json::to_string(&s).unwrap();
    assert!(!json.contains("AUTHENTICATIONFAILED"), "{json}");
}

#[tokio::test(start_paused = true)]
async fn dry_run_writes_nothing() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), true),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.outcome, Outcome::Completed);
    assert!(s.export.is_none());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    assert!(store.kv_get(LAST_RUN).unwrap().is_none());
}

#[tokio::test(start_paused = true)]
async fn a_portal_must_be_enabled() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let mut context = ctx(dir.path(), false);
    context.portals.clear();
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &context,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(&s.outcome, Outcome::Failed { error }
        if error.kind == ErrorKind::Invalid && error.params["reason"] == "noPortal"));
}

/// Details switched off for a portal: its alert mails are read, its pages never requested.
#[tokio::test(start_paused = true)]
async fn details_off_means_no_request_to_the_portal() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let mut context = ctx(dir.path(), false);
    context.fetch_portals = vec![Portal::Freelancermap];
    let (s, events) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &context,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.scan.as_ref().unwrap().new, 5, "all mails are read");
    let requested: Vec<Portal> = events
        .iter()
        .filter_map(|e| match e {
            RunEvent::Status {
                code: StatusCode::FetchingDetails | StatusCode::Waiting,
                portal,
                ..
            } => *portal,
            _ => None,
        })
        .collect();
    assert!(
        requested.iter().all(|p| *p == Portal::Freelancermap),
        "{requested:?}"
    );
    // A targeted fetch of a switched-off portal does not request it either.
    let key = crate::portal::job_link("https://www.linkedin.com/jobs/view/4999000002/")
        .unwrap()
        .key;
    let details = RunRequest {
        kind: RunKind::Details { keys: vec![key] },
    };
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &details,
        &context,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.fetch.as_ref().unwrap().queued, 0);
}

/// "Fetch details" for single jobs: only these, without the mailbox.
#[tokio::test(start_paused = true)]
async fn targeted_fetch_only_touches_the_chosen_jobs() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &scan_only(dir.path()),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let key = crate::portal::job_link("https://www.linkedin.com/jobs/view/4999000002/")
        .unwrap()
        .key;
    let targeted = RunRequest {
        kind: RunKind::Details {
            keys: vec![key.clone()],
        },
    };
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &targeted,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.kind, RunKindName::Details);
    assert!(s.scan.is_none(), "a targeted fetch without the mailbox");
    assert_eq!(s.fetch.as_ref().unwrap().queued, 1);
    assert_eq!(
        store.job(&key).unwrap().unwrap().desc_status,
        DescStatus::Ok
    );
}

/// Rescore: no mailbox, no fetch - only the export (the score step follows with a matcher).
#[tokio::test(start_paused = true)]
async fn rescore_only_exports() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_with_texts();
    let rescore = RunRequest {
        kind: RunKind::Rescore,
    };
    let (s, events) = go(
        &mut DemoBackends,
        &store,
        &rescore,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(s.scan.is_none() && s.fetch.is_none());
    assert_eq!(s.export.as_ref().unwrap().txt_written, 2);
    assert_eq!(finished(&events), 1);
}

/// Cancel after k of n job details => exactly k stored, exactly one `Finished`.
#[tokio::test(start_paused = true)]
async fn cancel_after_k_of_n_keeps_exactly_k() {
    for k in 0..=5 {
        let c = clock();
        let dir = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        let cancel = CancellationToken::new();
        let mut events = Vec::new();
        let mut updated = 0;
        let policy = Mutex::new(Policy::in_memory());
        run(
            &mut DemoBackends,
            &store,
            &policy,
            &request(),
            &ctx(dir.path(), false),
            &cancel,
            &c,
            |e| {
                if matches!(e, RunEvent::JobUpdated { .. }) {
                    updated += 1;
                }
                if updated >= k
                    && matches!(
                        e,
                        RunEvent::JobUpdated { .. }
                            | RunEvent::Status { .. }
                            | RunEvent::Progress { .. }
                    )
                {
                    cancel.cancel();
                }
                events.push(e);
            },
        )
        .await;
        let ok = store
            .jobs(&JobFilter::default())
            .unwrap()
            .iter()
            .filter(|j| j.desc_status == DescStatus::Ok)
            .count();
        assert_eq!(ok, updated, "k = {k}");
        assert_eq!(finished(&events), 1, "k = {k}");
    }
}

/// The biggest summary a run can send stays below 8 KB.
#[test]
fn the_largest_summary_is_a_small_event() {
    let now = Timestamp::now();
    let mut summary = RunSummary::new(RunKindName::FullMailbox, false, now);
    summary.outcome = Outcome::Failed {
        error: ErrorInfo::new(ErrorKind::Io).with("path", "x".repeat(400)),
    };
    summary.scan = Some(ScanCounts::default());
    summary.per_portal = Portal::ALL
        .into_iter()
        .map(|portal| PortalSummary {
            portal,
            new: 9999,
            known: 9999,
            dup: 9999,
            fetched: 9999,
            failed: 9999,
            gone: 9999,
            skipped: 9999,
            stopped: Some(PortalHealth::LayoutSuspect {
                empty_mails: 9999,
                pages: 99,
            }),
        })
        .collect();
    summary.score = Some(ScoreSummary::default());
    summary.export = Some(ExportSummary {
        overview_xlsx: Some(PathBuf::from("y".repeat(400))),
        overview_html: Some(PathBuf::from("y".repeat(400))),
        backup: Some(PathBuf::from("y".repeat(400))),
        error: Some(ErrorInfo::new(ErrorKind::FileLocked).with("path", "z".repeat(400))),
        ..ExportSummary::default()
    });
    summary.empty_alerts = (0..MAX_EMPTY_ALERTS)
        .map(|_| EmptyAlert {
            portal: Portal::FreelanceDe,
            subject: "ü".repeat(MAX_SUBJECT_CHARS),
            date: Some(now),
            gmail_id: Some(format!("{:x}", u64::MAX)),
        })
        .collect();
    assert_small(&[RunEvent::Finished {
        summary: Box::new(summary),
    }]);
}

/// Two jobs with a full text (without the mailbox), for export tests.
fn store_with_texts() -> (Store, Vec<JobKey>) {
    fill_texts(Store::in_memory().unwrap())
}

/// The same two jobs in a database on disk - only then can a database error be caused from
/// outside (second connection).
fn store_on_disk(dir: &Path) -> (Store, PathBuf) {
    let db = dir.join("jobs.db");
    let (store, _) = fill_texts(Store::open(&db).unwrap());
    (store, db)
}

fn fill_texts(store: Store) -> (Store, Vec<JobKey>) {
    let run = store.begin_run().unwrap();
    let mut keys = Vec::new();
    for id in [4_000_000_001_u64, 4_000_000_002] {
        let link =
            crate::portal::job_link(&format!("https://www.linkedin.com/jobs/view/{id}/")).unwrap();
        let posting = crate::model::Posting::new(link.key.clone(), link.url, "Rolle", "", "");
        let mail = crate::store::MailRef {
            subject: "Neue Jobs",
            date: None,
            gmail_id: None,
        };
        store
            .upsert_posting(run, &posting, mail, Timestamp::now())
            .unwrap();
        store
            .record_text(&link.key, "Volltext", false, false, Timestamp::now())
            .unwrap();
        keys.push(link.key);
    }
    (store, keys)
}

/// A foreign overview (e.g. from the old program) is backed up once - the app's own never
/// on any further run.
#[test]
fn only_a_foreign_overview_is_backed_up_and_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_with_texts();
    let result_dir = dir.path().join(RESULT_DIR);
    std::fs::create_dir_all(&result_dir).unwrap();
    std::fs::write(result_dir.join(export::XLSX_NAME), b"fremd").unwrap();
    // 09:30 in Berlin (07:30 UTC): the name says the time the user's clock says.
    let now: Timestamp = "2026-09-24T07:30:00Z".parse().unwrap();

    let first = export_all(&store, dir.path(), &[], 1, now, Language::De);
    let backup = first.backup.clone().expect("foreign file backed up");
    assert_eq!(std::fs::read(&backup).unwrap(), b"fremd");
    assert_eq!(
        backup.file_name().unwrap(),
        "JobAlerts.alt-20260924-093000.xlsx"
    );
    // The name the app can show in its folder, and only such a name.
    assert!(export::is_xlsx_backup("JobAlerts.alt-20260924-093000.xlsx"));
    for other in [
        "JobAlerts.xlsx",
        "JobAlerts.alt-x.txt",
        r"JobAlerts.alt-..\..\jobs.xlsx",
        "JobAlerts.alt-/x.xlsx",
    ] {
        assert!(!export::is_xlsx_backup(other), "{other}");
    }
    assert!(first.overview_xlsx.is_some());

    // Further runs continue the app's own file without backing it up again.
    let second = export_all(&store, dir.path(), &[], 2, now, Language::De);
    let back = export_all(&store, dir.path(), &[], 3, now, Language::De);
    assert_eq!((second.backup, back.backup), (None, None));
    assert!(back.overview_xlsx.is_some(), "new run: sheet \"Info\" anew");
    let backups = std::fs::read_dir(&result_dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("JobAlerts.alt-")
        })
        .count();
    assert_eq!(backups, 1);
    // Without a new run and without new data the overview stays.
    let again = export_all(&store, dir.path(), &[], 3, now, Language::De);
    assert_eq!(again.overview_xlsx, None);
}

/// If the export stamp cannot be read, the ownership of the file is unknown: the existing
/// overview stays - no backup, no overwrite.
#[test]
fn an_unreadable_export_stamp_leaves_the_overview_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (store, db) = store_on_disk(dir.path());
    let now = Timestamp::now();
    let first = export_all(&store, dir.path(), &[], 1, now, Language::De);
    assert!(first.overview_xlsx.is_some() && first.backup.is_none());
    let path = dir.path().join(RESULT_DIR).join(export::XLSX_NAME);
    let before = std::fs::read(&path).unwrap();

    // The table with the export stamp is missing: every `kv_get` fails.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute("DROP TABLE kv", [])
        .unwrap();
    let again = export_all(&store, dir.path(), &[], 2, now, Language::De);
    assert_eq!(again.backup, None, "no backup with unknown ownership");
    assert_eq!(again.overview_xlsx, None);
    assert_eq!(std::fs::read(&path).unwrap(), before, "file unchanged");
    let error = again.error.expect("an error");
    assert_eq!(error.params["target"], "overview");
    let backups = std::fs::read_dir(dir.path().join(RESULT_DIR))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".alt-"))
        .count();
    assert_eq!(backups, 0);
}

/// If a written text file cannot be marked, the remaining ones count as open too - and the
/// first message, closest to the cause, stays.
#[test]
fn a_failed_mark_counts_the_rest_and_keeps_the_first_error() {
    let dir = tempfile::tempdir().unwrap();
    let (store, db) = store_on_disk(dir.path());
    // No change of the job table: reading works, marking fails.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "CREATE TRIGGER no_marking BEFORE UPDATE ON job
             BEGIN SELECT RAISE(ABORT, 'locked'); END",
            [],
        )
        .unwrap();
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now(), Language::De);
    assert_eq!(
        (summary.txt_written, summary.txt_failed),
        (0, 2),
        "{summary:?}"
    );
    // The number comes with at least one example.
    assert!(!summary.txt_failed_keys.is_empty(), "{summary:?}");
    assert!(
        summary
            .error
            .as_ref()
            .is_some_and(|e| e.kind == ErrorKind::Db),
        "{summary:?}"
    );
}

/// If text files and overview fail at the same unusable folder, the run reports the first
/// message, closest to the cause - not the bare consequential error of the overview.
#[test]
fn the_first_error_survives_a_later_one() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_with_texts();
    // A file stands where the result folder should be: nothing can be written there.
    std::fs::write(dir.path().join(RESULT_DIR), b"no folder").unwrap();
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now(), Language::De);
    assert_eq!((summary.txt_written, summary.txt_failed), (0, 2));
    assert_eq!(summary.overview_xlsx, None);
    let error = summary.error.expect("an error");
    assert_eq!(error.params["target"], "txtFolder");
    assert!(
        error.params["path"].as_str().unwrap().contains(TXT_DIR),
        "{error:?}"
    );
}

/// A failed mailbox scan does not overwrite the numbers of the last good one.
#[tokio::test(start_paused = true)]
async fn the_info_sheet_keeps_the_last_good_scan() {
    struct Failing;
    impl Backends for Failing {
        type Mail = DemoMail;
        type Pages = DemoPages;
        async fn connect_mail(&mut self, _: &CancellationToken) -> Result<DemoMail, MailError> {
            Err(MailError::Timeout)
        }
        fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
            Ok(DemoPages)
        }
    }
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &scan_only(dir.path()),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let after_good = store.kv_get(LAST_SCAN_FACTS).unwrap().unwrap();
    assert!(after_good.contains("\"new\":5"), "{after_good}");
    assert!(!after_good.contains('@'), "no mail address in the file");
    let (failed, _) = go(
        &mut Failing,
        &store,
        &request(),
        &scan_only(dir.path()),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(failed.outcome, Outcome::Failed { .. }));
    let rescore = RunRequest {
        kind: RunKind::Rescore,
    };
    go(
        &mut DemoBackends,
        &store,
        &rescore,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(store.kv_get(LAST_SCAN_FACTS).unwrap().unwrap(), after_good);
    let rows = info_rows(&store, c(), &texts::DE);
    assert!(rows.iter().any(|(k, v)| k == texts::INFO_NEW && v == "5"));
    assert!(
        rows.iter()
            .any(|(k, v)| k == texts::INFO_SCOPE && v == texts::SCOPE_NEW)
    );
    // The same scan in English: the words follow the language, the numbers stay.
    let rows = info_rows(&store, c(), &texts::EN);
    assert!(
        rows.iter()
            .any(|(k, v)| k == texts::en::INFO_NEW && v == "5")
    );
    assert!(
        rows.iter()
            .any(|(k, v)| k == texts::en::INFO_SCOPE && v == texts::en::SCOPE_NEW)
    );
    assert!(
        rows.iter().all(|(k, _)| !k.contains("Postfach")),
        "{rows:?}"
    );
}

/// The Info sheet of the Excel file as `(label, value)` rows.
fn info_sheet(workspace: &Path) -> Vec<(String, String)> {
    use calamine::{Reader, Xlsx, open_workbook};
    let path = export::overview_path(&workspace.join(RESULT_DIR));
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    book.worksheet_range(texts::INFO_SHEET)
        .unwrap()
        .rows()
        .map(|row| (row[0].to_string(), row[1].to_string()))
        .collect()
}

/// The Info sheet says what the app says: "new" is the run card's number (one per job, the
/// excluded one left out), the job count is the sheet's rows, and a rescore writes the
/// moment the file was made - never a fetch time it did not have.
#[tokio::test(start_paused = true)]
async fn the_info_sheet_says_what_the_app_says() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let cancel = CancellationToken::new();
    let fetch = ctx(dir.path(), false);
    let (s, _) = go(&mut DemoBackends, &store, &request(), &fetch, &cancel, &c).await;
    let card = s.new_jobs.unwrap().count;
    assert!(card < s.scan.unwrap().new, "the excluded job is no new job");
    let value = |rows: &[(String, String)], label: &str| -> String {
        rows.iter()
            .find(|(k, _)| k == label)
            .map_or_else(|| panic!("{label}: {rows:?}"), |(_, v)| v.clone())
    };
    assert_eq!(
        value(&info_sheet(dir.path()), texts::INFO_NEW),
        card.to_string()
    );
    let listed = store.listed_count().unwrap();
    let key = store.jobs(&JobFilter::default()).unwrap()[0].key.clone();
    store
        .move_jobs(std::slice::from_ref(&key), Place::Archive, c())
        .unwrap();
    // A rescore an hour later writes the file again.
    let later = move || c() + SignedDuration::from_hours(1);
    let rescore = RunRequest {
        kind: RunKind::Rescore,
    };
    let (r, _) = go(&mut DemoBackends, &store, &rescore, &fetch, &cancel, &later).await;
    let rows = info_sheet(dir.path());
    assert_eq!(
        value(&rows, texts::INFO_JOBS_TOTAL),
        (listed - 1).to_string(),
        "the sheet's rows"
    );
    assert_eq!(
        value(&rows, texts::HTML_CREATED),
        texts::DE.moment(r.finished_at)
    );
    assert_ne!(
        value(&rows, texts::INFO_LAST_SCAN),
        value(&rows, texts::HTML_CREATED),
        "the fetch keeps its own time"
    );
    assert_eq!(
        value(&rows, texts::INFO_NEW),
        card.to_string(),
        "still the fetch's"
    );
    assert!(rows.iter().all(|(k, _)| k != texts::INFO_LAST_RUN));
}

/// Rows stored by an earlier version (mail address, "Lauf" for a mailbox scan) come out in
/// today's words and without the address.
#[test]
fn info_rows_of_an_earlier_version_use_todays_words() {
    let store = Store::in_memory().unwrap();
    let old = serde_json::json!([
        ["Letzter Postfach-Abruf", "01.09.2026 08:00"],
        [LEGACY_ACCOUNT_LABEL, "someone@example.com"],
        ["Umfang des letzten Laufs", "Neu seit letztem Lauf"],
        ["Neu (letzter Lauf)", "3"],
        ["Schon bekannt (letzter Lauf)", "1"],
        ["Doppelt in mehreren Mails (letzter Lauf)", "0"]
    ]);
    store.kv_set(LAST_SCAN_INFO, &old.to_string()).unwrap();
    let english = info_rows(&store, Timestamp::now(), &texts::EN);
    assert_eq!(
        english[0],
        (
            texts::en::INFO_LAST_SCAN.to_owned(),
            "01.09.2026 08:00".to_owned()
        )
    );
    assert_eq!(english[1].1, texts::en::SCOPE_NEW);
    assert_eq!(english[2], (texts::en::INFO_NEW.to_owned(), "3".to_owned()));
    let rows = info_rows(&store, Timestamp::now(), &texts::DE);
    let labels: Vec<&str> = rows.iter().map(|(label, _)| label.as_str()).collect();
    assert_eq!(
        labels,
        [
            texts::INFO_LAST_SCAN,
            texts::INFO_SCOPE,
            texts::INFO_NEW,
            texts::INFO_KNOWN,
            texts::INFO_DUP,
            texts::HTML_CREATED,
            texts::INFO_JOBS_TOTAL,
            texts::INFO_PROGRAM
        ]
    );
    assert_eq!(rows[1].1, texts::SCOPE_NEW);
    assert!(rows.iter().all(|(_, value)| !value.contains('@')));
}

/// Safety invariant: a text file the user deleted or emptied is not recreated by the next
/// run - the mark stays used. Only "rewrite text files" brings it back.
#[test]
fn a_text_file_the_user_removed_is_never_recreated_by_itself() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    let now = Timestamp::now();
    assert_eq!(
        export_all(&store, dir.path(), &[], 1, now, Language::De).txt_written,
        2
    );
    let txt_dir = dir.path().join(RESULT_DIR).join(TXT_DIR);
    let name = |key| store.job(key).unwrap().unwrap().txt_name.unwrap();
    let (deleted, emptied) = (txt_dir.join(name(&keys[0])), txt_dir.join(name(&keys[1])));
    std::fs::remove_file(&deleted).unwrap();
    std::fs::write(&emptied, b"").unwrap();

    let next = export_all(&store, dir.path(), &[], 2, now, Language::De);
    assert_eq!((next.txt_written, next.txt_failed), (0, 0));
    assert!(!deleted.exists(), "a deleted file stays away");
    assert!(
        std::fs::read(&emptied).unwrap().is_empty(),
        "empty stays empty"
    );
    // Only the explicit command brings them back.
    assert_eq!(rewrite_txt(&store, dir.path(), now).txt_written, 2);
    assert!(deleted.exists() && !std::fs::read(&emptied).unwrap().is_empty());
}

/// "Rewrite text files": what cannot be written keeps its mark - so the next run does not
/// recreate it by itself.
#[test]
fn a_failed_rewrite_keeps_the_marks() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    let now = Timestamp::now();
    let first = export_all(&store, dir.path(), &[], 1, now, Language::De);
    assert_eq!(first.txt_written, 2);
    let txt_dir = dir.path().join(RESULT_DIR).join(TXT_DIR);
    let blocked = txt_dir.join(store.job(&keys[1]).unwrap().unwrap().txt_name.unwrap());
    // A folder stands where the second file should be: writing fails.
    std::fs::remove_file(&blocked).unwrap();
    std::fs::create_dir(&blocked).unwrap();

    let rewrite = rewrite_txt(&store, dir.path(), now);
    assert_eq!((rewrite.txt_written, rewrite.txt_failed), (1, 1));
    assert_eq!(rewrite.txt_failed_keys, [keys[1].to_string()]);
    // No mark was removed: nothing counts as "still to write".
    assert!(store.txt_jobs(false).unwrap().is_empty());
    let next = export_all(&store, dir.path(), &[], 2, now, Language::De);
    assert_eq!(
        (next.txt_written, next.txt_failed),
        (0, 0),
        "nothing anew by itself"
    );
}

/// A work folder that cannot be reached (a network drive or a stick that is gone) is one
/// clear error that names it; nothing is written and the text files wait for the next
/// export. A text file of a job deleted meanwhile is remembered until the folder is back.
#[test]
fn an_unreachable_work_folder_is_one_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    // A file stands where the drive's folder would be: the folder cannot be made.
    let stick = dir.path().join("stick");
    std::fs::write(&stick, b"").unwrap();
    let gone = stick.join("Job-Alerts");
    let now = Timestamp::now();
    let s = export_all(&store, &gone, &[], 1, now, Language::De);
    let target = |s: &ExportSummary| s.error.as_ref().unwrap().params["target"].clone();
    assert_eq!(target(&s), "workspace");
    assert_eq!((s.txt_written, s.txt_failed), (0, 0));
    assert_eq!((s.overview_xlsx, s.overview_html), (None, None));
    assert_eq!(store.txt_jobs(false).unwrap().len(), 2, "the files wait");
    assert_eq!(target(&rewrite_txt(&store, &gone, now)), "workspace");
    // Emptied from the trash meanwhile: the name waits for the folder.
    store.mark_txt_written(&keys[0], "a.txt", now).unwrap();
    let one = &keys[..1];
    store.move_jobs(one, Place::Trash, now).unwrap();
    let deleted = delete_jobs(&store, Some(&gone), None, one, (now, Language::De)).unwrap();
    assert_eq!(deleted.count, 1);
    assert_eq!(store.txt_leftovers().unwrap(), ["a.txt"]);
    let (removed, _) = clear_txt(&store, &gone).unwrap();
    assert_eq!(removed, 0);
    assert_eq!(store.txt_leftovers().unwrap(), ["a.txt"], "still waiting");
}

/// If the folder for the text files is unusable, there is one clear error instead of one
/// attempt per file - and the real number.
#[test]
fn an_unusable_text_folder_is_one_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    let result_dir = dir.path().join(RESULT_DIR);
    std::fs::create_dir_all(&result_dir).unwrap();
    std::fs::write(result_dir.join(TXT_DIR), b"a file instead of the folder").unwrap();
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now(), Language::De);
    assert_eq!((summary.txt_written, summary.txt_failed), (0, 2));
    // Here too the list names the affected jobs - it is never empty next to a number.
    let mut failed = summary.txt_failed_keys.clone();
    failed.sort();
    let mut expected: Vec<String> = keys.iter().map(ToString::to_string).collect();
    expected.sort();
    assert_eq!(failed, expected);
    let error = summary.error.expect("an error");
    assert_eq!(error.params["target"], "txtFolder");
    assert!(error.params["path"].as_str().unwrap().contains(TXT_DIR));
}

/// If no run can be created (database locked or broken), it ends as an error - without
/// mailbox, without fetch, with exactly one `Finished`.
#[tokio::test(start_paused = true)]
async fn a_run_that_cannot_begin_fails() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    store.kv_set("run_seq", "broken").unwrap();
    let (s, events) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(&s.outcome, Outcome::Failed { error } if error.kind == ErrorKind::Corrupt));
    assert!(s.scan.is_none() && s.fetch.is_none() && s.export.is_none());
    assert_eq!(s.run, 0);
    assert_eq!(finished(&events), 1);
    assert_eq!(store.job_count().unwrap(), 0);
}

/// The dry-run backends without a matcher.
struct Unscored;

impl Backends for Unscored {
    type Mail = DemoMail;
    type Pages = DemoPages;
    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<DemoMail, MailError> {
        DemoBackends.connect_mail(cancel).await
    }
    fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
        Ok(DemoPages)
    }
}

/// A matcher that judges only LinkedIn jobs, with a changeable revision.
struct Picky(&'static str);

impl Matcher for Picky {
    fn rev(&self) -> &str {
        self.0
    }
    fn assess(&self, job: &JobRow, _text: Option<&str>) -> Option<crate::model::MatchRecord> {
        (job.key.portal == Portal::LinkedIn).then(|| crate::model::MatchRecord {
            status: crate::model::MatchStatus::Scored,
            score: 50,
            note: None,
            must_met: 0,
            must_total: 0,
            top: Vec::new(),
            facts: crate::model::KeyFacts::default(),
            rank: 0,
        })
    }
}

struct WithPicky(&'static str);

impl Backends for WithPicky {
    type Mail = DemoMail;
    type Pages = DemoPages;
    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<DemoMail, MailError> {
        DemoBackends.connect_mail(cancel).await
    }
    fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
        Ok(DemoPages)
    }
    fn matcher(&self) -> Option<Arc<dyn Matcher>> {
        Some(Arc::new(Picky(self.0)))
    }
}

/// Without a matcher nothing is scored and the summary has no score; jobs the matcher does
/// not judge stay pending without stalling the catch-up; a new revision scores again.
#[tokio::test(start_paused = true)]
async fn scoring_follows_the_matcher() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, _) = go(
        &mut Unscored,
        &store,
        &request(),
        &scan_only(dir.path()),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.score, None);
    assert!(store.best_matches(5).unwrap().is_empty());
    let rescore = RunRequest {
        kind: RunKind::Rescore,
    };
    let (s, _) = go(
        &mut WithPicky("r1"),
        &store,
        &rescore,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let first = s.score.unwrap();
    assert_eq!((first.scored, first.pending), (2, 3), "{first:?}");
    let (s, _) = go(
        &mut WithPicky("r1"),
        &store,
        &rescore,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.score.unwrap().scored, 0, "nothing stale");
    let (s, _) = go(
        &mut WithPicky("r2"),
        &store,
        &rescore,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.score.unwrap().scored, 2, "a new revision scores again");
}

/// Whatever subjects and paths a summary holds, its `Finished` event stays below 8 KB.
#[test]
fn the_finished_event_always_fits_the_channel() {
    let mut summary = RunSummary::new(RunKindName::Fetch, false, Timestamp::now());
    let wide = "\u{1f600}".repeat(crate::view::MAX_SUBJECT_CHARS);
    summary.empty_alerts = (0..MAX_EMPTY_ALERTS)
        .map(|_| crate::view::EmptyAlert {
            portal: Portal::LinkedIn,
            subject: wide.clone(),
            date: None,
            gmail_id: Some("18f0a1b2c3d4e5f6".into()),
        })
        .collect();
    let long = PathBuf::from("C:/".to_owned() + &"verzeichnis/".repeat(20));
    let mut error = ErrorInfo::new(ErrorKind::Io);
    error
        .params
        .insert("target".into(), long.display().to_string().into());
    summary.export = Some(ExportSummary {
        overview_xlsx: Some(long.join("a.xlsx")),
        overview_html: Some(long.join("a.html")),
        backup: Some(long.join("b.xlsx")),
        txt_written: 3,
        txt_failed: 0,
        txt_failed_keys: Vec::new(),
        error: Some(error),
    });
    assert!(
        serde_json::to_vec(&summary).unwrap().len() > 8 * 1024,
        "too big at first"
    );
    let event = summary.finished_event();
    assert_small(std::slice::from_ref(&event));
    let RunEvent::Finished { summary: fitted } = event else {
        panic!("finished");
    };
    assert!(
        !fitted.empty_alerts.is_empty(),
        "only as much goes as needed"
    );
    assert_eq!(fitted.export.as_ref().unwrap().txt_written, 3);
    // Absurd paths go too.
    let huge = PathBuf::from("C:/".to_owned() + &"verzeichnis/".repeat(400));
    let export = summary.export.as_mut().unwrap();
    export.overview_html = Some(huge.clone());
    export.overview_xlsx = Some(huge);
    assert_small(&[summary.finished_event()]);
    let small = RunSummary::new(RunKindName::Fetch, false, Timestamp::now());
    assert_eq!(
        small.finished_event(),
        RunEvent::Finished {
            summary: Box::new(small.clone())
        },
        "a small summary goes out unchanged"
    );
}

/// An engine that panics on LinkedIn jobs (like the splitter once did on some ads).
struct Panicky;

impl Matcher for Panicky {
    fn rev(&self) -> &'static str {
        "boom"
    }
    fn assess(&self, job: &JobRow, text: Option<&str>) -> Option<crate::model::MatchRecord> {
        assert!(job.key.portal != Portal::LinkedIn, "engine bug");
        Picky("boom").assess(job, text)
    }
    fn explain(&self, job: &JobRow, _text: Option<&str>) -> Option<crate::matching::Assessment> {
        assert!(job.key.portal != Portal::LinkedIn, "engine bug");
        None
    }
}

struct WithPanicky;

impl Backends for WithPanicky {
    type Mail = DemoMail;
    type Pages = DemoPages;
    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<DemoMail, MailError> {
        DemoBackends.connect_mail(cancel).await
    }
    fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
        Ok(DemoPages)
    }
    fn matcher(&self) -> Option<Arc<dyn Matcher>> {
        Some(Arc::new(Panicky))
    }
}

/// A panic of the engine on one job neither stops the run nor the export: the job is
/// unscorable with `engineFailed` and not asked again with the same revision.
#[tokio::test(start_paused = true)]
async fn an_engine_panic_marks_the_job_and_the_run_goes_on() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, _) = go(
        &mut WithPanicky,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.outcome, Outcome::Completed);
    assert!(s.export.is_some(), "the export still ran");
    let jobs = store.jobs(&crate::store::JobFilter::default()).unwrap();
    let linkedin: Vec<&JobRow> = jobs
        .iter()
        .filter(|j| j.key.portal == Portal::LinkedIn)
        .collect();
    assert!(!linkedin.is_empty(), "{jobs:?}");
    for job in linkedin {
        let record = job.match_.as_ref().unwrap();
        assert_eq!(record.status, crate::model::MatchStatus::Unscorable);
        assert_eq!(
            record.note.as_ref().unwrap().code,
            super::score::ENGINE_FAILED
        );
        assert_eq!(job.match_rev.as_deref(), Some("boom"), "not asked again");
    }
}

/// The text files do not know the match: byte-identical with and without a matcher.
#[tokio::test(start_paused = true)]
async fn txt_is_blind_to_the_match() {
    let c = clock();
    let read_all = |root: &Path| {
        let dir = root.join(RESULT_DIR).join(TXT_DIR);
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };
    let (with, without) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let scored = Store::in_memory().unwrap();
    let (s, _) = go(
        &mut DemoBackends,
        &scored,
        &request(),
        &ctx(with.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(s.score.is_some());
    let plain = Store::in_memory().unwrap();
    go(
        &mut Unscored,
        &plain,
        &request(),
        &ctx(without.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let (a, b) = (read_all(with.path()), read_all(without.path()));
    assert_eq!(a.len(), 4);
    // The fetch time is part of the header; the simulated clocks of both runs differ, so
    // compare everything but that line.
    let strip = |files: &[(String, Vec<u8>)]| -> Vec<(String, String)> {
        files
            .iter()
            .map(|(name, bytes)| {
                let text = String::from_utf8(bytes.clone()).unwrap();
                let kept: Vec<&str> = text
                    .split_inclusive('\n')
                    .filter(|l| !l.starts_with("Abgerufen am: "))
                    .collect();
                (name.clone(), kept.concat())
            })
            .collect()
    };
    assert_eq!(strip(&a), strip(&b));
}

/// The auto fetch at the start: switched on, with a mailbox, last fetch older than 6 hours.
#[tokio::test(start_paused = true)]
async fn the_auto_fetch_waits_six_hours() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let now = c();
    let on = crate::settings::Settings::default();
    let off = crate::settings::Settings {
        auto_fetch_on_start: false,
        ..on.clone()
    };
    let mut no_portal = on.clone();
    for switches in no_portal.portals.values_mut() {
        switches.enabled = false;
    }
    assert!(auto_fetch_due(&store, &on, true, now), "never fetched");
    assert!(!auto_fetch_due(&store, &off, true, now), "switched off");
    assert!(!auto_fetch_due(&store, &on, false, now), "no mailbox");
    assert!(
        !auto_fetch_due(&store, &no_portal, true, now),
        "no portal to read: no fetch that can only fail"
    );
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &scan_only(dir.path()),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let fetched = last_fetch_at(&store).unwrap();
    let after = |hours| fetched + SignedDuration::from_hours(hours);
    assert!(!auto_fetch_due(&store, &on, true, after(5)));
    assert!(auto_fetch_due(&store, &on, true, after(7)));
}

/// The request JSON is flat, and every kind round-trips.
#[test]
fn run_requests_are_flat_json() {
    let request: RunRequest = serde_json::from_str(
        r#"{"kind":"details","keys":[{"portal":"linkedin","id":"4123456789"}]}"#,
    )
    .unwrap();
    assert_eq!(request.kind.name(), RunKindName::Details);
    for kind in ["fetch", "rescore", "fullMailbox"] {
        let json = format!(r#"{{"kind":"{kind}"}}"#);
        let request: RunRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&request).unwrap(), json);
    }
    assert!(serde_json::from_str::<RunRequest>(r#"{"kind":"scan"}"#).is_err());
}

/// `top_matches.json` for the matching skill: the scored jobs of the mailbox run, best
/// first, with met and open requirements, checks and the text file - excluded ones not.
#[tokio::test(start_paused = true)]
async fn the_skill_gets_the_top_matches() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.outcome, Outcome::Completed);
    let path = dir.path().join(RESULT_DIR).join(export::TOP_MATCHES_NAME);
    let file: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(file["schema"], 2);
    assert_eq!(file["rev"], demo::matcher().rev());
    let jobs = file["jobs"].as_array().unwrap();
    let titles: Vec<&str> = jobs.iter().map(|j| j["title"].as_str().unwrap()).collect();
    assert_eq!(
        titles,
        [
            "Interim CFO (m/w/d)",
            "Leiter Controlling (m/w/d)",
            "SAP FI/CO Berater (m/w/d)"
        ],
        "scored only, best first"
    );
    let best = &jobs[0];
    assert_eq!(
        (best["score"].as_u64(), best["band"].as_str()),
        (Some(100), Some("high"))
    );
    assert_eq!(best["key"], "linkedin:4999000001");
    assert!(
        std::path::Path::new(best["txtFile"].as_str().unwrap())
            .extension()
            .is_some_and(|e| e == "txt")
    );
    let mid = &jobs[1];
    assert!(!mid["met"].as_array().unwrap().is_empty());
    assert!(
        mid["open"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o == "Staplerschein"),
        "{mid}"
    );
    assert_eq!(mid["mustTotal"], 7);
    assert!(mid["checks"].is_array() && best["url"].as_str().unwrap().starts_with("https://"));
    assert_eq!(best["appStatus"], serde_json::Value::Null);
    assert!(best["firstSeenAt"].is_string());
    // A second fetch without new jobs keeps the list (it follows the open jobs, not the run).
    let (again, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(again.new_jobs.map(|n| n.count), Some(0));
    let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let kept: Vec<&str> = kept["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["title"].as_str().unwrap())
        .collect();
    assert_eq!(kept, titles, "the list stays after a fetch without news");
    // Without a profile the file stays, with no jobs.
    write_top_matches(&store, dir.path(), None, c());
    store.clear_matches().unwrap();
    write_top_matches(&store, dir.path(), None, c());
    let file: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(file["jobs"].as_array().unwrap().len(), 0);
    assert_eq!(file["rev"], serde_json::Value::Null);
    assert!(export::app_files(&dir.path().join(RESULT_DIR), &[]).contains(&path));
}

/// Records which fetch path the run builds per portal (mails and pages from the demo).
#[derive(Default)]
struct Paths(Vec<(Portal, FetchPath)>);

impl Backends for Paths {
    type Mail = DemoMail;
    type Pages = DemoPages;
    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<DemoMail, MailError> {
        DemoBackends.connect_mail(cancel).await
    }
    fn pages(&mut self, portal: Portal, path: FetchPath) -> Result<DemoPages, String> {
        self.0.push((portal, path));
        Ok(DemoPages)
    }
}

/// The switches reach the fetch: details off = no fetch path at all (zero requests);
/// sign-in off = the guest path, never a session window; sign-in on = the session window.
#[tokio::test(start_paused = true)]
async fn switches_decide_the_fetch_path() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let mut settings = crate::settings::Settings::default();
    settings
        .portals
        .get_mut(&Portal::LinkedIn)
        .unwrap()
        .fetch_details = false;
    let context = RunContext {
        fetch_portals: settings.fetch_portals(),
        sign_in: Vec::new(),
        ..ctx(dir.path(), false)
    };
    let mut paths = Paths::default();
    let (s, _) = go(
        &mut paths,
        &store,
        &request(),
        &context,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.outcome, Outcome::Completed);
    assert_eq!(
        paths.0,
        [
            (Portal::Freelancermap, FetchPath::Guest),
            (Portal::FreelanceDe, FetchPath::Guest)
        ]
    );
    let fetch = s.fetch.unwrap();
    assert!(!fetch.per_portal.contains_key(&Portal::LinkedIn));

    let context = RunContext {
        sign_in: vec![Portal::FreelanceDe],
        ..context
    };
    let mut paths = Paths::default();
    let store = Store::in_memory().unwrap();
    go(
        &mut paths,
        &store,
        &request(),
        &context,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(paths.0.contains(&(Portal::FreelanceDe, FetchPath::Session)));
}

/// The demo, with the fetch paths read from the stored settings like in the app.
struct Stored(Arc<Store>);

impl Backends for Stored {
    type Mail = DemoMail;
    type Pages = DemoPages;
    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<DemoMail, MailError> {
        DemoBackends.connect_mail(cancel).await
    }
    fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
        Ok(DemoPages)
    }
    fn live_paths(&self) -> Option<LivePaths> {
        Some(stored_paths(Arc::clone(&self.0)))
    }
}

/// Details switched off while the run fetches: the portal gets no further request, although
/// the run started with it switched on ("off = zero requests").
#[tokio::test(start_paused = true)]
async fn a_portal_switched_off_during_the_run_stops_at_once() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::in_memory().unwrap());
    let settings = crate::settings::Settings::default();
    settings.save(&store).unwrap();
    let context = RunContext {
        fetch_portals: settings.fetch_portals(),
        sign_in: Vec::new(),
        ..ctx(dir.path(), false)
    };
    let policy = Mutex::new(Policy::in_memory());
    let mut switched = false;
    let summary = run(
        &mut Stored(Arc::clone(&store)),
        &store,
        &policy,
        &request(),
        &context,
        &CancellationToken::new(),
        &c,
        |event| {
            if let RunEvent::JobUpdated { job, .. } = &event
                && job.key.portal == Portal::LinkedIn
                && !switched
            {
                switched = true;
                let mut off = crate::settings::Settings::load(&store).unwrap();
                off.portals
                    .get_mut(&Portal::LinkedIn)
                    .unwrap()
                    .fetch_details = false;
                off.save(&store).unwrap();
            }
        },
    )
    .await;
    assert_eq!(summary.outcome, Outcome::Completed);
    let li = summary
        .per_portal
        .iter()
        .find(|p| p.portal == Portal::LinkedIn)
        .unwrap();
    assert_eq!((li.new, li.fetched, li.skipped), (2, 1, 1));
}

/// Every run begins with exactly one `Started` naming its kind: the page also follows the
/// runs it did not start itself (the auto fetch, a rescore after a profile change) as what
/// they are.
#[tokio::test(start_paused = true)]
async fn every_run_starts_with_its_kind() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let key = crate::portal::job_link("https://www.linkedin.com/jobs/view/4999000002/")
        .unwrap()
        .key;
    let kinds = [
        (RunKind::Fetch, RunKindName::Fetch),
        (RunKind::FullMailbox, RunKindName::FullMailbox),
        (RunKind::Details { keys: vec![key] }, RunKindName::Details),
        (RunKind::Rescore, RunKindName::Rescore),
    ];
    for (kind, name) in kinds {
        let (s, events) = go(
            &mut DemoBackends,
            &store,
            &RunRequest { kind },
            &ctx(dir.path(), false),
            &CancellationToken::new(),
            &c,
        )
        .await;
        assert_eq!(s.kind, name);
        assert_eq!(events.first(), Some(&RunEvent::Started { kind: name }));
        let started = events
            .iter()
            .filter(|e| matches!(e, RunEvent::Started { .. }))
            .count();
        assert_eq!((started, finished(&events)), (1, 1), "{name:?}");
        assert_small(&events);
    }
    // A run that cannot even begin says what it was, too.
    store.kv_set("run_seq", "broken").unwrap();
    let (_, events) = go(
        &mut DemoBackends,
        &store,
        &RunRequest {
            kind: RunKind::Rescore,
        },
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(
        events.first(),
        Some(&RunEvent::Started {
            kind: RunKindName::Rescore
        })
    );
    assert_eq!(finished(&events), 1);
}

/// "The last fetch" is the last mailbox run: a details run or a rescore never replaces it
/// (the sidebar, the failed-fetch retry and the empty alert mails all read it); a fetch that
/// fails before the mailbox (no portal) is still one.
#[tokio::test(start_paused = true)]
async fn the_last_run_is_the_last_fetch() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (fetch, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &scan_only(dir.path()),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let last = || last_run(&store).unwrap().unwrap();
    assert_eq!((last().run, last().kind), (fetch.run, RunKindName::Fetch));
    let key = crate::portal::job_link("https://www.linkedin.com/jobs/view/4999000002/")
        .unwrap()
        .key;
    for kind in [RunKind::Details { keys: vec![key] }, RunKind::Rescore] {
        let (s, _) = go(
            &mut DemoBackends,
            &store,
            &RunRequest { kind },
            &ctx(dir.path(), false),
            &CancellationToken::new(),
            &c,
        )
        .await;
        assert!(s.run > fetch.run && s.outcome == Outcome::Completed);
        assert_eq!(last().run, fetch.run, "{:?} is no fetch", s.kind);
    }
    let mut none = ctx(dir.path(), false);
    none.portals.clear();
    let (failed, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &none,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(failed.outcome, Outcome::Failed { .. }));
    assert_eq!(last().run, failed.run, "a failed fetch is the last fetch");
    let (full, _) = go(
        &mut DemoBackends,
        &store,
        &RunRequest {
            kind: RunKind::FullMailbox,
        },
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(
        (last().run, last().kind),
        (full.run, RunKindName::FullMailbox)
    );
}

/// What the run card counts, from the run itself: its new jobs (first seen in it, a
/// duplicate once, excluded ones left out) and how many of them fit well - not a capped top
/// list, not the scan's count with the excluded ones. Rows of the run say whether a job is
/// new in it.
#[tokio::test(start_paused = true)]
async fn a_fetch_counts_its_new_jobs() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (s, events) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let mut expected = NewJobs::default();
    let first_seen = store
        .jobs(&JobFilter {
            first_seen_run: Some(s.run),
            ..JobFilter::default()
        })
        .unwrap();
    for job in &first_seen {
        let status = job.match_.as_ref().map(|m| (m.status, m.score));
        if store.dup_of(&job.key).unwrap().is_some()
            || matches!(status, Some((crate::model::MatchStatus::Excluded, _)))
        {
            continue;
        }
        expected.count += 1;
        if matches!(status, Some((crate::model::MatchStatus::Scored, points))
            if points >= crate::model::HIGH_FROM)
        {
            expected.high += 1;
        }
    }
    assert_eq!(s.new_jobs, Some(expected));
    let scan = s.scan.as_ref().unwrap();
    assert!(expected.high >= 1, "the sample has a job that fits well");
    assert!(
        expected.count < scan.new,
        "the excluded job is no new job of the card: {expected:?} vs {} new",
        scan.new
    );
    let updated: Vec<bool> = events
        .iter()
        .filter_map(|e| match e {
            RunEvent::JobUpdated { fresh, .. } => Some(*fresh),
            _ => None,
        })
        .collect();
    assert!(!updated.is_empty() && updated.iter().all(|fresh| *fresh));
    // The stored "last fetch" has the numbers too (the run card after a restart).
    assert_eq!(last_run(&store).unwrap().unwrap().new_jobs, Some(expected));

    // Details of an older job: no mailbox, no new jobs, and its row is no new one.
    let key = first_seen
        .iter()
        .find(|job| job.desc_status != DescStatus::Ok)
        .map(|job| job.key.clone())
        .expect("a job without details");
    let (d, events) = go(
        &mut DemoBackends,
        &store,
        &RunRequest {
            kind: RunKind::Details { keys: vec![key] },
        },
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(d.new_jobs, None);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, RunEvent::JobUpdated { fresh: true, .. }))
    );
    let (r, _) = go(
        &mut DemoBackends,
        &store,
        &RunRequest {
            kind: RunKind::Rescore,
        },
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(r.new_jobs, None);
}

/// Summaries stored by an earlier version (without `newJobs`) are still read.
#[test]
fn an_older_stored_summary_still_reads() {
    let store = Store::in_memory().unwrap();
    let summary = RunSummary::new(RunKindName::Fetch, false, Timestamp::now());
    let mut json = serde_json::to_value(&summary).unwrap();
    json.as_object_mut().unwrap().remove("newJobs");
    store.kv_set(LAST_RUN, &json.to_string()).unwrap();
    assert_eq!(last_run(&store).unwrap(), Some(summary));
}

/// A failed export does not fail the run, but the summary names it as a code with its
/// target - the page says it (the HTML overview cannot be written where a folder sits).
#[tokio::test(start_paused = true)]
async fn a_failed_export_is_reported_as_a_code() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let html = export::overview_html_path(&dir.path().join(RESULT_DIR));
    std::fs::create_dir_all(html.join("blocked")).unwrap();
    let (store, _) = store_with_texts();
    let (s, events) = go(
        &mut DemoBackends,
        &store,
        &RunRequest {
            kind: RunKind::Rescore,
        },
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(
        s.outcome,
        Outcome::Completed,
        "the export never fails the run"
    );
    let error = s.export.as_ref().unwrap().error.as_ref().unwrap();
    assert_eq!(error.params["target"], "overviewHtml");
    assert!(
        matches!(error.kind, ErrorKind::Io | ErrorKind::FileLocked),
        "{error:?}"
    );
    let Some(RunEvent::Finished { summary }) = events.last() else {
        panic!("the last event is the end");
    };
    assert_eq!(summary.export.as_ref().unwrap().error.as_ref(), Some(error));
}

/// The Excel overview open in Excel (Windows: no sharing): the run completes, the file stays
/// as it was, and the summary says `fileLocked` for the overview.
#[cfg(windows)]
#[tokio::test(start_paused = true)]
async fn an_open_excel_file_is_reported_as_locked() {
    use std::os::windows::fs::OpenOptionsExt;
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_with_texts();
    let rescore = RunRequest {
        kind: RunKind::Rescore,
    };
    let (first, _) = go(
        &mut DemoBackends,
        &store,
        &rescore,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let xlsx = first
        .export
        .as_ref()
        .unwrap()
        .overview_xlsx
        .clone()
        .unwrap();
    let before = std::fs::read(&xlsx).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&xlsx)
        .unwrap();
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &rescore,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    drop(lock);
    assert_eq!(s.outcome, Outcome::Completed);
    let export = s.export.as_ref().unwrap();
    let error = export.error.as_ref().unwrap();
    assert_eq!(
        (error.kind, &error.params["target"]),
        (ErrorKind::FileLocked, &serde_json::json!("overview"))
    );
    assert_eq!(export.overview_xlsx, None);
    assert_eq!(std::fs::read(&xlsx).unwrap(), before, "the open file stays");
}

/// Rows of the Excel overview's job sheet (with the header).
fn overview_rows(workspace: &Path) -> usize {
    use calamine::{Reader, Xlsx, open_workbook};
    let path = export::overview_path(&workspace.join(RESULT_DIR));
    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    book.worksheet_range(texts::JOBS_SHEET)
        .unwrap()
        .rows()
        .count()
}

/// A job deleted for good takes its text file and its Excel row along, and the next run
/// never brings it back from the old alert mail.
#[tokio::test(start_paused = true)]
async fn a_deleted_job_leaves_its_files_and_stays_gone() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let fetch = ctx(dir.path(), false);
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &fetch,
        &CancellationToken::new(),
        &c,
    )
    .await;
    let listed = |store: &Store| {
        let filter = JobFilter {
            listed: true,
            ..JobFilter::default()
        };
        store.jobs(&filter).unwrap().len()
    };
    assert_eq!(overview_rows(dir.path()), 1 + listed(&store));
    let total = store.job_count().unwrap();
    let victim = store
        .jobs(&JobFilter::default())
        .unwrap()
        .into_iter()
        .find(|job| job.txt_name.is_some())
        .unwrap();
    let file = dir
        .path()
        .join(RESULT_DIR)
        .join(TXT_DIR)
        .join(victim.txt_name.as_deref().unwrap());
    assert!(file.exists());
    let rows = overview_rows(dir.path());
    let one = std::slice::from_ref(&victim.key);
    // Only the trash is deleted for good.
    let kept = delete_jobs(&store, Some(dir.path()), None, one, (c(), Language::De)).unwrap();
    assert_eq!(kept.count, 0);
    assert!(file.exists());
    store.move_jobs(one, Place::Trash, c()).unwrap();
    let rows = rows - 1;
    let deleted = delete_jobs(&store, Some(dir.path()), None, one, (c(), Language::De)).unwrap();
    assert_eq!(deleted.export_error, None);
    assert_eq!(deleted.count, 1, "the one row the user deleted");
    let gone = i64::try_from(deleted.keys.len()).unwrap();
    assert!(!file.exists());
    assert!(overview_rows(dir.path()) <= rows);
    assert_eq!(overview_rows(dir.path()), 1 + listed(&store));
    assert_eq!(store.job_count().unwrap(), total - gone);
    let files = txt_files(dir.path());
    // The next run reads the same alert mails: the job stays deleted.
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &fetch,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(s.scan.as_ref().unwrap().new, 0);
    assert!(store.job(&victim.key).unwrap().is_none());
    assert_eq!(store.job_count().unwrap(), total - gone);
    assert_eq!(txt_files(dir.path()), files);
    assert!(!file.exists());
}

/// At the end of a run old jobs without a stage archive themselves (by the days in the
/// settings; 0 = never); a saved one stays.
#[tokio::test(start_paused = true)]
async fn a_run_archives_old_jobs_without_a_stage() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let archiving = RunContext {
        auto_archive_days: 30,
        ..ctx(dir.path(), true)
    };
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &archiving,
        &CancellationToken::new(),
        &c,
    )
    .await;
    let archived = |store: &Store| -> usize {
        store
            .jobs(&JobFilter::default())
            .unwrap()
            .iter()
            .filter(|job| job.archived_at.is_some())
            .count()
    };
    assert_eq!(archived(&store), 0, "young jobs stay");
    let jobs = store.jobs(&JobFilter::default()).unwrap();
    let saved = jobs[0].key.clone();
    store.set_pinned(&saved, true, c()).unwrap();
    let later = move || c() + SignedDuration::from_hours(24 * 40);
    let off = RunContext {
        auto_archive_days: 0,
        ..ctx(dir.path(), true)
    };
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &off,
        &CancellationToken::new(),
        &later,
    )
    .await;
    assert_eq!(archived(&store), 0, "switched off");
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &archiving,
        &CancellationToken::new(),
        &later,
    )
    .await;
    let jobs = store.jobs(&JobFilter::default()).unwrap();
    assert!(jobs.len() > 1);
    for job in &jobs {
        assert_eq!(job.archived_at.is_none(), job.key == saved, "{}", job.key);
    }
}

/// The Excel sheet lists what the app lists: an archived job leaves it (and comes back when
/// listed again), a duplicate never has a row of its own.
#[tokio::test(start_paused = true)]
async fn the_excel_sheet_leaves_out_archived_jobs() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let rows = overview_rows(dir.path());
    let key = store.jobs(&JobFilter::default()).unwrap()[0].key.clone();
    let one = std::slice::from_ref(&key);
    store.move_jobs(one, Place::Archive, c()).unwrap();
    export_all(&store, dir.path(), &[], 2, c(), Language::De);
    assert_eq!(overview_rows(dir.path()), rows - 1);
    store.move_jobs(one, Place::Trash, c()).unwrap();
    export_all(&store, dir.path(), &[], 3, c(), Language::De);
    assert_eq!(overview_rows(dir.path()), rows - 1, "nor the trash");
    store.move_jobs(one, Place::Inbox, c()).unwrap();
    export_all(&store, dir.path(), &[], 3, c(), Language::De);
    assert_eq!(overview_rows(dir.path()), rows);
    let all = store.jobs(&JobFilter::default()).unwrap().len();
    let listed = store
        .jobs(&JobFilter {
            listed: true,
            ..JobFilter::default()
        })
        .unwrap()
        .len();
    assert_eq!(rows, 1 + listed, "the header and one row per listed job");
    assert!(listed <= all);
}

/// At the end of a run the trash empties itself of the jobs that lie there long enough
/// (0 = never): rows and text files go, a tombstone stays; the young trash stays.
#[tokio::test(start_paused = true)]
async fn a_run_empties_an_old_trash() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let fetch = ctx(dir.path(), false);
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &fetch,
        &CancellationToken::new(),
        &c,
    )
    .await;
    let jobs = store.jobs(&JobFilter::default()).unwrap();
    let old = jobs.iter().find(|j| j.txt_name.is_some()).unwrap().clone();
    let young = jobs.iter().find(|j| j.key != old.key).unwrap().key.clone();
    let file = dir
        .path()
        .join(RESULT_DIR)
        .join(TXT_DIR)
        .join(old.txt_name.as_deref().unwrap());
    let long_ago = c() - SignedDuration::from_hours(24 * 40);
    store
        .move_jobs(std::slice::from_ref(&old.key), Place::Trash, long_ago)
        .unwrap();
    store
        .move_jobs(std::slice::from_ref(&young), Place::Trash, c())
        .unwrap();
    // Switched off: nothing goes.
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &fetch,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(store.job(&old.key).unwrap().is_some());
    let emptying = RunContext {
        auto_empty_trash_days: 30,
        ..ctx(dir.path(), false)
    };
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &emptying,
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(store.job(&old.key).unwrap().is_none());
    assert!(store.is_deleted(&old.key).unwrap());
    assert!(!file.exists());
    assert_eq!(store.job(&young).unwrap().unwrap().place(), Place::Trash);
}

/// A fetch that brings nothing new keeps the unread matches in the HTML overview: it lists
/// the app's "Neu und passend", whatever run brought them, until they are read.
#[tokio::test(start_paused = true)]
async fn the_overview_keeps_the_unread_matches_of_earlier_fetches() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let fetch = ctx(dir.path(), false);
    let html_path = export::overview_html_path(&dir.path().join(RESULT_DIR));
    let html = || std::fs::read_to_string(&html_path).unwrap();
    let cancel = CancellationToken::new();
    go(&mut DemoBackends, &store, &request(), &fetch, &cancel, &c).await;
    assert!(html().contains("Interim CFO"));
    let (s, _) = go(&mut DemoBackends, &store, &request(), &fetch, &cancel, &c).await;
    assert_eq!(s.new_jobs.unwrap().count, 0, "nothing new");
    assert!(
        s.scan.unwrap().mails_checked > 0,
        "the mails were read again"
    );
    assert!(html().contains("Interim CFO"), "still unread, still listed");
    assert!(!html().contains(texts::HTML_EMPTY));
    let cfo = store
        .jobs(&JobFilter::default())
        .unwrap()
        .into_iter()
        .find(|job| job.title.starts_with("Interim CFO"))
        .unwrap();
    store.mark_read(&cfo.key, c()).unwrap();
    export_all(&store, dir.path(), &[], 3, c(), Language::De);
    assert!(!html().contains("Interim CFO"), "read, it leaves");
}

/// A mark changes the small files without a run: a job moved to the trash leaves the HTML
/// overview and the skill's `top_matches.json` at once, a new favourite shows; the Excel
/// file waits for the next run.
#[tokio::test(start_paused = true)]
async fn a_mark_refreshes_the_overview_and_the_top_matches() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let cancel = CancellationToken::new();
    go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &cancel,
        &c,
    )
    .await;
    let result_dir = dir.path().join(RESULT_DIR);
    let html = || std::fs::read_to_string(export::overview_html_path(&result_dir)).unwrap();
    let top = || std::fs::read_to_string(result_dir.join(export::TOP_MATCHES_NAME)).unwrap();
    let xlsx = || std::fs::read(export::overview_path(&result_dir)).unwrap();
    let excel = xlsx();
    let jobs = store.jobs(&JobFilter::default()).unwrap();
    let cfo = jobs
        .iter()
        .find(|job| job.title.starts_with("Interim CFO"))
        .unwrap();
    assert!(html().contains("Interim CFO") && top().contains("Interim CFO"));
    let matcher = demo::matcher();
    let refresh = || {
        refresh_exports(
            &store,
            dir.path(),
            Some(&*matcher as &dyn Matcher),
            c(),
            Language::De,
        );
    };
    store
        .move_jobs(std::slice::from_ref(&cfo.key), Place::Trash, c())
        .unwrap();
    refresh();
    assert!(
        !html().contains("Interim CFO"),
        "the trash leaves the overview"
    );
    assert!(!top().contains("Interim CFO"), "and the skill's list");
    assert_eq!(xlsx(), excel, "the Excel file waits for the next run");
    let controlling = jobs
        .iter()
        .find(|job| job.title.starts_with("Leiter Controlling"))
        .unwrap();
    store.set_pinned(&controlling.key, true, c()).unwrap();
    refresh();
    assert!(
        html().contains(texts::HTML_PINNED),
        "the new favourite shows"
    );
    assert!(
        html().contains(texts::HTML_NEW) && html().contains("SAP FI/CO Berater"),
        "and the other unread matches stay listed beside it"
    );
    let path = refresh_overview(&store, dir.path(), c(), Language::De).unwrap();
    assert_eq!(path, export::overview_html_path(&result_dir));
}

/// One job two portals announced, as the list shows it: the freelancermap row with the
/// LinkedIn duplicate behind it. `(original, duplicate)`.
fn job_on_two_portals(store: &Store) -> (JobKey, JobKey) {
    const AD: &str = "Für unseren Kunden suchen wir einen erfahrenen SAP FI/CO Berater. \
        Aufgaben: Einführung von S/4HANA Finance, Abstimmung mit den Fachbereichen.";
    let run = store.begin_run().unwrap();
    let add = |url: &str| {
        let link = crate::portal::job_link(url).unwrap();
        let posting = crate::model::Posting::new(
            link.key.clone(),
            link.url,
            "SAP FI/CO Berater (m/w/d)",
            "Ferrum Systems SE",
            "Hamburg",
        );
        let mail = crate::store::MailRef {
            subject: "Neue Jobs",
            date: None,
            gmail_id: None,
        };
        store
            .upsert_posting(run, &posting, mail, Timestamp::now())
            .unwrap();
        store
            .record_text(&link.key, AD, false, false, Timestamp::now())
            .unwrap();
        link.key
    };
    let original = add("https://www.freelancermap.de/nproj/12345.html");
    let duplicate = add("https://www.linkedin.com/jobs/view/4000000002/");
    assert_eq!(
        store.link_duplicate(&duplicate).unwrap(),
        Some(original.clone())
    );
    (original, duplicate)
}

/// "Endgültig löschen" counts the jobs the list showed: a duplicate that stood behind the
/// row goes with it (its key comes back for the page) but is no job of its own.
#[test]
fn a_purge_counts_the_rows_it_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let (original, duplicate) = job_on_two_portals(&store);
    let now = Timestamp::now();
    let one = std::slice::from_ref(&original);
    store.move_jobs(one, Place::Trash, now).unwrap();
    let deleted = delete_jobs(&store, Some(dir.path()), None, one, (now, Language::De)).unwrap();
    assert_eq!(deleted.count, 1, "one row");
    assert_eq!(deleted.keys.len(), 2);
    assert!(
        deleted.keys.contains(&duplicate),
        "the page drops both keys"
    );
    assert!(store.txt_leftovers().unwrap().is_empty());
    // Emptying the whole trash counts the same way.
    let (store, keys) = store_with_texts();
    store.move_jobs(&keys, Place::Trash, now).unwrap();
    let all = store.trashed_keys(None).unwrap();
    let deleted = delete_jobs(&store, Some(dir.path()), None, &all, (now, Language::De)).unwrap();
    assert_eq!(deleted.count, 2);
}

/// The text file of a job deleted for good that could not be removed is not forgotten with
/// its row: "Textdateien löschen" and a reset still find it, and the next export removes it.
#[test]
fn a_text_file_that_stayed_is_removed_later() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_with_texts();
    let now = Timestamp::now();
    let result_dir = dir.path().join(RESULT_DIR);
    let txt_dir = result_dir.join(TXT_DIR);
    std::fs::create_dir_all(&txt_dir).unwrap();
    let (stayed, gone) = (
        "20260901_LinkedIn_Rolle_4000000007.txt".to_string(),
        "20260901_LinkedIn_Rolle_4000000008.txt".to_string(),
    );
    std::fs::write(txt_dir.join(&stayed), b"alt").unwrap();
    store
        .set_txt_leftovers(&[stayed.clone(), gone.clone()])
        .unwrap();
    assert!(store.txt_names().unwrap().contains(&stayed), "still known");
    assert_eq!(
        export::txt_files(&result_dir, &store.txt_names().unwrap()).len(),
        1
    );
    export_all(&store, dir.path(), &[], 1, now, Language::De);
    assert!(
        !txt_dir.join(&stayed).exists(),
        "removed with the next export"
    );
    assert!(store.txt_leftovers().unwrap().is_empty(), "and forgotten");
    // "Textdateien löschen" takes such a file along as well.
    std::fs::write(txt_dir.join(&stayed), b"alt").unwrap();
    store
        .set_txt_leftovers(std::slice::from_ref(&stayed))
        .unwrap();
    let (removed, failed) = clear_txt(&store, dir.path()).unwrap();
    assert!(failed.is_empty() && removed >= 1);
    assert!(!txt_dir.join(&stayed).exists());
    assert!(store.txt_leftovers().unwrap().is_empty());
}

/// A text file open in another program (Windows: without delete sharing, as Word holds it)
/// stays when its job is deleted for good; it is remembered, and the next export removes it.
#[cfg(windows)]
#[test]
fn an_open_text_file_of_a_deleted_job_is_remembered_and_removed_later() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    let now = Timestamp::now();
    export_all(&store, dir.path(), &[], 1, now, Language::De);
    let name = store.job(&keys[0]).unwrap().unwrap().txt_name.unwrap();
    let file = dir.path().join(RESULT_DIR).join(TXT_DIR).join(&name);
    let one = std::slice::from_ref(&keys[0]);
    store.move_jobs(one, Place::Trash, now).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&file)
        .unwrap();
    let deleted = delete_jobs(&store, Some(dir.path()), None, one, (now, Language::De)).unwrap();
    assert_eq!(deleted.count, 1);
    assert!(file.exists());
    assert_eq!(store.txt_leftovers().unwrap(), std::slice::from_ref(&name));
    assert!(store.txt_names().unwrap().contains(&name));
    drop(lock);
    export_all(&store, dir.path(), &[], 2, now, Language::De);
    assert!(!file.exists());
    assert!(store.txt_leftovers().unwrap().is_empty());
}
