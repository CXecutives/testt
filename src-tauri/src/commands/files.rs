//! Result files: rewrite and delete text files, open checked targets.

use jiff::Timestamp;
use jobalert_core::error::{ErrorInfo, ErrorKind};
use jobalert_core::export::{self, RESULT_DIR};
use jobalert_core::model::gmail_url;
use jobalert_core::pipeline::{self, ExportSummary};
use jobalert_core::view::{ClearedTxt, OpenTarget};
use tauri::State;

use super::app::existing;
use super::{AppState, CmdResult, not_found};

/// Google page to create an app password.
const APP_PASSWORD_URL: &str = "https://myaccount.google.com/apppasswords";

/// Rewrites all text files (e.g. after a change of folder). The names stay.
#[tauri::command]
pub async fn rewrite_txt(state: State<'_, AppState>) -> CmdResult<ExportSummary> {
    state.ensure_idle()?;
    state.ensure_real()?;
    Ok(pipeline::rewrite_txt(
        &state.store,
        &state.workspace()?,
        Timestamp::now(),
    ))
}

/// Deletes only the app's text files; the Excel overview and the database stay (no fetch
/// again).
#[tauri::command]
pub async fn clear_txt(state: State<'_, AppState>) -> CmdResult<ClearedTxt> {
    state.ensure_idle()?;
    state.ensure_real()?;
    let result_dir = state.workspace()?.join(RESULT_DIR);
    let (removed, failed) = export::clear_txt_files(&result_dir, &state.store.txt_names()?);
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
    let what: std::ffi::OsString = match target {
        OpenTarget::JobUrl { key } => job(&key)??.url.to_string().into(),
        OpenTarget::Gmail { key } => job(&key)??
            .gmail_id
            .and_then(gmail_url)
            .ok_or_else(|| not_found("mail"))?
            .to_string()
            .into(),
        OpenTarget::AlertMail { gmail_id } => u64::from_str_radix(&gmail_id, 16)
            .ok()
            .and_then(gmail_url)
            .ok_or_else(|| not_found("mail"))?
            .to_string()
            .into(),
        OpenTarget::PortalHome { portal } => portal.home_url().into(),
        OpenTarget::AppPasswordPage => APP_PASSWORD_URL.into(),
        OpenTarget::Workspace => existing(state.workspace()?, "folder")?,
        OpenTarget::Excel => existing(
            export::overview_path(&state.workspace()?.join(RESULT_DIR)),
            "file",
        )?,
        OpenTarget::Overview => existing(
            export::overview_html_path(&state.workspace()?.join(RESULT_DIR)),
            "file",
        )?,
        OpenTarget::LogDir => existing(state.data_dir.join(jobalert_core::LOG_DIR), "folder")?,
    };
    open::that_detached(&what).map_err(|e| {
        log::warn!("could not open a target: {e}");
        ErrorInfo::new(ErrorKind::Io)
    })
}
