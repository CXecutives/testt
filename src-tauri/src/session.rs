//! Session window of a portal: a hidden web view (the real engine, WebView2 or `WKWebView`)
//! with its own persistent storage. The user signs in there once ("stay signed in");
//! afterwards the app fetches pages like a quiet browser - no disguise, no own user agent,
//! no aborting of running loads, no blocked page parts.
//!
//! Which portal it is lives only in [`PortalSite`]: addresses, probe script and judgement
//! come from there. This module knows no single portal. Where the storage lives differs per
//! OS and is `platform.rs`'s business.
//!
//! The window is in no capability (no app command is reachable from it), may only load
//! addresses of its portal, opens no further windows and downloads nothing. Evaluation runs
//! via host `eval` (no IPC); text is produced in Rust.
//!
//! A run never opens the sign-in window unasked: only portals switched to "sign in" via
//! [`Sessions::allow_login`] may show it. The user's own "sign in" action always may.
//!
//! Known residual signal: Tauri defines `__TAURI_INTERNALS__` and `isTauri` in every page as
//! non-deletable properties (checked in the Tauri 2.11.5 source) - a page script could see
//! them. No known check looks for them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jiff::Timestamp;
use jobalert_core::fetch::policy::{DWELL_SECS, Policy};
use jobalert_core::fetch::site::{PortalSite, SessionPage, eval_result};
use jobalert_core::fetch::{Cause, Login, PageFetcher, PageOutcome};
use jobalert_core::pipeline::RunEvent;
use jobalert_core::portal::{JobLink, Portal};
use jobalert_core::time::sleep_cancellable;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::platform;

// ------------------------------------------------------------------ window title
// User-facing text, German by product decision (UI language).
/// Title of the visible sign-in window, after the portal's name ("freelance.de – Anmeldung").
const TEXT_SIGN_IN_TITLE: &str = "Anmeldung";
// ------------------------------------------------------------------ end of user-facing text

/// How long a page may load.
const LOAD_TIMEOUT: Duration = Duration::from_secs(45);
/// After loading: let the page settle, then **one** probe.
const SETTLE: Duration = Duration::from_secs(2);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the app waits for the user to sign in.
const LOGIN_WAIT: Duration = Duration::from_secs(5 * 60);
/// Clearing browsing data completes inside the engine; the cookie jar is polled this long.
const CLEAR_WAIT: Duration = Duration::from_secs(5);
const CLEAR_STEP: Duration = Duration::from_millis(200);

/// What the window reports (the handlers do nothing else).
#[derive(Debug)]
enum Nav {
    Finished(Url),
    Closed,
}

/// Messages to the UI (sign-in needed / done).
pub type Notify = Arc<dyn Fn(RunEvent) + Send + Sync>;

/// The session windows of a run - at most one per portal, opened on first need.
pub struct Sessions {
    app: AppHandle,
    data_dir: PathBuf,
    notify: Notify,
    open: BTreeMap<Portal, Session>,
    /// Portals whose sign-in window this run may show. Empty by default: a run never opens
    /// a window unasked.
    login_allowed: BTreeSet<Portal>,
}

impl Sessions {
    pub fn new(app: AppHandle, data_dir: PathBuf, notify: Notify) -> Sessions {
        Sessions {
            app,
            data_dir,
            notify,
            open: BTreeMap::new(),
            login_allowed: BTreeSet::new(),
        }
    }

    /// Lets this run show the sign-in window of `portal` when the portal asks for a sign-in
    /// (setting "sign in" of that portal). Without it the run treats a sign-in wall as
    /// "not signed in" and never shows a window.
    pub fn allow_login(mut self, portal: Portal, enabled: bool) -> Sessions {
        if enabled {
            self.login_allowed.insert(portal);
        } else {
            self.login_allowed.remove(&portal);
        }
        self
    }

    /// Whether this run may show the sign-in window of `portal`.
    pub fn login_allowed(&self, portal: Portal) -> bool {
        self.login_allowed.contains(&portal)
    }

    /// Window of a portal; `None` for portals without sign-in (LinkedIn).
    fn of(&mut self, portal: Portal) -> Option<&mut Session> {
        let site = PortalSite::of(portal)?;
        let (app, data_dir, notify) = (&self.app, &self.data_dir, &self.notify);
        Some(
            self.open
                .entry(portal)
                .or_insert_with(|| Session::new(app.clone(), data_dir, site, notify.clone())),
        )
    }
}

impl PageFetcher for Sessions {
    async fn fetch(&mut self, link: &JobLink, cancel: &CancellationToken) -> PageOutcome {
        match self.of(link.key.portal) {
            Some(session) => session.fetch_page(link, cancel).await,
            None => PageOutcome::Suspicious(Cause::NoWindow),
        }
    }

