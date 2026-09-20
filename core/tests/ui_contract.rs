//! Die festen Regeln der Oberfläche. Sie beschreiben, was sich beim Weiterbauen nicht
//! von selbst wieder einschleichen soll – jede Regel steht hier, weil sie schon einmal
//! gebrochen wurde oder weil der Nutzer sie ausdrücklich verlangt hat.

use std::path::{Path, PathBuf};

fn repo(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(repo(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

/// Alle Dateien eines Ordners (rekursiv) mit passender Endung – Pfad und Inhalt.
fn files(dir: &str, ext: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![repo(dir)];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == ext) {
                let name = path
                    .strip_prefix(repo(""))
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Ok(text) = std::fs::read_to_string(&path) {
                    out.push((name, text));
                }
            }
        }
    }
    out.sort();
    out
}

fn scripts() -> Vec<(String, String)> {
    files("ui/js", "js")
}

/// Zeilen ohne Kommentar – sonst schlagen die Regeln auf ihrer eigenen Erklärung an.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter(|(_, line)| {
        let t = line.trim_start();
        !t.starts_with("//") && !t.starts_with('*') && !t.starts_with("/*")
    })
}

/// Nur `ui/js/api.js` darf Tauri kennen: sonst ließe sich die Oberfläche nicht im
/// Prüfstand ohne Rust betreiben – und ein zweiter Zugang wäre nie wieder zu finden.
#[test]
fn tauri_only_in_api() {
    for (name, text) in scripts() {
        if name.ends_with("api.js") {
            continue;
        }
        for (i, line) in code_lines(&text) {
            assert!(
                !line.contains("__TAURI__"),
                "{name}:{}: __TAURI__ gehört allein in api.js",
                i + 1
            );
        }
    }
}

/// Fremde Texte (Titel, Firmen) kommen aus Mails. Sie werden nie als HTML eingesetzt.
#[test]
fn no_html_injection() {
    for (name, text) in scripts() {
        for (i, line) in code_lines(&text) {
            for bad in ["innerHTML", "outerHTML", "insertAdjacentHTML", "document.write"] {
                assert!(
                    !line.contains(bad),
                    "{name}:{}: {bad} – Inhalte werden als Text gesetzt, nie als HTML",
                    i + 1
                );
            }
        }
    }
}

/// Es ist ein Werkzeug, kein Browserfenster: Tasten und fremde Maustasten werden an genau
/// einer Stelle behandelt. Sonst wächst wieder ein Geflecht aus Kürzeln (zuletzt: Escape
/// an vier Stellen, zwei Doppelklick-Gesten).
#[test]
fn input_handling_lives_in_one_file() {
    const EVENTS: [&str; 7] = [
        "keydown",
        "keyup",
        "keypress",
        "dblclick",
        "contextmenu",
        "auxclick",
        "dragstart",
    ];
    for (name, text) in scripts() {
        if name.ends_with("input.js") {
            continue;
        }
        for (i, line) in code_lines(&text) {
            for event in EVENTS {
                assert!(
                    !line.contains(event),
                    "{name}:{}: „{event}“ gehört nach ui/js/input.js",
                    i + 1
                );
            }
        }
    }
    // Diese muss input.js tatsächlich abfangen; die übrigen sind nur anderswo verboten.
    let input = read("ui/js/input.js");
    for event in ["keydown", "contextmenu", "dblclick", "auxclick", "dragstart"] {
        assert!(input.contains(event), "input.js fängt „{event}“ nicht ab");
    }
}

/// Bedienelemente entstehen ausschließlich über die Fabriken in `ui.js`. Damit kann es
/// keine zweite Bauart desselben Knopfs geben – das war der Grund für 27 Optiken.
#[test]
fn controls_come_from_the_factories() {
    for (name, text) in scripts() {
        if name.ends_with("ui.js") {
            continue;
        }
        for (i, line) in code_lines(&text) {
            for bad in ["el('button'", "el('input'", "el('select'", "el('textarea'"] {
                assert!(
                    !line.contains(bad),
                    "{name}:{}: {bad} – Bedienelemente kommen aus den Fabriken in ui.js",
                    i + 1
                );
            }
        }
    }
}

/// Die Klassenliste der Bausteine. Taucht eine neue auf, ist entweder ein Baustein
/// dazugekommen (dann gehört er in components.css und hierher) oder es war ein Alleingang.
#[test]
fn only_known_components() {
    const KNOWN: [&str; 42] = [
        // Bausteine
        "btn", "primary", "ghost", "danger", "icon", "seg", "seg-thumb", "seg-item", "seg-count",
        "switch", "switch-track", "field", "field-label", "field-hint", "input", "notice",
        "notice-dot", "notice-text", "row", "row-title", "row-date", "row-meta", "row-new",
        "row-flag", "row-where", "setting", "setting-text", "setting-label", "setting-hint",
        "setting-controls", "steps", "step", "step-num", "step-label", "dialog", "dialog-title",
        "dialog-body", "dialog-foot", "spinner", "skeleton", "win-btn", "win-close",
    ];
    // Klassen, die den Aufbau beschreiben (app.css), nicht die Bausteine.
    const LAYOUT: [&str; 26] = [
        "titlebar", "brand", "brand-mark", "brand-text", "titlebar-fill", "mode-tag",
        "win-buttons", "app", "toolbar", "toolbar-fill", "search", "page", "page-body", "split",
        "list", "reader", "reader-inner", "reader-title", "reader-meta", "reader-actions",
        "reader-rule", "reader-text", "reader-empty", "reader-skeleton", "reader-body", "blank",
    ];
    const EXTRA: [&str; 17] = [
        "settings", "settings-inner", "group", "group-title", "form-grid", "form-actions",
        "statusbar", "status-dot", "status-text", "status-count", "status-line", "panel",
        "panel-list", "log-line", "boot", "sheet", "notices",
    ];
    // Zustände, die nur gesetzt, nie als Baustein gemeint sind.
    const STATES: [&str; 8] = [
        "is-open", "is-new", "is-swapping", "indeterminate", "stacked", "compact", "busy",
        "pointer-away",
    ];
    // Teile der beiden Zeichnungen (Marke, Fensterknopf) – Grafik, kein Bedienelement.
    const DRAWING: [&str; 4] = ["plate", "paper", "tick", "caption-glyph"];

    let mut allowed: Vec<&str> = KNOWN.into_iter().collect();
    allowed.extend(LAYOUT);
    allowed.extend(EXTRA);
    allowed.extend(STATES);
    allowed.extend(DRAWING);

    for (name, text) in scripts() {
        for (i, line) in code_lines(&text) {
            let Some(rest) = line.split("class: '").nth(1) else {
                continue;
            };
            let Some(value) = rest.split('\'').next() else {
                continue;
            };
            for class in value.split_whitespace() {
                // Vorlagenteile (`${…}`) prüft die Laufzeit, nicht dieser Test.
                if class.contains('$') || class.contains('{') {
                    continue;
                }
                assert!(
                    allowed.contains(&class),
                    "{name}:{}: unbekannte Klasse „{class}“ – neuer Baustein oder Alleingang?",
                    i + 1
                );
            }
        }
    }
}

