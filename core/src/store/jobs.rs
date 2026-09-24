//! Jobs, alert mails, job details and text files: the types the rest of the app sees and
//! every query on the `job` and `alert_mail` tables.

use jiff::{SignedDuration, Timestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};
use url::Url;

use super::{Store, bump};
use crate::error::{Error, Result};
use crate::fetch::policy::MAX_FETCH_ATTEMPTS;
use crate::model::{
    AlertMail, DescStatus, HIGH_FROM, MAX_FIELD_CHARS, MAX_TITLE_CHARS, MatchRecord, Posting,
    is_usable_title,
};
use crate::portal::{Facts, JobKey, Portal};
use crate::text::{one_line, page_location, split_company_location, truncate_chars};
use crate::time::{from_db, to_db};

/// Maximum length of a stored failure reason (in characters).
const MAX_ERROR_CHARS: usize = 200;

/// How an entry is classified in the current scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    /// Seen for the first time.
    New,
    /// Already known from an earlier run.
    KnownBefore,
    /// Already seen in this run (the same job in two alert mails).
    DupInRun,
}

/// A job as the UI and the exports see it (without the full text).
#[derive(Debug, Clone, PartialEq)]
pub struct JobRow {
    pub key: JobKey,
    pub url: Url,
    pub title: String,
    pub company: String,
    pub location: String,
    pub mail_date: Option<Timestamp>,
    pub mail_subject: String,
    pub gmail_id: Option<u64>,
    pub first_seen_at: Timestamp,
    pub first_seen_run: i64,
    pub desc_status: DescStatus,
    pub desc_short: bool,
    pub desc_closed: bool,
    pub desc_len: i64,
    pub desc_fetched_at: Option<Timestamp>,
    pub desc_attempts: i64,
    pub desc_error: Option<String>,
    pub txt_name: Option<String>,
    /// Last fetch attempt (success or failure).
    pub desc_attempted_at: Option<Timestamp>,
    /// `None` = unread.
    pub read_at: Option<Timestamp>,
    pub pinned_at: Option<Timestamp>,
    /// `None` = not scored yet.
    pub match_: Option<MatchRecord>,
    /// Who scored it; `None` = to be scored (again).
    pub match_rev: Option<String>,
    /// The facts the job page stated (unreadable JSON counts as none).
    pub facts: Option<Facts>,
}

/// One page of the job list. The facet only narrows the page; the counts cover the
/// search, whatever the facet.
#[derive(Debug, Clone, Default)]
pub struct PageQuery {
    /// "New" = unread and not excluded.
    pub only_new: bool,
    /// Best match first; otherwise newest first. Excluded jobs come last either way.
    pub by_match: bool,
    /// Search term in title, company, location and full text (case-insensitive).
    pub search: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

/// Counts that belong to a page of the job list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageCounts {
    /// Unread and not excluded.
    pub new: u32,
    pub all: u32,
    pub excluded: u32,
    /// Scored in the high band.
    pub high: u32,
    /// Jobs without a full text.
    pub no_detail: u32,
}

/// Jobs of one portal with a given job details state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalCount {
    pub portal: Portal,
    pub status: DescStatus,
    pub count: i64,
    /// Of these, the ones with a short text.
    pub short: i64,
}

/// An alert mail without recognised entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertMailRow {
    pub portal: Portal,
    pub subject: String,
    pub mail_date: Option<Timestamp>,
    pub gmail_id: Option<u64>,
}

/// Selection for the list and the export.
#[derive(Debug, Clone, Default)]
pub struct JobFilter {
    /// Only jobs seen for the first time in this run.
    pub first_seen_run: Option<i64>,
    /// Search term in title, company, location and full text (case-insensitive).
    pub search: Option<String>,
}

/// Mail details a job takes over when it is first seen.
#[derive(Debug, Clone, Copy)]
pub struct MailRef<'a> {
    pub subject: &'a str,
    pub date: Option<Timestamp>,
    pub gmail_id: Option<u64>,
}

impl<'a> From<&'a AlertMail> for MailRef<'a> {
    fn from(mail: &'a AlertMail) -> Self {
        MailRef {
            subject: &mail.subject,
            date: mail.date,
            gmail_id: mail.gmail_id,
        }
    }
}

impl Store {
    // ------------------------------------------------------------------ Intake

    /// Records one entry from an alert mail (merge rule: [`upsert`]) - tests only; the app
    /// records whole alert mails ([`Store::record_alert`]).
    #[cfg(test)]
    pub(crate) fn upsert_posting(
        &self,
        run: i64,
        posting: &Posting,
        mail: MailRef<'_>,
        now: Timestamp,
    ) -> Result<Seen> {
        self.write(|conn| upsert(conn, run, posting, mail, now))
    }

