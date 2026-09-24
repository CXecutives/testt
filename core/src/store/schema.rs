//! Table layout and the migration chain. Opening a database brings it up to date one step
//! at a time (`BEGIN IMMEDIATE; step; user_version; COMMIT` per step, never a jump to the
//! latest version); a database from a newer version is refused and left untouched. A fresh
//! database is created as schema 2 and walks the same chain, so fresh and migrated
//! databases cannot drift apart.

use std::fmt::Write as _;

use rusqlite::Connection;

use super::marks::SCHEMA_4_JOB_COLUMNS;
use super::matches::SCHEMA_3_JOB_COLUMNS;
use crate::error::{Error, Result};

pub(super) const SCHEMA_VERSION: i64 = 4;

/// Schema 2, the base of every fresh database. Frozen: later changes are migration steps.
/// The same layout lies in `core/tests/fixtures/schema_v2.sql` for the migration tests.
const SCHEMA_2: &str = "
CREATE TABLE job (
    portal            TEXT    NOT NULL,
    job_id            TEXT    NOT NULL,
    url               TEXT    NOT NULL,
    title             TEXT    NOT NULL,
    company           TEXT    NOT NULL,
    location          TEXT    NOT NULL,
    mail_date         INTEGER,
    mail_subject      TEXT    NOT NULL,
    gmail_id          TEXT,
    first_seen_at     INTEGER NOT NULL,
    first_seen_run    INTEGER NOT NULL,
    last_seen_run     INTEGER NOT NULL,
    desc_status       TEXT    NOT NULL DEFAULT 'missing',
    desc_short        INTEGER NOT NULL DEFAULT 0,
    desc_closed       INTEGER NOT NULL DEFAULT 0,
    desc_text         TEXT,
    desc_fetched_at   INTEGER,
    desc_attempted_at INTEGER,
    desc_attempts     INTEGER NOT NULL DEFAULT 0,
    desc_error        TEXT,
    txt_name          TEXT,
    txt_written_at    INTEGER,
    search            TEXT    NOT NULL,
    PRIMARY KEY (portal, job_id)
) WITHOUT ROWID;
CREATE INDEX job_by_run ON job (first_seen_run);
CREATE TABLE alert_mail (
    mail_key       TEXT    PRIMARY KEY,
    portal         TEXT    NOT NULL,
    subject        TEXT    NOT NULL,
    mail_date      INTEGER,
    gmail_id       TEXT,
    n_postings     INTEGER NOT NULL,
    last_seen_run  INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE kv (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;
";

/// From schema 1 to 2: `alert_mail` lost two columns that were never used. The table only
/// remembers "mail without recognised entries" and rebuilds itself on the next scan - so
/// recreating it is enough, no copying needed.
const MIGRATE_1_TO_2: &str = "
DROP TABLE alert_mail;
CREATE TABLE alert_mail (
    mail_key       TEXT    PRIMARY KEY,
    portal         TEXT    NOT NULL,
    subject        TEXT    NOT NULL,
    mail_date      INTEGER,
    gmail_id       TEXT,
    n_postings     INTEGER NOT NULL,
    last_seen_run  INTEGER NOT NULL
) WITHOUT ROWID;
";

/// From schema 2 to 3: the nullable match, read and pin columns. Every job first seen
/// before the last mailbox run counts as read (the user has seen those lists); the jobs of
/// the last run stay new.
fn migrate_2_to_3() -> String {
    let mut sql = String::new();
    for (name, sql_type) in SCHEMA_3_JOB_COLUMNS {
        let _ = writeln!(sql, "ALTER TABLE job ADD COLUMN {name} {sql_type};");
    }
    sql.push_str(
        "UPDATE job SET read_at = first_seen_at
         WHERE first_seen_run < COALESCE(
             (SELECT CAST(value AS INTEGER) FROM kv WHERE key = 'last_scan_run'), 0);\n",
    );
    sql
}

/// From schema 3 to 4: the nullable columns of the user's marks (application status, note,
/// hidden). Nothing else changes: every job starts without a status, note or "hidden".
fn migrate_3_to_4() -> String {
    let mut sql = String::new();
    for (name, sql_type) in SCHEMA_4_JOB_COLUMNS {
        let _ = writeln!(sql, "ALTER TABLE job ADD COLUMN {name} {sql_type};");
    }
    sql
}

/// One step per version: `steps()[v - 1]` leads from `v` to `v + 1`.
fn steps() -> Vec<String> {
    vec![
        MIGRATE_1_TO_2.to_string(),
        migrate_2_to_3(),
        migrate_3_to_4(),
    ]
}

/// Brings a freshly opened connection to the current schema: creates the tables in an empty
/// database, migrates an older one step by step and refuses a newer one.
pub(super) fn migrate(conn: &Connection) -> Result<()> {
    let mut version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(Error::NewerSchema(version));
    }
    if version == 0 {
        // Schema and version number in one transaction: a crash in between would otherwise
        // leave tables with version 0, and every later start would fail.
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {SCHEMA_2} PRAGMA user_version = 2; COMMIT;"
        ))?;
        version = 2;
    }
    let steps = steps();
    while version < SCHEMA_VERSION {
        let step = usize::try_from(version - 1)
            .ok()
            .and_then(|i| steps.get(i))
            .ok_or_else(|| Error::Corrupt(format!("no migration from schema {version}")))?;
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {step} PRAGMA user_version = {}; COMMIT;",
            version + 1
        ))?;
        version += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::model::AlertMail;
    use crate::portal::Portal;
    use crate::store::Store;
    use crate::store::test_support::{mail, now, posting};

    /// The frozen schema 2 - the "old" database of the migration tests.
    const FIXTURE_V2: &str = include_str!("../../tests/fixtures/schema_v2.sql");
    /// The frozen schema 3 - the database before the user's marks.
    const FIXTURE_V3: &str = include_str!("../../tests/fixtures/schema_v3.sql");

    fn columns(conn: &Connection, table: &str) -> Vec<(String, String, bool)> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn indexes(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index' ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn the_fixture_is_schema_2() {
        let fixture = Connection::open_in_memory().unwrap();
        fixture.execute_batch(FIXTURE_V2).unwrap();
        let code = Connection::open_in_memory().unwrap();
        code.execute_batch(SCHEMA_2).unwrap();
        for table in ["job", "alert_mail", "kv"] {
            assert_eq!(columns(&fixture, table), columns(&code, table), "{table}");
        }
    }

    /// The frozen schema 3 is what the chain made of schema 2 (the same columns in the same
    /// order), so the migration tests start from a real schema 3 database.
    #[test]
    fn the_fixture_is_schema_3() {
        let fixture = Connection::open_in_memory().unwrap();
        fixture.execute_batch(FIXTURE_V3).unwrap();
        let code = Connection::open_in_memory().unwrap();
        code.execute_batch(&format!("{SCHEMA_2}{}", migrate_2_to_3()))
            .unwrap();
        for table in ["job", "alert_mail", "kv"] {
            assert_eq!(columns(&fixture, table), columns(&code, table), "{table}");
        }
        assert_eq!(indexes(&fixture), indexes(&code));
    }

    /// Schema 3 with data: the jobs, their scores and marks stay as they were; the new
    /// columns start empty.
    #[test]
    fn a_schema_3_database_is_migrated_and_keeps_its_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{FIXTURE_V3}
             INSERT INTO job (portal, job_id, url, title, company, location, mail_subject,
                              first_seen_at, first_seen_run, last_seen_run, search,
                              match_score, match_status, read_at, pinned_at)
             VALUES ('linkedin', '4000000001', 'https://www.linkedin.com/jobs/view/4000000001/',
                     'Interim CFO', '', '', 'x', 100, 1, 1, 'interim cfo', 84, 'scored', 150,
                     160);
             INSERT INTO kv (key, value) VALUES ('last_scan_run', '1'), ('run_seq', '1');
             PRAGMA user_version = 3;"
        ))
        .unwrap();
        drop(old);
        let store = Store::open(&path).unwrap();
        assert_eq!(version(&store.conn()), SCHEMA_VERSION);
        let key = crate::portal::job_link("https://www.linkedin.com/jobs/view/4000000001/")
            .unwrap()
            .key;
        let job = store.job(&key).unwrap().unwrap();
        assert_eq!(job.title, "Interim CFO");
        assert_eq!(job.match_.map(|m| m.score), Some(84));
        assert!(job.read_at.is_some() && job.pinned_at.is_some());
        assert_eq!(
            (job.app_status, job.app_status_at, job.hidden_at),
            (None, None, None)
        );
        assert_eq!(store.note(&key).unwrap(), None);
        // The new marks work on the migrated database.
        assert!(
            store
                .set_app_status(&key, Some(crate::model::AppStatus::Applied), now())
                .unwrap()
        );
        assert!(store.set_note(&key, "Termin am Freitag").unwrap());
        assert!(store.set_hidden(&key, true, now()).unwrap());
    }

    #[test]
    fn schema_is_created_once_and_reopened() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("jobs.db");
        let store = Store::open(&path).unwrap();
        let run = store.begin_run().unwrap();
        store
            .upsert_posting(
                run,
                &posting(
                    "https://www.linkedin.com/jobs/view/4123456789/",
                    "A",
                    "",
                    "",
                ),
                mail(),
                now(),
            )
            .unwrap();
        assert_eq!(version(&store.conn()), SCHEMA_VERSION);
        drop(store);
        let again = Store::open(&path).unwrap();
        assert_eq!(again.job_count().unwrap(), 1);
        assert_eq!(again.begin_run().unwrap(), 2);
    }

    /// A database from schema 1 (with the two never-used columns) walks 1 -> 2 -> 3; without
    /// that every mailbox scan failed on "NOT NULL `alert_mail.sender`".
    #[test]
    fn a_schema_1_database_is_migrated_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{FIXTURE_V2}
             DROP TABLE alert_mail;
             CREATE TABLE alert_mail (
                 mail_key       TEXT    PRIMARY KEY,
                 portal         TEXT    NOT NULL,
                 subject        TEXT    NOT NULL,
                 sender         TEXT    NOT NULL,
                 mail_date      INTEGER,
                 gmail_id       TEXT,
                 n_postings     INTEGER NOT NULL,
                 first_seen_run INTEGER NOT NULL,
                 last_seen_run  INTEGER NOT NULL
             ) WITHOUT ROWID;
             PRAGMA user_version = 1;"
        ))
        .unwrap();
        drop(old);

        let store = Store::open(&path).unwrap();
        assert_eq!(version(&store.conn()), SCHEMA_VERSION);
        let run = store.begin_run().unwrap();
        // Exactly the step that used to fail: remembering a mail without recognised entries.
        let alert = AlertMail {
            key: "m1".into(),
            portal: Portal::LinkedIn,
            subject: "Keine Treffer".into(),
            sender: "LinkedIn".into(),
            date: None,
            gmail_id: Some(1),
            postings: Vec::new(),
        };
        store.record_alert(run, &alert, now()).unwrap();
        assert_eq!(store.zero_posting_mails(run).unwrap().len(), 1);
    }

    /// Schema 2 with data: every job before the last mailbox run counts as read, the jobs of
    /// that run stay new; nothing else changes.
    #[test]
    fn a_schema_2_database_is_migrated_and_old_jobs_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{FIXTURE_V2}
             INSERT INTO job (portal, job_id, url, title, company, location, mail_subject,
                              first_seen_at, first_seen_run, last_seen_run, search)
             VALUES ('linkedin', '4000000001', 'https://www.linkedin.com/jobs/view/4000000001/',
                     'Alt', '', '', 'x', 100, 1, 1, 'alt'),
                    ('linkedin', '4000000002', 'https://www.linkedin.com/jobs/view/4000000002/',
                     'Neu', '', '', 'x', 200, 2, 2, 'neu');
             INSERT INTO kv (key, value) VALUES ('last_scan_run', '2'), ('run_seq', '2');
             PRAGMA user_version = 2;"
        ))
        .unwrap();
        drop(old);
        let store = Store::open(&path).unwrap();
        let conn = store.conn();
        assert_eq!(version(&conn), SCHEMA_VERSION);
        let read: Vec<(String, Option<i64>)> = conn
            .prepare("SELECT title, read_at FROM job ORDER BY job_id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            read,
            [("Alt".to_string(), Some(100)), ("Neu".to_string(), None)]
        );
    }

    /// A fresh database and a migrated one have the same tables, columns and indexes.
    #[test]
    fn a_migrated_database_equals_a_fresh_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!("{FIXTURE_V2} PRAGMA user_version = 2;"))
            .unwrap();
        drop(old);
        let migrated = Store::open(&path).unwrap();
        let fresh = Store::in_memory().unwrap();
        for table in ["job", "alert_mail", "kv"] {
            assert_eq!(
                columns(&migrated.conn(), table),
                columns(&fresh.conn(), table),
                "{table}"
            );
        }
        assert_eq!(indexes(&migrated.conn()), indexes(&fresh.conn()));
        let names: Vec<String> = columns(&fresh.conn(), "job")
            .into_iter()
            .map(|(name, ..)| name)
            .collect();
        for (column, _) in SCHEMA_3_JOB_COLUMNS.iter().chain(SCHEMA_4_JOB_COLUMNS) {
            assert!(names.iter().any(|n| n == column), "{column}");
        }
        // The same from the frozen schema 3.
        let path = dir.path().join("v3.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!("{FIXTURE_V3} PRAGMA user_version = 3;"))
            .unwrap();
        drop(old);
        let migrated = Store::open(&path).unwrap();
        for table in ["job", "alert_mail", "kv"] {
            assert_eq!(
                columns(&migrated.conn(), table),
                columns(&fresh.conn(), table),
                "{table} from schema 3"
            );
        }
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        // Not "corrupt" - otherwise someone would set a healthy database aside.
        assert!(matches!(Store::open(&path), Err(Error::NewerSchema(99))));
    }

    /// A file database runs in WAL mode.
    #[test]
    fn a_file_database_uses_wal() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("jobs.db")).unwrap();
        let mode: String = store
            .conn()
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
}
