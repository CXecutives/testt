//! The Gmail mailbox: address and app password in the system vault.

use jobalert_core::mail::imap::Credentials;
use jobalert_core::secrets::Vault;
use jobalert_core::view::Mailbox;
use tauri::State;

use super::app::mailbox;
use super::{AppState, CmdResult, GmailUser, lock};

/// Saves the Gmail access. Another account starts with its own scan state.
#[tauri::command]
pub async fn save_mailbox(
    state: State<'_, AppState>,
    user: String,
    password: String,
) -> CmdResult<Mailbox> {
    state.ensure_idle()?;
    state.ensure_real()?;
    let credentials = Credentials::new(&user, &password);
    let vault = Vault::app();
    let cached = match &*lock(&state.gmail_user) {
        GmailUser::Known(user) => Some(user.clone()),
        GmailUser::Unread => None,
    };
    let previous = cached.unwrap_or_else(|| vault.load_gmail().ok().flatten().map(|c| c.user));
    vault.save_gmail(&credentials)?;
    *lock(&state.gmail_user) = GmailUser::Known(Some(credentials.user.clone()));
    if previous.as_deref() != Some(credentials.user.as_str()) {
        state.store.clear_scan_state()?;
    }
    log::info!("mailbox saved");
    Ok(mailbox(&state))
}

#[tauri::command]
pub async fn remove_mailbox(state: State<'_, AppState>) -> CmdResult<bool> {
    state.ensure_idle()?;
    state.ensure_real()?;
    let removed = Vault::app().delete_gmail()?;
    *lock(&state.gmail_user) = GmailUser::Known(None);
    state.store.clear_scan_state()?;
    log::info!("mailbox removed");
    Ok(removed)
}
