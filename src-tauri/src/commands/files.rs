//! Result files: rewrite and delete text files, open checked targets, and the small files
//! that follow the user's marks.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use jiff::Timestamp;
use jobalert_core::error::{ErrorInfo, ErrorKind};
use jobalert_core::export::{self, RESULT_DIR};
use jobalert_core::model::gmail_url_for;
use jobalert_core::pipeline::{self, ExportSummary, Matcher};
use jobalert_core::view::{ClearedTxt, OpenTarget};
use tauri::{AppHandle, Manager, State};

use super::app::existing;
use super::{AppState, CmdResult, lock, not_found};

/// Google page to create an app password.
const APP_PASSWORD_URL: &str = "https://myaccount.google.com/apppasswords";
/// Google page to turn on 2-step verification, which an app password requires.
const TWO_STEP_URL: &str = "https://myaccount.google.com/signinoptions/twosv";
/// How long the files wait after the last mark: a few clicks in a row write once.
const SETTLE: Duration = Duration::from_secs(2);
/// How often a waiting refresh looks whether the app is idle again.
const IDLE_POLL: Duration = Duration::from_millis(500);

/// The small files a mark changes - the HTML overview and the skill's `top_matches.json`
/// (`pipeline::refresh_exports`) - follow the user's marks a moment after the last one:
/// never while a run, a sign-in or a file command holds the app (a run writes them at its
/// end, a refresh then follows), never in the dry run. Marks of the last moments before the
/// app ends are written when it ends ([`flush_marks`]).
#[derive(Default)]
pub struct Refresh {
    /// Counts the marks: only the wait of the last one writes.
    marks: AtomicU64,
    /// The mark the files follow; held while they are written, so two writes never overlap.
    written: Mutex<u64>,
}

impl Refresh {
    /// Counts a mark and returns its number.
    fn mark(&self) -> u64 {
        self.marks.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Runs `write` unless the files already follow the latest mark; says whether it ran.
    fn follow(&self, write: impl FnOnce()) -> bool {
        let mut written = lock(&self.written);
        let mark = self.marks.load(Ordering::SeqCst);
        if *written == mark {
            return false;
        }
        write();
        *written = mark;
        true
    }
}

/// A mark changed (moved, starred, "fits anyway", read or unread): the files follow shortly.
pub(super) fn marked(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.dry_run {
        return;
    }
    let mark = state.refresh.mark();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SETTLE).await;
        loop {
            let state = app.state::<AppState>();
            if state.refresh.marks.load(Ordering::SeqCst) != mark {
                return;
            }
            if !state.busy() {
                break;
            }
            tokio::time::sleep(IDLE_POLL).await;
        }
        let state = app.state::<AppState>();
        state.refresh.follow(|| refresh(&state));
    });
}

/// The app ends: files that still wait for a mark (closed within [`SETTLE`]) are written now,
/// which takes milliseconds, unless a run or a file command still holds the app.
pub fn flush_marks(state: &AppState) {
    if !state.dry_run && !state.busy() {
        state.refresh.follow(|| refresh(state));
    }
}

fn refresh(state: &AppState) {
    let Ok(settings) = state.settings() else {
        return;
    };
    let matcher = state.matcher();
    pipeline::refresh_exports(
        &state.store,
        &settings.workspace_or(&state.default_workspace),
        matcher.as_deref().map(|m| m as &dyn Matcher),
        Timestamp::now(),
        settings.language_or(state.system_language),
    );
}

