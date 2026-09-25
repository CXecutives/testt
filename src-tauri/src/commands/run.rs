//! Starting and cancelling runs; the event channel to the page and its snapshot for a
//! reload.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use jiff::Timestamp;
use jobalert_core::error::{ErrorInfo, ErrorKind, InvalidInput};
use jobalert_core::fetch::Fetchers;
use jobalert_core::fetch::http::HttpFetcher;
use jobalert_core::fetch::policy::Policy;
use jobalert_core::mail::imap::{Credentials, Gmail, MailError};
use jobalert_core::pipeline::{
    self, Backends, Matcher, Outcome, RunContext, RunEvent, RunKindName, RunRequest, RunSnapshot,
    RunSummary, demo::DemoBackends,
};
use jobalert_core::portal::{FetchPath, Portal};
use jobalert_core::secrets::Vault;
use jobalert_core::store::Store;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio_util::sync::CancellationToken;

use super::{Activity, AppState, CmdResult, lock};
use crate::session::{Notify, Sessions};

/// So many alert events the snapshot keeps (for a reload of the page).
const ALERTS_KEPT: usize = 50;

/// A run in progress: cancellation, replaceable channel to the page, and snapshot.
pub struct RunHandle {
    cancel: CancellationToken,
    sink: Arc<Mutex<Option<Channel<RunEvent>>>>,
    snapshot: Arc<Mutex<Snapshot>>,
}

impl RunHandle {
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// A reloaded page takes over the events.
    pub(super) fn attach(&self, channel: Channel<RunEvent>) {
        *lock(&self.sink) = Some(channel);
    }

    pub(super) fn snapshot(&self) -> RunSnapshot {
        lock(&self.snapshot).to_view()
    }
}

/// The events that describe the current state of a run.
struct Snapshot {
    kind: RunKindName,
    started_at: Timestamp,
    status: Option<RunEvent>,
    progress: Option<RunEvent>,
    /// Latest health event per portal.
    health: Vec<(Portal, RunEvent)>,
    login: Option<RunEvent>,
    alerts: VecDeque<RunEvent>,
}

impl Snapshot {
    fn new(kind: RunKindName) -> Snapshot {
        Snapshot {
            kind,
            started_at: Timestamp::now(),
            status: None,
            progress: None,
            health: Vec::new(),
            login: None,
            alerts: VecDeque::new(),
        }
    }

    fn remember(&mut self, event: &RunEvent) {
        match event {
            RunEvent::Status { .. } => self.status = Some(event.clone()),
            RunEvent::Progress { .. } => self.progress = Some(event.clone()),
            RunEvent::PortalHealth { portal, .. } => {
                self.health.retain(|(p, _)| p != portal);
                self.health.push((*portal, event.clone()));
            }
            RunEvent::LoginNeeded { .. } => self.login = Some(event.clone()),
            RunEvent::Alert { .. } => {
                if self.alerts.len() == ALERTS_KEPT {
                    self.alerts.pop_front();
                }
                self.alerts.push_back(event.clone());
            }
            RunEvent::Started { .. } | RunEvent::JobUpdated { .. } | RunEvent::Finished { .. } => {}
        }
    }

    fn to_view(&self) -> RunSnapshot {
        let mut replay: Vec<RunEvent> = self.alerts.iter().cloned().collect();
        replay.extend(self.health.iter().map(|(_, e)| e.clone()));
        replay.extend(
            [&self.progress, &self.login, &self.status]
                .into_iter()
                .flatten()
                .cloned(),
        );
        RunSnapshot {
            kind: self.kind,
            started_at: self.started_at,
            replay,
        }
    }
}

/// Real fetch paths: Gmail, and per portal either HTTP as a guest or a session window.
struct AppBackends {
    credentials: Option<Credentials>,
    user_agent: String,
    app: AppHandle,
    data_dir: PathBuf,
    notify: Notify,
    /// The engine with the profile as it was when the run started.
    matcher: Option<Arc<dyn Matcher>>,
    /// The same engine, typed: it also orders the fetch queue (likely matches first).
    local: Option<Arc<jobalert_core::pipeline::LocalMatcher>>,
    /// The settings stay live: a portal switched off during the run gets no further request.
    store: Arc<Store>,
}

