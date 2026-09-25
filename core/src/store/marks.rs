//! What the user keeps about a job: its place - the inbox ("Eingang"), the archive or the
//! trash ("Papierkorb"), like a mail - the favourite (the star, a flag of its own, with the
//! time it was set) and "fits anyway"; plus reading in bulk and deleting for good from the
//! trash (a tombstone stays). Runs never touch these columns, except that old jobs archive
//! themselves and an old trash empties itself (settings).
//!
//! Every column is nullable, so the migrations (in `schema`) add them with
//! `ALTER TABLE job ADD COLUMN`: schema 4 the first marks, schema 5 the rest.

use jiff::Timestamp;
use rusqlite::{Connection, OptionalExtension, params};

use super::{Store, bump};
use crate::error::Result;
use crate::model::Place;
use crate::portal::{JobKey, Portal};
use crate::time::to_db;

/// The nullable columns schema 4 added to the `job` table, as `(name, sql_type)` pairs.
/// Frozen: schema 5 renamed `hidden_at` to `archived_at`.
///
/// - `app_status`: `saved` = the favourite since schema 5 (schema 4 also stored `applied`,
///   `interview`, `offer` and `rejected`, which schema 5 turns into favourites); `NULL` =
///   none.
/// - `app_status_at`: when the favourite was set (Unix seconds).
/// - `note`: unused since schema 5 (the note is gone; the column stays, harmless).
/// - `hidden_at`, now `archived_at`: when the job went to the archive (by the user or by
///   age); `NULL` = not archived.
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
/// - `trashed_at`: when the job went to the trash; it wins over `archived_at`.
/// - `inbox_at`: when the user last moved the job into the inbox; the age of an old job
///   counts from then, so a job she took back is not archived again by the next run.
pub const SCHEMA_5_JOB_COLUMNS: &[(&str, &str)] = &[
    ("override_include", "INTEGER"),
    ("mail_version", "INTEGER"),
    ("trashed_at", "INTEGER"),
    ("inbox_at", "INTEGER"),
];

/// What the migration to schema 5 does beyond the new columns: "hidden" is "archived" now,
/// every other mark of schema 4 (an application status, the pin) becomes the favourite, and
/// the table of deleted job keys (a later scan of an old alert mail never brings them back).
pub const SCHEMA_5_EXTRA: &str = "ALTER TABLE job RENAME COLUMN hidden_at TO archived_at;
UPDATE job SET app_status = 'saved', app_status_at = COALESCE(app_status_at, pinned_at)
 WHERE app_status IS NOT NULL OR pinned_at IS NOT NULL;
CREATE TABLE tombstone (
    portal      TEXT    NOT NULL,
    job_id      TEXT    NOT NULL,
    deleted_at  INTEGER NOT NULL,
    PRIMARY KEY (portal, job_id)
) WITHOUT ROWID;";

/// The note code of a job the user marked as fitting although the engine excludes it.
pub const USER_OVERRIDE: &str = "userOverride";

/// The jobs in the inbox (the active ones; every list and count but the archive's and the
/// trash's).
pub(crate) const INBOX: &str = "archived_at IS NULL AND trashed_at IS NULL";

/// The favourites: starred jobs in the inbox or the archive (never the trash).
pub(crate) const FAVOURITES: &str = "app_status IS NOT NULL AND trashed_at IS NULL";

/// The condition of a place on the `job` table.
pub(crate) const fn place_condition(place: Place) -> &'static str {
    match place {
        Place::Inbox => INBOX,
        Place::Archive => "archived_at IS NOT NULL AND trashed_at IS NULL",
        Place::Trash => "trashed_at IS NOT NULL",
    }
}

