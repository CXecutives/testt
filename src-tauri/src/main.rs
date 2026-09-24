#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod platform;
mod session;
#[cfg(debug_assertions)]
mod smoke;

use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use commands::{Activity, AppState, GmailUser, Scoring};
use jobalert_core::error::ErrorKind;
use jobalert_core::secrets::Vault;
use jobalert_core::store::Store;
use tauri::Manager;

// ------------------------------------------------------------------ startup error texts
// User-facing text, German by product decision. The start dialog is the only prose here.
const TEXT_DIALOG_TITLE: &str = "Job-Alert-Monitor";
const TEXT_START_FAILED: &str = "Die App konnte nicht starten.";
/// Followed by the log folder and a period.
const TEXT_SEE_LOG: &str = "Details stehen im Protokoll unter";
const TEXT_WINDOW_FAILED: &str = "Das Fenster ließ sich nicht öffnen.";
const TEXT_DATABASE_FAILED: &str = "Die Datenbank ließ sich nicht öffnen.";
const TEXT_DATABASE_LOCKED: &str = "Die Datenbank ist in einem anderen Programm geöffnet.";
const TEXT_NEWER_SCHEMA: &str = "Die Daten stammen von einer neueren Version der App.";
/// Followed by the path of the database and a period.
const TEXT_DATABASE_AT: &str = "Die Datei liegt unter";
const TEXT_CLOSE_OTHER: &str = "Bitte dieses Programm schließen und die App neu starten.";
const TEXT_USE_NEWER: &str = "Bitte die neuere Version verwenden, die Daten bleiben unverändert.";
const TEXT_DATABASE_IN_USE: &str =
    "Ist sie in einem anderen Programm geöffnet, dieses schließen und die App neu starten.";
const TEXT_DATABASE_DAMAGED: &str = "Ist sie beschädigt, die Datei umbenennen. Die App legt \
    dann eine neue an, ohne die bisherigen Jobs und Einstellungen.";
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
            if let Err(failure) = setup(app, dry_run) {
                fail(&failure);
            }
            Ok(())
        });
    let app = platform::app(builder)
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| fail(&Failure::window(&error)));
    app.run_return(|app, event| match event {
        tauri::RunEvent::ExitRequested {
            code: Some(code), ..
        } => EXIT_CODE.store(code, Ordering::SeqCst),
        tauri::RunEvent::Exit => lifecycle::exiting(app),
        _ => {}
    });
    std::process::exit(EXIT_CODE.load(Ordering::SeqCst));
}

/// A startup failure: what the user reads (German, may be empty: then only that the app
/// could not start) and the cause for the log (English - the dialog never shows it).
struct Failure {
    message: String,
    cause: String,
}

impl Failure {
    fn other(cause: impl std::fmt::Display) -> Failure {
        Failure {
            message: String::new(),
            cause: cause.to_string(),
        }
    }

    /// The window or the UI could not be created; the platform may know what helps.
    fn window(cause: impl std::fmt::Display) -> Failure {
        let message = match platform::WINDOW_HINT {
            Some(hint) => format!("{TEXT_WINDOW_FAILED}\n\n{hint}"),
            None => TEXT_WINDOW_FAILED.to_owned(),
        };
        Failure {
            message,
            cause: format!("window: {cause}"),
        }
    }

    /// The database could not be opened: what happened, where the file is and what to do.
    /// A database of a newer version is not damaged - it must not be put aside.
    fn database(path: &Path, error: &jobalert_core::Error) -> Failure {
        let at = format!("{TEXT_DATABASE_AT} {}.", path.display());
        let message = match error.kind() {
            ErrorKind::NewerSchema => format!("{TEXT_NEWER_SCHEMA} {at}\n\n{TEXT_USE_NEWER}"),
            ErrorKind::FileLocked => format!("{TEXT_DATABASE_LOCKED} {at}\n\n{TEXT_CLOSE_OTHER}"),
            _ => format!(
                "{TEXT_DATABASE_FAILED} {at}\n\n{TEXT_DATABASE_IN_USE} {TEXT_DATABASE_DAMAGED}"
            ),
        };
        Failure {
            message,
            cause: format!("database: {error}"),
        }
    }
}

/// Shows a startup error and exits.
fn fail(failure: &Failure) -> ! {
    log::error!("startup failed: {}", failure.cause);
    let mut text = TEXT_START_FAILED.to_owned();
    if !failure.message.is_empty() {
        text = format!("{text}\n\n{}", failure.message);
    }
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(TEXT_DIALOG_TITLE)
        .set_description(format!(
            "{text}\n\n{TEXT_SEE_LOG} {}.",
            platform::LOG_DIR_HINT
        ))
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    std::process::exit(1);
}

