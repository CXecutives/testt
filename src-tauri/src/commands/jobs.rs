//! The job list, the reader and the per-job marks.

use jiff::Timestamp;
use jobalert_core::portal::JobKey;
use jobalert_core::view::{self, JobDetail, JobPage, JobQuery};
use tauri::State;

use super::{AppState, CmdResult, not_found};

/// One page of the list with its counts (one store query).
#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>, query: JobQuery) -> CmdResult<JobPage> {
    Ok(view::job_page(&state.store, &query)?)
}

/// The reader: the match is assessed again from the stored text (reasons are not stored).
/// A stale stored score is replaced only while no run is active - else the run's catch-up
/// does it.
#[tauri::command]
pub async fn job_detail(state: State<'_, AppState>, key: JobKey) -> CmdResult<JobDetail> {
    let matcher = state.matcher();
    let save = !state.running();
    view::job_detail(
        &state.store,
        &key,
        matcher.as_deref(),
        save,
        Timestamp::now(),
    )?
    .ok_or_else(|| not_found("job"))
}

/// Marks a job as read - only on a real click in the list; `false` = it was read already.
#[tauri::command]
pub async fn mark_read(state: State<'_, AppState>, key: JobKey) -> CmdResult<bool> {
    Ok(state.store.mark_read(&key, Timestamp::now())?)
}

/// Pins or unpins a job; `false` = nothing changed.
#[tauri::command]
pub async fn set_pinned(state: State<'_, AppState>, key: JobKey, on: bool) -> CmdResult<bool> {
    Ok(state.store.set_pinned(&key, on, Timestamp::now())?)
}
