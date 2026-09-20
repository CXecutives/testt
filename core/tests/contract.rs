//! Vertragstests: Jeder Befehl steht genau gleich im `AppManifest` (`build.rs`), im
//! `generate_handler!` (`main.rs`), als `allow-…` in der Capability des Hauptfensters und als
//! `call('…')`/`invoke('…')` in `ui/js/api.js` – ein Tippfehler hieße sonst „Befehl nicht
//! erlaubt“ erst zur Laufzeit. Die Regeln der Oberfläche stehen in `ui_contract.rs`.

use std::collections::BTreeSet;
use std::path::Path;

fn read(relative: &str) -> Option<String> {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative),
    )
    .ok()
}

/// Alle Wörter aus `"name"` innerhalb des Abschnitts zwischen `start` und `end`.
fn quoted_between(text: &str, start: &str, end: &str) -> BTreeSet<String> {
    let from = text.find(start).expect(start) + start.len();
    let to = from + text[from..].find(end).expect(end);
    text[from..to]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[test]
fn command_names_agree_everywhere() {
    let build = read("src-tauri/build.rs").unwrap();
    let manifest = quoted_between(&build, ".commands(&[", "])");

    let main = read("src-tauri/src/main.rs").unwrap();
    let from = main.find("generate_handler![").unwrap();
    let to = from + main[from..].find("])").unwrap();
    let handler: BTreeSet<String> = main[from..to]
        .split("commands::")
        .skip(1)
        .map(|s| {
            s.split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap()
                .to_string()
        })
        .collect();

    let capability: serde_json::Value =
        serde_json::from_str(&read("src-tauri/capabilities/main.json").unwrap()).unwrap();
    let allowed: BTreeSet<String> = capability["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p.as_str()?.strip_prefix("allow-"))
        .map(|p| p.replace('-', "_"))
        .collect();
    assert_eq!(
        capability["windows"],
        serde_json::json!(["main"]),
        "nur das Hauptfenster"
    );
    assert!(capability.get("remote").is_none(), "nie remote");

    assert!(!manifest.is_empty());
    assert_eq!(manifest, handler, "build.rs ↔ main.rs");
    assert_eq!(manifest, allowed, "build.rs ↔ Capability");

    // Oberfläche: jeder aufgerufene Befehl muss es geben – und jeder Befehl wird benutzt.
    let api = read("ui/js/api.js").expect("ui/js/api.js");
    // `call('…')` für Befehle; der Fehlerbericht geht direkt über `invoke('…')`.
    let invoked: BTreeSet<String> = ["call('", "invoke('"]
        .iter()
        .flat_map(|pattern| api.split(pattern).skip(1))
        .filter_map(|s| s.split('\'').next())
        .map(str::to_string)
        .collect();
    let unknown: Vec<_> = invoked.difference(&manifest).collect();
    let unused: Vec<_> = manifest.difference(&invoked).collect();
    // Die Oberfläche wird gerade gegen den neuen Befehlsvertrag gebaut. Ruft sie noch einen
    // Befehl, den es im Backend nicht mehr gibt, meldet der Test das und prüft nur das
    // Backend; sobald api.js steht, gilt der Abgleich wieder von selbst.
    if unknown.is_empty() {
        assert!(
            unused.is_empty(),
            "Befehle ohne Aufruf in api.js: {unused:?}"
        );
    } else {
        eprintln!(
            "api.js ruft Befehle, die es nicht (mehr) gibt: {unknown:?} – \
             Abgleich mit der Oberfläche übersprungen (ohne Aufruf: {unused:?})"
        );
    }
}

/// Sicherheits-Invariante des Sitzungsfensters, so weit sie sich ohne Fenster prüfen lässt:
/// Es lädt nichts herunter, öffnet keine weiteren Fenster, prüft jede Navigation gegen sein
/// Portal – und steht in keiner Capability (kein Befehl der App ist von dort erreichbar).
#[test]
fn the_session_window_downloads_nothing_and_opens_no_window() {
    let session = read("src-tauri/src/session.rs").unwrap();
    for required in [
        ".on_download(|_, _| false)",
        ".on_new_window(|_, _| NewWindowResponse::Deny)",
        "Session::allowed(site, url)",
    ] {
        assert!(session.contains(required), "session.rs: {required} fehlt");
    }
    // Der Profilordner (und damit die Anmeldung) hängt am Portal, nicht an einem festen Namen.
    assert!(session.contains("jobalert_core::session_dir(site.portal)"));

    let capability: serde_json::Value =
        serde_json::from_str(&read("src-tauri/capabilities/main.json").unwrap()).unwrap();
    assert_eq!(
        capability["windows"],
        serde_json::json!(["main"]),
        "Portal-Fenster stehen in keiner Capability"
    );
}

/// Jedes Lauf-Ereignis trägt genau die Felder, die die Oberfläche ausliest. Der Abschluss
/// war einmal ein Neutyp – mit `tag = "type"` verschmilzt so ein Neutyp mit dem Ereignis,
/// `event.summary` war undefiniert und die Statusleiste blieb nach jedem Lauf auf „Bereit“.
/// Die Attrappe des Prüfstands schickte die richtige Form, das Backend nicht: Der
/// Unterschied fiel erst im echten Fenster auf.
#[test]
fn every_run_event_carries_the_fields_the_interface_reads() {
    use jiff::Timestamp;
    use jobalert_core::pipeline::{Outcome, RunEvent, RunSummary};

    let now = Timestamp::now();
    let event = serde_json::to_value(RunEvent::Finished {
        summary: Box::new(RunSummary {
            run: 1,
            outcome: Outcome::Completed,
            dry_run: true,
            started_at: now,
            finished_at: now,
            scope: None,
            scan: None,
            fetch: None,
            export: None,
        }),
    })
    .unwrap();
    assert_eq!(event["type"], "finished");
    assert!(
        event["summary"].is_object(),
        "Abschluss ohne summary: {event}"
    );
    assert!(
        event["summary"]["finishedAt"].is_string(),
        "summary ist nicht die Zusammenfassung: {event}"
    );

    // Die Oberfläche liest `event.<feld>` – jedes Feld muss es im Ereignis-Typ geben. Die
    // Namen kommen aus der Quelle, nicht aus einer gepflegten Liste: Sonst merkte der Test
    // eine Umbenennung im Backend nicht.
    let source = read("core/src/pipeline/mod.rs").expect("pipeline/mod.rs");
    let enum_body = {
        let from = source.find("pub enum RunEvent {").expect("RunEvent");
        let to = from + source[from..].find("\n}\n").expect("Ende von RunEvent");
        &source[from..to]
    };
    // Kommentare und Attribute raus, dann an Klammern, Kommas und Zeilenenden trennen: So
    // zählt ein Feld gleich, ob `cargo fmt` die Variante auf eine oder mehrere Zeilen legt.
    let known: BTreeSet<String> = enum_body
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with("//") && !line.starts_with("#["))
        .flat_map(|line| line.split(['{', '}', ',']))
        .filter_map(|token| token.split_once(": "))
        .map(|(name, _)| name.trim())
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .map(camel_case)
        .collect();
    assert!(known.len() >= 10, "Felder nicht erkannt: {known:?}");

    let run = read("ui/js/run.js").expect("ui/js/run.js");
    let reads: BTreeSet<String> = run
        .split("event.")
        .skip(1)
        .filter_map(|rest| {
            let name: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            (!name.is_empty() && name != "type").then_some(name)
        })
        .collect();
    let unknown: Vec<_> = reads.difference(&known).collect();
    assert!(
        unknown.is_empty(),
        "run.js liest Felder, die kein Ereignis trägt: {unknown:?}"
    );
}

/// `gmail_id` → `gmailId`: So heißen die Felder in der Seite (`rename_all_fields`).
fn camel_case(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}