impl Backends for AppBackends {
    type Mail = Gmail;
    type Pages = Fetchers<Sessions>;

    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<Gmail, MailError> {
        let credentials = self.credentials.as_ref().ok_or(MailError::NoCredentials)?;
        Gmail::connect(credentials, cancel.clone()).await
    }

    /// One fetch path per portal, and only the one it needs - own HTTP session or own
    /// window (own label): the portals run side by side and share nothing. The session path
    /// exists only with the portal's sign-in switched on, so only then may this run show its
    /// sign-in window.
    fn pages(&mut self, portal: Portal, path: FetchPath) -> Result<Self::Pages, String> {
        Ok(match path {
            FetchPath::Guest => {
                Fetchers::Guest(HttpFetcher::new(&self.user_agent).map_err(|e| e.to_string())?)
            }
            FetchPath::Session => Fetchers::Session(
                Sessions::new(self.app.clone(), self.data_dir.clone(), self.notify.clone())
                    .allow_login(portal, true),
            ),
        })
    }

    fn matcher(&self) -> Option<Arc<dyn Matcher>> {
        self.matcher.clone()
    }

    fn prescore(&self) -> jobalert_core::fetch::Prescore {
        match &self.local {
            Some(local) => {
                let local = Arc::clone(local);
                Arc::new(move |title: &str, location: &str| {
                    jobalert_core::matching::prescore(local.profile(), title, location)
                })
            }
            None => jobalert_core::fetch::neutral_prescore(),
        }
    }

    fn live_paths(&self) -> Option<pipeline::LivePaths> {
        Some(pipeline::stored_paths(Arc::clone(&self.store)))
    }
}

/// Settings and Gmail access of a run. Only the mailbox step needs Gmail: fetching job
/// details also works when the vault entry is unreadable right now. A mailbox run needs a
/// portal to read: with every portal switched off it is refused here ("at least one portal
/// must be active"), never started only to be stored as a failed fetch.
fn run_context(
    state: &AppState,
    request: &RunRequest,
) -> CmdResult<(RunContext, Option<Credentials>)> {
    let settings = state.settings()?;
    let scans = request.kind.name().reads_mail();
    if scans && settings.enabled_portals().is_empty() {
        return Err(ErrorInfo::from(&InvalidInput::NoPortal));
    }
    let credentials = if state.dry_run || !scans {
        None
    } else {
        Vault::app().load_gmail()?
    };
    let ctx = RunContext {
        workspace: settings.workspace_or(&state.default_workspace),
        dry_run: state.dry_run,
        portals: settings.enabled_portals(),
        fetch_portals: settings.fetch_portals(),
        sign_in: Portal::ALL
            .into_iter()
            .filter(|&p| settings.fetch_path(p) == Some(FetchPath::Session))
            .collect(),
        auto_archive_days: settings.auto_archive_days,
        auto_empty_trash_days: settings.auto_empty_trash_days,
        language: settings.language_or(state.system_language),
    };
    Ok((ctx, credentials))
}

/// Starts a run and returns at once; events come through `channel`.
#[tauri::command]
pub async fn start_run(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RunRequest,
    channel: Channel<RunEvent>,
) -> CmdResult<()> {
    launch(&app, &state, request, channel)
}

