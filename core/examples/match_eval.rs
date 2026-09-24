//! Evaluate the new and the old engine against the blind labels of the private gold set:
//!
//! ```text
//! cargo run -p jobalert-core --example match_eval -- [--gold DIR] [--labels FILE] [--profile FILE]...
//! ```
//!
//! Reads `gold/jobs.json`, the TXT files (from `export_gold`) and `gold/labels.json`, scores
//! every job with the new engine (`compile_profile` + `assess`: title, raw location, portal,
//! text, page facts, teaser kind, mail date) and the old one (`legacy::legacy_percent`) for
//! every profile (`core/tests/fixtures/matching/sample_profile*.json` or `--profile`), and
//! reports per profile and in total: NDCG@10/@20, P@5, Spearman, high-band precision,
//! grade-3 jobs buried, exclusion precision/recall and a paired bootstrap of NDCG@10
//! new - old (`examples/common/metrics.rs`). Prints a Markdown table (numbers only, never
//! ad text) and writes `gold/report.md` (private; it also lists job keys of misses).
//!
//! Exit code 1 when a gate of `docs/PLAN.md` fails with at least 60 labelled jobs; below
//! that the result is "preliminary" and the exit code 0.

mod common;

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use common::gold::{self, GoldJob};
use common::labels::{self, Labels};
use common::metrics::{self, MIN_LABELLED_JOBS, Metrics, Outcome, Pair, Status};
use jobalert_core::matching::{self, JobInput, TextKind, Verdict, legacy};
use jobalert_core::model::is_usable_title;
use jobalert_core::portal::Portal;
use jobalert_core::time::local_date;
use jobalert_core::view::slug_title;
use serde_json::Value;

type Res<T> = Result<T, Box<dyn Error>>;

struct Args {
    gold: Option<PathBuf>,
    labels: Option<PathBuf>,
    profiles: Vec<PathBuf>,
}