    /// Records a recognised alert mail with its entries - as one change. A mail without
    /// entries is remembered too (layout guard). Returns the classification of each entry.
    pub fn record_alert(&self, run: i64, alert: &AlertMail, now: Timestamp) -> Result<Vec<Seen>> {
        self.write(|conn| {
            conn.execute(
                "INSERT INTO alert_mail (mail_key, portal, subject, mail_date, gmail_id,
                                         n_postings, last_seen_run)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT (mail_key) DO UPDATE SET
                    n_postings = excluded.n_postings, last_seen_run = excluded.last_seen_run",
                params![
                    alert.key,
                    alert.portal.key(),
                    alert.subject,
                    alert.date.map(to_db),
                    alert.gmail_id.map(|id| id.to_string()),
                    i64::try_from(alert.postings.len()).unwrap_or(i64::MAX),
                    run,
                ],
            )?;
            alert
                .postings
                .iter()
                .map(|posting| upsert(conn, run, posting, MailRef::from(alert), now))
                .collect()
        })
    }

    /// Alert mails of a run without a single recognised entry (layout guard: grey rows with
    /// a Gmail link).
    pub fn zero_posting_mails(&self, run: i64) -> Result<Vec<AlertMailRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT portal, subject, mail_date, gmail_id FROM alert_mail
             WHERE last_seen_run = ?1 AND n_postings = 0 ORDER BY mail_date DESC",
        )?;
        let rows = stmt.query_map([run], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (portal, subject, date, gmail_id) = row?;
            let portal = Portal::from_key(&portal)
                .ok_or_else(|| Error::Corrupt(format!("unknown portal `{portal}`")))?;
            out.push(AlertMailRow {
                portal,
                subject,
                mail_date: date.and_then(from_db),
                gmail_id: gmail_id.and_then(|id| id.parse().ok()),
            });
        }
        Ok(out)
    }

    // ------------------------------------------------------------------ Queries

    pub fn job(&self, key: &JobKey) -> Result<Option<JobRow>> {
        let conn = self.conn();
        let row = conn
            .query_row(
                &format!("SELECT {JOB_COLUMNS} FROM job WHERE portal = ?1 AND job_id = ?2"),
                params![key.portal.key(), key.id],
                job_row,
            )
            .optional()?;
        row.transpose()
    }

    /// Text of a job: the full text (status `ok`) or the teaser a guest sees (`teaser`).
    pub fn description(&self, key: &JobKey) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT desc_text FROM job WHERE portal = ?1 AND job_id = ?2
                 AND desc_status IN ('ok', 'teaser')",
                params![key.portal.key(), key.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    /// Jobs, newest first (first sighting, then mail date).
    pub fn jobs(&self, filter: &JobFilter) -> Result<Vec<JobRow>> {
        let conn = self.conn();
        let pattern = like_pattern(filter.search.as_deref());
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS} FROM job
             WHERE (?1 IS NULL OR first_seen_run = ?1)
               AND (?2 IS NULL OR search LIKE ?2 ESCAPE '\\')
             ORDER BY first_seen_at DESC, mail_date DESC, portal, job_id"
        ))?;
        let rows = stmt.query_map(params![filter.first_seen_run, pattern], job_row)?;
        rows.map(|r| r?).collect()
    }

    /// One page of the job list and its counts - from one statement, so list and counts
    /// never disagree.
    pub fn job_page(&self, query: &PageQuery) -> Result<(Vec<JobRow>, PageCounts)> {
        let conn = self.conn();
        let pattern = like_pattern(query.search.as_deref());
        // Excluded jobs always come last; "match" puts the best score first (unscored after
        // scored), "newest" the latest first sighting.
        let order = |p: &str| {
            let by_match = if query.by_match {
                format!("({p}match_score IS NULL), {p}match_score DESC, ")
            } else {
                String::new()
            };
            format!(
                "({p}match_status IS 'excluded'), {by_match}{p}first_seen_at DESC, \
                 {p}portal, {p}job_id"
            )
        };
        let new = "read_at IS NULL AND match_status IS NOT 'excluded'";
        let sql = format!(
            "WITH base AS (
                 SELECT * FROM job WHERE dup_of IS NULL
                                     AND (?1 IS NULL OR search LIKE ?1 ESCAPE '\\')
             ), counts AS (
                 SELECT COUNT(*) AS n_all,
                        COALESCE(SUM({new}), 0) AS n_new,
                        COALESCE(SUM(match_status IS 'excluded'), 0) AS n_excluded,
                        COALESCE(SUM(match_status IS 'scored' AND match_score >= ?5), 0)
                            AS n_high,
                        COALESCE(SUM(desc_status <> 'ok'), 0) AS n_no_detail
                 FROM base
             ), page AS (
                 SELECT {JOB_COLUMNS} FROM base
                 WHERE (?2 = 0 OR ({new}))
                 ORDER BY {}
                 LIMIT ?3 OFFSET ?4
             )
             SELECT counts.n_all, counts.n_new, counts.n_excluded, counts.n_high,
                    counts.n_no_detail, page.*
             FROM counts LEFT JOIN page
             ORDER BY {}",
            order(""),
            order("page.")
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let mut counts = PageCounts::default();
        let mut jobs = Vec::new();
        let mut rows = stmt.query(params![
            pattern,
            query.only_new,
            query.limit,
            query.offset,
            HIGH_FROM
        ])?;
        while let Some(row) = rows.next()? {
            counts = PageCounts {
                all: row.get(0)?,
                new: row.get(1)?,
                excluded: row.get(2)?,
                high: row.get(3)?,
                no_detail: row.get(4)?,
            };
            if row.get::<_, Option<String>>(5)?.is_some() {
                jobs.push(job_row_at(row, 5)??);
            }
        }
        Ok((jobs, counts))
    }

    /// Per portal: jobs first seen in this run and jobs known before that appeared again.
    pub fn scan_counts(&self, run: i64) -> Result<Vec<(Portal, usize, usize)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT portal, SUM(first_seen_run = ?1), SUM(first_seen_run < ?1)
             FROM job WHERE last_seen_run = ?1 GROUP BY portal",
        )?;
        let rows = stmt.query_map([run], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (portal, new, known) = row?;
            let portal = Portal::from_key(&portal)
                .ok_or_else(|| Error::Corrupt(format!("unknown portal `{portal}`")))?;
            out.push((
                portal,
                usize::try_from(new).unwrap_or(0),
                usize::try_from(known).unwrap_or(0),
            ));
        }
        Ok(out)
    }

    /// Number of all jobs.
    pub fn job_count(&self) -> Result<i64> {
        Ok(self
            .conn()
            .query_row("SELECT COUNT(*) FROM job", [], |r| r.get(0))?)
    }

    /// Per portal and job details state: the count and how many of them are short (for the
    /// portal view).
    pub fn portal_counts(&self) -> Result<Vec<PortalCount>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT portal, desc_status, COUNT(*), SUM(desc_short) FROM job GROUP BY portal, desc_status",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (portal, status, count, short) = row?;
            out.push(PortalCount {
                portal: Portal::from_key(&portal)
                    .ok_or_else(|| Error::Corrupt(format!("unknown portal `{portal}`")))?,
                status: DescStatus::parse(&status)
                    .ok_or_else(|| Error::Corrupt(format!("unknown status `{status}`")))?,
                count,
                short,
            });
        }
        Ok(out)
    }

    // ------------------------------------------------------------------ Job details

    /// Jobs whose full text should be fetched automatically: open or failed (at the earliest
    /// `retry_after` after the last attempt), mail at most `max_age` old. Order: open ones
    /// before retries, then newest mail first.
    pub fn fetch_queue(
        &self,
        now: Timestamp,
        max_age: SignedDuration,
        retry_after: SignedDuration,
    ) -> Result<Vec<JobRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS} FROM job WHERE {DUE}
             ORDER BY desc_status = 'failed', COALESCE(mail_date, first_seen_at) DESC, portal, job_id"
        ))?;
        let rows = stmt.query_map(due_params(now, max_age, retry_after), job_row)?;
        rows.map(|r| r?).collect()
    }

    /// Per portal, the number of jobs in the queue (like [`Store::fetch_queue`]).
    pub fn due_counts(
        &self,
        now: Timestamp,
        max_age: SignedDuration,
        retry_after: SignedDuration,
    ) -> Result<Vec<(Portal, usize)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT portal, COUNT(*) FROM job WHERE {DUE} GROUP BY portal"
        ))?;
        let rows = stmt.query_map(due_params(now, max_age, retry_after), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (portal, count) = row?;
            let portal = Portal::from_key(&portal)
                .ok_or_else(|| Error::Corrupt(format!("unknown portal `{portal}`")))?;
            out.push((portal, usize::try_from(count).unwrap_or(0)));
        }
        Ok(out)
    }

    /// Full text stored (also short, checked texts and closed ads).
    pub fn record_text(
        &self,
        key: &JobKey,
        text: &str,
        short: bool,
        closed: bool,
        now: Timestamp,
    ) -> Result<()> {
        self.write(|conn| {
            conn.execute(
                "UPDATE job SET desc_status = 'ok', desc_text = ?3, desc_short = ?4, desc_closed = ?5,
                                desc_fetched_at = ?6, desc_attempted_at = ?6, desc_error = NULL,
                                match_rev = NULL
                 WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id, text, short, closed, to_db(now)],
            )?;
            bump(conn)?;
            refresh_search(conn, key)
        })
    }

    /// Only the teaser a guest sees: stored for matching and marked, never as a text file.
    /// A full text is never downgraded. The attempt counter starts again, so that a sign-in
    /// switched on later fetches the full text right away.
    pub fn record_teaser(&self, key: &JobKey, text: &str, now: Timestamp) -> Result<()> {
        self.write(|conn| {
            let changed = conn.execute(
                "UPDATE job SET desc_status = 'teaser', desc_text = ?3, desc_short = 0,
                                desc_closed = 0, desc_fetched_at = ?4, desc_attempted_at = ?4,
                                desc_attempts = 0, desc_error = NULL, match_rev = NULL
                 WHERE portal = ?1 AND job_id = ?2 AND desc_status <> 'ok'",
                params![key.portal.key(), key.id, text, to_db(now)],
            )?;
            if changed > 0 {
                bump(conn)?;
                refresh_search(conn, key)?;
            }
            Ok(())
        })
    }

    /// The ad no longer exists.
    pub fn record_gone(&self, key: &JobKey, now: Timestamp) -> Result<()> {
        self.write(|conn| {
            // A text fetched earlier stays valid - a success is never downgraded.
            let changed = conn.execute(
                "UPDATE job SET desc_status = 'gone', desc_attempted_at = ?3, desc_error = NULL
                 WHERE portal = ?1 AND job_id = ?2 AND desc_status <> 'ok'",
                params![key.portal.key(), key.id, to_db(now)],
            )?;
            if changed > 0 {
                bump(conn)?;
            }
            Ok(())
        })
    }

    /// Page loaded, but no valid text: count the attempt; after `MAX_FETCH_ATTEMPTS` the job
    /// counts as unfetchable. A job with a valid text stays unchanged (returns its status); a
    /// teaser stays a teaser (it is only fetched again with a sign-in, see `DUE`).
    /// The reason often comes from the page (redirect target, script error) - it is stored
    /// as one short line.
    pub fn record_failed(&self, key: &JobKey, error: &str, now: Timestamp) -> Result<DescStatus> {
        self.record_failure(key, error, now, true)
    }

    /// Like [`Store::record_failed`]; `count_attempt: false` records the failure without
    /// costing the job an attempt (a page in a series of suspicious pages - the portal's
    /// fault, not the job's).
    pub fn record_failure(
        &self,
        key: &JobKey,
        error: &str,
        now: Timestamp,
        count_attempt: bool,
    ) -> Result<DescStatus> {
        let error = truncate_chars(&one_line(error), MAX_ERROR_CHARS);
        let status: String = self.write(|conn| {
            let updated: Option<String> = conn
                .query_row(
                    "UPDATE job SET desc_attempts = desc_attempts + ?6, desc_attempted_at = ?3, desc_error = ?4,
                                    desc_status = CASE WHEN desc_status = 'teaser' THEN 'teaser'
                                                       WHEN desc_attempts + ?6 >= ?5 THEN 'unfetchable'
                                                       ELSE 'failed' END
                     WHERE portal = ?1 AND job_id = ?2 AND desc_status <> 'ok'
                     RETURNING desc_status",
                    params![
                        key.portal.key(),
                        key.id,
                        to_db(now),
                        error,
                        MAX_FETCH_ATTEMPTS,
                        i64::from(count_attempt)
                    ],
                    |r| r.get(0),
                )
                .optional()?;
            match updated {
                Some(status) => {
                    bump(conn)?;
                    Ok(status)
                }
                None => Ok(conn.query_row(
                    "SELECT desc_status FROM job WHERE portal = ?1 AND job_id = ?2",
                    params![key.portal.key(), key.id],
                    |r| r.get(0),
                )?),
            }
        })?;
        DescStatus::parse(&status)
            .ok_or_else(|| Error::Corrupt(format!("unknown status `{status}`")))
    }

    /// Structured page details (LinkedIn header, freelancermap data) are more reliable than
    /// the mail heuristics: non-empty values replace the mail values - with the same length
    /// limits as on intake, and the work mode from the mail ("Remote") stays.
    pub fn record_page_fields(
        &self,
        key: &JobKey,
        title: &str,
        company: &str,
        location: &str,
    ) -> Result<()> {
        let title = truncate_chars(&one_line(title), MAX_TITLE_CHARS);
        let company = truncate_chars(&one_line(company), MAX_FIELD_CHARS);
        let location = one_line(location);
        self.write(|conn| {
            let stored: String = conn
                .query_row(
                    "SELECT location FROM job WHERE portal = ?1 AND job_id = ?2",
                    params![key.portal.key(), key.id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_default();
            let location = page_location(&stored, &location, MAX_FIELD_CHARS);
            conn.execute(
                "UPDATE job SET
                    match_rev = CASE WHEN ?3 <> '' AND ?3 <> title THEN NULL ELSE match_rev END,
                    title    = CASE WHEN ?3 <> '' THEN ?3 ELSE title END,
                    company  = CASE WHEN ?4 <> '' THEN ?4 ELSE company END,
                    location = CASE WHEN ?5 <> '' THEN ?5 ELSE location END
                 WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id, title, company, location],
            )?;
            bump(conn)?;
            refresh_search(conn, key)
        })
    }

    // ------------------------------------------------------------------ Text files

    /// Jobs with a full text, together with the text: only those whose text file was never
    /// written - or, with `all`, every one ("rewrite text files").
    pub fn txt_jobs(&self, all: bool) -> Result<Vec<(JobRow, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS}, desc_text FROM job
             WHERE desc_status = 'ok' AND desc_text IS NOT NULL
               AND (?1 OR txt_written_at IS NULL)
             ORDER BY first_seen_at, portal, job_id"
        ))?;
        let rows = stmt.query_map([all], |r| {
            let text: String = r.get(JOB_COLUMN_COUNT)?;
            Ok(job_row(r)?.map(|job| (job, text)))
        })?;
        rows.map(|r| r?).collect()
    }

    /// Text file written - it is never created again on its own, even if the user deletes
    /// it (the matching skill reads every file in the folder).
    pub fn mark_txt_written(&self, key: &JobKey, file_name: &str, now: Timestamp) -> Result<()> {
        self.conn().execute(
            "UPDATE job SET txt_name = ?3, txt_written_at = ?4 WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id, file_name, to_db(now)],
        )?;
        Ok(())
    }

    /// Names of all text files the app has written (for "empty the results folder").
    pub fn txt_names(&self) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT txt_name FROM job WHERE txt_name IS NOT NULL")?;
        let names = stmt.query_map([], |r| r.get(0))?;
        Ok(names.collect::<rusqlite::Result<_>>()?)
    }
}