/// Startup: log -> panic hook -> crypto -> pending reset -> database -> window.
fn setup(app: &mut tauri::App, dry_run: bool) -> Result<(), Failure> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| Failure::other(format!("data folder: {e}")))?;
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
    // Without a console (GUI program) a panic would otherwise vanish without a trace. Only
    // place and kind: the message of a slice panic quotes ad or mail text.
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or_default();
        log::error!(
            "{}",
            jobalert_core::logging::panic_line(info.location(), message)
        );
    }));
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
    let store = Arc::new(store.map_err(|e| Failure::database(&database, &e))?);
    // The sessions' storage outside the data folder (macOS data stores) goes too.
    if reset_report.is_some() {
        session::forget_all(app.handle().clone(), data_dir.clone());
    }
    app.manage(AppState {
        store: store.clone(),
        default_workspace: app
            .path()
            .document_dir()
            .map_err(|e| Failure::other(format!("documents folder: {e}")))?
            .join("Job-Alert-Monitor"),
        data_dir,
        dry_run,
        user_agent: platform::USER_AGENT.to_owned(),
        system_language: platform::system_language(),
        reset_report: Mutex::new(reset_report),
        gmail_user: Mutex::new(GmailUser::Unread),
        activity: Mutex::new(Activity::Idle),
        scoring: Scoring::default(),
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
        .ok_or_else(|| Failure::window("no configuration for the main window"))?;
    let builder =
        tauri::WebviewWindowBuilder::from_config(app.handle(), &config).map_err(Failure::window)?;
    let builder = platform::harden(builder, app.config());
    #[cfg(debug_assertions)]
    let builder = smoke::attach(builder);
    let window = builder.build().map_err(Failure::window)?;
    let maximized = geometry::restore(&window, &store);
    platform::apply(&window).map_err(Failure::window)?;
    platform::reveal_after_first_load(&window, maximized);
    lifecycle::watch(&window, store);
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--devtools") {
        window.open_devtools();
    }
    Ok(())
}

/// Window placement (size, position, maximized) across restarts - only if it lies on an
/// existing screen (otherwise the window stays centered). The rules live in
/// `jobalert_core::window`; this only reads and moves the window.
mod geometry {
    use jobalert_core::store::Store;
    use jobalert_core::window::{self, Placement, Screen};
    use tauri::{PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

    /// Moves and sizes the (still hidden) window. Returns whether it should be maximized:
    /// maximizing shows a window at once, so that waits for the first paint
    /// (`platform::reveal_after_first_load`).
    pub fn restore<R: Runtime>(window: &WebviewWindow<R>, store: &Store) -> bool {
        let screens: Vec<Screen> = window
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| Screen {
                x: m.position().x,
                y: m.position().y,
                width: m.size().width,
                height: m.size().height,
            })
            .collect();
        let restore = window::restore(Placement::load(store), &screens);
        if let Some(bounds) = restore.bounds {
            // Move first, then size: moving to a screen with another scale factor would
            // otherwise convert the size.
            let _ = window.set_position(PhysicalPosition::new(bounds.x, bounds.y));
            let _ = window.set_size(PhysicalSize::new(bounds.width, bounds.height));
        }
        restore.maximized
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
        let placement = Placement::on_close(
            Placement::load(store),
            maximized,
            (pos.x, pos.y),
            (size.width, size.height),
        );
        if let Err(e) = placement.save(store) {
            log::warn!("window placement not saved: {e}");
        }
    }
}

/// Closing and quitting: the close button never asks. If something is running, the window
/// stays briefly (the page shows a blocker on the `closing` event), the run is cancelled and
/// gets at most ten seconds to finish writing its files - then the app ends in any case.
/// Once the main window is gone the app ends too: no process stays behind the single-instance
/// lock. An end without any window event (macOS: quit from the Dock, logout) still saves the
/// placement and gives a running fetch the same grace (`exiting`).
mod lifecycle {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use jobalert_core::store::Store;
    use tauri::{AppHandle, Emitter as _, Manager as _, Runtime, WebviewWindow, WindowEvent};

    use crate::commands::AppState;

    /// How long a cancelled run may still clean up.
    const GRACE: Duration = Duration::from_secs(10);
    const STEP: Duration = Duration::from_millis(100);

    /// The closing sequence runs exactly once: further clicks on the close button change
    /// nothing, and the end of the process does not wait a second time.
    static CLOSING: AtomicBool = AtomicBool::new(false);

    pub fn watch<R: Runtime>(window: &WebviewWindow<R>, store: Arc<Store>) {
        let win = window.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                super::geometry::save(&win, &store);
                let state = win.state::<AppState>();
                if !state.busy() {
                    return;
                }
                api.prevent_close();
                if CLOSING.swap(true, Ordering::SeqCst) {
                    return;
                }
                let _ = win.emit("closing", ());
                state.scoring.stop();
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

    /// The process ends (`RunEvent::Exit`). Without the closing sequence before it - macOS
    /// quits from the Dock or at logout through `terminate:` without any window event - this
    /// is the last chance: save the placement, start no own run any more, cancel a running
    /// one and wait for it the same grace. The runtime threads keep running meanwhile.
    pub fn exiting<R: Runtime>(app: &AppHandle<R>) {
        if CLOSING.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(state) = app.try_state::<AppState>() else {
            return;
        };
        if let Some(window) = app.get_webview_window(crate::platform::MAIN) {
            super::geometry::save(&window, &state.store);
        }
        state.scoring.stop();
        state.cancel_run();
        for _ in 0..(GRACE.as_millis() / STEP.as_millis()) {
            if !state.busy() {
                break;
            }
            std::thread::sleep(STEP);
        }
    }
}
