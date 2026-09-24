//! Turns an engine evaluation into an [`Assessment`]: codes and params (no prose), quotes
//! from the ad, the profile evidence with its JSON path, UTF-16 highlight ranges and the
//! state of every hard criterion.

use std::ops::Range;

use serde_json::{Map, Value, json};

use super::engine::{EngineProfile, Evaluation};
use super::facts::Availability;
use super::job::{Class, Stage};
use super::normalize::{char_len, strip};
use super::params::{E_FULL, E_NONE, W_MUST};
use super::sections::ReqKind;
use super::types::{
    Assessment, CriterionKey, CriterionState, CriterionStatus, Evidence, EvidenceLevel, Highlight,
    Reason, ReasonCode, ReasonKind, Summary, Via, Weight,
};

/// Longest quote in a reason.
const QUOTE_MAX_CHARS: usize = 120;

/// Collects reasons and highlights with running ids.
struct Builder<'t> {
    text: &'t str,
    reasons: Vec<Reason>,
    highlights: Vec<Highlight>,
}

impl Builder<'_> {
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
        if range.is_empty() || range.end > self.text.len() {
            return;
        }
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

fn class_name(class: &Class) -> &'static str {
    match class {
        Class::Skill => "skill",
        Class::Language(..) => "language",
        Class::Degree => "degree",
        Class::Licence(_) => "licence",
        Class::Soft => "soft",
        Class::Frame => "frame",
    }
}

/// Builds the assessment of one evaluated job.
pub(crate) fn assessment(
    profile: &EngineProfile,
    text: &str,
    evaluation: &Evaluation,
) -> Assessment {
    let mut b = Builder {
        text,
        reasons: Vec::new(),
        highlights: Vec::new(),
    };
    let mut summary = Summary {
        must_met: 0,
        must_partial: 0,
        must_open: 0,
        must_total: 0,
        nice_met: 0,
        nice_total: 0,
        evidence: evaluation.evidence,
    };
    for scored in &evaluation.items {
        let item = &scored.item;
        let kind = match scored.fit.value {
            E_FULL => ReasonKind::Met,
            E_NONE => ReasonKind::Open,
            _ => ReasonKind::Partial,
        };
        let weight = match (item.kind, scored.weight) {
            (_, 0) => Weight::Info,
            (ReqKind::Must, W_MUST) => Weight::Must,
            (ReqKind::Must, _) if item.class == Class::Soft => Weight::Info,
            (ReqKind::Must, _) => Weight::Must,
            (ReqKind::Nice, _) => Weight::Nice,
        };
        match (weight, kind) {
            (Weight::Must, ReasonKind::Met) => summary.must_met += 1,
            (Weight::Must, ReasonKind::Partial) => summary.must_partial += 1,
            (Weight::Must, _) => summary.must_open += 1,
            (Weight::Nice, ReasonKind::Met | ReasonKind::Partial) => summary.nice_met += 1,
            _ => {}
        }
        if weight == Weight::Nice {
            summary.nice_total += 1;
        }
        let code = if item.stage == Stage::Vocabulary {
            ReasonCode::Term
        } else {
            ReasonCode::Requirement
        };
        let source = match item.stage {
            Stage::Section => "section",
            Stage::Sentence => "sentence",
            Stage::Vocabulary => "vocabulary",
        };
        let mut params = json!({ "source": source, "class": class_name(&item.class) });
        if let Some(years) = item.years {
            params["years"] = json!(years);
        }
        if let Some((index, true)) = scored.focus {
            // Met in full through a Schwerpunkt: it counts double.
            params["focus"] = json!(profile.focus[index].text);
        }
        let reason = b.reason(kind, weight, code);
        reason.label = Some(quote(&item.text));
        reason.params = object(&params);
        reason.evidence = scored.fit.entry.map(|index| {
            let entry = &profile.skills.entries[index];
            Evidence {
                profile: entry.text.clone(),
                path: entry.path.clone(),
                via: scored.fit.via,
                quote: quote(&item.text),
            }
        });
        if let Some(span) = item.span.clone() {
            b.highlight(span);
        }
    }
    summary.must_total = summary.must_met + summary.must_partial + summary.must_open;

    let criteria = criterion_states(profile, &mut b, evaluation);
    preferences(profile, &mut b, evaluation);
    if evaluation.short {
        b.reason(ReasonKind::Check, Weight::Info, ReasonCode::ShortText);
    } else if evaluation.evidence != EvidenceLevel::Full {
        b.reason(ReasonKind::Check, Weight::Info, ReasonCode::LowEvidence);
    }
    Assessment {
        verdict: evaluation.verdict,
        score: evaluation.score,
        summary,
        reasons: b.reasons,
        highlights: b.highlights,
        criteria,
    }
}