// ---------------------------------------------------------------------- Helpers

pub(super) const JOB_COLUMNS: &str = "portal, job_id, url, title, company, location, mail_date,
    mail_subject, gmail_id, first_seen_at, first_seen_run, desc_status, desc_short, desc_closed,
    COALESCE(LENGTH(desc_text), 0) AS desc_len, desc_fetched_at, desc_attempts, desc_error,
    txt_name, desc_attempted_at, read_at, pinned_at, match_status, match_score, match_note,
    match_rev, desc_facts";
pub(super) const JOB_COLUMN_COUNT: usize = 27;

/// Fetchable automatically: open or failed (at the earliest `?2` after the last attempt),
/// or a teaser (right away, after a failed attempt like a failure, at most
/// `MAX_FETCH_ATTEMPTS` = `?3` times) - mail not older than `?1`. Teasers are only fetched
/// on a session path (`fetch::fetch_all`).
const DUE: &str = "COALESCE(mail_date, first_seen_at) >= ?1
    AND (desc_status = 'missing'
         OR (desc_status = 'failed' AND COALESCE(desc_attempted_at, 0) <= ?2)
         OR (desc_status = 'teaser' AND desc_attempts < ?3
             AND (desc_attempts = 0 OR COALESCE(desc_attempted_at, 0) <= ?2)))";

