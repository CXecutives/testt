//! Held-out regression corpora (`core/tests/fixtures/matching/heldout1` to `heldout6`):
//! invented ads with blind labels (grade 0-3, excluded) written by independent agents for
//! profiles the engine was not tuned on at the time. Every set was later used to find and
//! fix systematic gaps, so they are regression gates now, not an unseen measurement.
//!
//! Every set runs the new engine and the old one (`legacy_percent`) over all labelled
//! (profile, job) pairs and computes the metrics of `docs/MATCHING.md` (NDCG@10/20, P@5
//! against what each profile can reach, Spearman over all and over the relevant pairs, the
//! concordance per grade pair, high band, buried grade-3 jobs, exclusion precision and
//! recall). The gates below freeze what the engine reaches;
//! `-- --ignored heldout_report --nocapture` prints the tables.

#[allow(dead_code)]
#[path = "common/eval.rs"]
mod eval;
#[allow(dead_code)]
#[path = "../examples/common/gold.rs"]
mod gold;
#[allow(dead_code)]
#[path = "../examples/common/labels.rs"]
mod labels;
#[allow(dead_code)]
#[path = "../examples/common/metrics.rs"]
mod metrics;

use std::path::{Path, PathBuf};

use jobalert_core::matching::{JobInput, TextKind, Verdict, assess, compile_profile, legacy};
use jobalert_core::portal::Portal;
use metrics::{Metrics, Outcome, Pair};
use serde_json::Value;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matching")
}

/// One labelled pair with the job it belongs to (for the miss lists).
struct Row {
    profile: String,
    job: String,
    pair: Pair,
}

struct SetRun {
    rows: Vec<Row>,
    per_profile: Vec<(String, Metrics)>,
    total: Metrics,
}

/// A profile file of a set: its own folder first, then the sample profiles.
fn profile_path(set: &Path, name: &str) -> PathBuf {
    let own = set.join(name);
    if own.exists() {
        own
    } else {
        fixtures().join(name)
    }
}

fn run_set(name: &str) -> SetRun {
    let dir = fixtures().join(name);
    let jobs = gold::read_jobs(&dir.join("jobs.json")).expect("jobs.json");
    let text = std::fs::read_to_string(dir.join("labels.json")).expect("labels.json");
    let all_labels = labels::parse_labels(&text).expect("labels");
    let mut rows = Vec::new();
    let mut per_profile = Vec::new();
    for (index, (profile_name, own)) in all_labels.iter().enumerate() {
        let path = profile_path(&dir, profile_name);
        let data: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("profile")).expect("json");
        let profile = compile_profile(&data);
        let mut pairs = Vec::new();
        for job in &jobs {
            let Some(label) = labels::for_job(own, job) else {
                continue;
            };
            let content = std::fs::read_to_string(dir.join(job.txt_name())).expect("job file");
            let file = gold::parse_txt(&content);
            let posted = job
                .mail_date
                .as_deref()
                .unwrap_or(&job.first_seen_at)
                .parse::<jiff::Timestamp>()
                .ok()
                .map(|t| t.to_zoned(jiff::tz::TimeZone::UTC).date());
            let input = JobInput {
                title: &job.title,
                company: &job.company,
                location: &job.location,
                portal: Portal::from_key(&job.portal).expect("portal"),
                text: &file.body,
                facts: job.facts.as_ref(),
                posted,
                kind: if job.is_teaser() {
                    TextKind::Teaser
                } else {
                    TextKind::Full
                },
            };
            let a = assess(&profile, &input, None).expect("a usable profile");
            // Debugging aid: `HELDOUT_EXPLAIN="heldout1 sample_profile D11"` prints the
            // reasons of that pair (with `heldout_rows -- --nocapture`).
            let pair_name = format!(
                "{name} {} {}",
                profile_name.trim_end_matches(".json"),
                job.file
            );
            if std::env::var("HELDOUT_EXPLAIN").is_ok_and(|v| v == pair_name) {
                println!("{pair_name} score {}", a.score);
                for r in &a.reasons {
                    let evidence = r.evidence.as_ref().map(|e| e.profile.as_str());
                    println!(
                        "  {:?} {:?} {:?} {:?} <- {evidence:?}",
                        r.kind, r.weight, r.code, r.label
                    );
                }
            }
            let outcome = match a.verdict {
                Verdict::Scored => Outcome::Scored,
                Verdict::Excluded => Outcome::Excluded,
                Verdict::Unscorable => Outcome::Unscorable,
            };
            let old = legacy::parse_job_file(&content)
                .and_then(|file| legacy::legacy_percent(&data, &file.text))
                .map_or(0, |s| s.pct);
            let pair = Pair {
                profile: index,
                new_score: a.score,
                new_rank: a.rank,
                new_outcome: outcome,
                old_score: old,
                grade: label.grade,
                label_excluded: label.excluded,
            };
            rows.push(Row {
                profile: profile_name.clone(),
                job: job.file.clone(),
                pair,
            });
            pairs.push(pair);
        }
        per_profile.push((profile_name.clone(), metrics::metrics(&pairs)));
    }
    let all: Vec<Pair> = rows.iter().map(|r| r.pair).collect();
    SetRun {
        total: metrics::metrics(&all),
        rows,
        per_profile,
    }
}

