//! Selbstprüfung für Entwicklung und Abnahme: `job-alert-monitor --smoke` lädt die
//! Oberfläche, liest ein paar Kennwerte per Host-`eval`, druckt sie als JSON und beendet
//! sich mit 0 (in Ordnung), 1 (Kennwerte falsch) oder 2 (Zeitüberschreitung). Mit
//! `--dry-run --smoke-run` klickt sie danach „Postfach abrufen“ und wartet, bis der
//! Demo-Lauf fertig ist und seine Jobs in der Tabelle stehen.
//!
//! Nur im Entwickler-Build (`#[cfg(debug_assertions)]` an der Einbindung in `main.rs`).

use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

const PROBE: &str = r"(() => { try { return JSON.stringify({
    ready: document.documentElement.dataset.ready === '1',
    views: document.querySelectorAll('.view').length,
    primary: getComputedStyle(document.querySelector('#topbar-actions .btn.primary')).backgroundColor,
    tauri: typeof window.__TAURI__ === 'object',
    runbarVisible: document.querySelector('#runbar').getBoundingClientRect().bottom <= window.innerHeight,
    cards: document.querySelectorAll('.view .card').length,
    actions: document.querySelectorAll('#topbar-actions .btn').length,
    busy: document.querySelectorAll('.btn[aria-busy=true]').length,
  }); } catch (e) { return JSON.stringify({ error: String(e) }); } })()";

const START: &str = "document.querySelector('#topbar-actions .btn.primary').click()";

const RUN_PROBE: &str = r"(() => { try { return JSON.stringify({
    status: document.querySelector('#run-status').textContent,
    rows: document.querySelectorAll('#view-alerts tbody tr[data-key]').length,
    ok: document.querySelectorAll('#view-alerts tbody .badge.ok').length,
    log: document.querySelectorAll('#log .log-line').length,
    busy: document.querySelectorAll('.btn[aria-busy=true]').length,
  }); } catch (e) { return JSON.stringify({ error: String(e) }); } })()";

/// Letzte Antwort der Seite – der Wächter druckt sie bei Zeitüberschreitung.
static LAST: Mutex<String> = Mutex::new(String::new());

fn flag(name: &str) -> bool {
    std::env::args().any(|arg| arg == name)
}

pub fn attach<R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'_, R, M>,
) -> WebviewWindowBuilder<'_, R, M> {
    // `--smoke-run` schließt `--smoke` ein: Ohne Selbstprüfung gäbe es keinen Wächter – die
    // Prüfung bliebe als gewöhnliches Fenster stehen, statt mit einem Code zu enden.
    if !flag("--smoke") && !flag("--smoke-run") {
        return builder;
    }
    // `--smoke-run` prüft den Demo-Lauf – der gibt es nur im Trockenlauf.
    if flag("--smoke-run") && !flag("--dry-run") {
        println!("SMOKE --smoke-run braucht --dry-run");
        std::process::exit(1);
    }
    let limit = if flag("--smoke-run") { 120 } else { 30 };
    // Der Wächter läuft ab Fenstererzeugung: Hängt schon das Laden, endet der
    // Prozess trotzdem – direkt, weil die Ereignisschleife dann blockiert sein kann.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(limit));
        let last = LAST.lock().map(|l| l.clone()).unwrap_or_default();
        println!("SMOKE Zeitüberschreitung {last}");
        std::process::exit(2);
    });
    builder.on_page_load(|window, payload| {
        if payload.event() == PageLoadEvent::Finished {
            ask(&window, PROBE, |v| v["ready"] == true, check_page);
        }
    })
}

fn check_page<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    // Die Markenfarbe wird am Primärknopf gemessen: ein aufgelöster Wert, der die
    // Palette (hsl-Tokens) nicht festschreibt, aber eine leere oder durchsichtige Fläche auffliegen lässt.
    let ok = value["views"] == 4
        && value["primary"]
            .as_str()
            .is_some_and(|c| c.starts_with("rgb(") && !c.contains(", 0)"))
        && value["tauri"] == true
        && value["runbarVisible"] == true
        && value["cards"].as_u64().is_some_and(|n| n >= 9)
        && value["actions"] == 2
        && value["busy"] == 0;
    if ok && flag("--smoke-run") && flag("--dry-run") {
        if let Err(error) = window.eval(START) {
            println!("SMOKE eval fehlgeschlagen: {error}");
            window.app_handle().exit(1);
            return;
        }
        ask(window, RUN_PROBE, run_done, check_run);
    } else {
        window.app_handle().exit(i32::from(!ok));
    }
}

fn run_done(value: &Value) -> bool {
    value["status"]
        .as_str()
        .is_some_and(|s| s.starts_with("Fertig") || s.starts_with("Fehler"))
        && value["rows"].as_u64().is_some_and(|n| n > 0)
}

fn check_run<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    let ok = value["status"]
        .as_str()
        .is_some_and(|s| s.starts_with("Fertig"))
        && value["rows"].as_u64().is_some_and(|n| n >= 5)
        && value["ok"].as_u64().is_some_and(|n| n >= 1)
        // Nach dem Lauf wartet kein Knopf mehr auf das Backend.
        && value["busy"] == 0;
    window.app_handle().exit(i32::from(!ok));
}

/// Fragt die Seite alle 200 ms, bis `done` gilt (die Gesamtzeit begrenzt der Wächter),
/// druckt die Antwort und übergibt sie an `then`.
fn ask<R: Runtime>(
    window: &WebviewWindow<R>,
    js: &'static str,
    done: fn(&Value) -> bool,
    then: fn(&WebviewWindow<R>, &Value),
) {
    let again = window.clone();
    let result = window.eval_with_callback(js, move |raw| {
        // Das Ergebnis ist JSON-serialisiert – hier also ein JSON-String mit JSON darin.
        let json: String = serde_json::from_str(&raw).unwrap_or(raw);
        let value = serde_json::from_str::<Value>(&json).unwrap_or_default();
        if let Ok(mut last) = LAST.lock() {
            last.clone_from(&json);
        }
        if !done(&value) {
            let again = again.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                ask(&again, js, done, then);
            });
            return;
        }
        println!("SMOKE {json}");
        then(&again, &value);
    });
    if let Err(error) = result {
        println!("SMOKE eval fehlgeschlagen: {error}");
        window.app_handle().exit(1);
    }
}
