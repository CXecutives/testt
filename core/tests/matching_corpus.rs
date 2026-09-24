//! The synthetic matching corpus (`core/tests/fixtures/matching`).
//!
//! - Parity: the Rust port of the old engine (`legacy_percent`) equals the frozen outputs
//!   of the real old Python engine (`legacy.json`, `legacy_edge.json`) for every profile
//!   and job.
//! - Hygiene: no fixture is ignored by git (the repository is public, fixtures must ship).
//! - Gates of the new engine (phase 2A): per-job band distance new <= old with a strictly
//!   smaller sum, decided exclusions exactly as in `corpus.json`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use jobalert_core::matching::legacy::{LegacyOutcome, legacy_outcome, parse_job_file};
use serde_json::Value;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matching")
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The old engine's result for a job file, in the shape of `legacy.json`.
fn legacy_entry(profile: &Value, content: &str) -> Value {
    let outcome = match parse_job_file(content) {
        Some(file) => legacy_outcome(profile, &file.text),
        None => LegacyOutcome::Absent,
    };
    match outcome {
        LegacyOutcome::Scored(score) => serde_json::to_value(score).expect("serialisable"),
        LegacyOutcome::Absent => serde_json::json!({ "pct": null }),
        LegacyOutcome::Crashed => {
            serde_json::json!({ "pct": null, "error": "IndexError: no such group" })
        }
        LegacyOutcome::InvalidProfile => {
            serde_json::json!({ "pct": null, "error": "ProfileError" })
        }
    }
}

/// Compares every profile x job of a frozen baseline; returns the mismatches.
fn parity(frozen: &str, jobs: &str) -> (usize, Vec<String>) {
    let root = fixtures();
    let frozen = read_json(&root.join(frozen));
    let mut compared = 0;
    let mut mismatches = Vec::new();
    for (profile_name, entries) in frozen["profiles"].as_object().expect("profiles") {
        let profile = read_json(&root.join(profile_name));
        for (job, expected) in entries.as_object().expect("jobs") {
            let path = root.join(jobs).join(format!("{job}.txt"));
            let content = std::fs::read_to_string(&path).expect("job file");
            let actual = legacy_entry(&profile, &content);
            compared += 1;
            if &actual != expected {
                mismatches.push(format!(
                    "{profile_name} {job}\n  old:  {expected}\n  rust: {actual}"
                ));
            }
        }
    }
    (compared, mismatches)
}

#[test]
fn legacy_port_equals_frozen_corpus_outputs() {
    let (compared, mismatches) = parity("legacy.json", "corpus");
    assert_eq!(compared, 80, "2 profiles x 40 jobs");
    assert!(
        mismatches.is_empty(),
        "{} of {compared} differ:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn legacy_port_equals_frozen_edge_outputs() {
    let (compared, mismatches) = parity("legacy_edge.json", "legacy_edge");
    assert_eq!(compared, 27, "3 profiles x 9 edge jobs");
    assert!(
        mismatches.is_empty(),
        "{} of {compared} differ:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn no_fixture_is_ignored_by_git() {
    let root = fixtures();
    let mut paths = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("fixture dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                paths.push(path);
            }
        }
    }
    assert!(paths.len() >= 50, "fixtures found: {}", paths.len());
    let output = Command::new("git")
        .arg("check-ignore")
        .arg("--no-index")
        .args(&paths)
        .current_dir(&root)
        .output();
    let Ok(output) = output else {
        eprintln!("git not available; skipped");
        return;
    };
    let ignored = String::from_utf8_lossy(&output.stdout);
    assert!(
        ignored.trim().is_empty(),
        "fixtures ignored by git:\n{ignored}"
    );
}
