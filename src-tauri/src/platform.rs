//! The only place in `src-tauri` with per-OS code (docs/PLAN.md, "Platforms").
//!
//! The app is a tool, not a browser: no browser context menu, no browser shortcuts
//! (reload, print, find, zoom, devtools), no pinch or swipe gestures, no autofill, no
//! navigation away from the app, no pop-up windows. Editing inside fields stays
//! (Ctrl/Cmd+C/V/X/A/Z). The same rules apply in every build, so exactly what ships is
//! what gets tested.
//!
//! Documented differences: WebView2 switches (Windows) vs. a minimal app menu, link preview
//! and first-mouse clicks (macOS), and the user agent of the HTTP client.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::webview::{NewWindowResponse, PageLoadEvent, PageLoadPayload};
use tauri::{Manager, Runtime, Url, Webview, WebviewWindow, WebviewWindowBuilder};

/// Label of the app's own window (`tauri.conf.json`).
pub const MAIN: &str = "main";

// ------------------------------------------------------------------ user agent

/// Current stable Edge on Windows (checked 2026-09-24: Edge stable 153.0.4234.48, the same
/// major version as the WebView2 runtime). Like Edge itself it names only the major version
/// (User-Agent Reduction).
const WINDOWS_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36 Edg/153.0.0.0";

/// Current Safari on macOS (checked 2026-09-24: Safari 27.0, released 2026-09-14). Safari
/// freezes the OS and `WebKit` parts of its user agent; only `Version/` moves.
const MACOS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0 Safari/605.1.15";

/// Oldest versions the constants above may name. When updating a user agent, raise its
/// minimum with it: a user agent that no current browser sends is what bot detection
/// reacts to.
const MIN_EDGE_MAJOR: u32 = 153;
const MIN_SAFARI_MAJOR: u32 = 27;

/// User agent of the HTTP client (`jobalert_core::fetch::http::HttpFetcher`): a real,
/// current browser of the OS the app runs on. Linux is no target; it gets the Windows value.
pub const USER_AGENT: &str = if cfg!(target_os = "macos") {
    MACOS_USER_AGENT
} else {
    WINDOWS_USER_AGENT
};

// Checked at compile time on every OS (the app binary has no test harness, see Cargo.toml):
// each constant names its OS, and neither is older than its minimum.
const _: () = {
    assert!(contains(
        WINDOWS_USER_AGENT,
        "(Windows NT 10.0; Win64; x64)"
    ));
    assert!(major_after(WINDOWS_USER_AGENT, "Chrome/") >= MIN_EDGE_MAJOR);
    assert!(major_after(WINDOWS_USER_AGENT, "Edg/") == major_after(WINDOWS_USER_AGENT, "Chrome/"));
    assert!(contains(
        MACOS_USER_AGENT,
        "(Macintosh; Intel Mac OS X 10_15_7)"
    ));
    assert!(major_after(MACOS_USER_AGENT, "Version/") >= MIN_SAFARI_MAJOR);
    assert!(contains(MACOS_USER_AGENT, "Safari/605.1.15"));
    #[cfg(windows)]
    assert!(contains(USER_AGENT, "Windows NT"));
    #[cfg(target_os = "macos")]
    assert!(contains(USER_AGENT, "Macintosh"));
};

/// Byte offset of `needle` in `haystack` (`usize::MAX` if absent). `const` so that the user
/// agent checks above run in every build.
const fn find(haystack: &str, needle: &str) -> usize {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    let mut start = 0;
    while start + n.len() <= h.len() {
        let mut i = 0;
        while i < n.len() && h[start + i] == n[i] {
            i += 1;
        }
        if i == n.len() {
            return start;
        }
        start += 1;
    }
    usize::MAX
}

const fn contains(haystack: &str, needle: &str) -> bool {
    find(haystack, needle) != usize::MAX
}

/// The number right after `marker` (`0` if the marker is missing).
const fn major_after(haystack: &str, marker: &str) -> u32 {
    let at = find(haystack, marker);
    if at == usize::MAX {
        return 0;
    }
    let bytes = haystack.as_bytes();
    let mut i = at + marker.len();
    let mut major = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        major = major * 10 + (bytes[i] - b'0') as u32;
        i += 1;
    }
    major
}

// ------------------------------------------------------------------ app

/// App-wide options: the first page load reveals the main window; on macOS a minimal menu
/// replaces Tauri's default one.
pub fn app<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    let builder = builder.on_page_load(on_page_load);
    #[cfg(target_os = "macos")]
    let builder = builder.enable_macos_default_menu(false).menu(macos::menu);
    builder
}

// ------------------------------------------------------------------ main window

