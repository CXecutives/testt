//! Ein Klick: Postfach → neue Jobs → Jobdetails → Export.
//!
//! Die Schritte bleiben einzeln auslösbar. Der Export läuft immer am Ende – auch nach
//! Abbruch, Portalstopp oder Fehler (er ist rein lokal). Jeder Lauf endet mit genau einem
//! `Finished`, das auch als `last_run_summary` gespeichert wird.

pub mod demo;

use std::future::Future;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::export::{self, RESULT_DIR, TXT_DIR, write_job_txt, write_xlsx};
use crate::fetch::policy::Policy;
use crate::fetch::{FetchEvent, FetchSummary, PageFetcher, Selection, fetch_all};
use crate::mail::imap::{MailError, MailSource};
use crate::mail::scan::{ScanError, ScanEvent, ScanSummary, Scope, scan};
use crate::portal::{JobKey, Portal};
use crate::store::{JobFilter, JobRow, Store};
use crate::text::{plural, truncate_chars};
use crate::time;
use crate::view::JobView;

/// Was die Oberfläche startet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub scan: bool,
    pub fetch: bool,
    pub export: bool,
    pub scope: Scope,
    /// Gewählte Portale (Klick-Reihenfolge).
    pub portals: Vec<Portal>,
    /// Gezielter Abruf („Details holen“) – dann nur diese Jobs.
    #[serde(default)]
    pub jobs: Option<Vec<JobKey>>,
}

/// Lauf-Einstellungen, die nicht aus der Seite kommen.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub workspace: PathBuf,
    /// Gmail-Adresse fürs Info-Blatt (leer, wenn unbekannt).
    pub account: String,
    /// Trockenlauf: nichts wird geschrieben.
    pub dry_run: bool,
}

/// Postfach und Abrufwege eines Laufs (in Tests und im Trockenlauf Attrappen).
pub trait Backends {
    type Mail: MailSource + Send;
    type Pages: PageFetcher + Send;
    fn connect_mail(
        &mut self,
        cancel: &CancellationToken,
    ) -> impl Future<Output = Result<Self::Mail, MailError>> + Send;
    fn pages(&mut self) -> Result<Self::Pages, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Scan,
    Fetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Info,
    Ok,
    Warn,
    Error,
}

