//! The matching engine behind [`Matcher`]: one compiled profile, its revision
//! `e{ENGINE_VERSION}:{fingerprint}` and the mapping of an [`Assessment`] onto what the store
//! keeps per job ([`MatchRecord`]).

use serde_json::{Map, Value};

use super::Matcher;
use crate::matching::{
    self, Assessment, CompiledProfile, JobInput, ProfileQuality, ReasonCode, ReasonKind, TextKind,
    Verdict, Weight,
};
use crate::model::{MatchRecord, MatchStatus, Notice, is_usable_title};
use crate::store::JobRow;

/// Met requirements quoted in the list row.
const TOP: usize = 2;

/// The local engine with one profile.
pub struct LocalMatcher {
    profile: CompiledProfile,
    rev: String,
}

impl LocalMatcher {
    pub fn new(profile: CompiledProfile) -> LocalMatcher {
        let rev = format!("e{}:{}", matching::ENGINE_VERSION, profile.fingerprint());
        LocalMatcher { profile, rev }
    }

    /// Compiles a profile file's JSON (anything that is no object gives an empty profile).
    pub fn from_json(value: &Value) -> LocalMatcher {
        LocalMatcher::new(matching::compile_profile(value))
    }

    pub fn profile(&self) -> &CompiledProfile {
        &self.profile
    }

    /// The profile names competences: jobs can be scored. An empty profile scores nothing
    /// and leaves nothing pending.
    pub fn usable(&self) -> bool {
        self.profile.quality() != ProfileQuality::Empty
    }

    /// The full assessment of a job (`text`: its stored description, if any). A job without
    /// text is judged from title and location alone - usually `unscorable`, but a clear
    /// location can already exclude it.
    pub fn assessment(&self, job: &JobRow, text: Option<&str>) -> Option<Assessment> {
        let title = if is_usable_title(&job.title) {
            job.title.clone()
        } else {
            crate::view::slug_title(job.url.as_str()).unwrap_or_default()
        };
        let posted = crate::time::local_date(job.mail_date.unwrap_or(job.first_seen_at));
        let input = JobInput {
            title: &title,
            location: &job.location,
            portal: job.key.portal,
            text: text.unwrap_or_default(),
            facts: None,
            posted: Some(posted),
            kind: TextKind::Full,
        };
        matching::assess(&self.profile, &input, None)
    }
}

impl Matcher for LocalMatcher {
    fn rev(&self) -> &str {
        &self.rev
    }

    fn assess(&self, job: &JobRow, text: Option<&str>) -> Option<MatchRecord> {
        self.assessment(job, text).map(|a| record(&a))
    }

    fn explain(&self, job: &JobRow, text: Option<&str>) -> Option<Assessment> {
        self.assessment(job, text)
    }
}

/// What the list keeps of an assessment: status, score, the one note, must counts and up to
/// two met requirements.
pub fn record(assessment: &Assessment) -> MatchRecord {
    let status = match assessment.verdict {
        Verdict::Scored => MatchStatus::Scored,
        Verdict::Excluded => MatchStatus::Excluded,
        Verdict::Unscorable => MatchStatus::Unscorable,
    };
    MatchRecord {
        status,
        score: assessment.score,
        note: note(assessment),
        must_met: assessment.summary.must_met,
        must_total: assessment.summary.must_total,
        top: top(assessment),
    }
}

/// The statement of the list row: the first decided violation of an excluded job, why an
/// unscorable one has no score, else the first point to check (`None` if nothing).
fn note(assessment: &Assessment) -> Option<Notice> {
    let first = |kind: ReasonKind| assessment.reasons.iter().find(|r| r.kind == kind);
    let reason = match assessment.verdict {
        Verdict::Excluded => first(ReasonKind::Violation),
        Verdict::Unscorable => assessment
            .reasons
            .iter()
            .find(|r| r.code == ReasonCode::ShortText)
            .or_else(|| first(ReasonKind::Check)),
        Verdict::Scored => first(ReasonKind::Check),
    }?;
    Some(Notice {
        code: code_name(&reason.code),
        params: flat_params(&reason.params),
    })
}

/// Up to two met requirements quoted from the ad, musts first.
fn top(assessment: &Assessment) -> Vec<String> {
    let met = |weight: Weight| {
        assessment.reasons.iter().filter(move |r| {
            r.kind == ReasonKind::Met
                && r.weight == weight
                && matches!(r.code, ReasonCode::Requirement | ReasonCode::Term)
        })
    };
    met(Weight::Must)
        .chain(met(Weight::Nice))
        .filter_map(|r| r.label.clone())
        .take(TOP)
        .collect()
}

/// The camelCase name of a serialisable code (`ReasonCode::DayRate` -> `dayRate`).
pub(crate) fn code_name(code: &impl serde::Serialize) -> String {
    match serde_json::to_value(code) {
        Ok(Value::String(name)) => name,
        _ => String::new(),
    }
}

