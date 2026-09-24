//! Export the app's fetched job texts as the private gold set (read-only on the database):
//!
//! ```text
//! cargo run -p jobalert-core --example export_gold -- [--db PATH] [--out DIR] [--blind [--profile FILE]...]
//! ```
//!
//! Opens the app database (default: the app's data folder) with `SQLite`'s read-only flags and
//! `query_only` (no migration, no write), takes every job with a description (`ok` or
//! `teaser`) and writes to `core/tests/fixtures/private/gold/` (ignored by git; the tool
//! refuses a folder git would track):
//! - `<portal>_<id>.txt` per job in the TXT contract format (written by the app's own
//!   `export::write_job_txt`),
//! - `jobs.json` (key, file, portal, url, title, company, raw location, mail date, first
//!   sighting, description status, page facts). Jobs of an earlier export that are no longer
//!   in the database are kept, so a reset app does not shrink the gold set; a cross-portal
//!   duplicate whose primary is exported is left out.
//!
//! `--blind` also writes `labeling/<profile>/<job>.md` (only the profile without contact data
//! and the ad: no link, no score, nothing from the app) for every profile
//! (`core/tests/fixtures/matching/sample_profile*.json` or `--profile`) and
//! `labeling/RUBRIC.md` for the blind labelers.
//!
//! Prints counts only, never ad text.

mod common;

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::gold::{self, AdView, GoldJob};
use jobalert_core::export::{TXT_DIR, write_job_txt};
use jobalert_core::model::DescStatus;
use jobalert_core::portal::{JobKey, Portal};
use jobalert_core::store::JobRow;
use jobalert_core::time::from_db;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use url::Url;

/// The app's identifier, the name of its data folder.
const APP_ID: &str = "de.cxecutives.job-alert-monitor";
/// Temporary folder inside the gold folder for `write_job_txt` (it writes into `TXT_DIR`).
const STAGING: &str = ".staging";

type Res<T> = Result<T, Box<dyn Error>>;
/// Portal label -> (full texts, teasers).
type PortalCounts = BTreeMap<&'static str, (usize, usize)>;

struct Args {
    db: Option<PathBuf>,
    out: Option<PathBuf>,
    blind: bool,
    profiles: Vec<PathBuf>,
}

fn parse_args() -> Res<Args> {
    let mut args = Args {
        db: None,
        out: None,
        blind: false,
        profiles: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .map(PathBuf::from)
                .ok_or(format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--db" => args.db = Some(value()?),
            "--out" => args.out = Some(value()?),
            "--profile" => args.profiles.push(value()?),
            "--blind" => args.blind = true,
            "--help" | "-h" => {
                println!(
                    "export_gold [--db PATH] [--out DIR] [--blind [--profile FILE]...]\n\
                     Default database: the app's data folder; default output: {}",
                    gold::GOLD_DIR
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other} (see --help)").into()),
        }
    }
    Ok(args)
}

/// `jobs.db` in the app's data folder: `%LOCALAPPDATA%` (Windows), `~/Library/Application
/// Support` (macOS), `$XDG_DATA_HOME` or `~/.local/share` (Linux).
fn default_db() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share")))
    };
    base.map(|b| b.join(APP_ID).join(jobalert_core::DB_FILE))
}

/// Read-only: no creation, no migration; `query_only` refuses any write on top.
fn open_read_only(path: &Path) -> Res<Connection> {
    if !path.is_file() {
        return Err(format!("no database at {}", path.display()).into());
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "query_only", true)?;
    Ok(conn)
}

/// Refuses an output folder that git would track (the gold set is private).
fn ensure_ignored(dir: &Path) -> Res<()> {
    let probe = dir.join(gold::JOBS_FILE);
    let status = Command::new("git")
        .args(["check-ignore", "-q", "--no-index"])
        .arg(&probe)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status();
    match status.map(|s| s.code()) {
        Ok(Some(0)) => Ok(()),
        Ok(Some(1)) => {
            Err(format!("{} is not ignored by git: refusing to write", dir.display()).into())
        }
        // Outside the repository or no git: nothing can be committed from there by accident.
        _ => {
            println!(
                "note: git could not check {}; writing anyway",
                dir.display()
            );
            Ok(())
        }
    }
}

