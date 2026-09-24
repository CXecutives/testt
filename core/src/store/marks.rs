//! Schema 4: what the user keeps about a job - where the application stands, a free note, and
//! "not interesting" (hidden). Runs never touch these columns; a hidden job leaves the lists
//! "Neu" and "Alle" and every count but its own.
//!
//! Every column is nullable, so the migration (in `schema`) is one
//! `ALTER TABLE job ADD COLUMN` per entry of [`SCHEMA_4_JOB_COLUMNS`].

use jiff::Timestamp;
use rusqlite::{OptionalExtension, params};

use super::{Store, bump};
use crate::error::{Error, InvalidInput, Result};
use crate::model::AppStatus;
use crate::portal::JobKey;
use crate::time::to_db;

/// The nullable columns schema 4 adds to the `job` table, as `(name, sql_type)` pairs.
///
/// - `app_status`: `applied`, `interview`, `offer` or `rejected`; `NULL` = no application.
/// - `app_status_at`: when the status was set last (Unix seconds).
/// - `note`: the user's note (at most [`MAX_NOTE_CHARS`] characters); `NULL` = none.
/// - `hidden_at`: when the user hid the job ("not interesting"); `NULL` = listed.
pub const SCHEMA_4_JOB_COLUMNS: &[(&str, &str)] = &[
    ("app_status", "TEXT"),
    ("app_status_at", "INTEGER"),
    ("note", "TEXT"),
    ("hidden_at", "INTEGER"),
];

/// Longest note in characters.
pub const MAX_NOTE_CHARS: usize = 2000;

