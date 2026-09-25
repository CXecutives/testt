//! `top_matches.json` in the result folder: the best open scored jobs with what the engine
//! found, for the external matching skill (an optional second stage): scored (or counted
//! anyway by the user, "Trotzdem passend"), unread or a favourite, in the inbox, the ad still
//! open, the alert mail at most 14 days old. It does not depend on the last run, so a fetch
//! without new jobs keeps the list; every run (a rescore too) writes it anew.
//! The file name and the English keys are a contract with the skill - do not rename.

use jiff::Timestamp;
use serde::Serialize;

use crate::error::Result;
use crate::matching::{Assessment, ReasonCode, ReasonKind};
use crate::model::{Band, MatchStatus, Notice, band};
use crate::pipeline::Matcher;
use crate::pipeline::local::{code_name, flat_params, record};
use crate::store::marks::USER_OVERRIDE;
use crate::store::{JobRow, Store};
use crate::view::JobView;

pub const TOP_MATCHES_NAME: &str = "top_matches.json";
/// Most jobs in the file.
pub const TOP_MATCHES_MAX: u32 = 10;
/// Version of the file layout (2: `appStatus` and `firstSeenAt` per job).
pub const TOP_MATCHES_SCHEMA: u32 = 2;
/// The `appStatus` of a favourite (the only mark there is).
const SAVED: &str = "saved";

/// The file.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopMatches {
    pub schema: u32,
    pub generated_at: Timestamp,
    /// Revision of engine and profile the jobs were scored with (`null` without a profile).
    pub rev: Option<String>,
    /// Best first; unscorable jobs are left out, excluded ones unless the user counts them
    /// anyway.
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
    /// `saved` for a favourite (the star), `null` for none (the key of schema 2 stays).
    pub app_status: Option<String>,
    /// When the app first saw the job.
    pub first_seen_at: Timestamp,
    /// Requirements of the ad the profile meets (quoted).
    pub met: Vec<String>,
    /// Requirements the profile meets half (a more general entry).
    pub partial: Vec<String>,
    /// Requirements the profile does not meet.
    pub open: Vec<String>,
    /// Codes of the points to check (`anueOptional`, `availabilityGap`, ...). A job the user
    /// counts although the engine excludes it has `userOverride` first, then the codes of the
    /// exclusion (`dayRate`, `permanent`, ...).
    pub checks: Vec<String>,
    /// Name of the job's text file in `beschreibungen_txt`, once written.
    pub txt_file: Option<String>,
}

/// What the app found for one job ([`found`]), and why the engine excludes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The findings as the file carries them.
    pub top: TopMatch,
    /// Why the engine excludes the job: its decided violations (code and params), the first
    /// first; empty when it scores the job. The file has no key for it (its jobs count as
    /// scored; `checks` names the codes of one the user counts anyway).
    pub excluded: Vec<Notice>,
    /// The user counts the job although the engine excludes it ("Trotzdem passend").
    pub overridden: bool,
}

impl Found {
    /// The job counts as scored: the engine scores it, or the user counts it anyway.
    pub fn counts(&self) -> bool {
        self.excluded.is_empty() || self.overridden
    }
}