/// A job of the database with its text.
struct DbJob {
    row: JobRow,
    text: String,
    facts: Option<Value>,
    dup_of: Option<String>,
}

/// The column, or `NULL` for an older schema without it.
fn column_or_null(columns: &HashSet<String>, name: &str) -> String {
    if columns.contains(name) {
        name.to_owned()
    } else {
        "NULL".to_owned()
    }
}

/// Jobs with a description (`ok` or `teaser`); the second value counts unreadable rows.
fn read_db(conn: &Connection) -> Res<(Vec<DbJob>, usize)> {
    let columns: HashSet<String> = conn
        .prepare("PRAGMA table_info(job)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    let sql = format!(
        "SELECT portal, job_id, url, title, company, location, mail_date, mail_subject,
                first_seen_at, first_seen_run, desc_status, desc_text, desc_fetched_at,
                {}, {}
         FROM job
         WHERE desc_status IN ('ok', 'teaser') AND desc_text IS NOT NULL AND TRIM(desc_text) <> ''
         ORDER BY portal, job_id",
        column_or_null(&columns, "desc_facts"),
        column_or_null(&columns, "dup_of"),
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let (mut jobs, mut unreadable) = (Vec::new(), 0);
    while let Some(r) = rows.next()? {
        let portal = Portal::from_key(&r.get::<_, String>(0)?);
        let url = Url::parse(&r.get::<_, String>(2)?).ok();
        let status = DescStatus::parse(&r.get::<_, String>(10)?);
        let first_seen_at = from_db(r.get(8)?);
        let (Some(portal), Some(url), Some(desc_status), Some(first_seen_at)) =
            (portal, url, status, first_seen_at)
        else {
            unreadable += 1;
            continue;
        };
        let facts = r
            .get::<_, Option<String>>(13)?
            .and_then(|f| serde_json::from_str::<Value>(&f).ok())
            .filter(|f| f.as_object().is_some_and(|o| !o.is_empty()));
        jobs.push(DbJob {
            row: JobRow {
                key: JobKey {
                    portal,
                    id: r.get(1)?,
                },
                url,
                title: r.get(3)?,
                company: r.get(4)?,
                location: r.get(5)?,
                mail_date: r.get::<_, Option<i64>>(6)?.and_then(from_db),
                mail_subject: r.get(7)?,
                gmail_id: None,
                first_seen_at,
                first_seen_run: r.get(9)?,
                desc_status,
                desc_short: false,
                desc_closed: false,
                desc_len: 0,
                desc_fetched_at: r.get::<_, Option<i64>>(12)?.and_then(from_db),
                desc_attempts: 0,
                desc_error: None,
                txt_name: None,
                desc_attempted_at: None,
                read_at: None,
                match_: None,
                match_rev: None,
                facts: None,
                app_status: None,
                app_status_at: None,
                note: None,
                archived_at: None,
                override_include: false,
            },
            text: r.get(11)?,
            facts,
            dup_of: r.get(14)?,
        });
    }
    Ok((jobs, unreadable))
}

fn gold_job(job: &DbJob, file: String) -> GoldJob {
    let row = &job.row;
    GoldJob {
        key: row.key.to_string(),
        file,
        portal: row.key.portal.key().to_owned(),
        url: row.url.to_string(),
        title: row.title.clone(),
        company: row.company.clone(),
        location: row.location.clone(),
        mail_date: row.mail_date.map(|d| d.to_string()),
        first_seen_at: row.first_seen_at.to_string(),
        desc_status: row.desc_status.as_str().to_owned(),
        facts: job.facts.clone(),
    }
}

/// Writes the TXT files with the app's writer (via a staging folder, since it writes into
/// `TXT_DIR`) and returns the gold entries plus per-portal counts (ok, teaser).
fn write_txts(out: &Path, jobs: &[DbJob]) -> Res<(Vec<GoldJob>, PortalCounts)> {
    let staging = out.join(STAGING);
    let mut entries = Vec::new();
    let mut counts = PortalCounts::new();
    let mut stems = HashSet::new();
    for job in jobs {
        let stem = gold::file_stem(job.row.key.portal.key(), &job.row.key.id);
        if !stems.insert(stem.clone()) {
            return Err(format!("two jobs share the file name {stem}").into());
        }
        let mut row = job.row.clone();
        row.txt_name = Some(format!("{stem}.txt"));
        let name = write_job_txt(&staging, &row, &job.text)?;
        std::fs::rename(staging.join(TXT_DIR).join(&name), out.join(&name))?;
        let entry = counts.entry(job.row.key.portal.label()).or_default();
        if job.row.desc_status == DescStatus::Teaser {
            entry.1 += 1;
        } else {
            entry.0 += 1;
        }
        entries.push(gold_job(job, stem));
    }
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    Ok((entries, counts))
}

/// The labeling files of every profile and job, plus the rubric; returns the file count.
fn write_blind(out: &Path, jobs: &[GoldJob], profiles: &[PathBuf]) -> Res<usize> {
    let root = out.join(gold::LABELING_DIR);
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join(gold::RUBRIC_FILE), gold::RUBRIC)?;
    let mut written = 0;
    for path in profiles {
        let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let profile: Value =
            serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?;
        let profile = gold::strip_contact(&profile);
        let dir = root.join(gold::profile_stem(path));
        std::fs::create_dir_all(&dir)?;
        for job in jobs {
            let txt = gold::parse_txt(&std::fs::read_to_string(out.join(job.txt_name()))?);
            let portal = Portal::from_key(&job.portal).map_or(job.portal.as_str(), |p| p.label());
            let ad = AdView {
                title: &txt.title,
                company: &txt.company,
                location: &txt.location,
                portal,
                teaser: job.is_teaser(),
                facts: job.facts.as_ref(),
                body: &txt.body,
            };
            std::fs::write(
                dir.join(format!("{}.md", job.file)),
                gold::labeling_markdown(&profile, &ad),
            )?;
            written += 1;
        }
    }
    Ok(written)
}