/// Ereignisse an die Oberfläche. Jedes bleibt klein (< 8 KB): Volltexte und Jobzeilen holt
/// die Seite selbst (`list_jobs`, `job_detail`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RunEvent {
    Log {
        level: Level,
        text: String,
        /// Zeitpunkt der Meldung (auch nach einem Neuladen der Seite richtig).
        at: Timestamp,
    },
    /// Was gerade geschieht. `until`: Ende einer Wartezeit (die Oberfläche zeigt einen
    /// Countdown); sonst weggelassen.
    Status {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        until: Option<Timestamp>,
    },
    Progress {
        step: Step,
        done: usize,
        total: usize,
    },
    Alert {
        portal: Portal,
        subject: String,
        date: Option<Timestamp>,
        postings: usize,
        /// Gmail-Nachrichten-ID (hexadezimal) – geöffnet über `open_target`.
        gmail_id: Option<String>,
    },
    /// Ein Job hat einen neuen Stand – die fertige Tabellenzeile.
    JobUpdated {
        job: Box<JobView>,
    },
    /// Ein Portal ruht für den Rest des Laufs; `text` ist der fertige Stopptext.
    PortalStopped {
        portal: Portal,
        skipped: usize,
        text: String,
    },
    /// Anmeldung nötig: Das Sitzungsfenster ist offen (`waiting`) bzw. wieder zu.
    LoginNeeded {
        portal: Portal,
        waiting: bool,
    },
    Finished(Box<RunSummary>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outcome {
    Completed,
    Cancelled,
    Failed { error: String, message: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    /// Erzeugte Übersichtsdatei (wenn erzeugt).
    pub overview: Option<PathBuf>,
    /// Eine fremde Übersicht am selben Pfad wurde hierhin gesichert.
    pub backup: Option<PathBuf>,
    pub txt_written: usize,
    /// Zahl der Textdateien, die sich nicht schreiben ließen – die Zahl für jede Anzeige.
    pub txt_failed_count: usize,
    /// Beispiele dazu (die ersten höchstens 20 Schlüssel), nie zum Zählen: Die Liste ist
    /// gekappt und bleibt bei einem Fehler vor der ersten Datei kurz.
    pub txt_failed: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub run: i64,
    pub outcome: Outcome,
    pub dry_run: bool,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub scope: Option<Scope>,
    pub scan: Option<ScanSummary>,
    pub fetch: Option<FetchSummary>,
    pub export: Option<ExportSummary>,
}

/// Zusammenfassung des letzten Laufs (JSON).
pub const LAST_RUN: &str = "last_run_summary";
/// Nummer des letzten Laufs mit Postfach-Abruf.
const LAST_SCAN_RUN: &str = "last_scan_run";
/// Stand der Übersicht je Pfad (Schlüssel = Präfix + Pfad).
const EXPORT_STAMP: &str = "export:";
/// Blatt-„Info“-Zeilen des letzten erfolgreichen Postfach-Abrufs.
const LAST_SCAN_INFO: &str = "last_scan_info";
/// So viele Schlüssel nicht geschriebener Textdateien nennt die Zusammenfassung.
const MAX_FAILED_NAMES: usize = 20;

/// Führt einen Lauf aus. Datenbankfehler beenden ihn als `Failed`; der Export läuft trotzdem
/// – außer der Lauf ließ sich gar nicht erst anlegen.
#[expect(
    clippy::too_many_arguments,
    reason = "Lauf-Kontext: Speicher, Regeln, Auftrag, Abbruch, Uhr und Ereignisse einzeln (in Tests austauschbar)"
)]
pub async fn run<B: Backends>(
    backends: &mut B,
    store: &Store,
    policy: &mut Policy,
    request: &RunRequest,
    ctx: &RunContext,
    cancel: &CancellationToken,
    clock: impl Fn() -> Timestamp,
    mut emit: impl FnMut(RunEvent),
) -> RunSummary {
    let started_at = clock();
    let mut summary = RunSummary {
        run: 0,
        outcome: Outcome::Completed,
        dry_run: ctx.dry_run,
        started_at,
        finished_at: started_at,
        scope: request.scan.then_some(request.scope),
        scan: None,
        fetch: None,
        export: None,
    };
    // Ohne Laufnummer (Datenbank gesperrt oder defekt) lässt sich nichts zuordnen: dann
    // weder Postfach noch Abruf noch Export.
    let run = match store.begin_run() {
        Ok(run) => run,
        Err(e) => {
            summary.outcome = failed(e.kind(), &e.to_string());
            summary.finished_at = clock();
            emit(RunEvent::Finished(Box::new(summary.clone())));
            return summary;
        }
    };
    summary.run = run;
    let targeted = request.jobs.as_deref().filter(|j| !j.is_empty());

    if request.scan && targeted.is_none() {
        let before_scan = last_scan_run(store).unwrap_or(0);
        if request.portals.is_empty() {
            summary.outcome = failed("invalid", "Bitte mindestens ein Portal wählen.");
        } else if let Err(e) = store.kv_set(LAST_SCAN_RUN, &run.to_string()) {
            // „Neu in diesem Lauf“ zeigt die Jobs des letzten Laufs mit Postfach-Abruf.
            summary.outcome = failed(e.kind(), &e.to_string());
        } else {
            // Stand des letzten echten Abrufs merken: Wer das Postfach nie erreicht (falsches
            // App-Passwort, kein Netz, sofortiger Abbruch), darf ihn nicht leeren.
            let previous = before_scan;
            let mut scanned = ScanSummary::default();
            let result = scan_step(
                backends,
                store,
                request,
                run,
                started_at,
                cancel,
                &mut scanned,
                &mut emit,
            )
            .await;
            summary.outcome = match result {
                Ok(()) => {
                    remember_scan(store, ctx, request.scope, &scanned, started_at);
                    Outcome::Completed
                }
                Err(ScanError::Mail(MailError::Cancelled)) => Outcome::Cancelled,
                Err(ScanError::Mail(e)) => failed(e.kind(), &e.to_string()),
                Err(ScanError::Store(e)) => failed(e.kind(), &e.to_string()),
            };
            if scanned.mails_checked == 0 {
                let _ = store.kv_set(LAST_SCAN_RUN, &previous.to_string());
            }
            summary.scan = Some(scanned);
        }
    }

    if request.fetch && summary.outcome == Outcome::Completed {
        let mut fetched = FetchSummary::default();
        summary.outcome = fetch_step(
            backends,
            store,
            policy,
            request,
            targeted,
            cancel,
            &clock,
            &mut fetched,
            &mut emit,
        )
        .await;
        summary.fetch = Some(fetched);
    }

    summary.finished_at = clock();
    if request.export && !ctx.dry_run {
        emit(status("Ergebnisdateien werden geschrieben…"));
        let info = info_rows(store, started_at);
        let exported = export_all(store, &ctx.workspace, &info, run, summary.finished_at);
        log_export(&exported, &mut emit);
        summary.export = Some(exported);
    }
    if !ctx.dry_run
        && let Ok(json) = serde_json::to_string(&summary)
        && let Err(e) = store.kv_set(LAST_RUN, &json)
    {
        log::warn!("Laufzusammenfassung nicht gespeichert: {e}");
    }
    emit(RunEvent::Finished(Box::new(summary.clone())));
    summary
}

