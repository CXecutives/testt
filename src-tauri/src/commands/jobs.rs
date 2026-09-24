//! The job list, the reader and the per-job marks.

use jiff::Timestamp;
use jobalert_core::export;
use jobalert_core::model::AppStatus;
use jobalert_core::pipeline::demo;
use jobalert_core::portal::JobKey;
use jobalert_core::profile;
use jobalert_core::view::{self, JobDetail, JobPage, JobQuery, JobView};
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

/// Sets or clears where the application for a job stands; `false` = nothing changed.
#[tauri::command]
pub async fn set_app_status(
    state: State<'_, AppState>,
    key: JobKey,
    status: Option<AppStatus>,
) -> CmdResult<bool> {
    Ok(state.store.set_app_status(&key, status, Timestamp::now())?)
}

/// Stores the note of a job (blank = none, at most 2000 characters); `false` = unchanged.
#[tauri::command]
pub async fn set_note(state: State<'_, AppState>, key: JobKey, note: String) -> CmdResult<bool> {
    Ok(state.store.set_note(&key, &note)?)
}

/// Hides a job ("not interesting") or lists it again; `false` = nothing changed.
#[tauri::command]
pub async fn set_hidden(state: State<'_, AppState>, key: JobKey, hidden: bool) -> CmdResult<bool> {
    Ok(state.store.set_hidden(&key, hidden, Timestamp::now())?)
}

/// The prompt for a deep analysis of a job in the user's own Claude: the rubric in short,
/// the profile without name and contact data, the ad. The app sends it nowhere itself.
#[tauri::command]
pub async fn claude_prompt(state: State<'_, AppState>, key: JobKey) -> CmdResult<String> {
    let profile = if state.dry_run {
        serde_json::from_str(demo::PROFILE_JSON).ok()
    } else {
        profile::load(&state.workspace()?)?
    }
    .ok_or_else(|| not_found("profile"))?;
    let job = state.store.job(&key)?.ok_or_else(|| not_found("job"))?;
    let text = state.store.description(&key)?;
    Ok(export::claude_prompt(
        &profile,
        &JobView::from(&job),
        job.url.as_str(),
        text.as_deref(),
        state.language()?,
    ))
}
