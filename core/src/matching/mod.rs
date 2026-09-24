//! Matching engine: scores a job text against the consultant profile.
//!
//! Pure, synchronous and deterministic; no I/O. [`compile_profile`] once per profile,
//! then [`assess`] per job. [`legacy`] reproduces the old Python engine for parity tests.
//!
//! Phase 1 skeleton: `assess` runs the old engine's logic (with the old crash on inline
//! headings fixed) and explains it with reasons, highlights and criteria.

mod criteria;
mod explain;
mod ladder;
mod lexicon;
mod normalize;
mod profile;
mod pyre;
mod requirements;
mod score;
mod sections;
mod signals;
mod types;

#[doc(hidden)]
pub mod legacy;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub use types::*;

use legacy::LegacyProfile;

/// Version of the scoring behaviour; part of the match revision (`match_rev`).
pub const ENGINE_VERSION: u32 = 1;

/// Profiles with fewer competences than this are `Thin`.
const THIN_BELOW: usize = 5;
/// Competences listed in the profile summary.
const SUMMARY_COMPETENCES: usize = 40;

/// A profile prepared for matching. Building it never fails.
pub struct CompiledProfile {
    legacy: LegacyProfile,
    quality: ProfileQuality,
    summary: ProfileSummary,
    fingerprint: String,
}

impl CompiledProfile {
    pub fn quality(&self) -> ProfileQuality {
        self.quality
    }

    pub fn summary(&self) -> &ProfileSummary {
        &self.summary
    }

    /// 16 hex characters of SHA-256 over the engine version and an order-independent view
    /// of what the engine uses from the profile.
    pub fn fingerprint(&self) -> String {
        self.fingerprint.clone()
    }
}

/// Prepares a profile (any JSON; a non-object gives an `Empty` profile).
pub fn compile_profile(value: &Value) -> CompiledProfile {
    let empty = Value::Object(Map::new());
    let data = if value.is_object() { value } else { &empty };
    let legacy = LegacyProfile::new(data);
    let count = legacy.phrases.len();
    let quality = match count {
        0 => ProfileQuality::Empty,
        n if n < THIN_BELOW => ProfileQuality::Thin,
        _ => ProfileQuality::Good,
    };
    let summary = summarize(&legacy, data, quality);
    let fingerprint = fingerprint(&legacy);
    CompiledProfile {
        legacy,
        quality,
        summary,
        fingerprint,
    }
}

