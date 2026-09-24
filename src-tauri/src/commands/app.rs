//! App state, settings, workspace, reset and UI error reports.

// Tauri passes command arguments (`State` too) by value. Commands with file, vault or
// database work are `async`: synchronous commands would run on the window thread and make
// the interface stutter.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use jiff::Timestamp;
use jobalert_core::error::{ErrorInfo, ErrorKind};
use jobalert_core::export::{self, RESULT_DIR};
use jobalert_core::fetch::policy::Policy;
use jobalert_core::pipeline::{self, Matcher as _, RunEvent, demo};
use jobalert_core::profile;
use jobalert_core::reset::{self, ResetPlan};
use jobalert_core::view::{
    self, JobFacet, JobQuery, JobSort, Mailbox, ProfileInfo, ResetSummary, SettingsPatch,
    SettingsView,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, State, WebviewWindow};

use super::{Activity, AppState, CmdResult, lock, scoring, texts};

/// Key of the name of the profile file the user chose last.
pub(super) const PROFILE_SOURCE: &str = "profile_source";
/// UI error reports per minute that reach the log.
const UI_ERRORS_PER_MINUTE: usize = 10;
const MAX_UI_MESSAGE_CHARS: usize = 500;
const MAX_UI_SOURCE_CHARS: usize = 200;

/// The mailbox as the interface shows it. The dry run never touches the vault but shows a
/// mailbox: otherwise the app would stay in the setup state and exactly what the dry run
/// should demonstrate could not be seen. `example.org` is reserved for examples and cannot
/// be a real mailbox.
pub(super) fn mailbox(state: &AppState) -> Mailbox {
    let (user, error) = if state.dry_run {
        (Some("probelauf@example.org".to_string()), None)
    } else {
        state.gmail_user()
    };
    Mailbox {
        user,
        vault: crate::platform::vault_kind(),
        error,
    }
}

/// The stored profile for the interface with what the engine understood of it; an
/// unreadable file is logged and shown as none. The dry run shows its sample profile.
pub(super) fn profile_info(state: &AppState, workspace: &std::path::Path) -> Option<ProfileInfo> {
    let info = if state.dry_run {
        let sample = profile::ProfileInfo {
            path: PathBuf::from(demo::PROFILE_NAME),
            bytes: demo::PROFILE_JSON.len() as u64,
            saved_at: None,
            parse_error: None,
        };
        ProfileInfo::of(&sample, Some(demo::PROFILE_NAME.to_owned()))
            .with_form(profile::form_of(demo::PROFILE_JSON))
    } else {
        match profile::info(workspace) {
            Ok(info) => {
                let source = state.store.kv_get(PROFILE_SOURCE).ok().flatten();
                ProfileInfo::of(&info?, source).with_form(profile::stored_form(workspace))
            }
            Err(e) => {
                log::warn!("profile not readable: {e}");
                return None;
            }
        }
    };
    if info.parse_error.is_some() {
        return Some(info);
    }
    let Some(matcher) = state.compiled_profile() else {
        return Some(info);
    };
    match info.clone().understood_by(&matcher, &state.store) {
        Ok(understood) => Some(understood),
        Err(e) => {
            log::warn!("profile state not read: {e}");
            Some(info)
        }
    }
}

/// Everything the page needs - without attaching to a run.
fn build_state(state: &AppState) -> CmdResult<view::AppState> {
    let settings = state.settings()?;
    let workspace = settings.workspace_or(&state.default_workspace);
    let now = Timestamp::now();
    let policy = if state.dry_run {
        Policy::in_memory()
    } else {
        Policy::load(&state.policy_path(), now)
    };
    let last_scan_run = pipeline::last_scan_run(&state.store)?;
    let empty_mails = state.store.zero_posting_mails(last_scan_run)?;
    let counts = view::job_page(
        &state.store,
        &JobQuery {
            facet: JobFacet::All,
            sort: JobSort::Newest,
            search: None,
            limit: 0,
            offset: 0,
        },
    )?
    .counts;
    let last_run = pipeline::last_run(&state.store)?;
    let result_dir = workspace.join(RESULT_DIR);
    // Only the app's own text files - exactly those "delete text files" would remove.
    let txt_files = export::txt_files(&result_dir, &state.store.txt_names()?).len();
    let running = match &*lock(&state.activity) {
        Activity::Run(run) => Some(run.snapshot()),
        _ => None,
    };
    Ok(view::AppState {
        platform: crate::platform::platform(),
        dry_run: state.dry_run,
        // The dry run is a demo with a mailbox and a sample profile: it starts in the app itself,
        // never on the first-run page (the smoke probe on a fresh CI machine relies on it).
        first_run: !state.dry_run && last_run.is_none() && counts.all == 0,
        running,
        settings: SettingsView {
            workspace_is_default: settings.workspace.is_none(),
            txt_files,
            excel_exists: export::overview_path(&result_dir).is_file(),
            workspace: workspace.clone(),
        },
        mailbox: mailbox(state),
        profile: profile_info(state, &workspace),
        portals: view::portal_states(&policy, &settings, &empty_mails, now),
        auto_fetch_on_start: settings.auto_fetch_on_start,
        auto_archive_days: settings.auto_archive_days,
        last_run,
        counts,
        match_pending: state.match_pending(),
        log_dir: state.data_dir.join(jobalert_core::LOG_DIR),
        data_dir: state.data_dir.clone(),
        reset_report: lock(&state.reset_report)
            .as_ref()
            .map(|report| ResetSummary {
                removed: report.removed,
                failed: report.failed.len(),
            }),
    })
}

