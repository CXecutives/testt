//! Metric wiring of `match_eval`: one [`Pair`] per labelled (profile, job), the ranking
//! metrics of `tests/common/eval.rs` for the new and the old engine, exclusion precision and
//! recall, the paired bootstrap of NDCG@10 new - old, and the private gates of `docs/PLAN.md`.
//!
//! Conventions: the gain of a job is `2^grade - 1` with grade 0 when the labelers excluded it
//! (nobody applies to an excluded job); the new engine ranks excluded jobs last and
//! unscorable ones just above them, as the app's list does; the old engine had no exclusion.
#![allow(clippy::cast_precision_loss)] // Counts are tiny; these are report numbers.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::eval::{self, Interval};

/// A job counts as relevant for P@5 and the high band from this grade on.
pub const RELEVANT: u8 = 2;
/// Scores below this bury a job (`no grade-3 job below 40`).
pub const LOW_BELOW: u8 = 40;
pub const BOOTSTRAP_ROUNDS: usize = 2000;
pub const BOOTSTRAP_SEED: u64 = 20_260_924;
/// Below this many labelled jobs the gates are reported but not enforced ("preliminary").
pub const MIN_LABELLED_JOBS: usize = 60;

/// The new engine's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Scored,
    Excluded,
    Unscorable,
}

/// One labelled job for one profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    /// Index of the profile (groups the per-profile rankings).
    pub profile: usize,
    pub new_score: u8,
    /// The new engine's per-mille score before caps: breaks ties of equal scores.
    pub new_rank: u16,
    pub new_outcome: Outcome,
    /// Old engine's percentage; 0 when it showed no row.
    pub old_score: u8,
    pub grade: u8,
    pub label_excluded: bool,
}

impl Pair {
    /// Grade used for the gain: 0 for a job the labelers excluded.
    pub fn gain(&self) -> u8 {
        if self.label_excluded { 0 } else { self.grade }
    }

    /// Ranking key of the new engine: excluded last, unscorable just above them; equal
    /// scores follow the score before the caps, as in the app's list.
    pub fn new_key(&self) -> f64 {
        let score = f64::from(self.new_score) + f64::from(self.new_rank) / 10_000.0;
        match self.new_outcome {
            Outcome::Scored => score,
            Outcome::Unscorable => -1.0,
            Outcome::Excluded => score - 1000.0,
        }
    }

    pub fn old_key(&self) -> f64 {
        f64::from(self.old_score)
    }

    /// The new engine hides the job: excluded, unscorable or below 40.
    pub fn new_buried(&self) -> bool {
        self.new_outcome != Outcome::Scored || self.new_score < LOW_BELOW
    }

    pub fn old_buried(&self) -> bool {
        self.old_score < LOW_BELOW
    }
}

/// Ranking quality of one engine.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ranking {
    pub ndcg10: f64,
    pub ndcg20: f64,
    pub p5: f64,
    pub spearman: f64,
    /// Share of relevant jobs among those scored 80 or more; `None` when there are none.
    pub high_band: Option<f64>,
    /// Grade-3 jobs the engine buries.
    pub grade3_low: usize,
}

/// Exclusions of the new engine against the labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Exclusions {
    /// Excluded by both.
    pub hit: usize,
    /// Excluded by the engine only.
    pub wrong: usize,
    /// Excluded by the labelers only.
    pub missed: usize,
}

impl Exclusions {
    pub fn of(pairs: &[Pair]) -> Exclusions {
        let mut e = Exclusions::default();
        for p in pairs {
            match (p.new_outcome == Outcome::Excluded, p.label_excluded) {
                (true, true) => e.hit += 1,
                (true, false) => e.wrong += 1,
                (false, true) => e.missed += 1,
                (false, false) => {}
            }
        }
        e
    }

    pub fn precision(&self) -> Option<f64> {
        let decided = self.hit + self.wrong;
        (decided > 0).then(|| self.hit as f64 / decided as f64)
    }

    pub fn recall(&self) -> Option<f64> {
        let labelled = self.hit + self.missed;
        (labelled > 0).then(|| self.hit as f64 / labelled as f64)
    }
}

/// All numbers of one profile (or of the total).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub n: usize,
    pub new: Ranking,
    pub old: Ranking,
    pub exclusions: Exclusions,
    /// NDCG@10 new - old with its 95 % interval; `None` without pairs.
    pub delta_ndcg10: Option<Interval>,
}

fn grades(pairs: &[Pair]) -> Vec<u8> {
    pairs.iter().map(Pair::gain).collect()
}

