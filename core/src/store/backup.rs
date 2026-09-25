//! Copies of the database in `backups/` next to it - in the data folder, never in the work
//! folder: the marks, tombstones, overrides and old texts it holds come back from no fetch.
//! One copy before every migration (`jobs.pre-v4.db`: the database as schema 4 left it) and
//! one a day (`jobs-2026-09-25.db`, local date), a few of each kept. `VACUUM INTO` writes a
//! consistent, compact copy while the app keeps working; it goes to a temporary name first,
//! so a copy that was cut off never looks like one. The dry run (in memory) has none, and
//! "Alles zurücksetzen" deletes the folder (`reset.rs`).

use std::path::{Path, PathBuf};

use jiff::civil::Date;
use rusqlite::Connection;

use crate::error::{Error, Result};

/// The folder of the copies, next to the database.
pub const BACKUP_DIR: &str = "backups";
/// Daily copies kept.
const DAILY_KEPT: usize = 3;
/// Copies from before a migration kept.
const MIGRATION_KEPT: usize = 3;
/// Name parts of the copies.
const DAILY_PREFIX: &str = "jobs-";
const MIGRATION_PREFIX: &str = "jobs.pre-v";
const SUFFIX: &str = ".db";
/// A copy while it is written.
const PARTIAL: &str = ".partial";

/// The folder of the copies of the database at `db`.
pub fn backup_dir(db: &Path) -> PathBuf {
    db.parent().unwrap_or(Path::new(".")).join(BACKUP_DIR)
}

/// Before a migration from `version`: a copy of the database as that schema left it. A copy
/// from an earlier attempt at the same step stays (it is the same schema). `None` if there
/// was one already.
pub(super) fn before_migration(
    conn: &Connection,
    db: &Path,
    version: i64,
) -> Result<Option<PathBuf>> {
    let dir = backup_dir(db);
    let target = dir.join(format!("{MIGRATION_PREFIX}{version}{SUFFIX}"));
    if target.exists() {
        return Ok(None);
    }
    copy(conn, &target)?;
    prune(&dir, MIGRATION_KEPT, migration_version);
    Ok(Some(target))
}

/// The copy of the day `today` (local), unless there is one: through a connection of its
/// own, so the app's connection is never held while it writes. `None` if the day has its
/// copy already.
pub(super) fn daily(db: &Path, today: Date) -> Result<Option<PathBuf>> {
    let dir = backup_dir(db);
    let target = dir.join(format!("{DAILY_PREFIX}{today}{SUFFIX}"));
    if target.exists() {
        return Ok(None);
    }
    let conn = Connection::open(db)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    copy(&conn, &target)?;
    prune(&dir, DAILY_KEPT, daily_date);
    Ok(Some(target))
}

/// `VACUUM INTO` a temporary name, then the real one.
fn copy(conn: &Connection, target: &Path) -> Result<()> {
    let dir = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let mut partial = target.as_os_str().to_owned();
    partial.push(PARTIAL);
    let partial = PathBuf::from(partial);
    // A copy cut off before: VACUUM INTO writes only into a missing or empty file.
    match std::fs::remove_file(&partial) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::io(&partial, e)),
    }
    let name = partial
        .to_str()
        .ok_or_else(|| Error::Corrupt(format!("backup path {}", partial.display())))?;
    if let Err(e) = conn.execute("VACUUM INTO ?1", [name]) {
        let _ = std::fs::remove_file(&partial);
        return Err(e.into());
    }
    std::fs::rename(&partial, target).map_err(|e| Error::io(target, e))
}

/// Keeps the `kept` newest copies of one kind in `dir` (newest by `order`, a name that is no
/// copy of this kind has none), deletes the older ones and the copies of this kind that were
/// cut off before (the app ended while it wrote one; only a copy of the same day or schema
/// would replace it). A failure only goes to the log.
fn prune<K: Ord>(dir: &Path, kept: usize, order: impl Fn(&str) -> Option<K>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut copies: Vec<(K, PathBuf)> = Vec::new();
    let mut old: Vec<PathBuf> = Vec::new();
    for entry in entries.filter_map(std::result::Result::ok) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if let Some(key) = order(name) {
            copies.push((key, entry.path()));
        } else if name.strip_suffix(PARTIAL).and_then(&order).is_some() {
            old.push(entry.path());
        }
    }
    copies.sort_by(|a, b| b.0.cmp(&a.0));
    old.extend(copies.into_iter().skip(kept).map(|(_, path)| path));
    for path in old {
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("old database copy {} not deleted: {e}", path.display());
        }
    }
}