/// Options of the main window before it is built.
///
/// File drops: Tauri's drag-drop handler stays on (`dragDropEnabled` in `tauri.conf.json`).
/// It takes every drop away from the web view, which would otherwise open the file in place
/// of the app, and reports it as an event nobody listens to - the drop is swallowed.
pub fn harden<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    config: &tauri::Config,
) -> WebviewWindowBuilder<'a, R, M> {
    let origins = AppOrigins::new(config);
    let builder = builder
        // Release builds never offer the inspector; debug builds keep it for `--devtools`.
        .devtools(cfg!(debug_assertions))
        .zoom_hotkeys_enabled(false)
        .general_autofill_enabled(false)
        .on_navigation(move |url| {
            let own = origins.allows(url);
            if !own {
                // Scheme and host only: a full URL could carry personal data into the log.
                log::warn!(
                    "navigation blocked: {}://{}",
                    url.scheme(),
                    url.host_str().unwrap_or_default()
                );
            }
            own
        })
        .on_new_window(|url, _features| {
            log::warn!(
                "new window blocked: {}://{}",
                url.scheme(),
                url.host_str().unwrap_or_default()
            );
            NewWindowResponse::Deny
        });
    // A click into the inactive window acts at once (as on Windows); a force click on a
    // link shows no preview.
    #[cfg(target_os = "macos")]
    let builder = builder.accept_first_mouse(true).allow_link_preview(false);
    builder
}

/// Settings that exist only on the built web view.
///
/// macOS (`WKWebView`) has no counterpart and needs none: it binds neither reload, print,
/// find nor the inspector to keys, the minimal app menu adds no such items, and the UI
/// blocks the context menu in JavaScript on both OS.
#[cfg(windows)]
pub fn apply<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    window.with_webview(|webview| {
        if let Err(error) = webview2::disable_browser_features(&webview.controller()) {
            log::warn!("WebView2 settings not applied: {error}");
        }
    })
}

#[cfg(not(windows))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "same signature as the Windows variant"
)]
pub fn apply<R: Runtime>(_window: &WebviewWindow<R>) -> tauri::Result<()> {
    Ok(())
}

#[cfg(windows)]
mod webview2 {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, ICoreWebView2Settings3, ICoreWebView2Settings4,
        ICoreWebView2Settings5, ICoreWebView2Settings6,
    };
    use windows_core::Interface as _;

    /// The app's only `unsafe` block: Tauri does not pass these WebView2 switches through,
    /// so they are set on WebView2 directly.
    #[expect(
        unsafe_code,
        reason = "WebView2 switches that Tauri does not pass through"
    )]
    pub fn disable_browser_features(
        controller: &ICoreWebView2Controller,
    ) -> windows_core::Result<()> {
        // SAFETY: `with_webview` calls us on the UI thread with this window's valid
        // controller; these are plain COM setters without pointers from Rust.
        unsafe {
            let settings = controller.CoreWebView2()?.Settings()?;
            // Back, reload, print, inspect ...
            settings.SetAreDefaultContextMenusEnabled(false)?;
            // The URL bubble in the bottom corner while hovering a link.
            settings.SetIsStatusBarEnabled(false)?;
            // F5/Ctrl+R, Ctrl+P, Ctrl+F, F12, Ctrl+Shift+I ...
            settings
                .cast::<ICoreWebView2Settings3>()?
                .SetAreBrowserAcceleratorKeysEnabled(false)?;
            let settings4 = settings.cast::<ICoreWebView2Settings4>()?;
            settings4.SetIsPasswordAutosaveEnabled(false)?;
            settings4.SetIsGeneralAutofillEnabled(false)?;
            settings
                .cast::<ICoreWebView2Settings5>()?
                .SetIsPinchZoomEnabled(false)?;
            settings
                .cast::<ICoreWebView2Settings6>()?
                .SetIsSwipeNavigationEnabled(false)
        }
    }
}

// ------------------------------------------------------------------ first paint

/// If the first page load never reports back, the window still appears after this long -
/// a hidden window would otherwise hold the single-instance lock with nothing to close.
const REVEAL_FALLBACK: Duration = Duration::from_secs(8);

static REVEALED: AtomicBool = AtomicBool::new(false);
static MAXIMIZE_ON_REVEAL: AtomicBool = AtomicBool::new(false);

/// The main window is created hidden and appears once its first page has loaded: no white
/// flash on either OS (WebView2 also paints `backgroundColor`, `WKWebView` does not).
/// `maximized` is applied only then, because maximizing shows a window at once.
pub fn reveal_after_first_load<R: Runtime>(window: &WebviewWindow<R>, maximized: bool) {
    MAXIMIZE_ON_REVEAL.store(maximized, Ordering::SeqCst);
    let app = window.app_handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(REVEAL_FALLBACK);
        if !REVEALED.load(Ordering::SeqCst) {
            log::warn!(
                "first page load did not finish within {REVEAL_FALLBACK:?}; showing the window"
            );
            reveal(&app);
        }
    });
}