/// What a set must reach (frozen from the engine; see `docs/MATCHING.md`).
struct Floor {
    ndcg10: f64,
    spearman: f64,
    exclusion_precision: f64,
    exclusion_recall: f64,
    grade3_buried: usize,
}

const HELDOUT1: Floor = Floor {
    ndcg10: 0.92,
    spearman: 0.75,
    exclusion_precision: 1.0,
    exclusion_recall: 1.0,
    grade3_buried: 0,
};
const HELDOUT4: Floor = Floor {
    ndcg10: 0.82,
    spearman: 0.54,
    exclusion_precision: 1.0,
    exclusion_recall: 0.98,
    grade3_buried: 0,
};
const HELDOUT3: Floor = Floor {
    ndcg10: 0.95,
    spearman: 0.49,
    exclusion_precision: 1.0,
    exclusion_recall: 1.0,
    grade3_buried: 0,
};
/// Open decision (the ANÜ wage policy): an hourly wage of temporary agency work
/// (`82 € entspricht dem Bruttostundenlohn`) is read as a day rate x 8, so F03 is excluded
/// for two profiles by the day rate; the labels read it as employment pay (no rate, and a
/// salary per year only for F08). The engine keeps its reading until that is decided.
const HELDOUT6: Floor = Floor {
    ndcg10: 0.90,
    spearman: 0.50,
    exclusion_precision: 0.98,
    exclusion_recall: 0.99,
    grade3_buried: 0,
};
/// Set 5 excludes C10 (a student job) for two profiles by its hourly wage per year, the
/// rule of set 3 (T04); the set-5 labels read a student wage as no salary. The labels of the
/// two sets disagree, the engine keeps the set-3 rule, so the precision floor is below 1.
const HELDOUT5: Floor = Floor {
    ndcg10: 0.81,
    spearman: 0.48,
    exclusion_precision: 0.98,
    exclusion_recall: 1.0,
    grade3_buried: 0,
};
/// Engine 9 moved set 2 from 0.864 to 0.856: a language met is a light fit now, so off-field
/// ads whose only fitting musts are languages (grade 0 and 1 alike) fall below the cap they
/// shared, and Y05 loses its German where `Projekt Management` (written apart) stays open.
const HELDOUT2: Floor = Floor {
    ndcg10: 0.85,
    spearman: 0.64,
    exclusion_precision: 1.0,
    exclusion_recall: 1.0,
    grade3_buried: 1,
};

fn misses(run: &SetRun) -> String {
    let list = |f: &dyn Fn(&Pair) -> bool| -> Vec<String> {
        run.rows
            .iter()
            .filter(|r| f(&r.pair))
            .map(|r| format!("{} {}", r.profile.trim_end_matches(".json"), r.job))
            .collect()
    };
    format!(
        "buried grade 3: {:?}\nwrong exclusions: {:?}\nmissed exclusions: {:?}",
        list(&|p| p.gain() == 3 && p.new_buried()),
        list(&|p| p.new_outcome == Outcome::Excluded && !p.label_excluded),
        list(&|p| p.new_outcome != Outcome::Excluded && p.label_excluded),
    )
}