/// The best open scored jobs (see the module), assessed again from their stored text when a
/// matcher can explain them (then its score counts, and the order follows it). A job the
/// user counts anyway is one of them with its fit score, like everywhere in the app.
pub fn top_matches(
    store: &Store,
    matcher: Option<&dyn Matcher>,
    now: Timestamp,
) -> Result<TopMatches> {
    let mut jobs = Vec::new();
    // A few more than needed: a fresh assessment can move a job out of the list.
    let since = crate::store::new_since(now);
    for job in store.skill_matches(since, TOP_MATCHES_MAX * 2)? {
        if let Some(entry) = found(store, matcher, &job)?.filter(Found::counts) {
            jobs.push(entry.top);
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

/// What the app found for a job that counts as scored (score, band, met, partly met, open,
/// to check; see [`Found::counts`]), assessed again from its stored text when a matcher can
/// explain it; `None` otherwise. The AI prompts carry the same findings.
pub fn findings(
    store: &Store,
    matcher: Option<&dyn Matcher>,
    job: &JobRow,
) -> Result<Option<TopMatch>> {
    Ok(found(store, matcher, job)?
        .filter(Found::counts)
        .map(|found| found.top))
}

/// What the app found for one job, an excluded one too (then with why), assessed again from
/// its stored text when a matcher can explain it; `None` if it has no score (not scored yet,
/// or too little text).
pub fn found(store: &Store, matcher: Option<&dyn Matcher>, job: &JobRow) -> Result<Option<Found>> {
    let explained = match matcher {
        Some(m) => {
            let text = store.description(&job.key)?;
            crate::pipeline::score::guarded(&job.key, || m.explain(job, text.as_deref())).flatten()
        }
        None => None,
    };
    Ok(entry(job, explained.as_ref()))
}

/// The entry of a job; `None` if it has no score (any more).
fn entry(job: &JobRow, explained: Option<&Assessment>) -> Option<Found> {
    let stored = job.match_.clone()?;
    let fresh = explained.map(record);
    let current = fresh.as_ref().unwrap_or(&stored);
    let reasons = || explained.into_iter().flat_map(|a| a.reasons.iter());
    // Why the engine excludes the job: its decided violations. Without a fresh assessment
    // the stored note names the first - a job counted anyway is stored as scored and keeps
    // the note of the verdict it overrides.
    let excluded: Vec<Notice> = match current.status {
        MatchStatus::Unscorable => return None,
        MatchStatus::Excluded => {
            let violations: Vec<Notice> = reasons()
                .filter(|r| r.kind == ReasonKind::Violation)
                .map(|r| Notice {
                    code: code_name(&r.code),
                    params: flat_params(&r.params),
                })
                .collect();
            if violations.is_empty() {
                current.note.clone().into_iter().collect()
            } else {
                violations
            }
        }
        MatchStatus::Scored if job.override_include && fresh.is_none() => {
            stored.note.clone().into_iter().collect()
        }
        MatchStatus::Scored => Vec::new(),
    };
    let overridden = job.override_include && !excluded.is_empty();
    let labels = |kind: ReasonKind| -> Vec<String> {
        reasons()
            .filter(|r| {
                r.kind == kind && matches!(r.code, ReasonCode::Requirement | ReasonCode::Term)
            })
            .filter_map(|r| r.label.clone())
            .collect()
    };
    // A job counted anyway: the user's word first, then what the engine excludes it for
    // (like the reader), then the points to check.
    let mut checks = Vec::new();
    if overridden {
        checks.push(USER_OVERRIDE.to_owned());
        checks.extend(excluded.iter().map(|n| n.code.clone()));
    }
    for code in reasons()
        .filter(|r| r.kind == ReasonKind::Check)
        .map(|r| code_name(&r.code))
    {
        if !checks.contains(&code) {
            checks.push(code);
        }
    }
    let view = JobView::from(job);
    let top = TopMatch {
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
        app_status: job.pinned_at.map(|_| SAVED.to_owned()),
        first_seen_at: job.first_seen_at,
        met: labels(ReasonKind::Met),
        partial: labels(ReasonKind::Partial),
        open: labels(ReasonKind::Open),
        checks,
        txt_file: job.txt_name.clone(),
    };
    Some(Found {
        top,
        excluded,
        overridden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Posting;
    use crate::portal::job_link;
    use crate::store::MailRef;

    /// An ad the sample profile of the dry run excludes: the day rate is below its minimum.
    const EXCLUDED_AD: &str = "Wir suchen einen Interim CFO (m/w/d).

Anforderungen:
- Erfahrung im Controlling
- Konzernrechnungslegung nach IFRS
- Kenntnisse in Zollabwicklung

Rahmenbedingungen:
- Tagessatz bis 700 €
- Einsatzort Hamburg";

    /// A store with one unread job of today, its text scored by the engine with the sample
    /// profile of the dry run.
    fn scored(text: &str, now: Timestamp) -> (Store, JobRow) {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let link = job_link("https://www.linkedin.com/jobs/view/4000000009/").unwrap();
        let key = link.key.clone();
        let posting = Posting::new(
            link.key,
            link.url,
            "Interim CFO (m/w/d)",
            "Nordlicht AG",
            "Hamburg",
        );
        let mail = MailRef {
            subject: "Neue Jobs",
            date: None,
            gmail_id: None,
        };
        store.upsert_posting(run, &posting, mail, now).unwrap();
        store.record_text(&key, text, false, false, now).unwrap();
        let matcher = crate::pipeline::demo::matcher();
        let row = store.job(&key).unwrap().unwrap();
        let record = matcher.assess(&row, Some(text)).unwrap();
        store
            .save_matches(&[(key.clone(), record)], matcher.rev(), now)
            .unwrap();
        let row = store.job(&key).unwrap().unwrap();
        (store, row)
    }

    /// An excluded job stays out of the file and has no findings for the prompts yet, but
    /// what the app found says why. Counted anyway ("Trotzdem passend"), it is one of the top
    /// matches like a scored job: with its fit score, the user's word first among the checks
    /// and then the codes of the exclusion, so the skill knows what it looks at. Taken back,
    /// it is out again.
    #[test]
    fn a_job_counted_anyway_is_a_top_match() {
        let now = Timestamp::now();
        let matcher = crate::pipeline::demo::matcher();
        let matcher = Some(&*matcher as &dyn Matcher);
        let (store, row) = scored(EXCLUDED_AD, now);
        assert_eq!(row.match_.as_ref().unwrap().status, MatchStatus::Excluded);
        assert!(top_matches(&store, matcher, now).unwrap().jobs.is_empty());
        assert_eq!(findings(&store, matcher, &row).unwrap(), None);
        let why = found(&store, matcher, &row).unwrap().unwrap();
        assert_eq!(why.excluded[0].code, "dayRate", "{:?}", why.excluded);
        assert!(!why.counts() && !why.top.checks.iter().any(|c| c == USER_OVERRIDE));

        assert!(store.set_override(&row.key, true).unwrap());
        let row = store.job(&row.key).unwrap().unwrap();
        let top = top_matches(&store, matcher, now).unwrap();
        assert_eq!(top.jobs.len(), 1, "{top:?}");
        let job = &top.jobs[0];
        assert_eq!(job.score, row.match_.as_ref().unwrap().score);
        assert_eq!(job.checks[..2], [USER_OVERRIDE, "dayRate"]);
        let json = serde_json::to_value(&top).unwrap();
        assert!(json["jobs"][0].get("excluded").is_none(), "not in the file");
        assert_eq!(findings(&store, matcher, &row).unwrap().as_ref(), Some(job));
        // Without a matcher the stored note still names the exclusion.
        let stored = found(&store, None, &row).unwrap().unwrap();
        assert!(stored.overridden && stored.excluded[0].code == "dayRate");
        assert_eq!(stored.top.checks[..2], [USER_OVERRIDE, "dayRate"]);

        // Taken back, the job is out again (before the engine's verdict is even stored).
        assert!(store.set_override(&row.key, false).unwrap());
        assert!(top_matches(&store, matcher, now).unwrap().jobs.is_empty());
    }

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
                    Some(SAVED.to_owned()),
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
