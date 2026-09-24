//! Hard criteria of the profile checked against job signals (Python `check_criteria`).

use super::lexicon;
use super::profile::Criteria;
use super::signals::{JobSignals, rate_below};

/// A violated hard criterion as the old engine decided it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Violation {
    Anue,
    DayRate {
        rate: String,
        min: i128,
    },
    StartFuture,
    Country {
        outside: Vec<String>,
        allowed: Vec<String>,
    },
}

impl Violation {
    /// The old engine's text for this violation.
    pub(crate) fn legacy_text(&self) -> String {
        match self {
            Violation::Anue => lexicon::VIOLATION_ANUE.to_owned(),
            Violation::DayRate { rate, min } => lexicon::violation_day_rate(rate, &min.to_string()),
            Violation::StartFuture => lexicon::VIOLATION_START.to_owned(),
            Violation::Country { outside, allowed } => lexicon::violation_country(outside, allowed),
        }
    }
}

/// Python `check_criteria`, in its order.
pub(crate) fn check(criteria: &Criteria, signals: &JobSignals) -> Vec<Violation> {
    let mut violations = Vec::new();
    if criteria.anue_excluded == Some(true) && signals.anue {
        violations.push(Violation::Anue);
    }
    if let (Some(min), Some(rate)) = (criteria.min_day_rate, &signals.rate)
        && min != 0
        && rate != "0"
        && rate_below(rate, min)
    {
        violations.push(Violation::DayRate {
            rate: rate.clone(),
            min,
        });
    }
    if criteria.available_now && signals.start_future {
        violations.push(Violation::StartFuture);
    }
    if let Some(allowed) = criteria.countries.as_ref().filter(|c| !c.is_empty())
        && !signals.countries.is_empty()
        && !(criteria.remote_outside_allowed == Some(true) && signals.remote == Some(100))
    {
        let outside: Vec<String> = signals
            .countries
            .keys()
            .filter(|code| !allowed.iter().any(|a| a == *code))
            .map(|code| (*code).to_owned())
            .collect();
        if !outside.is_empty() {
            violations.push(Violation::Country {
                outside,
                allowed: allowed.clone(),
            });
        }
    }
    violations
}
