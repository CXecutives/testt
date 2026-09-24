//! Matching engine: scores a job text against the consultant profile.
//!
//! Pure, synchronous and deterministic; no I/O, integer arithmetic. [`compile_profile`]
//! once per profile, then [`assess`] per job. [`legacy`] reproduces the old Python engine
//! for parity tests. See `docs/MATCHING.md`.

mod atoms;
mod contract;
mod criteria;
mod engine;
mod explain;
mod facts;
mod fit;
mod job;
mod ladder;
mod lexicon;
mod normalize;
mod params;
mod permanent;
mod profile;
mod pyre;
mod relevance;
mod requirements;
mod score;
mod sections;
mod seniority;
mod signals;
mod types;

#[doc(hidden)]
pub mod legacy;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub use types::*;

use engine::EngineProfile;
use facts::{Availability, HardCriteria};

/// Version of the scoring behaviour; part of the match revision (`match_rev`).
pub const ENGINE_VERSION: u32 = 3;

/// Keys of the facts JSON the engine reads ([`JobInput::facts`]) - the one definition for
/// the engine and for the pipeline that hands it the facts stored from the job page.
pub mod fact_key {
    /// Contract type as the page words it ("Freiberuflich", "Arbeitnehmerüberlassung").
    pub const CONTRACT: &str = "contract";
    /// Work location as the page states it; replaces the location of the alert mail.
    pub const LOCATION: &str = "location";
    /// Remote share in percent (a number).
    pub const REMOTE_PERCENT: &str = "remotePercent";
    /// Rate as the page words it ("95 €/h").
    pub const RATE: &str = "rate";
    /// Start as the page words it ("ab sofort", "01.11.2026").
    pub const START: &str = "start";
    /// Every key the engine reads.
    pub const ALL: &[&str] = &[CONTRACT, LOCATION, REMOTE_PERCENT, RATE, START];
}

/// Profiles with fewer competences than this are `Thin`.
const THIN_BELOW: usize = 5;
/// Competences listed in the profile summary.
const SUMMARY_COMPETENCES: usize = 40;

/// A profile prepared for matching. Building it never fails.
pub struct CompiledProfile {
    engine: EngineProfile,
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
    let engine = EngineProfile::new(data);
    let quality = match engine.skills.competences().count() {
        0 => ProfileQuality::Empty,
        n if n < THIN_BELOW => ProfileQuality::Thin,
        _ => ProfileQuality::Good,
    };
    let summary = summarize(&engine, data, quality);
    let fingerprint = fingerprint(&engine);
    CompiledProfile {
        engine,
        quality,
        summary,
        fingerprint,
    }
}

/// The hard criteria as understood from the profile.
fn criteria_info(c: &HardCriteria) -> Vec<CriterionInfo> {
    let info = |key, set: bool, params: Value| CriterionInfo {
        key,
        set,
        params: params.as_object().cloned().unwrap_or_default(),
    };
    let available = match c.available {
        Availability::Unset => Value::Null,
        Availability::Now => json!("now"),
        Availability::From(day) => json!(day.to_string()),
    };
    vec![
        info(
            CriterionKey::MinDayRate,
            c.min_rate.is_some(),
            json!({ "min": c.min_rate.map(|m| m.to_string()) }),
        ),
        info(
            CriterionKey::Countries,
            c.countries.is_some(),
            json!({ "countries": c.countries, "remoteOutsideAllowed": c.remote_outside }),
        ),
        info(CriterionKey::NoAnue, c.anue_excluded, json!({})),
        info(
            CriterionKey::Availability,
            c.available != Availability::Unset,
            json!({ "from": available }),
        ),
        info(
            CriterionKey::MinSalary,
            c.min_salary.is_some(),
            json!({ "min": c.min_salary }),
        ),
        info(
            CriterionKey::PermanentRegion,
            c.places.is_some(),
            json!({ "places": c.places, "remoteMin": c.remote_min }),
        ),
        info(
            CriterionKey::TargetYears,
            c.target_years.is_some(),
            json!({ "min": c.target_years }),
        ),
    ]
}

