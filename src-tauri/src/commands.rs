//! Die Befehle der Oberfläche – je Befehl ein dünner Adapter auf `jobalert-core`. Pfade
//! kommen nie aus der Seite: Ordner und Dateien wählt der Nutzer im Dialog, geöffnet wird
//! nur, was hier geprüft wurde (`open_target`).

// Tauri übergibt Befehlsargumente (auch `State`) als Wert. Befehle mit Datei-, Tresor- oder
// Datenbankarbeit sind `async`: Synchrone Befehle liefen im Fenster-Thread und ließen die
// Oberfläche stocken.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri übergibt Befehlsargumente als Wert"
)]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use jiff::Timestamp;
use jobalert_core::export::{self, RESULT_DIR};
use jobalert_core::fetch::policy::Policy;
use jobalert_core::fetch::site::PortalSite;
use jobalert_core::fetch::{Admission, Fetchers, Login, StopReason, admit, http::HttpFetcher};
use jobalert_core::mail::imap::{Credentials, Gmail, MailError};
use jobalert_core::pipeline::{
    self, Backends, LAST_RUN, Outcome, RunContext, RunEvent, RunRequest, RunSummary,
    demo::DemoBackends,
};
use jobalert_core::portal::{JobKey, Portal};
use jobalert_core::profile::{self, ProfileInfo};
use jobalert_core::reset::{self, ResetPlan, ResetReport};
use jobalert_core::secrets::Vault;
use jobalert_core::settings::Settings;
use jobalert_core::store::{JobFilter, Store};
use jobalert_core::view::{AlertMailView, JobView, PortalView, portal_views};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tokio_util::sync::CancellationToken;

use crate::session::{Notify, Session, Sessions};

/// So viele Verlaufszeilen hält der Schnappschuss (für ein Neuladen der Seite).
const LOG_KEEP: usize = 500;
/// Seite zum Anlegen eines Gmail-App-Passworts.
const APP_PASSWORD_URL: &str = "https://myaccount.google.com/apppasswords";
/// Betriebssystem der Oberfläche – die Seite formuliert ihre Texte danach.
const PLATFORM: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    "linux"
};
/// Wie der Ort des Gmail-App-Passworts hier heißt. Eine Stelle für alle Texte – das Backend
/// nennt ihn nie selbst.
const VAULT_NAME: &str = if cfg!(windows) {
    "Windows-Tresor"
} else {
    "Schlüsselbund"
};

/// Zustand der App, von allen Befehlen geteilt.
pub struct AppState {
    pub store: Arc<Store>,
    pub data_dir: PathBuf,
    pub default_workspace: PathBuf,
    pub dry_run: bool,
    pub user_agent: String,
    pub reset_report: Mutex<Option<ResetReport>>,
    /// Gmail-Adresse aus dem Tresor. So wird der Tresor samt Passwort nur einmal je Start für
    /// die Anzeige gelesen – sonst nur für den Postfach-Abruf.
    pub gmail_user: Mutex<GmailUser>,
    /// Was die App gerade tut. Geprüft und belegt wird unter derselben Sperre – ein Lauf und
    /// eine Anmeldung schließen sich aus (zwei Schreiber von `policy.json` verlören Zugriffe).
    pub activity: Mutex<Activity>,
}

/// Gespeicherte Gmail-Adresse im Zwischenspeicher.
pub enum GmailUser {
    /// Noch nicht aus dem Tresor gelesen.
    Unread,
    Known(Option<String>),
}

/// Lauf oder An-/Abmelden bei einem Portal – nie beides zugleich.
pub enum Activity {
    Idle,
    Run(RunHandle),
    /// Das Sitzungsfenster ist für An- oder Abmelden in Gebrauch (abbrechbar).
    Session(CancellationToken),
}

impl AppState {
    fn policy_path(&self) -> PathBuf {
        self.data_dir.join(jobalert_core::POLICY_FILE)
    }

    /// Gespeicherte Gmail-Adresse und ggf. warum der Tresor unlesbar ist. Ein Lesefehler
    /// wird nicht gemerkt – der nächste Aufruf versucht es erneut.
    fn gmail_user(&self) -> (Option<String>, Option<String>) {
        let mut cached = lock(&self.gmail_user);
        if let GmailUser::Known(user) = &*cached {
            return (user.clone(), None);
        }
        match Vault::app().load_gmail() {
            Ok(credentials) => {
                let user = credentials.map(|c| c.user);
                *cached = GmailUser::Known(user.clone());
                (user, None)
            }
            Err(e) => (None, Some(e.to_string())),
        }
    }

