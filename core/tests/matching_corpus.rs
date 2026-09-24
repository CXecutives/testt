//! The synthetic matching corpus (`core/tests/fixtures/matching`).
//!
//! - Parity: the Rust port of the old engine (`legacy_percent`) equals the frozen outputs
//!   of the real old Python engine (`legacy.json`, `legacy_edge.json`) for every profile
//!   and job.
//! - Hygiene: no fixture is ignored by git (the repository is public, fixtures must ship).
//! - Gates of the new engine: per-job band distance new <= old with a strictly smaller sum,
//!   decided exclusions and status exactly as in `corpus.json`, expected checks raised,
//!   `mustOpen` items never met; a golden digest of all corpus results (same on Windows
//!   and macOS) and the speed budget (release builds). Expectations written for a later
//!   engine (`since`, `profileSince` in `corpus.json`) are gated from that `ENGINE_VERSION` on.

mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use jobalert_core::matching::legacy::{self, LegacyOutcome, legacy_outcome, parse_job_file};
use jobalert_core::matching::{
    Assessment, JobInput, ReasonCode, ReasonKind, TextKind, Verdict, Weight, assess,
    compile_profile,
};
use jobalert_core::portal::Portal;
use serde_json::Value;
use sha2::Digest as _;

/// SHA-256 (16 hex) over every profile x job result of the corpus. Update it only together
/// with `ENGINE_VERSION` and the before/after table in `docs/MATCHING.md`.
const GOLDEN_DIGEST: &str = "8029aabf1b5cecad";

/// SHA-256 (16 hex) over the results of the four profiles without the version-4 keys
/// (`schwerpunkte`, `wunschrollen`, the wishes in `einsatzpraeferenzen`; the IT profile's
/// old `remote` text is taken out) on K01-K52, without the engine version. It stayed at the
/// `ENGINE_VERSION` 3 value `cc7ce7f0ce68f654` while the version-4 inputs were added (with
/// the line `engine 3` in front these rows gave its golden digest `df1d52ce75759f41`):
/// without their keys the new inputs change nothing. The held-out fixes of version 4 move
/// these rows on purpose; each such change updates this value (`docs/MATCHING.md`).
const V3_ROWS_DIGEST: &str = "9dda40ce8b5f1dd8";

/// The profiles of `V3_ROWS_DIGEST` and the jobs it covers.
const V3_PROFILES: &[&str] = &["fin", "it", "senior", "sap"];
const V3_JOBS: usize = 52;

/// Keys of the version-4 inputs (`einsatzpraeferenzen` holds older keys as well).
const V4_KEYS: &[&str] = &["schwerpunkte", "wunschrollen"];
const V4_PREFERENCE_KEYS: &[&str] = &["tagessatz_wunsch", "remote", "regionen", "branchen"];

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
    parity_in(&root.join(frozen), &root.join(jobs))
}