fn due_params(now: Timestamp, max_age: SignedDuration, retry_after: SignedDuration) -> [i64; 3] {
    [
        to_db(now.saturating_sub(max_age).unwrap_or(Timestamp::MIN)),
        to_db(now.saturating_sub(retry_after).unwrap_or(Timestamp::MIN)),
        i64::from(MAX_FETCH_ATTEMPTS),
    ]
}

/// Reads one row; unknown values become `Error::Corrupt` (the inner `Result`).
pub(super) fn job_row(r: &Row<'_>) -> rusqlite::Result<Result<JobRow>> {
    job_row_at(r, 0)
}

/// Reads the job columns starting at column `at`.
fn job_row_at(r: &Row<'_>, at: usize) -> rusqlite::Result<Result<JobRow>> {
    let col = |i: usize| at + i;
    let portal: String = r.get(col(0))?;
    let url: String = r.get(col(2))?;
    let status: String = r.get(col(11))?;
    let gmail_id: Option<String> = r.get(col(8))?;
    let first_seen_at: i64 = r.get(col(9))?;
    let (Some(portal), Ok(url), Some(desc_status), Some(first_seen_at)) = (
        Portal::from_key(&portal),
        Url::parse(&url),
        DescStatus::parse(&status),
        from_db(first_seen_at),
    ) else {
        return Ok(Err(Error::Corrupt(format!(
            "job row {portal}/{url}/{status}"
        ))));
    };
    Ok(Ok(JobRow {
        key: JobKey {
            portal,
            id: r.get(col(1))?,
        },
        url,
        title: r.get(col(3))?,
        company: r.get(col(4))?,
        location: r.get(col(5))?,
        mail_date: r.get::<_, Option<i64>>(col(6))?.and_then(from_db),
        mail_subject: r.get(col(7))?,
        gmail_id: gmail_id.and_then(|s| s.parse().ok()),
        first_seen_at,
        first_seen_run: r.get(col(10))?,
        desc_status,
        desc_short: r.get(col(12))?,
        desc_closed: r.get(col(13))?,
        desc_len: r.get(col(14))?,
        desc_fetched_at: r.get::<_, Option<i64>>(col(15))?.and_then(from_db),
        desc_attempts: r.get(col(16))?,
        desc_error: r.get(col(17))?,
        txt_name: r.get(col(18))?,
        desc_attempted_at: r.get::<_, Option<i64>>(col(19))?.and_then(from_db),
        read_at: r.get::<_, Option<i64>>(col(20))?.and_then(from_db),
        pinned_at: r.get::<_, Option<i64>>(col(21))?.and_then(from_db),
        match_: super::matches::decode_match(
            r.get::<_, Option<String>>(col(22))?.as_deref(),
            r.get(col(23))?,
            r.get::<_, Option<String>>(col(24))?.as_deref(),
        ),
        match_rev: r.get(col(25))?,
        facts: r
            .get::<_, Option<String>>(col(26))?
            .and_then(|json| serde_json::from_str(&json).ok()),
    }))
}

