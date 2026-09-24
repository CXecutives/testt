//! Signing in and out at a portal by hand (session window).

use std::sync::{Arc, Mutex};

use jiff::Timestamp;
use jobalert_core::error::{ErrorInfo, ErrorKind, InvalidInput};
use jobalert_core::fetch::policy::Policy;
use jobalert_core::fetch::site::PortalSite;
use jobalert_core::fetch::{Admission, Login, StopReason, admit};
use jobalert_core::portal::Portal;
use tauri::{AppHandle, State};
use tokio_util::sync::CancellationToken;

use super::{Activity, AppState, CmdResult, lock};
use crate::session::Session;

/// Holds "session window in use" and frees it safely at the end.
struct SessionGuard<'a> {
    app: AppHandle,
    state: &'a AppState,
    /// Cancels signing in or out (`cancel_run`, quitting).
    cancel: CancellationToken,
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        {
            let mut activity = lock(&self.state.activity);
            if matches!(*activity, Activity::Session(_)) {
                *activity = Activity::Idle;
            }
        }
        // A profile change while the window was open found the slot busy: its rescore is
        // owed and starts now.
        super::scoring::after_run(&self.app);
    }
}

/// Claims the session window for signing in or out - checked and claimed under one lock,
/// never next to a run. Then the same rules apply as for every request: no sign-in during a
/// pause or above the cap, gap to the last request, counted and saved before the contact.
/// `None`: cancelled before the portal was contacted.
async fn claim_session<'a>(
    app: &AppHandle,
    state: &'a AppState,
    portal: Portal,
) -> CmdResult<Option<SessionGuard<'a>>> {
    state.ensure_real()?;
    if PortalSite::of(portal).is_none() {
        return Err(ErrorInfo::from(&InvalidInput::NoSignIn { portal }));
    }
    let cancel = CancellationToken::new();
    {
        let mut activity = lock(&state.activity);
        if !matches!(*activity, Activity::Idle) {
            return Err(ErrorInfo::new(ErrorKind::Busy));
        }
        *activity = Activity::Session(cancel.clone());
    }
    let guard = SessionGuard {
        app: app.clone(),
        state,
        cancel,
    };
    let policy = Mutex::new(Policy::load(&state.policy_path(), Timestamp::now()));
    match admit(&policy, portal, &guard.cancel, &Timestamp::now, |_| {}).await? {
        Admission::Go => Ok(Some(guard)),
        Admission::Cancelled => Ok(None),
        Admission::Stop(StopReason::Quota { next_at }) => {
            Err(ErrorInfo::new(ErrorKind::PortalQuota)
                .with("portal", portal.key())
                .with("until", next_at.to_string()))
        }
        Admission::Stop(stop) => {
            log::info!("{}", stop.log_line(portal, 0));
            let health = serde_json::to_value(stop.health()).unwrap_or_default();
            let mut error = ErrorInfo::new(ErrorKind::PortalPaused).with("portal", portal.key());
            for field in ["until", "reason"] {
                if let Some(value) = health.get(field) {
                    error = error.with(field, value.clone());
                }
            }
            Err(error)
        }
    }
}

/// End of signing in or out at a portal: the session state, if known, and - as the last
/// answer - the time from which the next request keeps its gap.
fn record_session(state: &AppState, portal: Portal, signed_in: Option<bool>) {
    let now = Timestamp::now();
    let mut policy = Policy::load(&state.policy_path(), now);
    if let Some(signed_in) = signed_in {
        policy.set_session(portal, signed_in, now);
    }
    policy.record_done(portal, now);
    if let Err(e) = policy.save() {
        log::warn!("session state not saved: {e}");
    }
}

/// A session window for signing in or out by hand (without run events).
fn session_window(app: AppHandle, state: &AppState, portal: Portal) -> Option<Session> {
    let site = PortalSite::of(portal)?;
    Some(Session::new(app, &state.data_dir, site, Arc::new(|_| {})))
}

/// Sign in at a portal once by yourself (window visible, at most 5 minutes). The app never
/// sees or stores the password. After a security check the sign-in counts anyway (the next
/// run fetches again).
#[tauri::command]
pub async fn portal_login(
    app: AppHandle,
    state: State<'_, AppState>,
    portal: Portal,
) -> CmdResult<bool> {
    let Some(guard) = claim_session(&app, &state, portal).await? else {
        return Ok(false);
    };
    let Some(mut session) = session_window(app, &state, portal) else {
        return Ok(false);
    };
    let login = session.sign_in(&guard.cancel).await;
    drop(session);
    let signed_in = login != Login::NotSignedIn;
    // Cancelled (quitting): whether a session exists is open - the state stays.
    let known = if signed_in || !guard.cancel.is_cancelled() {
        Some(signed_in)
    } else {
        None
    };
    record_session(&state, portal, known);
    log::info!("{}: signed in by hand: {signed_in}", portal.key());
    Ok(signed_in)
}

/// Sign out at a portal. Other portals are not affected (own profile). Only a really
/// loaded sign-out page ends the session - otherwise the state stays.
#[tauri::command]
pub async fn portal_logout(
    app: AppHandle,
    state: State<'_, AppState>,
    portal: Portal,
) -> CmdResult<bool> {
    let Some(guard) = claim_session(&app, &state, portal).await? else {
        return Ok(false);
    };
    let Some(mut session) = session_window(app, &state, portal) else {
        return Ok(false);
    };
    let done = session.sign_out(&guard.cancel).await;
    drop(session);
    record_session(&state, portal, done.then_some(false));
    log::info!("{}: signed out by hand: {done}", portal.key());
    Ok(done)
}
