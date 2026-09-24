#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod platform;
mod session;
#[cfg(debug_assertions)]
mod smoke;

use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use commands::{Activity, AppState, GmailUser};
use jobalert_core::secrets::Vault;
use jobalert_core::store::Store;
use tauri::Manager;

// ------------------------------------------------------------------ startup error texts
// User-facing text, German by product decision. The start dialog is the only prose here.
const TEXT_DIALOG_TITLE: &str = "Job-Alert-Monitor";
const TEXT_START_FAILED: &str = "Die App konnte nicht starten:";
const TEXT_SEE_LOG: &str = "Details stehen im Protokoll unter";
const TEXT_LOG_DIR_WINDOWS: &str = "%LOCALAPPDATA%\\de.cxecutives.job-alert-monitor\\logs";
const TEXT_LOG_DIR_OTHER: &str = "im Datenordner der App (Unterordner „logs“)";
const TEXT_WINDOW_FAILED: &str = "Das Fenster ließ sich nicht öffnen:";
const TEXT_WINDOW_CONFIG_MISSING: &str = "Fensterkonfiguration 'main' fehlt";
const TEXT_WEBVIEW2_HINT: &str = "Fehlt die Microsoft-Edge-WebView2-Laufzeit, hilft deren \
    Installation (https://developer.microsoft.com/microsoft-edge/webview2/).";
const TEXT_FILE: &str = "Datei:";
const TEXT_NEWER_SCHEMA: &str = "Bitte die neuere Version des Job-Alert-Monitors verwenden. \
    Die Daten bleiben unverändert.";
const TEXT_DATABASE_FAILED: &str = "Die Datenbank ließ sich nicht öffnen:";
const TEXT_DATABASE_ADVICE: &str = "Benutzt ein anderes Programm die Datei gerade, dieses \
    schließen und die App neu starten. Ist sie beschädigt, die Datei umbenennen \
    (beiseitelegen) – die App legt beim nächsten Start eine neue an. Folge: Die bisherigen \
    Jobs und Einstellungen sind dann nicht mehr bekannt, und der nächste Abruf legt ihre \
    Textdateien neu an.";
// ------------------------------------------------------------------ end of user-facing text

/// Exit code from `AppHandle::exit(code)`. Otherwise Tauri always ends the process with 0 on
/// Windows (the event loop only knows `ExitWithCode(0)`).
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

fn main() {
    // Dry run: database and vault state in memory only, fakes instead of mailbox and
    // portals, no files.
    let dry_run = std::env::args().any(|arg| arg == "--dry-run");
    let builder = tauri::Builder::default()
        // Must be the first plugin: a second start only brings the existing window to the
        // front.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window(platform::MAIN) {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(commands::invoke_handler())
        // A startup error (database, WebView2 ...) shows up as a dialog with cause and advice;
        // a GUI program without a console would otherwise end without a word.
        .setup(move |app| {
            if let Err(message) = setup(app, dry_run) {
                fail(&message);
            }
            Ok(())
        });
    let app = platform::app(builder)
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

/// Shows a startup error and exits. `message` names cause and advice.
fn fail(message: &str) -> ! {
    log::error!("startup failed: {message}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(TEXT_DIALOG_TITLE)
        .set_description(format!(
            "{TEXT_START_FAILED}\n\n{message}\n\n{TEXT_SEE_LOG} {}.",
            log_hint()
        ))
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    std::process::exit(1);
}

/// Where the log lives - a hint in dialogs shown before there is an app handle.
fn log_hint() -> &'static str {
    if cfg!(windows) {
        TEXT_LOG_DIR_WINDOWS
    } else {
        TEXT_LOG_DIR_OTHER
    }
}

/// The window or the UI could not be created. On Windows the WebView2 runtime is usually
/// missing; the other systems bring their engine with them.
fn window_failure(error: &dyn std::fmt::Display) -> String {
    if cfg!(windows) {
        format!("{TEXT_WINDOW_FAILED} {error}\n\n{TEXT_WEBVIEW2_HINT}")
    } else {
        format!("{TEXT_WINDOW_FAILED} {error}")
    }
}

/// The database could not be opened: file, cause and what to do. A database of a newer
/// version is not damaged - it must not be put aside.
fn database_failure(path: &Path, error: &jobalert_core::Error) -> String {
    if matches!(error, jobalert_core::Error::NewerSchema(_)) {
        return format!(
            "{error}\n\n{TEXT_FILE} {}\n\n{TEXT_NEWER_SCHEMA}",
            path.display()
        );
    }
    format!(
        "{TEXT_DATABASE_FAILED}\n{}\n\n{error}\n\n{TEXT_DATABASE_ADVICE}",
        path.display()
    )
}

/// Startup: log -> panic hook -> crypto -> pending reset -> database -> window.
/// The error is the finished message for the user.
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
        eprintln!("log file unavailable: {error}");
    }
    // Without a console (GUI program) a panic would otherwise vanish without a trace.
    std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));
    jobalert_core::install_crypto();
    // A requested reset runs before anything else - nothing holds a file open yet. The dry
    // run never deletes anything.
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
    app.manage(AppState {
        store: store.clone(),
        default_workspace: app
            .path()
            .document_dir()
            .map_err(|e| e.to_string())?
            .join("Job-Alert-Monitor"),
        data_dir,
        dry_run,
        user_agent: platform::USER_AGENT.to_owned(),
        reset_report: Mutex::new(reset_report),
        gmail_user: Mutex::new(GmailUser::Unread),
        activity: Mutex::new(Activity::Idle),
    });
    // The web view version goes to the log only: the UI does not need it, and for debugging
    // the log is more reliable than a screenshot.
    let webview_version = tauri::webview_version().unwrap_or_default();
    log::info!(
        "start {}{} (web view {})",
        env!("CARGO_PKG_VERSION"),
        if dry_run { " (dry run)" } else { "" },
        if webview_version.is_empty() {
            "unknown"
        } else {
            &webview_version
        }
    );

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == platform::MAIN)
        .cloned()
        .ok_or(TEXT_WINDOW_CONFIG_MISSING)?;
    let builder = tauri::WebviewWindowBuilder::from_config(app.handle(), &config)
        .map_err(|e| window_failure(&e))?;
    let builder = platform::harden(builder, app.config());
    #[cfg(debug_assertions)]
    let builder = smoke::attach(builder);
    let window = builder.build().map_err(|e| window_failure(&e))?;
    let maximized = geometry::restore(&window, &store);
    platform::apply(&window).map_err(|e| window_failure(&e))?;
    platform::reveal_after_first_load(&window, maximized);
    lifecycle::watch(&window, store);
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--devtools") {
        window.open_devtools();
    }
    Ok(())
}

