#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod session;
#[cfg(debug_assertions)]
mod smoke;

use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use commands::{Activity, AppState, GmailUser};
use jobalert_core::fetch::http::edge_user_agent;
use jobalert_core::secrets::Vault;
use jobalert_core::store::Store;
use tauri::Manager;

/// Exit-Code aus `AppHandle::exit(code)`. Tauri beendet den Prozess unter Windows sonst
/// immer mit 0 (die Ereignisschleife kennt nur `ExitWithCode(0)`).
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

fn main() {
    // Trockenlauf: Datenbank und Sicherheitsstand nur im Arbeitsspeicher, Attrappen statt
    // Postfach und Portalen, keine Dateien.
    let dry_run = std::env::args().any(|arg| arg == "--dry-run");
    let app = tauri::Builder::default()
        // Muss als erstes Plugin registriert werden: Ein zweiter Start holt nur das
        // vorhandene Fenster nach vorn.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            commands::app_state,
            commands::save_settings,
            commands::pick_workspace,
            commands::save_gmail_credentials,
            commands::delete_gmail_credentials,
            commands::start_run,
            commands::cancel_run,
            commands::list_jobs,
            commands::job_detail,
            commands::pick_profile,
            commands::remove_profile,
            commands::rewrite_txt,
            commands::clear_result_files,
            commands::open_target,
            commands::reset_all,
            commands::report_ui_error,
            commands::close_answered,
            commands::quit,
            commands::portal_login,
            commands::portal_logout,
        ])
        // Ein Fehler beim Start (Datenbank, WebView2 …) erscheint als Meldung mit Ursache und
        // Rat – ein GUI-Programm ohne Konsole endete sonst wortlos.
        .setup(move |app| {
            if let Err(message) = setup(app, dry_run) {
                fail(&message);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| fail(&window_failure(&error)));
    app.run_return(|_, event| {
        if let tauri::RunEvent::ExitRequested {
            code: Some(code), ..
        } = event
        {
            EXIT_CODE.store(code, Ordering::SeqCst);
        }
    });
    std::process::exit(EXIT_CODE.load(Ordering::SeqCst));
}

/// Startfehler anzeigen und beenden. `message` nennt Ursache und Rat.
fn fail(message: &str) -> ! {
    log::error!("Start fehlgeschlagen: {message}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Job-Alert-Monitor")
        .set_description(format!(
            "Die App konnte nicht starten:\n\n{message}\n\nDetails stehen im Protokoll unter {}.",
            log_hint()
        ))
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    std::process::exit(1);
}

/// Wo das Protokoll liegt – als Hinweis in Meldungen, bevor es einen App-Handle gibt.
fn log_hint() -> &'static str {
    if cfg!(windows) {
        "%LOCALAPPDATA%\\de.cxecutives.job-alert-monitor\\logs"
    } else {
        "im Datenordner der App (Unterordner „logs“)"
    }
}

/// Fenster oder Oberfläche ließen sich nicht erzeugen – unter Windows fehlt meist die
/// WebView2-Laufzeit; andere Systeme bringen ihre Engine selbst mit.
fn window_failure(error: &dyn std::fmt::Display) -> String {
    let hint = if cfg!(windows) {
        "\n\nFehlt die Microsoft-Edge-WebView2-Laufzeit, hilft deren Installation \
         (https://developer.microsoft.com/microsoft-edge/webview2/)."
    } else {
        ""
    };
    format!("Das Fenster ließ sich nicht öffnen: {error}{hint}")
}

/// Die Datenbank ließ sich nicht öffnen: Datei, Ursache und was zu tun ist. Eine Datenbank
/// einer neueren Version ist nicht beschädigt – sie darf nicht beiseitegelegt werden.
fn database_failure(path: &Path, error: &jobalert_core::Error) -> String {
    if matches!(error, jobalert_core::Error::NewerSchema(_)) {
        return format!(
            "{error}\n\nDatei: {}\n\nBitte die neuere Version des Job-Alert-Monitors \
             verwenden. Die Daten bleiben unverändert.",
            path.display()
        );
    }
    format!(
        "Die Datenbank ließ sich nicht öffnen:\n{}\n\n{error}\n\n\
         Benutzt ein anderes Programm die Datei gerade, dieses schließen und die App neu \
         starten. Ist sie beschädigt, die Datei umbenennen (beiseitelegen) – die App legt \
         beim nächsten Start eine neue an. Folge: Die bisherigen Jobs und Einstellungen sind \
         dann nicht mehr bekannt, und der nächste Abruf legt ihre Textdateien neu an.",
        path.display()
    )
}

