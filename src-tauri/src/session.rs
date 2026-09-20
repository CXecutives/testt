//! freelance.de-Sitzungsfenster: ein unsichtbares WebView (echte Edge-Engine) mit eigenem,
//! dauerhaftem Profil. Der Nutzer meldet sich darin einmal selbst an („angemeldet bleiben“);
//! danach ruft die App Projektseiten wie ein ruhiger Browser ab – ohne Tarnung, ohne
//! eigenen User-Agent, ohne Abbruch laufender Ladevorgänge, ohne blockierte Seitenteile.
//!
//! Das Fenster steht in keiner Capability (kein Befehl der App ist von dort erreichbar),
//! darf nur `*.freelance.de` laden, öffnet keine weiteren Fenster und lädt nichts herunter.
//! Ausgewertet wird per Host-`eval` (kein IPC); Text entsteht in Rust.
//!
//! Bekanntes Restsignal: Tauri legt `__TAURI_INTERNALS__` und `isTauri` in jeder Seite als
//! nicht löschbare Eigenschaften an (geprüft im Tauri-Quelltext 2.11.5) – ein Skript der Seite
//! könnte sie sehen. Ein bekanntes Prüfen darauf gibt es nicht.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jobalert_core::fetch::freelance_de::{
    self, LOGIN_URL, LOGOUT_URL, PROBE_JS, SessionPage, is_portal_url,
};
use jobalert_core::fetch::policy::DWELL_SECS;
use jobalert_core::fetch::{Login, PageFetcher, PageOutcome};
use jobalert_core::pipeline::RunEvent;
use jobalert_core::portal::{JobLink, Portal};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// Fensterkennung – derselbe Name wie der Profilordner.
const LABEL: &str = jobalert_core::SESSION_DIR;
/// So lange darf eine Seite laden.
const LOAD_TIMEOUT: Duration = Duration::from_secs(45);
/// Nach dem Laden: Seite sich setzen lassen, dann **ein** Befund.
const SETTLE: Duration = Duration::from_secs(2);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// So lange wartet die App auf die Anmeldung des Nutzers.
const LOGIN_WAIT: Duration = Duration::from_secs(5 * 60);

/// Was das Fenster meldet (die Handler tun nichts anderes).
#[derive(Debug)]
enum Nav {
    Finished(Url),
    Closed,
}

/// Meldungen an die Oberfläche (Anmeldung nötig / erledigt).
pub type Notify = Arc<dyn Fn(RunEvent) + Send + Sync>;

pub struct Session {
    app: AppHandle,
    profile: PathBuf,
    window: Option<WebviewWindow>,
    events: Option<mpsc::UnboundedReceiver<Nav>>,
    /// Frühester Zeitpunkt der nächsten Navigation (Verweildauer).
    next_at: Option<Instant>,
    notify: Notify,
}

impl Session {
    pub fn new(app: AppHandle, profile: PathBuf, notify: Notify) -> Session {
        Session {
            app,
            profile,
            window: None,
            events: None,
            next_at: None,
            notify,
        }
    }

    /// Nur freelance.de über https – verhindert auch den Sprung auf eigene App-Adressen.
    fn allowed(url: &Url) -> bool {
        url.as_str() == "about:blank" || is_portal_url(url)
    }

