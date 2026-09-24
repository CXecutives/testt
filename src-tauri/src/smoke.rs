//! Self-check for development and acceptance: `job-alert-monitor --smoke` loads the UI,
//! checks the shell against the UI contract (`data-testid`), clicks through the three tabs
//! and exits with 0 (all fine), 1 (contract broken or a CSP violation) or 2 (timeout).
//!
//! Every view stays on screen for [`HOLD`], longer than two intervals of the CI screenshot
//! loop (1.5 s), so each one is captured; the output names the view shown
//! (`SMOKE view jobs`). CSP violations are collected from the first byte on (an
//! initialization script listens for `securitypolicyviolation`) and reported at the end.
//!
//! `--smoke-run` is still accepted (CI and scripts pass it with `--dry-run`); it runs the
//! same check until the Jobs screen has test ids for a demo run.
//!
//! Debug build only (`#[cfg(debug_assertions)]` where `main.rs` includes it).

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::Value;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

/// Tabs in the order the probe visits them (`tab-<id>` opens `view-<id>`).
const TABS: [&str; 3] = ["jobs", "profile", "settings"];
/// How long each view stays on screen for the CI screenshots.
const HOLD: Duration = Duration::from_secs(3);

/// Runs before any page script: every CSP violation lands in `window.__smokeCsp`.
const CSP_LISTENER: &str = "window.__smokeCsp = []; \
    document.addEventListener('securitypolicyviolation', (e) => \
    window.__smokeCsp.push(e.violatedDirective + ' ' + (e.blockedURI || 'inline')));";

const PROBE: &str = r#"(() => { try {
    const q = (id) => document.querySelector(`[data-testid="${id}"]`);
    const shown = (el) => !!el && el.getBoundingClientRect().width > 0;
    return JSON.stringify({
      ready: shown(q('shell')) && shown(q('titlebar')),
      tabs: document.querySelectorAll('[data-testid^="tab-"]').length,
      named: ['tab-jobs', 'tab-profile', 'tab-settings'].every((id) => !!q(id)),
      tauri: '__TAURI_INTERNALS__' in window,
      csp: window.__smokeCsp ?? null,
    });
  } catch (e) { return JSON.stringify({ error: String(e) }); } })()"#;

const CSP_PROBE: &str = "JSON.stringify({ csp: window.__smokeCsp ?? null })";

/// The tab currently visited (index into [`TABS`]).
static TAB: AtomicUsize = AtomicUsize::new(0);
/// Last answer of the page - the watchdog prints it on timeout.
static LAST: Mutex<String> = Mutex::new(String::new());

fn flag(name: &str) -> bool {
    std::env::args().any(|arg| arg == name)
}

pub fn attach<R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'_, R, M>,
) -> WebviewWindowBuilder<'_, R, M> {
    // `--smoke-run` implies `--smoke`: without the self-check there would be no watchdog -
    // the check would stay open as an ordinary window instead of ending with a code.
    if !flag("--smoke") && !flag("--smoke-run") {
        return builder;
    }
    // The watchdog runs from window creation: if loading already hangs, the process still
    // ends - directly, because the event loop may then be blocked.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(60));
        let last = LAST.lock().map(|l| l.clone()).unwrap_or_default();
        println!("SMOKE timeout {last}");
        std::process::exit(2);
    });
    builder
        .initialization_script(CSP_LISTENER)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                ask(
                    &window,
                    PROBE.to_owned(),
                    |v| v["ready"] == true,
                    check_shell,
                );
            }
        })
}

fn check_shell<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    let ok = value["tabs"] == 3
        && value["named"] == true
        && value["tauri"] == true
        && no_csp_violation(value);
    if ok {
        TAB.store(0, Ordering::SeqCst);
        show_next_tab(window);
    } else {
        window.app_handle().exit(1);
    }
}

/// Clicks the next tab and waits until its view is the only one on screen; after the
/// last tab, the final CSP check.
fn show_next_tab<R: Runtime>(window: &WebviewWindow<R>) {
    let Some(tab) = TABS.get(TAB.load(Ordering::SeqCst)) else {
        ask(window, CSP_PROBE.to_owned(), |_| true, check_csp);
        return;
    };
    let click = format!("document.querySelector('[data-testid=\"tab-{tab}\"]').click()");
    if let Err(error) = window.eval(click) {
        println!("SMOKE eval failed: {error}");
        window.app_handle().exit(1);
        return;
    }
    // Views cross-fade: done once the target is present and no other view remains.
    let probe = format!(
        "JSON.stringify({{ tab: '{tab}', views: [...document.querySelectorAll('[data-testid^=\"view-\"]')].map((v) => v.dataset.testid) }})"
    );
    ask(window, probe, view_settled, view_shown);
}

fn view_settled(value: &Value) -> bool {
    let tab = value["tab"].as_str().unwrap_or_default();
    value["views"]
        .as_array()
        .is_some_and(|views| views.len() == 1 && views[0] == format!("view-{tab}"))
}

fn view_shown<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    println!("SMOKE view {}", value["tab"].as_str().unwrap_or_default());
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(HOLD);
        TAB.fetch_add(1, Ordering::SeqCst);
        show_next_tab(&window);
    });
}

fn check_csp<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    window
        .app_handle()
        .exit(i32::from(!no_csp_violation(value)));
}

/// The listener ran (an array) and caught nothing; otherwise the violations are printed.
fn no_csp_violation(value: &Value) -> bool {
    match value["csp"].as_array() {
        Some(list) if list.is_empty() => true,
        Some(list) => {
            for violation in list {
                println!("SMOKE CSP violation: {violation}");
            }
            false
        }
        None => {
            println!("SMOKE CSP listener missing");
            false
        }
    }
}

/// Asks the page every 200 ms until `done` holds (the watchdog bounds the total time),
/// prints the answer and hands it to `then`.
fn ask<R: Runtime>(
    window: &WebviewWindow<R>,
    js: String,
    done: fn(&Value) -> bool,
    then: fn(&WebviewWindow<R>, &Value),
) {
    let again = window.clone();
    let script = js.clone();
    let result = window.eval_with_callback(js, move |raw| {
        // The result is JSON-serialized - so here a JSON string with JSON inside.
        let json = jobalert_core::fetch::site::eval_result(raw);
        let value = serde_json::from_str::<Value>(&json).unwrap_or_default();
        if let Ok(mut last) = LAST.lock() {
            last.clone_from(&json);
        }
        if !done(&value) {
            let again = again.clone();
            let script = script.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                ask(&again, script, done, then);
            });
            return;
        }
        println!("SMOKE {json}");
        then(&again, &value);
    });
    if let Err(error) = result {
        println!("SMOKE eval failed: {error}");
        window.app_handle().exit(1);
    }
}
