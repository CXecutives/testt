//! Scores.
//!
//! The old engine computed its percentage in Python floats. Exact rational arithmetic
//! with half-to-even rounding disagrees with it in thousands of ordinary cases (for
//! example 85 % x 0.7 is 59.5 exactly, which rounds to 60, while Python's float product is
//! 59.49999... and rounds to 59). The legacy path therefore repeats Python's IEEE-754
//! double operations in the same order; they are deterministic on every platform. The new
//! engine stays integer-only.

/// Python's `0.7 ** k` for k = 0..=4 (the old engine has four criteria), bit for bit.
const DECAY: [f64; 5] = [
    1.0,
    f64::from_bits(0x3FE6_6666_6666_6666),
    f64::from_bits(0x3FDF_5C28_F5C2_8F5B),
    f64::from_bits(0x3FD5_F3B6_45A1_CABF),
    f64::from_bits(0x3FCE_BB98_C7E2_823F),
];

/// Which rule produced the old score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyBasis {
    /// Must requirements (plus nice-to-have bonus).
    Must,
    /// Nice-to-haves only (60 % cap).
    NiceOnly,
    /// Vocabulary terms.
    Vocab,
}

/// Counts that the old score is computed from.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LegacyCounts {
    pub must_total: usize,
    pub must_covered: usize,
    pub nice_total: usize,
    pub nice_covered: usize,
    pub vocab_total: usize,
    pub vocab_matched: usize,
    pub violations: usize,
}

/// (percentage, coverage percentage, basis); `None` when nothing was assessable.
pub(crate) fn legacy(c: &LegacyCounts) -> Option<(u8, u8, LegacyBasis)> {
    let ratio = |a: usize, b: usize| as_f64(a) / as_f64(b);
    let (base, coverage, basis) = if c.must_total > 0 {
        let coverage = ratio(c.must_covered, c.must_total);
        let base = if c.nice_total > 0 {
            0.75 * coverage + 0.25 * ratio(c.nice_covered, c.nice_total)
        } else {
            coverage
        };
        (base, coverage, LegacyBasis::Must)
    } else if c.nice_total > 0 {
        let bonus = ratio(c.nice_covered, c.nice_total);
        (0.6 * bonus, bonus, LegacyBasis::NiceOnly)
    } else if c.vocab_total > 0 {
        let base = ratio(c.vocab_matched, c.vocab_total);
        (base, base, LegacyBasis::Vocab)
    } else {
        return None;
    };
    let decay = DECAY[c.violations.min(DECAY.len() - 1)];
    Some((
        python_round(100.0 * base * decay),
        python_round(100.0 * coverage),
        basis,
    ))
}

fn as_f64(n: usize) -> f64 {
    // Requirement counts are far below 2^32.
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// Python `round(x)` for 0 <= x <= 100: half to even.
fn python_round(x: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rounded = x.round_ties_even().clamp(0.0, 255.0) as u8;
    rounded
}

/// Half-to-even rounding of `numerator / denominator` for non-negative integers.
#[allow(dead_code)] // Used by the engine API.
pub(crate) fn div_round_half_even(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let quotient = numerator / denominator;
    let twice_rest = 2 * (numerator % denominator);
    match twice_rest.cmp(&denominator) {
        std::cmp::Ordering::Less => quotient,
        std::cmp::Ordering::Greater => quotient + 1,
        std::cmp::Ordering::Equal => quotient + (quotient & 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_matches_python() {
        assert_eq!(DECAY[1].to_bits(), 0.7_f64.to_bits());
        assert_eq!(DECAY[2].to_string(), "0.48999999999999994");
        assert_eq!(DECAY[3].to_string(), "0.3429999999999999");
        assert_eq!(DECAY[4].to_string(), "0.24009999999999995");
    }

    #[test]
    fn float_rounding_like_python() {
        // 1 of 1 must, 2 of 5 nice, one violation: exactly 59.5, Python says 59.
        let c = LegacyCounts {
            must_total: 1,
            must_covered: 1,
            nice_total: 5,
            nice_covered: 2,
            violations: 1,
            ..Default::default()
        };
        assert_eq!(legacy(&c), Some((59, 100, LegacyBasis::Must)));
        let c = LegacyCounts {
            must_total: 3,
            must_covered: 2,
            ..Default::default()
        };
        assert_eq!(legacy(&c), Some((67, 67, LegacyBasis::Must)));
    }

    #[test]
    fn integer_rounding() {
        assert_eq!(div_round_half_even(5, 2), 2);
        assert_eq!(div_round_half_even(7, 2), 4);
        assert_eq!(div_round_half_even(2, 3), 1);
    }
}