impl Store {
    /// Sets or clears the application status; `true` if it changed. A new status takes the
    /// time of the change; clearing it clears the time too. The Excel file shows the status,
    /// so a change is a visible one.
    pub fn set_app_status(
        &self,
        key: &JobKey,
        status: Option<AppStatus>,
        now: Timestamp,
    ) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET app_status = ?3, app_status_at = CASE WHEN ?3 IS NULL THEN NULL
                                                                   ELSE ?4 END
                 WHERE portal = ?1 AND job_id = ?2 AND app_status IS NOT ?3",
                params![
                    key.portal.key(),
                    key.id,
                    status.map(AppStatus::as_str),
                    to_db(now)
                ],
            )? > 0;
            if changed {
                bump(conn)?;
            }
            Ok(changed)
        })
    }

    /// Stores the note of a job (blank = none); `true` if it changed. The note is the user's
    /// alone: no export shows it.
    pub fn set_note(&self, key: &JobKey, note: &str) -> Result<bool> {
        let chars = note.chars().count();
        if chars > MAX_NOTE_CHARS {
            return Err(Error::Invalid(InvalidInput::NoteTooLong {
                max: MAX_NOTE_CHARS,
            }));
        }
        let note = (!note.trim().is_empty()).then_some(note);
        Ok(self.conn().execute(
            "UPDATE job SET note = ?3 WHERE portal = ?1 AND job_id = ?2 AND note IS NOT ?3",
            params![key.portal.key(), key.id, note],
        )? > 0)
    }

    /// The note of a job, if it has one.
    pub fn note(&self, key: &JobKey) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT note FROM job WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// Hides a job ("not interesting") or lists it again; `true` if something changed. The
    /// HTML overview and the skill's top matches leave hidden jobs out: a visible change.
    pub fn set_hidden(&self, key: &JobKey, hidden: bool, now: Timestamp) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET hidden_at = CASE WHEN ?3 THEN COALESCE(hidden_at, ?4) END
                 WHERE portal = ?1 AND job_id = ?2 AND (hidden_at IS NULL) = ?3",
                params![key.portal.key(), key.id, hidden, to_db(now)],
            )? > 0;
            if changed {
                bump(conn)?;
            }
            Ok(changed)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::{mail, now, posting};

    fn store_with_job() -> (Store, JobKey) {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let p = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        store.upsert_posting(run, &p, mail(), now()).unwrap();
        (store, p.key)
    }

    #[test]
    fn the_application_status_keeps_the_time_of_its_change() {
        let (store, key) = store_with_job();
        let rev = store.data_rev().unwrap();
        let at = now();
        assert!(
            store
                .set_app_status(&key, Some(AppStatus::Applied), at)
                .unwrap()
        );
        let job = store.job(&key).unwrap().unwrap();
        assert_eq!(
            (job.app_status, job.app_status_at),
            (Some(AppStatus::Applied), Some(at))
        );
        assert!(store.data_rev().unwrap() > rev, "the Excel file shows it");
        // The same status again changes nothing, not even the time.
        let later = at + jiff::SignedDuration::from_hours(1);
        assert!(
            !store
                .set_app_status(&key, Some(AppStatus::Applied), later)
                .unwrap()
        );
        assert!(
            store
                .set_app_status(&key, Some(AppStatus::Interview), later)
                .unwrap()
        );
        let job = store.job(&key).unwrap().unwrap();
        assert_eq!(
            (job.app_status, job.app_status_at),
            (Some(AppStatus::Interview), Some(later))
        );
        assert!(store.set_app_status(&key, None, later).unwrap());
        let job = store.job(&key).unwrap().unwrap();
        assert_eq!((job.app_status, job.app_status_at), (None, None));
    }

    #[test]
    fn a_note_is_stored_blank_clears_it_and_it_has_a_limit() {
        let (store, key) = store_with_job();
        assert_eq!(store.note(&key).unwrap(), None);
        assert!(store.set_note(&key, "Rückruf am Montag").unwrap());
        assert_eq!(
            store.note(&key).unwrap().as_deref(),
            Some("Rückruf am Montag")
        );
        assert!(!store.set_note(&key, "Rückruf am Montag").unwrap());
        assert!(store.set_note(&key, "  \n ").unwrap());
        assert_eq!(store.note(&key).unwrap(), None);
        let longest = "ä".repeat(MAX_NOTE_CHARS);
        assert!(store.set_note(&key, &longest).unwrap());
        let too_long = format!("{longest}x");
        assert!(matches!(
            store.set_note(&key, &too_long),
            Err(Error::Invalid(InvalidInput::NoteTooLong {
                max: MAX_NOTE_CHARS
            }))
        ));
        assert_eq!(store.note(&key).unwrap(), Some(longest));
    }

    #[test]
    fn hiding_keeps_its_first_time_and_can_be_undone() {
        let (store, key) = store_with_job();
        let at = now();
        assert!(store.set_hidden(&key, true, at).unwrap());
        let later = at + jiff::SignedDuration::from_hours(1);
        assert!(!store.set_hidden(&key, true, later).unwrap());
        assert_eq!(store.job(&key).unwrap().unwrap().hidden_at, Some(at));
        assert!(store.set_hidden(&key, false, later).unwrap());
        assert_eq!(store.job(&key).unwrap().unwrap().hidden_at, None);
        assert!(!store.set_hidden(&key, false, later).unwrap());
    }

    /// "Not interesting" also leaves the HTML overview (new and pinned) and the skill's top
    /// matches; listed again, the job is back.
    #[test]
    fn a_hidden_job_leaves_the_overview_and_the_top_matches() {
        let (store, key) = store_with_job();
        let run = store.job(&key).unwrap().unwrap().first_seen_run;
        let scored = crate::model::MatchRecord {
            status: crate::model::MatchStatus::Scored,
            score: 88,
            note: None,
            must_met: 3,
            must_total: 3,
            top: Vec::new(),
            facts: crate::model::KeyFacts::default(),
        };
        store
            .save_matches(&[(key.clone(), scored)], "r", now())
            .unwrap();
        let keys = |jobs: Vec<crate::store::JobRow>| -> Vec<JobKey> {
            jobs.into_iter().map(|j| j.key).collect()
        };
        assert_eq!(
            keys(store.top_matches(run, 5).unwrap()),
            std::slice::from_ref(&key)
        );
        assert_eq!(
            keys(store.overview_jobs(run).unwrap().0),
            std::slice::from_ref(&key)
        );
        store.set_hidden(&key, true, now()).unwrap();
        assert!(store.top_matches(run, 5).unwrap().is_empty());
        assert!(store.overview_jobs(run).unwrap().0.is_empty());
        // Pinned and hidden: hidden wins, the overview falls back to the (empty) new list.
        store.set_pinned(&key, true, now()).unwrap();
        assert_eq!(store.overview_jobs(run).unwrap(), (Vec::new(), false));
        store.set_hidden(&key, false, now()).unwrap();
        assert_eq!(
            keys(store.overview_jobs(run).unwrap().0),
            std::slice::from_ref(&key)
        );
        assert_eq!(keys(store.top_matches(run, 5).unwrap()), [key]);
    }

    /// The best matches for an AI chat: pinned first, then by score; never excluded, hidden,
    /// unscored or gone.
    #[test]
    fn the_best_matches_put_the_pinned_first_and_leave_out_the_rest() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let mut keys = Vec::new();
        for i in 1..=6 {
            let p = posting(
                &format!("https://www.linkedin.com/jobs/view/400000000{i}/"),
                &format!("Job {i}"),
                "",
                "",
            );
            store.upsert_posting(run, &p, mail(), now()).unwrap();
            keys.push(p.key);
        }
        let record = |status, score| crate::model::MatchRecord {
            status,
            score,
            note: None,
            must_met: 1,
            must_total: 1,
            top: Vec::new(),
        };
        store
            .save_matches(
                &[
                    (
                        keys[0].clone(),
                        record(crate::model::MatchStatus::Scored, 60),
                    ),
                    (
                        keys[1].clone(),
                        record(crate::model::MatchStatus::Scored, 90),
                    ),
                    (
                        keys[2].clone(),
                        record(crate::model::MatchStatus::Excluded, 99),
                    ),
                    (
                        keys[3].clone(),
                        record(crate::model::MatchStatus::Scored, 95),
                    ),
                    (
                        keys[4].clone(),
                        record(crate::model::MatchStatus::Scored, 80),
                    ),
                ],
                "r",
                now(),
            )
            .unwrap();
        // Job 6 has no score; job 4 is hidden; job 1 is pinned.
        store.set_hidden(&keys[3], true, now()).unwrap();
        store.set_pinned(&keys[0], true, now()).unwrap();
        let titles = |limit| -> Vec<String> {
            store
                .best_matches(limit)
                .unwrap()
                .into_iter()
                .map(|j| j.title)
                .collect()
        };
        assert_eq!(titles(5), ["Job 1", "Job 2", "Job 5"]);
        assert_eq!(titles(2), ["Job 1", "Job 2"]);
        store.record_gone(&keys[1], now()).unwrap();
        assert_eq!(
            titles(5),
            ["Job 1", "Job 5"],
            "an ad no longer online is out"
        );
    }

    #[test]
    fn an_unknown_job_changes_nothing() {
        let (store, _) = store_with_job();
        let other = crate::portal::job_link("https://www.linkedin.com/jobs/view/4000000009/")
            .unwrap()
            .key;
        assert!(
            !store
                .set_app_status(&other, Some(AppStatus::Offer), now())
                .unwrap()
        );
        assert!(!store.set_note(&other, "x").unwrap());
        assert!(!store.set_hidden(&other, true, now()).unwrap());
        assert_eq!(store.note(&other).unwrap(), None);
    }
}
