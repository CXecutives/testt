//! The new engine: requirements read from the job (V5-V7, V16), each item scored on the
//! ladder (V1-V4, V8, V9, V15), hard criteria decided or checked (V10-V13), relevance
//! (V17, V18) and the shrunk score (V19). Integer arithmetic only.
//!
//! `M = sum(w*e)/sum(w)` over musts (w 1000, soft 250, frame 0), `K = sum(e)/n_nice`,
//! `P = must ? (nice ? (3M+K)/4 : M) : K`, evidence `n = sum(w_must) + 500*n_nice`,
//! `P' = (n*P + k*R)/(n + k)`, `score = round_half_even(P'/10)`.

use serde_json::Value;

use super::facts::{self, Finding, HardCriteria, JobFacts};
use super::fit::{self, ItemFit, Skills};
use super::job::{self, Class, Item, Stage};
use super::legacy::LegacyProfile;
use super::normalize::{char_len, strip};
use super::params::{
    E_HALF, K_SHRINK, LOW_EVIDENCE_ITEMS, LOW_PRIOR, LOW_PRIOR_WEIGHT, MIN_TEXT_CHARS, N_NICE,
    W_MUST, W_SOFT, W_TERM,
};
use super::relevance;
use super::score::div_round_half_even;
use super::sections::ReqKind;
use super::types::{EvidenceLevel, JobInput, TextKind, Verdict};

/// A profile prepared for the new engine.
pub(crate) struct EngineProfile {
    pub legacy: LegacyProfile,
    pub skills: Skills,
    pub criteria: HardCriteria,
    pub query: Vec<(String, u64)>,
}

impl EngineProfile {
    pub(crate) fn new(data: &Value) -> Self {
        let legacy = LegacyProfile::new(data);
        let skills = Skills::new(&legacy, data);
        let criteria = HardCriteria::new(&legacy.criteria, data);
        let query = relevance::query(&skills);
        Self {
            legacy,
            skills,
            criteria,
            query,
        }
    }
}

/// One item with its fit and weight.
#[derive(Debug, Clone)]
pub(crate) struct Scored {
    pub item: Item,
    pub fit: ItemFit,
    pub weight: u64,
}

/// Everything the engine found for one job.
#[derive(Debug, Clone)]
pub(crate) struct Evaluation {
    pub items: Vec<Scored>,
    pub findings: Vec<Finding>,
    pub verdict: Verdict,
    pub score: u8,
    pub evidence: EvidenceLevel,
    pub short: bool,
}

fn weight(item: &Item) -> u64 {
    match (&item.class, item.stage) {
        (Class::Frame, _) => 0,
        (Class::Soft, _) => W_SOFT,
        (_, Stage::Vocabulary) => W_TERM,
        _ => W_MUST,
    }
}

/// Assesses one job.
pub(crate) fn evaluate(profile: &EngineProfile, job: &JobInput<'_>) -> Evaluation {
    let text = job.text;
    let facts = JobFacts {
        text,
        location: job.location,
        facts: job.facts,
        posted: job.posted,
    };
    let findings = facts::check(&profile.criteria, &facts);
    let decided = findings.iter().any(|f| f.decided);
    let short = char_len(strip(text)) < MIN_TEXT_CHARS;
    let doc = if short {
        job::JobDoc::default()
    } else {
        job::read(text)
    };
    let items: Vec<Scored> = doc
        .items
        .into_iter()
        .map(|item| {
            let mut fit = fit::item_fit(&profile.skills, &item);
            if item.class == Class::Soft {
                // A profile rarely proves soft skills: unproven counts half.
                fit.value = fit.value.max(E_HALF);
            }
            Scored {
                weight: weight(&item),
                fit,
                item,
            }
        })
        .collect();

    let (mut must_weight, mut must_mass, mut nice_count, mut nice_mass) = (0u64, 0u64, 0u64, 0u64);
    for s in &items {
        match s.item.kind {
            ReqKind::Must => {
                must_weight += s.weight;
                must_mass += s.weight * u64::from(s.fit.value);
            }
            ReqKind::Nice => {
                nice_count += 1;
                nice_mass += u64::from(s.fit.value);
            }
        }
    }
    let must_score = must_mass.checked_div(must_weight);
    let nice_score = nice_mass.checked_div(nice_count);
    let fit_score = match (must_score, nice_score) {
        (Some(must), Some(nice)) => (3 * must + nice) / 4,
        (Some(must), None) => must,
        (None, Some(nice)) => nice,
        (None, None) => 0,
    };
    let hard_items = items.iter().filter(|s| s.weight > 0).count();
    let vocabulary = items.iter().any(|s| s.item.stage == Stage::Vocabulary);
    let evidence = if job.kind == TextKind::Teaser {
        EvidenceLevel::Teaser
    } else if vocabulary || must_weight == 0 || hard_items < LOW_EVIDENCE_ITEMS {
        EvidenceLevel::Low
    } else {
        EvidenceLevel::Full
    };

    let weight_of_evidence = must_weight + N_NICE * nice_count;
    let relevance = relevance::relevance(&profile.query, job.title, text, &doc.requirement_lines);
    let (prior_weight, prior) = if evidence == EvidenceLevel::Full {
        (0, 0)
    } else {
        (LOW_PRIOR_WEIGHT, LOW_PRIOR)
    };
    let shrunk = (weight_of_evidence * fit_score + K_SHRINK * relevance + prior_weight * prior)
        / (weight_of_evidence + K_SHRINK + prior_weight);
    let score = u8::try_from(div_round_half_even(shrunk, 10).min(100)).unwrap_or(100);
    let verdict = if decided {
        Verdict::Excluded
    } else if short || items.is_empty() {
        Verdict::Unscorable
    } else {
        Verdict::Scored
    };
    let score = if short || items.is_empty() { 0 } else { score };
    Evaluation {
        items,
        findings,
        verdict,
        score,
        evidence,
        short,
    }
}
