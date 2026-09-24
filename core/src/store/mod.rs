//! The app's memory: one `SQLite` file, the single source of truth about jobs, mails and job
//! details. Excel and TXT are generated from it - never the other way round.
//!
//! One connection behind a mutex; every method is short and synchronous (the caller never
//! holds the lock across an `await`).
//!
//! `schema` creates and migrates the tables, `jobs` holds the job, alert mail, job detail
//! and text file methods, `matches` reserves the columns of the coming schema 3.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use jiff::Timestamp;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::error::{Error, Result};
use crate::portal::Portal;
use crate::time::{from_db, to_db};

mod jobs;
pub mod matches;
mod schema;

pub use jobs::{AlertMailRow, JobFilter, JobRow, MailRef, PortalCount, Seen};

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Opens (or creates) the database and brings the schema up to date.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Database in memory only (dry run, tests).
    pub fn in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        schema::migrate(&conn)?;
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic in another caller leaves the connection intact (SQLite rolls back open
        // transactions itself) - so the poisoning is ignored.
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Several statements as **one** change: all or nothing - a crash in between would
    /// otherwise leave, say, a full text without its search column or without a new change
    /// counter - and one write to disk instead of several.
    fn write<T>(&self, work: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn();
        let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)?;
        let value = work(&tx)?;
        tx.commit()?;
        Ok(value)
    }

    // ------------------------------------------------------------------ Runs

    /// Starts a run and returns its number (sequential, across restarts).
    pub fn begin_run(&self) -> Result<i64> {
        let conn = self.conn();
        let next = kv_get_i64(&conn, "run_seq")?.unwrap_or(0) + 1;
        kv_set(&conn, "run_seq", &next.to_string())?;
        Ok(next)
    }

    /// Start of the last fully successful mailbox scan for a portal.
    pub fn last_scan(&self, portal: Portal) -> Result<Option<Timestamp>> {
        Ok(kv_get_i64(&self.conn(), &scan_key(portal))?.and_then(from_db))
    }

    /// Forgets the scan state of every portal - after a switch of the Gmail account the new
    /// mailbox starts with the first run (7 days) instead of at the state of the old one.
    pub fn clear_scan_state(&self) -> Result<()> {
        self.conn()
            .execute("DELETE FROM kv WHERE key LIKE 'last_scan:%'", [])?;
        Ok(())
    }

    pub fn set_last_scan(&self, portal: Portal, at: Timestamp) -> Result<()> {
        kv_set(&self.conn(), &scan_key(portal), &to_db(at).to_string())
    }

    // ------------------------------------------------------------------ Key/value

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        kv_get(&self.conn(), key)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        kv_set(&self.conn(), key, value)
    }

    /// Change counter of the job table: rises with every change that would be visible in an
    /// export. Exports are only regenerated when it has risen since the last export.
    pub fn data_rev(&self) -> Result<i64> {
        Ok(kv_get_i64(&self.conn(), "data_rev")?.unwrap_or(0))
    }
}

// ---------------------------------------------------------------------- Helpers

fn scan_key(portal: Portal) -> String {
    format!("last_scan:{}", portal.key())
}

fn kv_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
        .optional()?)
}

fn kv_get_i64(conn: &Connection, key: &str) -> Result<Option<i64>> {
    kv_get(conn, key)?
        .map(|v| {
            v.parse()
                .map_err(|_| Error::Corrupt(format!("{key} = {v}")))
        })
        .transpose()
}

fn kv_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Raises the change counter (see [`Store::data_rev`]).
fn bump(conn: &Connection) -> Result<()> {
    let next = kv_get_i64(conn, "data_rev")?.unwrap_or(0) + 1;
    kv_set(conn, "data_rev", &next.to_string())
}

/// Fixtures shared by the tests of `schema` and `jobs`.
#[cfg(test)]
mod test_support {
    use jiff::Timestamp;

    use super::MailRef;
    use crate::model::Posting;
    use crate::portal::job_link;

    pub(super) fn now() -> Timestamp {
        "2026-09-19T10:00:00Z".parse().unwrap()
    }

    pub(super) fn posting(url: &str, title: &str, company: &str, location: &str) -> Posting {
        let link = job_link(url).unwrap();
        Posting::new(link.key, link.url, title, company, location)
    }

    pub(super) fn mail() -> MailRef<'static> {
        MailRef {
            subject: "3 neue Jobs",
            date: Some("2026-09-18T07:00:00Z".parse().unwrap()),
            gmail_id: Some(0x1a2b),
        }
    }
}
