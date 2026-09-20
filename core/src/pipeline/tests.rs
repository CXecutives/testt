//! Ein-Klick-Lauf mit den Trockenlauf-Attrappen; simulierte Zeit (Tempo kostet nichts).

use std::path::Path;
use std::sync::Mutex;

use jiff::SignedDuration;
use tokio::time::Instant;

use super::demo::{DemoBackends, DemoMail, DemoPages};
use super::*;
use crate::export::TXT_DIR;
use crate::model::DescStatus;

fn clock() -> impl Fn() -> Timestamp {
    let base = Timestamp::now();
    let start = Instant::now();
    move || base + SignedDuration::try_from(start.elapsed()).unwrap()
}

fn request() -> RunRequest {
    RunRequest {
        scan: true,
        fetch: true,
        export: true,
        scope: Scope::New,
        portals: Portal::ALL.to_vec(),
        jobs: None,
    }
}

fn ctx(workspace: &Path, dry_run: bool) -> RunContext {
    RunContext {
        workspace: workspace.to_path_buf(),
        account: "ich@gmail.com".into(),
        dry_run,
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

#[tokio::test(start_paused = true)]
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
    let scan = s.scan.as_ref().unwrap();
    assert_eq!((scan.alert_mails, scan.new), (3, 5));
    let fetch = s.fetch.as_ref().unwrap();
    let ok: usize = fetch.per_portal.values().map(|p| p.ok).sum();
    assert_eq!(ok, 4, "freelance.de zeigt im Trockenlauf einen Portalstopp");
    let freelance = &fetch.per_portal[&Portal::FreelanceDe];
    assert_eq!(freelance.skipped, 1);
    assert!(
        freelance
            .stop
            .as_deref()
            .is_some_and(|t| t.starts_with("freelance.de: pausiert bis"))
    );
    let export = s.export.as_ref().unwrap();
    assert_eq!(export.txt_written, 4);
    assert!(export.overview.as_ref().unwrap().exists());
    assert_eq!(txt_files(dir.path()), 4);
    assert_eq!(finished(&events), 1);
    // Jedes Ereignis bleibt unter 8 KB (größere parkt der Tauri-Channel in einer
    // Warteschlange, die jede Seite abrufen könnte).
    for event in &events {
        let size = serde_json::to_vec(event).unwrap().len();
        assert!(size < 8 * 1024, "{size} Bytes: {event:?}");
    }
    assert!(store.kv_get(LAST_RUN).unwrap().is_some());
    // Die Oberfläche sieht Wartezeiten (Countdown), jede Phase und den fertigen Stopptext.
    let statuses: Vec<(&str, bool)> = events
        .iter()
        .filter_map(|e| match e {
            RunEvent::Status { text, until } => Some((text.as_str(), until.is_some())),
            _ => None,
        })
        .collect();
    for phase in [
        "Gmail wird verbunden…",
        "Postfach wird durchsucht…",
        "Mails werden gelesen…",
        "Jobdetails werden geholt (freelancermap.de)…",
        "Jobdetails werden geholt (LinkedIn)…",
        "Jobdetails werden geholt (freelance.de)…",
        "Ergebnisdateien werden geschrieben…",
    ] {
        assert!(statuses.contains(&(phase, false)), "{phase}: {statuses:?}");
    }
    let waits: Vec<_> = statuses.iter().filter(|(_, until)| *until).collect();
    assert!(!waits.is_empty());
    assert!(
        waits
            .iter()
            .all(|(text, _)| text.starts_with("Pause vor dem nächsten Abruf ("))
    );
    // Nach einer Wartezeit folgt wieder eine Tätigkeit – dazwischen dürfen andere Portale
    // ebenfalls warten, sie laufen ja nebeneinander. Die Statuszeile bleibt nie in der Pause
    // stehen.
    for (i, _) in statuses.iter().enumerate().filter(|(_, (_, wait))| *wait) {
        let next = statuses[i + 1..].iter().find(|(_, wait)| !wait);
        assert!(
            next.is_some_and(|(text, _)| text.starts_with("Jobdetails werden geholt")
                || text.starts_with("Anmeldung bei")
                || text.starts_with("Ergebnisdateien")),
            "{statuses:?}"
        );
    }
    let status_json = serde_json::to_value(status("x")).unwrap();
    assert!(
        status_json.get("until").is_none(),
        "ohne Wartezeit weggelassen"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        RunEvent::PortalStopped { portal: Portal::FreelanceDe, skipped: 1, text }
            if text == freelance.stop.as_deref().unwrap()
    )));

    // Zweiter Lauf: nichts neu, keine Textdatei doppelt; Übersicht neu (neuer Lauf).
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
    // Ohne Änderung wird die Übersicht nicht angefasst (offene Excel-Datei stört dann nicht).
    let again = export_all(&store, dir.path(), &[], s.run, c());
    assert_eq!(again.overview, None);
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
        "Geholtes ist gespeichert und exportiert"
    );
    assert_eq!(finished(&events), 1);
}