/// Rewrites all text files (e.g. after a change of folder). The names stay. It holds the app
/// meanwhile: no run and no "Textdateien löschen" touch the folder.
#[tauri::command]
pub async fn rewrite_txt(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ExportSummary> {
    state.ensure_real()?;
    let _files = state.claim_files(&app)?;
    Ok(pipeline::rewrite_txt(
        &state.store,
        &state.workspace()?,
        Timestamp::now(),
    ))
}

/// Deletes only the app's text files; the Excel overview and the database stay (no fetch
/// again). It holds the app meanwhile: a run's text files never lose their temporary files.
#[tauri::command]
pub async fn clear_txt(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ClearedTxt> {
    state.ensure_real()?;
    let _files = state.claim_files(&app)?;
    let (removed, failed) = pipeline::clear_txt(&state.store, &state.workspace()?)?;
    log::info!(
        "text files deleted: {removed}, not deleted: {}",
        failed.len()
    );
    Ok(ClearedTxt { removed, failed })
}

/// Opens a checked target in the browser, the mail client or the file manager.
#[tauri::command]
pub async fn open_target(state: State<'_, AppState>, target: OpenTarget) -> CmdResult<()> {
    let job = |key| {
        state
            .store
            .job(key)
            .map(|job| job.ok_or_else(|| not_found("job")))
    };
    // A mail opens in the account of the mailbox the app reads (the address is cached after
    // the first read; the dry run never touches the vault).
    let mail = |id: Option<u64>| -> CmdResult<std::ffi::OsString> {
        let mailbox = if state.dry_run {
            None
        } else {
            state.gmail_user().0
        };
        Ok(id
            .and_then(|id| gmail_url_for(id, mailbox.as_deref()))
            .ok_or_else(|| not_found("mail"))?
            .to_string()
            .into())
    };
    let what: std::ffi::OsString = match target {
        OpenTarget::JobUrl { key } => job(&key)??.url.to_string().into(),
        OpenTarget::Gmail { key } => mail(job(&key)??.gmail_id)?,
        OpenTarget::AlertMail { gmail_id } => mail(u64::from_str_radix(&gmail_id, 16).ok())?,
        OpenTarget::PortalHome { portal } => portal.home_url().into(),
        OpenTarget::AppPasswordPage => APP_PASSWORD_URL.into(),
        OpenTarget::TwoStepPage => TWO_STEP_URL.into(),
        OpenTarget::Workspace => existing(state.workspace()?, "folder")?,
        OpenTarget::ProfileDir => existing(
            state.workspace()?.join(jobalert_core::profile::PROFILE_DIR),
            "folder",
        )?,
        OpenTarget::Excel => existing(
            export::overview_path(&state.workspace()?.join(RESULT_DIR)),
            "file",
        )?,
        OpenTarget::ExcelInFolder => {
            let workspace = state.workspace()?;
            let excel = export::overview_path(&workspace.join(RESULT_DIR));
            if excel.is_file() {
                return show_in_folder(&excel);
            }
            // No file yet (before the first fetch): the work folder it will be in.
            existing(workspace, "folder")?
        }
        OpenTarget::ExcelBackupInFolder { name } => {
            if !export::is_xlsx_backup(&name) {
                return Err(not_found("file"));
            }
            let path = state.workspace()?.join(RESULT_DIR).join(name);
            existing(path.clone(), "file")?;
            return show_in_folder(&path);
        }
        OpenTarget::Overview => {
            let workspace = state.workspace()?;
            // Opened as the jobs are now (a run writes it itself at its end).
            if !state.dry_run
                && !state.busy()
                && let Err(e) = pipeline::refresh_overview(
                    &state.store,
                    &workspace,
                    Timestamp::now(),
                    state.language()?,
                )
            {
                log::warn!("overview not written before opening: {e}");
            }
            existing(
                export::overview_html_path(&workspace.join(RESULT_DIR)),
                "file",
            )?
        }
        OpenTarget::LogDir => existing(state.data_dir.join(jobalert_core::LOG_DIR), "folder")?,
        OpenTarget::DataDir => existing(state.data_dir.clone(), "folder")?,
    };
    open::that_detached(&what).map_err(|e| {
        log::warn!("could not open a target: {e}");
        ErrorInfo::new(ErrorKind::Io)
    })
}

/// Shows a checked file selected in its folder (Explorer, Finder).
fn show_in_folder(path: &std::path::Path) -> CmdResult<()> {
    crate::platform::show_in_folder(path).map_err(|e| {
        log::warn!("could not show a file in its folder: {e}");
        ErrorInfo::new(ErrorKind::Io)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files follow the latest mark once: the wait of a mark and the flush when the app
    /// ends never write twice for the same mark, and a new mark writes again.
    #[test]
    fn the_files_follow_the_latest_mark_once() {
        let refresh = Refresh::default();
        let mut writes = 0;
        assert!(!refresh.follow(|| writes += 1), "no mark, nothing to write");
        refresh.mark();
        refresh.mark();
        assert!(refresh.follow(|| writes += 1));
        assert!(!refresh.follow(|| writes += 1), "written already");
        refresh.mark();
        assert!(refresh.follow(|| writes += 1));
        assert_eq!(writes, 2);
    }
}