    fn session(&self) -> bool {
        true
    }

    async fn login(&mut self, portal: Portal, cancel: &CancellationToken) -> Login {
        if !self.login_allowed(portal) {
            log::info!("{portal}: sign-in is switched off - the run shows no sign-in window");
            return Login::NotSignedIn;
        }
        match self.of(portal) {
            Some(session) => session.sign_in(cancel).await,
            None => Login::NotSignedIn,
        }
    }
}

pub struct Session {
    app: AppHandle,
    site: &'static PortalSite,
    data_dir: PathBuf,
    profile: PathBuf,
    window: Option<WebviewWindow>,
    events: Option<mpsc::UnboundedReceiver<Nav>>,
    /// Earliest time of the next navigation (dwell time).
    next_at: Option<Instant>,
    notify: Notify,
}

impl Session {
    /// `data_dir` is the app's data folder; the profile lies in it under the portal's name.
    pub fn new(
        app: AppHandle,
        data_dir: &Path,
        site: &'static PortalSite,
        notify: Notify,
    ) -> Session {
        Session {
            app,
            site,
            data_dir: data_dir.to_path_buf(),
            profile: data_dir.join(jobalert_core::session_dir(site.portal)),
            window: None,
            events: None,
            next_at: None,
            notify,
        }
    }

    /// Only addresses of the own portal - this also prevents a jump to the app's own pages.
    fn allowed(site: &PortalSite, url: &Url) -> bool {
        url.as_str() == "about:blank" || (site.is_allowed)(url)
    }

