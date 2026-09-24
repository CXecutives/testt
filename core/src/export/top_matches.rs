//! `top_matches.json` in the result folder: the best open scored jobs with what the engine
//! found, for the external matching skill (an optional second stage): unread or saved, not
//! archived, not sent, the alert mail at most 14 days old. It does not depend on the last
//! run, so a fetch without new jobs keeps the list; every run (a rescore too) writes it anew.
//! The file name and the English keys are a contract with the skill - do not rename.

use jiff::Timestamp;
use serde::Serialize;

use crate::error::Result;
use crate::matching::{Assessment, ReasonCode, ReasonKind};
use crate::model::{AppStatus, Band, MatchStatus, band};
use crate::pipeline::Matcher;
use crate::pipeline::local::{code_name, record};
use crate::store::{JobRow, Store};
use crate::view::JobView;

pub const TOP_MATCHES_NAME: &str = "top_matches.json";
/// Most jobs in the file.
pub const TOP_MATCHES_MAX: u32 = 10;
/// Version of the file layout (2: `appStatus` and `firstSeenAt` per job).
pub const TOP_MATCHES_SCHEMA: u32 = 2;

/// The file.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopMatches {
    pub schema: u32,
    pub generated_at: Timestamp,
    /// Revision of engine and profile the jobs were scored with (`null` without a profile).
    pub rev: Option<String>,
    /// Best first; excluded and unscorable jobs are left out.
    pub jobs: Vec<TopMatch>,
}

/// One job.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopMatch {
    /// `portal:id`.
    pub key: String,
    pub title: String,
    pub company: String,
    pub location: String,
    pub portal: String,
    pub url: String,
    pub score: u8,
    pub band: Band,
    pub must_met: u16,
    pub must_total: u16,
    /// The stage in the user's pipeline (`saved`, `applied`, ...; `null` = none).
    pub app_status: Option<AppStatus>,
    /// When the app first saw the job.
    pub first_seen_at: Timestamp,
    /// Requirements of the ad the profile meets (quoted).
    pub met: Vec<String>,
    /// Requirements the profile meets half (a more general entry).
    pub partial: Vec<String>,
    /// Requirements the profile does not meet.
    pub open: Vec<String>,
    /// Codes of the points to check (`anueOptional`, `availabilityGap`, ...).
    pub checks: Vec<String>,
    /// Name of the job's text file in `beschreibungen_txt`, once written.
    pub txt_file: Option<String>,
}

/// The best open scored jobs (see the module), assessed again from their stored text when a
/// matcher can explain them (then its score counts, and the order follows it).
pub fn top_matches(
    store: &Store,
    matcher: Option<&dyn Matcher>,
    now: Timestamp,
) -> Result<TopMatches> {
    let mut jobs = Vec::new();
    // A few more than needed: a fresh assessment can move a job out of the list.
    let since = crate::store::new_since(now);
    for job in store.skill_matches(since, TOP_MATCHES_MAX * 2)? {
        if let Some(entry) = findings(store, matcher, &job)? {
            jobs.push(entry);
        }
    }
    jobs.sort_by_key(|j| std::cmp::Reverse(j.score));
    jobs.truncate(TOP_MATCHES_MAX as usize);
    Ok(TopMatches {
        schema: TOP_MATCHES_SCHEMA,
        generated_at: now,
        rev: matcher.map(|m| m.rev().to_owned()),
        jobs,
    })
}

/// What the app found for one job (score, band, met, partly met, open, to check), assessed
/// again from its stored text when a matcher can explain it; `None` if it is not scored.
/// The AI prompts carry the same findings.
pub fn findings(
    store: &Store,
    matcher: Option<&dyn Matcher>,
    job: &JobRow,
) -> Result<Option<TopMatch>> {
    let explained = match matcher {
        Some(m) => {
            let text = store.description(&job.key)?;
            crate::pipeline::score::guarded(&job.key, || m.explain(job, text.as_deref())).flatten()
        }
        None => None,
    };
    Ok(entry(job, explained.as_ref()))
}

/// The entry of a job; `None` if it is not scored (any more).
fn entry(job: &JobRow, explained: Option<&Assessment>) -> Option<TopMatch> {
    let stored = job.match_.clone()?;
    let fresh = explained.map(record);
    let current = fresh.as_ref().unwrap_or(&stored);
    if current.status != MatchStatus::Scored {
        return None;
    }
    let labels = |kind: ReasonKind| -> Vec<String> {
        explained
            .into_iter()
            .flat_map(|a| a.reasons.iter())
            .filter(|r| {
                r.kind == kind && matches!(r.code, ReasonCode::Requirement | ReasonCode::Term)
            })
            .filter_map(|r| r.label.clone())
            .collect()
    };
    let view = JobView::from(job);
    Some(TopMatch {
        key: job.key.to_string(),
        title: view.title,
        company: view.company,
        location: view.location,
        portal: job.key.portal.key().to_owned(),
        url: job.url.to_string(),
        score: current.score,
        band: band(current.score),
        must_met: current.must_met,
        must_total: current.must_total,
        app_status: job.app_status,
        first_seen_at: job.first_seen_at,
        met: labels(ReasonKind::Met),
        partial: labels(ReasonKind::Partial),
        open: labels(ReasonKind::Open),
        checks: explained
            .into_iter()
            .flat_map(|a| a.reasons.iter())
            .filter(|r| r.kind == ReasonKind::Check)
            .map(|r| code_name(&r.code))
            .collect(),
        txt_file: job.txt_name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file of schema 2 as the app writes it, kept next to the skill's tests: the Python
    /// reader is tested on exactly what this writer produces (the test fails while the
    /// committed file differs - commit the result).
    #[test]
    fn the_skill_reads_what_the_app_writes() {
        let at = |s: &str| s.parse::<Timestamp>().unwrap();
        let job = |key: &str, title: &str, score: u8, app_status| TopMatch {
            key: key.into(),
            title: title.into(),
            company: "Hanseatic Holding GmbH".into(),
            location: "Hamburg".into(),
            portal: key.split(':').next().unwrap_or_default().into(),
            url: format!("https://example.org/{key}"),
            score,
            band: band(score),
            must_met: 3,
            must_total: 4,
            app_status,
            first_seen_at: at("2026-09-20T07:30:00Z"),
            met: vec!["Konzernabschluss nach HGB".into()],
            partial: vec!["Reporting nach IFRS".into()],
            open: vec!["Power BI".into()],
            checks: vec!["availabilityGap".into()],
            txt_file: None,
        };
        let top = TopMatches {
            schema: TOP_MATCHES_SCHEMA,
            generated_at: at("2026-09-24T07:30:00Z"),
            rev: Some("e3:test".into()),
            jobs: vec![
                job(
                    "freelancermap:2801",
                    "Interim CFO",
                    91,
                    Some(AppStatus::Saved),
                ),
                job("linkedin:4100200301", "Head of Controlling", 84, None),
            ],
        };
        let json = format!("{}\n", serde_json::to_string_pretty(&top).unwrap());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tools/job-matching-skill/tests/app_top_matches.json");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(&path, &json).unwrap();
            panic!("regenerated {} - commit it", path.display());
        }
        assert!(json.contains("\"appStatus\": \"saved\"") && json.contains("\"firstSeenAt\""));
    }
}
