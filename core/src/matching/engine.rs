//! The new engine: requirements read from the job (V5-V7, V16), each item scored on the
//! ladder (V1-V4, V8, V9, V15), hard criteria decided or checked (V10-V13), relevance
//! (V17, V18) and the shrunk score (V19). Integer arithmetic only.
//!
//! `M = sum(w*f*e)/sum(w*f)` over musts (w 1000, soft 250, frame 0; f = 2 for a requirement
//! met in full through a Schwerpunkt, else 1), `K = sum(f*e)/sum(f)` over nice-to-haves,
//! `P = must ? (nice ? (3M+K)/4 : M) : K`, evidence `n = sum(w_must) + 500*n_nice`,
//! `R' = min(1000, R + 100*demanded Schwerpunkte (at most 3))`, `P' = (n*P + k*R')/(n + k)`,
//! permanent roles `P' * 0.9`, then `+ role + clamp(sum(wishes), -100, 100)` per-mille (a
//! lift ends at 790 while fewer than half of the musts are met),
//! `score = round_half_even(P'/10)`, the caps, the floor.

use std::ops::Range;

use serde_json::{Value, json};

use super::ad_facts::{self, AdFacts};
use super::atoms::{self, Fit, Vocab, fold};
use super::contract::{self, Contract, ContractKind};
use super::facts::{self, Finding, HardCriteria, JobFacts, Segment};
use super::fit::{self, ItemFit, Skills};
use super::focus::{self, Focus};
use super::job::{self, Class, Item, Stage};
use super::legacy::LegacyProfile;
use super::lexicon::engine as lex;
use super::normalize::{char_len, strip};
use super::params::{
    E_FULL, E_HALF, E_NONE, FOCUS_FACTOR, FOCUS_RELEVANCE, FOCUS_RELEVANCE_MAX, FORMAL_CAP,
    K_SHRINK, LIFT_CAP, LOW_EVIDENCE_ITEMS, LOW_PRIOR, LOW_PRIOR_WEIGHT, MIN_TEXT_CHARS, N_NICE,
    NO_ITEMS_CAP, OFF_FIELD_CAP, PERMANENT_FACTOR, ROLE_FULL, ROLE_HALF, SCORE_FLOOR,
    SEVERAL_OPEN_CAP, TITLE_OPEN_CAP, W_MUST, W_SOFT, W_TERM, WISH_MAX,
};
use super::permanent;
use super::relevance;
use super::roles::{self, Role, RoleFit};
use super::score::div_round_half_even;
use super::sections::ReqKind;
use super::seniority;
use super::types::{EvidenceLevel, JobInput, ReasonCode, TextKind, Verdict};
use super::wishes::{self, WishResult, Wishes};