    fn open(&mut self, url: &Url, visible: bool) -> Result<(), String> {
        if self.window.is_some() {
            return Ok(());
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let on_load = tx.clone();
        let site = self.site;
        let builder =
            WebviewWindowBuilder::new(&self.app, site.label(), WebviewUrl::External(url.clone()))
                .title(format!("{} – {TEXT_SIGN_IN_TITLE}", site.portal.label()));
        let window = platform::session_storage(builder, site.portal.key(), &self.profile)
            .inner_size(1100.0, 820.0)
            .center()
            .visible(visible)
            .on_page_load(move |_, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    let _ = on_load.send(Nav::Finished(payload.url().clone()));
                }
            })
            .on_navigation(move |url| {
                let ok = Session::allowed(site, url);
                if !ok {
                    log::warn!(
                        "session window {}: navigation refused ({})",
                        site.portal.key(),
                        url.host_str().unwrap_or("?")
                    );
                }
                ok
            })
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .on_download(|_, _| false)
            .build()
            .map_err(|e| format!("session window not created: {e}"))?;
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                let _ = tx.send(Nav::Closed);
            }
        });
        self.window = Some(window);
        self.events = Some(rx);
        Ok(())
    }

    /// Drops old messages; notices a window closed in the meantime.
    fn drain(&mut self) {
        let Some(events) = self.events.as_mut() else {
            return;
        };
        let mut closed = false;
        while let Ok(event) = events.try_recv() {
            closed |= matches!(event, Nav::Closed);
        }
        if closed {
            self.window = None;
            self.events = None;
        }
    }

    fn close(&mut self) {
        if let Some(window) = self.window.take() {
            let _ = window.destroy();
        }
        self.events = None;
    }

    /// Waits out the dwell time of the previous page; `false` on cancel.
    async fn await_dwell(&mut self, cancel: &CancellationToken) -> bool {
        match self.next_at {
            Some(at) => {
                sleep_cancellable(at.saturating_duration_since(Instant::now()), cancel).await
            }
            None => true,
        }
    }

    /// Sets the dwell time for the next page (random within the range).
    fn arm_dwell(&mut self) {
        self.next_at = Some(Instant::now() + Duration::from_secs(fastrand::u64(DWELL_SECS)));
    }

    /// Loads `url` and waits until the page is finished (redirects included). Honours the
    /// dwell time of the previous page.
    async fn load(&mut self, url: &Url, cancel: &CancellationToken) -> Result<Url, PageOutcome> {
        if !self.await_dwell(cancel).await {
            return Err(PageOutcome::Cancelled);
        }
        self.drain();
        if let Some(window) = &self.window {
            window.navigate(url.clone()).map_err(|e| {
                log::warn!("session window: navigation failed: {e}");
                no_window()
            })?;
        } else {
            self.open(url, false).map_err(|detail| {
                log::warn!("{detail}");
                no_window()
            })?;
        }
        // From here on the page is requested: the dwell time applies even if it never
        // finishes - restraint is right especially after a timeout.
        self.arm_dwell();
        let finished = self.wait_finished(cancel).await?;
        self.arm_dwell();
        if !sleep_cancellable(SETTLE, cancel).await {
            return Err(PageOutcome::Cancelled);
        }
        Ok(finished)
    }

    async fn wait_finished(&mut self, cancel: &CancellationToken) -> Result<Url, PageOutcome> {
        let Some(events) = self.events.as_mut() else {
            return Err(no_window());
        };
        let event = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(PageOutcome::Cancelled),
            () = tokio::time::sleep(LOAD_TIMEOUT) => {
                return Err(PageOutcome::NetError { timeout: true, cause: Cause::Timeout });
            }
            event = events.recv() => event,
        };
        match event {
            Some(Nav::Finished(url)) => Ok(url),
            Some(Nav::Closed) | None => {
                self.window = None;
                Err(window_closed())
            }
        }
    }

    /// One probe of the current page (host `eval`, no IPC). A hanging page does not hold up
    /// "cancel" and "quit": the cancel applies at once, not only after [`PROBE_TIMEOUT`].
    ///
    /// The error case is already the page's result: only a probe that truly failed to
    /// arrive or is unreadable is suspicious (page layout changed?). A cancel and a closed
    /// window are no page result - they must neither count as a failed attempt nor feed the
    /// circuit breaker.
    async fn probe(&self, cancel: &CancellationToken) -> Result<SessionPage, PageOutcome> {
        let Some(window) = self.window.as_ref() else {
            return Err(no_window());
        };
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let tx = Mutex::new(Some(tx));
        window
            .eval_with_callback(self.site.probe_js, move |raw| {
                if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                    let _ = tx.send(raw);
                }
            })
            .map_err(|e| {
                log::warn!("session window: probe not started: {e}");
                no_window()
            })?;
        let answer = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(PageOutcome::Cancelled),
            answer = tokio::time::timeout(PROBE_TIMEOUT, rx) => answer,
        };
        let raw = answer
            .map_err(|_| PageOutcome::Suspicious(Cause::NoProbeAnswer))?
            .map_err(|_| window_closed())?;
        serde_json::from_str(&eval_result(raw))
            .map_err(|_| PageOutcome::Suspicious(Cause::ProbeUnreadable))
    }

    async fn fetch_page(&mut self, link: &JobLink, cancel: &CancellationToken) -> PageOutcome {
        if let Err(outcome) = self.load(&link.url, cancel).await {
            return outcome;
        }
        match self.probe(cancel).await {
            // Some portals redirect the first page after sign-in once: the fetch repeats it -
            // as its own, counted access with a pause.
            Ok(page) if (self.site.is_postlogin)(&page.url) => {
                PageOutcome::Retry(Cause::PostLoginRedirect)
            }
            Ok(page) => (self.site.judge)(&page, &link.key.id),
            Err(outcome) => outcome,
        }
    }

    /// Sign-in by the user: window visible on the sign-in page, wait at most five minutes -
    /// signed in as soon as a page shows the portal as signed in.
    pub async fn sign_in(&mut self, cancel: &CancellationToken) -> Login {
        // The sign-in page too follows only after the dwell time of the previous page.
        if !self.await_dwell(cancel).await {
            return Login::NotSignedIn;
        }
        let portal = self.site.portal;
        (self.notify)(RunEvent::LoginNeeded {
            portal,
            waiting: true,
        });
        let login: Url = self.site.login_url.parse().expect("fixed address");
        self.drain();
        let shown = match &self.window {
            Some(window) => window.navigate(login.clone()).is_ok() && window.show().is_ok(),
            None => self.open(&login, true).is_ok(),
        };
        if let Some(window) = &self.window {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        let login = if shown {
            self.await_login(cancel).await
        } else {
            Login::NotSignedIn
        };
        if let Some(window) = &self.window {
            let _ = window.hide();
        }
        // The page after the sign-in gets its dwell time too.
        self.arm_dwell();
        (self.notify)(RunEvent::LoginNeeded {
            portal,
            waiting: false,
        });
        log::info!(
            "{portal} sign-in {}",
            match login {
                Login::SignedIn => "succeeded",
                Login::Challenged => "succeeded, but with a security check",
                Login::NotSignedIn => "not completed",
            }
        );
        login
    }

    /// Waits until a page shows the portal as signed in. If it showed a security check
    /// (captcha) on the way, the session counts - but the portal rests until the next run
    /// (the app never solves a check itself).
    async fn await_login(&mut self, cancel: &CancellationToken) -> Login {
        let deadline = Instant::now() + LOGIN_WAIT;
        let mut challenged = false;
        loop {
            let Some(events) = self.events.as_mut() else {
                return Login::NotSignedIn;
            };
            let event = tokio::select! {
                biased;
                () = cancel.cancelled() => return Login::NotSignedIn,
                () = tokio::time::sleep_until(deadline) => return Login::NotSignedIn,
                event = events.recv() => event,
            };
            match event {
                Some(Nav::Finished(url)) => {
                    // If the probe fails once (page still busy), ask again on the same page.
                    // Otherwise the sign-in would wait for a navigation that never comes
                    // after a successful login.
                    let mut found = None;
                    for _ in 0..3 {
                        match self.probe(cancel).await {
                            Ok(page) => {
                                found = Some(page);
                                break;
                            }
                            Err(_) if sleep_cancellable(SETTLE, cancel).await => {}
                            Err(_) => return Login::NotSignedIn,
                        }
                    }
                    let Some(page) = found else {
                        continue;
                    };
                    challenged |= page.has_captcha;
                    // The sign-in page itself never counts as signed in - not even if it
                    // already shows an account menu.
                    let on_login_page = self
                        .site
                        .login_url
                        .parse::<Url>()
                        .is_ok_and(|l| l.path() == url.path());
                    if (self.site.signed_in)(&page) && !on_login_page {
                        return if challenged {
                            Login::Challenged
                        } else {
                            Login::SignedIn
                        };
                    }
                }
                Some(Nav::Closed) | None => {
                    self.window = None;
                    self.events = None;
                    return Login::NotSignedIn;
                }
            }
        }
    }

    /// Sign out and delete the session: load the portal's logout page (hidden), clear all
    /// browsing data of the window, close it, delete its storage (`platform.rs`) and forget
    /// the confirmed session in `policy.json`. `true` only once the local session is
    /// verifiably gone: no cookie left in the window, storage deleted. The portal's own
    /// logout is best effort (offline, changed page) - without cookies and storage the app
    /// is signed out either way.
    ///
    /// `false` means "not confirmed", not "failed". A cancel ends here too, before anything
    /// is deleted - the session state then stays unchanged (see `portal_logout`).
    pub async fn sign_out(&mut self, cancel: &CancellationToken) -> bool {
        let logout: Url = self.site.logout_url.parse().expect("fixed address");
        let site = self.site;
        let portal = site.portal.key();
        let remote = match self.load(&logout, cancel).await {
            Ok(_) => self.probe(cancel).await.is_ok_and(|page| {
                page.ok
                    && !(site.signed_in)(&page)
                    && Url::parse(&page.url).is_ok_and(|url| (site.is_allowed)(&url))
            }),
            Err(_) => false,
        };
        if cancel.is_cancelled() {
            self.close();
            return false;
        }
        if !remote {
            log::warn!("{portal}: logout page not confirmed - deleting the local session anyway");
        }
        let cookies_gone = self.clear_browsing_data().await;
        self.close();
        let storage_gone = platform::delete_session_storage(&self.app, portal, &self.profile).await;
        self.forget_session();
        let done = cookies_gone && storage_gone;
        log::info!(
            "{portal} sign-out: remote logout {remote}, cookies gone {cookies_gone}, storage gone {storage_gone}"
        );
        done
    }

    /// Clears cookies, cache and storage of the open window and waits until its cookie jar
    /// is empty. Without a window there is nothing in memory to clear (`true`).
    async fn clear_browsing_data(&self) -> bool {
        let Some(window) = self.window.clone() else {
            return true;
        };
        if let Err(error) = window.clear_all_browsing_data() {
            log::warn!("browsing data not cleared: {error}");
            return false;
        }
        for _ in 0..(CLEAR_WAIT.as_millis() / CLEAR_STEP.as_millis()) {
            // Reading cookies blocks until the engine answers on the UI thread (WebView2
            // deadlocks if that happens on it): ask from a worker thread.
            let jar = window.clone();
            let left =
                tauri::async_runtime::spawn_blocking(move || jar.cookies().map(|c| c.len())).await;
            match left {
                Ok(Ok(0)) => return true,
                Ok(Ok(_)) => tokio::time::sleep(CLEAR_STEP).await,
                Ok(Err(error)) => {
                    log::warn!("cookies not readable: {error}");
                    return false;
                }
                Err(error) => {
                    log::warn!("cookie check failed: {error}");
                    return false;
                }
            }
        }
        false
    }

    /// The confirmed session is gone: the next run needs a sign-in again.
    fn forget_session(&self) {
        let mut policy = Policy::load(
            &self.data_dir.join(jobalert_core::POLICY_FILE),
            Timestamp::now(),
        );
        policy.forget_session(self.site.portal);
        if let Err(error) = policy.save() {
            log::warn!("session state not saved: {error}");
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// The window is gone or could not be used - a network-like error, never a page result.
fn no_window() -> PageOutcome {
    PageOutcome::NetError {
        timeout: false,
        cause: Cause::NoWindow,
    }
}

/// The user closed the window.
fn window_closed() -> PageOutcome {
    PageOutcome::NetError {
        timeout: false,
        cause: Cause::WindowClosed,
    }
}
