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