fn summarize(legacy: &LegacyProfile, data: &Value, quality: ProfileQuality) -> ProfileSummary {
    let core = &legacy.signals.core;
    let mut sources: BTreeMap<String, (u16, bool)> = BTreeMap::new();
    for entry in core {
        let pattern = path_pattern(&entry.path);
        let slot = sources.entry(pattern).or_insert((0, true));
        slot.0 = slot.0.saturating_add(1);
        slot.1 &= !entry.explicit;
    }
    let criteria = &legacy.criteria;
    let availability_raw = data
        .get(lexicon::KEY_CRITERIA)
        .and_then(|h| h.get(lexicon::KEY_AVAILABLE))
        .filter(|v| profile::truthy(v))
        .or_else(|| {
            data.get(lexicon::KEY_PREFERENCES)
                .and_then(|p| p.get(lexicon::KEY_AVAILABLE))
        })
        .and_then(Value::as_str);
    let info = |key, set: bool, params: Value| CriterionInfo {
        key,
        set,
        params: params.as_object().cloned().unwrap_or_default(),
    };
    let criteria_info = vec![
        info(
            CriterionKey::MinDayRate,
            criteria.min_day_rate.is_some_and(|m| m != 0),
            json!({
                "min": criteria.min_day_rate.map(|m| m.to_string()),
            }),
        ),
        info(
            CriterionKey::Countries,
            criteria.countries.as_ref().is_some_and(|c| !c.is_empty()),
            json!({
                "countries": criteria.countries,
                "remoteOutsideAllowed": criteria.remote_outside_allowed,
            }),
        ),
        info(
            CriterionKey::NoAnue,
            criteria.anue_excluded == Some(true),
            json!({}),
        ),
        info(
            CriterionKey::Availability,
            criteria.available_now,
            json!({ "from": availability_raw }),
        ),
    ];
    let mut warnings = Vec::new();
    let warn = |code, params: Value| ProfileWarning {
        code,
        params: params.as_object().cloned().unwrap_or_default(),
    };
    match quality {
        ProfileQuality::Empty => warnings.push(warn(ProfileWarningCode::NoCompetences, json!({}))),
        ProfileQuality::Thin => warnings.push(warn(
            ProfileWarningCode::FewCompetences,
            json!({ "count": core.len() }),
        )),
        ProfileQuality::Good => {}
    }
    if criteria_info.iter().all(|c| !c.set) {
        warnings.push(warn(ProfileWarningCode::NoCriteria, json!({})));
    }
    if let Some(raw) = availability_raw.filter(|_| !criteria.available_now) {
        warnings.push(warn(
            ProfileWarningCode::AvailabilityNotUnderstood,
            json!({ "value": raw }),
        ));
    }
    ProfileSummary {
        competence_count: u16::try_from(core.len()).unwrap_or(u16::MAX),
        competences: core
            .iter()
            .take(SUMMARY_COMPETENCES)
            .map(|c| c.text.clone())
            .collect(),
        sources: sources
            .into_iter()
            .map(|(path, (count, guessed))| SourceInfo {
                path,
                count,
                guessed,
            })
            .collect(),
        criteria: criteria_info,
        warnings,
    }
}

/// `kernkompetenzen[3].kompetenz` -> `kernkompetenzen[].kompetenz`.
fn path_pattern(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut in_index = false;
    for c in path.chars() {
        match c {
            '[' => {
                in_index = true;
                out.push_str("[]");
            }
            ']' => in_index = false,
            _ if in_index => {}
            _ => out.push(c),
        }
    }
    out
}

fn fingerprint(legacy: &LegacyProfile) -> String {
    let mut keys: Vec<&str> = legacy.phrases.iter().map(|p| p.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    let c = &legacy.criteria;
    let mut countries = c.countries.clone().unwrap_or_default();
    countries.sort();
    let canonical = format!(
        "engine {ENGINE_VERSION}\nphrases {}\nmin {:?}\ncountries {:?}\nanue {:?}\nnow {}\nremote {:?}\n",
        keys.join("|"),
        c.min_day_rate,
        countries,
        c.anue_excluded,
        c.available_now,
        c.remote_outside_allowed,
    );
    let digest = Sha256::digest(canonical.as_bytes());
    digest.iter().take(8).fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

/// Assesses one job; `None` when the profile is `Empty` (nothing is scored then).
pub fn assess(
    profile: &CompiledProfile,
    job: &JobInput<'_>,
    embedder: Option<&dyn Embedder>,
) -> Option<Assessment> {
    let _ = embedder; // Semantic matching is a later experiment.
    if profile.quality == ProfileQuality::Empty {
        return None;
    }
    Some(explain::assess(&profile.legacy, job))
}

/// Cheap relevance of a job from title and location only (per-mille), to order fetches.
pub fn prescore(profile: &CompiledProfile, title: &str, location: &str) -> u16 {
    let _ = location;
    let title_tokens = legacy::canonical_tokens(title);
    if title_tokens.is_empty() {
        return 0;
    }
    let known = title_tokens
        .iter()
        .filter(|t| profile.legacy.phrases.iter().any(|p| p.tokens.contains(t)))
        .count();
    let permille = score::div_round_half_even(1000 * known as u64, title_tokens.len() as u64);
    u16::try_from(permille.min(1000)).unwrap_or(1000)
}
