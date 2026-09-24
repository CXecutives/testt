//! Table layout and the migration chain. Opening a database brings it up to date step by
//! step; a database from a newer version is refused and left untouched.

use rusqlite::Connection;

use crate::error::{Error, Result};

const SCHEMA_VERSION: i64 = 2;

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

const SCHEMA: &str = "
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

/// Brings a freshly opened connection to the current schema: creates the tables in an empty
/// database, migrates an older one and refuses a newer one.
pub(super) fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    match version {
        // Schema and version number in one transaction: a crash in between would otherwise
        // leave tables with version 0, and every later start would fail.
        0 => conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {SCHEMA} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"
        ))?,
        // Older database: upgrade step by step, each step in one transaction.
        1 => conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {MIGRATE_1_TO_2} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"
        ))?,
        SCHEMA_VERSION => {}
        newer => return Err(Error::NewerSchema(newer)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AlertMail;
    use crate::portal::Portal;
    use crate::store::Store;
    use crate::store::test_support::{mail, now, posting};

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
        drop(store);
        let again = Store::open(&path).unwrap();
        assert_eq!(again.job_count().unwrap(), 1);
        assert_eq!(again.begin_run().unwrap(), 2);
    }

    /// A database from schema 1 (with the two never-used columns) is upgraded on open;
    /// without that every mailbox scan failed on "NOT NULL `alert_mail.sender`".
    #[test]
    fn an_old_database_is_migrated_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{SCHEMA}
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
}
