//! What the user keeps about a job - one mark (saved, the star, or "Beworben") with
//! the time it was set, a free note, "archived", "fits anyway" - plus reading in bulk and
//! deleting for good. Runs never touch these columns (only the age archives a job without a
//! mark); an archived job leaves every list and count but its own.
//!
//! Every column is nullable, so the migrations (in `schema`) add them with
//! `ALTER TABLE job ADD COLUMN`: schema 4 the first marks, schema 5 the rest.

use jiff::Timestamp;
use rusqlite::{OptionalExtension, params};

use super::{ListFacet, Store, bump};
use crate::error::{Error, InvalidInput, Result};
use crate::model::AppStatus;
use crate::portal::JobKey;
use crate::time::to_db;

/// The nullable columns schema 4 added to the `job` table, as `(name, sql_type)` pairs.
/// Frozen: schema 5 renamed `hidden_at` to `archived_at`.
///
/// - `app_status`: the mark, `saved` or `sent` since schema 5 (schema 4 stored `applied`,
///   `interview`, `offer` and `rejected`); `NULL` = none.
/// - `app_status_at`: when the mark was set last (Unix seconds).
/// - `note`: the user's note (at most [`MAX_NOTE_CHARS`] characters); `NULL` = none.
/// - `hidden_at`, now `archived_at`: when the job was archived (by the user or by age);
///   `NULL` = listed.
pub const SCHEMA_4_JOB_COLUMNS: &[(&str, &str)] = &[
    ("app_status", "TEXT"),
    ("app_status_at", "INTEGER"),
    ("note", "TEXT"),
    ("hidden_at", "INTEGER"),
];

/// The nullable columns schema 5 adds to the `job` table.
///
/// - `override_include`: `1` = the user marked an excluded job as fitting anyway; the
///   engine's exclusion is then stored as "scored" (with the fit score) on every rescore.
/// - `mail_version`: the mail parser that read title, company and location
///   (`mail::MAIL_PARSER_VERSION`); `NULL` = one before the versions.
pub const SCHEMA_5_JOB_COLUMNS: &[(&str, &str)] =
    &[("override_include", "INTEGER"), ("mail_version", "INTEGER")];

/// What the migration to schema 5 does beyond the new columns: "hidden" is "archived" now,
/// every application status is "sent" (one mark instead of four stages), a pinned job
/// without a mark is saved, and the table of deleted job keys (a later scan of an old alert
/// mail never brings them back).
pub const SCHEMA_5_EXTRA: &str = "ALTER TABLE job RENAME COLUMN hidden_at TO archived_at;
UPDATE job SET app_status = 'sent'
 WHERE app_status IN ('applied', 'interview', 'offer', 'rejected');
UPDATE job SET app_status = 'saved', app_status_at = pinned_at
 WHERE pinned_at IS NOT NULL AND app_status IS NULL;
CREATE TABLE tombstone (
    portal      TEXT    NOT NULL,
    job_id      TEXT    NOT NULL,
    deleted_at  INTEGER NOT NULL,
    PRIMARY KEY (portal, job_id)
) WITHOUT ROWID;";

/// The note code of a job the user marked as fitting although the engine excludes it.
pub const USER_OVERRIDE: &str = "userOverride";

/// Longest note in characters.
pub const MAX_NOTE_CHARS: usize = 2000;

impl Store {
    /// Sets or clears the mark of a job; `true` if it changed. A new mark takes the time of
    /// the change; clearing clears both. The Excel file shows "Beworben am", so a change
    /// is a visible one.
    pub fn set_app_status(
        &self,
        key: &JobKey,
        status: Option<AppStatus>,
        now: Timestamp,
    ) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET app_status = ?3,
                                app_status_at = CASE WHEN ?3 IS NULL THEN NULL ELSE ?4 END
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

    /// The star, a thin alias of the mark "saved": on saves a job without a mark ("sent"
    /// stays), off clears only "saved". `true` if something changed.
    pub fn set_pinned(&self, key: &JobKey, on: bool, now: Timestamp) -> Result<bool> {
        let mark = self.job(key)?.and_then(|job| job.app_status);
        match (on, mark) {
            (true, None) => self.set_app_status(key, Some(AppStatus::Saved), now),
            (false, Some(AppStatus::Saved)) => self.set_app_status(key, None, now),
            _ => Ok(false),
        }
    }