/// A profile prepared for the new engine.
pub(crate) struct EngineProfile {
    pub legacy: LegacyProfile,
    pub skills: Skills,
    pub criteria: HardCriteria,
    pub query: Vec<(String, u64)>,
    /// Schwerpunkte (at most `FOCUS_MAX`, those the profile entries carry).
    pub focus: Vec<Focus>,
    /// How many Schwerpunkte the profile names.
    pub focus_count: usize,
    pub roles: Vec<Role>,
    pub wishes: Wishes,
    /// Keys of Schwerpunkte, target roles and wishes present with a value that cannot be
    /// read: (key, value).
    pub unreadable: Vec<(&'static str, String)>,
}

impl EngineProfile {
    pub(crate) fn new(data: &Value) -> Self {
        let legacy = LegacyProfile::new(data);
        let declared = focus::declared(data);
        let skills = Skills::new(&legacy, data, &declared.items);
        let criteria = HardCriteria::new(&legacy.criteria, data);
        let query = relevance::query(&skills);
        let (focus, mut unreadable) = focus::resolve(&skills, &declared.items);
        unreadable.extend(declared.unreadable);
        let (roles, roles_unreadable) = roles::read(data, &skills.vocab);
        unreadable.extend(roles_unreadable);
        let (wishes, wishes_unreadable) = Wishes::new(data, &skills.vocab);
        unreadable.extend(wishes_unreadable);
        Self {
            legacy,
            skills,
            criteria,
            query,
            focus,
            focus_count: declared.count,
            roles,
            wishes,
            unreadable,
        }
    }
}

/// One item with its fit and weight.
#[derive(Debug, Clone)]
pub(crate) struct Scored {
    pub item: Item,
    pub fit: ItemFit,
    pub weight: u64,
    /// The Schwerpunkt that meets the item, and whether in full (then it counts double).
    pub focus: Option<(usize, bool)>,
}

/// A Schwerpunkt the ad demands.
#[derive(Debug, Clone)]
pub(crate) struct FocusHit {
    /// Index into [`EngineProfile::focus`].
    pub index: usize,
    /// Items it meets in full / in half (indices into [`Evaluation::items`]).
    pub met: Vec<usize>,
    pub partial: Vec<usize>,
    /// The title names it.
    pub title: bool,
    /// Relevance it added (per-mille).
    pub relevance: u64,
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
    /// Demanded Schwerpunkte (empty for an unscorable job).
    pub focus: Vec<FocusHit>,
    /// The best target role with its points (per-mille) and the title it met.
    pub role: Option<(RoleFit, i64, String)>,
    /// The wishes with their points (per-mille).
    pub wishes: Vec<WishResult>,
    /// What the ad states about rate, start, duration, remote share, place and contract.
    pub facts: AdFacts,
    /// The score in per-mille before the caps and the rounding: orders jobs with the same
    /// score (two jobs capped at 40 are not equally good).
    pub rank: u16,
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

/// `P` with the must weight and the number of nice-to-haves. A requirement met in full
/// through a Schwerpunkt weighs `FOCUS_FACTOR` times in `M` and `K`; the returned must
/// weight and nice count (the evidence `n`) stay unweighted.
fn fit_score(items: &[Scored]) -> (u64, u64, u64) {
    let (mut must_weight, mut must_fit_weight, mut must_mass) = (0u64, 0u64, 0u64);
    let (mut nice_count, mut nice_fit_count, mut nice_mass) = (0u64, 0u64, 0u64);
    for s in items {
        let factor = if matches!(s.focus, Some((_, true))) {
            FOCUS_FACTOR
        } else {
            1
        };
        match s.item.kind {
            ReqKind::Must => {
                must_weight += s.weight;
                must_fit_weight += s.weight * factor;
                must_mass += s.weight * factor * u64::from(s.fit.value);
            }
            ReqKind::Nice => {
                nice_count += 1;
                nice_fit_count += factor;
                nice_mass += factor * u64::from(s.fit.value);
            }
        }
    }
    let must_score = must_mass.checked_div(must_fit_weight);
    let nice_score = nice_mass.checked_div(nice_fit_count);
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
fn criteria_findings(
    profile: &EngineProfile,
    facts: &JobFacts<'_>,
    segments: &[Segment],
    folded: &str,
) -> (Vec<Finding>, Contract) {
    let anue = facts::anue(facts, segments);
    let contract = contract::infer(facts, segments, &anue);
    let criteria = &profile.criteria;
    let mut findings = facts::check(criteria, facts, segments, folded, &contract, anue);
    findings.extend(permanent::salary(
        criteria,
        &contract,
        facts.title,
        segments,
    ));
    findings.extend(permanent::region(
        criteria, &contract, facts, segments, folded,
    ));
    (findings, contract)
}

/// Demanded Schwerpunkte in profile order; the first `FOCUS_RELEVANCE_MAX` add relevance.
fn focus_hits(profile: &EngineProfile, items: &[Scored], title: &str) -> Vec<FocusHit> {
    if profile.focus.is_empty() {
        return Vec::new();
    }
    let in_title = focus::in_title(&profile.skills, &profile.focus, title);
    let mut hits: Vec<FocusHit> = profile
        .focus
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let with = |full: bool| -> Vec<usize> {
                items
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.focus == Some((index, full)))
                    .map(|(i, _)| i)
                    .collect()
            };
            FocusHit {
                index,
                met: with(true),
                partial: with(false),
                title: in_title[index],
                relevance: 0,
            }
        })
        .filter(|h| h.title || !h.met.is_empty() || !h.partial.is_empty())
        .collect();
    // Named in full (title or a requirement met in full): the Schwerpunkt is demanded.
    for hit in hits
        .iter_mut()
        .filter(|h| h.title || !h.met.is_empty())
        .take(FOCUS_RELEVANCE_MAX)
    {
        hit.relevance = FOCUS_RELEVANCE;
    }
    hits
}

/// The target role the title matches (with its points) and the wishes for one ad.
fn preferences(
    profile: &EngineProfile,
    ad: &wishes::Ad<'_>,
) -> (Option<(RoleFit, i64, String)>, Vec<WishResult>) {
    let title = ad.job.title;
    let role = roles::best(&profile.roles, title, ad.vocab, ad.contract).map(|fit| {
        let points = if fit.full { ROLE_FULL } else { ROLE_HALF };
        (fit, points, title.to_owned())
    });
    let context: Vec<Range<usize>> = if profile.wishes.industries.is_some() {
        job::context_lines(ad.job.text)
    } else {
        Vec::new()
    };
    let ad = wishes::Ad {
        context: &context,
        ..*ad
    };
    (role, wishes::evaluate(&profile.wishes, &ad))
}

