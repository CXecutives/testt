//! Self-check for development and acceptance: `job-alert-monitor --smoke` loads the UI,
//! checks the shell against the UI contract (`data-testid`), clicks through the three sidebar
//! entries and exits with 0 (all fine), 1 (contract broken, a step failed or a CSP violation)
//! or 2 (timeout).
//!
//! Every view stays on screen for [`HOLD`], longer than two intervals of the CI screenshot
//! loop (1.5 s), so each one is captured; the output names the view shown
//! (`SMOKE view jobs`). CSP violations are collected from the first byte on (an
//! initialization script listens for `securitypolicyviolation`) and reported at the end.
//!
//! `--smoke-run` (with `--dry-run`) then drives the real fetch path of the app twice in a row:
//! "Abrufen" -> run events over the channel -> rows with filled rings -> `Finished`, and each
//! run has to end in the idle state. After that it measures frames while switching views,
//! scrolling and opening jobs in the reader (`SMOKE {"step":"views",...}`): frame intervals
//! (p50, p95, max), dropped frames, long tasks and long animation frames. The numbers are
//! printed, not gated - frame times on CI machines vary too much for a threshold.
//!
//! Debug build only (`#[cfg(debug_assertions)]` where `main.rs` includes it).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::Value;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

/// Sidebar entries in the order the probe visits them (`nav-<id>` opens `view-<id>`).
const TABS: [&str; 3] = ["jobs", "profile", "settings"];
/// How long each view stays on screen for the CI screenshots.
const HOLD: Duration = Duration::from_secs(3);
/// The steps of `--smoke-run`, in this order (`window.__smoke` in [`INIT`] runs them).
const SCENARIOS: [&str; 5] = ["run", "run", "views", "scroll", "reader"];
/// The whole check ends after this long (CI kills the process after 150 s).
const WATCHDOG: Duration = Duration::from_secs(60);
const WATCHDOG_RUN: Duration = Duration::from_secs(130);