/// Start: Protokoll → Panik-Hook → Krypto → offenes Zurücksetzen → Datenbank → Fenster.
/// Der Fehler ist die fertige Meldung für den Nutzer.
fn setup(app: &mut tauri::App, dry_run: bool) -> Result<(), String> {
    let data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    if let Err(error) =
        jobalert_core::logging::FileLogger::install(&data_dir.join(jobalert_core::LOG_DIR), level)
    {
        eprintln!("Protokoll nicht verfügbar: {error}");
    }
    // Ohne Konsole (GUI-Programm) ginge eine Panik sonst spurlos verloren.
    std::panic::set_hook(Box::new(|info| log::error!("Absturz: {info}")));
    jobalert_core::install_crypto();
    // Ein angefordertes Zurücksetzen läuft vor allem anderen – nichts hält jetzt
    // eine Datei offen. Der Trockenlauf löscht nie etwas.
    let reset_report = (!dry_run)
        .then(|| jobalert_core::reset::perform_pending(&data_dir, &Vault::app()))
        .flatten();
    let database = data_dir.join(jobalert_core::DB_FILE);
    let store = if dry_run {
        Store::in_memory()
    } else {
        Store::open(&database)
    };
    let store = Arc::new(store.map_err(|e| database_failure(&database, &e))?);
    let webview_version = tauri::webview_version().unwrap_or_default();
    app.manage(AppState {
        store: store.clone(),
        default_workspace: app
            .path()
            .document_dir()
            .map_err(|e| e.to_string())?
            .join("Job-Alert-Monitor"),
        data_dir,
        dry_run,
        user_agent: edge_user_agent(Some(&webview_version)),
        webview_version,
        reset_report: Mutex::new(reset_report),
        gmail_user: Mutex::new(GmailUser::Unread),
        activity: Mutex::new(Activity::Idle),
        close_asked: Mutex::new(None),
    });
    log::info!(
        "Start {}{}",
        env!("CARGO_PKG_VERSION"),
        if dry_run { " (Trockenlauf)" } else { "" }
    );

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .ok_or("Fensterkonfiguration 'main' fehlt")?;
    let builder = tauri::WebviewWindowBuilder::from_config(app.handle(), &config)
        .map_err(|e| window_failure(&e))?;
    #[cfg(debug_assertions)]
    let builder = smoke::attach(builder);
    let window = builder.build().map_err(|e| window_failure(&e))?;
    geometry::restore(&window, &store);
    tool_mode::apply(&window).map_err(|e| window_failure(&e))?;
    lifecycle::watch(&window, store);
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--devtools") {
        window.open_devtools();
    }
    Ok(())
}

/// Ein Werkzeug, keine Webseite: kein Browser-Kontextmenü (Zurück, Neu laden, Drucken,
/// Untersuchen …) und keine Browser-Tastenkürzel (F5/Strg+R, Strg+P, Strg+F, Zoom, F12,
/// Entwicklertools). Bearbeiten bleibt: Strg+C/V/X/A/Z, Pos1/Ende, Bild auf/ab.
/// Gilt in jedem Build, damit genau das geprüft wird, was ausgeliefert wird.
mod tool_mode {
    use tauri::{Runtime, WebviewWindow};