fn main() -> Res<()> {
    let args = parse_args()?;
    let db = args
        .db
        .or_else(default_db)
        .ok_or("no app data folder known on this system; pass --db")?;
    let out = args.out.unwrap_or_else(gold::gold_dir);
    let conn = open_read_only(&db)?;
    let schema: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let (db_jobs, unreadable) = read_db(&conn)?;
    drop(conn);

    let exported: HashSet<String> = db_jobs.iter().map(|j| j.row.key.to_string()).collect();
    let (duplicates, jobs): (Vec<DbJob>, Vec<DbJob>) = db_jobs
        .into_iter()
        .partition(|j| j.dup_of.as_ref().is_some_and(|d| exported.contains(d)));

    ensure_ignored(&out)?;
    std::fs::create_dir_all(&out)?;
    let (mut entries, counts) = write_txts(&out, &jobs)?;

    // Keep jobs of an earlier export that the database no longer has.
    let jobs_path = out.join(gold::JOBS_FILE);
    let mut kept = 0;
    if jobs_path.exists() {
        let fresh: HashSet<String> = entries.iter().map(|e| e.key.clone()).collect();
        for old in gold::read_jobs(&jobs_path)? {
            if !fresh.contains(&old.key) && out.join(old.txt_name()).is_file() {
                entries.push(old);
                kept += 1;
            }
        }
    }
    std::fs::write(&jobs_path, gold::jobs_json(&entries))?;

    println!("database schema {schema}");
    for (portal, (ok, teaser)) in &counts {
        println!("{portal}: {ok} full texts, {teaser} teasers");
    }
    println!(
        "exported {} jobs, {} duplicates left out, {unreadable} unreadable rows, {kept} kept from an earlier export",
        jobs.len(),
        duplicates.len()
    );
    println!("{} jobs in {}", entries.len(), jobs_path.display());

    if args.blind {
        let profiles = if args.profiles.is_empty() {
            gold::default_profiles()?
        } else {
            args.profiles
        };
        let files = write_blind(&out, &entries, &profiles)?;
        println!(
            "blind labeling: {files} files for {} profiles in {}",
            profiles.len(),
            out.join(gold::LABELING_DIR).display()
        );
    }
    Ok(())
}