/// Records one entry. Merge rule for known jobs: mail details and first sighting stay
/// unchanged; title, company and location are only filled in when they are empty or the
/// placeholder - never replaced just because another value is longer.
fn upsert(
    conn: &Connection,
    run: i64,
    posting: &Posting,
    mail: MailRef<'_>,
    now: Timestamp,
) -> Result<Seen> {
    let key = &posting.key;
    let known: Option<(i64, String, String, String)> = conn
        .query_row(
            "SELECT last_seen_run, title, company, location FROM job
             WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((last_run, title, company, location)) = known else {
        conn.execute(
            "INSERT INTO job (portal, job_id, url, title, company, location, mail_date,
                              mail_subject, gmail_id, first_seen_at, first_seen_run,
                              last_seen_run, search)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12)",
            params![
                key.portal.key(),
                key.id,
                posting.url.as_str(),
                posting.title,
                posting.company,
                posting.location,
                mail.date.map(to_db),
                mail.subject,
                mail.gmail_id.map(|id| id.to_string()),
                to_db(now),
                run,
                search_text(&posting.title, &posting.company, &posting.location, ""),
            ],
        )?;
        bump(conn)?;
        return Ok(Seen::New);
    };
    // The last sighting is invisible to export and UI - hence no bump().
    conn.execute(
        "UPDATE job SET last_seen_run = ?3 WHERE portal = ?1 AND job_id = ?2",
        params![key.portal.key(), key.id, run],
    )?;
    let new_title = if !is_usable_title(&title) && posting.has_real_title() {
        posting.title.clone()
    } else {
        title.clone()
    };
    let (new_company, new_location) =
        merge_details((&company, &location), (&posting.company, &posting.location));
    if new_title != title || new_company != company || new_location != location {
        conn.execute(
            "UPDATE job SET title = ?3, company = ?4, location = ?5,
                            match_rev = CASE WHEN title <> ?3 THEN NULL ELSE match_rev END
             WHERE portal = ?1 AND job_id = ?2",
            params![
                key.portal.key(),
                key.id,
                new_title,
                new_company,
                new_location
            ],
        )?;
        refresh_search(conn, key)?;
        bump(conn)?;
    }
    Ok(if last_run == run {
        Seen::DupInRun
    } else {
        Seen::KnownBefore
    })
}

