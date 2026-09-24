//! The job list, the reader and the per-job marks.

use jobalert_core::pipeline;
use jobalert_core::portal::JobKey;
use jobalert_core::view::{self, JobDetail, JobPage, JobQuery};
use tauri::State;

use super::{AppState, CmdResult, not_found};

/// One page of the list with its counts (one store query).
#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>, query: JobQuery) -> CmdResult<JobPage> {
    let new_run = pipeline::last_scan_run(&state.store)?;
    Ok(view::job_page(&state.store, &query, new_run)?)
}

#[tauri::command]
pub async fn job_detail(state: State<'_, AppState>, key: JobKey) -> CmdResult<JobDetail> {
    view::job_detail(&state.store, &key)?.ok_or_else(|| not_found("job"))
}

/// Marks a job as read; `false` = nothing changed. Placeholder until schema 3 stores it.
#[tauri::command]
pub async fn mark_read(state: State<'_, AppState>, key: JobKey) -> CmdResult<bool> {
    let _ = (state, key);
    Ok(false)
}

/// Pins or unpins a job; `false` = nothing changed. Placeholder until schema 3 stores it.
#[tauri::command]
pub async fn set_pinned(state: State<'_, AppState>, key: JobKey, on: bool) -> CmdResult<bool> {
    let _ = (state, key, on);
    Ok(false)
}