/// Scheitert das Postfach, wird nicht abgerufen – exportiert wird trotzdem.
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
    assert!(matches!(&s.outcome, Outcome::Failed { error, .. } if error == "mailAuth"));
    assert!(s.fetch.is_none());
    assert!(s.export.is_some());
    assert_eq!(finished(&events), 1);
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
async fn a_source_must_be_chosen() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let mut req = request();
    req.portals.clear();
    let (s, _) = go(
        &mut DemoBackends,
        &store,
        &req,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(&s.outcome, Outcome::Failed { error, .. } if error == "invalid"));
}

/// „Details holen“ für einzelne Jobs: nur diese, ohne Postfach.
#[tokio::test(start_paused = true)]
async fn targeted_fetch_only_touches_the_chosen_jobs() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let mut scan_only = request();
    scan_only.fetch = false;
    go(
        &mut DemoBackends,
        &store,
        &scan_only,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let key = crate::portal::job_link("https://www.linkedin.com/jobs/view/4999000002/")
        .unwrap()
        .key;
    let targeted = RunRequest {
        scan: true,
        jobs: Some(vec![key.clone()]),
        ..request()
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
    assert!(s.scan.is_none(), "gezielter Abruf ohne Postfach");
    assert_eq!(s.fetch.as_ref().unwrap().queued, 1);
    assert_eq!(
        store.job(&key).unwrap().unwrap().desc_status,
        DescStatus::Ok
    );
}

/// Abbruch nach k von n Jobdetails ⇒ genau k gespeichert, genau ein `Finished`.
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
                if updated >= k && matches!(e, RunEvent::JobUpdated { .. } | RunEvent::Log { .. }) {
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

/// Zwei Jobs mit Volltext (ohne Postfach), für Export-Tests.
fn store_with_texts() -> (Store, Vec<JobKey>) {
    fill_texts(Store::in_memory().unwrap())
}

/// Dieselben zwei Jobs in einer Datenbank auf der Platte – nur so lässt sich ein
/// Datenbankfehler von außen erzeugen (zweite Verbindung).
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

/// Eine fremde Übersicht (etwa vom alten Programm) wird einmal gesichert – die eigene bei
/// jedem weiteren Lauf nie.
#[test]
fn only_a_foreign_overview_is_backed_up_and_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_with_texts();
    let result_dir = dir.path().join(RESULT_DIR);
    std::fs::create_dir_all(&result_dir).unwrap();
    std::fs::write(result_dir.join(export::XLSX_NAME), b"fremd").unwrap();
    let now = Timestamp::now();

    let first = export_all(&store, dir.path(), &[], 1, now);
    let backup = first.backup.clone().expect("fremde Datei gesichert");
    assert_eq!(std::fs::read(&backup).unwrap(), b"fremd");
    assert!(first.overview.is_some());
    let mut events = Vec::new();
    log_export(&first, &mut |e| events.push(e));
    assert!(events.iter().any(|e| matches!(
        e,
        RunEvent::Log { level: Level::Warn, text, .. } if text.contains("gesichert")
    )));

    // Weitere Läufe schreiben die eigene Datei fort, ohne sie noch einmal zu sichern.
    let second = export_all(&store, dir.path(), &[], 2, now);
    let back = export_all(&store, dir.path(), &[], 3, now);
    assert_eq!((second.backup, back.backup), (None, None));
    assert!(back.overview.is_some(), "neuer Lauf: Blatt „Info“ neu");
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
    // Ohne neuen Lauf und ohne neue Daten bleibt die Übersicht liegen.
    let again = export_all(&store, dir.path(), &[], 3, now);
    assert_eq!(again.overview, None);
}

/// Lässt sich der Exportstand nicht lesen, ist der Besitz der Datei unbekannt: Die
/// vorhandene Übersicht bleibt liegen – kein Backup, kein Überschreiben.
#[test]
fn an_unreadable_export_stamp_leaves_the_overview_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (store, db) = store_on_disk(dir.path());
    let now = Timestamp::now();
    let first = export_all(&store, dir.path(), &[], 1, now);
    assert!(first.overview.is_some() && first.backup.is_none());
    let path = dir.path().join(RESULT_DIR).join(export::XLSX_NAME);
    let before = std::fs::read(&path).unwrap();

    // Die Tabelle mit dem Exportstand fehlt: jedes `kv_get` scheitert.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute("DROP TABLE kv", [])
        .unwrap();
    let again = export_all(&store, dir.path(), &[], 2, now);
    assert_eq!(again.backup, None, "kein Backup bei unbekanntem Besitz");
    assert_eq!(again.overview, None);
    assert_eq!(std::fs::read(&path).unwrap(), before, "Datei unverändert");
    assert!(again.error.is_some(), "{again:?}");
    let backups = std::fs::read_dir(dir.path().join(RESULT_DIR))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".alt-"))
        .count();
    assert_eq!(backups, 0);
}