/// Company and location are only taken over as a pair - never mixed from two different
/// mails: when there are no details yet, or when the stored "company" was really just a
/// place ("D-20038 Hamburg") and the new mail names a real company.
fn merge_details(stored: (&str, &str), new: (&str, &str)) -> (String, String) {
    let keep = (stored.0.to_string(), stored.1.to_string());
    let take = (new.0.to_string(), new.1.to_string());
    if new.0.is_empty() && new.1.is_empty() {
        return keep;
    }
    if stored.0.is_empty() && stored.1.is_empty() {
        return take;
    }
    let stored_has_company = !split_company_location(stored.0, stored.1).0.is_empty();
    let new_has_company = !split_company_location(new.0, new.1).0.is_empty();
    if !stored_has_company && new_has_company {
        take
    } else {
        keep
    }
}

/// Recomputes the search column (lower case, umlauts included).
fn refresh_search(conn: &Connection, key: &JobKey) -> Result<()> {
    let row: Option<(String, String, String, Option<String>)> = conn
        .query_row(
            "SELECT title, company, location, desc_text FROM job WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    if let Some((title, company, location, text)) = row {
        conn.execute(
            "UPDATE job SET search = ?3 WHERE portal = ?1 AND job_id = ?2",
            params![
                key.portal.key(),
                key.id,
                search_text(&title, &company, &location, text.as_deref().unwrap_or(""))
            ],
        )?;
    }
    Ok(())
}

fn search_text(title: &str, company: &str, location: &str, text: &str) -> String {
    fold(&format!("{title}\n{company}\n{location}\n{text}"))
}

/// Comparison form for the search: lower case (Unicode, so umlauts too).
fn fold(text: &str) -> String {
    text.to_lowercase()
}