fn failed(error: &str, message: &str) -> Outcome {
    Outcome::Failed {
        error: error.into(),
        message: message.into(),
    }
}

/// Nummer des letzten Laufs mit Postfach-Abruf („Neu in diesem Lauf“); 0 = noch keiner.
pub fn last_scan_run(store: &Store) -> crate::Result<i64> {
    Ok(store
        .kv_get(LAST_SCAN_RUN)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

/// Statuszeile ohne Wartezeit.
fn status(text: impl Into<String>) -> RunEvent {
    RunEvent::Status {
        text: text.into(),
        until: None,
    }
}

/// Verlaufszeile mit dem Zeitpunkt der Meldung.
fn log_line(level: Level, text: impl Into<String>) -> RunEvent {
    RunEvent::Log {
        level,
        text: text.into(),
        at: Timestamp::now(),
    }
}

/// Meldet eine Tätigkeit, wenn sie nicht schon in der Statuszeile steht.
fn announce(activity: &mut Option<String>, text: String, emit: &mut impl FnMut(RunEvent)) {
    if activity.as_deref() != Some(text.as_str()) {
        emit(status(text.clone()));
        *activity = Some(text);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Lauf-Kontext: Speicher, Regeln, Auftrag, Abbruch, Uhr und Ereignisse einzeln (in Tests austauschbar)"
)]
async fn scan_step<B: Backends>(
    backends: &mut B,
    store: &Store,
    request: &RunRequest,
    run: i64,
    started_at: Timestamp,
    cancel: &CancellationToken,
    scanned: &mut ScanSummary,
    emit: &mut impl FnMut(RunEvent),
) -> Result<(), ScanError> {
    emit(status("Gmail wird verbunden…"));
    let mut mail = backends.connect_mail(cancel).await?;
    emit(log_line(
        Level::Info,
        format!(
            "Postfach-Abruf startet: {}, Portale: {}.",
            scope_text(request.scope),
            portal_list(&request.portals)
        ),
    ));
    emit(status("Postfach wird durchsucht…"));
    let result = scan(
        &mut mail,
        store,
        run,
        request.scope,
        &request.portals,
        started_at,
        cancel,
        scanned,
        |event| match event {
            ScanEvent::Found { total } => {
                if total > 0 {
                    emit(status("Mails werden gelesen…"));
                }
            }
            ScanEvent::Alert(alert) => emit(RunEvent::Alert {
                portal: alert.portal,
                subject: truncate_chars(&alert.subject, 160),
                date: alert.date,
                postings: alert.postings.len(),
                gmail_id: alert.gmail_id.map(|id| format!("{id:x}")),
            }),
            ScanEvent::Progress { done, total } => emit(RunEvent::Progress {
                step: Step::Scan,
                done,
                total,
            }),
        },
    )
    .await;
    let s = &*scanned;
    let (level, text) = match &result {
        Ok(()) if s.alert_mails > 0 && s.postings_total == 0 => (
            Level::Warn,
            format!(
                "{}, aber keine Jobs erkannt – vermutlich hat sich das Mail-Layout geändert.",
                plural(s.alert_mails, "Alert-Mail", "Alert-Mails")
            ),
        ),
        Ok(()) => (
            Level::Ok,
            format!(
                "Postfach geprüft: {}, {}, {}, {} schon bekannt, {} doppelt.",
                plural(s.mails_checked, "Mail", "Mails"),
                plural(s.alert_mails, "Alert-Mail", "Alert-Mails"),
                plural(s.new, "neuer Job", "neue Jobs"),
                s.known_before,
                s.dup_in_run
            ),
        ),
        Err(ScanError::Mail(MailError::Cancelled)) => {
            (Level::Warn, "Postfach-Abruf abgebrochen.".into())
        }
        Err(e) => (Level::Error, format!("Postfach-Abruf fehlgeschlagen: {e}")),
    };
    emit(log_line(level, text));
    if s.mails_defective > 0 {
        emit(log_line(
            Level::Warn,
            format!(
                "{} nicht lesbar und übersprungen.",
                plural(s.mails_defective, "Mail war", "Mails waren")
            ),
        ));
    }
    result
}

#[expect(
    clippy::too_many_arguments,
    reason = "Lauf-Kontext: Speicher, Regeln, Auftrag, Abbruch, Uhr und Ereignisse einzeln (in Tests austauschbar)"
)]
async fn fetch_step<B: Backends>(
    backends: &mut B,
    store: &Store,
    policy: &mut Policy,
    request: &RunRequest,
    targeted: Option<&[JobKey]>,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    fetched: &mut FetchSummary,
    emit: &mut impl FnMut(RunEvent),
) -> Outcome {
    let mut pages = match backends.pages() {
        Ok(pages) => pages,
        Err(e) => return failed("fetch", &format!("Abruf nicht möglich: {e}")),
    };
    let selection = match targeted {
        Some(keys) => Selection::Jobs(keys),
        None => Selection::Queue(&request.portals),
    };
    // Tätigkeit in der Statuszeile – nicht bei jedem Job neu, nach einer Wartezeit wieder.
    let mut activity: Option<String> = None;
    let result = fetch_all(
        &mut pages,
        store,
        policy,
        selection,
        cancel,
        clock,
        fetched,
        |event| match event {
            FetchEvent::Queued { total } => {
                emit(log_line(Level::Info, format!("Jobdetails: {total} offen.")));
            }
            FetchEvent::Fetching { portal } => announce(
                &mut activity,
                format!("Jobdetails werden geholt ({})…", portal.label()),
                emit,
            ),
            FetchEvent::SigningIn { portal } => announce(
                &mut activity,
                format!("Anmeldung bei {}…", portal.label()),
                emit,
            ),
            FetchEvent::Waiting { portal, until } => {
                activity = None;
                emit(RunEvent::Status {
                    text: format!("Pause vor dem nächsten Abruf ({})", portal.label()),
                    until: Some(until),
                });
            }
            FetchEvent::JobUpdated { key, .. } => {
                if let Ok(Some(job)) = store.job(key) {
                    emit(RunEvent::JobUpdated {
                        job: Box::new(JobView::from(&job)),
                    });
                }
            }
            FetchEvent::PortalStopped {
                portal,
                skipped,
                text,
                ..
            } => {
                emit(log_line(Level::Warn, text));
                emit(RunEvent::PortalStopped {
                    portal,
                    skipped,
                    text: text.to_string(),
                });
            }
            FetchEvent::Progress { done, total } => emit(RunEvent::Progress {
                step: Step::Fetch,
                done,
                total,
            }),
        },
    )
    .await;
    match result {
        Ok(true) => {
            let ok: usize = fetched.per_portal.values().map(|c| c.ok).sum();
            let open: usize = fetched.queued.saturating_sub(ok);
            emit(if open > 0 {
                let (job, go) = if open == 1 {
                    ("Job", "geht")
                } else {
                    ("Jobs", "gehen")
                };
                log_line(
                    Level::Warn,
                    format!(
                        "Jobdetails: {ok} geholt. {open} {job} ohne Jobdetails {go} nicht ins Matching."
                    ),
                )
            } else {
                log_line(Level::Ok, format!("Jobdetails: {ok} geholt."))
            });
            Outcome::Completed
        }
        Ok(false) => {
            emit(log_line(
                Level::Warn,
                "Jobdetails abgebrochen – Geholtes ist gespeichert.",
            ));
            Outcome::Cancelled
        }
        Err(e) => failed(e.kind(), &e.to_string()),
    }
}

