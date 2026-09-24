//! Turns an evaluation into an [`Assessment`]: reasons with quotes, profile evidence and
//! UTF-16 highlight ranges, plus the state of every hard criterion.

use std::ops::Range;

use serde_json::{Map, Value, json};

use super::criteria::Violation;
use super::ladder::{Hit, Rule};
use super::legacy::{self, Evaluation, LegacyProfile};
use super::normalize::{casefold_mapped, char_len, strip};
use super::requirements::Origin;
use super::sections::ReqKind;
use super::types::{
    Assessment, CriterionKey, CriterionState, CriterionStatus, Evidence, EvidenceLevel, Highlight,
    JobInput, Reason, ReasonCode, ReasonKind, Summary, TextKind, Verdict, Via, Weight,
};

/// Longest quote in a reason.
const QUOTE_MAX_CHARS: usize = 120;
/// Fewer assessable requirement items than this is low evidence.
const LOW_EVIDENCE_ITEMS: usize = 3;

/// Collects reasons and highlights with running ids.
struct Builder<'t> {
    text: &'t str,
    reasons: Vec<Reason>,
    highlights: Vec<Highlight>,
}

impl<'t> Builder<'t> {
    fn new(text: &'t str) -> Self {
        Self {
            text,
            reasons: Vec::new(),
            highlights: Vec::new(),
        }
    }

    fn reason(&mut self, kind: ReasonKind, weight: Weight, code: ReasonCode) -> &mut Reason {
        let id = count(self.reasons.len());
        self.reasons.push(Reason {
            id,
            kind,
            weight,
            code,
            label: None,
            evidence: None,
            params: Map::new(),
            ranges: Vec::new(),
        });
        self.reasons.last_mut().expect("just added")
    }

    /// Highlights bytes `range` of the text for the last reason.
    fn highlight(&mut self, range: Range<usize>) {
        let id = count(self.highlights.len());
        let reason = self.reasons.last_mut().expect("a reason to highlight");
        reason.ranges.push(id);
        let highlight = Highlight {
            id,
            start: utf16_offset(self.text, range.start),
            end: utf16_offset(self.text, range.end),
            kind: reason.kind,
            reason: reason.id,
        };
        self.highlights.push(highlight);
    }
}