    #[cfg(windows)]
    pub fn apply<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
        window.with_webview(|webview| {
            if let Err(error) = disable_browser_features(&webview.controller()) {
                log::warn!("WebView2-Einstellungen nicht gesetzt: {error}");
            }
        })
    }

    /// macOS (`WKWebView`): Es gibt keine entsprechenden Schalter. Das Kontextmenü hängt am
    /// `WKUIDelegate` und der umgebenden `NSView`, die Tastenkürzel am Hauptmenü der App –
    /// beides wäre nur über die Objective-C-Laufzeit zu erreichen und brächte eine zweite
    /// `unsafe`-Stelle. Nötig ist es nicht: `WKWebView` bindet weder Neu laden noch Drucken,
    /// Suchen oder die Entwicklertools auf Tasten, und die Oberfläche sperrt Kontextmenü
    /// und Browser-Kürzel ohnehin plattformneutral in JavaScript. Hier bleibt nichts zu tun.
    #[cfg(not(windows))]
    #[allow(
        clippy::unnecessary_wraps,
        reason = "dieselbe Signatur wie der Windows-Zweig"
    )]
    pub fn apply<R: Runtime>(_window: &WebviewWindow<R>) -> tauri::Result<()> {
        Ok(())
    }

    /// Die einzige `unsafe`-Stelle der App: Tauri reicht diese beiden WebView2-Schalter
    /// nicht durch, also werden sie direkt an der WebView2 gesetzt.
    #[cfg(windows)]
    #[expect(unsafe_code, reason = "WebView2-Schalter, die Tauri nicht durchreicht")]
    fn disable_browser_features(
        controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    ) -> windows_core::Result<()> {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows_core::Interface as _;
        // SAFETY: `with_webview` ruft uns auf dem UI-Thread mit dem gültigen Controller
        // dieses Fensters auf; es sind reine COM-Setter ohne Zeiger aus Rust.
        unsafe {
            let settings = controller.CoreWebView2()?.Settings()?;
            settings.SetAreDefaultContextMenusEnabled(false)?;
            settings
                .cast::<ICoreWebView2Settings3>()?
                .SetAreBrowserAcceleratorKeysEnabled(false)
        }
    }
}

