//! The consultant profile: the editor's form is saved by merging it into the profile file;
//! a chosen file or a pasted answer of Claude fills the form first. Remove it.

use jobalert_core::profile;
use jobalert_core::view::{ProfileDraft, ProfileInfo, ProfileSave};
use tauri::{AppHandle, State, WebviewWindow};

use super::app::{PROFILE_SOURCE, profile_info};
use super::{AppState, CmdResult, scoring, texts};

/// Chooses a profile file and reads it into the form for review; nothing is stored until
/// the user saves. `None` if cancelled.
#[tauri::command]
pub async fn pick_profile(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> CmdResult<Option<ProfileDraft>> {
    let workspace = state.workspace()?;
    let words = texts::of(state.language()?);
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title(words.pick_profile)
        .add_filter(words.profile_filter, &["json"])
        .set_directory(&workspace)
        .set_parent(&window)
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    Ok(Some(profile::draft_from_file(file.path())?.into()))
}

/// Reads Claude's answer to the profile request (pasted) into the form for review.
#[tauri::command]
pub async fn parse_profile(text: String) -> CmdResult<ProfileDraft> {
    Ok(profile::draft_from_answer(&text)
        .map_err(jobalert_core::Error::from)?
        .into())
}

/// The request for Claude that turns a CV into a profile (copied by the page), in the app's
/// language.
#[tauri::command]
pub async fn profile_prompt(state: State<'_, AppState>) -> CmdResult<String> {
    Ok(profile::prompt::text(state.language()?))
}

/// Saves the editor: merges the form into the profile (or the draft it came from), keeps
/// the previous file as the backup and scores every job again (in the background).
#[tauri::command]
pub async fn save_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    save: ProfileSave,
) -> CmdResult<ProfileInfo> {
    state.ensure_real()?;
    let workspace = state.workspace()?;
    let info = profile::save_form(
        &workspace,
        save.source.as_deref(),
        &save.before,
        &save.after,
    )?;
    // The file is now the one the app keeps; a chosen file's name no longer applies.
    if let Err(e) = state.store.kv_set(PROFILE_SOURCE, profile::PROFILE_FILE) {
        log::warn!("profile file name not stored: {e}");
    }
    scoring::profile_changed(&app, &state);
    Ok(profile_info(&state, &workspace).unwrap_or_else(|| ProfileInfo::of(&info, None)))
}

/// Removes the profile; the scores go with it.
#[tauri::command]
pub async fn remove_profile(app: AppHandle, state: State<'_, AppState>) -> CmdResult<bool> {
    state.ensure_real()?;
    let removed = profile::remove(&state.workspace()?)?;
    scoring::profile_changed(&app, &state);
    Ok(removed)
}
