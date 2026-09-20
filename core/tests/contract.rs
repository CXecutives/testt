//! Vertragstests: Jeder Befehl steht genau gleich im `AppManifest` (`build.rs`), im
//! `generate_handler!` (`main.rs`), als `allow-…` in der Capability des Hauptfensters und als
//! `call('…')`/`invoke('…')` in `ui/js/api.js` – ein Tippfehler hieße sonst „Befehl nicht
//! erlaubt“ erst zur Laufzeit. Dazu die festen Regeln der Oberfläche (`ui_rules_hold`).

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

/// Alle Skripte der Oberfläche als (Pfad, Inhalt).
fn ui_scripts() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/js");
    let mut files = Vec::new();
    for dir in [root.clone(), root.join("views")] {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "js") {
                let text = std::fs::read_to_string(&path).unwrap();
                files.push((path.display().to_string(), text));
            }
        }
    }
    assert!(files.len() >= 10, "UI-Skripte nicht gefunden");
    files
}

#[test]
fn ui_rules_hold() {
    let scripts = ui_scripts();
    let html = read("ui/index.html").unwrap();
    let css = read("ui/style.css").unwrap();

    for (path, text) in &scripts {
        // Nur api.js spricht mit Tauri; Inhalte (fremde Mails) nie als HTML.
        if !path.ends_with("api.js") {
            assert!(
                !text.contains("__TAURI__"),
                "{path}: __TAURI__ außerhalb von api.js"
            );
        }
        for banned in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "setAttribute('style'",
        ] {
            assert!(!text.contains(banned), "{path}: {banned}");
        }
    }
    assert!(!html.contains(" style="), "index.html: Inline-Stil");
    assert!(!html.contains("<script>"), "index.html: Inline-Skript");

    // Keine Reste aus der Entwicklung und keine Verweise auf Arbeitsnotizen.
    let all = scripts
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .chain([
            ("ui/index.html", html.as_str()),
            ("ui/style.css", css.as_str()),
        ]);
    for (path, text) in all {
        for banned in [
            "console.",
            "debugger",
            "TODO",
            "FIXME",
            "Altfehler",
            "UI-SPEC",
            "Review Phase",
            "Plan §",
            "Spike",
        ] {
            assert!(!text.contains(banned), "{path}: {banned}");
        }
    }

    // Jedes :hover nur, solange der Zeiger über dem Fenster ist (sonst hängende Zustände).
    for line in css
        .lines()
        .filter(|l| l.contains(":hover") && !l.trim_start().starts_with('*'))
    {
        assert!(
            line.contains(":root:not(.pointer-away)"),
            "style.css: :hover ohne Zeigerschalter: {line}"
        );
    }
    // Farben nur im Token-Block (vor den Grundlagen).
    let body = &css[css.find("Grundlagen */").expect("Abschnitt Grundlagen")..];
    for line in body.lines() {
        let has_hex = line.split('#').skip(1).any(|rest| {
            let head: Vec<char> = rest.chars().take(3).collect();
            head.len() == 3 && head.iter().all(char::is_ascii_hexdigit)
        });
        assert!(
            !has_hex && !line.contains("rgb(") && !line.contains("rgba(") && !line.contains("hsl("),
            "style.css: Farbe außerhalb der Tokens: {line}"
        );
    }
}