/// Lässt sich eine geschriebene Textdatei nicht markieren, zählen auch die übrigen als
/// offen – und die erste, ursachennächste Meldung bleibt stehen.
#[test]
fn a_failed_mark_counts_the_rest_and_keeps_the_first_error() {
    let dir = tempfile::tempdir().unwrap();
    let (store, db) = store_on_disk(dir.path());
    // Keine Änderung an der Jobtabelle: Lesen geht, Markieren scheitert.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "CREATE TRIGGER kein_markieren BEFORE UPDATE ON job
             BEGIN SELECT RAISE(ABORT, 'gesperrt'); END",
            [],
        )
        .unwrap();
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now());
    assert_eq!(
        (summary.txt_written, summary.txt_failed_count),
        (0, 2),
        "{summary:?}"
    );
    // Zur Zahl gehört wenigstens ein Beispiel.
    assert!(!summary.txt_failed.is_empty(), "{summary:?}");
    assert!(summary.error.is_some(), "{summary:?}");
}

/// Scheitern Textdateien und Übersicht am selben unbrauchbaren Ordner, meldet der Lauf die
/// erste, ursachennächste Meldung – nicht den nackten Folgefehler der Übersicht.
#[test]
fn the_first_error_survives_a_later_one() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_with_texts();
    // An der Stelle des Ergebnisordners steht eine Datei: nichts lässt sich dort schreiben.
    std::fs::write(dir.path().join(RESULT_DIR), b"kein Ordner").unwrap();
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now());
    assert_eq!((summary.txt_written, summary.txt_failed_count), (0, 2));
    assert_eq!(summary.overview, None);
    let error = summary.error.as_deref().unwrap_or_default();
    assert!(
        error.contains(TXT_DIR) && error.contains("nicht erreichbar"),
        "{error}"
    );
}

/// Ein gescheiterter Postfach-Abruf überschreibt die Zahlen des letzten guten nicht.
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
    let mut good = request();
    good.fetch = false;
    go(
        &mut DemoBackends,
        &store,
        &good,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    let after_good = store.kv_get(LAST_SCAN_INFO).unwrap().unwrap();
    assert!(after_good.contains("ich@gmail.com") && after_good.contains("\"5\""));
    let (failed, _) = go(
        &mut Failing,
        &store,
        &good,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(failed.outcome, Outcome::Failed { .. }));
    let details_only = RunRequest {
        scan: false,
        ..request()
    };
    go(
        &mut DemoBackends,
        &store,
        &details_only,
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert_eq!(store.kv_get(LAST_SCAN_INFO).unwrap().unwrap(), after_good);
    let rows = info_rows(&store, c());
    assert!(
        rows.iter()
            .any(|(k, v)| k == "Neu (letzter Lauf)" && v == "5")
    );
}

/// Sicherheits-Invariante: Eine vom Nutzer gelöschte oder geleerte Textdatei legt der
/// nächste Lauf nicht wieder an – die Marke bleibt verbraucht. Zurück holt sie nur
/// „Textdateien neu schreiben“.
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
    assert_eq!((next.txt_written, next.txt_failed_count), (0, 0));
    assert!(!deleted.exists(), "gelöschte Datei bleibt weg");
    assert!(
        std::fs::read(&emptied).unwrap().is_empty(),
        "leer bleibt leer"
    );
    // Erst der ausdrückliche Befehl holt sie zurück.
    assert_eq!(rewrite_txt(&store, dir.path(), now).txt_written, 2);
    assert!(deleted.exists() && !std::fs::read(&emptied).unwrap().is_empty());
}