/// Runs before any page script: every CSP violation lands in `window.__smokeCsp`; the frame
/// probe and the steps of `--smoke-run` wait in `window.__smoke`.
const INIT: &str = r#"(() => {
  const csp = [];
  window.__smokeCsp = csp;
  document.addEventListener('securitypolicyviolation', (e) =>
    csp.push(e.violatedDirective + ' ' + (e.blockedURI || 'inline')));

  // Frame probe: requestAnimationFrame intervals while recording, long tasks and long
  // animation frames (both engines that lack an entry type simply report none).
  const long = [];
  let rec = null;
  const loop = (t) => {
    if (rec) {
      if (rec.last !== null) rec.frames.push(t - rec.last);
      rec.last = t;
    }
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
  for (const type of ['longtask', 'long-animation-frame']) {
    try {
      new PerformanceObserver((list) => {
        for (const e of list.getEntries()) long.push({ type, start: e.startTime, dur: e.duration });
      }).observe({ type, buffered: true });
    } catch (e) {
      // This engine does not know the entry type.
    }
  }
  const start = () => { rec = { t0: performance.now(), last: null, frames: [] }; };
  const stop = () => {
    const r = rec;
    rec = null;
    if (r === null) return null;
    const t1 = performance.now();
    const f = [...r.frames].sort((a, b) => a - b);
    const at = (p) => (f.length ? f[Math.min(f.length - 1, Math.floor(p * f.length))] : 0);
    const vsync = at(0.5) || 1000 / 60;
    const dropped = r.frames.reduce((n, d) => n + Math.max(0, Math.round(d / vsync) - 1), 0);
    const inside = long.filter((e) => e.start >= r.t0 && e.start <= t1);
    const tasks = inside.filter((e) => e.type === 'longtask');
    const frames = inside.filter((e) => e.type === 'long-animation-frame');
    const one = (x) => Math.round(x * 10) / 10;
    return {
      ms: Math.round(t1 - r.t0), frames: f.length, p50: one(at(0.5)), p95: one(at(0.95)),
      max: one(f.length ? f[f.length - 1] : 0), dropped,
      longTasks: tasks.length, longTaskMs: Math.round(tasks.reduce((s, e) => s + e.dur, 0)),
      longFrames: frames.length, longFrameMax: Math.round(Math.max(0, ...frames.map((e) => e.dur))),
    };
  };

  const q = (id) => document.querySelector(`[data-testid="${id}"]`);
  const all = (css) => document.querySelectorAll(css);
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const frame = () => new Promise((resolve) => requestAnimationFrame(() => resolve()));
  const until = async (test, ms) => {
    const end = performance.now() + ms;
    while (performance.now() < end) {
      if (test()) return true;
      await wait(50);
    }
    return test();
  };
  const fetchButton = () => q('fetch') || q('first-fetch');
  const ready = () => fetchButton() !== null && fetchButton().getAttribute('aria-disabled') !== 'true';

  const scenarios = {
    // Abrufen -> run events -> rows with rings -> Finished, back in the idle state.
    async run() {
      if (!(await until(ready, 5000))) return { ok: false, why: 'no enabled fetch button' };
      start();
      fetchButton().click();
      if (!(await until(() => q('run-running') !== null, 5000))) {
        return { ok: false, why: 'the run did not start', perf: stop() };
      }
      const idle = () => q('run-running') === null && q('run-finished') !== null && ready();
      if (!(await until(idle, 30000))) {
        return { ok: false, why: 'the run did not end in the idle state', perf: stop() };
      }
      await wait(600);
      const rows = all('[data-testid^="job-row-"]').length;
      // Scored rings in either language (the class, not the German aria label).
      const rings = all('[data-testid^="job-row-"] .ring.scored').length;
      return { ok: rows > 0 && rings > 0, rows, rings, perf: stop() };
    },
    async views() {
      const out = { ok: true };
      for (const tab of ['profile', 'settings', 'jobs']) {
        start();
        q('nav-' + tab).click();
        await wait(600);
        out[tab] = stop();
        out.ok = out.ok && q('view-' + tab) !== null;
      }
      return out;
    },
    async scroll() {
      const out = { ok: true };
      for (const id of ['list-scroll', 'reader-pane']) {
        const el = q(id);
        if (el === null) continue;
        start();
        for (let i = 0; i < 40; i += 1) { el.scrollBy(0, 40); await frame(); }
        for (let i = 0; i < 40; i += 1) { el.scrollBy(0, -40); await frame(); }
        out[id] = { ...stop(), range: el.scrollHeight - el.clientHeight };
      }
      return out;
    },
    async reader() {
      const rows = [...all('[data-testid^="job-row-"]')].slice(0, 5);
      if (rows.length === 0) return { ok: false, why: 'no rows' };
      start();
      for (const row of rows) {
        row.click();
        await wait(450);
      }
      return { ok: q('reader') !== null, opened: rows.length, perf: stop() };
    },
  };

  const results = [];
  window.__smoke = {
    start(name, index) {
      results[index] = null;
      scenarios[name]().then(
        (r) => { results[index] = { step: name, ...r }; },
        (e) => { results[index] = { step: name, ok: false, why: String(e) }; },
      );
      return true;
    },
    result(index) {
      return results[index] ?? null;
    },
  };
})();"#;

const PROBE: &str = r#"(() => { try {
    const q = (id) => document.querySelector(`[data-testid="${id}"]`);
    const shown = (el) => !!el && el.getBoundingClientRect().width > 0;
    return JSON.stringify({
      // The sidebar's entries come with the app state (nothing is guessed before it).
      ready: shown(q('shell')) && shown(q('sidebar')) && q('nav-jobs') !== null,
      tabs: document.querySelectorAll('[data-testid^="nav-"]').length,
      named: ['nav-jobs', 'nav-archive', 'nav-trash', 'nav-profile', 'nav-settings'].every((id) => !!q(id)),
      tauri: '__TAURI_INTERNALS__' in window,
      csp: window.__smokeCsp ?? null,
    });
  } catch (e) { return JSON.stringify({ error: String(e) }); } })()"#;

const CSP_PROBE: &str = "JSON.stringify({ csp: window.__smokeCsp ?? null })";

