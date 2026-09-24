//! Scoring jobs against the profile. The engine plugs in through [`Matcher`]; without one
//! nothing is scored and nothing counts as pending.
//!
//! Jobs are scored right when their details arrive (the list shows the ring at once) and in
//! a catch-up step for everything whose score is missing or stale (another revision of
//! engine, profile or model) - in pages of 250, one transaction per page, cancellable.

use jiff::Timestamp;
use tokio_util::sync::CancellationToken;

use super::{RunEvent, ScoreSummary, StatusCode, Step, status};
use crate::model::{MatchRecord, MatchStatus};
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
        Ok(matcher.assess(&job, text.as_deref()))
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
            match matcher.assess(job, text.as_deref()) {
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