fn on_page_load<R: Runtime>(webview: &Webview<R>, payload: &PageLoadPayload<'_>) {
    if payload.event() == PageLoadEvent::Finished && webview.label() == MAIN {
        reveal(webview);
    }
}

/// Shows the main window exactly once.
fn reveal<R: Runtime, M: Manager<R>>(manager: &M) {
    let Some(window) = manager.get_webview_window(MAIN) else {
        return;
    };
    if REVEALED.swap(true, Ordering::SeqCst) {
        return;
    }
    if MAXIMIZE_ON_REVEAL.load(Ordering::SeqCst) {
        let _ = window.maximize();
    }
    let _ = window.show();
    let _ = window.set_focus();
}

// ------------------------------------------------------------------ navigation guard

/// Where the main window may navigate: only the app's own origin - the embedded assets
/// (`tauri://localhost`, on Windows served as `http://tauri.localhost`) or, in a dev build
/// with `build.devUrl`, the dev server.
struct AppOrigins(Vec<Url>);

impl AppOrigins {
    fn new(config: &tauri::Config) -> AppOrigins {
        let mut origins: Vec<Url> = [
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
        ]
        .iter()
        .filter_map(|url| Url::parse(url).ok())
        .collect();
        if tauri::is_dev()
            && let Some(dev) = &config.build.dev_url
        {
            origins.push(dev.clone());
        }
        AppOrigins(origins)
    }

    fn allows(&self, url: &Url) -> bool {
        // Compared by hand: `Url::origin` is opaque (never equal) for the `tauri` scheme.
        self.0.iter().any(|own| {
            own.scheme() == url.scheme()
                && own.host_str() == url.host_str()
                && own.port_or_known_default() == url.port_or_known_default()
        })
    }
}

// ------------------------------------------------------------------ macOS menu

#[cfg(target_os = "macos")]
mod macos {
    use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID};
    use tauri::{AppHandle, Runtime};

    // User-facing text, German by product decision.
    const ABOUT: &str = "Über Job-Alert-Monitor";
    const HIDE: &str = "Job-Alert-Monitor ausblenden";
    const HIDE_OTHERS: &str = "Andere ausblenden";
    const QUIT: &str = "Job-Alert-Monitor beenden";
    const EDIT: &str = "Bearbeiten";
    const UNDO: &str = "Widerrufen";
    const REDO: &str = "Wiederholen";
    const CUT: &str = "Ausschneiden";
    const COPY: &str = "Kopieren";
    const PASTE: &str = "Einfügen";
    const SELECT_ALL: &str = "Alles auswählen";
    const WINDOW: &str = "Fenster";
    const MINIMIZE: &str = "Minimieren";
    const CLOSE_WINDOW: &str = "Fenster schließen";

    /// Minimal app menu instead of Tauri's default (no View menu with reload or zoom, no
    /// Help, no Services). It carries the system shortcuts the app keeps: Cmd+Q, Cmd+H,
    /// Cmd+M, Cmd+W, and Cmd+C/V/X/A/Z, which `WKWebView` only receives through an Edit menu.
    pub fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
        let info = app.package_info();
        let about = AboutMetadata {
            name: Some(info.name.clone()),
            version: Some(info.version.to_string()),
            ..AboutMetadata::default()
        };
        let app_menu = Submenu::with_items(
            app,
            &info.name,
            true,
            &[
                &PredefinedMenuItem::about(app, Some(ABOUT), Some(about))?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, Some(HIDE))?,
                &PredefinedMenuItem::hide_others(app, Some(HIDE_OTHERS))?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::quit(app, Some(QUIT))?,
            ],
        )?;
        let edit = Submenu::with_items(
            app,
            EDIT,
            true,
            &[
                &PredefinedMenuItem::undo(app, Some(UNDO))?,
                &PredefinedMenuItem::redo(app, Some(REDO))?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::cut(app, Some(CUT))?,
                &PredefinedMenuItem::copy(app, Some(COPY))?,
                &PredefinedMenuItem::paste(app, Some(PASTE))?,
                &PredefinedMenuItem::select_all(app, Some(SELECT_ALL))?,
            ],
        )?;
        // The window-list id makes macOS treat it as the standard Window menu.
        let window = Submenu::with_id_and_items(
            app,
            WINDOW_SUBMENU_ID,
            WINDOW,
            true,
            &[
                &PredefinedMenuItem::minimize(app, Some(MINIMIZE))?,
                &PredefinedMenuItem::close_window(app, Some(CLOSE_WINDOW))?,
            ],
        )?;
        Menu::with_items(app, &[&app_menu, &edit, &window])
    }
}