/// Everything the page needs at the start (and after a reload). If a run is in progress,
/// `channel` attaches the page to its events again (otherwise it stays unused).
#[tauri::command]
pub async fn app_state(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Channel<RunEvent>,
) -> CmdResult<view::AppState> {
    if let Activity::Run(run) = &*lock(&state.activity) {
        run.attach(channel.clone());
    }
    state.scoring.set_page(channel.clone());
    at_start(&app, &state, channel);
    build_state(&state)
}

/// Once per app start, on the first page load: the auto fetch if it is due, else a rescore
/// if jobs wait for a score (new profile, engine update). A fetch scores them too.
fn at_start(app: &AppHandle, state: &AppState, channel: Channel<RunEvent>) {
    static CHECKED: AtomicBool = AtomicBool::new(false);
    if CHECKED.swap(true, Ordering::SeqCst) || state.busy() {
        return;
    }
    if !auto_fetch(app, state, channel) {
        scoring::rescore_if_pending(app, state);
    }
}

/// The auto fetch: switched on, mailbox connected, last fetch older than 6 hours. Never in
/// the dry run. `true` if it started.
fn auto_fetch(app: &AppHandle, state: &AppState, channel: Channel<RunEvent>) -> bool {
    if state.dry_run {
        return false;
    }
    let Ok(settings) = state.settings() else {
        return false;
    };
    let connected = state.gmail_user().0.is_some();
    let due = pipeline::auto_fetch_due(
        &state.store,
        settings.auto_fetch_on_start,
        connected,
        Timestamp::now(),
    );
    if due {
        log::info!("auto fetch at the start");
        let request = pipeline::RunRequest {
            kind: pipeline::RunKind::Fetch,
        };
        match super::run::launch(app, state, request, channel) {
            Ok(()) => return true,
            Err(e) => log::warn!("auto fetch not started: {:?}", e.kind),
        }
    }
    false
}

/// Saves portal switches and the auto fetch. The workspace only changes through the dialog.
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> CmdResult<view::AppState> {
    let mut settings = state.settings()?;
    patch.apply(&mut settings);
    settings.save(&state.store)?;
    build_state(&state)
}

/// Folder dialog for the workspace; `None` if cancelled.
#[tauri::command]
pub async fn pick_workspace(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> CmdResult<Option<PathBuf>> {
    state.ensure_idle()?;
    let current = state.workspace()?;
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_title(texts::PICK_WORKSPACE)
        .set_directory(current.parent().unwrap_or(&current))
        .set_parent(&window)
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let folder = folder.path().to_path_buf();
    // Writable? Better a clear error now than later at the export. The dry run never
    // writes anything outside its in-memory database.
    if !state.dry_run {
        let probe = folder.join(".job-alert-monitor-write-test");
        std::fs::write(&probe, b"")
            .map_err(|e| ErrorInfo::from(jobalert_core::Error::io(&folder, e)))?;
        let _ = std::fs::remove_file(probe);
    }
    let before = state.matcher().map(|m| m.rev().to_owned());
    let mut settings = state.settings()?;
    settings.workspace = Some(folder.clone());
    settings.save(&state.store)?;
    // The profile lives in the workspace: another folder can mean another profile.
    if state.matcher().map(|m| m.rev().to_owned()) != before {
        scoring::profile_changed(&app, &state);
    }
    Ok(Some(folder))
}

/// "Reset everything": leave the order, then restart - deleting happens at the start.
#[tauri::command]
pub async fn reset_all(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.ensure_idle()?;
    state.ensure_real()?;
    let plan = ResetPlan {
        workspace: state.workspace()?,
        txt_names: state.store.txt_names()?,
    };
    reset::request(&state.data_dir, &plan)?;
    log::info!("reset requested, restarting");
    app.request_restart();
    Ok(())
}

/// Errors of the page into the log (cut, single line, at most ten per minute).
#[tauri::command]
pub fn report_ui_error(message: String, source: Option<String>, line: Option<u32>) {
    static RECENT: Mutex<VecDeque<Instant>> = Mutex::new(VecDeque::new());
    {
        let mut recent = lock(&RECENT);
        let now = Instant::now();
        while recent
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
        {
            recent.pop_front();
        }
        if recent.len() >= UI_ERRORS_PER_MINUTE {
            return;
        }
        recent.push_back(now);
    }
    let flat = |text: &str, max: usize| -> String {
        text.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(max)
            .collect()
    };
    log::error!(
        "ui: {} ({}:{})",
        flat(&message, MAX_UI_MESSAGE_CHARS),
        flat(source.as_deref().unwrap_or_default(), MAX_UI_SOURCE_CHARS),
        line.unwrap_or(0)
    );
}

/// Errors of a missing folder or file carry what was looked for.
pub(super) fn existing(path: PathBuf, what: &str) -> CmdResult<std::ffi::OsString> {
    if path.exists() {
        Ok(path.into_os_string())
    } else {
        Err(ErrorInfo::new(ErrorKind::NotFound)
            .with("what", what)
            .with("path", path.display().to_string()))
    }
}