/// Window placement (size, position, maximized) across restarts - only if it lies on an
/// existing screen (otherwise the window stays centered).
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

    /// Moves and sizes the (still hidden) window. Returns whether it should be maximized:
    /// maximizing shows a window at once, so that waits for the first paint
    /// (`platform::reveal_after_first_load`).
    pub fn restore<R: Runtime>(window: &WebviewWindow<R>, store: &Store) -> bool {
        let Some(g) = load(store) else { return false };
        // Only "maximized" is known (first closed while maximized): placement stays default.
        if g.width == 0 {
            return g.maximized;
        }
        // The title bar must be reachable on an existing screen.
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
            return false;
        };
        let fitted = fit(&g, *monitor.position(), *monitor.size());
        // Move first, then size: moving to a screen with another scale factor would
        // otherwise convert the size.
        let _ = window.set_position(PhysicalPosition::new(g.x, g.y));
        let _ = window.set_size(fitted);
        g.maximized
    }

    /// Never beyond this screen - the size was perhaps saved on a larger one that is gone
    /// now, and parts of the UI would lie outside. Trimmed from the window corner: what
    /// still fits right of and below it stays.
    fn fit(g: &Geometry, pos: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> PhysicalSize<u32> {
        let left = u32::try_from(g.x.saturating_sub(pos.x)).unwrap_or(0);
        let above = u32::try_from(g.y.saturating_sub(pos.y)).unwrap_or(0);
        PhysicalSize::new(
            g.width.min(size.width.saturating_sub(left)),
            g.height.min(size.height.saturating_sub(above)),
        )
    }

    pub fn save<R: Runtime>(window: &WebviewWindow<R>, store: &Store) {
        // Minimized, Windows reports only placeholders (-32000, 0x0) - overwrite nothing.
        if window.is_minimized().unwrap_or(false) {
            return;
        }
        let maximized = window.is_maximized().unwrap_or(false);
        let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else {
            return;
        };
        // Maximized: keep the last saved normal placement, remember only the state.
        let g = match load(store) {
            Some(previous) if maximized => Geometry {
                maximized,
                ..previous
            },
            // Maximized without a known normal placement: remember only the state.
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

        /// Placement from a wider screen: the window ends at the right and bottom edge -
        /// otherwise part of the UI would lie outside and be unreachable.
        #[test]
        fn a_window_near_the_edge_is_trimmed_in_both_directions() {
            let fitted = fit(
                &geometry(1000, 100, 1500, 1050),
                PhysicalPosition::new(0, 0),
                PhysicalSize::new(1920, 1080),
            );
            assert_eq!((fitted.width, fitted.height), (920, 980));
        }

        /// A fitting placement on a screen left of the main display (negative coordinates):
        /// nothing is trimmed.
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

/// Closing and quitting: the close button never asks. If something is running, the window
/// stays briefly (the page shows a blocker on the `closing` event), the run is cancelled and
/// gets at most ten seconds to finish writing its files - then the app ends in any case.
/// Once the main window is gone the app ends too: no process stays behind the single-instance
/// lock.
mod lifecycle {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use jobalert_core::store::Store;
    use tauri::{Emitter as _, Manager as _, Runtime, WebviewWindow, WindowEvent};

    use crate::commands::AppState;

    /// How long a cancelled run may still clean up.
    const GRACE: Duration = Duration::from_secs(10);
    const STEP: Duration = Duration::from_millis(100);

    pub fn watch<R: Runtime>(window: &WebviewWindow<R>, store: Arc<Store>) {
        let win = window.clone();
        // Further clicks on the close button change nothing: the sequence runs exactly once.
        let closing = Arc::new(AtomicBool::new(false));
        window.on_window_event(move |event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                super::geometry::save(&win, &store);
                let state = win.state::<AppState>();
                if !state.busy() {
                    return;
                }
                api.prevent_close();
                if closing.swap(true, Ordering::SeqCst) {
                    return;
                }
                let _ = win.emit("closing", ());
                state.cancel_run();
                let app = win.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    for _ in 0..(GRACE.as_millis() / STEP.as_millis()) {
                        if !app.state::<AppState>().busy() {
                            break;
                        }
                        tokio::time::sleep(STEP).await;
                    }
                    app.exit(0);
                });
            }
            WindowEvent::Destroyed => {
                win.state::<AppState>().cancel_run();
                win.app_handle().exit(0);
            }
            _ => {}
        });
    }
}
