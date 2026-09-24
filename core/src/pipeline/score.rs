//! Scoring jobs against the profile. The engine plugs in through [`Matcher`]; without one
//! nothing is scored and nothing counts as pending.
//!
//! Jobs are scored right when their details arrive (the list shows the ring at once) and in
//! a catch-up step for everything whose score is missing or stale (another revision of
//! engine, profile or model) - in pages of 250, one transaction per page, cancellable.

use jiff::Timestamp;
use tokio_util::sync::CancellationToken;

use super::{RunEvent, ScoreSummary, StatusCode, Step, status};
use crate::matching::Assessment;
use crate::model::{MatchRecord, MatchStatus, Notice};
use crate::portal::JobKey;
use crate::store::{JobRow, Store};

/// Jobs per catch-up page (one transaction each).
pub const PAGE: u32 = 250;

/// Scores jobs. Pure and synchronous; the pipeline stores what it says.
pub trait Matcher: Send + Sync {
    /// Revision of engine, profile and model - a stored score of another revision is stale.
    fn rev(&self) -> &str;
    /// The match of one job (`text`: its full text, if fetched); `None` = no judgement, the
    /// job stays pending.
    fn assess(&self, job: &JobRow, text: Option<&str>) -> Option<MatchRecord>;
    /// The full assessment behind [`Matcher::assess`] (reasons for the exports); `None` if
    /// the matcher has none.
    fn explain(&self, job: &JobRow, text: Option<&str>) -> Option<Assessment> {
        let _ = (job, text);
        None
    }
}

/// Counters of the scoring in one run.
#[derive(Debug, Default)]
pub(super) struct Tally {
    scored: usize,
    excluded: usize,
    unscorable: usize,
    best: Option<u8>,
}

impl Tally {
    fn count(&mut self, record: &MatchRecord) {
        match record.status {
            MatchStatus::Scored => {
                self.scored += 1;
                self.best = self.best.max(Some(record.score));
            }
            MatchStatus::Excluded => self.excluded += 1,
            MatchStatus::Unscorable => self.unscorable += 1,
        }
    }

    pub(super) fn summary(&self, pending: usize) -> ScoreSummary {
        ScoreSummary {
            scored: self.scored,
            excluded: self.excluded,
            unscorable: self.unscorable,
            pending,
            best: self.best,
        }
    }
}

/// Note code of a job the engine failed on (a panic): unscorable, never retried with the
/// same revision, and the run goes on.
pub const ENGINE_FAILED: &str = "engineFailed";

/// The record of a job the engine failed on.
pub fn engine_failed() -> MatchRecord {
    MatchRecord {
        status: MatchStatus::Unscorable,
        score: 0,
        note: Some(Notice {
            code: ENGINE_FAILED.to_owned(),
            params: serde_json::Map::new(),
        }),
        must_met: 0,
        must_total: 0,
        top: Vec::new(),
        facts: crate::model::KeyFacts::default(),
    }
}

/// Runs `judge` for one job; a panic inside is caught and logged with the job key only (never
/// the ad text) - `None` then.
pub(crate) fn guarded<T>(key: &JobKey, judge: impl FnOnce() -> T) -> Option<T> {
    let judged = std::panic::catch_unwind(std::panic::AssertUnwindSafe(judge)).ok();
    if judged.is_none() {
        log::error!("{key}: the engine failed on this job; it stays unscorable");
    }
    judged
}

/// [`Matcher::assess`] that survives a panic of the engine: the job becomes unscorable
/// with [`ENGINE_FAILED`].
fn assess_guarded(matcher: &dyn Matcher, job: &JobRow, text: Option<&str>) -> Option<MatchRecord> {
    guarded(&job.key, || matcher.assess(job, text)).unwrap_or_else(|| Some(engine_failed()))
}

/// Scores one job right after its details changed. Errors only go to the log - the fetch
/// goes on, and the catch-up step tries again.
pub(super) fn score_one(
    store: &Store,
    matcher: &dyn Matcher,
    key: &JobKey,
    now: Timestamp,
    tally: &mut Tally,
) {
    let assessed = store.job(key).and_then(|job| {
        let Some(job) = job else { return Ok(None) };
        let text = store.description(key)?;
        Ok(assess_guarded(matcher, &job, text.as_deref()))
    });
    match assessed {
        Ok(Some(record)) => {
            match store.save_matches(&[(key.clone(), record.clone())], matcher.rev(), now) {
                Ok(()) => tally.count(&record),
                Err(e) => log::warn!("score of {key} not stored: {e}"),
            }
        }
        Ok(None) => {}
        Err(e) => log::warn!("{key} not scored: {e}"),
    }
}

/// Scores every job whose score is missing or stale; `false` if cancelled.
pub(super) fn catch_up(
    store: &Store,
    matcher: &dyn Matcher,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    tally: &mut Tally,
    emit: &mut impl FnMut(RunEvent),
) -> crate::Result<bool> {
    let total = usize::try_from(store.match_pending(matcher.rev())?).unwrap_or(0);
    if total == 0 {
        return Ok(true);
    }
    emit(status(StatusCode::Scoring, None, None));
    // Jobs the matcher does not judge stay pending; they are skipped, not asked again.
    let mut skipped = 0;
    let mut done = 0;
    loop {
        if cancel.is_cancelled() {
            return Ok(false);
        }
        let page = store.unscored(matcher.rev(), PAGE, skipped)?;
        if page.is_empty() {
            return Ok(true);
        }
        let mut records = Vec::with_capacity(page.len());
        for (job, text) in &page {
            match assess_guarded(matcher, job, text.as_deref()) {
                Some(record) => records.push((job.key.clone(), record)),
                None => skipped += 1,
            }
        }
        store.save_matches(&records, matcher.rev(), clock())?;
        for (_, record) in &records {
            tally.count(record);
        }
        done += page.len();
        emit(RunEvent::Progress {
            step: Step::Score,
            portal: None,
            done: done.min(total),
            total,
        });
    }
}