    fn settings(&self) -> Result<Settings, CommandError> {
        Ok(Settings::load(&self.store)?)
    }

    fn workspace(&self) -> Result<PathBuf, CommandError> {
        Ok(self.settings()?.workspace_or(&self.default_workspace))
    }

    pub fn busy(&self) -> bool {
        !matches!(*lock(&self.activity), Activity::Idle)
    }

    fn ensure_idle(&self) -> Result<(), CommandError> {
        if self.busy() {
            return Err(busy());
        }
        Ok(())
    }

    /// Der Trockenlauf ändert nichts außerhalb seiner Speicher-Datenbank.
    fn ensure_real(&self, message: &str) -> Result<(), CommandError> {
        if self.dry_run {
            return Err(CommandError::new("dryRun", message));
        }
        Ok(())
    }

    /// Bricht einen Lauf oder eine laufende An-/Abmeldung ab (idempotent).
    pub fn cancel_run(&self) {
        match &*lock(&self.activity) {
            Activity::Run(run) => run.cancel.cancel(),
            Activity::Session(cancel) => cancel.cancel(),
            Activity::Idle => {}
        }
    }
}

/// Ein laufender Lauf: Abbruch, austauschbarer Kanal zur Seite und Schnappschuss.
pub struct RunHandle {
    cancel: CancellationToken,
    sink: Arc<Mutex<Option<Channel<RunEvent>>>>,
    snapshot: Arc<Mutex<Snapshot>>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    status: String,
    /// Ende der Wartezeit, die `status` nennt (Countdown).
    status_until: Option<Timestamp>,
    progress: Option<RunEvent>,
    log: VecDeque<RunEvent>,
}

