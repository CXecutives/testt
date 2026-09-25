//! Metric wiring of `match_eval`: one [`Pair`] per labelled (profile, job), the ranking
//! metrics of `tests/common/eval.rs` for the new and the old engine, exclusion precision and
//! recall, the paired bootstrap of NDCG@10 new - old, and the private gates of `docs/PLAN.md`
//! (with the Spearman gate replaced as `docs/MATCHING.md` "Corpus and gates" explains).
//!
//! Conventions: the gain of a job is `2^grade - 1` with grade 0 when the labelers excluded it
//! (nobody applies to an excluded job); the new engine ranks excluded jobs last and
//! unscorable ones just above them, as the app's list does; the old engine had no exclusion.
//!
//! Two measures look at the order below the top: the Spearman correlation over the relevant
//! pairs only (gain > 0: list order against gain), and the concordance per grade pair (share
//! of same-profile job pairs whose displayed scores order them like their label grades, ties
//! half; the score of an excluded job is kept, so it measures the fit, not the exclusions).
//! The pooled Spearman stays in the tables, but most pairs of a set have gain 0, so its
//! ceiling is low (0.51 on set 7 even for a perfect order) and it gates nothing.
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
/// The grade pairs of the concordance (lower grade, higher grade), in report order.
pub const GRADE_PAIRS: [(u8, u8); 6] = [(0, 3), (0, 2), (0, 1), (1, 3), (1, 2), (2, 3)];
/// Gates of the order below the top (`docs/MATCHING.md` "Corpus and gates"): the Spearman
/// correlation over the relevant pairs, and the concordance of the grade pairs that decide
/// what a user sees first. Grade 0 against grade 1 and grade 1 against grade 2 are reported
/// only: the labelers themselves disagree most there.
pub const MIN_SPEARMAN_RELEVANT: f64 = 0.50;
pub const MIN_CONCORDANCE: [(u8, u8, f64); 4] =
    [(0, 3, 0.95), (0, 2, 0.85), (1, 3, 0.80), (2, 3, 0.70)];

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

    /// The score the new engine shows (kept for an excluded job, 0 when unscorable).
    pub fn new_shown(&self) -> f64 {
        f64::from(self.new_score)
    }

    pub fn old_shown(&self) -> f64 {
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

/// Share of same-profile job pairs of two grades that the scores order like the grades
/// (ties count half), and the number of such pairs; `None` without pairs.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Concordance {
    pub share: Option<f64>,
    pub pairs: usize,
}

/// Ranking quality of one engine.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ranking {
    pub ndcg10: f64,
    pub ndcg20: f64,
    pub p5: f64,
    pub spearman: f64,
    /// Spearman of list order and gain over the relevant pairs only (gain > 0); `None` with
    /// fewer than two of them.
    pub spearman_relevant: Option<f64>,
    /// Concordance of label grade and shown score per pair of [`GRADE_PAIRS`].
    pub concordance: [Concordance; 6],
    /// Share of relevant jobs among those scored 80 or more; `None` when there are none.
    pub high_band: Option<f64>,
    /// Grade-3 jobs the engine buries.
    pub grade3_low: usize,
}