fn parse_args() -> Res<Args> {
    let mut args = Args {
        gold: None,
        labels: None,
        profiles: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .map(PathBuf::from)
                .ok_or(format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--gold" => args.gold = Some(value()?),
            "--labels" => args.labels = Some(value()?),
            "--profile" => args.profiles.push(value()?),
            "--help" | "-h" => {
                println!(
                    "match_eval [--gold DIR] [--labels FILE] [--profile FILE]...\n\
                     Default gold folder: {}",
                    gold::GOLD_DIR
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other} (see --help)").into()),
        }
    }
    Ok(args)
}

/// A gold job ready for both engines.
struct Loaded {
    job: GoldJob,
    portal: Portal,
    title: String,
    /// The TXT file as written (the old engine reads the whole file).
    content: String,
    body: String,
    posted: Option<jiff::civil::Date>,
}

fn load(dir: &Path, jobs: Vec<GoldJob>) -> (Vec<Loaded>, usize) {
    let mut loaded = Vec::new();
    let mut skipped = 0;
    for job in jobs {
        let (Some(portal), Ok(content)) = (
            Portal::from_key(&job.portal),
            std::fs::read_to_string(dir.join(job.txt_name())),
        ) else {
            skipped += 1;
            continue;
        };
        // The same title as the app's matcher (`pipeline::LocalMatcher`).
        let title = if is_usable_title(&job.title) {
            job.title.clone()
        } else {
            slug_title(&job.url).unwrap_or_default()
        };
        let seen = job
            .mail_date
            .as_deref()
            .unwrap_or(&job.first_seen_at)
            .parse::<jiff::Timestamp>()
            .ok();
        let body = gold::parse_txt(&content).body;
        loaded.push(Loaded {
            posted: seen.map(local_date),
            job,
            portal,
            title,
            content,
            body,
        });
    }
    (loaded, skipped)
}

/// Counts of one profile over all jobs, labelled or not.
#[derive(Default)]
struct Counts {
    scored: usize,
    excluded: usize,
    unscorable: usize,
    high: usize,
    old_absent: usize,
    labelled: usize,
    unknown_labels: usize,
}

/// Job keys behind the misses of one profile (for the private report only).
#[derive(Default)]
struct Misses {
    grade3_buried: Vec<String>,
    wrong_exclusions: Vec<String>,
    missed_exclusions: Vec<String>,
}

struct ProfileRun {
    name: String,
    counts: Counts,
    pairs: Vec<Pair>,
    misses: Misses,
}

fn run_profile(
    index: usize,
    path: &Path,
    jobs: &[Loaded],
    labels: &Labels,
) -> Res<Option<ProfileRun>> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let data: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let profile = matching::compile_profile(&data);
    let name = gold::profile_key(path);
    let own = labels::for_profile(labels, &name);
    let mut counts = Counts {
        unknown_labels: own.map_or(0, |l| {
            let all: Vec<GoldJob> = jobs.iter().map(|j| j.job.clone()).collect();
            labels::unknown_jobs(l, &all)
        }),
        ..Counts::default()
    };
    let mut pairs = Vec::new();
    let mut misses = Misses::default();
    for job in jobs {
        let input = JobInput {
            title: &job.title,
            location: &job.job.location,
            portal: job.portal,
            text: &job.body,
            facts: job.job.facts.as_ref(),
            posted: job.posted,
            kind: if job.job.is_teaser() {
                TextKind::Teaser
            } else {
                TextKind::Full
            },
        };
        let Some(assessment) = matching::assess(&profile, &input, None) else {
            println!("{name}: the profile names no competences; skipped");
            return Ok(None);
        };
        let outcome = match assessment.verdict {
            Verdict::Scored => Outcome::Scored,
            Verdict::Excluded => Outcome::Excluded,
            Verdict::Unscorable => Outcome::Unscorable,
        };
        match outcome {
            Outcome::Scored => counts.scored += 1,
            Outcome::Excluded => counts.excluded += 1,
            Outcome::Unscorable => counts.unscorable += 1,
        }
        if outcome == Outcome::Scored && assessment.score >= 80 {
            counts.high += 1;
        }
        let old = legacy::parse_job_file(&job.content)
            .and_then(|file| legacy::legacy_percent(&data, &file.text))
            .map(|s| s.pct);
        if old.is_none() {
            counts.old_absent += 1;
        }
        let Some(label) = own.and_then(|l| labels::for_job(l, &job.job)) else {
            continue;
        };
        counts.labelled += 1;
        let pair = Pair {
            profile: index,
            new_score: assessment.score,
            new_outcome: outcome,
            old_score: old.unwrap_or(0),
            grade: label.grade,
            label_excluded: label.excluded,
        };
        let key = &job.job.file;
        if pair.gain() == 3 && pair.new_buried() {
            misses.grade3_buried.push(key.clone());
        }
        match (outcome == Outcome::Excluded, label.excluded) {
            (true, false) => misses.wrong_exclusions.push(key.clone()),
            (false, true) => misses.missed_exclusions.push(key.clone()),
            _ => {}
        }
        pairs.push(pair);
    }
    Ok(Some(ProfileRun {
        name,
        counts,
        pairs,
        misses,
    }))
}

fn counts_table(runs: &[ProfileRun]) -> String {
    let mut out = String::from(
        "| Profile | Scored | Excluded | Unscorable | High band (new) | Old absent | Labelled | Unknown labels |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for run in runs {
        let c = &run.counts;
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            run.name,
            c.scored,
            c.excluded,
            c.unscorable,
            c.high,
            c.old_absent,
            c.labelled,
            c.unknown_labels
        );
    }
    out
}

fn misses_section(runs: &[ProfileRun]) -> String {
    let mut out = String::from("## Misses (job keys)\n\n");
    let list = |keys: &[String]| {
        if keys.is_empty() {
            "-".to_owned()
        } else {
            keys.join(", ")
        }
    };
    for run in runs {
        let m = &run.misses;
        let _ = writeln!(
            out,
            "- {}: grade 3 buried: {}; wrong exclusions: {}; missed exclusions: {}",
            run.name,
            list(&m.grade3_buried),
            list(&m.wrong_exclusions),
            list(&m.missed_exclusions)
        );
    }
    out
}

