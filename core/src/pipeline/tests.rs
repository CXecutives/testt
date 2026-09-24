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
use crate::model::DescStatus;

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
        account: "ich@gmail.com".into(),
        dry_run,
        portals: Portal::ALL.to_vec(),
        fetch_portals: Portal::ALL.to_vec(),
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
    assert_eq!(s.score, None);
    let export = s.export.as_ref().unwrap();
    assert_eq!(export.txt_written, 4);
    assert!(export.overview_xlsx.as_ref().unwrap().exists());
    assert_eq!(export.overview_html, None);
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
        RunEvent::PortalHealth { portal: Portal::FreelanceDe, health: h } if *h == health
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
    let again = export_all(&store, dir.path(), &[], s.run, c());
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
        fn pages(&mut self, _portal: Portal) -> Result<DemoPages, String> {
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
    let now = Timestamp::now();

    let first = export_all(&store, dir.path(), &[], 1, now);
    let backup = first.backup.clone().expect("foreign file backed up");
    assert_eq!(std::fs::read(&backup).unwrap(), b"fremd");
    assert!(first.overview_xlsx.is_some());

    // Further runs continue the app's own file without backing it up again.
    let second = export_all(&store, dir.path(), &[], 2, now);
    let back = export_all(&store, dir.path(), &[], 3, now);
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
    let again = export_all(&store, dir.path(), &[], 3, now);
    assert_eq!(again.overview_xlsx, None);
}

/// If the export stamp cannot be read, the ownership of the file is unknown: the existing
/// overview stays - no backup, no overwrite.
#[test]
fn an_unreadable_export_stamp_leaves_the_overview_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (store, db) = store_on_disk(dir.path());
    let now = Timestamp::now();
    let first = export_all(&store, dir.path(), &[], 1, now);
    assert!(first.overview_xlsx.is_some() && first.backup.is_none());
    let path = dir.path().join(RESULT_DIR).join(export::XLSX_NAME);
    let before = std::fs::read(&path).unwrap();

    // The table with the export stamp is missing: every `kv_get` fails.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute("DROP TABLE kv", [])
        .unwrap();
    let again = export_all(&store, dir.path(), &[], 2, now);
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
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now());
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
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now());
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
        fn pages(&mut self, _portal: Portal) -> Result<DemoPages, String> {
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
    let after_good = store.kv_get(LAST_SCAN_INFO).unwrap().unwrap();
    assert!(after_good.contains("ich@gmail.com") && after_good.contains("\"5\""));
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
    assert_eq!(store.kv_get(LAST_SCAN_INFO).unwrap().unwrap(), after_good);
    let rows = info_rows(&store, c());
    assert!(rows.iter().any(|(k, v)| k == texts::INFO_NEW && v == "5"));
}

/// Safety invariant: a text file the user deleted or emptied is not recreated by the next
/// run - the mark stays used. Only "rewrite text files" brings it back.
#[test]
fn a_text_file_the_user_removed_is_never_recreated_by_itself() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    let now = Timestamp::now();
    assert_eq!(export_all(&store, dir.path(), &[], 1, now).txt_written, 2);
    let txt_dir = dir.path().join(RESULT_DIR).join(TXT_DIR);
    let name = |key| store.job(key).unwrap().unwrap().txt_name.unwrap();
    let (deleted, emptied) = (txt_dir.join(name(&keys[0])), txt_dir.join(name(&keys[1])));
    std::fs::remove_file(&deleted).unwrap();
    std::fs::write(&emptied, b"").unwrap();

    let next = export_all(&store, dir.path(), &[], 2, now);
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
    let first = export_all(&store, dir.path(), &[], 1, now);
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
    let next = export_all(&store, dir.path(), &[], 2, now);
    assert_eq!(
        (next.txt_written, next.txt_failed),
        (0, 0),
        "nothing anew by itself"
    );
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
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now());
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