/// Textdateien (genau einmal je Job) und Übersicht. Die Übersicht wird nur neu erzeugt,
/// wenn sich etwas geändert hat – Daten, Lauf, Format, Ordner – oder sie fehlt; eine in
/// Excel offene Datei stört dann nicht unnötig.
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
        Err(e) => note_error(&mut summary, e.to_string()),
    }
    write_overview(
        store,
        &export::overview_path(&result_dir),
        info,
        run,
        now,
        &mut summary,
    );
    summary
}

/// „Textdateien neu schreiben“ (z. B. nach einem Ordnerwechsel): alle Jobs mit Volltext,
/// die Namen bleiben. Was sich nicht schreiben lässt, behält seine Marke – der nächste Lauf
/// legt eine absichtlich gelöschte Datei also nicht von selbst wieder an.
pub fn rewrite_txt(store: &Store, workspace: &Path, now: Timestamp) -> ExportSummary {
    let mut summary = ExportSummary::default();
    match store.txt_jobs(true) {
        Ok(jobs) => write_txts(store, &workspace.join(RESULT_DIR), jobs, now, &mut summary),
        Err(e) => note_error(&mut summary, e.to_string()),
    }
    summary
}

/// Der erste Fehler bleibt stehen: Er liegt der Ursache am nächsten (der Ordner ist nicht
/// erreichbar), spätere Folgefehler stehen nur im Protokoll. Gemeldet wird ohnehin einer.
fn note_error(summary: &mut ExportSummary, error: String) {
    if let Some(first) = &summary.error {
        log::warn!("Weiterer Fehler beim Schreiben (gemeldet wird „{first}“): {error}");
    } else {
        summary.error = Some(error);
    }
}

