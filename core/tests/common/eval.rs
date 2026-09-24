//! Ranking metrics for the matching evaluation (docs/MATCHING.md).
//!
//! Grades are 0-3 (3 = would apply). All functions are deterministic: ties in the
//! ranking keep the input order, the bootstrap uses a fixed seed.
#![allow(clippy::cast_precision_loss)] // Counts are tiny; these are report numbers.

/// Indices ordered by score, highest first; ties keep the input order.
pub fn ranking(scores: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    order
}

fn dcg(grades: impl Iterator<Item = u8>) -> f64 {
    grades
        .enumerate()
        .map(|(i, g)| (2f64.powi(i32::from(g)) - 1.0) / ((i + 2) as f64).log2())
        .sum()
}

/// NDCG@k of the ranking by `scores`, gain `2^grade - 1`; 0 when no job is relevant.
pub fn ndcg_at(scores: &[f64], grades: &[u8], k: usize) -> f64 {
    let actual = dcg(ranking(scores).into_iter().take(k).map(|i| grades[i]));
    let mut ideal: Vec<u8> = grades.to_vec();
    ideal.sort_unstable_by(|a, b| b.cmp(a));
    let best = dcg(ideal.into_iter().take(k));
    if best == 0.0 { 0.0 } else { actual / best }
}

/// Precision@k: share of the top k with a grade of at least `relevant`.
pub fn precision_at(scores: &[f64], grades: &[u8], k: usize, relevant: u8) -> f64 {
    let top: Vec<usize> = ranking(scores).into_iter().take(k).collect();
    if top.is_empty() {
        return 0.0;
    }
    top.iter().filter(|&&i| grades[i] >= relevant).count() as f64 / top.len() as f64
}

/// Average ranks (1-based), ties share their mean rank.
fn ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut result = vec![0.0; values.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && values[order[j + 1]].total_cmp(&values[order[i]]).is_eq() {
            j += 1;
        }
        let mean = (i + j) as f64 / 2.0 + 1.0;
        for &index in &order[i..=j] {
            result[index] = mean;
        }
        i = j + 1;
    }
    result
}

/// Spearman rank correlation (Pearson on average ranks); 0 for constant input.
pub fn spearman(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let (ra, rb) = (ranks(a), ranks(b));
    let n = ra.len() as f64;
    let (ma, mb) = (ra.iter().sum::<f64>() / n, rb.iter().sum::<f64>() / n);
    let cov: f64 = ra.iter().zip(&rb).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = ra.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = rb.iter().map(|y| (y - mb).powi(2)).sum();
    if va == 0.0 || vb == 0.0 {
        0.0
    } else {
        cov / (va * vb).sqrt()
    }
}

/// Share of jobs in the high band (score >= 80) with a grade of at least `relevant`;
/// `None` when the high band is empty.
pub fn high_band_precision(scores: &[f64], grades: &[u8], relevant: u8) -> Option<f64> {
    let high: Vec<usize> = (0..scores.len()).filter(|&i| scores[i] >= 80.0).collect();
    if high.is_empty() {
        return None;
    }
    Some(high.iter().filter(|&&i| grades[i] >= relevant).count() as f64 / high.len() as f64)
}

/// Result of a paired bootstrap: mean difference and the 95 % percentile interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub mean: f64,
    pub low: f64,
    pub high: f64,
}

/// Paired bootstrap over `n` items: `diff(sample)` is the metric difference (new - old)
/// on a resample of item indices. Fixed seed, so reports are reproducible.
pub fn paired_bootstrap(
    n: usize,
    rounds: usize,
    seed: u64,
    diff: impl Fn(&[usize]) -> f64,
) -> Interval {
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut values: Vec<f64> = (0..rounds)
        .map(|_| {
            let sample: Vec<usize> = (0..n).map(|_| rng.usize(0..n)).collect();
            diff(&sample)
        })
        .collect();
    values.sort_by(f64::total_cmp);
    let at = |q: f64| {
        let index = ((values.len() - 1) as f64 * q).round();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = index as usize;
        values[index]
    };
    Interval {
        mean: values.iter().sum::<f64>() / values.len() as f64,
        low: at(0.025),
        high: at(0.975),
    }
}

/// Distance of a score to an expected band (0 inside).
pub fn band_distance(score: u8, band: (u8, u8)) -> u8 {
    if score < band.0 {
        band.0 - score
    } else {
        score.saturating_sub(band.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn ndcg_perfect_and_reversed() {
        let grades = [3, 2, 0, 1];
        assert!(close(ndcg_at(&[4.0, 3.0, 1.0, 2.0], &grades, 4), 1.0));
        let reversed = ndcg_at(&[1.0, 2.0, 4.0, 3.0], &grades, 4);
        assert!(reversed < 0.6, "{reversed}");
        assert!(close(ndcg_at(&[1.0, 2.0], &[0, 0], 2), 0.0));
        // Hand-computed: ranking [g=2, g=3] -> (3 + 7/log2 3) / (7 + 3/log2 3).
        let expected = (3.0 + 7.0 / 3f64.log2()) / (7.0 + 3.0 / 3f64.log2());
        assert!(close(ndcg_at(&[2.0, 1.0], &[2, 3], 2), expected));
    }

    #[test]
    fn precision_and_high_band() {
        let scores = [90.0, 85.0, 50.0, 10.0];
        let grades = [3, 1, 2, 0];
        assert!(close(precision_at(&scores, &grades, 2, 2), 0.5));
        assert!(close(precision_at(&scores, &grades, 3, 2), 2.0 / 3.0));
        assert_eq!(high_band_precision(&scores, &grades, 2), Some(0.5));
        assert_eq!(high_band_precision(&[10.0], &[3], 2), None);
    }

    #[test]
    fn spearman_with_ties() {
        assert!(close(spearman(&[1.0, 2.0, 3.0], &[10.0, 20.0, 30.0]), 1.0));
        assert!(close(spearman(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]), -1.0));
        let tied = spearman(&[1.0, 1.0, 2.0, 3.0], &[1.0, 2.0, 3.0, 4.0]);
        assert!(close(tied, 0.948_683_298_050_513_8), "{tied}");
        assert!(close(spearman(&[1.0, 1.0], &[1.0, 2.0]), 0.0));
    }

    #[test]
    fn bootstrap_is_reproducible_and_centred() {
        let diffs = [1.0, 2.0, 3.0, 4.0, 5.0];
        let run = || {
            paired_bootstrap(5, 2000, 7, |s| {
                s.iter().map(|&i| diffs[i]).sum::<f64>() / s.len() as f64
            })
        };
        let a = run();
        assert_eq!(a, run());
        assert!(
            a.low < 3.0
                && 3.0 < a.high
                && close((a.mean - 3.0).abs().min(0.2), (a.mean - 3.0).abs())
        );
    }

    #[test]
    fn distance_to_band() {
        assert_eq!(band_distance(50, (40, 60)), 0);
        assert_eq!(band_distance(30, (40, 60)), 10);
        assert_eq!(band_distance(75, (40, 60)), 15);
    }
}