fn keys(pairs: &[Pair], key: fn(&Pair) -> f64) -> Vec<f64> {
    pairs.iter().map(key).collect()
}

/// NDCG@k of one profile's pairs.
fn ndcg(pairs: &[Pair], key: fn(&Pair) -> f64, k: usize) -> f64 {
    eval::ndcg_at(&keys(pairs, key), &grades(pairs), k)
}

/// Pairs grouped by profile, in profile order.
fn by_profile(pairs: &[Pair]) -> Vec<Vec<Pair>> {
    let mut groups: BTreeMap<usize, Vec<Pair>> = BTreeMap::new();
    for p in pairs {
        groups.entry(p.profile).or_default().push(*p);
    }
    groups.into_values().collect()
}

/// Mean over the profiles that have at least one job with a gain: a profile without a
/// relevant job has no ranking to judge (its NDCG would be 0 whatever the engine does).
fn ranked_mean(pairs: &[Pair], metric: impl Fn(&[Pair]) -> f64) -> f64 {
    let groups: Vec<Vec<Pair>> = by_profile(pairs)
        .into_iter()
        .filter(|g| g.iter().any(|p| p.gain() > 0))
        .collect();
    if groups.is_empty() {
        return 0.0;
    }
    groups.iter().map(|g| metric(g)).sum::<f64>() / groups.len() as f64
}

/// P@5 of the profiles that have relevant jobs, each divided by what it can reach
/// (`min(5, relevant jobs)`: a profile with two relevant jobs scores 1.0 when both are in its
/// top 5); 0 when no profile has a relevant job.
fn reachable_p5(pairs: &[Pair], key: fn(&Pair) -> f64) -> f64 {
    let values: Vec<f64> = by_profile(pairs)
        .iter()
        .filter_map(|g| eval::reachable_precision_at(&keys(g, key), &grades(g), 5, RELEVANT))
        .collect();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Ranking metrics of one engine. NDCG and P@5 are means over the profiles (one ranking per
/// profile; P@5 over the profiles with relevant jobs, against what each can reach);
/// Spearman, the high band and the grade-3 count are pooled over all pairs.
pub fn ranking(pairs: &[Pair], key: fn(&Pair) -> f64, buried: fn(&Pair) -> bool) -> Ranking {
    let scores = keys(pairs, key);
    let gains = grades(pairs);
    let gains_f: Vec<f64> = gains.iter().map(|&g| f64::from(g)).collect();
    Ranking {
        ndcg10: ranked_mean(pairs, |g| ndcg(g, key, 10)),
        ndcg20: ranked_mean(pairs, |g| ndcg(g, key, 20)),
        p5: reachable_p5(pairs, key),
        spearman: if pairs.is_empty() {
            0.0
        } else {
            eval::spearman(&scores, &gains_f)
        },
        high_band: eval::high_band_precision(&scores, &gains, RELEVANT),
        grade3_low: pairs.iter().filter(|p| p.gain() == 3 && buried(p)).count(),
    }
}

/// NDCG@10 new - old of a resample, as a mean over the profiles in it.
fn delta_ndcg10(pairs: &[Pair]) -> f64 {
    ranked_mean(pairs, |g| {
        ndcg(g, Pair::new_key, 10) - ndcg(g, Pair::old_key, 10)
    })
}

/// Everything for a set of pairs (one profile, or all profiles for the total).
pub fn metrics(pairs: &[Pair]) -> Metrics {
    let delta_ndcg10 = (!pairs.is_empty()).then(|| {
        eval::paired_bootstrap(pairs.len(), BOOTSTRAP_ROUNDS, BOOTSTRAP_SEED, |sample| {
            let resample: Vec<Pair> = sample.iter().map(|&i| pairs[i]).collect();
            delta_ndcg10(&resample)
        })
    });
    Metrics {
        n: pairs.len(),
        new: ranking(pairs, Pair::new_key, Pair::new_buried),
        old: ranking(pairs, Pair::old_key, Pair::old_buried),
        exclusions: Exclusions::of(pairs),
        delta_ndcg10,
    }
}

/// One private gate of `docs/PLAN.md`, checked on the total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    pub name: &'static str,
    pub value: String,
    pub target: &'static str,
    pub pass: bool,
}

fn optional(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.3}"))
}