/// Beispiel zu einer nicht geschriebenen Textdatei merken. Gezählt wird nur in
/// `txt_failed_count`: Diese Liste ist gekappt und nennt bloß die ersten Schlüssel.
fn note_failed(summary: &mut ExportSummary, key: &JobKey) {
    if summary.txt_failed.len() < MAX_FAILED_NAMES {
        summary.txt_failed.push(key.to_string());
    }
}

/// Schreibt Textdateien und markiert nur, was geschrieben ist. Der Ordner wird einmal
/// angelegt: Ist er unbrauchbar (Laufwerk getrennt, keine Rechte), gibt es eine klare
/// Meldung statt eines vergeblichen Versuchs je Datei.
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
        summary.txt_failed_count = jobs.len();
        for (job, _) in &jobs {
            note_failed(summary, &job.key);
        }
        note_error(
            summary,
            format!(
                "Die Textdateien ließen sich nicht schreiben – der Ordner ist nicht erreichbar oder nicht beschreibbar:\n{}\n\n{e}",
                dir.display()
            ),
        );
        return;
    }
    let total = jobs.len();
    for (done, (job, text)) in jobs.into_iter().enumerate() {
        match write_job_txt(result_dir, &job, &text) {
            Ok(name) => match store.mark_txt_written(&job.key, &name, now) {
                Ok(()) => summary.txt_written += 1,
                Err(e) => {
                    // Ohne Datenbank lässt sich nichts mehr markieren: Diese und alle
                    // übrigen Dateien gelten als offen (der nächste Lauf holt sie nach).
                    note_error(summary, e.to_string());
                    summary.txt_failed_count += total - done;
                    note_failed(summary, &job.key);
                    return;
                }
            },
            Err(e) => {
                log::warn!("Textdatei für {} nicht geschrieben: {e}", job.key);
                summary.txt_failed_count += 1;
                note_failed(summary, &job.key);
            }
        }
    }
}

/// Übersicht schreiben, wenn sie fehlt oder sich seit dem letzten Mal an diesem Pfad etwas
/// geändert hat. Der Stand wird je Pfad gemerkt: Was die App dort schrieb, bleibt ihres –
/// auch nach einem Wechsel des Ordners und zurück.
fn write_overview(
    store: &Store,
    path: &Path,
    info: &[(String, String)],
    run: i64,
    now: Timestamp,
    summary: &mut ExportSummary,
) {
    // Die Lauf-Nummer gehört zum Blatt „Info“ und ändert die Datei bei jedem Lauf.
    let stamp = serde_json::json!({
        "rev": store.data_rev().unwrap_or(-1),
        "run": run,
    })
    .to_string();
    let key = format!("{EXPORT_STAMP}{}", path.display());
    // Ohne lesbaren Stand ist der Besitz der Datei unbekannt – dann wird sie weder gesichert
    // noch ersetzt. Ein Datenbankfehler darf die eigene Übersicht nicht wegsichern.
    let last = match store.kv_get(&key) {
        Ok(last) => last,
        Err(e) => {
            note_error(
                summary,
                format!(
                    "Die Übersicht {} wurde nicht geschrieben – der letzte Exportstand ließ sich nicht lesen:\n\n{e}",
                    path.display()
                ),
            );
            return;
        }
    };
    if path.exists() && last.as_deref() == Some(stamp.as_str()) {
        return;
    }
    // Eine Übersicht, die nicht von dieser App stammt (z. B. aus dem alten Programm im selben
    // Ordner), wird vor dem ersten Schreiben gesichert – nie still ersetzt.
    if path.exists() && last.is_none() {
        let backup = path.with_file_name(format!(
            "JobAlerts.alt-{}.{}",
            now.strftime("%Y%m%d-%H%M%S"),
            path.extension().and_then(|e| e.to_str()).unwrap_or("xlsx")
        ));
        if let Err(e) = std::fs::rename(path, &backup) {
            note_error(
                summary,
                format!(
                    "Die vorhandene Datei {} ließ sich nicht sichern ({e}) – sie bleibt unverändert.",
                    path.display()
                ),
            );
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
                log::warn!("Exportstand nicht gespeichert: {e}");
            }
            summary.overview = Some(path.to_path_buf());
        }
        Err(e) => note_error(summary, e.to_string()),
    }
}