fn busy() -> CommandError {
    CommandError::new(
        "busy",
        "Während eines Laufs nicht möglich – bitte erst abbrechen oder warten.",
    )
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Fehler für die Oberfläche: `{ kind, message }`.
#[derive(Debug, Serialize)]
pub struct CommandError {
    kind: &'static str,
    message: String,
}

impl CommandError {
    fn new(kind: &'static str, message: impl Into<String>) -> CommandError {
        CommandError {
            kind,
            message: message.into(),
        }
    }
}

impl From<jobalert_core::Error> for CommandError {
    fn from(e: jobalert_core::Error) -> CommandError {
        CommandError::new(e.kind(), e.to_string())
    }
}

impl From<jobalert_core::secrets::SecretError> for CommandError {
    fn from(e: jobalert_core::secrets::SecretError) -> CommandError {
        let kind = match e {
            jobalert_core::secrets::SecretError::Invalid(_) => "invalid",
            _ => "secret",
        };
        CommandError::new(kind, e.to_string())
    }
}

type CmdResult<T> = Result<T, CommandError>;

// ------------------------------------------------------------------ Zustand

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateView {
    /// Betriebssystem: „windows“ | „macos“ | „linux“ – die Seite formuliert danach.
    platform: &'static str,
    /// Wie der Ort des App-Passworts dort heißt.
    vault_name: &'static str,
    dry_run: bool,
    data_dir: PathBuf,
    settings: Settings,
    workspace: PathBuf,
    gmail_user: Option<String>,
    gmail_error: Option<String>,
    profile: Option<ProfileInfo>,
    profile_error: Option<String>,
    portals: Vec<PortalView>,
    /// Alert-Mails ohne erkannte Einträge aus dem letzten Lauf mit Postfach-Abruf.
    zero_posting_mails: Vec<AlertMailView>,
    jobs_total: i64,
    last_scan_run: i64,
    last_run: Option<serde_json::Value>,
    result_dir: PathBuf,
    /// Abgeleitete Orte – die Oberfläche setzt keine Pfade selbst zusammen.
    profile_dir: PathBuf,
    txt_dir: PathBuf,
    result_dir_exists: bool,
    /// Zahl der Textdateien der App in `auswertung/beschreibungen_txt` – genau das, was
    /// „Textdateien löschen“ entfernt.
    txt_files: usize,
    reset_report: Option<ResetReport>,
    running: Option<Snapshot>,
}

/// Alles, was die Seite beim Start (und nach einem Neuladen) braucht. Läuft gerade ein
/// Lauf, hängt `channel` die Seite wieder an seine Ereignisse an (sonst bleibt er unbenutzt).
#[tauri::command]
pub async fn app_state(
    state: State<'_, AppState>,
    channel: Channel<RunEvent>,
) -> CmdResult<AppStateView> {
    let settings = state.settings()?;
    let workspace = settings.workspace_or(&state.default_workspace);
    // Der Trockenlauf fasst den Tresor nie an, zeigt aber ein Postfach: Sonst stünde die
    // App dauerhaft im Einrichtungszustand und „Abrufen“ bliebe gesperrt – zu sehen wäre
    // dann gerade das nicht, was der Trockenlauf vorführen soll. `example.org` ist für
    // Beispiele reserviert und kann kein echtes Postfach sein.
    let (gmail_user, gmail_error) = if state.dry_run {
        (Some("trockenlauf@example.org".to_string()), None)
    } else {
        state.gmail_user()
    };
    let now = Timestamp::now();
    let policy = if state.dry_run {
        Policy::in_memory()
    } else {
        Policy::load(&state.policy_path(), now)
    };
    let running = match &*lock(&state.activity) {
        Activity::Run(run) => {
            *lock(&run.sink) = Some(channel);
            Some(lock(&run.snapshot).clone())
        }
        _ => None,
    };
    let workspace_for_paths = workspace.clone();
    let result_dir = workspace.join(RESULT_DIR);
    // Nur die eigenen Textdateien – genau die, die „Textdateien löschen“ entfernen würde.
    let txt_files = export::txt_files(&result_dir, &state.store.txt_names()?).len();
    let (profile, profile_error) = match profile::info(&workspace) {
        Ok(info) => (info, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let last_scan_run = pipeline::last_scan_run(&state.store)?;
    Ok(AppStateView {
        platform: PLATFORM,
        vault_name: VAULT_NAME,
        dry_run: state.dry_run,
        data_dir: state.data_dir.clone(),
        profile,
        profile_error,
        portals: portal_views(&policy, &state.store, &settings, now)?,
        zero_posting_mails: state
            .store
            .zero_posting_mails(last_scan_run)?
            .iter()
            .map(AlertMailView::from)
            .collect(),
        jobs_total: state.store.job_count()?,
        last_scan_run,
        result_dir_exists: result_dir.is_dir(),
        last_run: state
            .store
            .kv_get(LAST_RUN)?
            .and_then(|json| serde_json::from_str(&json).ok()),
        reset_report: lock(&state.reset_report).clone(),
        settings,
        workspace,
        gmail_user,
        gmail_error,
        profile_dir: workspace_for_paths.join(profile::PROFILE_DIR),
        txt_dir: result_dir.join(export::TXT_DIR),
        result_dir,
        txt_files,
        running,
    })
}

// ------------------------------------------------------------------ Einstellungen

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    portals: Vec<Portal>,
}

/// Speichert die Portalwahl. Der Arbeitsordner ändert sich nur per Dialog.
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    input: SettingsInput,
) -> CmdResult<Settings> {
    let mut settings = state.settings()?;
    settings.portals = input.portals;
    settings.save(&state.store)?;
    state.settings()
}

/// Ordnerdialog für den Arbeitsordner; `None`, wenn abgebrochen.
#[tauri::command]
pub async fn pick_workspace(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> CmdResult<Option<PathBuf>> {
    state.ensure_idle()?;
    let current = state.workspace()?;
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_title("Arbeitsordner wählen")
        .set_directory(current.parent().unwrap_or(&current))
        .set_parent(&window)
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let folder = folder.path().to_path_buf();
    // Schreibbar? Lieber jetzt eine klare Meldung als später beim Export. Der Trockenlauf
    // schreibt nie etwas außerhalb seiner Speicher-Datenbank.
    if !state.dry_run {
        let probe = folder.join(".job-alert-monitor-schreibtest");
        std::fs::write(&probe, b"").map_err(|e| {
            CommandError::new(
                "io",
                format!(
                    "In diesen Ordner kann nicht geschrieben werden:\n{}\n\n{e}",
                    folder.display()
                ),
            )
        })?;
        let _ = std::fs::remove_file(probe);
    }
    let mut settings = state.settings()?;
    settings.workspace = Some(folder.clone());
    settings.save(&state.store)?;
    Ok(Some(folder))
}

// ------------------------------------------------------------------ Gmail

/// Speichert den Gmail-Zugang. Ein anderes Konto beginnt mit eigenem Scan-Stand.
#[tauri::command]
pub async fn save_gmail_credentials(
    state: State<'_, AppState>,
    user: String,
    password: String,
) -> CmdResult<String> {
    state.ensure_idle()?;
    state.ensure_real("Im Trockenlauf werden keine Zugangsdaten gespeichert.")?;
    let credentials = Credentials::new(&user, &password);
    let vault = Vault::app();
    let cached = match &*lock(&state.gmail_user) {
        GmailUser::Known(user) => Some(user.clone()),
        GmailUser::Unread => None,
    };
    let previous = cached.unwrap_or_else(|| vault.load_gmail().ok().flatten().map(|c| c.user));
    vault.save_gmail(&credentials)?;
    *lock(&state.gmail_user) = GmailUser::Known(Some(credentials.user.clone()));
    if previous.as_deref() != Some(credentials.user.as_str()) {
        state.store.clear_scan_state()?;
    }
    Ok(credentials.user)
}

#[tauri::command]
pub async fn delete_gmail_credentials(state: State<'_, AppState>) -> CmdResult<bool> {
    state.ensure_idle()?;
    state.ensure_real("Im Trockenlauf werden keine Zugangsdaten entfernt.")?;
    let removed = Vault::app().delete_gmail()?;
    *lock(&state.gmail_user) = GmailUser::Known(None);
    state.store.clear_scan_state()?;
    Ok(removed)
}

// ------------------------------------------------------------------ Lauf

/// Echte Abrufwege: Gmail, HTTP für den Gastweg und je Portal ein Sitzungsfenster.
struct AppBackends {
    credentials: Option<Credentials>,
    user_agent: String,
    app: AppHandle,
    data_dir: PathBuf,
    notify: Notify,
}

impl Backends for AppBackends {
    type Mail = Gmail;
    type Pages = Fetchers<Sessions>;

    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<Gmail, MailError> {
        let credentials = self.credentials.as_ref().ok_or(MailError::NoCredentials)?;
        Gmail::connect(credentials, cancel.clone()).await
    }

    /// Je Portal ein eigener Abrufweg – eigene HTTP-Sitzung, eigenes Fenster (eigenes
    /// Label): Die Portale laufen nebeneinander und teilen sich nichts.
    fn pages(&mut self, _portal: Portal) -> Result<Self::Pages, String> {
        Ok(Fetchers {
            http: HttpFetcher::new(&self.user_agent).map_err(|e| e.to_string())?,
            session: Sessions::new(self.app.clone(), self.data_dir.clone(), self.notify.clone()),
        })
    }
}

/// Einstellungen und Gmail-Zugang eines Laufs. Nur der Postfach-Schritt braucht Gmail:
/// Der Abruf der Jobdetails geht auch, wenn der Tresor-Eintrag gerade unlesbar ist.
fn run_context(
    state: &AppState,
    request: &RunRequest,
) -> CmdResult<(RunContext, Option<Credentials>)> {
    let settings = state.settings()?;
    let credentials = if state.dry_run || !request.scan {
        None
    } else {
        Vault::app().load_gmail()?
    };
    let ctx = RunContext {
        workspace: settings.workspace_or(&state.default_workspace),
        account: credentials
            .as_ref()
            .map(|c| c.user.clone())
            .unwrap_or_default(),
        dry_run: state.dry_run,
    };
    Ok((ctx, credentials))
}

/// Startet einen Lauf und kehrt sofort zurück; Ereignisse kommen über `channel`.
#[tauri::command]
pub async fn start_run(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RunRequest,
    channel: Channel<RunEvent>,
) -> CmdResult<()> {
    // Prüfen und belegen unter einer Sperre (kein `await` dazwischen).
    let mut activity = lock(&state.activity);
    match *activity {
        Activity::Idle => {}
        Activity::Run(_) => {
            return Err(CommandError::new("busy", "Es läuft bereits ein Lauf."));
        }
        Activity::Session(_) => {
            return Err(CommandError::new(
                "busy",
                "Ein Anmeldefenster ist gerade offen – bitte dort erst fertig werden.",
            ));
        }
    }
    let (ctx, credentials) = run_context(&state, &request)?;
    let handle = RunHandle {
        cancel: CancellationToken::new(),
        sink: Arc::new(Mutex::new(Some(channel))),
        snapshot: Arc::new(Mutex::new(Snapshot::default())),
    };
    let cancel = handle.cancel.clone();
    let mine = handle.snapshot.clone();
    // „Fertig“ erst senden, wenn der Laufplatz frei ist – sonst bekäme eine sofortige
    // Folgeaktion der Seite noch „busy“.
    let emit = {
        let mut send = emitter(handle.sink.clone(), handle.snapshot.clone());
        let app = app.clone();
        let mine = mine.clone();
        move |event: RunEvent| {
            if matches!(event, RunEvent::Finished { .. }) {
                release_run(&app.state::<AppState>(), &mine);
            }
            send(event);
        }
    };
    let finish = emitter(handle.sink.clone(), handle.snapshot.clone());
    let notify: Notify = {
        let send = Mutex::new(emitter(handle.sink.clone(), handle.snapshot.clone()));
        Arc::new(move |event| (lock(&send))(event))
    };
    *activity = Activity::Run(handle);
    drop(activity);

    let store = state.store.clone();
    let dry_run = state.dry_run;
    let policy = (!dry_run).then(|| state.policy_path());
    let started = Timestamp::now();
    let work = if dry_run {
        tauri::async_runtime::spawn(drive(
            DemoBackends,
            store,
            policy,
            request,
            ctx,
            cancel,
            emit,
        ))
    } else {
        let backends = AppBackends {
            credentials,
            user_agent: state.user_agent.clone(),
            app: app.clone(),
            data_dir: state.data_dir.clone(),
            notify,
        };
        tauri::async_runtime::spawn(drive(backends, store, policy, request, ctx, cancel, emit))
    };
    // Aufsicht: Auch eine Panik endet mit genau einem `Finished`, und der Lauf wird frei.
    // Den Platz räumt sie nur für ihren eigenen Lauf – ein neuer könnte schon laufen.
    tauri::async_runtime::spawn(async move {
        if let Err(error) = work.await {
            log::error!("Lauf abgestürzt: {error}");
            release_run(&app.state::<AppState>(), &mine);
            let mut finish = finish;
            finish(RunEvent::Finished {
                summary: Box::new(crashed(dry_run, started)),
            });
        }
    });
    Ok(())
}

/// Ein Lauf mit Attrappen oder echten Abrufwegen. `policy`: Pfad von `policy.json`
/// (`None` im Trockenlauf – dann nur im Arbeitsspeicher).
async fn drive<B: Backends>(
    mut backends: B,
    store: Arc<Store>,
    policy: Option<PathBuf>,
    request: RunRequest,
    ctx: RunContext,
    cancel: CancellationToken,
    emit: impl FnMut(RunEvent),
) {
    let policy = Mutex::new(policy.map_or_else(Policy::in_memory, |path| {
        Policy::load(&path, Timestamp::now())
    }));
    pipeline::run(
        &mut backends,
        &store,
        &policy,
        &request,
        &ctx,
        &cancel,
        Timestamp::now,
        emit,
    )
    .await;
}

/// Gibt den Laufplatz frei – nur, wenn er noch diesem Lauf gehört.
fn release_run(state: &AppState, snapshot: &Arc<Mutex<Snapshot>>) {
    let mut activity = lock(&state.activity);
    if matches!(&*activity, Activity::Run(run) if Arc::ptr_eq(&run.snapshot, snapshot)) {
        *activity = Activity::Idle;
    }
}

/// Zusammenfassung eines abgestürzten Laufs (die Aufsicht meldet sie statt seiner).
fn crashed(dry_run: bool, started_at: Timestamp) -> RunSummary {
    RunSummary {
        run: 0,
        outcome: Outcome::Failed {
            error: "panic".into(),
            message: "Interner Fehler – Einzelheiten stehen im Protokoll.".into(),
        },
        dry_run,
        started_at,
        finished_at: Timestamp::now(),
        scope: None,
        scan: None,
        fetch: None,
        export: None,
    }
}

/// Schickt Ereignisse an die (gerade angehängte) Seite und merkt sie für ein Neuladen.
fn emitter(
    sink: Arc<Mutex<Option<Channel<RunEvent>>>>,
    snapshot: Arc<Mutex<Snapshot>>,
) -> impl FnMut(RunEvent) + Send + 'static {
    move |event: RunEvent| {
        {
            let mut snap = lock(&snapshot);
            match &event {
                RunEvent::Status { text, until } => {
                    snap.status.clone_from(text);
                    snap.status_until = *until;
                }
                RunEvent::Progress { .. } => snap.progress = Some(event.clone()),
                RunEvent::Log { .. } | RunEvent::PortalStopped { .. } => {
                    if snap.log.len() == LOG_KEEP {
                        snap.log.pop_front();
                    }
                    snap.log.push_back(event.clone());
                }
                _ => {}
            }
        }
        if let Some(channel) = lock(&sink).as_ref()
            && let Err(e) = channel.send(event)
        {
            log::debug!("Ereignis nicht zugestellt: {e}");
        }
    }
}

/// Bricht einen Lauf oder eine Anmeldung ab (mehrfach aufrufbar).
#[tauri::command]
pub fn cancel_run(state: State<'_, AppState>) {
    state.cancel_run();
}

// ------------------------------------------------------------------ Portal-Anmeldung

/// Hält „Sitzungsfenster in Gebrauch“ und gibt es am Ende sicher frei.
struct SessionGuard<'a> {
    state: &'a AppState,
    /// Bricht An- oder Abmelden ab (`cancel_run`, Beenden).
    cancel: CancellationToken,
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        let mut activity = lock(&self.state.activity);
        if matches!(*activity, Activity::Session(_)) {
            *activity = Activity::Idle;
        }
    }
}