/// The gates; a metric without data (`n/a`) does not fail.
pub fn gates(total: &Metrics) -> Vec<Gate> {
    const EPS: f64 = 1e-9;
    let (new, old, e) = (&total.new, &total.old, &total.exclusions);
    let gate = |name, value: String, target, pass| Gate {
        name,
        value,
        target,
        pass,
    };
    vec![
        gate(
            "Exclusion precision",
            optional(e.precision()),
            "1.000",
            e.precision().is_none_or(|p| p >= 1.0 - EPS),
        ),
        gate(
            "Exclusion recall",
            optional(e.recall()),
            ">= 0.800",
            e.recall().is_none_or(|r| r >= 0.8 - EPS),
        ),
        gate(
            "NDCG@10",
            format!("{:.3}", new.ndcg10),
            ">= 0.750",
            new.ndcg10 >= 0.75 - EPS,
        ),
        gate(
            "NDCG@10 new - old",
            format!("{:+.3}", new.ndcg10 - old.ndcg10),
            ">= +0.100",
            new.ndcg10 - old.ndcg10 >= 0.10 - EPS,
        ),
        gate(
            "P@5",
            format!("{:.3}", new.p5),
            ">= 0.800",
            new.p5 >= 0.8 - EPS,
        ),
        gate(
            "P@5 new - old",
            format!("{:+.3}", new.p5 - old.p5),
            ">= 0",
            new.p5 >= old.p5 - EPS,
        ),
        gate(
            "Grade-3 jobs buried (< 40, excluded or unscorable)",
            new.grade3_low.to_string(),
            "0",
            new.grade3_low == 0,
        ),
        gate(
            "High-band precision",
            optional(new.high_band),
            ">= 0.800",
            new.high_band.is_none_or(|h| h >= 0.8 - EPS),
        ),
        gate(
            "Spearman",
            format!("{:.3}", new.spearman),
            ">= 0.550",
            new.spearman >= 0.55 - EPS,
        ),
        gate(
            "Spearman new - old",
            format!("{:+.3}", new.spearman - old.spearman),
            "> 0",
            new.spearman > old.spearman + EPS,
        ),
    ]
}

/// Overall result of an evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NoLabels,
    /// Fewer than [`MIN_LABELLED_JOBS`] labelled jobs: gates shown, not enforced.
    Preliminary,
    Pass,
    Fail,
}

pub fn status(labelled_jobs: usize, gates: &[Gate]) -> Status {
    if labelled_jobs == 0 {
        Status::NoLabels
    } else if labelled_jobs < MIN_LABELLED_JOBS {
        Status::Preliminary
    } else if gates.iter().all(|g| g.pass) {
        Status::Pass
    } else {
        Status::Fail
    }
}

fn pair_of(new: f64, old: f64) -> String {
    format!("{new:.3} / {old:.3}")
}

fn optional_pair(new: Option<f64>, old: Option<f64>) -> String {
    format!("{} / {}", optional(new), optional(old))
}

/// The observed NDCG@10 new - old with the bootstrap's 95 % interval.
fn delta(m: &Metrics) -> String {
    m.delta_ndcg10.map_or_else(
        || "n/a".to_owned(),
        |d| {
            let observed = m.new.ndcg10 - m.old.ndcg10;
            format!("{observed:+.3} [{:+.3}, {:+.3}]", d.low, d.high)
        },
    )
}

/// The metrics table (Markdown): one row per profile and the total. New / old in each cell.
pub fn table(rows: &[(String, Metrics)], total: &Metrics) -> String {
    let mut out = String::from(
        "| Profile | n | NDCG@10 new / old | NDCG@20 new / old | P@5 new / old | Spearman new / old \
         | High band new / old | Grade 3 buried new / old | Excl. precision / recall | \
         NDCG@10 new - old [95 % CI] |\n|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let mut row = |name: &str, m: &Metrics| {
        let _ = writeln!(
            out,
            "| {name} | {} | {} | {} | {} | {} | {} | {} / {} | {} | {} |",
            m.n,
            pair_of(m.new.ndcg10, m.old.ndcg10),
            pair_of(m.new.ndcg20, m.old.ndcg20),
            pair_of(m.new.p5, m.old.p5),
            pair_of(m.new.spearman, m.old.spearman),
            optional_pair(m.new.high_band, m.old.high_band),
            m.new.grade3_low,
            m.old.grade3_low,
            optional_pair(m.exclusions.precision(), m.exclusions.recall()),
            delta(m),
        );
    };
    for (name, m) in rows {
        row(name, m);
    }
    row("**Total**", total);
    out
}

/// The gate table (Markdown).
pub fn gate_table(gates: &[Gate]) -> String {
    let mut out = String::from("| Gate | Value | Target | Result |\n|---|---|---|---|\n");
    for g in gates {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            g.name,
            g.value,
            g.target,
            if g.pass { "pass" } else { "FAIL" }
        );
    }
    out
}