/// Params as the interface takes them: scalars only. Lists of scalars become one
/// comma-separated string (`["DE", "AT"]` -> `"DE, AT"`), nested objects are dropped.
pub(crate) fn flat_params(params: &Map<String, Value>) -> Map<String, Value> {
    params
        .iter()
        .filter_map(|(key, value)| {
            let flat = match value {
                Value::Array(items) => Value::String(
                    items
                        .iter()
                        .filter_map(|item| match item {
                            Value::String(s) => Some(s.clone()),
                            Value::Number(n) => Some(n.to_string()),
                            Value::Bool(b) => Some(b.to_string()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Value::Object(_) => return None,
                scalar => scalar.clone(),
            };
            Some((key.clone(), flat))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use serde_json::json;

    use super::*;
    use crate::model::Posting;
    use crate::portal::{JobKey, job_link};
    use crate::store::{MailRef, Store};

    fn profile() -> Value {
        serde_json::from_str(super::super::demo::PROFILE_JSON).unwrap()
    }

    /// A stored job with `text` as its description (none if empty).
    fn job(title: &str, location: &str, text: &str) -> (Store, JobKey) {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let link = job_link("https://www.linkedin.com/jobs/view/4000000001/").unwrap();
        let posting = Posting::new(link.key.clone(), link.url, title, "Muster AG", location);
        let mail = MailRef {
            subject: "Neue Jobs",
            date: Some("2026-09-18T07:00:00Z".parse().unwrap()),
            gmail_id: None,
        };
        let now = Timestamp::now();
        store.upsert_posting(run, &posting, mail, now).unwrap();
        if !text.is_empty() {
            store
                .record_text(&link.key, text, false, false, now)
                .unwrap();
        }
        (store, link.key)
    }

    fn assess(matcher: &LocalMatcher, title: &str, location: &str, text: &str) -> MatchRecord {
        let (store, key) = job(title, location, text);
        let row = store.job(&key).unwrap().unwrap();
        let text = store.description(&key).unwrap();
        matcher.assess(&row, text.as_deref()).unwrap()
    }

    const FIT: &str = "Wir suchen einen Interim CFO (m/w/d).

Anforderungen:
- Erfahrung im Controlling
- Konzernrechnungslegung nach IFRS
- Erfahrung mit SAP S/4HANA
- Kenntnisse in Zollabwicklung

Rahmenbedingungen:
- Einsatzort Hamburg";

    #[test]
    fn the_revision_names_engine_and_profile() {
        let matcher = LocalMatcher::from_json(&profile());
        let rev = matcher.rev();
        assert!(
            rev.starts_with(&format!("e{}:", matching::ENGINE_VERSION)),
            "{rev}"
        );
        assert_eq!(
            rev.len(),
            2 + matching::ENGINE_VERSION.to_string().len() + 16
        );
        assert_eq!(rev, LocalMatcher::from_json(&profile()).rev(), "stable");
        let mut other = profile();
        other["keywords"] = json!(["Treasury"]);
        assert_ne!(rev, LocalMatcher::from_json(&other).rev());
        assert!(matcher.usable());
        let empty = LocalMatcher::from_json(&json!({"name": "Nur ein Name"}));
        assert!(!empty.usable());
        let (store, key) = job("Interim CFO", "Hamburg", FIT);
        let row = store.job(&key).unwrap().unwrap();
        assert_eq!(
            empty.assess(&row, Some(FIT)),
            None,
            "an empty profile judges nothing"
        );
    }

    #[test]
    fn a_scored_job_keeps_counts_and_two_met_quotes() {
        let matcher = LocalMatcher::from_json(&profile());
        let record = assess(&matcher, "Interim CFO (m/w/d)", "Hamburg", FIT);
        assert_eq!(record.status, MatchStatus::Scored);
        assert!(record.score >= 40, "{record:?}");
        assert_eq!((record.must_met, record.must_total), (3, 4), "{record:?}");
        assert_eq!(
            record.top,
            [
                "Erfahrung im Controlling",
                "Konzernrechnungslegung nach IFRS"
            ]
        );
        assert_eq!(record.note, None, "nothing to check");
    }

    #[test]
    fn the_note_says_why_excluded_unscorable_or_what_to_check() {
        let matcher = LocalMatcher::from_json(&profile());
        let excluded = assess(
            &matcher,
            "Interim CFO",
            "Hamburg",
            &format!("{FIT}\n- Tagessatz bis 700 €"),
        );
        assert_eq!(excluded.status, MatchStatus::Excluded);
        let note = excluded.note.unwrap();
        assert_eq!(note.code, "dayRate");
        assert_eq!(note.params["min"], "1000", "{note:?}");
        let country = assess(&matcher, "Interim CFO", "Paris, Frankreich", "");
        assert_eq!(country.status, MatchStatus::Excluded, "{country:?}");
        let country = country.note.unwrap();
        assert_eq!(country.code, "country");
        assert!(
            country
                .params
                .values()
                .all(|v| !v.is_array() && !v.is_object()),
            "lists are flattened for the interface: {country:?}"
        );
        let untold = assess(&matcher, "Interim CFO", "Hamburg", "");
        assert_eq!(untold.status, MatchStatus::Unscorable);
        assert_eq!(
            (untold.score, untold.note.unwrap().code.as_str()),
            (0, "shortText")
        );
        let check = assess(
            &matcher,
            "Interim CFO",
            "Hamburg",
            &format!("{FIT}\n- Einsatz optional im Rahmen der Arbeitnehmerüberlassung"),
        );
        assert_eq!(check.status, MatchStatus::Scored, "{check:?}");
        assert_eq!(check.note.unwrap().code, "anueOptional");
    }

    #[test]
    fn params_become_scalars() {
        let params = json!({"a": ["DE", "AT"], "b": 3, "c": {"x": 1}, "d": null, "e": [1, true]});
        let flat = flat_params(params.as_object().unwrap());
        assert_eq!(
            Value::Object(flat),
            json!({"a": "DE, AT", "b": 3, "d": null, "e": "1, true"})
        );
        assert_eq!(code_name(&ReasonCode::AvailabilityGap), "availabilityGap");
    }
}
