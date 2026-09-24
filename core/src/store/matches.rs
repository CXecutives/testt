//! Schema 3: match results and the per-job state around them (read, pinned).
//!
//! Every column is nullable, so the migration (in `schema`) is one
//! `ALTER TABLE job ADD COLUMN` per entry of [`SCHEMA_3_JOB_COLUMNS`].

use jiff::Timestamp;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::jobs::{JOB_COLUMNS, JobRow, job_row};
use super::{Store, bump};
use crate::error::Result;
use crate::model::{HIGH_FROM, MatchRecord, MatchStatus, Notice};
use crate::portal::JobKey;
use crate::text::truncate_chars;
use crate::time::{from_db, to_db};

/// The nullable columns schema 3 adds to the `job` table, as `(name, sql_type)` pairs.
/// Timestamps are Unix seconds, like every other time column.
///
/// - `match_score`: score 0-100.
/// - `match_status`: `scored`, `excluded` or `unscorable`.
/// - `match_note`: JSON `{code, params, mustMet, mustTotal, top[<=2]}`, at most 400 bytes.
/// - `match_at`: when the job was scored.
/// - `match_rev`: revision of engine, profile and model that produced the score; `NULL`
///   after a change of title or text (the job is scored again).
/// - `read_at`: when the user read the job; `NULL` means unread.
/// - `desc_facts`: facts taken from the job page.
/// - `dup_of`: `portal:id` of the job this one duplicates.
/// - `parser_version`: version of the page parser that produced the full text.
/// - `pinned_at`: when the user pinned the job; `NULL` means not pinned.
pub const SCHEMA_3_JOB_COLUMNS: &[(&str, &str)] = &[
    ("match_score", "INTEGER"),
    ("match_status", "TEXT"),
    ("match_note", "TEXT"),
    ("match_at", "INTEGER"),
    ("match_rev", "TEXT"),
    ("read_at", "INTEGER"),
    ("desc_facts", "TEXT"),
    ("dup_of", "TEXT"),
    ("parser_version", "INTEGER"),
    ("pinned_at", "INTEGER"),
];

/// Most bytes a stored note takes.
const MAX_NOTE_BYTES: usize = 400;
/// Most characters of one quoted requirement in the note.
const MAX_TOP_CHARS: usize = 80;
/// Most quoted requirements in the note.
const MAX_TOP: usize = 2;

/// `match_note` as stored.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredNote {
    code: Option<String>,
    params: serde_json::Map<String, serde_json::Value>,
    must_met: u16,
    must_total: u16,
    top: Vec<String>,
}

/// The note of a match as JSON of at most 400 bytes (quotes are cut, then dropped).
pub(super) fn encode_note(record: &MatchRecord) -> String {
    let mut note = StoredNote {
        code: record.note.as_ref().map(|n| n.code.clone()),
        params: record
            .note
            .as_ref()
            .map(|n| n.params.clone())
            .unwrap_or_default(),
        must_met: record.must_met,
        must_total: record.must_total,
        top: record
            .top
            .iter()
            .take(MAX_TOP)
            .map(|t| truncate_chars(t, MAX_TOP_CHARS))
            .collect(),
    };
    loop {
        let json = serde_json::to_string(&note).unwrap_or_default();
        if json.len() <= MAX_NOTE_BYTES || (note.top.is_empty() && note.params.is_empty()) {
            return json;
        }
        if note.top.pop().is_none() {
            note.params.clear();
        }
    }
}

/// A match from its stored columns; `None` without a status (not scored).
pub(super) fn decode_match(
    status: Option<&str>,
    score: Option<i64>,
    note: Option<&str>,
) -> Option<MatchRecord> {
    let status = MatchStatus::parse(status?)?;
    let note: StoredNote = note
        .and_then(|n| serde_json::from_str(n).ok())
        .unwrap_or_default();
    Some(MatchRecord {
        status,
        score: score
            .and_then(|s| u8::try_from(s.clamp(0, 100)).ok())
            .unwrap_or(0),
        note: note.code.map(|code| Notice {
            code,
            params: note.params,
        }),
        must_met: note.must_met,
        must_total: note.must_total,
        top: note.top,
    })
}