/// „Textdateien neu schreiben“: Was sich nicht schreiben lässt, behält seine Marke – der
/// nächste Lauf legt es also nicht von selbst neu an.
#[test]
fn a_failed_rewrite_keeps_the_marks() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    let now = Timestamp::now();
    let first = export_all(&store, dir.path(), &[], 1, now);
    assert_eq!(first.txt_written, 2);
    let txt_dir = dir.path().join(RESULT_DIR).join(TXT_DIR);
    let blocked = txt_dir.join(store.job(&keys[1]).unwrap().unwrap().txt_name.unwrap());
    // An der Stelle der zweiten Datei steht ein Ordner: Schreiben scheitert.
    std::fs::remove_file(&blocked).unwrap();
    std::fs::create_dir(&blocked).unwrap();

    let rewrite = rewrite_txt(&store, dir.path(), now);
    assert_eq!((rewrite.txt_written, rewrite.txt_failed_count), (1, 1));
    assert_eq!(rewrite.txt_failed, [keys[1].to_string()]);
    // Keine Marke wurde gelöscht: Nichts gilt als „noch zu schreiben“.
    assert!(store.txt_jobs(false).unwrap().is_empty());
    let next = export_all(&store, dir.path(), &[], 2, now);
    assert_eq!(
        (next.txt_written, next.txt_failed_count),
        (0, 0),
        "nichts von selbst neu"
    );
}

/// Ist der Ordner für die Textdateien unbrauchbar, gibt es eine klare Meldung statt eines
/// Versuchs je Datei – und die echte Zahl.
#[test]
fn an_unusable_text_folder_is_one_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keys) = store_with_texts();
    let result_dir = dir.path().join(RESULT_DIR);
    std::fs::create_dir_all(&result_dir).unwrap();
    std::fs::write(result_dir.join(TXT_DIR), b"eine Datei statt des Ordners").unwrap();
    let summary = export_all(&store, dir.path(), &[], 1, Timestamp::now());
    assert_eq!((summary.txt_written, summary.txt_failed_count), (0, 2));
    // Auch hier nennt die Liste die betroffenen Jobs – sie bleibt nie leer neben einer Zahl.
    let mut failed = summary.txt_failed.clone();
    failed.sort();
    let mut expected: Vec<String> = keys.iter().map(ToString::to_string).collect();
    expected.sort();
    assert_eq!(failed, expected);
    assert!(
        summary
            .error
            .as_deref()
            .is_some_and(|e| e.contains(TXT_DIR)),
        "{summary:?}"
    );
    let mut events = Vec::new();
    log_export(&summary, &mut |e| events.push(e));
    assert!(events.iter().any(|e| matches!(
        e,
        RunEvent::Log { text, .. } if text.starts_with("2 Textdateien")
    )));
}

/// Lässt sich kein Lauf anlegen (Datenbank gesperrt oder defekt), endet er als Fehler – ohne
/// Postfach, ohne Abruf, mit genau einem `Finished`.
#[tokio::test(start_paused = true)]
async fn a_run_that_cannot_begin_fails() {
    let c = clock();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    store.kv_set("run_seq", "kaputt").unwrap();
    let (s, events) = go(
        &mut DemoBackends,
        &store,
        &request(),
        &ctx(dir.path(), false),
        &CancellationToken::new(),
        &c,
    )
    .await;
    assert!(matches!(&s.outcome, Outcome::Failed { error, .. } if error == "corrupt"));
    assert!(s.scan.is_none() && s.fetch.is_none() && s.export.is_none());
    assert_eq!(s.run, 0);
    assert_eq!(finished(&events), 1);
    assert_eq!(store.job_count().unwrap(), 0);
}
