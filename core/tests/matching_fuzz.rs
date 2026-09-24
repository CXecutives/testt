//! Deterministic fuzzing of the engine: every corpus ad line by line, and seeded mutations
//! of the ads (separators split and joined, brackets, example markers, long digit runs,
//! empty lines, emoji, very long lines), through `compile_profile` + `assess` for every
//! sample profile. No input may panic, and every score stays within 0..=100.
//!
//! Fixed seed: a failure names the input and reproduces on every machine.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use jobalert_core::matching::{CompiledProfile, JobInput, TextKind, assess, compile_profile};
use jobalert_core::portal::Portal;
use serde_json::{Value, json};

const SEED: u64 = 0x5EED_2026_0924;
/// Opens every one-line ad: without 100 characters of text the engine reads no requirements.
const LEAD: &str = "Für ein Transformationsprojekt im Finanzbereich eines Konzerns suchen wir ab sofort eine erfahrene Unterstützung.";
/// Mutated versions of every ad, one per profile (debug build: the file stays below 5 s).
const MUTANTS_PER_AD: usize = 5;

const PROFILES: &[&str] = &[
    "sample_profile.json",
    "sample_profile_it.json",
    "sample_profile_sap.json",
    "sample_profile_senior.json",
    "legacy_edge_profile.json",
];

/// Tokens the requirement splitter cuts at or looks for.
const SEPARATORS: &[&str] = &[
    ", ", " oder ", " und ", "(", ")", "z. B.", " bzw. ", "; ", " / ", "e.g. ",
];
const ODD: &[&str] = &[
    "😀",
    "👩‍💻",
    "İ",
    "ẞ",
    "ß",
    "\u{0301}",
    "\u{200b}",
    "\u{feff}",
    "€",
    "%",
    "\t",
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matching")
}

fn profiles() -> Vec<(&'static str, CompiledProfile)> {
    PROFILES
        .iter()
        .map(|name| {
            let text = std::fs::read_to_string(fixtures().join(name)).expect("profile");
            let value: Value = serde_json::from_str(&text).expect("json");
            (*name, compile_profile(&value))
        })
        .collect()
}

fn corpus() -> Vec<(String, String)> {
    let mut ads: Vec<(String, String)> = std::fs::read_dir(fixtures().join("corpus"))
        .expect("corpus")
        .map(|e| {
            let path = e.expect("entry").path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&path).expect("ad"))
        })
        .collect();
    ads.sort();
    assert!(ads.len() >= 40, "corpus too small: {}", ads.len());
    ads
}

/// A char boundary of `text` near `at`.
fn boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn insert(rng: &mut fastrand::Rng, text: &mut String, what: &str) {
    let at = boundary(text, rng.usize(..=text.len()));
    text.insert_str(at, what);
}

/// One seeded change of `text`.
fn mutate_once(rng: &mut fastrand::Rng, text: &mut String) {
    match rng.u8(..8) {
        // Split: a separator at a random place.
        0 => {
            let sep = SEPARATORS[rng.usize(..SEPARATORS.len())];
            insert(rng, text, sep);
        }
        // Join or swap: one separator becomes another, or disappears.
        1 => {
            let from = SEPARATORS[rng.usize(..SEPARATORS.len())];
            let to = if rng.bool() {
                ""
            } else {
                SEPARATORS[rng.usize(..SEPARATORS.len())]
            };
            if let Some(at) = text.find(from) {
                text.replace_range(at..at + from.len(), to);
            }
        }
        // Long digit runs, alone or as a rate, salary or percentage.
        2 => {
            let digits: String = (0..rng.usize(18..60))
                .map(|_| char::from(b'0' + rng.u8(..10)))
                .collect();
            let tail = [" €/h", " € pro Tag", " EUR Tagessatz", "%", " Jahre", ""][rng.usize(..6)];
            let run = format!(" {digits}{tail} ");
            insert(rng, text, &run);
        }
        // Empty lines.
        3 => {
            let empty = "\n".repeat(rng.usize(2..6));
            insert(rng, text, &empty);
        }
        // Emoji and folding traps.
        4 => {
            let odd = ODD[rng.usize(..ODD.len())];
            insert(rng, text, odd);
        }
        // A very long line: one line of the ad many times over, without line breaks.
        5 => {
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            if !lines.is_empty() {
                let line = lines[rng.usize(..lines.len())].to_owned();
                let long = format!("\n- {}\n", line.repeat(rng.usize(10..40)));
                insert(rng, text, &long);
            }
        }
        // Cut out a piece (at char boundaries).
        6 => {
            let start = boundary(text, rng.usize(..=text.len()));
            let end = boundary(text, start + rng.usize(..200));
            if start < end {
                text.replace_range(start..end, "");
            }
        }
        // A bracket that never closes, or a marker right after one that did.
        _ => {
            let trap = [" (z. B. ", ") z. B. ", "(", ", oder "][rng.usize(..4)];
            insert(rng, text, trap);
        }
    }
}