/// The shrunk fit plus the target role and the bounded wishes (per-mille, 0..=1000). While
/// fewer than half of the musts are met, a lift ends at `LIFT_CAP` (below the high band).
fn adjust(
    shrunk: u64,
    role: Option<&(RoleFit, i64, String)>,
    wishes: &[WishResult],
    items: &[Scored],
) -> u64 {
    let role_points = role.map_or(0, |(_, points, _)| *points);
    let wish_points = wishes
        .iter()
        .map(|w| w.points)
        .sum::<i64>()
        .clamp(-WISH_MAX, WISH_MAX);
    let adjusted = i64::try_from(shrunk).unwrap_or(1000) + role_points + wish_points;
    let adjusted = u64::try_from(adjusted.clamp(0, 1000)).unwrap_or(0);
    let (met, total) = musts_met(items);
    if adjusted > shrunk && 2 * met < total {
        adjusted.min(shrunk.max(LIFT_CAP))
    } else {
        adjusted
    }
}

/// Musts met in full and all musts, counted like the summary of the assessment (a soft
/// skill below the must weight is information, not a must).
pub(crate) fn musts_met(items: &[Scored]) -> (usize, usize) {
    let musts = items.iter().filter(|s| {
        s.item.kind == ReqKind::Must
            && s.weight > 0
            && (s.weight == W_MUST || s.item.class != Class::Soft)
    });
    musts.fold((0, 0), |(met, total), s| {
        (met + usize::from(s.fit.value == E_FULL), total + 1)
    })
}

/// How much the text allowed to assess: a teaser, low (vocabulary terms, no weighted
/// must, fewer than two weighted items) or full.
fn evidence_level(kind: TextKind, items: &[Scored], must_weight: u64) -> EvidenceLevel {
    let hard_items = items.iter().filter(|s| s.weight > 0).count();
    let vocabulary = items.iter().any(|s| s.item.stage == Stage::Vocabulary);
    if kind == TextKind::Teaser {
        EvidenceLevel::Teaser
    } else if vocabulary || must_weight == 0 || hard_items < LOW_EVIDENCE_ITEMS {
        EvidenceLevel::Low
    } else {
        EvidenceLevel::Full
    }
}

/// Every requirement item with its fit, weight and Schwerpunkt.
fn scored(profile: &EngineProfile, items: Vec<Item>) -> Vec<Scored> {
    items
        .into_iter()
        .map(|item| {
            let mut fit = fit::item_fit(&profile.skills, &item);
            if item.class == Class::Soft {
                // A profile rarely proves soft skills: unproven counts half.
                fit.value = fit.value.max(E_HALF);
            }
            let focus = focus::of_item(&profile.skills, &profile.focus, &item, fit.value);
            Scored {
                weight: weight(&item),
                fit,
                item,
                focus,
            }
        })
        .collect()
}

/// The fields of a job the hard criteria read.
fn job_facts<'a>(job: &JobInput<'a>) -> JobFacts<'a> {
    JobFacts {
        title: job.title,
        text: job.text,
        location: job.location,
        portal: job.portal,
        facts: job.facts,
        posted: job.posted,
    }
}

/// The page's own career level and employment type (LinkedIn's criteria), folded.
fn page_levels(job: &JobInput<'_>) -> Vec<String> {
    [super::fact_key::LEVEL, super::fact_key::CONTRACT]
        .iter()
        .filter_map(|key| facts::fact(job.facts, key).and_then(Value::as_str))
        .map(fold)
        .collect()
}

/// The relevance `R'` of a job: lexical and title fit plus the demanded Schwerpunkte;
/// without any requirement only the title speaks for the field (a teaser's few words name
/// tools of every field).
fn relevance_of(
    profile: &EngineProfile,
    job: &JobInput<'_>,
    requirement_lines: &[Range<usize>],
    no_items: bool,
    focus_relevance: u64,
) -> u64 {
    let vocab = &profile.skills.vocab;
    if no_items {
        return relevance::title_fit(&profile.query, job.title, vocab);
    }
    (relevance::relevance(
        &profile.query,
        vocab,
        job.title,
        job.text,
        requirement_lines,
    ) + focus_relevance)
        .min(1000)
}

/// The score (rounded, capped, at least the floor; 0 when unscorable) and the rank (the
/// per-mille score before caps and rounding).
fn final_score(adjusted: u64, cap: Option<u8>, unscorable: bool) -> (u8, u16) {
    if unscorable {
        return (0, 0);
    }
    let rounded = u8::try_from(div_round_half_even(adjusted, 10).min(100)).unwrap_or(100);
    let score = cap.map_or(rounded, |c| rounded.min(c)).max(SCORE_FLOOR);
    (score, u16::try_from(adjusted).unwrap_or(1000))
}

