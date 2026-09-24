//! The new engine: requirements read from the job (V5-V7, V16), each item scored on the
//! ladder (V1-V4, V8, V9, V15), hard criteria decided or checked (V10-V13), relevance
//! (V17, V18) and the shrunk score (V19). Integer arithmetic only.
//!
//! `M = sum(w*e)/sum(w)` over musts (w 1000, soft 250, frame 0), `K = sum(e)/n_nice`,
//! `P = must ? (nice ? (3M+K)/4 : M) : K`, evidence `n = sum(w_must) + 500*n_nice`,
//! `P' = (n*P + k*R)/(n + k)`, `score = round_half_even(P'/10)`.

use serde_json::{Value, json};

use super::atoms::{self, Fit, Vocab, fold};
use super::contract::{self, ContractKind};
use super::facts::{self, Finding, HardCriteria, JobFacts};
use super::fit::{self, ItemFit, Skills};
use super::job::{self, Class, Item, Stage};
use super::legacy::LegacyProfile;
use super::lexicon::engine as lex;
use super::normalize::{char_len, strip};
use super::params::{
    E_HALF, E_NONE, FORMAL_CAP, K_SHRINK, LOW_EVIDENCE_ITEMS, LOW_PRIOR, LOW_PRIOR_WEIGHT,
    MIN_TEXT_CHARS, N_NICE, OFF_FIELD_CAP, PERMANENT_FACTOR, SCORE_FLOOR, SEVERAL_OPEN_CAP,
    TITLE_OPEN_CAP, W_MUST, W_SOFT, W_TERM,
};
use super::permanent;
use super::relevance;
use super::score::div_round_half_even;
use super::sections::ReqKind;
use super::seniority;
use super::types::{EvidenceLevel, JobInput, ReasonCode, TextKind, Verdict};

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

/// Formal requirements the profile does not hold: without any degree in the profile a
/// check (as before); a must degree of another field or a missing licence is a check with
/// a score cap, decided only when the ad makes it mandatory.
fn formal(profile: &EngineProfile, items: &[Scored]) -> (Vec<Finding>, bool) {
    let mut findings = Vec::new();
    let mut cap = false;
    let mut no_degree = false;
    for s in items {
        let class = match s.item.class {
            Class::Degree => "degree",
            Class::Licence(_) => "licence",
            _ => continue,
        };
        if class == "degree" && profile.skills.degree_fields.is_none() {
            no_degree = true;
            continue;
        }
        if s.item.kind != ReqKind::Must || s.fit.value != E_NONE {
            continue;
        }
        let folded = fold(&s.item.text);
        let mandatory = lex::MANDATORY_WORDS.iter().any(|w| folded.contains(w));
        cap = true;
        findings.push(Finding::new(
            ReasonCode::FormalOpen,
            mandatory,
            None,
            json!({ "class": class, "mandatory": mandatory }),
            s.item.span.clone().into_iter().collect(),
        ));
    }
    if no_degree && findings.is_empty() {
        findings.push(Finding::new(
            ReasonCode::FormalOpen,
            false,
            None,
            json!({}),
            Vec::new(),
        ));
    }
    (findings, cap)
}

/// The rubric's caps: an open formal must; several musts open (at least two and at least
/// half); no skill must met at all (outside the field); an open must on the topic of the
/// title (the core of the role).
fn cap(items: &[Scored], title: &str, vocab: &Vocab, formal_cap: bool) -> Option<u8> {
    let musts: Vec<&Scored> = items
        .iter()
        .filter(|s| s.item.kind == ReqKind::Must && matches!(s.weight, W_MUST | W_TERM))
        .collect();
    let open = musts.iter().filter(|s| s.fit.value == E_NONE).count();
    let several_open = open >= 2 && 2 * open >= musts.len();
    let skills: Vec<&&Scored> = musts
        .iter()
        .filter(|s| s.item.class == Class::Skill)
        .collect();
    let off_field = skills.len() >= 2 && skills.iter().all(|s| s.fit.value == E_NONE);
    let title_atoms: Vec<String> = atoms::atoms(title, vocab)
        .into_iter()
        .filter(|a| !atoms::is_generic(a))
        .collect();
    let related = |a: &str, t: &str| {
        matches!(atoms::fit(a, t), Fit::Equal | Fit::Specific)
            || matches!(atoms::fit(t, a), Fit::Equal | Fit::Specific)
    };
    let core_open = skills.iter().any(|s| {
        s.fit.value == E_NONE
            && atoms::atoms(&s.item.text, vocab)
                .iter()
                .any(|a| !atoms::is_generic(a) && title_atoms.iter().any(|t| related(a, t)))
    });
    [
        formal_cap.then_some(FORMAL_CAP),
        several_open.then_some(SEVERAL_OPEN_CAP),
        off_field.then_some(OFF_FIELD_CAP),
        core_open.then_some(TITLE_OPEN_CAP),
    ]
    .into_iter()
    .flatten()
    .min()
}

