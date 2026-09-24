//! The consultant profile: choose, remove, save a template.

use std::path::PathBuf;

use jobalert_core::profile;
use jobalert_core::view::ProfileInfo;
use tauri::{AppHandle, State, WebviewWindow};

use super::app::{PROFILE_SOURCE, profile_info};
use super::{AppState, CmdResult, scoring, texts};

/// Chooses the profile file; every job is scored again with it (in the background).
#[tauri::command]
pub async fn pick_profile(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> CmdResult<Option<ProfileInfo>> {
    state.ensure_real()?;
    let workspace = state.workspace()?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title(texts::PICK_PROFILE)
        .add_filter(texts::PROFILE_FILTER, &["json"])
        .set_directory(&workspace)
        .set_parent(&window)
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let info = profile::save_from(&workspace, file.path())?;
    let name = file.file_name();
    if let Err(e) = state.store.kv_set(PROFILE_SOURCE, &name) {
        log::warn!("profile file name not stored: {e}");
    }
    scoring::profile_changed(&app, &state);
    Ok(Some(
        profile_info(&state, &workspace).unwrap_or_else(|| ProfileInfo::of(&info, Some(name))),
    ))
}

/// Removes the profile; the scores go with it.
#[tauri::command]
pub async fn remove_profile(app: AppHandle, state: State<'_, AppState>) -> CmdResult<bool> {
    state.ensure_real()?;
    let removed = profile::remove(&state.workspace()?)?;
    scoring::profile_changed(&app, &state);
    Ok(removed)
}

/// Saves an empty profile template where the user chooses; `None` if cancelled.
#[tauri::command]
pub async fn save_profile_template(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> CmdResult<Option<PathBuf>> {
    state.ensure_real()?;
    let workspace = state.workspace()?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title(texts::SAVE_TEMPLATE)
        .add_filter(texts::PROFILE_FILTER, &["json"])
        .set_file_name(texts::TEMPLATE_NAME)
        .set_directory(&workspace)
        .set_parent(&window)
        .save_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_path_buf();
    profile::write_template(&path)?;
    Ok(Some(path))
}