    /// Stores the note of a job (blank = none); `true` if it changed. The Excel file shows
    /// it, so a change is a visible one.
    pub fn set_note(&self, key: &JobKey, note: &str) -> Result<bool> {
        let chars = note.chars().count();
        if chars > MAX_NOTE_CHARS {
            return Err(Error::Invalid(InvalidInput::NoteTooLong {
                max: MAX_NOTE_CHARS,
            }));
        }
        let note = (!note.trim().is_empty()).then_some(note);
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET note = ?3 WHERE portal = ?1 AND job_id = ?2 AND note IS NOT ?3",
                params![key.portal.key(), key.id, note],
            )? > 0;
            if changed {
                bump(conn)?;
            }
            Ok(changed)
        })
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

    /// Archives a job or lists it again; `true` if something changed. The
    /// HTML overview and the skill's top matches leave archived jobs out: a visible change.
    pub fn set_archived(&self, key: &JobKey, archived: bool, now: Timestamp) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET archived_at = CASE WHEN ?3 THEN COALESCE(archived_at, ?4) END
                 WHERE portal = ?1 AND job_id = ?2 AND (archived_at IS NULL) = ?3",
                params![key.portal.key(), key.id, archived, to_db(now)],
            )? > 0;
            if changed {
                bump(conn)?;
            }
            Ok(changed)
        })
    }

    /// Marks every unread job the list of `facet` holds as read ("Neu": the last
    /// [`super::NEW_DAYS`] days) and returns their keys - the page can undo it with
    /// [`Store::mark_unread`]. Reading exports nothing: no change counter.
    pub fn mark_all_read(&self, facet: ListFacet, now: Timestamp) -> Result<Vec<JobKey>> {
        let since = to_db(super::new_since(now));
        self.write(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT portal, job_id FROM job
                 WHERE dup_of IS NULL AND read_at IS NULL AND {}
                 ORDER BY portal, job_id",
                facet.condition(since)
            ))?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut keys = Vec::new();
            for row in rows {
                let (portal, id) = row?;
                if let Some(portal) = crate::portal::Portal::from_key(&portal) {
                    keys.push(JobKey { portal, id });
                }
            }
            let mut mark = conn
                .prepare_cached("UPDATE job SET read_at = ?3 WHERE portal = ?1 AND job_id = ?2")?;
            for key in &keys {
                mark.execute(params![key.portal.key(), key.id, to_db(now)])?;
            }
            Ok(keys)
        })
    }

    /// Archives the jobs first seen before `before` that have no mark (neither saved nor
    /// sent) - "old jobs archive themselves" at the end of a run; returns how many.
    pub fn auto_archive(&self, before: Timestamp, now: Timestamp) -> Result<usize> {
        self.write(|conn| {
            let archived = conn.execute(
                "UPDATE job SET archived_at = ?2
                 WHERE archived_at IS NULL AND app_status IS NULL AND first_seen_at < ?1",
                params![to_db(before), to_db(now)],
            )?;
            if archived > 0 {
                bump(conn)?;
            }
            Ok(archived)
        })
    }

    /// The keys of every archived job ("empty the archive").
    pub fn archived_keys(&self) -> Result<Vec<JobKey>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT portal, job_id FROM job WHERE archived_at IS NOT NULL ORDER BY portal, job_id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut keys = Vec::new();
        for row in rows {
            let (portal, id) = row?;
            if let Some(portal) = crate::portal::Portal::from_key(&portal) {
                keys.push(JobKey { portal, id });
            }
        }
        Ok(keys)
    }

    /// Deletes jobs for good (with the duplicates that stand for them): their rows go, only a
    /// tombstone of each key stays, so a scan never imports them again from an old alert
    /// mail. Returns how many rows went and the names of their text files (the caller
    /// removes the files).
    pub fn delete_jobs(&self, keys: &[JobKey], now: Timestamp) -> Result<(usize, Vec<String>)> {
        self.write(|conn| {
            let mut doomed: Vec<(String, String)> = Vec::new();
            {
                let mut dups = conn.prepare_cached(
                    "SELECT portal, job_id FROM job WHERE dup_of = ?1 OR (portal = ?2 AND job_id = ?3)",
                )?;
                for key in keys {
                    let rows = dups.query_map(
                        params![key.to_string(), key.portal.key(), key.id],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    )?;
                    for row in rows {
                        let row = row?;
                        if !doomed.contains(&row) {
                            doomed.push(row);
                        }
                    }
                }
            }
            let mut names = Vec::new();
            let mut name = conn.prepare_cached(
                "SELECT txt_name FROM job WHERE portal = ?1 AND job_id = ?2 AND txt_name IS NOT NULL",
            )?;
            let mut tomb = conn.prepare_cached(
                "INSERT OR REPLACE INTO tombstone (portal, job_id, deleted_at) VALUES (?1, ?2, ?3)",
            )?;
            let mut delete =
                conn.prepare_cached("DELETE FROM job WHERE portal = ?1 AND job_id = ?2")?;
            for (portal, id) in &doomed {
                if let Some(file) = name
                    .query_row(params![portal, id], |r| r.get::<_, String>(0))
                    .optional()?
                {
                    names.push(file);
                }
                tomb.execute(params![portal, id, to_db(now)])?;
                delete.execute(params![portal, id])?;
            }
            if !doomed.is_empty() {
                bump(conn)?;
            }
            Ok((doomed.len(), names))
        })
    }

    /// Was this job deleted for good?
    pub fn is_deleted(&self, key: &JobKey) -> Result<bool> {
        Ok(self
            .conn()
            .query_row(
                "SELECT 1 FROM tombstone WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// "Fits anyway": the user includes an excluded job (it counts as scored with its fit
    /// score, and every rescore keeps it so) or takes that back (`false`; the caller stores
    /// the engine's verdict again). `true` if the mark changed.
    pub fn set_override(&self, key: &JobKey, include: bool) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET override_include = CASE WHEN ?3 THEN 1 END,
                                match_status = CASE WHEN ?3 AND match_status = 'excluded'
                                                    THEN 'scored' ELSE match_status END
                 WHERE portal = ?1 AND job_id = ?2 AND (override_include IS NULL) = ?3",
                params![key.portal.key(), key.id, include],
            )? > 0;
            if changed {
                if !include {
                    // The engine's verdict is due again (the caller assesses it right away).
                    conn.execute(
                        "UPDATE job SET match_rev = NULL WHERE portal = ?1 AND job_id = ?2",
                        params![key.portal.key(), key.id],
                    )?;
                }
                bump(conn)?;
            }
            Ok(changed)
        })
    }

    /// Marks these jobs unread again (the undo of [`Store::mark_all_read`]); returns how many
    /// were read.
    pub fn mark_unread(&self, keys: &[JobKey]) -> Result<usize> {
        self.write(|conn| {
            let mut stmt = conn.prepare_cached(
                "UPDATE job SET read_at = NULL
                 WHERE portal = ?1 AND job_id = ?2 AND read_at IS NOT NULL",
            )?;
            let mut changed = 0;
            for key in keys {
                changed += stmt.execute(params![key.portal.key(), key.id])?;
            }
            Ok(changed)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MatchStatus;
    use crate::store::Seen;
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
    fn the_mark_keeps_the_time_it_was_set() {
        let (store, key) = store_with_job();
        let rev = store.data_rev().unwrap();
        let at = now();
        assert!(
            store
                .set_app_status(&key, Some(AppStatus::Sent), at)
                .unwrap()
        );
        let job = store.job(&key).unwrap().unwrap();
        assert_eq!(
            (job.app_status, job.app_status_at),
            (Some(AppStatus::Sent), Some(at))
        );
        assert!(store.data_rev().unwrap() > rev, "the Excel file shows it");
        // The same mark again changes nothing, not even the time.
        let later = at + jiff::SignedDuration::from_hours(1);
        assert!(
            !store
                .set_app_status(&key, Some(AppStatus::Sent), later)
                .unwrap()
        );
        assert_eq!(store.job(&key).unwrap().unwrap().app_status_at, Some(at));
        assert!(store.set_app_status(&key, None, later).unwrap());
        let job = store.job(&key).unwrap().unwrap();
        assert_eq!((job.app_status, job.app_status_at), (None, None));
    }

    /// The star is the mark "saved": it never overwrites "sent", and taking it off clears
    /// only "saved".
    #[test]
    fn the_star_never_overwrites_sent() {
        let (store, key) = store_with_job();
        assert!(store.set_pinned(&key, true, now()).unwrap());
        assert!(!store.set_pinned(&key, true, now()).unwrap());
        assert_eq!(
            store.job(&key).unwrap().unwrap().app_status,
            Some(AppStatus::Saved)
        );
        assert!(store.set_pinned(&key, false, now()).unwrap());
        assert_eq!(store.job(&key).unwrap().unwrap().app_status, None);
        store
            .set_app_status(&key, Some(AppStatus::Sent), now())
            .unwrap();
        assert!(!store.set_pinned(&key, true, now()).unwrap());
        assert!(!store.set_pinned(&key, false, now()).unwrap());
        assert_eq!(
            store.job(&key).unwrap().unwrap().app_status,
            Some(AppStatus::Sent)
        );
    }

    #[test]
    fn a_note_is_stored_blank_clears_it_and_it_has_a_limit() {
        let (store, key) = store_with_job();
        let rev = store.data_rev().unwrap();
        assert_eq!(store.note(&key).unwrap(), None);
        assert!(store.set_note(&key, "Rückruf am Montag").unwrap());
        assert!(store.data_rev().unwrap() > rev, "the Excel file shows it");
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
    fn archiving_keeps_its_first_time_and_can_be_undone() {
        let (store, key) = store_with_job();
        let at = now();
        assert!(store.set_archived(&key, true, at).unwrap());
        let later = at + jiff::SignedDuration::from_hours(1);
        assert!(!store.set_archived(&key, true, later).unwrap());
        assert_eq!(store.job(&key).unwrap().unwrap().archived_at, Some(at));
        assert!(store.set_archived(&key, false, later).unwrap());
        assert_eq!(store.job(&key).unwrap().unwrap().archived_at, None);
        assert!(!store.set_archived(&key, false, later).unwrap());
    }

    /// "All read" marks exactly the unread jobs of the list and hands their keys back; the
    /// undo makes exactly those unread again.
    #[test]
    fn all_read_and_its_undo() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let mut keys = Vec::new();
        for i in 1..=4 {
            let p = posting(
                &format!("https://www.linkedin.com/jobs/view/400000000{i}/"),
                &format!("Job {i}"),
                "",
                "",
            );
            store.upsert_posting(run, &p, mail(), now()).unwrap();
            keys.push(p.key);
        }
        store.mark_read(&keys[0], now()).unwrap();
        store.set_archived(&keys[1], true, now()).unwrap();
        let rev = store.data_rev().unwrap();
        let marked = store.mark_all_read(ListFacet::New, now()).unwrap();
        assert_eq!(marked, [keys[2].clone(), keys[3].clone()]);
        assert_eq!(store.data_rev().unwrap(), rev, "reading exports nothing");
        assert!(store.job(&keys[1]).unwrap().unwrap().read_at.is_none());
        assert!(
            store
                .mark_all_read(ListFacet::New, now())
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.mark_unread(&marked).unwrap(), 2);
        for key in &marked {
            assert!(store.job(key).unwrap().unwrap().read_at.is_none());
        }
        assert!(store.job(&keys[0]).unwrap().unwrap().read_at.is_some());
    }

    /// "Not interesting" also leaves the HTML overview (new and saved) and the skill's top
    /// matches; listed again, the job is back.
    #[test]
    fn an_archived_job_leaves_the_overview_and_the_top_matches() {
        let (store, key) = store_with_job();
        let run = store.job(&key).unwrap().unwrap().first_seen_run;
        let scored = crate::model::MatchRecord {
            status: MatchStatus::Scored,
            score: 88,
            note: None,
            must_met: 3,
            must_total: 3,
            top: Vec::new(),
            facts: crate::model::KeyFacts::default(),
            rank: 0,
        };
        store
            .save_matches(&[(key.clone(), scored)], "r", now())
            .unwrap();
        let keys = |jobs: Vec<crate::store::JobRow>| -> Vec<JobKey> {
            jobs.into_iter().map(|j| j.key).collect()
        };
        let since = super::super::new_since(now());
        assert_eq!(
            keys(store.skill_matches(since, 5).unwrap()),
            std::slice::from_ref(&key)
        );
        assert_eq!(
            keys(store.overview_jobs(run).unwrap().0),
            std::slice::from_ref(&key)
        );
        store.set_archived(&key, true, now()).unwrap();
        assert!(store.skill_matches(since, 5).unwrap().is_empty());
        assert!(store.overview_jobs(run).unwrap().0.is_empty());
        // Saved and archived: archived wins, the overview falls back to the (empty) new list.
        store.set_pinned(&key, true, now()).unwrap();
        assert_eq!(store.overview_jobs(run).unwrap(), (Vec::new(), false));
        store.set_archived(&key, false, now()).unwrap();
        assert_eq!(
            keys(store.overview_jobs(run).unwrap().0),
            std::slice::from_ref(&key)
        );
        assert_eq!(
            keys(store.skill_matches(since, 5).unwrap()),
            std::slice::from_ref(&key)
        );
    }

    /// The best matches for an AI chat: saved first, then the best open ones (no mark) by
    /// score; never excluded, archived, unscored, gone or sent.
    #[test]
    fn the_best_matches_put_the_saved_first_and_leave_out_the_rest() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let mut keys = Vec::new();
        for i in 1..=7 {
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
            facts: crate::model::KeyFacts::default(),
            rank: 0,
        };
        store
            .save_matches(
                &[
                    (keys[0].clone(), record(MatchStatus::Scored, 60)),
                    (keys[1].clone(), record(MatchStatus::Scored, 90)),
                    (keys[2].clone(), record(MatchStatus::Excluded, 99)),
                    (keys[3].clone(), record(MatchStatus::Scored, 95)),
                    (keys[4].clone(), record(MatchStatus::Scored, 80)),
                    (keys[6].clone(), record(MatchStatus::Scored, 97)),
                ],
                "r",
                now(),
            )
            .unwrap();
        // Job 6 has no score; job 4 is archived; job 1 is saved; job 7 is sent.
        store.set_archived(&keys[3], true, now()).unwrap();
        store.set_pinned(&keys[0], true, now()).unwrap();
        store
            .set_app_status(&keys[6], Some(AppStatus::Sent), now())
            .unwrap();
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

    /// A deleted job leaves only its tombstone: the row, its duplicate and its text file
    /// name go, and the same link in an old alert mail never brings it back.
    #[test]
    fn a_deleted_job_never_comes_back() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let link = "https://www.linkedin.com/jobs/view/4000000001/";
        let p = posting(link, "A", "", "");
        store.upsert_posting(run, &p, mail(), now()).unwrap();
        let other = posting(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "B",
            "",
            "",
        );
        store.upsert_posting(run, &other, mail(), now()).unwrap();
        store.mark_txt_written(&p.key, "a.txt", now()).unwrap();
        store.set_archived(&p.key, true, now()).unwrap();
        assert_eq!(store.archived_keys().unwrap(), std::slice::from_ref(&p.key));
        let rev = store.data_rev().unwrap();
        let (count, names) = store
            .delete_jobs(&store.archived_keys().unwrap(), now())
            .unwrap();
        assert_eq!((count, names), (1, vec!["a.txt".to_owned()]));
        assert!(
            store.data_rev().unwrap() > rev,
            "the Excel file loses the row"
        );
        assert!(store.job(&p.key).unwrap().is_none());
        assert!(store.is_deleted(&p.key).unwrap());
        assert!(store.job(&other.key).unwrap().is_some());
        assert!(!store.is_deleted(&other.key).unwrap());
        // The next scan finds the old mail again.
        let next = store.begin_run().unwrap();
        let again = posting(link, "A", "", "");
        assert_eq!(
            store.upsert_posting(next, &again, mail(), now()).unwrap(),
            Seen::KnownBefore
        );
        assert!(store.job(&p.key).unwrap().is_none());
        assert_eq!(store.job_count().unwrap(), 1);
        // Nothing left to delete.
        assert_eq!(store.delete_jobs(&[p.key], now()).unwrap(), (0, Vec::new()));
    }

    /// Old jobs archive themselves unless they have a mark (saved or sent); the young
    /// and the archived stay as they are.
    #[test]
    fn old_jobs_archive_themselves_except_the_marked() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let old = now() - jiff::SignedDuration::from_hours(24 * 40);
        let mut keys = Vec::new();
        for (i, seen) in [(1, old), (2, old), (3, old), (4, now())] {
            let p = posting(
                &format!("https://www.linkedin.com/jobs/view/400000000{i}/"),
                &format!("Job {i}"),
                "",
                "",
            );
            store.upsert_posting(run, &p, mail(), seen).unwrap();
            keys.push(p.key);
        }
        store.set_pinned(&keys[1], true, now()).unwrap();
        store
            .set_app_status(&keys[2], Some(AppStatus::Sent), now())
            .unwrap();
        let before = now() - jiff::SignedDuration::from_hours(24 * 30);
        assert_eq!(store.auto_archive(before, now()).unwrap(), 1);
        assert_eq!(store.archived_keys().unwrap(), [keys[0].clone()]);
        assert_eq!(store.auto_archive(before, now()).unwrap(), 0);
    }

    /// "Fits anyway" turns an excluded job into a scored one with its fit score; a rescore
    /// keeps that, taking it back makes the engine's verdict due again.
    #[test]
    fn the_override_survives_a_rescore_and_can_be_taken_back() {
        let (store, key) = store_with_job();
        let excluded = crate::model::MatchRecord {
            status: MatchStatus::Excluded,
            score: 71,
            note: None,
            must_met: 1,
            must_total: 2,
            top: Vec::new(),
            facts: crate::model::KeyFacts::default(),
            rank: 0,
        };
        store
            .save_matches(&[(key.clone(), excluded.clone())], "r1", now())
            .unwrap();
        let status = |store: &Store| {
            let job = store.job(&key).unwrap().unwrap();
            let record = job.match_.unwrap();
            (record.status, record.score, job.override_include)
        };
        assert_eq!(status(&store), (MatchStatus::Excluded, 71, false));
        assert!(store.set_override(&key, true).unwrap());
        assert!(!store.set_override(&key, true).unwrap());
        assert_eq!(status(&store), (MatchStatus::Scored, 71, true));
        // A rescore with the same verdict keeps the user's word.
        store
            .save_matches(&[(key.clone(), excluded.clone())], "r2", now())
            .unwrap();
        assert_eq!(status(&store), (MatchStatus::Scored, 71, true));
        assert!(
            store
                .save_match_if(&key, &excluded, "r3", Some("r2"), now())
                .unwrap()
        );
        assert_eq!(status(&store), (MatchStatus::Scored, 71, true));
        // Taken back: the score is due again; the next assessment stores the verdict.
        assert!(store.set_override(&key, false).unwrap());
        assert_eq!(store.job(&key).unwrap().unwrap().match_rev, None);
        store
            .save_matches(&[(key.clone(), excluded)], "r3", now())
            .unwrap();
        assert_eq!(status(&store), (MatchStatus::Excluded, 71, false));
    }

    #[test]
    fn an_unknown_job_changes_nothing() {
        let (store, _) = store_with_job();
        let other = crate::portal::job_link("https://www.linkedin.com/jobs/view/4000000009/")
            .unwrap()
            .key;
        assert!(
            !store
                .set_app_status(&other, Some(AppStatus::Sent), now())
                .unwrap()
        );
        assert!(!store.set_pinned(&other, true, now()).unwrap());
        assert!(!store.set_note(&other, "x").unwrap());
        assert!(!store.set_archived(&other, true, now()).unwrap());
        assert!(!store.set_override(&other, true).unwrap());
        assert_eq!(store.note(&other).unwrap(), None);
        assert_eq!(store.mark_unread(std::slice::from_ref(&other)).unwrap(), 0);
    }
}