/// `P` with the must weight and the number of nice-to-haves.
fn fit_score(items: &[Scored]) -> (u64, u64, u64) {
    let (mut must_weight, mut must_mass, mut nice_count, mut nice_mass) = (0u64, 0u64, 0u64, 0u64);
    for s in items {
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
    let fit = match (must_score, nice_score) {
        (Some(must), Some(nice)) => (3 * must + nice) / 4,
        (Some(must), None) => must,
        (None, Some(nice)) => nice,
        (None, None) => 0,
    };
    (fit, must_weight, nice_count)
}

/// Contract type and the findings of the hard criteria (ANÜ, country, day rate,
/// availability, salary and region of permanent roles).
fn criteria_findings(profile: &EngineProfile, job: &JobInput<'_>) -> (Vec<Finding>, ContractKind) {
    let facts = JobFacts {
        title: job.title,
        text: job.text,
        location: job.location,
        portal: job.portal,
        facts: job.facts,
        posted: job.posted,
    };
    let segments = facts::segments(job.text);
    let folded = fold(job.text);
    let anue = facts::anue(&facts, &segments);
    let contract = contract::infer(&facts, &segments, &anue);
    let criteria = &profile.criteria;
    let mut findings = facts::check(criteria, &facts, &segments, &folded, &contract, anue);
    findings.extend(permanent::salary(criteria, &contract, &segments));
    findings.extend(permanent::region(
        criteria, &contract, &facts, &segments, &folded,
    ));
    (findings, contract.kind)
}

/// Assesses one job.
pub(crate) fn evaluate(profile: &EngineProfile, job: &JobInput<'_>) -> Evaluation {
    let text = job.text;
    let vocab = &profile.skills.vocab;
    let criteria = &profile.criteria;
    let (mut findings, contract) = criteria_findings(profile, job);
    let short = char_len(strip(text)) < MIN_TEXT_CHARS;
    let doc = if short {
        job::JobDoc::default()
    } else {
        job::read(text, vocab)
    };
    findings.extend(seniority::check(
        criteria.target_years,
        job.title,
        text,
        &doc,
        vocab,
    ));
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

    let (fit_score, must_weight, nice_count) = fit_score(&items);
    let hard_items = items.iter().filter(|s| s.weight > 0).count();
    let vocabulary = items.iter().any(|s| s.item.stage == Stage::Vocabulary);
    let evidence = if job.kind == TextKind::Teaser {
        EvidenceLevel::Teaser
    } else if vocabulary || must_weight == 0 || hard_items < LOW_EVIDENCE_ITEMS {
        EvidenceLevel::Low
    } else {
        EvidenceLevel::Full
    };

    let (formal, formal_cap) = formal(profile, &items);
    findings.extend(formal);
    let cap = cap(&items, job.title, vocab, formal_cap);
    let decided = findings.iter().any(|f| f.decided);

    let weight_of_evidence = must_weight + N_NICE * nice_count;
    let relevance = relevance::relevance(
        &profile.query,
        vocab,
        job.title,
        text,
        &doc.requirement_lines,
    );
    let (prior_weight, prior) = if evidence == EvidenceLevel::Full {
        (0, 0)
    } else {
        (LOW_PRIOR_WEIGHT, LOW_PRIOR)
    };
    let mut shrunk = (weight_of_evidence * fit_score + K_SHRINK * relevance + prior_weight * prior)
        / (weight_of_evidence + K_SHRINK + prior_weight);
    if contract == ContractKind::Permanent {
        shrunk = shrunk * PERMANENT_FACTOR / 1000;
    }
    let mut score = u8::try_from(div_round_half_even(shrunk, 10).min(100)).unwrap_or(100);
    if let Some(cap) = cap {
        score = score.min(cap);
    }
    score = score.max(SCORE_FLOOR);
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