/// Assesses one job.
pub(crate) fn evaluate(profile: &EngineProfile, job: &JobInput<'_>) -> Evaluation {
    let text = job.text;
    let vocab = &profile.skills.vocab;
    let criteria = &profile.criteria;
    let facts = job_facts(job);
    let segments = facts::segments(job.text);
    let folded = fold(job.text);
    let (mut findings, stated_contract) = criteria_findings(profile, &facts, &segments, &folded);
    let contract = stated_contract.kind;
    let short = char_len(strip(text)) < MIN_TEXT_CHARS;
    let doc = if short {
        job::JobDoc::default()
    } else {
        job::read(text, vocab)
    };
    // The seniority of an ad is the same for every profile (all packs, not the profile's).
    findings.extend(seniority::check(
        criteria.target_years,
        job.title,
        text,
        &doc,
        Vocab::every_pack(),
        &page_levels(job),
    ));
    let ad_facts = ad_facts::read(&facts, &segments, &folded, &stated_contract, &doc);
    let requirement_lines = doc.requirement_lines;
    let items = scored(profile, doc.items);
    let (fit_score, must_weight, nice_count) = fit_score(&items);
    let evidence = evidence_level(job.kind, &items, must_weight);
    let (formal, formal_cap) = formal(profile, &items);
    findings.extend(formal);
    // A text long enough to read but without any requirement is judged from its title and
    // words alone: low evidence, at most `NO_ITEMS_CAP`.
    let cap = cap(&items, job.title, vocab, formal_cap)
        .into_iter()
        .chain((!short && items.is_empty()).then_some(NO_ITEMS_CAP))
        .min();
    let decided = findings.iter().any(|f| f.decided);

    let weight_of_evidence = must_weight + N_NICE * nice_count;
    let unscorable = short;
    let focus = if unscorable {
        Vec::new()
    } else {
        focus_hits(profile, &items, job.title)
    };
    let focus_relevance: u64 = focus.iter().map(|h| h.relevance).sum();
    let relevance = relevance_of(
        profile,
        job,
        &requirement_lines,
        items.is_empty(),
        focus_relevance,
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
    let (role, wishes) = if unscorable {
        (None, Vec::new())
    } else {
        let ad = wishes::Ad {
            job: &facts,
            company: job.company,
            segments: &segments,
            folded: &folded,
            contract,
            context: &[],
            vocab,
        };
        preferences(profile, &ad)
    };
    let adjusted = adjust(shrunk, role.as_ref(), &wishes, &items);
    let (score, rank) = final_score(adjusted, cap, unscorable);
    let verdict = if decided {
        Verdict::Excluded
    } else if unscorable {
        Verdict::Unscorable
    } else {
        Verdict::Scored
    };
    Evaluation {
        items,
        findings,
        verdict,
        score,
        evidence,
        short,
        focus,
        role,
        wishes,
        facts: ad_facts,
        rank,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::types::Via;
    use crate::matching::wishes::State;

    fn must(value: u16) -> Scored {
        Scored {
            item: Item {
                span: None,
                text: "SAP FI".into(),
                alternatives: Vec::new(),
                kind: ReqKind::Must,
                class: Class::Skill,
                years: None,
                stage: Stage::Section,
            },
            fit: ItemFit {
                value,
                entry: None,
                via: Via::Exact,
            },
            weight: W_MUST,
            focus: None,
        }
    }

    fn lift(points: i64) -> Vec<WishResult> {
        vec![WishResult {
            code: ReasonCode::RemoteWish,
            state: State::Met,
            points,
            params: json!({}),
            spans: Vec::new(),
        }]
    }

    #[test]
    fn wishes_lift_below_the_high_band_while_most_musts_are_open() {
        let role = (
            RoleFit {
                role: 0,
                full: true,
            },
            ROLE_FULL,
            "CFO".to_owned(),
        );
        // One of four musts met, three half: role and wishes stop at the cap.
        let weak = [must(E_FULL), must(E_HALF), must(E_HALF), must(E_HALF)];
        assert_eq!(adjust(760, Some(&role), &lift(WISH_MAX), &weak), LIFT_CAP);
        // A score already above the cap is not lowered, a malus still applies.
        assert_eq!(adjust(800, Some(&role), &lift(WISH_MAX), &weak), 800);
        assert_eq!(adjust(760, None, &lift(-WISH_MAX), &weak), 660);
        // Half of the musts met: the lift is free (bounded by the wish maximum).
        let half = [must(E_FULL), must(E_FULL), must(E_HALF), must(E_NONE)];
        assert_eq!(adjust(760, Some(&role), &lift(WISH_MAX), &half), 940);
        // A soft skill below the must weight is no must.
        let mut soft = must(E_NONE);
        soft.item.class = Class::Soft;
        soft.weight = W_SOFT;
        assert_eq!(musts_met(&[must(E_FULL), soft]), (1, 1));
    }
}
