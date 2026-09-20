//! Selbstprüfung für Entwicklung und Abnahme: `job-alert-monitor --smoke` lädt die
//! Oberfläche, liest ein paar Kennwerte per Host-`eval`, druckt sie als JSON und beendet
//! sich mit 0 (in Ordnung), 1 (Kennwerte falsch) oder 2 (Zeitüberschreitung). Mit
//! `--dry-run --smoke-run` klickt sie danach „Abrufen“ und wartet, bis der Demo-Lauf
//! fertig ist und seine Jobs in der Liste stehen.
//!
//! Nur im Entwickler-Build (`#[cfg(debug_assertions)]` an der Einbindung in `main.rs`).

use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

const PROBE: &str = r"(() => { try { const q = (s) => document.querySelector(s); return JSON.stringify({
    ready: q('#boot').hidden && !q('#app').hidden,
    pages: document.querySelectorAll('.page').length,
    primary: getComputedStyle(q('#run')).backgroundColor,
    tauri: typeof window.__TAURI__ === 'object',
    statusbarVisible: q('#statusbar').getBoundingClientRect().bottom <= window.innerHeight + 1,
    actions: document.querySelectorAll('#toolbar .btn').length,
    busy: document.querySelectorAll('.btn[aria-busy=true]').length,
  }); } catch (e) { return JSON.stringify({ error: String(e) }); } })()";

// Der Stand des Verlaufs vor dem Klick: Nur daran lässt sich später sehen, dass dieser
// Lauf stattgefunden hat und nicht ein Ergebnis von vorhin in der Liste steht.
const START: &str = "window.__smokeLog = document.querySelectorAll('#log .log-line').length; \
                     document.querySelector('#run').click()";

const RUN_PROBE: &str = r"(() => { try { const q = (s) => document.querySelector(s); return JSON.stringify({
    status: q('#status-text').textContent,
    running: !q('#statusbar .spinner').hidden,
    swapping: q('#status-text').classList.contains('is-swapping'),
    level: q('#statusbar').dataset.level || '',
    rows: document.querySelectorAll('#list .row').length,
    ran: document.querySelectorAll('#log .log-line').length > (window.__smokeLog ?? 0),
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
    let ok = value["pages"] == 2
        && value["primary"]
            .as_str()
            .is_some_and(|c| c.starts_with("rgb(") && !c.contains(", 0)"))
        && value["tauri"] == true
        && value["statusbarVisible"] == true
        && value["actions"].as_u64().is_some_and(|n| n >= 3)
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

/// Der Lauf ist vorbei, wenn der Kreisel weg ist **und** der Verlauf seit dem Klick
/// gewachsen ist. Ohne das Zweite gälte ein Ergebnis von vorhin schon als fertig.
/// Der Satz blendet über; solange er wechselt, steht dort noch der vorige.
fn run_done(value: &Value) -> bool {
    value["running"] == false && value["ran"] == true && value["swapping"] == false
}

fn check_run<R: Runtime>(window: &WebviewWindow<R>, value: &Value) {
    let ok = value["running"] == false
        && value["ran"] == true
        && value["level"] != "error"
        && value["rows"].as_u64().is_some_and(|n| n >= 5)
        && !value["status"].as_str().unwrap_or_default().is_empty()
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
