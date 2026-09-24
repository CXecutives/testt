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

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::webview::{NewWindowResponse, PageLoadEvent, PageLoadPayload};
use tauri::{AppHandle, Manager, Runtime, Url, Webview, WebviewWindow, WebviewWindowBuilder};

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

// ------------------------------------------------------------------ portal sessions

/// How long deleting a session's storage may retry: after its window closes, the engine
/// still holds the files (WebView2) or the data store (`WKWebView`) for a moment.
const STORAGE_RELEASE: Duration = Duration::from_secs(6);
const STORAGE_STEP: Duration = Duration::from_millis(250);

/// Where a portal's session window keeps cookies and cache: its own profile folder
/// (WebView2 user data folder) on Windows, its own persistent data store on macOS, where
/// `WKWebView` has no folder option (`data_store_identifier` needs macOS 14, the minimum).
pub fn session_storage<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    portal_key: &str,
    profile: &Path,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "macos")]
    {
        let _ = profile;
        builder.data_store_identifier(session_store_id(portal_key))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = portal_key;
        builder.data_directory(profile.to_path_buf())
    }
}

/// Deletes a portal's session storage once its window is closed. `true` only if it is
/// verifiably gone: the profile folder (all OS; older versions created it on macOS too)
/// and, on macOS, the data store.
pub async fn delete_session_storage<R: Runtime>(
    app: &AppHandle<R>,
    portal_key: &str,
    profile: &Path,
) -> bool {
    let mut folder_gone = false;
    let mut store_gone = !cfg!(target_os = "macos");
    for _ in 0..(STORAGE_RELEASE.as_millis() / STORAGE_STEP.as_millis()) {
        folder_gone = folder_gone
            || match std::fs::remove_dir_all(profile) {
                Ok(()) => true,
                Err(error) => error.kind() == std::io::ErrorKind::NotFound,
            };
        store_gone = store_gone || remove_data_store(app, portal_key).await;
        if folder_gone && store_gone {
            return true;
        }
        tokio::time::sleep(STORAGE_STEP).await;
    }
    log::warn!(
        "session storage of {portal_key} not deleted (folder gone: {folder_gone}, data store gone: {store_gone})"
    );
    false
}

/// macOS: removes the portal's data store and checks it is no longer listed.
#[cfg(target_os = "macos")]
async fn remove_data_store<R: Runtime>(app: &AppHandle<R>, portal_key: &str) -> bool {
    let id = session_store_id(portal_key);
    if let Err(error) = app.remove_data_store(id).await {
        log::debug!("data store of {portal_key} not removed yet: {error}");
    }
    app.fetch_data_store_identifiers()
        .await
        .is_ok_and(|ids| !ids.contains(&id))
}

#[cfg(not(target_os = "macos"))]
#[allow(clippy::unused_async, reason = "same signature as the macOS variant")]
async fn remove_data_store<R: Runtime>(_app: &AppHandle<R>, _portal_key: &str) -> bool {
    true
}

/// Stable data store identifier of a portal session (macOS): a UUID (version 8, RFC 9562)
/// from the 128-bit FNV-1a hash of the app id and the portal key. It must never change -
/// a new identifier would silently lose the user's sign-in.
#[cfg_attr(
    not(target_os = "macos"),
    allow(
        dead_code,
        reason = "only macOS has data stores; checked at compile time"
    )
)]
const fn session_store_id(portal_key: &str) -> [u8; 16] {
    const PREFIX: &[u8] = b"de.cxecutives.job-alert-monitor/session/";
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013B;
    let mut hash: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    let mut i = 0;
    while i < PREFIX.len() + portal_key.len() {
        let byte = if i < PREFIX.len() {
            PREFIX[i]
        } else {
            portal_key.as_bytes()[i - PREFIX.len()]
        };
        hash = (hash ^ byte as u128).wrapping_mul(PRIME);
        i += 1;
    }
    let mut id = hash.to_be_bytes();
    id[6] = (id[6] & 0x0f) | 0x80;
    id[8] = (id[8] & 0x3f) | 0x80;
    id
}

// Golden values: the identifiers of existing sessions never change.
const _: () = {
    let freelance = session_store_id("freelance");
    assert!(freelance[0] == 0xde && freelance[1] == 0x11 && freelance[15] == 0xad);
    let freelancermap = session_store_id("freelancermap");
    assert!(freelancermap[0] == 0xa6 && freelancermap[15] == 0xa1);
    assert!(freelance[6] >> 4 == 8 && freelance[8] >> 6 == 0b10);
};

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
    // end of user-facing text

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

/// Operating system of the interface (the page words its texts accordingly).
pub fn platform() -> jobalert_core::view::Platform {
    if cfg!(target_os = "macos") {
        jobalert_core::view::Platform::Macos
    } else {
        jobalert_core::view::Platform::Windows
    }
}

/// Where the mailbox password lives on this operating system.
pub fn vault_kind() -> jobalert_core::view::VaultKind {
    if cfg!(target_os = "macos") {
        jobalert_core::view::VaultKind::MacosKeychain
    } else {
        jobalert_core::view::VaultKind::WindowsCredentialManager
    }
}

// ------------------------------------------------------------------ startup dialog

/// The log folder the startup dialog names (it shows before there is an app handle): the
/// app's local data folder of this OS, `logs` in it.
pub const LOG_DIR_HINT: &str = if cfg!(target_os = "macos") {
    "~/Library/Application Support/de.cxecutives.job-alert-monitor/logs"
} else {
    "%LOCALAPPDATA%\\de.cxecutives.job-alert-monitor\\logs"
};

// User-facing text, German by product decision.
/// What helps when the window cannot open: on Windows the WebView2 runtime is usually
/// missing; macOS brings its engine along.
pub const WINDOW_HINT: Option<&str> = if cfg!(windows) {
    Some(
        "Fehlt die Microsoft-Edge-WebView2-Laufzeit, hilft deren Installation \
         (https://developer.microsoft.com/microsoft-edge/webview2/).",
    )
} else {
    None
};
// end of user-facing text