impl Store {
    /// The favourite (the star), a flag of its own whatever the place; `true` if it changed.
    /// Set, it keeps the time it was set.
    pub fn set_pinned(&self, key: &JobKey, on: bool, now: Timestamp) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET app_status = CASE WHEN ?3 THEN 'saved' END,
                                app_status_at = CASE WHEN ?3 THEN ?4 END
                 WHERE portal = ?1 AND job_id = ?2 AND (app_status IS NULL) = ?3",
                params![key.portal.key(), key.id, on, to_db(now)],
            )? > 0;
            if changed {
                bump(conn)?;
            }
            Ok(changed)
        })
    }

    /// Moves jobs to a place: the inbox (out of archive and trash; the time of the move is
    /// kept for the age that archives old jobs), the archive (out of the trash too) or the
    /// trash (the archive time stays for the way back). A job keeps the time it first went to
    /// the archive. Returns the keys that really moved (a job already there or gone is not).
    pub fn move_jobs(&self, keys: &[JobKey], to: Place, now: Timestamp) -> Result<Vec<JobKey>> {
        self.place_jobs(keys.iter().map(|key| (key, to, Some(now))))
    }

    /// Takes moves back (the undo of a toast): each job returns to the place it came from as
    /// it was there. Into the trash with the time it first went there (`trashed_at`, never
    /// later than `now`): its date and the days until the trash empties itself stay. Into the
    /// inbox with its age: the time of the last move into it stays for the old jobs that
    /// archive themselves. Returns the keys that really moved.
    pub fn move_back(
        &self,
        back: &[(JobKey, Place, Option<Timestamp>)],
        now: Timestamp,
    ) -> Result<Vec<JobKey>> {
        self.place_jobs(back.iter().map(|(key, to, trashed_at)| {
            let at = match to {
                Place::Inbox => None,
                Place::Archive => Some(now),
                Place::Trash => Some(trashed_at.filter(|at| *at <= now).unwrap_or(now)),
            };
            (key, *to, at)
        }))
    }

    /// Moves each job to its place at its time (`None`: the inbox keeps the time of the last
    /// move into it); returns the keys that really moved.
    fn place_jobs<'a>(
        &self,
        jobs: impl Iterator<Item = (&'a JobKey, Place, Option<Timestamp>)>,
    ) -> Result<Vec<JobKey>> {
        self.write(|conn| {
            let mut moved = Vec::new();
            for (key, to, at) in jobs {
                let (set, from) = match to {
                    Place::Inbox => (
                        "archived_at = NULL, trashed_at = NULL, inbox_at = COALESCE(?3, inbox_at)",
                        "(archived_at IS NOT NULL OR trashed_at IS NOT NULL)",
                    ),
                    Place::Archive => (
                        "archived_at = COALESCE(archived_at, ?3), trashed_at = NULL",
                        "(archived_at IS NULL OR trashed_at IS NOT NULL)",
                    ),
                    Place::Trash => ("trashed_at = ?3", "trashed_at IS NULL"),
                };
                let mut stmt = conn.prepare_cached(&format!(
                    "UPDATE job SET {set} WHERE portal = ?1 AND job_id = ?2 AND {from}"
                ))?;
                if stmt.execute(params![key.portal.key(), key.id, at.map(to_db)])? > 0
                    && !moved.contains(key)
                {
                    moved.push(key.clone());
                }
            }
            if !moved.is_empty() {
                bump(conn)?;
            }
            Ok(moved)
        })
    }

    /// Marks every unread job of a place as read - with a search only its hits, as the list
    /// shows them - and returns their keys: the page can undo it with [`Store::mark_unread`].
    /// No change counter (the Excel file stays); the app's small result files follow the mark.
    pub fn mark_all_read(
        &self,
        place: Place,
        search: Option<&str>,
        now: Timestamp,
    ) -> Result<Vec<JobKey>> {
        let words = super::jobs::search_words(search);
        self.write(|conn| {
            let keys = keys_where(
                conn,
                &format!(
                    "dup_of IS NULL AND read_at IS NULL AND {} AND {}",
                    place_condition(place),
                    super::jobs::matches_words("?1")
                ),
                [words],
            )?;
            let mut mark = conn
                .prepare_cached("UPDATE job SET read_at = ?3 WHERE portal = ?1 AND job_id = ?2")?;
            for key in &keys {
                mark.execute(params![key.portal.key(), key.id, to_db(now)])?;
            }
            Ok(keys)
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

    /// Moves the inbox jobs that are no favourite to the archive when they are older than
    /// `before` - counted from their first sighting, or from when the user last moved them
    /// into the inbox (her choice stands) - "old jobs archive themselves" at the end of a run;
    /// returns how many.
    pub fn auto_archive(&self, before: Timestamp, now: Timestamp) -> Result<usize> {
        self.write(|conn| {
            let archived = conn.execute(
                &format!(
                    "UPDATE job SET archived_at = ?2
                     WHERE {INBOX} AND app_status IS NULL
                       AND COALESCE(inbox_at, first_seen_at) < ?1"
                ),
                params![to_db(before), to_db(now)],
            )?;
            if archived > 0 {
                bump(conn)?;
            }
            Ok(archived)
        })
    }

    /// The keys in the trash: all of them, or those trashed before `before` (the trash that
    /// empties itself).
    pub fn trashed_keys(&self, before: Option<Timestamp>) -> Result<Vec<JobKey>> {
        let conn = self.conn();
        keys_where(
            &conn,
            "trashed_at IS NOT NULL AND (?1 IS NULL OR trashed_at < ?1)",
            [before.map(to_db)],
        )
    }

    /// Of these keys, the ones in the trash: only they may be deleted for good.
    pub fn in_trash(&self, keys: &[JobKey]) -> Result<Vec<JobKey>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT 1 FROM job WHERE portal = ?1 AND job_id = ?2 AND trashed_at IS NOT NULL",
        )?;
        let mut out = Vec::new();
        for key in keys {
            if stmt
                .query_row(params![key.portal.key(), key.id], |_| Ok(()))
                .optional()?
                .is_some()
            {
                out.push(key.clone());
            }
        }
        Ok(out)
    }

    /// Deletes jobs for good (with the duplicates that stand for them): their rows go, only a
    /// tombstone of each key stays, so a scan never imports them again from an old alert
    /// mail. Returns the keys of the rows that went and the names of their text files (the
    /// caller removes the files). The commands delete only from the trash ([`Store::in_trash`]).
    pub fn delete_jobs(
        &self,
        keys: &[JobKey],
        now: Timestamp,
    ) -> Result<(Vec<JobKey>, Vec<String>)> {
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
            let gone = doomed
                .into_iter()
                .filter_map(|(portal, id)| Portal::from_key(&portal).map(|portal| JobKey { portal, id }))
                .collect();
            Ok((gone, names))
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
}

/// The keys of the jobs that meet `condition` (fixed SQL of this module, never input).
fn keys_where(
    conn: &Connection,
    condition: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<JobKey>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT portal, job_id FROM job WHERE {condition} ORDER BY portal, job_id"
    ))?;
    let rows = stmt.query_map(params, |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut keys = Vec::new();
    for row in rows {
        let (portal, id) = row?;
        if let Some(portal) = Portal::from_key(&portal) {
            keys.push(JobKey { portal, id });
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MatchStatus;
    use crate::store::Seen;
    use crate::store::test_support::{mail, now, posting};

    fn store_with_jobs(count: u8) -> (Store, Vec<JobKey>) {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let mut keys = Vec::new();
        for i in 1..=count {
            let p = posting(
                &format!("https://www.linkedin.com/jobs/view/400000000{i}/"),
                &format!("Job {i}"),
                "",
                "",
            );
            store.upsert_posting(run, &p, mail(), now()).unwrap();
            keys.push(p.key);
        }
        (store, keys)
    }

    fn place(store: &Store, key: &JobKey) -> Place {
        store.job(key).unwrap().unwrap().place()
    }

    /// The favourite keeps the time it was set and is a flag of its own: moving the job
    /// keeps it.
    #[test]
    fn the_favourite_keeps_its_time_and_follows_the_job() {
        let (store, keys) = store_with_jobs(1);
        let key = &keys[0];
        let rev = store.data_rev().unwrap();
        let at = now();
        assert!(store.set_pinned(key, true, at).unwrap());
        assert!(store.data_rev().unwrap() > rev, "the overview shows it");
        let later = at + jiff::SignedDuration::from_hours(1);
        assert!(!store.set_pinned(key, true, later).unwrap());
        assert_eq!(store.job(key).unwrap().unwrap().pinned_at, Some(at));
        store
            .move_jobs(std::slice::from_ref(key), Place::Archive, later)
            .unwrap();
        assert_eq!(store.job(key).unwrap().unwrap().pinned_at, Some(at));
        assert!(store.set_pinned(key, false, later).unwrap());
        assert_eq!(store.job(key).unwrap().unwrap().pinned_at, None);
        assert!(!store.set_pinned(key, false, later).unwrap());
    }

    /// A job is in exactly one place; moving counts only real moves and keeps the first time
    /// a job went to a place.
    #[test]
    fn a_job_is_in_one_place_at_a_time() {
        let (store, keys) = store_with_jobs(2);
        let at = now();
        let later = at + jiff::SignedDuration::from_hours(1);
        assert_eq!(place(&store, &keys[0]), Place::Inbox);
        assert_eq!(store.move_jobs(&keys, Place::Archive, at).unwrap(), keys);
        assert!(
            store
                .move_jobs(&keys, Place::Archive, later)
                .unwrap()
                .is_empty(),
            "only real moves come back"
        );
        assert_eq!(store.job(&keys[0]).unwrap().unwrap().archived_at, Some(at));
        assert_eq!(
            store
                .move_jobs(std::slice::from_ref(&keys[0]), Place::Trash, later)
                .unwrap(),
            [keys[0].clone()]
        );
        assert_eq!(place(&store, &keys[0]), Place::Trash);
        assert_eq!(place(&store, &keys[1]), Place::Archive);
        // From the trash back to the archive, then to the inbox.
        store
            .move_jobs(std::slice::from_ref(&keys[0]), Place::Archive, later)
            .unwrap();
        assert_eq!(place(&store, &keys[0]), Place::Archive);
        assert_eq!(store.move_jobs(&keys, Place::Inbox, later).unwrap(), keys);
        assert_eq!(place(&store, &keys[0]), Place::Inbox);
        let job = store.job(&keys[0]).unwrap().unwrap();
        assert_eq!((job.archived_at, job.trashed_at), (None, None));
    }

    /// An undo puts a job back as it was: the trash keeps the time the job first went there,
    /// the inbox the age of the job (the old jobs that archive themselves count from it).
    #[test]
    fn a_move_taken_back_keeps_the_earlier_times() {
        let (store, keys) = store_with_jobs(3);
        let at = now();
        let later = at + jiff::SignedDuration::from_hours(72);
        let one = std::slice::from_ref(&keys[0]);
        // Wiederherstellen three days later, then its undo: back with the first trash time.
        store.move_jobs(one, Place::Trash, at).unwrap();
        store.move_jobs(one, Place::Inbox, later).unwrap();
        let back = [(keys[0].clone(), Place::Trash, Some(at))];
        assert_eq!(store.move_back(&back, later).unwrap(), one);
        assert_eq!(store.job(&keys[0]).unwrap().unwrap().trashed_at, Some(at));
        assert!(
            store.move_back(&back, later).unwrap().is_empty(),
            "there already"
        );
        // A time from the future is no time of the trash.
        let ahead = later + jiff::SignedDuration::from_hours(1);
        store
            .move_back(&[(keys[1].clone(), Place::Trash, Some(ahead))], later)
            .unwrap();
        assert_eq!(
            store.job(&keys[1]).unwrap().unwrap().trashed_at,
            Some(later)
        );
        // Archived, then taken back: the job is as old as before, so it archives itself
        // again with the others (a plain move into the inbox would make it young).
        let three = std::slice::from_ref(&keys[2]);
        store.move_jobs(three, Place::Archive, later).unwrap();
        let back = [(keys[2].clone(), Place::Inbox, None)];
        assert_eq!(store.move_back(&back, later).unwrap(), three);
        assert_eq!(place(&store, &keys[2]), Place::Inbox);
        let cutoff = at + jiff::SignedDuration::from_hours(1);
        assert_eq!(store.auto_archive(cutoff, later).unwrap(), 1);
        assert_eq!(place(&store, &keys[2]), Place::Archive);
    }

    /// "All read" marks exactly the unread jobs of the place and hands their keys back; the
    /// undo makes exactly those unread again.
    #[test]
    fn all_read_and_its_undo() {
        let (store, keys) = store_with_jobs(4);
        store.mark_read(&keys[0], now()).unwrap();
        store
            .move_jobs(std::slice::from_ref(&keys[1]), Place::Archive, now())
            .unwrap();
        let rev = store.data_rev().unwrap();
        // With a search only its hits: "Job 3" is unread in the inbox, "Job 4" stays unread.
        let hits = store
            .mark_all_read(Place::Inbox, Some("job 3"), now())
            .unwrap();
        assert_eq!(hits, [keys[2].clone()]);
        assert!(store.job(&keys[3]).unwrap().unwrap().read_at.is_none());
        store.mark_unread(&hits).unwrap();
        let marked = store.mark_all_read(Place::Inbox, None, now()).unwrap();
        assert_eq!(marked, [keys[2].clone(), keys[3].clone()]);
        assert_eq!(store.data_rev().unwrap(), rev, "no change counter");
        assert!(store.job(&keys[1]).unwrap().unwrap().read_at.is_none());
        assert!(
            store
                .mark_all_read(Place::Inbox, None, now())
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.mark_unread(&marked).unwrap(), 2);
        for key in &marked {
            assert!(store.job(key).unwrap().unwrap().read_at.is_none());
        }
        assert!(store.job(&keys[0]).unwrap().unwrap().read_at.is_some());
    }

    /// Archive and trash leave the HTML overview and the skill's top matches; back in the
    /// inbox, the job is back.
    #[test]
    fn only_inbox_jobs_reach_the_overview_and_the_top_matches() {
        let (store, keys) = store_with_jobs(1);
        let key = &keys[0];
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
        let listed = |jobs: Vec<crate::store::JobRow>| -> Vec<JobKey> {
            jobs.into_iter().map(|j| j.key).collect()
        };
        let since = super::super::new_since(now());
        let one = std::slice::from_ref(key);
        let overview = || store.overview_jobs(20).unwrap();
        assert_eq!(listed(store.skill_matches(since, 5).unwrap()), one);
        assert_eq!(listed(overview().new), one);
        for away in [Place::Archive, Place::Trash] {
            store.move_jobs(one, away, now()).unwrap();
            assert!(store.skill_matches(since, 5).unwrap().is_empty());
            assert!(overview().new.is_empty());
            // A favourite away from the inbox is none of the overview's either.
            store.set_pinned(key, true, now()).unwrap();
            assert_eq!(overview(), super::super::matches::OverviewJobs::default());
            store.set_pinned(key, false, now()).unwrap();
            store.move_jobs(one, Place::Inbox, now()).unwrap();
            assert_eq!(listed(overview().new), one);
        }
    }

    /// The best matches for an AI chat: favourites first, then the best by score; never
    /// excluded, archived, trashed, unscored or gone.
    #[test]
    fn the_best_matches_put_the_favourites_first_and_leave_out_the_rest() {
        let (store, keys) = store_with_jobs(7);
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
        // Job 6 has no score; job 4 is archived; job 7 is in the trash; job 1 a favourite.
        store
            .move_jobs(std::slice::from_ref(&keys[3]), Place::Archive, now())
            .unwrap();
        store
            .move_jobs(std::slice::from_ref(&keys[6]), Place::Trash, now())
            .unwrap();
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

    /// Only the trash is deleted for good; a deleted job leaves only its tombstone: the row,
    /// its duplicate and its text file name go, and the same link in an old alert mail never
    /// brings it back.
    #[test]
    fn a_deleted_job_never_comes_back() {
        let (store, keys) = store_with_jobs(2);
        let link = "https://www.linkedin.com/jobs/view/4000000001/";
        store.mark_txt_written(&keys[0], "a.txt", now()).unwrap();
        store
            .move_jobs(std::slice::from_ref(&keys[0]), Place::Trash, now())
            .unwrap();
        assert_eq!(store.trashed_keys(None).unwrap(), [keys[0].clone()]);
        assert_eq!(store.in_trash(&keys).unwrap(), [keys[0].clone()]);
        let rev = store.data_rev().unwrap();
        let (gone, names) = store
            .delete_jobs(&store.trashed_keys(None).unwrap(), now())
            .unwrap();
        assert_eq!(
            (gone, names),
            (vec![keys[0].clone()], vec!["a.txt".to_owned()])
        );
        assert!(
            store.data_rev().unwrap() > rev,
            "the Excel file loses the row"
        );
        assert!(store.job(&keys[0]).unwrap().is_none());
        assert!(store.is_deleted(&keys[0]).unwrap());
        assert!(store.job(&keys[1]).unwrap().is_some());
        assert!(!store.is_deleted(&keys[1]).unwrap());
        // The next scan finds the old mail again.
        let next = store.begin_run().unwrap();
        let again = posting(link, "Job 1", "", "");
        assert_eq!(
            store.upsert_posting(next, &again, mail(), now()).unwrap(),
            Seen::KnownBefore
        );
        assert!(store.job(&keys[0]).unwrap().is_none());
        assert_eq!(store.job_count().unwrap(), 1);
        assert_eq!(
            store
                .delete_jobs(std::slice::from_ref(&keys[0]), now())
                .unwrap(),
            (Vec::new(), Vec::new())
        );
    }

    /// The trash that empties itself takes only what lies there long enough.
    #[test]
    fn an_old_trash_is_found_by_its_time() {
        let (store, keys) = store_with_jobs(2);
        let old = now() - jiff::SignedDuration::from_hours(24 * 40);
        store
            .move_jobs(std::slice::from_ref(&keys[0]), Place::Trash, old)
            .unwrap();
        store
            .move_jobs(std::slice::from_ref(&keys[1]), Place::Trash, now())
            .unwrap();
        let before = now() - jiff::SignedDuration::from_hours(24 * 30);
        assert_eq!(store.trashed_keys(Some(before)).unwrap(), [keys[0].clone()]);
        assert_eq!(store.trashed_keys(None).unwrap().len(), 2);
    }

    /// Old inbox jobs archive themselves unless they are favourites; the young, the archived
    /// and the trashed stay where they are.
    #[test]
    fn old_jobs_archive_themselves_except_the_favourites() {
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
            .move_jobs(std::slice::from_ref(&keys[2]), Place::Trash, now())
            .unwrap();
        let before = now() - jiff::SignedDuration::from_hours(24 * 30);
        assert_eq!(store.auto_archive(before, now()).unwrap(), 1);
        assert_eq!(place(&store, &keys[0]), Place::Archive);
        assert_eq!(place(&store, &keys[2]), Place::Trash);
        assert_eq!(store.auto_archive(before, now()).unwrap(), 0);
        // Taken back into the inbox (from the archive or the trash): her choice stands, the
        // age counts from the move.
        store
            .move_jobs(&[keys[0].clone(), keys[2].clone()], Place::Inbox, now())
            .unwrap();
        assert_eq!(store.auto_archive(before, now()).unwrap(), 0);
        assert_eq!(place(&store, &keys[0]), Place::Inbox);
        let later = now() + jiff::SignedDuration::from_hours(24 * 31);
        let later_before = later - jiff::SignedDuration::from_hours(24 * 30);
        // A month after the move the two go, with job 4 (young then, old now); the favourite stays.
        assert_eq!(store.auto_archive(later_before, later).unwrap(), 3);
        assert_eq!(place(&store, &keys[1]), Place::Inbox);
    }

    /// "Fits anyway" turns an excluded job into a scored one with its fit score; a rescore
    /// keeps that, taking it back makes the engine's verdict due again.
    #[test]
    fn the_override_survives_a_rescore_and_can_be_taken_back() {
        let (store, keys) = store_with_jobs(1);
        let key = &keys[0];
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
            let job = store.job(key).unwrap().unwrap();
            let record = job.match_.unwrap();
            (record.status, record.score, job.override_include)
        };
        assert_eq!(status(&store), (MatchStatus::Excluded, 71, false));
        assert!(store.set_override(key, true).unwrap());
        assert!(!store.set_override(key, true).unwrap());
        assert_eq!(status(&store), (MatchStatus::Scored, 71, true));
        // A rescore with the same verdict keeps the user's word.
        store
            .save_matches(&[(key.clone(), excluded.clone())], "r2", now())
            .unwrap();
        assert_eq!(status(&store), (MatchStatus::Scored, 71, true));
        assert!(
            store
                .save_match_if(key, &excluded, "r3", Some("r2"), now())
                .unwrap()
        );
        assert_eq!(status(&store), (MatchStatus::Scored, 71, true));
        // Taken back: the score is due again; the next assessment stores the verdict.
        assert!(store.set_override(key, false).unwrap());
        assert_eq!(store.job(key).unwrap().unwrap().match_rev, None);
        store
            .save_matches(&[(key.clone(), excluded)], "r3", now())
            .unwrap();
        assert_eq!(status(&store), (MatchStatus::Excluded, 71, false));
    }

    #[test]
    fn an_unknown_job_changes_nothing() {
        let (store, _) = store_with_jobs(1);
        let other = crate::portal::job_link("https://www.linkedin.com/jobs/view/4000000009/")
            .unwrap()
            .key;
        let one = std::slice::from_ref(&other);
        assert!(!store.set_pinned(&other, true, now()).unwrap());
        assert!(
            store
                .move_jobs(one, Place::Trash, now())
                .unwrap()
                .is_empty()
        );
        assert!(!store.set_override(&other, true).unwrap());
        assert_eq!(store.mark_unread(one).unwrap(), 0);
        assert!(store.in_trash(one).unwrap().is_empty());
    }
}