/// `LIKE` pattern of a search term; an empty search matches everything (`None`).
fn like_pattern(search: Option<&str>) -> Option<String> {
    search
        .map(|s| format!("%{}%", escape_like(&fold(s.trim()))))
        .filter(|p| p != "%%")
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::job_link;
    use crate::store::test_support::{mail, now, posting};

    #[test]
    fn seen_counts_new_known_and_duplicates() {
        let store = Store::in_memory().unwrap();
        let run1 = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4123456789/",
            "Interim CFO",
            "Nordlicht AG",
            "Hamburg",
        );
        assert_eq!(
            store.upsert_posting(run1, &a, mail(), now()).unwrap(),
            Seen::New
        );
        // The same job in a second mail of the same run.
        assert_eq!(
            store.upsert_posting(run1, &a, mail(), now()).unwrap(),
            Seen::DupInRun
        );
        let run2 = store.begin_run().unwrap();
        assert_eq!(
            store.upsert_posting(run2, &a, mail(), now()).unwrap(),
            Seen::KnownBefore
        );
        // A known job is a duplicate in the second mail of the same run too, not
        // "already known" once more.
        assert_eq!(
            store.upsert_posting(run2, &a, mail(), now()).unwrap(),
            Seen::DupInRun
        );
        // Two link forms of the same project = the same job.
        let f1 = posting(
            "https://www.freelance.de/project/index.php?id=1255067",
            "SAP",
            "",
            "",
        );
        let f2 = posting(
            "https://www.freelance.de/projekte/projekt-1255067-sap",
            "SAP",
            "",
            "",
        );
        assert_eq!(
            store.upsert_posting(run2, &f1, mail(), now()).unwrap(),
            Seen::New
        );
        assert_eq!(
            store.upsert_posting(run2, &f2, mail(), now()).unwrap(),
            Seen::DupInRun
        );
        assert_eq!(store.job_count().unwrap(), 2);
    }

    #[test]
    fn merge_fills_gaps_but_never_replaces() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let url = "https://www.linkedin.com/jobs/view/4123456789/";
        store
            .upsert_posting(run, &posting(url, "", "", "Hamburg"), mail(), now())
            .unwrap();
        let later = MailRef {
            subject: "Andere Mail",
            date: None,
            gmail_id: None,
        };
        store
            .upsert_posting(
                run,
                &posting(url, "Interim CFO", "Nordlicht AG", "Berlin (anderer Wert)"),
                later,
                now(),
            )
            .unwrap();
        store
            .upsert_posting(
                run,
                &posting(url, "Interim CFO (m/w/d) – längerer Titel", "X", "Y"),
                later,
                now(),
            )
            .unwrap();
        let job = store.job(&job_link(url).unwrap().key).unwrap().unwrap();
        assert_eq!(job.title, "Interim CFO"); // placeholder replaced, then never again
        // Company and location as a pair from the same mail: the first named only a place,
        // the second company and place - never mixed; the third changes nothing.
        assert_eq!(job.company, "Nordlicht AG");
        assert_eq!(job.location, "Berlin (anderer Wert)");
        assert_eq!(job.mail_subject, "3 neue Jobs"); // the first sighting stays
        assert_eq!(job.gmail_id, Some(0x1a2b));
    }

    /// The "company" was only a place (freelance.de often names just the city after the
    /// title) - a later mail with a real company replaces the pair; a real company is never
    /// replaced by another one.
    #[test]
    fn a_real_company_replaces_a_place_only_pair() {
        assert_eq!(
            merge_details(
                ("D-20038 Hamburg", ""),
                ("Muster Consulting GmbH", "D-20038 Hamburg")
            ),
            (
                "Muster Consulting GmbH".to_string(),
                "D-20038 Hamburg".to_string()
            )
        );
        assert_eq!(
            merge_details(("Firma A", "Köln"), ("Firma B", "Berlin")),
            ("Firma A".to_string(), "Köln".to_string())
        );
        assert_eq!(
            merge_details(("Firma A", ""), ("", "")),
            ("Firma A".to_string(), String::new())
        );
    }

    #[test]
    fn fetch_lifecycle_and_queue_order() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let old_mail = MailRef {
            subject: "alt",
            date: Some("2026-08-01T07:00:00Z".parse().unwrap()),
            gmail_id: None,
        };
        let new_mail = MailRef {
            subject: "neu",
            date: Some("2026-09-18T07:00:00Z".parse().unwrap()),
            gmail_id: None,
        };
        let mid_mail = MailRef {
            subject: "mittel",
            date: Some("2026-09-10T07:00:00Z".parse().unwrap()),
            gmail_id: None,
        };
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        let b = posting(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "B",
            "",
            "",
        );
        let c = posting(
            "https://www.linkedin.com/jobs/view/4000000003/",
            "C",
            "",
            "",
        );
        let d = posting(
            "https://www.linkedin.com/jobs/view/4000000004/",
            "D",
            "",
            "",
        );
        store.upsert_posting(run, &a, mid_mail, now()).unwrap();
        store.upsert_posting(run, &b, new_mail, now()).unwrap();
        store.upsert_posting(run, &c, old_mail, now()).unwrap(); // older than 30 days
        store.upsert_posting(run, &d, new_mail, now()).unwrap();
        let month = SignedDuration::from_hours(30 * 24);
        let half_day = SignedDuration::from_hours(12);

        // d fails now -> it is not in the queue for the time being.
        assert_eq!(
            store.record_failed(&d.key, "leer", now()).unwrap(),
            DescStatus::Failed
        );
        let queue: Vec<_> = store
            .fetch_queue(now(), month, half_day)
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(queue, ["B", "A"]); // newest mail first, the old mail not automatically

        // 13 h later: d is due again, but after the open ones.
        let later = now() + SignedDuration::from_hours(13);
        let queue: Vec<_> = store
            .fetch_queue(later, month, half_day)
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(queue, ["B", "A", "D"]);

        assert_eq!(
            store.record_failed(&d.key, "leer", later).unwrap(),
            DescStatus::Failed
        );
        assert_eq!(
            store.record_failed(&d.key, "leer", later).unwrap(),
            DescStatus::Unfetchable
        );
        store
            .record_text(&b.key, "Volltext B", false, false, now())
            .unwrap();
        store.record_gone(&a.key, now()).unwrap();
        let later2 = later + SignedDuration::from_hours(24);
        assert!(
            store
                .fetch_queue(later2, month, half_day)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store.description(&b.key).unwrap().as_deref(),
            Some("Volltext B")
        );
        assert_eq!(store.description(&a.key).unwrap(), None);
    }

    /// A failure or "gone" after a successful fetch does not downgrade the job - the text
    /// stays, nothing is fetched again.
    #[test]
    fn success_is_never_downgraded() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting("https://www.freelancermap.de/nproj/12345.html", "A", "", "");
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        store
            .record_text(&a.key, "Volltext", false, false, now())
            .unwrap();
        let rev = store.data_rev().unwrap();
        assert_eq!(
            store.record_failed(&a.key, "leer", now()).unwrap(),
            DescStatus::Ok
        );
        store.record_gone(&a.key, now()).unwrap();
        let job = store.job(&a.key).unwrap().unwrap();
        assert_eq!((job.desc_status, job.desc_attempts), (DescStatus::Ok, 0));
        assert_eq!(
            store.description(&a.key).unwrap().as_deref(),
            Some("Volltext")
        );
        assert_eq!(store.data_rev().unwrap(), rev);
    }

    #[test]
    fn txt_written_exactly_once() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        assert!(store.txt_jobs(false).unwrap().is_empty());
        store
            .record_text(&a.key, "Text", false, false, now())
            .unwrap();
        let pending = store.txt_jobs(false).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1, "Text");
        store
            .mark_txt_written(&a.key, "20260918_LinkedIn_A_4000000001.txt", now())
            .unwrap();
        assert!(store.txt_jobs(false).unwrap().is_empty());
        assert_eq!(
            store.txt_names().unwrap(),
            ["20260918_LinkedIn_A_4000000001.txt"]
        );
        // Rewriting sees every job - without touching the mark.
        assert_eq!(store.txt_jobs(true).unwrap().len(), 1);
        assert!(store.txt_jobs(false).unwrap().is_empty());
    }

    /// Failure reasons often come from the page (redirect target, script error): one short
    /// line is stored - it travels all the way to the UI in the run events.
    #[test]
    fn failure_reasons_are_short_and_flat() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting("https://www.freelancermap.de/nproj/12345.html", "A", "", "");
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        let reason = format!(
            "unerwartete Umleitung nach /{}\nzweite Zeile",
            "x".repeat(20_000)
        );
        store.record_failed(&a.key, &reason, now()).unwrap();
        let error = store.job(&a.key).unwrap().unwrap().desc_error.unwrap();
        assert_eq!(error.chars().count(), MAX_ERROR_CHARS);
        assert!(!error.contains('\n'));
    }

    /// An alert mail is taken over completely or not at all.
    #[test]
    fn an_alert_mail_is_one_change() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let alert = AlertMail {
            key: "gm:1".into(),
            portal: Portal::LinkedIn,
            subject: "2 neue Jobs".into(),
            sender: "LinkedIn".into(),
            date: None,
            gmail_id: Some(1),
            postings: vec![
                posting(
                    "https://www.linkedin.com/jobs/view/4000000001/",
                    "A",
                    "",
                    "",
                ),
                posting(
                    "https://www.linkedin.com/jobs/view/4000000001/",
                    "A",
                    "",
                    "",
                ),
                posting(
                    "https://www.linkedin.com/jobs/view/4000000002/",
                    "B",
                    "",
                    "",
                ),
            ],
        };
        assert_eq!(
            store.record_alert(run, &alert, now()).unwrap(),
            [Seen::New, Seen::DupInRun, Seen::New]
        );
        assert_eq!(store.job_count().unwrap(), 2);
        // If one statement fails, nothing of the mail remains.
        store
            .conn()
            .execute_batch(
                "CREATE TRIGGER no_b BEFORE INSERT ON job WHEN NEW.job_id = '4000000003'
                 BEGIN SELECT RAISE(ABORT, 'Test'); END;",
            )
            .unwrap();
        let failing = AlertMail {
            key: "gm:2".into(),
            postings: vec![
                posting(
                    "https://www.linkedin.com/jobs/view/4000000004/",
                    "D",
                    "",
                    "",
                ),
                posting(
                    "https://www.linkedin.com/jobs/view/4000000003/",
                    "C",
                    "",
                    "",
                ),
            ],
            ..alert
        };
        assert!(store.record_alert(run, &failing, now()).is_err());
        assert_eq!(store.job_count().unwrap(), 2);
        assert!(store.zero_posting_mails(run).unwrap().is_empty());
        let mails: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM alert_mail", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mails, 1);
    }

    #[test]
    fn search_is_case_insensitive_with_umlauts_and_wildcards_are_literal() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "Überwachung SAP",
            "Müller GmbH",
            "Köln",
        );
        let b = posting(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "100% Remote",
            "",
            "",
        );
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        store.upsert_posting(run, &b, mail(), now()).unwrap();
        let find = |q: &str| {
            store
                .jobs(&JobFilter {
                    search: Some(q.into()),
                    ..JobFilter::default()
                })
                .unwrap()
                .into_iter()
                .map(|j| j.title)
                .collect::<Vec<_>>()
        };
        assert_eq!(find("überwachung"), ["Überwachung SAP"]);
        assert_eq!(find("MÜLLER"), ["Überwachung SAP"]);
        assert_eq!(find("100%"), ["100% Remote"]);
        assert!(find("%").iter().all(|t| t == "100% Remote"));
        assert_eq!(find("  ").len(), 2); // empty search = everything
        store
            .record_text(
                &a.key,
                "Wir suchen einen Projektleiter",
                false,
                false,
                now(),
            )
            .unwrap();
        assert_eq!(find("projektleiter"), ["Überwachung SAP"]);
    }

    #[test]
    fn filter_by_run_and_newest_first() {
        let store = Store::in_memory().unwrap();
        let run1 = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        store.upsert_posting(run1, &a, mail(), now()).unwrap();
        let run2 = store.begin_run().unwrap();
        let b = posting(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "B",
            "",
            "",
        );
        let later = now() + SignedDuration::from_mins(1);
        store.upsert_posting(run2, &b, mail(), later).unwrap();
        store.upsert_posting(run2, &a, mail(), later).unwrap(); // known, does not count as new
        let only_new: Vec<_> = store
            .jobs(&JobFilter {
                first_seen_run: Some(run2),
                ..JobFilter::default()
            })
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(only_new, ["B"]);
        let all: Vec<_> = store
            .jobs(&JobFilter::default())
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(all, ["B", "A"]);
    }

    #[test]
    fn data_rev_changes_only_with_visible_content() {
        let store = Store::in_memory().unwrap();
        let v0 = store.data_rev().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        let v1 = store.data_rev().unwrap();
        assert!(v1 > v0);
        // The same mail again: nothing changes, so no new export either.
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        assert_eq!(store.data_rev().unwrap(), v1);
        store
            .upsert_posting(
                run,
                &posting(
                    "https://www.linkedin.com/jobs/view/4000000001/",
                    "A",
                    "Firma",
                    "",
                ),
                mail(),
                now(),
            )
            .unwrap();
        let v2 = store.data_rev().unwrap();
        assert!(v2 > v1);
        store
            .record_text(&a.key, "Text", false, false, now())
            .unwrap();
        assert!(store.data_rev().unwrap() > v2);
        let v3 = store.data_rev().unwrap();
        store.mark_txt_written(&a.key, "x.txt", now()).unwrap();
        assert_eq!(store.data_rev().unwrap(), v3);
    }

    /// A mail that linked the title as a bare address left the URL as the title - and it
    /// stayed, because it was "not empty". A later run with a real title now replaces it. A
    /// proper title, on the other hand, stays untouched.
    #[test]
    fn a_stored_url_is_no_title_and_gets_replaced() {
        let store = Store::in_memory().unwrap();
        let url = "https://www.freelance.de/project/index.php?id=1291188";
        let key = job_link(url).unwrap().key;
        let seen = |title: &str| {
            let run = store.begin_run().unwrap();
            let mail = AlertMail {
                key: format!("m{run}"),
                portal: Portal::FreelanceDe,
                subject: "1 neues Projekt".into(),
                sender: "freelance.de".into(),
                date: None,
                gmail_id: Some(u64::try_from(run).unwrap()),
                postings: vec![posting(url, title, "", "")],
            };
            store.record_alert(run, &mail, now()).unwrap();
            store.job(&key).unwrap().unwrap().title
        };
        // The mail only links the address - for lack of anything better it becomes the title.
        assert_eq!(seen(url), url);
        // A run with a real title replaces it.
        assert_eq!(
            seen("Senior Requirements Engineer (w/m/d)"),
            "Senior Requirements Engineer (w/m/d)"
        );
        // A real title is not replaced by another one.
        assert_eq!(
            seen("Etwas anderes"),
            "Senior Requirements Engineer (w/m/d)"
        );
    }
}