fn summarize(engine: &EngineProfile, data: &Value, quality: ProfileQuality) -> ProfileSummary {
    let core = &engine.legacy.signals.core;
    let mut sources: BTreeMap<String, (u16, bool)> = BTreeMap::new();
    for entry in core {
        let slot = sources
            .entry(path_pattern(&entry.path))
            .or_insert((0, true));
        slot.0 = slot.0.saturating_add(1);
        slot.1 &= !entry.explicit;
    }
    let c = &engine.criteria;
    let availability_raw = facts::availability_text(data);
    let criteria = criteria_info(c);
    let warn = |code, params: Value| ProfileWarning {
        code,
        params: params.as_object().cloned().unwrap_or_default(),
    };
    let mut warnings = Vec::new();
    match quality {
        ProfileQuality::Empty => warnings.push(warn(ProfileWarningCode::NoCompetences, json!({}))),
        ProfileQuality::Thin => {
            warnings.push(warn(
                ProfileWarningCode::FewCompetences,
                json!({ "count": core.len() }),
            ));
        }
        ProfileQuality::Good => {}
    }
    if criteria.iter().all(|c| !c.set) {
        warnings.push(warn(ProfileWarningCode::NoCriteria, json!({})));
    }
    if let Some(raw) = availability_raw.filter(|_| c.available == Availability::Unset) {
        warnings.push(warn(
            ProfileWarningCode::AvailabilityNotUnderstood,
            json!({ "value": raw }),
        ));
    }
    for (key, value) in &c.not_understood {
        warnings.push(warn(
            ProfileWarningCode::CriterionNotUnderstood,
            json!({ "key": key, "value": value }),
        ));
    }
    if c.remote_min.is_some() && c.places.is_none() {
        warnings.push(warn(ProfileWarningCode::RegionWithoutPlaces, json!({})));
    }
    let skills = &engine.skills;
    let aliases = skills
        .entries
        .iter()
        .filter_map(|e| {
            Some(AliasInfo {
                competence: e.alias_of.clone()?,
                alias: e.text.clone(),
                path: e.path.clone(),
            })
        })
        .collect();
    ProfileSummary {
        aliases,
        packs: skills
            .vocab
            .packs()
            .iter()
            .map(|p| (*p).to_owned())
            .collect(),
        years: skills.total_years,
        degrees: skills.degrees.clone(),
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
        criteria,
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

fn fingerprint(engine: &EngineProfile) -> String {
    let mut entries: Vec<String> = engine
        .skills
        .entries
        .iter()
        .map(|e| format!("{}:{:?}", e.atoms.join(" "), e.years))
        .collect();
    entries.sort();
    entries.dedup();
    let mut languages = engine.skills.languages.clone();
    languages.sort();
    let c = &engine.criteria;
    let mut countries = c.countries.clone().unwrap_or_default();
    countries.sort();
    let mut places: Vec<String> = c.places.iter().flatten().map(|p| atoms::fold(p)).collect();
    places.sort();
    let canonical = format!(
        "engine {ENGINE_VERSION}\nentries {}\nlanguages {languages:?}\ndegree {:?} {}\nyears {:?}\n\
         min {:?}\ncountries {countries:?}\nremote {:?}\nanue {}\navailable {:?}\n\
         salary {:?}\nplaces {places:?}\nremoteMin {:?}\ntarget {:?}\npacks {:?}\n",
        entries.join("|"),
        engine.skills.degree_fields,
        engine.skills.degree_level,
        engine.skills.total_years,
        c.min_rate,
        c.remote_outside,
        c.anue_excluded,
        c.available,
        c.min_salary,
        c.remote_min,
        c.target_years,
        engine.skills.vocab.packs(),
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
    let evaluation = engine::evaluate(&profile.engine, job);
    Some(explain::assessment(&profile.engine, job.text, &evaluation))
}

/// Cheap relevance of a job from its title (per-mille), to order fetches before any text
/// is known. The location is not used yet.
pub fn prescore(profile: &CompiledProfile, title: &str, location: &str) -> u16 {
    let _ = location;
    let fit = relevance::title_fit(&profile.engine.query, title, &profile.engine.skills.vocab);
    u16::try_from(fit.min(1000)).unwrap_or(1000)
}