/// Starts a run in the background (from the page or by the app itself at the start).
pub(super) fn launch(
    app: &AppHandle,
    state: &AppState,
    request: RunRequest,
    channel: Channel<RunEvent>,
) -> CmdResult<()> {
    // A run that cannot start never asks the keychain.
    if state.busy() {
        return Err(ErrorInfo::new(ErrorKind::Busy));
    }
    // Settings and Gmail access outside the lock: reading the keychain can wait for a prompt
    // (macOS), and the reader, the close button and every busy check wait for this lock.
    let (ctx, credentials) = run_context(state, &request)?;
    // Check and claim under one lock (no `await` in between); a run or sign-in that began
    // meanwhile keeps the slot, and the credentials read for nothing are dropped.
    let mut activity = lock(&state.activity);
    if !matches!(*activity, Activity::Idle) {
        return Err(ErrorInfo::new(ErrorKind::Busy));
    }
    let kind = request.kind.name();
    let handle = RunHandle {
        cancel: CancellationToken::new(),
        sink: Arc::new(Mutex::new(Some(channel))),
        snapshot: Arc::new(Mutex::new(Snapshot::new(kind))),
    };
    let cancel = handle.cancel.clone();
    let mine = handle.snapshot.clone();
    // Send "finished" only once the run slot is free - otherwise an immediate follow-up
    // action of the page would still get "busy".
    let emit = {
        let mut send = emitter(handle.sink.clone(), handle.snapshot.clone());
        let app = app.clone();
        let mine = mine.clone();
        move |event: RunEvent| {
            let finished = matches!(event, RunEvent::Finished { .. });
            if finished {
                release_run(&app.state::<AppState>(), &mine);
            }
            send(event);
            if finished {
                super::scoring::after_run(&app);
            }
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
            matcher: state.matcher().map(|m| m as Arc<dyn Matcher>),
            local: state.matcher(),
            store: Arc::clone(&store),
        };
        tauri::async_runtime::spawn(drive(backends, store, policy, request, ctx, cancel, emit))
    };
    // Supervisor: a panic too ends with exactly one `Finished`, and the slot becomes free.
    // It only clears the slot for its own run - a new one could already be running.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = work.await {
            // Only the kind: the error's text repeats the panic message, which can quote ad
            // or mail text (the panic hook already logged the place).
            match &error {
                tauri::Error::JoinError(join) => {
                    log::error!("{}", jobalert_core::logging::task_failure_line("run", join));
                }
                _ => log::error!("run crashed"),
            }
            release_run(&app.state::<AppState>(), &mine);
            let mut finish = finish;
            finish(crashed(kind, dry_run, started).finished_event());
            // A profile change during the crashed run is still owed its rescore.
            super::scoring::after_run(&app);
        }
    });
    Ok(())
}

/// A run with dummies or real fetch routes. `policy`: path of `policy.json` (`None` in the
/// dry run - then in memory only).
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

/// Frees the run slot - only if it still belongs to this run.
fn release_run(state: &AppState, snapshot: &Arc<Mutex<Snapshot>>) {
    let mut activity = lock(&state.activity);
    if matches!(&*activity, Activity::Run(run) if Arc::ptr_eq(&run.snapshot, snapshot)) {
        *activity = Activity::Idle;
    }
}

/// Summary of a crashed run (the supervisor reports it instead).
fn crashed(kind: RunKindName, dry_run: bool, started_at: Timestamp) -> RunSummary {
    let mut summary = RunSummary::new(kind, dry_run, started_at);
    summary.outcome = Outcome::Failed {
        error: ErrorInfo::new(ErrorKind::Internal),
    };
    summary.finished_at = Timestamp::now();
    summary
}

/// Sends events to the (currently attached) page and remembers them for a reload.
fn emitter(
    sink: Arc<Mutex<Option<Channel<RunEvent>>>>,
    snapshot: Arc<Mutex<Snapshot>>,
) -> impl FnMut(RunEvent) + Send + 'static {
    move |event: RunEvent| {
        lock(&snapshot).remember(&event);
        if let Some(channel) = lock(&sink).as_ref()
            && let Err(e) = channel.send(event)
        {
            log::debug!("event not delivered: {e}");
        }
    }
}

/// Cancels a run or a sign-in (callable several times).
#[tauri::command]
pub fn cancel_run(state: State<'_, AppState>) {
    state.cancel_run();
}