/// Belegt das Sitzungsfenster für An- oder Abmelden – geprüft und belegt unter einer Sperre,
/// nie neben einem Lauf. Danach gelten dieselben Regeln wie für jeden Abruf: keine
/// Anmeldung während einer Pause oder über der Obergrenze, Abstand zum letzten Zugriff,
/// gezählt und gesichert vor dem Kontakt. `None`: abgebrochen, bevor das Portal kontaktiert
/// wurde.
async fn claim_session(state: &AppState, portal: Portal) -> CmdResult<Option<SessionGuard<'_>>> {
    state.ensure_real("Im Trockenlauf gibt es keine Portal-Anmeldung.")?;
    if PortalSite::of(portal).is_none() {
        return Err(CommandError::new(
            "invalid",
            format!("{portal} kennt keine Anmeldung."),
        ));
    }
    let cancel = CancellationToken::new();
    {
        let mut activity = lock(&state.activity);
        if !matches!(*activity, Activity::Idle) {
            return Err(busy());
        }
        *activity = Activity::Session(cancel.clone());
    }
    let guard = SessionGuard { state, cancel };
    let policy = Mutex::new(Policy::load(&state.policy_path(), Timestamp::now()));
    match admit(&policy, portal, &guard.cancel, &Timestamp::now, |_| {}).await? {
        Admission::Go => Ok(Some(guard)),
        Admission::Cancelled => Ok(None),
        Admission::Stop(StopReason::Paused { until, reason }) => Err(CommandError::new(
            "paused",
            format!(
                "{portal} ist pausiert bis {} ({reason}). Bis dahin meldet die App sich dort weder an noch ab – bei einer Sperre das Portal im eigenen Browser öffnen.",
                jobalert_core::time::display(until)
            ),
        )),
        Admission::Stop(StopReason::Quota { next_at }) => Err(CommandError::new(
            "paused",
            format!(
                "Die Obergrenze für {portal} ist erreicht – An- und Abmelden wieder ab {}.",
                jobalert_core::time::display(next_at)
            ),
        )),
        Admission::Stop(other) => Err(CommandError::new("paused", other.text(portal, 0))),
    }
}