impl Ranking {
    /// The concordance of one grade pair (`lower`, `higher`).
    pub fn concordance_of(&self, lower: u8, higher: u8) -> Concordance {
        GRADE_PAIRS
            .iter()
            .position(|&pair| pair == (lower, higher))
            .map_or_else(Concordance::default, |i| self.concordance[i])
    }
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

/// Spearman of the list order and the gain over the pairs with a gain; `None` when their
/// gains are all equal (no order to judge).
fn spearman_relevant(pairs: &[Pair], key: fn(&Pair) -> f64) -> Option<f64> {
    let relevant: Vec<Pair> = pairs.iter().filter(|p| p.gain() > 0).copied().collect();
    let varied = relevant.windows(2).any(|w| w[0].gain() != w[1].gain());
    varied.then(|| {
        let gains: Vec<f64> = relevant.iter().map(|p| f64::from(p.gain())).collect();
        eval::spearman(&keys(&relevant, key), &gains)
    })
}

/// Concordance of the label grade (not the gain: the score of an excluded job is kept) and
/// the shown score, per grade pair, over the job pairs of each profile.
fn concordance(pairs: &[Pair], shown: fn(&Pair) -> f64) -> [Concordance; 6] {
    let groups = by_profile(pairs);
    GRADE_PAIRS.map(|(lower, higher)| {
        let (mut count, mut agree) = (0usize, 0.0f64);
        for group in &groups {
            for high in group.iter().filter(|p| p.grade == higher) {
                for low in group.iter().filter(|p| p.grade == lower) {
                    count += 1;
                    let (h, l) = (shown(high), shown(low));
                    agree += if h > l {
                        1.0
                    } else if h < l {
                        0.0
                    } else {
                        0.5
                    };
                }
            }
        }
        Concordance {
            share: (count > 0).then(|| agree / count as f64),
            pairs: count,
        }
    })
}

/// Ranking metrics of one engine. NDCG and P@5 are means over the profiles (one ranking per
/// profile; P@5 over the profiles with relevant jobs, against what each can reach);
/// Spearman, the high band and the grade-3 count are pooled over all pairs, the concordance
/// over the job pairs of each profile.
pub fn ranking(
    pairs: &[Pair],
    key: fn(&Pair) -> f64,
    shown: fn(&Pair) -> f64,
    buried: fn(&Pair) -> bool,
) -> Ranking {
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
        spearman_relevant: spearman_relevant(pairs, key),
        concordance: concordance(pairs, shown),
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
        new: ranking(pairs, Pair::new_key, Pair::new_shown, Pair::new_buried),
        old: ranking(pairs, Pair::old_key, Pair::old_shown, Pair::old_buried),
        exclusions: Exclusions::of(pairs),
        delta_ndcg10,
    }
}

/// One private gate of `docs/PLAN.md` (or of `docs/MATCHING.md` "Corpus and gates"), checked on
/// the total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    pub name: String,
    pub value: String,
    pub target: String,
    pub pass: bool,
}

fn optional(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.3}"))
}

/// The gates; a metric without data (`n/a`) does not fail.
pub fn gates(total: &Metrics) -> Vec<Gate> {
    const EPS: f64 = 1e-9;
    let (new, old, e) = (&total.new, &total.old, &total.exclusions);
    let gate = |name: &str, value: String, target: &str, pass| Gate {
        name: name.to_owned(),
        value,
        target: target.to_owned(),
        pass,
    };
    let concordance = MIN_CONCORDANCE.map(|(lower, higher, min)| {
        let share = new.concordance_of(lower, higher).share;
        Gate {
            name: format!("Concordance grade {lower} v {higher}"),
            value: optional(share),
            target: format!(">= {min:.3}"),
            pass: share.is_none_or(|s| s >= min - EPS),
        }
    });
    let relevant = new.spearman_relevant;
    let mut all = vec![
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
            "Spearman, relevant pairs",
            optional(relevant),
            &format!(">= {MIN_SPEARMAN_RELEVANT:.3}"),
            relevant.is_none_or(|s| s >= MIN_SPEARMAN_RELEVANT - EPS),
        ),
        gate(
            "Spearman, relevant pairs, new - old",
            match (relevant, old.spearman_relevant) {
                (Some(n), Some(o)) => format!("{:+.3}", n - o),
                _ => "n/a".to_owned(),
            },
            "> 0",
            match (relevant, old.spearman_relevant) {
                (Some(n), Some(o)) => n > o + EPS,
                _ => true,
            },
        ),
    ];
    all.extend(concordance);
    all
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

/// The order below the top (Markdown): Spearman over the relevant pairs and the concordance
/// of every grade pair, new / old with the number of job pairs, one row per profile and the
/// total.
pub fn order_table(rows: &[(String, Metrics)], total: &Metrics) -> String {
    let mut out = String::from("| Profile | Spearman relevant new / old |");
    for (lower, higher) in GRADE_PAIRS {
        let _ = write!(out, " {lower}v{higher} new / old (n) |");
    }
    out.push_str("\n|---|---|");
    out.push_str(&"---|".repeat(GRADE_PAIRS.len()));
    out.push('\n');
    let mut row = |name: &str, m: &Metrics| {
        let _ = write!(
            out,
            "| {name} | {} |",
            optional_pair(m.new.spearman_relevant, m.old.spearman_relevant)
        );
        for (new, old) in m.new.concordance.iter().zip(&m.old.concordance) {
            let _ = write!(
                out,
                " {} ({}) |",
                optional_pair(new.share, old.share),
                new.pairs
            );
        }
        out.push('\n');
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