fn mutate(rng: &mut fastrand::Rng, ad: &str) -> String {
    let mut text = ad.to_owned();
    for _ in 0..rng.usize(1..12) {
        mutate_once(rng, &mut text);
    }
    text
}

/// Assesses `text` and checks the result; a panic or a score beyond 100 fails with the input.
fn check(
    (name, profile): (&str, &CompiledProfile),
    what: &str,
    text: &str,
    facts: Option<&Value>,
    kind: TextKind,
) {
    let job = JobInput {
        title: "Interim Controller (m/w/d)",
        location: "Hamburg",
        portal: Portal::LinkedIn,
        text,
        facts,
        posted: Some(jiff::civil::date(2026, 9, 1)),
        kind,
    };
    let result = catch_unwind(AssertUnwindSafe(|| assess(profile, &job, None)));
    let Ok(assessment) = result else {
        panic!("{what} with {name} panicked; text:\n{text}");
    };
    if let Some(a) = assessment {
        assert!(a.score <= 100, "{what} with {name}: score {}", a.score);
    }
}

/// Lines per ad in the pass where every profile reads every line (as bullets of one ad).
const LINES_PER_AD: usize = 8;

/// Every non-empty corpus line with its origin.
fn corpus_lines() -> Vec<(String, String)> {
    let mut lines = Vec::new();
    for (name, ad) in corpus() {
        for (n, line) in ad.lines().enumerate() {
            if !line.trim().is_empty() {
                lines.push((format!("{name} line {n}"), line.to_owned()));
            }
        }
    }
    assert!(lines.len() > 500, "too few lines: {}", lines.len());
    lines
}

/// Every line alone as the one requirement of an ad (the profiles take turns).
#[test]
fn every_corpus_line_alone_assesses_without_panic() {
    let profiles = profiles();
    for (i, (what, line)) in corpus_lines().iter().enumerate() {
        let (profile, compiled) = &profiles[i % profiles.len()];
        let text = format!("{LEAD}\n\nIhr Profil:\n- {line}\n");
        check((profile, compiled), what, &text, None, TextKind::Full);
    }
}

/// Every line for every profile: the splitter reads each bullet of an ad on its own.
#[test]
fn every_corpus_line_assesses_for_every_profile() {
    let profiles = profiles();
    for chunk in corpus_lines().chunks(LINES_PER_AD) {
        let mut text = format!("{LEAD}\n\nIhr Profil:\n");
        for (_, line) in chunk {
            text.push_str("- ");
            text.push_str(line);
            text.push('\n');
        }
        let what = format!("{} ff.", chunk[0].0);
        for (profile, compiled) in &profiles {
            check((profile, compiled), &what, &text, None, TextKind::Full);
        }
    }
}

#[test]
fn seeded_mutations_assess_without_panic() {
    let profiles = profiles();
    let mut rng = fastrand::Rng::with_seed(SEED);
    for (name, ad) in corpus() {
        // One mutant per profile, with facts now and then (long digit runs too), full text
        // and teaser in turn.
        for m in 0..MUTANTS_PER_AD {
            let text = mutate(&mut rng, &ad);
            let facts = rng.bool().then(|| {
                let rate = if rng.bool() {
                    "9".repeat(rng.usize(18..40)) + " €/h"
                } else {
                    "95 €/h".to_owned()
                };
                let start = ["ab sofort", "01.13.2026", "Q4/2026"][rng.usize(..3)];
                let contract = ["Freiberuflich", "Arbeitnehmerüberlassung", ""][rng.usize(..3)];
                json!({
                    "rate": rate,
                    "start": start,
                    "remotePercent": rng.u32(..200),
                    "contract": contract,
                })
            });
            let (profile, compiled) = &profiles[m % profiles.len()];
            let kind = if m % 2 == 0 {
                TextKind::Full
            } else {
                TextKind::Teaser
            };
            let what = format!("{name} mutant {m}");
            check((profile, compiled), &what, &text, facts.as_ref(), kind);
        }
    }
}