fn main() -> Res<ExitCode> {
    let args = parse_args()?;
    let dir = args.gold.unwrap_or_else(gold::gold_dir);
    let jobs_path = dir.join(gold::JOBS_FILE);
    if !jobs_path.is_file() {
        return Err(format!(
            "no {} - run `cargo run -p jobalert-core --example export_gold` first",
            jobs_path.display()
        )
        .into());
    }
    let (jobs, skipped) = load(&dir, gold::read_jobs(&jobs_path)?);
    let labels_path = args.labels.unwrap_or_else(|| dir.join(gold::LABELS_FILE));
    let labels = if labels_path.is_file() {
        labels::parse_labels(&std::fs::read_to_string(&labels_path)?)?
    } else {
        Labels::new()
    };
    let profiles = if args.profiles.is_empty() {
        gold::default_profiles()?
    } else {
        args.profiles
    };

    let mut runs = Vec::new();
    for (index, path) in profiles.iter().enumerate() {
        if let Some(run) = run_profile(index, path, &jobs, &labels)? {
            runs.push(run);
        }
    }

    let all_pairs: Vec<Pair> = runs.iter().flat_map(|r| r.pairs.clone()).collect();
    let labelled_jobs: BTreeSet<&str> = runs
        .iter()
        .flat_map(|run| {
            jobs.iter()
                .filter(|j| {
                    labels::for_profile(&labels, &run.name)
                        .and_then(|l| labels::for_job(l, &j.job))
                        .is_some()
                })
                .map(|j| j.job.file.as_str())
        })
        .collect();
    let rows: Vec<(String, Metrics)> = runs
        .iter()
        .filter(|r| !r.pairs.is_empty())
        .map(|r| (r.name.clone(), metrics::metrics(&r.pairs)))
        .collect();
    let total = metrics::metrics(&all_pairs);
    let gates = metrics::gates(&total);
    let status = metrics::status(labelled_jobs.len(), &gates);

    let mut summary = format!(
        "# Matching on the private gold set\n\n{} jobs ({} skipped: unknown portal or missing TXT), \
         {} profiles, {} labelled jobs, {} labelled pairs. Engine version {}.\n\n",
        jobs.len(),
        skipped,
        runs.len(),
        labelled_jobs.len(),
        all_pairs.len(),
        matching::ENGINE_VERSION,
    );
    summary.push_str(&counts_table(&runs));
    let verdict = match status {
        Status::NoLabels => format!("No labels yet ({}).", labels_path.display()),
        Status::Preliminary => format!(
            "Preliminary: {} labelled jobs < {MIN_LABELLED_JOBS}, gates not enforced.",
            labelled_jobs.len()
        ),
        Status::Pass => "All gates pass.".to_owned(),
        Status::Fail => "At least one gate FAILS.".to_owned(),
    };
    if status != Status::NoLabels {
        summary.push_str(
            "\nNew / old per cell. NDCG and P@5: mean over profiles; Spearman, high band and \
             buried grade-3 jobs pooled. Gain 2^grade - 1, grade 0 when the labelers excluded \
             the job; the new engine ranks excluded jobs last. Bootstrap: ",
        );
        let _ = writeln!(
            summary,
            "{} rounds, seed {}.\n",
            metrics::BOOTSTRAP_ROUNDS,
            metrics::BOOTSTRAP_SEED
        );
        summary.push_str(&metrics::table(&rows, &total));
        summary.push('\n');
        summary.push_str(&metrics::gate_table(&gates));
    }
    let _ = writeln!(summary, "\n**{verdict}**");
    println!("{summary}");

    let report = dir.join(gold::REPORT_FILE);
    std::fs::write(&report, format!("{summary}\n{}", misses_section(&runs)))?;
    println!("Report: {}", report.display());
    Ok(if status == Status::Fail {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