/// Merkt die Zahlen eines erfolgreichen Postfach-Abrufs für das Blatt „Info“ – nur dann:
/// Ein gescheiterter Abruf überschreibt den letzten guten Stand nicht.
fn remember_scan(store: &Store, ctx: &RunContext, scope: Scope, scan: &ScanSummary, at: Timestamp) {
    let rows = [
        ("Gmail-Konto", ctx.account.clone()),
        ("Letzter Postfach-Abruf", time::display(at)),
        ("Umfang des letzten Laufs", scope_text(scope).to_string()),
        ("Neu (letzter Lauf)", scan.new.to_string()),
        (
            "Schon bekannt (letzter Lauf)",
            scan.known_before.to_string(),
        ),
        (
            "Doppelt in mehreren Mails (letzter Lauf)",
            scan.dup_in_run.to_string(),
        ),
    ];
    let saved = serde_json::to_string(&rows)
        .map_err(|e| e.to_string())
        .and_then(|json| {
            store
                .kv_set(LAST_SCAN_INFO, &json)
                .map_err(|e| e.to_string())
        });
    if let Err(e) = saved {
        log::warn!("Angaben zum Postfach-Abruf nicht gespeichert: {e}");
    }
}

/// Blatt „Info“ der Excel-Datei (Konto, letzter Lauf, Zähler, Programm). Konto und Zahlen
/// stammen vom letzten erfolgreichen Postfach-Abruf – auch nach reinen Detail-Läufen.
fn info_rows(store: &Store, started_at: Timestamp) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = store
        .kv_get(LAST_SCAN_INFO)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    rows.push(("Letzter Lauf".into(), time::display(started_at)));
    rows.push((
        "Jobs gesamt".into(),
        store.job_count().unwrap_or(0).to_string(),
    ));
    rows.push(("Programm".into(), "Job-Alert-Monitor".into()));
    rows
}

fn log_export(exported: &ExportSummary, emit: &mut impl FnMut(RunEvent)) {
    if exported.txt_written > 0 {
        emit(log_line(
            Level::Ok,
            format!(
                "{} für das Matching geschrieben.",
                plural(exported.txt_written, "neue Textdatei", "neue Textdateien")
            ),
        ));
    }
    if exported.txt_failed_count > 0 {
        emit(log_line(
            Level::Warn,
            format!(
                "{} sich nicht schreiben (nächster Lauf versucht es erneut).",
                plural(
                    exported.txt_failed_count,
                    "Textdatei ließ",
                    "Textdateien ließen"
                )
            ),
        ));
    }
    if let Some(backup) = &exported.backup {
        emit(log_line(
            Level::Warn,
            format!(
                "Eine vorhandene Übersicht stammte nicht von dieser App – sie wurde gesichert als {}",
                backup.display()
            ),
        ));
    }
    if let Some(path) = &exported.overview {
        emit(log_line(
            Level::Ok,
            format!("Übersicht gespeichert: {}", path.display()),
        ));
    }
    if let Some(error) = &exported.error {
        emit(log_line(Level::Error, error.as_str()));
    }
}

fn scope_text(scope: Scope) -> &'static str {
    match scope {
        Scope::New => "Neu seit letztem Lauf",
        Scope::All => "Alle",
    }
}

fn portal_list(portals: &[Portal]) -> String {
    portals
        .iter()
        .map(|p| p.label())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
