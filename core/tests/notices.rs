//! `ui/THIRD-PARTY.txt` muss zu den Abhängigkeiten passen. MIT und Apache-2.0 verlangen,
//! dass der Urheberhinweis bei der Weitergabe mitgeht – eine veraltete Liste wäre schlimmer
//! als keine, weil sie Vollständigkeit vortäuscht.
//!
//! Erzeugt wird die Datei mit `node tools/third-party.mjs`.

use std::path::Path;

fn read(relative: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative),
    )
    .unwrap_or_else(|e| panic!("{relative}: {e}"))
}

/// Die Namen der Abhängigkeiten aus dem `[dependencies]`-Abschnitt einer Cargo.toml.
fn direct_dependencies(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // Test-Abhängigkeiten landen nicht im Programm und brauchen keinen Hinweis.
            inside = line.contains("dependencies") && !line.contains("dev-dependencies");
            continue;
        }
        if !inside || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            let name = name.trim().trim_matches('"');
            if !name.is_empty() && !name.contains('.') {
                out.push(name.to_string());
            }
        }
    }
    out
}

#[test]
fn every_dependency_is_listed() {
    let notices = read("ui/THIRD-PARTY.txt");
    let mut missing = Vec::new();
    for manifest in ["core/Cargo.toml", "src-tauri/Cargo.toml"] {
        for name in direct_dependencies(&read(manifest)) {
            if name == "jobalert-core" {
                continue; // eigener Code
            }
            if !notices.contains(&format!("\n{name} ")) {
                missing.push(format!("{name} (aus {manifest})"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "Diese Abhängigkeiten fehlen in ui/THIRD-PARTY.txt: {}\n\
         Neu erzeugen mit: node tools/third-party.mjs",
        missing.join(", ")
    );
}

/// Ein leerer oder abgeschnittener Hinweis wäre schlimmer als keiner.
#[test]
fn the_notice_file_is_complete() {
    let notices = read("ui/THIRD-PARTY.txt");
    assert!(
        notices.len() > 100_000,
        "ui/THIRD-PARTY.txt ist nur {} Bytes groß – abgeschnitten?",
        notices.len()
    );
    for marker in [
        "Mitgelieferte Bausteine",
        "Lizenztexte",
        "MIT",
        "Apache License",
    ] {
        assert!(
            notices.contains(marker),
            "„{marker}“ fehlt in ui/THIRD-PARTY.txt"
        );
    }
    // Die Schrift wird getrennt gehalten, der Verweis darauf muss stimmen.
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ui/fonts/Inter-LICENSE.txt")
            .exists(),
        "Die Lizenz der mitgelieferten Schrift fehlt"
    );
}
