//! `top_matches.json` in the result folder: the best scored jobs of the last mailbox run with
//! what the engine found, for the external matching skill (an optional second stage). The
//! file name and the English keys are a contract with the skill - do not rename.

use jiff::Timestamp;
use serde::Serialize;

use crate::error::Result;
use crate::matching::{Assessment, ReasonCode, ReasonKind};
use crate::model::{Band, MatchStatus, band};
use crate::pipeline::Matcher;
use crate::pipeline::local::{code_name, record};
use crate::store::{JobRow, Store};
use crate::view::JobView;

pub const TOP_MATCHES_NAME: &str = "top_matches.json";
/// Most jobs in the file.
pub const TOP_MATCHES_MAX: u32 = 10;
/// Version of the file layout.
const SCHEMA: u32 = 1;

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

/// The best scored jobs first seen in `run`, assessed again from their stored text when a
/// matcher can explain them (then its score counts, and the order follows it).
pub fn top_matches(
    store: &Store,
    matcher: Option<&dyn Matcher>,
    run: i64,
    now: Timestamp,
) -> Result<TopMatches> {
    let mut jobs = Vec::new();
    // A few more than needed: a fresh assessment can move a job out of the list.
    for job in store.top_matches(run, TOP_MATCHES_MAX * 2)? {
        let explained = match matcher {
            Some(m) => {
                let text = store.description(&job.key)?;
                crate::pipeline::score::guarded(&job.key, || m.explain(&job, text.as_deref()))
                    .flatten()
            }
            None => None,
        };
        if let Some(entry) = entry(&job, explained.as_ref()) {
            jobs.push(entry);
        }
    }
    jobs.sort_by_key(|j| std::cmp::Reverse(j.score));
    jobs.truncate(TOP_MATCHES_MAX as usize);
    Ok(TopMatches {
        schema: SCHEMA,
        generated_at: now,
        rev: matcher.map(|m| m.rev().to_owned()),
        jobs,
    })
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