    fn open(&mut self, url: &Url, visible: bool) -> Result<(), String> {
        if self.window.is_some() {
            return Ok(());
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let on_load = tx.clone();
        let window = WebviewWindowBuilder::new(&self.app, LABEL, WebviewUrl::External(url.clone()))
            .title("freelance.de – Anmeldung")
            .data_directory(self.profile.clone())
            .inner_size(1100.0, 820.0)
            .center()
            .visible(visible)
            .on_page_load(move |_, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    let _ = on_load.send(Nav::Finished(payload.url().clone()));
                }
            })
            .on_navigation(|url| {
                let ok = Session::allowed(url);
                if !ok {
                    log::warn!(
                        "Sitzungsfenster: Navigation verweigert ({})",
                        url.host_str().unwrap_or("?")
                    );
                }
                ok
            })
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .on_download(|_, _| false)
            .build()
            .map_err(|e| format!("Sitzungsfenster nicht erzeugt: {e}"))?;
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                let _ = tx.send(Nav::Closed);
            }
        });
        self.window = Some(window);
        self.events = Some(rx);
        Ok(())
    }

    /// Alte Meldungen verwerfen; ein inzwischen geschlossenes Fenster bemerken.
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

    /// Verweildauer der vorigen Seite abwarten; `false` bei Abbruch.
    async fn await_dwell(&mut self, cancel: &CancellationToken) -> bool {
        match self.next_at {
            Some(at) => sleep_until(at, cancel).await,
            None => true,
        }
    }

    /// Die Verweildauer für die nächste Seite setzen (zufällig im Bereich).
    fn arm_dwell(&mut self) {
        self.next_at = Some(Instant::now() + Duration::from_secs(fastrand::u64(DWELL_SECS)));
    }

    /// Lädt `url` und wartet, bis die Seite fertig ist (samt Umleitungen). Beachtet die
    /// Verweildauer der vorigen Seite.
    async fn load(&mut self, url: &Url, cancel: &CancellationToken) -> Result<Url, PageOutcome> {
        if !self.await_dwell(cancel).await {
            return Err(PageOutcome::Cancelled);
        }
        self.drain();
        if let Some(window) = &self.window {
            window
                .navigate(url.clone())
                .map_err(|e| PageOutcome::NetError {
                    timeout: false,
                    detail: e.to_string(),
                })?;
        } else {
            self.open(url, false)
                .map_err(|detail| PageOutcome::NetError {
                    timeout: false,
                    detail,
                })?;
        }
        // Ab hier ist die Seite angefragt: Die Verweildauer gilt auch, wenn sie nicht
        // fertig wird – gerade bei Zeitüberschreitungen ist Zurückhaltung richtig.
        self.arm_dwell();
        let finished = self.wait_finished(cancel).await?;
        self.arm_dwell();
        if !sleep_until(Instant::now() + SETTLE, cancel).await {
            return Err(PageOutcome::Cancelled);
        }
        Ok(finished)
    }

    async fn wait_finished(&mut self, cancel: &CancellationToken) -> Result<Url, PageOutcome> {
        let Some(events) = self.events.as_mut() else {
            return Err(PageOutcome::NetError {
                timeout: false,
                detail: "kein Fenster".into(),
            });
        };
        let event = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(PageOutcome::Cancelled),
            () = tokio::time::sleep(LOAD_TIMEOUT) => {
                return Err(PageOutcome::NetError { timeout: true, detail: "Seite lädt nicht".into() });
            }
            event = events.recv() => event,
        };
        match event {
            Some(Nav::Finished(url)) => Ok(url),
            Some(Nav::Closed) | None => {
                self.window = None;
                Err(PageOutcome::NetError {
                    timeout: false,
                    detail: "Fenster geschlossen".into(),
                })
            }
        }
    }

    /// Ein Befund der aktuellen Seite (Host-`eval`, kein IPC). Eine hängende Seite hält
    /// „Abbrechen“ und „Beenden“ nicht auf: Der Abbruch gilt sofort, nicht erst nach
    /// [`PROBE_TIMEOUT`].
    ///
    /// Der Fehlerfall ist bereits das Ergebnis der Seite: Nur ein wirklich ausgebliebener
    /// oder unlesbarer Befund ist verdächtig (Seitenaufbau geändert?). Abbruch und ein
    /// geschlossenes Fenster sind kein Seitenbefund – sie dürfen weder als Fehlversuch
    /// gebucht werden noch den Schutzschalter füttern.
    async fn probe(&self, cancel: &CancellationToken) -> Result<SessionPage, PageOutcome> {
        let Some(window) = self.window.as_ref() else {
            return Err(PageOutcome::NetError {
                timeout: false,
                detail: "kein Fenster".into(),
            });
        };
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let tx = Mutex::new(Some(tx));
        window
            .eval_with_callback(PROBE_JS, move |raw| {
                if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                    let _ = tx.send(raw);
                }
            })
            .map_err(|e| PageOutcome::NetError {
                timeout: false,
                detail: e.to_string(),
            })?;
        let answer = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(PageOutcome::Cancelled),
            answer = tokio::time::timeout(PROBE_TIMEOUT, rx) => answer,
        };
        let raw = answer
            .map_err(|_| PageOutcome::Suspicious("keine Antwort der Seite".into()))?
            .map_err(|_| PageOutcome::NetError {
                timeout: false,
                detail: "Fenster geschlossen".into(),
            })?;
        // Das Ergebnis ist JSON-kodiert – hier also ein JSON-Text in einem JSON-String.
        let json: String = serde_json::from_str(&raw).unwrap_or(raw);
        serde_json::from_str(&json)
            .map_err(|e| PageOutcome::Suspicious(format!("Befund unlesbar: {e}")))
    }

    /// Anmeldung durch den Nutzer: Fenster sichtbar auf der Anmeldeseite, höchstens fünf
    /// Minuten warten – angemeldet, sobald eine Seite mit Abmelde-Link erscheint.
    pub async fn sign_in(&mut self, cancel: &CancellationToken) -> Login {
        // Auch die Anmeldeseite folgt erst nach der Verweildauer der vorigen Seite.
        if !self.await_dwell(cancel).await {
            return Login::NotSignedIn;
        }
        (self.notify)(RunEvent::LoginNeeded {
            portal: Portal::FreelanceDe,
            waiting: true,
        });
        let login: Url = LOGIN_URL.parse().expect("feste Adresse");
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
        // Auch die Seite nach der Anmeldung bekommt ihre Verweildauer.
        self.arm_dwell();
        (self.notify)(RunEvent::LoginNeeded {
            portal: Portal::FreelanceDe,
            waiting: false,
        });
        log::info!(
            "freelance.de-Anmeldung {}",
            match login {
                Login::SignedIn => "erfolgreich",
                Login::Challenged => "erfolgreich, aber mit Sicherheitsprüfung",
                Login::NotSignedIn => "nicht abgeschlossen",
            }
        );
        login
    }

    /// Wartet, bis eine Seite mit Abmelde-Link erscheint. Zeigte freelance.de dabei eine
    /// Sicherheitsprüfung (Captcha), gilt die Sitzung – das Portal ruht aber bis zum
    /// nächsten Lauf (die App löst nie selbst eine Prüfung).
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
                    // Bleibt der Befund einmal aus (Seite noch beschäftigt), auf derselben
                    // Seite erneut fragen. Sonst wartete die Anmeldung auf eine Navigation,
                    // die nach einem erfolgreichen Login gar nicht mehr kommt.
                    let mut found = None;
                    for _ in 0..3 {
                        match self.probe(cancel).await {
                            Ok(page) => {
                                found = Some(page);
                                break;
                            }
                            Err(_) if sleep_until(Instant::now() + SETTLE, cancel).await => {}
                            Err(_) => return Login::NotSignedIn,
                        }
                    }
                    let Some(page) = found else {
                        continue;
                    };
                    challenged |= page.has_captcha;
                    if page.has_logout && !url.path().starts_with("/login") {
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

    /// Abmelden: die Abmeldeseite laden (unsichtbar), dann Fenster schließen. Erfolgreich
    /// nur, wenn danach wirklich eine Seite des Portals ohne Abmelde-Link geladen ist – eine
    /// gescheiterte Navigation meldet die WebView ebenfalls als „geladen“. Das Profil bleibt;
    /// ohne Sitzungs-Cookie ist es nur ein leerer Browser.
    ///
    /// `false` heißt „nicht bestätigt“, nicht „fehlgeschlagen“: Auch ein Abbruch endet hier –
    /// der Sitzungsstand bleibt dann unverändert (siehe `portal_logout`).
    pub async fn sign_out(&mut self, cancel: &CancellationToken) -> bool {
        let logout: Url = LOGOUT_URL.parse().expect("feste Adresse");
        let done = match self.load(&logout, cancel).await {
            Ok(_) => self.probe(cancel).await.is_ok_and(|page| {
                page.ok
                    && !page.has_logout
                    && Url::parse(&page.url).is_ok_and(|url| is_portal_url(&url))
            }),
            Err(_) => false,
        };
        self.close();
        done
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

impl PageFetcher for Session {
    async fn fetch(&mut self, link: &JobLink, cancel: &CancellationToken) -> PageOutcome {
        if let Err(outcome) = self.load(&link.url, cancel).await {
            return outcome;
        }
        match self.probe(cancel).await {
            // Nach der Anmeldung leitet freelance.de die erste Seite einmal um: Der Abruf
            // wiederholt sie – als eigener, gezählter Zugriff mit Abstand.
            Ok(page) if freelance_de::is_postlogin(&page.url) => {
                PageOutcome::Retry("Weiterleitung nach der Anmeldung".into())
            }
            Ok(page) => freelance_de::judge_page(&page, &link.key.id),
            Err(outcome) => outcome,
        }
    }

    async fn login(&mut self, portal: Portal, cancel: &CancellationToken) -> Login {
        if portal == Portal::FreelanceDe {
            self.sign_in(cancel).await
        } else {
            Login::NotSignedIn
        }
    }
}

async fn sleep_until(at: Instant, cancel: &CancellationToken) -> bool {
    tokio::select! {
        biased;
        () = cancel.cancelled() => false,
        () = tokio::time::sleep_until(at) => true,
    }
}