impl Store {
    /// Marks a job as read; `true` if it was unread. Reading changes no export - so no new
    /// change counter.
    pub fn mark_read(&self, key: &JobKey, now: Timestamp) -> Result<bool> {
        Ok(self.conn().execute(
            "UPDATE job SET read_at = ?3 WHERE portal = ?1 AND job_id = ?2 AND read_at IS NULL",
            params![key.portal.key(), key.id, to_db(now)],
        )? > 0)
    }

    /// Pins or unpins a job; `true` if something changed. The HTML overview shows pinned
    /// jobs, so this is a visible change.
    pub fn set_pinned(&self, key: &JobKey, on: bool, now: Timestamp) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET pinned_at = CASE WHEN ?3 THEN COALESCE(pinned_at, ?4) END
                 WHERE portal = ?1 AND job_id = ?2
                   AND (pinned_at IS NULL) = ?3",
                params![key.portal.key(), key.id, on, to_db(now)],
            )? > 0;
            if changed {
                bump(conn)?;
            }
            Ok(changed)
        })
    }

    /// Stores the matches of a page of jobs as one change (`rev`: who scored them).
    pub fn save_matches(
        &self,
        matches: &[(JobKey, MatchRecord)],
        rev: &str,
        now: Timestamp,
    ) -> Result<()> {
        if matches.is_empty() {
            return Ok(());
        }
        self.write(|conn| {
            let mut stmt = conn.prepare_cached(
                "UPDATE job SET match_score = ?3, match_status = ?4, match_note = ?5,
                                match_rev = ?6, match_at = ?7
                 WHERE portal = ?1 AND job_id = ?2",
            )?;
            for (key, record) in matches {
                stmt.execute(params![
                    key.portal.key(),
                    key.id,
                    record.score,
                    record.status.as_str(),
                    encode_note(record),
                    rev,
                    to_db(now),
                ])?;
            }
            bump(conn)
        })
    }

    /// Stores the match of one job only while its revision is still `expected` (compare and
    /// set): a run that scored the job in the meantime wins. A duplicate of another portal's
    /// job is never scored (it shows as that job's row). `true` if stored.
    pub fn save_match_if(
        &self,
        key: &JobKey,
        record: &MatchRecord,
        rev: &str,
        expected: Option<&str>,
        now: Timestamp,
    ) -> Result<bool> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET match_score = ?3, match_status = ?4, match_note = ?5,
                                match_rev = ?6, match_at = ?7
                 WHERE portal = ?1 AND job_id = ?2 AND match_rev IS ?8 AND dup_of IS NULL",
                params![
                    key.portal.key(),
                    key.id,
                    record.score,
                    record.status.as_str(),
                    encode_note(record),
                    rev,
                    to_db(now),
                    expected,
                ],
            )?;
            if changed > 0 {
                bump(conn)?;
            }
            Ok(changed > 0)
        })
    }

    /// Up to `limit` jobs not scored with `rev` yet (after skipping `offset`), with their
    /// full text; newest first. A duplicate of another portal's job is scored with that one.
    pub fn unscored(
        &self,
        rev: &str,
        limit: u32,
        offset: usize,
    ) -> Result<Vec<(JobRow, Option<String>)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS}, desc_text FROM job
             WHERE match_rev IS NOT ?1 AND dup_of IS NULL
             ORDER BY first_seen_at DESC, portal, job_id LIMIT ?2 OFFSET ?3"
        ))?;
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![rev, limit, offset], |r| {
            let text: Option<String> = r.get(super::jobs::JOB_COLUMN_COUNT)?;
            Ok(job_row(r)?.map(|job| (job, text)))
        })?;
        rows.map(|r| r?).collect()
    }

    /// Number of jobs not scored with `rev` yet.
    pub fn match_pending(&self, rev: &str) -> Result<u32> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM job WHERE match_rev IS NOT ?1 AND dup_of IS NULL",
            [rev],
            |r| r.get(0),
        )?)
    }

    /// Forgets every match (the profile is gone); the number of jobs that had one.
    pub fn clear_matches(&self) -> Result<usize> {
        self.write(|conn| {
            let cleared = conn.execute(
                "UPDATE job SET match_score = NULL, match_status = NULL, match_note = NULL,
                                match_at = NULL, match_rev = NULL
                 WHERE match_status IS NOT NULL OR match_rev IS NOT NULL",
                [],
            )?;
            if cleared > 0 {
                bump(conn)?;
            }
            Ok(cleared)
        })
    }

    /// When a job was scored last (with whatever revision).
    pub fn match_at(&self, key: &JobKey) -> Result<Option<Timestamp>> {
        let at: Option<i64> = self
            .conn()
            .query_row(
                "SELECT match_at FROM job WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(at.and_then(from_db))
    }

    /// When the last job was scored with `rev`; `None` if none was.
    pub fn scored_at(&self, rev: &str) -> Result<Option<Timestamp>> {
        let at: Option<i64> = self.conn().query_row(
            "SELECT MAX(match_at) FROM job WHERE match_rev = ?1",
            [rev],
            |r| r.get(0),
        )?;
        Ok(at.and_then(from_db))
    }

    /// The best scored (not excluded) jobs first seen in `run`; duplicates show as their
    /// original, hidden jobs ("not interesting") are left out.
    pub fn top_matches(&self, run: i64, limit: u32) -> Result<Vec<JobRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS} FROM job
             WHERE first_seen_run = ?1 AND match_status = 'scored' AND dup_of IS NULL
               AND hidden_at IS NULL
             ORDER BY match_score DESC, first_seen_at DESC, portal, job_id LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![run, limit], job_row)?;
        rows.map(|r| r?).collect()
    }

    /// The jobs a mailbox run brought and how many of them are scored in the high band: first
    /// seen in `run`, a job several portals announce once (as its original), excluded ones
    /// left out - the numbers of the run card.
    pub fn new_jobs(&self, run: i64) -> Result<(usize, usize)> {
        let (count, high): (i64, i64) = self.conn().query_row(
            "SELECT COUNT(*), COALESCE(SUM(match_status IS 'scored' AND match_score >= ?2), 0)
             FROM job WHERE first_seen_run = ?1 AND dup_of IS NULL
                        AND match_status IS NOT 'excluded'",
            params![run, HIGH_FROM],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((
            usize::try_from(count).unwrap_or(0),
            usize::try_from(high).unwrap_or(0),
        ))
    }

    /// The jobs of the HTML overview: the pinned ones if there are any (`true`), else the
    /// unread scored jobs of the mailbox run `run`; best first. Hidden jobs are in neither.
    pub fn overview_jobs(&self, run: i64) -> Result<(Vec<JobRow>, bool)> {
        let conn = self.conn();
        let mut pinned = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS} FROM job WHERE pinned_at IS NOT NULL AND hidden_at IS NULL
             ORDER BY (match_status IS 'excluded'), match_score DESC, pinned_at DESC"
        ))?;
        let jobs: Vec<JobRow> = pinned
            .query_map([], job_row)?
            .map(|r| r?)
            .collect::<Result<_>>()?;
        if !jobs.is_empty() {
            return Ok((jobs, true));
        }
        let mut new = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS} FROM job
             WHERE first_seen_run = ?1 AND read_at IS NULL AND match_status = 'scored'
               AND dup_of IS NULL AND hidden_at IS NULL
             ORDER BY match_score DESC, first_seen_at DESC, portal, job_id"
        ))?;
        let jobs = new
            .query_map([run], job_row)?
            .map(|r| r?)
            .collect::<Result<_>>()?;
        Ok((jobs, false))
    }

    /// Revision a job was scored with (tests and checks).
    pub fn match_rev(&self, key: &JobKey) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT match_rev FROM job WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
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

    fn record(status: MatchStatus, score: u8) -> MatchRecord {
        MatchRecord {
            status,
            score,
            note: Some(Notice {
                code: "fewMust".into(),
                params: serde_json::Map::new(),
            }),
            must_met: 2,
            must_total: 3,
            top: vec!["SAP FI".into(), "x".repeat(500), "dritter".into()],
        }
    }

    #[test]
    fn read_and_pinned_marks() {
        let (store, key) = store_with_job();
        let rev = store.data_rev().unwrap();
        assert!(store.mark_read(&key, now()).unwrap());
        assert!(!store.mark_read(&key, now()).unwrap(), "only once");
        assert_eq!(store.data_rev().unwrap(), rev, "reading exports nothing");
        assert!(store.set_pinned(&key, true, now()).unwrap());
        assert!(!store.set_pinned(&key, true, now()).unwrap());
        assert!(store.data_rev().unwrap() > rev);
        let job = store.job(&key).unwrap().unwrap();
        assert!(job.read_at.is_some() && job.pinned_at.is_some());
        assert!(store.set_pinned(&key, false, now()).unwrap());
        assert!(store.job(&key).unwrap().unwrap().pinned_at.is_none());
    }

    #[test]
    fn matches_round_trip_and_notes_stay_small() {
        let (store, key) = store_with_job();
        assert_eq!(store.match_pending("r1").unwrap(), 1);
        let scored = record(MatchStatus::Scored, 83);
        store
            .save_matches(&[(key.clone(), scored.clone())], "r1", now())
            .unwrap();
        assert_eq!(store.match_pending("r1").unwrap(), 0);
        assert_eq!(store.match_pending("r2").unwrap(), 1);
        let back = store.job(&key).unwrap().unwrap().match_.unwrap();
        assert_eq!((back.status, back.score), (MatchStatus::Scored, 83));
        assert_eq!(back.note, scored.note);
        assert_eq!(back.top.len(), 2);
        assert!(back.top[1].chars().count() <= MAX_TOP_CHARS);
        assert!(encode_note(&scored).len() <= MAX_NOTE_BYTES);
        let mut huge = scored;
        huge.note
            .as_mut()
            .unwrap()
            .params
            .insert("x".into(), "y".repeat(600).into());
        assert!(encode_note(&huge).len() <= MAX_NOTE_BYTES);
        // A new text or title makes the job pending again.
        store
            .record_text(&key, "Volltext", false, false, now())
            .unwrap();
        assert_eq!(store.match_rev(&key).unwrap(), None);
        assert_eq!(store.unscored("r1", 10, 0).unwrap().len(), 1);
        assert!(store.unscored("r1", 10, 1).unwrap().is_empty());
    }

    /// The reader stores its fresh score only over the one it read: a run that scored the
    /// job in between keeps its score.
    #[test]
    fn a_fresh_score_only_replaces_the_one_it_read() {
        let (store, key) = store_with_job();
        let run = record(MatchStatus::Scored, 70);
        store
            .save_matches(&[(key.clone(), run.clone())], "r2", now())
            .unwrap();
        let stale = record(MatchStatus::Scored, 10);
        assert!(
            !store
                .save_match_if(&key, &stale, "r1", None, now())
                .unwrap()
        );
        let job = store.job(&key).unwrap().unwrap();
        assert_eq!(
            (job.match_.unwrap().score, job.match_rev.unwrap()),
            (70, "r2".into())
        );
        assert!(
            store
                .save_match_if(&key, &stale, "r3", Some("r2"), now())
                .unwrap()
        );
        assert_eq!(store.match_rev(&key).unwrap().as_deref(), Some("r3"));
    }

    #[test]
    fn match_times_and_clearing() {
        let (store, key) = store_with_job();
        assert_eq!(store.scored_at("r1").unwrap(), None);
        assert_eq!(store.match_at(&key).unwrap(), None);
        store
            .save_matches(
                &[(key.clone(), record(MatchStatus::Scored, 70))],
                "r1",
                now(),
            )
            .unwrap();
        let at = store.match_at(&key).unwrap().unwrap();
        assert_eq!(at.as_second(), now().as_second());
        assert_eq!(store.scored_at("r1").unwrap(), Some(at));
        assert_eq!(store.scored_at("r2").unwrap(), None);
        let rev = store.data_rev().unwrap();
        assert_eq!(store.clear_matches().unwrap(), 1);
        assert!(store.data_rev().unwrap() > rev, "the export changes");
        let job = store.job(&key).unwrap().unwrap();
        assert!(job.match_.is_none() && job.match_rev.is_none());
        assert_eq!(store.match_at(&key).unwrap(), None);
        assert_eq!(store.clear_matches().unwrap(), 0, "nothing left");
    }
}