/// The day of a daily copy's name.
fn daily_date(name: &str) -> Option<Date> {
    name.strip_prefix(DAILY_PREFIX)?
        .strip_suffix(SUFFIX)?
        .parse()
        .ok()
}

/// The schema of a migration copy's name.
fn migration_version(name: &str) -> Option<i64> {
    name.strip_prefix(MIGRATION_PREFIX)?
        .strip_suffix(SUFFIX)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    /// The `marker` of a copy, read without changing it.
    fn marker(copy: &Path) -> Option<String> {
        Connection::open(copy)
            .unwrap()
            .query_row("SELECT value FROM kv WHERE key = 'marker'", [], |r| {
                r.get(0)
            })
            .ok()
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    /// One copy a day, a readable database with the jobs of the day; the three newest days
    /// stay, other files in the folder too.
    #[test]
    fn a_copy_a_day_and_three_kept() {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("jobs.db");
        let store = Store::open(&db).unwrap();
        store.kv_set("marker", "eins").unwrap();
        let first = store.backup_daily(day("2026-09-20")).unwrap().unwrap();
        assert_eq!(
            first,
            root.path().join("backups").join("jobs-2026-09-20.db")
        );
        assert_eq!(
            store.backup_daily(day("2026-09-20")).unwrap(),
            None,
            "once a day"
        );
        assert_eq!(marker(&first).as_deref(), Some("eins"));
        std::fs::write(backup_dir(&db).join("notes.txt"), b"mine").unwrap();
        for date in ["2026-09-21", "2026-09-22", "2026-09-23", "2026-09-24"] {
            store.backup_daily(day(date)).unwrap().unwrap();
        }
        assert_eq!(
            names(&backup_dir(&db)),
            [
                "jobs-2026-09-22.db",
                "jobs-2026-09-23.db",
                "jobs-2026-09-24.db",
                "notes.txt"
            ]
        );
    }

    /// The dry run keeps no copy: its database lives in memory only.
    #[test]
    fn a_database_in_memory_has_no_copy() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.backup_daily(day("2026-09-20")).unwrap(), None);
    }

    /// A copy that was cut off is written again, never taken for a copy; one of an earlier
    /// day, which no copy replaces, goes with the next copy. Other files stay.
    #[test]
    fn a_cut_off_copy_is_replaced() {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("jobs.db");
        let store = Store::open(&db).unwrap();
        store.kv_set("marker", "ganz").unwrap();
        let dir = backup_dir(&db);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("jobs-2026-09-20.db.partial"), b"halb").unwrap();
        let copy = store.backup_daily(day("2026-09-20")).unwrap().unwrap();
        assert_eq!(marker(&copy).as_deref(), Some("ganz"));
        assert_eq!(names(&dir), ["jobs-2026-09-20.db"]);

        std::fs::write(dir.join("jobs-2026-09-18.db.partial"), b"halb").unwrap();
        std::fs::write(dir.join("notes.partial"), b"mine").unwrap();
        store.backup_daily(day("2026-09-21")).unwrap().unwrap();
        assert_eq!(
            names(&dir),
            ["jobs-2026-09-20.db", "jobs-2026-09-21.db", "notes.partial"]
        );
    }

    #[test]
    fn copy_names_are_read_back() {
        assert_eq!(daily_date("jobs-2026-09-20.db"), Some(day("2026-09-20")));
        assert_eq!(daily_date("jobs-2026-09-20.db.partial"), None);
        assert_eq!(daily_date("jobs.pre-v4.db"), None);
        assert_eq!(migration_version("jobs.pre-v4.db"), Some(4));
        assert_eq!(migration_version("jobs.pre-v12.db"), Some(12));
        assert_eq!(migration_version("jobs-2026-09-20.db"), None);
    }
}