/// Profiles named in the frozen file are read from the fixture folder.
fn parity_in(frozen: &Path, jobs: &Path) -> (usize, Vec<String>) {
    let root = fixtures();
    let frozen = read_json(frozen);
    let mut compared = 0;
    let mut mismatches = Vec::new();
    for (profile_name, entries) in frozen["profiles"].as_object().expect("profiles") {
        let profile = read_json(&root.join(profile_name));
        for (job, expected) in entries.as_object().expect("jobs") {
            let path = jobs.join(format!("{job}.txt"));
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
    assert_eq!(compared, 348, "6 profiles x 58 jobs");
    assert!(
        mismatches.is_empty(),
        "{} of {compared} differ:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

/// Fidelity on local private ads (nothing of them is committed). Freeze the old engine
/// first, then compare:
///
/// ```text
/// py -3.14 tools/eval/legacy_baseline.py run --corpus DIR --profile core/tests/fixtures/matching/sample_profile.json --out OUT.json
/// JOBALERT_FIDELITY_DIR=DIR JOBALERT_FIDELITY_JSON=OUT.json cargo test -p jobalert-core --test matching_corpus -- --ignored local
/// ```
#[test]
#[ignore = "needs local private data"]
fn legacy_port_equals_python_on_local_data() {
    let (Ok(dir), Ok(json)) = (
        std::env::var("JOBALERT_FIDELITY_DIR"),
        std::env::var("JOBALERT_FIDELITY_JSON"),
    ) else {
        panic!("set JOBALERT_FIDELITY_DIR and JOBALERT_FIDELITY_JSON");
    };
    let (compared, mismatches) = parity_in(Path::new(&json), Path::new(&dir));
    println!(
        "local fidelity: {} of {compared} identical",
        compared - mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "{} of {compared} differ:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

/// The new engine on local private ads: no panic, one line per job (old vs new score).
/// `JOBALERT_FIDELITY_DIR=DIR cargo test -p jobalert-core --test matching_corpus -- --ignored new_engine_on_local`
#[test]
#[ignore = "needs local private data"]
fn new_engine_on_local_data() {
    let dir = std::env::var("JOBALERT_FIDELITY_DIR").expect("set JOBALERT_FIDELITY_DIR");
    let root = fixtures();
    let (corpus, _) = corpus();
    for name in corpus["profiles"]
        .as_object()
        .expect("profiles")
        .values()
        .filter_map(Value::as_str)
    {
        let data = read_json(&root.join(name));
        let profile = compile_profile(&data);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("folder")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "txt"))
            .collect();
        files.sort();
        for (index, path) in files.iter().enumerate() {
            let content = std::fs::read_to_string(path).expect("job file");
            let file = parse_any_job_file(&content);
            let input = JobInput {
                title: &file.title,
                company: &file.company,
                location: &file.location,
                portal: Portal::LinkedIn,
                text: &file.text,
                facts: None,
                posted: None,
                kind: TextKind::Full,
            };
            let new = assess(&profile, &input, None);
            let old = legacy::legacy_percent(&data, &file.text).map(|s| s.pct);
            let verdict = new.as_ref().map(|a| code_of_verdict(a.verdict));
            println!(
                "{name} #{index}: old {old:?} new {:?} {verdict:?}",
                new.map(|a| a.score)
            );
        }
    }
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

// --- gates of the new engine ---------------------------------------------------------

/// Expectation of one job for one profile (`corpus.json`).
struct Expect {
    band: (u8, u8),
    status: String,
    violations: Vec<String>,
    checks: Vec<String>,
    must_open: Vec<String>,
}

/// One corpus job with its expectations per profile key (`fin`, `it`, `senior`, `sap`).
struct CorpusJob {
    id: String,
    /// First `ENGINE_VERSION` whose gates check this job.
    since: u32,
    file: legacy::JobFile,
    portal: Portal,
    mail_date: Option<jiff::civil::Date>,
    kind: TextKind,
    facts: Value,
    expect: Vec<(String, Expect)>,
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn corpus() -> (Value, Vec<CorpusJob>) {
    let root = fixtures();
    let corpus = read_json(&root.join("corpus.json"));
    let jobs = corpus["jobs"]
        .as_array()
        .expect("jobs")
        .iter()
        .map(|job| {
            let id = job["id"].as_str().expect("id").to_owned();
            let content = std::fs::read_to_string(root.join("corpus").join(format!("{id}.txt")))
                .expect("job file");
            let file = parse_any_job_file(&content);
            let expect = job["expect"]
                .as_object()
                .expect("expect")
                .iter()
                .map(|(key, e)| {
                    let band = e["band"].as_array().expect("band");
                    let bound =
                        |i: usize| u8::try_from(band[i].as_u64().expect("bound")).expect("0-100");
                    let expect = Expect {
                        band: (bound(0), bound(1)),
                        status: e["status"].as_str().expect("status").to_owned(),
                        violations: strings(&e["violations"]),
                        checks: strings(&e["checks"]),
                        must_open: strings(&e["mustOpen"]),
                    };
                    (key.clone(), expect)
                })
                .collect();
            CorpusJob {
                file,
                portal: Portal::from_key(job["portal"].as_str().expect("portal"))
                    .expect("portal key"),
                mail_date: job["mailDate"].as_str().and_then(|d| d.parse().ok()),
                kind: if job["kind"] == "teaser" {
                    TextKind::Teaser
                } else {
                    TextKind::Full
                },
                facts: job["facts"].clone(),
                expect,
                since: since(&job["since"]),
                id,
            }
        })
        .collect();
    (corpus, jobs)
}

/// A gate version (`since`); missing = the first gated engine.
fn since(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(2)
}

/// Header and body of a TXT file, also for bodies below the old 100-character minimum.
fn parse_any_job_file(content: &str) -> legacy::JobFile {
    let (header, body) = content.split_once("\n\n").unwrap_or((content, ""));
    let field = |name: &str| {
        header
            .lines()
            .find_map(|l| l.strip_prefix(name).map(|v| v.trim().to_owned()))
            .unwrap_or_default()
    };
    legacy::JobFile {
        title: field("Titel:"),
        company: field("Unternehmen:"),
        location: field("Ort:"),
        url: field("Link:"),
        source: field("Quelle:"),
        text: body.trim().to_owned(),
    }
}

/// Score the gates compare: 0 when the engine showed no score.
fn effective(assessment: Option<&Assessment>) -> u8 {
    match assessment {
        Some(a) if a.verdict != Verdict::Unscorable => a.score,
        _ => 0,
    }
}

fn code(code: ReasonCode) -> String {
    serde_json::to_value(code)
        .expect("code")
        .as_str()
        .expect("string")
        .to_owned()
}

/// New and old results per (profile key, job id).
struct Run {
    rows: Vec<Row>,
}

struct Row {
    profile: String,
    job: String,
    expect_band: (u8, u8),
    expect_status: String,
    expect_violations: Vec<String>,
    expect_checks: Vec<String>,
    must_open: Vec<String>,
    new: Option<Assessment>,
    old: u8,
    /// The gates check this row (its expectations are for this engine or an older one).
    gated: bool,
}

/// A profile without the version-4 keys.
fn without_v4_keys(mut profile: Value) -> Value {
    if let Some(map) = profile.as_object_mut() {
        for key in V4_KEYS {
            map.remove(*key);
        }
        if let Some(preferences) = map
            .get_mut("einsatzpraeferenzen")
            .and_then(Value::as_object_mut)
        {
            for key in V4_PREFERENCE_KEYS {
                preferences.remove(*key);
            }
        }
    }
    profile
}

fn run() -> Run {
    run_with(|_, profile| profile)
}

/// The corpus with every profile passed through `prepare` (profile key, profile JSON).
fn run_with(prepare: impl Fn(&str, Value) -> Value) -> Run {
    let root = fixtures();
    let (corpus, jobs) = corpus();
    let legacy = read_json(&root.join("legacy.json"));
    let mut rows = Vec::new();
    for (key, file) in corpus["profiles"].as_object().expect("profiles") {
        let file = file.as_str().expect("profile file");
        let profile_since = since(&corpus["profileSince"][key]);
        let data = prepare(key, read_json(&root.join(file)));
        let profile = compile_profile(&data);
        for job in &jobs {
            let input = JobInput {
                title: &job.file.title,
                company: &job.file.company,
                location: &job.file.location,
                portal: job.portal,
                text: &job.file.text,
                facts: (!job.facts.is_null()).then_some(&job.facts),
                posted: job.mail_date,
                kind: job.kind,
            };
            let new = assess(&profile, &input, None);
            let old = legacy["profiles"][file][&job.id]["pct"]
                .as_u64()
                .unwrap_or(0);
            let (_, expect) = job
                .expect
                .iter()
                .find(|(k, _)| k == key)
                .expect("expectation");
            rows.push(Row {
                profile: key.clone(),
                job: job.id.clone(),
                expect_band: expect.band,
                expect_status: expect.status.clone(),
                expect_violations: expect.violations.clone(),
                expect_checks: expect.checks.clone(),
                must_open: expect.must_open.clone(),
                new,
                old: u8::try_from(old).unwrap_or(0),
                gated: profile_since.max(job.since) <= jobalert_core::matching::ENGINE_VERSION,
            });
        }
    }
    Run { rows }
}

#[test]
fn new_engine_is_closer_to_the_bands_than_the_old_one() {
    let run = run();
    let mut worse = Vec::new();
    let (mut sum_new, mut sum_old) = (0u32, 0u32);
    for row in run.rows.iter().filter(|r| r.gated) {
        let new = common::eval::band_distance(effective(row.new.as_ref()), row.expect_band);
        let old = common::eval::band_distance(row.old, row.expect_band);
        sum_new += u32::from(new);
        sum_old += u32::from(old);
        if new > old {
            worse.push(format!(
                "{} {}: new {new} > old {old}",
                row.profile, row.job
            ));
        }
    }
    assert!(
        worse.is_empty(),
        "jobs further from their band than before:\n{}",
        worse.join("\n")
    );
    assert!(
        sum_new < sum_old,
        "total band distance new {sum_new} must be below old {sum_old}"
    );
}

#[test]
fn decided_exclusions_are_exactly_the_expected_ones() {
    let run = run();
    let mut wrong = Vec::new();
    for row in run.rows.iter().filter(|r| r.gated) {
        let Some(new) = &row.new else { continue };
        let mut actual: Vec<String> = new
            .reasons
            .iter()
            .filter(|r| r.kind == ReasonKind::Violation)
            .map(|r| code(r.code))
            .collect();
        actual.sort();
        let mut expected = row.expect_violations.clone();
        expected.sort();
        let status = code_of_verdict(new.verdict);
        if actual != expected || status != row.expect_status {
            wrong.push(format!(
                "{} {}: {status} {actual:?}, expected {} {expected:?}",
                row.profile, row.job, row.expect_status
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "decided exclusions differ:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn checks_and_open_musts_match_the_expectations() {
    let run = run();
    let mut wrong = Vec::new();
    for row in run.rows.iter().filter(|r| r.gated) {
        let Some(new) = &row.new else { continue };
        let checks: Vec<String> = new
            .reasons
            .iter()
            .filter(|r| r.kind == ReasonKind::Check)
            .map(|r| code(r.code))
            .collect();
        for expected in &row.expect_checks {
            if !checks.contains(expected) {
                wrong.push(format!(
                    "{} {}: check {expected} missing",
                    row.profile, row.job
                ));
            }
        }
        for open in &row.must_open {
            let needle = open.to_lowercase();
            let met = new.reasons.iter().any(|r| {
                r.kind == ReasonKind::Met
                    && r.weight == Weight::Must
                    && r.label
                        .as_deref()
                        .is_some_and(|l| l.to_lowercase().contains(&needle))
            });
            if met {
                wrong.push(format!(
                    "{} {}: '{open}' counted as met",
                    row.profile, row.job
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// One line of a digest: verdict, score, must counts, highlights and every reason.
fn digest_line(canonical: &mut String, row: &Row) {
    let Some(a) = &row.new else {
        let _ = writeln!(canonical, "{} {} none", row.profile, row.job);
        return;
    };
    let codes: Vec<String> = a
        .reasons
        .iter()
        .map(|r| format!("{}:{}", code(r.code), code_of_kind(r.kind)))
        .collect();
    let _ = writeln!(
        canonical,
        "{} {} {} {} {}/{}/{} {}",
        row.profile,
        row.job,
        code_of_verdict(a.verdict),
        a.score,
        a.summary.must_met,
        a.summary.must_total,
        a.highlights.len(),
        codes.join(",")
    );
}

/// The first 16 hex characters of the SHA-256 of `canonical`.
fn hex16(canonical: &str) -> String {
    let digest = sha2::Sha256::digest(canonical.as_bytes());
    digest.iter().take(8).fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[test]
fn golden_digest_of_all_corpus_results() {
    let run = run();
    let mut canonical = format!("engine {}\n", jobalert_core::matching::ENGINE_VERSION);
    for row in &run.rows {
        digest_line(&mut canonical, row);
    }
    let hex = hex16(&canonical);
    assert_eq!(hex, GOLDEN_DIGEST, "corpus results changed:\n{canonical}");
}

/// Profiles without the version-4 keys: every new input is off while its key is missing, so
/// only deliberate engine changes move their rows (see `V3_ROWS_DIGEST`).
#[test]
fn profiles_without_the_new_keys_score_as_before() {
    let run = run_with(|key, profile| {
        if V3_PROFILES.contains(&key) {
            without_v4_keys(profile)
        } else {
            profile
        }
    });
    let jobs: Vec<String> = (1..=V3_JOBS).map(|n| format!("K{n:02}")).collect();
    let mut canonical = String::new();
    for row in run
        .rows
        .iter()
        .filter(|r| V3_PROFILES.contains(&r.profile.as_str()) && jobs.contains(&r.job))
    {
        digest_line(&mut canonical, row);
    }
    assert_eq!(
        canonical.lines().count(),
        V3_PROFILES.len() * V3_JOBS,
        "rows"
    );
    let hex = hex16(&canonical);
    assert_eq!(hex, V3_ROWS_DIGEST, "old profiles changed:\n{canonical}");
}

/// The target role and the wishes never lift a job into the high band (80) while fewer
/// than half of its musts are met: every corpus row against the same profile without them.
#[test]
fn wishes_never_lift_a_weak_job_into_the_high_band() {
    let with = run();
    let without = run_with(|_, mut profile| {
        if let Some(map) = profile.as_object_mut() {
            map.remove("wunschrollen");
            if let Some(preferences) = map
                .get_mut("einsatzpraeferenzen")
                .and_then(Value::as_object_mut)
            {
                for key in V4_PREFERENCE_KEYS {
                    preferences.remove(*key);
                }
            }
        }
        profile
    });
    let mut lifted = 0;
    let mut failures = Vec::new();
    for (row, base) in with.rows.iter().zip(&without.rows) {
        assert_eq!((&row.profile, &row.job), (&base.profile, &base.job));
        let (Some(new), Some(old)) = (&row.new, &base.new) else {
            continue;
        };
        if new.verdict != Verdict::Scored || new.score <= old.score {
            continue;
        }
        lifted += 1;
        let s = &new.summary;
        if new.score >= 80 && old.score < 80 && 2 * s.must_met < s.must_total {
            failures.push(format!(
                "{} {}: {} -> {} with {} of {} musts",
                row.profile, row.job, old.score, new.score, s.must_met, s.must_total
            ));
        }
    }
    assert!(lifted > 0, "no row is lifted: the test checks nothing");
    assert!(failures.is_empty(), "{failures:#?}");
}

fn code_of_kind(kind: ReasonKind) -> String {
    serde_json::to_value(kind)
        .expect("kind")
        .as_str()
        .expect("string")
        .to_owned()
}

/// Median <= 0.5 ms per job and 2000 jobs <= 3 s (engine part; measured in release builds).
#[test]
fn engine_is_fast_enough() {
    if cfg!(debug_assertions) {
        eprintln!("speed budget is checked in release builds only");
        return;
    }
    let root = fixtures();
    let (_, jobs) = corpus();
    let profile = compile_profile(&read_json(&root.join("sample_profile.json")));
    let mut times = Vec::with_capacity(2000);
    let started = Instant::now();
    for i in 0..2000 {
        let job = &jobs[i % jobs.len()];
        let input = JobInput {
            title: &job.file.title,
            company: &job.file.company,
            location: &job.file.location,
            portal: job.portal,
            text: &job.file.text,
            facts: (!job.facts.is_null()).then_some(&job.facts),
            posted: job.mail_date,
            kind: job.kind,
        };
        let one = Instant::now();
        let _ = assess(&profile, &input, None);
        times.push(one.elapsed());
    }
    let total = started.elapsed();
    times.sort();
    let median = times[times.len() / 2];
    assert!(median.as_micros() <= 500, "median {median:?} per job");
    assert!(total.as_millis() <= 3000, "2000 jobs took {total:?}");
}

/// Prints the before/after table of the corpus (`-- --ignored report --nocapture`).
#[test]
#[ignore = "report"]
fn report() {
    let run = run();
    println!("profile job band old new status/expected findings");
    for row in &run.rows {
        let new = row.new.as_ref();
        let status = new.map_or("-".to_owned(), |a| code_of_verdict(a.verdict));
        let findings: Vec<String> = new
            .map(|a| {
                a.reasons
                    .iter()
                    .filter(|r| matches!(r.kind, ReasonKind::Violation | ReasonKind::Check))
                    .map(|r| code(r.code))
                    .collect()
            })
            .unwrap_or_default();
        let (lo, hi) = row.expect_band;
        let score = effective(new);
        let d_new = common::eval::band_distance(score, row.expect_band);
        let d_old = common::eval::band_distance(row.old, row.expect_band);
        let flag = if !row.gated {
            " (not gated yet)"
        } else if d_new > d_old {
            " WORSE"
        } else if d_new > 0 {
            " off"
        } else {
            ""
        };
        println!(
            "{} {} [{lo},{hi}] old {} new {score} {status}/{} {findings:?} exp {:?}{flag}",
            row.profile, row.job, row.old, row.expect_status, row.expect_checks
        );
    }
}

/// Prints every reason of corpus rows (`profile:job`, comma-separated):
/// `JOBALERT_ROWS=fin:K01,sap:K12 cargo test -p jobalert-core --test matching_corpus -- --ignored explain_rows --nocapture`
#[test]
#[ignore = "debugging aid"]
fn explain_rows() {
    let wanted = std::env::var("JOBALERT_ROWS").expect("set JOBALERT_ROWS");
    let wanted: Vec<(&str, &str)> = wanted
        .split(',')
        .filter_map(|pair| pair.split_once(':'))
        .collect();
    let run = run();
    for row in run
        .rows
        .iter()
        .filter(|r| wanted.iter().any(|(p, j)| *p == r.profile && *j == r.job))
    {
        let Some(a) = &row.new else { continue };
        println!(
            "{} {} score {} {:?}",
            row.profile, row.job, a.score, a.verdict
        );
        for r in &a.reasons {
            let evidence = r
                .evidence
                .as_ref()
                .map(|e| format!("<- {} ({:?}, {})", e.profile, e.via, e.path))
                .unwrap_or_default();
            println!(
                "  {:?} {:?} {} {:?} {} {evidence}",
                r.kind,
                r.weight,
                code(r.code),
                r.label.as_deref().unwrap_or(""),
                Value::Object(r.params.clone())
            );
        }
    }
}

fn code_of_verdict(verdict: Verdict) -> String {
    serde_json::to_value(verdict)
        .expect("verdict")
        .as_str()
        .expect("string")
        .to_owned()
}
