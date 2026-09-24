//! `src-tauri/resources/THIRD-PARTY.txt` must match the dependencies. MIT and
//! Apache-2.0 require the copyright notice to travel along on redistribution - a stale
//! list would be worse than none, because it fakes completeness.
//!
//! The file is generated with `node tools/third-party.mjs`.

use std::path::Path;

const NOTICES: &str = "src-tauri/resources/THIRD-PARTY.txt";

fn read(relative: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative),
    )
    .unwrap_or_else(|e| panic!("{relative}: {e}"))
}

/// The names of the dependencies from the `[dependencies]` section of a Cargo.toml.
fn direct_dependencies(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // Test dependencies don't end up in the program and need no notice.
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

/// The runtime packages of the UI (`dependencies` in package.json, not the dev tooling).
fn npm_runtime_dependencies(package_json: &str) -> Vec<String> {
    let manifest: serde_json::Value = serde_json::from_str(package_json).expect("package.json");
    manifest["dependencies"]
        .as_object()
        .map(|deps| deps.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn every_dependency_is_listed() {
    let notices = read(NOTICES);
    let mut missing = Vec::new();
    for manifest in ["core/Cargo.toml", "src-tauri/Cargo.toml"] {
        for name in direct_dependencies(&read(manifest)) {
            if name == "jobalert-core" {
                continue; // our own code
            }
            if !notices.contains(&format!("\n{name} ")) {
                missing.push(format!("{name} (from {manifest})"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "These dependencies are missing from {NOTICES}: {}\n\
         Regenerate with: node tools/third-party.mjs",
        missing.join(", ")
    );
}

/// The npm packages bundled into the UI need their notices as much as the crates.
#[test]
fn every_npm_runtime_package_is_listed() {
    let notices = read(NOTICES);
    let runtime = npm_runtime_dependencies(&read("package.json"));
    assert!(
        runtime.len() >= 3,
        "package.json lists only {runtime:?} as runtime dependencies"
    );
    let missing: Vec<&String> = runtime
        .iter()
        .filter(|name| !notices.contains(&format!("\n{name} ")))
        .collect();
    assert!(
        missing.is_empty(),
        "npm packages missing in {NOTICES}: {missing:?}\n\
         Regenerate with: node tools/third-party.mjs"
    );
}

/// An empty or truncated notice would be worse than none.
#[test]
fn the_notice_file_is_complete() {
    let notices = read(NOTICES);
    assert!(
        notices.len() > 100_000,
        "{NOTICES} is only {} bytes - truncated?",
        notices.len()
    );
    for marker in [
        "Bundled components",
        "npm packages",
        "License texts",
        "MIT",
        "Apache License",
        "SIL OPEN FONT LICENSE",
    ] {
        assert!(
            notices.contains(marker),
            "\"{marker}\" missing from {NOTICES}"
        );
    }
    // The font is kept separate, so the reference to it must be correct.
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ui/src/assets/fonts/Inter-LICENSE.txt")
            .exists(),
        "the licence of the bundled font is missing"
    );
}