/// Farben, Radien und Dauern stehen in `tokens.css`. Ein Literal daneben wäre der Anfang
/// einer zweiten Farbwelt.
#[test]
fn colours_only_in_tokens() {
    for file in ["ui/components.css", "ui/app.css"] {
        let text = read(file);
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("/*") || t.starts_with('*') {
                continue;
            }
            // Schatten und Fokusringe dürfen eine Farbe nennen, sie sind Teil der Form.
            if t.starts_with("box-shadow") || t.contains("outline:") {
                continue;
            }
            for bad in ["#", "rgb(", "rgba(", "hsl("] {
                assert!(
                    !t.contains(bad),
                    "{file}:{}: Farbe „{bad}“ außerhalb von tokens.css",
                    i + 1
                );
            }
        }
    }
}

/// Was unter Windows funktioniert, muss auch unter macOS funktionieren: `-webkit-app-region`
/// kennt WKWebView nicht, und die Symbolschrift „Segoe Fluent Icons“ gibt es dort nicht.
#[test]
fn nothing_windows_only_in_the_interface() {
    let mut all: Vec<(String, String)> = files("ui", "css");
    all.extend(scripts());
    all.push(("ui/index.html".into(), read("ui/index.html")));
    for (name, text) in all {
        for (i, line) in code_lines(&text) {
            for bad in ["app-region", "Segoe Fluent", "Segoe MDL2"] {
                assert!(
                    !line.contains(bad),
                    "{name}:{}: „{bad}“ gibt es auf dem Mac nicht",
                    i + 1
                );
            }
        }
    }
    // Der Ziehbereich muss über den plattformneutralen Weg laufen.
    assert!(
        read("ui/index.html").contains("data-tauri-drag-region"),
        "Die Titelleiste braucht data-tauri-drag-region, sonst lässt sich das Fenster auf dem Mac nicht ziehen"
    );
}

/// Der Prüfstand hängt an genau dieser Zeile: Er ersetzt sie, um das nachgebaute Backend
/// davorzuschieben. Ändert sie sich, lädt der Prüfstand stillschweigend eine Seite ohne Backend.
#[test]
fn the_harness_hook_is_intact() {
    assert!(
        read("ui/index.html").contains(r#"<script type="module" src="js/main.js"></script>"#),
        "tools/ui-harness/server.mjs ersetzt genau diese Zeichenkette"
    );
}

/// Es ist ein Werkzeug: kurze Beschriftungen, kurze Hinweise, keine Textwände.
/// Gemessen wurde vorher: 22 Texte über 60 Zeichen, der längste 640.
#[test]
fn the_interface_stays_short() {
    const MAX: usize = 140;
    for (name, text) in scripts() {
        for (i, line) in code_lines(&text) {
            for quoted in line.split('\'').skip(1).step_by(2) {
                // Kein Fließtext: Vorlagen, Selektoren und SVG-Pfaddaten zählen nicht.
                if quoted.contains("${") || !quoted.contains(' ') || is_path_data(quoted) {
                    continue;
                }
                assert!(
                    quoted.chars().count() <= MAX,
                    "{name}:{}: {} Zeichen – höchstens {MAX} („{}…“)",
                    i + 1,
                    quoted.chars().count(),
                    quoted.chars().take(50).collect::<String>()
                );
            }
        }
    }
}

/// SVG-Pfaddaten („M12 9.2a2.8 …“) sind lang, aber niemand liest sie.
fn is_path_data(text: &str) -> bool {
    let body = text.trim();
    body.starts_with('M')
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || "MLHVCSQTAZmlhvcsqtaz .,-".contains(c))
}

/// Ein Wort je Sache. Die alten Begriffe sind gefallen, sie dürfen nicht zurückkommen.
#[test]
fn one_word_per_thing() {
    const GONE: [(&str, &str); 4] = [
        ("Quelle", "Portal"),
        ("Eintrag", "Job"),
        ("Volltext", "Jobdetails"),
        ("Kandidat", "Job"),
    ];
    for (name, text) in scripts() {
        for (i, line) in code_lines(&text) {
            for (old, new) in GONE {
                assert!(
                    !line.contains(old),
                    "{name}:{}: „{old}“ heißt überall „{new}“",
                    i + 1
                );
            }
        }
    }
}