/// The tab currently visited (index into [`TABS`]).
static TAB: AtomicUsize = AtomicUsize::new(0);
/// The step of `--smoke-run` currently running (index into [`SCENARIOS`]).
static SCENARIO: AtomicUsize = AtomicUsize::new(0);
/// A step of `--smoke-run` failed: the check goes on (the numbers of the later steps still
/// help) and ends with 1.
static FAILED: AtomicBool = AtomicBool::new(false);
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
    let limit = if flag("--smoke-run") {
        WATCHDOG_RUN
    } else {
        WATCHDOG
    };
    // The watchdog runs from window creation: if loading already hangs, the process still
    // ends - directly, because the event loop may then be blocked.
    std::thread::spawn(move || {
        std::thread::sleep(limit);
        let last = LAST.lock().map(|l| l.clone()).unwrap_or_default();
        println!("SMOKE timeout {last}");
        std::process::exit(2);
    });
    builder
        .initialization_script(INIT)
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
    // Jobs with its two places (Archiv, Papierkorb), Profil, Einstellungen.
    let ok = value["tabs"] == 5
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
/// last tab the steps of `--smoke-run`, then the final CSP check.
fn show_next_tab<R: Runtime>(window: &WebviewWindow<R>) {
    let Some(tab) = TABS.get(TAB.load(Ordering::SeqCst)) else {
        if flag("--smoke-run") {
            // The run starts in the Jobs view, where the list shows its rows and rings.
            let back = "document.querySelector('[data-testid=\"nav-jobs\"]').click()";
            if let Err(error) = window.eval(back) {
                println!("SMOKE eval failed: {error}");
                window.app_handle().exit(1);
                return;
            }
            SCENARIO.store(0, Ordering::SeqCst);
            next_scenario(window);
        } else {
            finish(window);
        }
        return;
    };
    let click = format!("document.querySelector('[data-testid=\"nav-{tab}\"]').click()");
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
    #[cfg(target_os = "macos")]
    report_traffic_lights(window, value);
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(HOLD);
        TAB.fetch_add(1, Ordering::SeqCst);
        show_next_tab(&window);
    });
}

/// macOS, once the Jobs view is on screen: where the close button sits
/// (`SMOKE {"lights":...}`: configured position, the button's x, y from the top, width and
/// height in points, and its centre), so the CI log shows whether the lights are centred in
/// the page's 52 px toolbar row (centre 26).
#[cfg(target_os = "macos")]
fn report_traffic_lights<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    if value["tab"] != "jobs" {
        return;
    }
    crate::platform::lights::report(window, |report| {
        let centre = report.close.map(|[_, top, _, height]| top + height / 2.0);
        let line = serde_json::json!({
            "lights": {
                "configured": report.configured.map(|(x, y)| [x, y]),
                "close": report.close,
                "centreY": centre,
                "windowHeight": report.window_height,
            }
        });
        println!("SMOKE {line}");
    });
}

/// Starts the next step of `--smoke-run` in the page and waits for its result.
fn next_scenario<R: Runtime>(window: &WebviewWindow<R>) {
    let index = SCENARIO.load(Ordering::SeqCst);
    let Some(name) = SCENARIOS.get(index) else {
        finish(window);
        return;
    };
    if let Err(error) = window.eval(format!("window.__smoke.start('{name}', {index})")) {
        println!("SMOKE eval failed: {error}");
        window.app_handle().exit(1);
        return;
    }
    let probe = format!("JSON.stringify(window.__smoke.result({index}))");
    ask(window, probe, |v| v["ok"].is_boolean(), scenario_done);
}

fn scenario_done<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    if value["ok"] != true {
        println!(
            "SMOKE step {} failed: {}",
            value["step"].as_str().unwrap_or_default(),
            value["why"].as_str().unwrap_or("see above")
        );
        FAILED.store(true, Ordering::SeqCst);
    }
    SCENARIO.fetch_add(1, Ordering::SeqCst);
    next_scenario(window);
}

fn finish<R: Runtime>(window: &WebviewWindow<R>) {
    ask(window, CSP_PROBE.to_owned(), |_| true, check_csp);
}

fn check_csp<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    let ok = no_csp_violation(value) && !FAILED.load(Ordering::SeqCst);
    window.app_handle().exit(i32::from(!ok));
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