fn check(name: &str, floor: &Floor) {
    let run = run_set(name);
    let t = &run.total;
    let e = &t.exclusions;
    let report = format!(
        "{name}\n{}\n{}",
        metrics::table(&run.per_profile, t),
        misses(&run)
    );
    assert!(
        e.precision().unwrap_or(1.0) >= floor.exclusion_precision,
        "exclusions the labels do not share:\n{report}"
    );
    assert!(
        e.recall().unwrap_or(1.0) >= floor.exclusion_recall,
        "exclusion recall:\n{report}"
    );
    assert!(t.new.ndcg10 >= floor.ndcg10, "NDCG@10:\n{report}");
    assert!(t.new.spearman >= floor.spearman, "Spearman:\n{report}");
    assert!(
        t.new.grade3_low <= floor.grade3_buried,
        "buried grade-3 jobs:\n{report}"
    );
    assert!(
        t.new.ndcg10 > t.old.ndcg10,
        "the new engine ranks below the old one:\n{report}"
    );
}

#[test]
fn heldout1_holds_its_gates() {
    check("heldout1", &HELDOUT1);
}

#[test]
fn heldout2_holds_its_gates() {
    check("heldout2", &HELDOUT2);
}

#[test]
fn heldout3_holds_its_gates() {
    check("heldout3", &HELDOUT3);
}

#[test]
fn heldout4_holds_its_gates() {
    check("heldout4", &HELDOUT4);
}

#[test]
fn heldout5_holds_its_gates() {
    check("heldout5", &HELDOUT5);
}

#[test]
fn heldout6_holds_its_gates() {
    check("heldout6", &HELDOUT6);
}

/// Every held-out set.
const SETS: [&str; 6] = [
    "heldout1", "heldout2", "heldout3", "heldout4", "heldout5", "heldout6",
];

/// Prints every set's tables and misses (`-- --ignored heldout_report --nocapture`), then
/// one line per set with the numbers of `docs/MATCHING.md`.
#[test]
#[ignore = "report"]
fn heldout_report() {
    let mut lines = Vec::new();
    for name in SETS {
        let run = run_set(name);
        println!("## {name}\n");
        println!("{}", metrics::table(&run.per_profile, &run.total));
        println!("{}", metrics::order_table(&run.per_profile, &run.total));
        println!("{}", metrics::gate_table(&metrics::gates(&run.total)));
        println!("{}\n", misses(&run));
        lines.push(summary_line(name, &run.total));
    }
    println!(
        "| Set | NDCG@10 | P@5 | Spearman relevant | Concordance 2v3 | Concordance 1v2 | \
         Buried | Exclusions P / R |\n|---|---|---|---|---|---|---|---|"
    );
    for line in lines {
        println!("{line}");
    }
}

/// One row of the per-set table of `docs/MATCHING.md`.
fn summary_line(name: &str, t: &Metrics) -> String {
    let three = |v: Option<f64>| v.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.3}"));
    format!(
        "| {name} | {:.3} | {:.3} | {} | {} | {} | {} | {} / {} |",
        t.new.ndcg10,
        t.new.p5,
        three(t.new.spearman_relevant),
        three(t.new.concordance_of(2, 3).share),
        three(t.new.concordance_of(1, 2).share),
        t.new.grade3_low,
        three(t.exclusions.precision()),
        three(t.exclusions.recall()),
    )
}

/// Every pair of both sets (`-- --ignored heldout_rows --nocapture`): score, verdict,
/// grade, label exclusion.
#[test]
#[ignore = "debugging aid"]
fn heldout_rows() {
    for name in SETS {
        for r in run_set(name).rows {
            let p = r.pair;
            println!(
                "{name} {} {} new {} {:?} old {} grade {} excluded {}",
                r.profile.trim_end_matches(".json"),
                r.job,
                p.new_score,
                p.new_outcome,
                p.old_score,
                p.grade,
                p.label_excluded
            );
        }
    }
}