/// Fensterlage (Größe, Position, maximiert) über Neustarts hinweg – nur, wenn sie auf einem
/// vorhandenen Bildschirm liegt (sonst bleibt das Fenster zentriert).
mod geometry {
    use jobalert_core::store::Store;
    use serde::{Deserialize, Serialize};
    use tauri::{PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

    const KEY: &str = "window";

    #[derive(Serialize, Deserialize)]
    struct Geometry {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        maximized: bool,
    }

    fn load(store: &Store) -> Option<Geometry> {
        store
            .kv_get(KEY)
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
    }

    pub fn restore<R: Runtime>(window: &WebviewWindow<R>, store: &Store) {
        let Some(g) = load(store) else { return };
        // Nur „maximiert“ bekannt (zuerst maximiert geschlossen): Lage bleibt die Vorgabe.
        if g.width == 0 {
            if g.maximized {
                let _ = window.maximize();
            }
            return;
        }
        // Die Titelleiste muss auf einem vorhandenen Bildschirm greifbar sein.
        let monitors = window.available_monitors().unwrap_or_default();
        let Some(monitor) = monitors.iter().find(|m| {
            let (pos, size) = (m.position(), m.size());
            let right = pos
                .x
                .saturating_add(i32::try_from(size.width).unwrap_or(i32::MAX));
            let bottom = pos
                .y
                .saturating_add(i32::try_from(size.height).unwrap_or(i32::MAX));
            let middle = g.x.saturating_add(i32::try_from(g.width / 2).unwrap_or(0));
            middle > pos.x + 100 && middle < right - 100 && g.y >= pos.y && g.y < bottom - 40
        }) else {
            return;
        };
        let fitted = fit(&g, *monitor.position(), *monitor.size());
        // Erst verschieben, dann die Größe: Ein Wechsel auf einen Bildschirm mit anderer
        // Skalierung rechnete die Größe sonst um.
        let _ = window.set_position(PhysicalPosition::new(g.x, g.y));
        let _ = window.set_size(fitted);
        if g.maximized {
            let _ = window.maximize();
        }
    }

    /// Nie über diesen Bildschirm hinaus – gespeichert wurde die Größe vielleicht auf einem
    /// größeren, der jetzt fehlt; Seitenleiste und Statuszeile lägen sonst außerhalb. Gekürzt
    /// wird ab der Fensterecke: Was rechts und unterhalb davon noch Platz hat, bleibt.
    fn fit(g: &Geometry, pos: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> PhysicalSize<u32> {
        let left = u32::try_from(g.x.saturating_sub(pos.x)).unwrap_or(0);
        let above = u32::try_from(g.y.saturating_sub(pos.y)).unwrap_or(0);
        PhysicalSize::new(
            g.width.min(size.width.saturating_sub(left)),
            g.height.min(size.height.saturating_sub(above)),
        )
    }

    pub fn save<R: Runtime>(window: &WebviewWindow<R>, store: &Store) {
        // Minimiert liefert Windows nur Platzhalter (−32000, 0×0) – nichts überschreiben.
        if window.is_minimized().unwrap_or(false) {
            return;
        }
        let maximized = window.is_maximized().unwrap_or(false);
        let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else {
            return;
        };
        // Maximiert: die zuletzt gespeicherte normale Lage behalten, nur den Zustand merken.
        let g = match load(store) {
            Some(previous) if maximized => Geometry {
                maximized,
                ..previous
            },
            // Maximiert ohne bekannte normale Lage: nur den Zustand merken.
            None if maximized => Geometry {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                maximized,
            },
            _ => Geometry {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
                maximized,
            },
        };
        if let Ok(json) = serde_json::to_string(&g) {
            let _ = store.kv_set(KEY, &json);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{Geometry, fit};
        use tauri::{PhysicalPosition, PhysicalSize};

        fn geometry(x: i32, y: i32, width: u32, height: u32) -> Geometry {
            Geometry {
                x,
                y,
                width,
                height,
                maximized: false,
            }
        }

        /// Lage von einem breiteren Bildschirm: Das Fenster endet am rechten und unteren
        /// Rand – sonst läge ein Teil der Oberfläche außerhalb und wäre nicht erreichbar.
        #[test]
        fn a_window_near_the_edge_is_trimmed_in_both_directions() {
            let fitted = fit(
                &geometry(1000, 100, 1500, 1050),
                PhysicalPosition::new(0, 0),
                PhysicalSize::new(1920, 1080),
            );
            assert_eq!((fitted.width, fitted.height), (920, 980));
        }

        /// Passende Lage auf einem Bildschirm links der Hauptanzeige (negative Koordinaten):
        /// nichts wird gekürzt.
        #[test]
        fn a_window_that_fits_keeps_its_size() {
            let fitted = fit(
                &geometry(-1800, 60, 1200, 800),
                PhysicalPosition::new(-1920, 0),
                PhysicalSize::new(1920, 1080),
            );
            assert_eq!((fitted.width, fitted.height), (1200, 800));
        }
    }
}

/// Schließen und Beenden: Läuft ein Lauf, fragt die Seite nach (Ereignis
/// `close-requested`); ein zweiter Klick auf ✕ binnen 10 s schließt trotzdem, falls die
/// Seite nicht antwortet. Ist das Hauptfenster weg, endet die App – kein Prozess bleibt
/// hinter dem Einzelinstanz-Schloss zurück.
mod lifecycle {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use jobalert_core::store::Store;
    use tauri::{Emitter as _, Manager as _, Runtime, WebviewWindow, WindowEvent};

    use crate::commands::AppState;

    pub fn watch<R: Runtime>(window: &WebviewWindow<R>, store: Arc<Store>) {
        let win = window.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                super::geometry::save(&win, &store);
                let state = win.state::<AppState>();
                if !state.busy() {
                    return;
                }
                let mut asked = state
                    .close_asked
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                // Hart schließen nur, wenn die Seite auf die erste Frage nicht geantwortet hat.
                if asked.is_some_and(|at| at.elapsed() < Duration::from_secs(10)) {
                    state.cancel_run();
                    return;
                }
                api.prevent_close();
                *asked = Some(Instant::now());
                let _ = win.emit("close-requested", ());
            }
            WindowEvent::Destroyed => {
                win.state::<AppState>().cancel_run();
                win.app_handle().exit(0);
            }
            _ => {}
        });
    }
}