fn count(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

fn utf16_offset(text: &str, byte: usize) -> u32 {
    let units = text
        .get(..byte)
        .map_or(0, |prefix| prefix.encode_utf16().count());
    u32::try_from(units).unwrap_or(u32::MAX)
}

/// Byte offset of `child` inside `parent` (`child` must be a slice of `parent`).
fn offset_in(parent: &str, child: &str) -> usize {
    (child.as_ptr() as usize).saturating_sub(parent.as_ptr() as usize)
}

fn quote(text: &str) -> String {
    let text = strip(text);
    if char_len(text) <= QUOTE_MAX_CHARS {
        return text.to_owned();
    }
    let cut: String = text.chars().take(QUOTE_MAX_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn evidence(profile: &LegacyProfile, hit: Hit, item: &str) -> Evidence {
    let core = &profile.signals.core[profile.phrase_source[hit.phrase]];
    Evidence {
        profile: core.text.clone(),
        path: core.path.clone(),
        via: match hit.rule {
            Rule::Exact => Via::Exact,
            Rule::Stem => Via::Stem,
            Rule::TwoThirds => Via::General,
        },
        quote: quote(item),
    }
}

fn unscorable(evidence: EvidenceLevel) -> Assessment {
    let mut b = Builder::new("");
    b.reason(ReasonKind::Check, Weight::Info, ReasonCode::ShortText);
    Assessment {
        verdict: Verdict::Unscorable,
        score: 0,
        summary: Summary {
            must_met: 0,
            must_partial: 0,
            must_open: 0,
            must_total: 0,
            nice_met: 0,
            nice_total: 0,
            evidence,
        },
        reasons: b.reasons,
        highlights: b.highlights,
        criteria: Vec::new(),
    }
}

pub(crate) fn assess(profile: &LegacyProfile, job: &JobInput<'_>) -> Assessment {
    let text = job.text;
    let teaser = job.kind == TextKind::Teaser;
    let thin = if teaser {
        EvidenceLevel::Teaser
    } else {
        EvidenceLevel::Low
    };
    if char_len(strip(text)) < legacy::MIN_TEXT_CHARS {
        return unscorable(thin);
    }
    let Ok(evaluation) = legacy::evaluate(profile, text, false) else {
        return unscorable(thin);
    };
    let Some(old) = evaluation.legacy_score() else {
        return unscorable(thin);
    };
    let mut b = Builder::new(text);
    let mut summary = requirement_reasons(&mut b, profile, &evaluation);
    let criteria = criterion_reasons(&mut b, profile, &evaluation);
    summary.evidence = if teaser {
        EvidenceLevel::Teaser
    } else if evaluation.items.len() < LOW_EVIDENCE_ITEMS {
        EvidenceLevel::Low
    } else {
        EvidenceLevel::Full
    };
    if summary.evidence != EvidenceLevel::Full {
        b.reason(ReasonKind::Check, Weight::Info, ReasonCode::LowEvidence);
    }
    let verdict = if evaluation.violations.is_empty() {
        Verdict::Scored
    } else {
        Verdict::Excluded
    };
    Assessment {
        verdict,
        score: old.pct,
        summary,
        reasons: b.reasons,
        highlights: b.highlights,
        criteria,
    }
}

/// Met, partial and open requirements (and vocabulary terms); returns the counts.
fn requirement_reasons(
    b: &mut Builder<'_>,
    profile: &LegacyProfile,
    evaluation: &Evaluation<'_>,
) -> Summary {
    let mut summary = Summary {
        must_met: 0,
        must_partial: 0,
        must_open: 0,
        must_total: 0,
        nice_met: 0,
        nice_total: 0,
        evidence: EvidenceLevel::Full,
    };
    let text = b.text;
    for item in &evaluation.items {
        let must = item.requirement.kind == ReqKind::Must;
        let kind = match item.hits.first() {
            None => ReasonKind::Open,
            Some(_) if item.hits.iter().all(|h| h.rule == Rule::TwoThirds) => ReasonKind::Partial,
            Some(_) => ReasonKind::Met,
        };
        let slot = match (must, kind) {
            (true, ReasonKind::Met) => &mut summary.must_met,
            (true, ReasonKind::Partial) => &mut summary.must_partial,
            (true, _) => &mut summary.must_open,
            (false, ReasonKind::Open) => &mut summary.nice_total,
            (false, _) => &mut summary.nice_met,
        };
        *slot += 1;
        if !must && kind != ReasonKind::Open {
            summary.nice_total += 1;
        }
        let best = item
            .hits
            .iter()
            .find(|h| h.rule != Rule::TwoThirds)
            .or(item.hits.first());
        let source = match item.requirement.origin {
            Origin::Section => "section",
            Origin::Inline => "inline",
            Origin::Cue => "sentence",
        };
        let weight = if must { Weight::Must } else { Weight::Nice };
        let reason = b.reason(kind, weight, ReasonCode::Requirement);
        reason.label = Some(quote(item.item));
        reason.evidence = best.map(|&hit| evidence(profile, hit, item.item));
        reason.params = object(&json!({ "source": source }));
        let start = offset_in(text, item.item);
        b.highlight(start..start + item.item.len());
    }
    summary.must_total = summary.must_met + summary.must_partial + summary.must_open;
    for (skill, hits) in &evaluation.vocab {
        let kind = if hits.is_empty() {
            ReasonKind::Open
        } else {
            ReasonKind::Met
        };
        let reason = b.reason(kind, Weight::Must, ReasonCode::Term);
        reason.label = Some(skill.clone());
        reason.evidence = hits.first().map(|&hit| evidence(profile, hit, skill));
    }
    summary
}

/// Violations of hard criteria and the state of every criterion.
fn criterion_reasons(
    b: &mut Builder<'_>,
    profile: &LegacyProfile,
    evaluation: &Evaluation<'_>,
) -> Vec<CriterionState> {
    let c = &profile.criteria;
    let state = |key, set: bool| CriterionState {
        key,
        status: if set {
            CriterionStatus::Ok
        } else {
            CriterionStatus::Inactive
        },
        reason: None,
    };
    let mut criteria = vec![
        state(
            CriterionKey::MinDayRate,
            c.min_day_rate.is_some_and(|m| m != 0),
        ),
        state(
            CriterionKey::Countries,
            c.countries.as_ref().is_some_and(|l| !l.is_empty()),
        ),
        state(CriterionKey::NoAnue, c.anue_excluded == Some(true)),
        state(CriterionKey::Availability, c.available_now),
    ];
    let (folded, origin) = casefold_mapped(b.text);
    let back = |range: &Range<usize>| origin[range.start]..origin[range.end.min(folded.len())];
    let signals = &evaluation.signals;
    for violation in &evaluation.violations {
        let (key, code, params, ranges): (_, _, _, Vec<Range<usize>>) = match violation {
            Violation::Anue => (
                CriterionKey::NoAnue,
                ReasonCode::Anue,
                json!({}),
                signals.anue_at.iter().cloned().collect(),
            ),
            Violation::DayRate { rate, min } => (
                CriterionKey::MinDayRate,
                ReasonCode::DayRate,
                json!({ "rate": rate, "min": min.to_string() }),
                signals.rate_at.iter().cloned().collect(),
            ),
            Violation::StartFuture => (
                CriterionKey::Availability,
                ReasonCode::Availability,
                json!({}),
                signals.future_at.iter().cloned().collect(),
            ),
            Violation::Country { outside, allowed } => (
                CriterionKey::Countries,
                ReasonCode::Country,
                json!({ "outside": outside, "allowed": allowed }),
                signals
                    .countries
                    .iter()
                    .filter(|(code, _)| outside.iter().any(|o| o == *code))
                    .map(|(_, r)| r.clone())
                    .collect(),
            ),
        };
        let reason = b.reason(ReasonKind::Violation, Weight::Hard, code);
        reason.params = object(&params);
        let id = reason.id;
        for range in &ranges {
            b.highlight(back(range));
        }
        if let Some(state) = criteria.iter_mut().find(|s| s.key == key) {
            state.status = CriterionStatus::Violated;
            state.reason = Some(id);
        }
    }
    criteria
}