/// Ende einer An- oder Abmeldung bei einem Portal: den Sitzungsstand, falls er feststeht,
/// und – als letzte Antwort – den Zeitpunkt, ab dem der nächste Abruf Abstand hält.
fn record_session(state: &AppState, portal: Portal, signed_in: Option<bool>) {
    let now = Timestamp::now();
    let mut policy = Policy::load(&state.policy_path(), now);
    if let Some(signed_in) = signed_in {
        policy.set_session(portal, signed_in, now);
    }
    policy.record_done(portal, now);
    if let Err(e) = policy.save() {
        log::warn!("Sitzungsstand nicht gespeichert: {e}");
    }
}

/// Ein Sitzungsfenster für An- oder Abmelden von Hand (ohne Lauf-Ereignisse).
fn session_window(app: AppHandle, state: &AppState, portal: Portal) -> Option<Session> {
    let site = PortalSite::of(portal)?;
    Some(Session::new(app, &state.data_dir, site, Arc::new(|_| {})))
}

/// Einmal selbst bei einem Portal anmelden (Fenster sichtbar, höchstens 5 Minuten). Die App
/// sieht und speichert das Passwort nie. Nach einer Sicherheitsprüfung gilt die Anmeldung
/// trotzdem (der nächste Lauf ruft wieder ab).
#[tauri::command]
pub async fn portal_login(
    app: AppHandle,
    state: State<'_, AppState>,
    portal: Portal,
) -> CmdResult<bool> {
    let Some(guard) = claim_session(&state, portal).await? else {
        return Ok(false);
    };
    let Some(mut session) = session_window(app, &state, portal) else {
        return Ok(false);
    };
    let login = session.sign_in(&guard.cancel).await;
    drop(session);
    let signed_in = login != Login::NotSignedIn;
    // Abgebrochen (Beenden): Ob eine Sitzung besteht, ist offen – der Stand bleibt.
    let known = if signed_in || !guard.cancel.is_cancelled() {
        Some(signed_in)
    } else {
        None
    };
    record_session(&state, portal, known);
    Ok(signed_in)
}