/// Reasons of the version-4 inputs: every demanded Schwerpunkt (with the passages it
/// meets), the target role the title matches, every wish.
fn preferences(profile: &EngineProfile, b: &mut Builder<'_>, evaluation: &Evaluation) {
    for hit in &evaluation.focus {
        let focus = &profile.focus[hit.index];
        let kind = if hit.title || !hit.met.is_empty() {
            ReasonKind::Met
        } else {
            ReasonKind::Partial
        };
        let first = hit.met.iter().chain(&hit.partial).next();
        let quote_of = first.map(|&i| quote(&evaluation.items[i].item.text));
        let reason = b.reason(kind, Weight::Info, ReasonCode::Focus);
        reason.params = object(&json!({
            "focus": focus.text,
            "met": hit.met.len(),
            "partial": hit.partial.len(),
            "inTitle": hit.title,
            "relevance": hit.relevance,
        }));
        reason.evidence = quote_of.map(|quote| Evidence {
            profile: focus.text.clone(),
            path: focus.path.clone(),
            via: Via::Exact,
            quote,
        });
        for &i in hit.met.iter().chain(&hit.partial) {
            if let Some(span) = evaluation.items[i].item.span.clone() {
                b.highlight(span);
            }
        }
    }
    if let Some((fit, points, title)) = &evaluation.role {
        let role = &profile.roles[fit.role];
        let (kind, name, via) = if fit.full {
            (ReasonKind::Met, "full", Via::Exact)
        } else {
            (ReasonKind::Partial, "half", Via::General)
        };
        let reason = b.reason(kind, Weight::Info, ReasonCode::TargetRole);
        reason.params = object(&json!({ "role": role.text, "fit": name, "points": points / 10 }));
        reason.evidence = Some(Evidence {
            profile: role.text.clone(),
            path: role.path.clone(),
            via,
            quote: quote(title),
        });
    }
    for wish in &evaluation.wishes {
        let text = b.text;
        let reason = b.reason(wish.state.kind(), Weight::Info, wish.code);
        reason.params = object(&wish.params);
        reason.evidence = profile
            .wishes
            .source(wish.code)
            .map(|(value, path)| Evidence {
                profile: value.to_owned(),
                path: path.to_owned(),
                via: Via::Exact,
                quote: wish
                    .spans
                    .first()
                    .and_then(|span| text.get(span.clone()))
                    .map(quote)
                    .unwrap_or_default(),
            });
        for span in &wish.spans {
            b.highlight(span.clone());
        }
    }
}

fn criterion_states(
    profile: &EngineProfile,
    b: &mut Builder<'_>,
    evaluation: &Evaluation,
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
    let mut states = vec![
        state(CriterionKey::MinDayRate, c.min_rate.is_some()),
        state(CriterionKey::Countries, c.countries.is_some()),
        state(CriterionKey::NoAnue, c.anue_excluded),
        state(
            CriterionKey::Availability,
            c.available != Availability::Unset,
        ),
        state(CriterionKey::MinSalary, c.min_salary.is_some()),
        state(CriterionKey::PermanentRegion, c.places.is_some()),
        state(CriterionKey::TargetYears, c.target_years.is_some()),
    ];
    for finding in &evaluation.findings {
        let weight = if finding.decided {
            Weight::Hard
        } else {
            Weight::Info
        };
        let reason = b.reason(finding.kind, weight, finding.code);
        reason.params = object(&finding.params);
        let id = reason.id;
        for span in &finding.spans {
            b.highlight(span.clone());
        }
        if let Some(state) = finding
            .key
            .and_then(|key| states.iter_mut().find(|s| s.key == key))
        {
            let next = match finding.kind {
                ReasonKind::Violation => CriterionStatus::Violated,
                ReasonKind::Check => CriterionStatus::Check,
                // A frame row (over-qualified) leaves the criterion met.
                _ => state.status,
            };
            if state.status != CriterionStatus::Violated {
                state.status = next;
                state.reason = Some(id);
            }
        }
    }
    states
}
