//! The job list, the reader and the per-job marks.

use jiff::Timestamp;
use jiff::civil::Date;
use jobalert_core::export;
use jobalert_core::model::AppStatus;
use jobalert_core::pipeline::{self, Matcher, demo};
use jobalert_core::portal::JobKey;
use jobalert_core::profile;
use jobalert_core::store::{JobRow, ListFacet};
use jobalert_core::view::{self, Deleted, JobDetail, JobFacet, JobPage, JobQuery, JobView};
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

/// Archives a job or lists it again; `false` = nothing changed.
#[tauri::command]
pub async fn set_archived(
    state: State<'_, AppState>,
    key: JobKey,
    archived: bool,
) -> CmdResult<bool> {
    Ok(state.store.set_archived(&key, archived, Timestamp::now())?)
}

/// "Fits anyway": an excluded job counts as scored with its fit score (`include`), or the
/// engine's verdict applies again - assessed right away unless a run is active (then its
/// catch-up does it); `false` = nothing changed.
#[tauri::command]
pub async fn set_override(
    state: State<'_, AppState>,
    key: JobKey,
    include: bool,
) -> CmdResult<bool> {
    let changed = state.store.set_override(&key, include)?;
    if changed && !include && !state.running() {
        let matcher = state.matcher();
        view::job_detail(
            &state.store,
            &key,
            matcher.as_deref(),
            true,
            Timestamp::now(),
        )?;
    }
    Ok(changed)
}

/// Deletes jobs for good: rows, text files and Excel rows go; only a tombstone of each key
/// stays, so no later scan brings them back. Not while a run is active.
#[tauri::command]
pub async fn delete_jobs(state: State<'_, AppState>, keys: Vec<JobKey>) -> CmdResult<Deleted> {
    forget(&state, &keys)
}

/// Deletes every archived job for good (see [`delete_jobs`]).
#[tauri::command]
pub async fn empty_archive(state: State<'_, AppState>) -> CmdResult<Deleted> {
    let keys = state.store.archived_keys()?;
    forget(&state, &keys)
}

fn forget(state: &AppState, keys: &[JobKey]) -> CmdResult<Deleted> {
    state.ensure_idle()?;
    let workspace = if state.dry_run {
        None
    } else {
        Some(state.workspace()?)
    };
    let matcher = state.matcher();
    let matcher = matcher.as_deref().map(|m| m as &dyn Matcher);
    Ok(pipeline::delete_jobs(
        &state.store,
        workspace.as_deref(),
        matcher,
        keys,
        Timestamp::now(),
    )?)
}

/// The profile as the prompts use it (the sample profile in the dry run).
fn prompt_profile(state: &AppState) -> CmdResult<serde_json::Value> {
    let profile = if state.dry_run {
        serde_json::from_str(demo::PROFILE_JSON).ok()
    } else {
        profile::load(&state.workspace()?)?
    };
    profile.ok_or_else(|| not_found("profile"))
}

/// What a prompt needs of one job: its row, link, text and the app's findings.
struct Prompted {
    view: JobView,
    url: String,
    text: Option<String>,
    findings: Option<export::TopMatch>,
}

fn prompted(state: &AppState, row: &JobRow) -> CmdResult<Prompted> {
    let matcher = state.matcher();
    let matcher = matcher.as_deref().map(|m| m as &dyn Matcher);
    Ok(Prompted {
        view: JobView::from(row),
        url: row.url.to_string(),
        text: state.store.description(&row.key)?,
        findings: export::findings(&state.store, matcher, row)?,
    })
}

impl Prompted {
    fn item(&self) -> export::PromptJob<'_> {
        export::PromptJob {
            job: &self.view,
            url: &self.url,
            text: self.text.as_deref(),
            findings: self.findings.as_ref(),
        }
    }
}

/// The prompt for a deep analysis of a job in any AI chat: the rubric in short, the profile
/// without name and contact data, the ad and the app's findings. The app sends it nowhere.
#[tauri::command]
pub async fn ai_prompt(state: State<'_, AppState>, key: JobKey) -> CmdResult<String> {
    let profile = prompt_profile(&state)?;
    let row = state.store.job(&key)?.ok_or_else(|| not_found("job"))?;
    Ok(export::ai_prompt(&profile, prompted(&state, &row)?.item()))
}

/// One prompt that compares the best current matches (3 to 5; saved first, then the best
/// open ones; never excluded, archived or in an application) with the profile.
#[tauri::command]
pub async fn ai_prompt_top(state: State<'_, AppState>, limit: u32) -> CmdResult<String> {
    let profile = prompt_profile(&state)?;
    let limit = usize::try_from(limit)
        .unwrap_or(usize::MAX)
        .clamp(*export::TOP_LIMITS.start(), *export::TOP_LIMITS.end());
    let rows = state
        .store
        .best_matches(u32::try_from(limit).unwrap_or(u32::MAX))?;
    if rows.is_empty() {
        return Err(not_found("jobs"));
    }
    let jobs = rows
        .iter()
        .map(|row| prompted(&state, row))
        .collect::<CmdResult<Vec<_>>>()?;
    let items: Vec<export::PromptJob<'_>> = jobs.iter().map(Prompted::item).collect();
    Ok(export::ai_prompt_top(&profile, &items))
}

/// The day to follow up an application (while applied or in talks; `null` clears it);
/// `false` = nothing changed.
#[tauri::command]
pub async fn set_follow_up(
    state: State<'_, AppState>,
    key: JobKey,
    on: Option<Date>,
) -> CmdResult<bool> {
    Ok(state.store.set_follow_up(&key, on)?)
}

/// "All read": every unread job of the facet's list; the keys come back for the undo.
#[tauri::command]
pub async fn mark_all_read(state: State<'_, AppState>, facet: JobFacet) -> CmdResult<Vec<JobKey>> {
    let facet = match facet {
        JobFacet::New => ListFacet::New,
        JobFacet::All => ListFacet::All,
        JobFacet::Saved => ListFacet::Saved,
        JobFacet::Applications => ListFacet::Applications,
        JobFacet::Archived => ListFacet::Archived,
    };
    Ok(state.store.mark_all_read(facet, Timestamp::now())?)
}

/// The undo of "all read": these jobs are unread again; returns how many.
#[tauri::command]
pub async fn mark_unread(state: State<'_, AppState>, keys: Vec<JobKey>) -> CmdResult<u32> {
    let changed = state.store.mark_unread(&keys)?;
    Ok(u32::try_from(changed).unwrap_or(u32::MAX))
}