/// Bei einem Portal abmelden. Andere Portale sind nicht betroffen (eigenes Profil). Nur eine
/// wirklich geladene Abmeldeseite beendet die Sitzung – sonst bleibt der Stand.
#[tauri::command]
pub async fn portal_logout(
    app: AppHandle,
    state: State<'_, AppState>,
    portal: Portal,
) -> CmdResult<bool> {
    let Some(guard) = claim_session(&state, portal).await? else {
        return Ok(false);
    };
    let Some(mut session) = session_window(app, &state, portal) else {
        return Ok(false);
    };
    let done = session.sign_out(&guard.cancel).await;
    drop(session);
    record_session(&state, portal, done.then_some(false));
    Ok(done)
}

// ------------------------------------------------------------------ Jobs

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobQuery {
    /// Nur die Jobs des letzten Laufs mit Postfach-Abruf („Neu in diesem Lauf“).
    #[serde(default)]
    latest_run: bool,
    #[serde(default)]
    search: Option<String>,
}

#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>, query: JobQuery) -> CmdResult<Vec<JobView>> {
    let first_seen_run = query
        .latest_run
        .then(|| pipeline::last_scan_run(&state.store))
        .transpose()?;
    let jobs = state.store.jobs(&JobFilter {
        first_seen_run,
        search: query.search,
    })?;
    Ok(jobs.iter().map(JobView::from).collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDetail {
    job: JobView,
    text: Option<String>,
}

#[tauri::command]
pub async fn job_detail(state: State<'_, AppState>, key: JobKey) -> CmdResult<JobDetail> {
    Ok(JobDetail {
        job: job_view(&state, &key)?,
        text: state.store.description(&key)?,
    })
}

// ------------------------------------------------------------------ Beraterprofil

#[tauri::command]
pub async fn pick_profile(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> CmdResult<Option<ProfileInfo>> {
    state.ensure_real("Im Trockenlauf wird kein Beraterprofil gespeichert.")?;
    let workspace = state.workspace()?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Beraterprofil (JSON) wählen")
        .add_filter("Beraterprofil", &["json"])
        .set_directory(&workspace)
        .set_parent(&window)
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    Ok(Some(profile::save_from(&workspace, file.path())?))
}

#[tauri::command]
pub async fn remove_profile(state: State<'_, AppState>) -> CmdResult<bool> {
    state.ensure_real("Im Trockenlauf wird kein Beraterprofil entfernt.")?;
    Ok(profile::remove(&state.workspace()?)?)
}

// ------------------------------------------------------------------ Ergebnisse

/// Schreibt alle Textdateien neu (z. B. nach einem Ordnerwechsel). Namen bleiben.
#[tauri::command]
pub async fn rewrite_txt(state: State<'_, AppState>) -> CmdResult<pipeline::ExportSummary> {
    state.ensure_idle()?;
    state.ensure_real("Im Trockenlauf werden keine Dateien geschrieben.")?;
    Ok(pipeline::rewrite_txt(
        &state.store,
        &state.workspace()?,
        Timestamp::now(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cleared {
    removed: usize,
    failed: Vec<String>,
}

/// Löscht nur die eigenen Textdateien; die Excel-Übersicht und die Datenbank bleiben
/// (kein erneuter Abruf).
#[tauri::command]
pub async fn clear_txt_files(state: State<'_, AppState>) -> CmdResult<Cleared> {
    state.ensure_idle()?;
    state.ensure_real("Im Trockenlauf werden keine Dateien gelöscht.")?;
    let result_dir = state.workspace()?.join(RESULT_DIR);
    let (removed, failed) = export::clear_txt_files(&result_dir, &state.store.txt_names()?);
    Ok(Cleared { removed, failed })
}

// ------------------------------------------------------------------ Öffnen

/// Was sich öffnen lässt – nie ein beliebiger Pfad oder Link aus der Seite.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Target {
    JobUrl {
        key: JobKey,
    },
    /// Alert-Mail per Gmail-ID (hexadezimal, z. B. aus einer grauen Zeile).
    Gmail {
        id: String,
    },
    /// Startseite eines Portals („Im Browser öffnen“ nach einer Sperre).
    PortalHome {
        portal: Portal,
    },
    ResultFolder,
    AppPasswordPage,
    LogFolder,
}

#[tauri::command]
pub async fn open_target(state: State<'_, AppState>, target: Target) -> CmdResult<()> {
    let what: std::ffi::OsString = match target {
        Target::JobUrl { key } => job_view(&state, &key)?.url.into(),
        Target::ResultFolder => existing_dir(state.workspace()?.join(RESULT_DIR))?,
        Target::Gmail { id } => u64::from_str_radix(&id, 16)
            .ok()
            .and_then(jobalert_core::model::gmail_url)
            .ok_or_else(|| CommandError::new("notFound", "Diese Mail ist nicht bekannt."))?
            .to_string()
            .into(),
        Target::PortalHome { portal } => portal.home_url().into(),
        Target::AppPasswordPage => APP_PASSWORD_URL.into(),
        Target::LogFolder => existing_dir(state.data_dir.join(jobalert_core::LOG_DIR))?,
    };
    open::that_detached(&what)
        .map_err(|e| CommandError::new("io", format!("Ließ sich nicht öffnen: {e}")))
}

fn job_view(state: &AppState, key: &JobKey) -> CmdResult<JobView> {
    let job = state
        .store
        .job(key)?
        .ok_or_else(|| CommandError::new("notFound", "Diesen Job gibt es nicht (mehr)."))?;
    Ok(JobView::from(&job))
}

fn existing_dir(dir: PathBuf) -> CmdResult<std::ffi::OsString> {
    if dir.is_dir() {
        Ok(dir.into_os_string())
    } else {
        Err(CommandError::new(
            "notFound",
            format!("Den Ordner gibt es noch nicht:\n{}", dir.display()),
        ))
    }
}

// ------------------------------------------------------------------ App

/// „Alles zurücksetzen“: Auftrag ablegen, dann neu starten – gelöscht wird beim Start.
#[tauri::command]
pub async fn reset_all(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.ensure_idle()?;
    state.ensure_real("Im Trockenlauf wird nichts zurückgesetzt.")?;
    let plan = ResetPlan {
        workspace: state.workspace()?,
        txt_names: state.store.txt_names()?,
    };
    reset::request(&state.data_dir, &plan)?;
    app.request_restart();
    Ok(())
}

/// Fehler aus der Seite ins Protokoll (gekürzt, einzeilig).
#[tauri::command]
pub fn report_ui_error(message: String, source: Option<String>, line: Option<u32>) {
    let text: String = message.chars().take(500).collect();
    log::error!(
        "Oberfläche: {text} ({}:{})",
        source.unwrap_or_default(),
        line.unwrap_or(0)
    );
}
