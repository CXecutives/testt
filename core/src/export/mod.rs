//! Generated files in the workspace: `JobAlerts.xlsx` (overview of all jobs) and one text
//! file per job for the matching. Everything is generated from the database and written
//! atomically - an open Excel file or a crash never leaves half a file behind.

mod job_txt;
mod overview_html;
pub mod scale;
pub mod texts;
mod top_matches;
mod xlsx;

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::store::JobRow;
use crate::text::split_company_location;

pub use job_txt::{TXT_DIR, write_job_txt};
pub use overview_html::write_overview_html;
pub use texts::{COLUMNS, details_label};
pub use top_matches::{TOP_MATCHES_MAX, TOP_MATCHES_NAME, TopMatch, TopMatches, top_matches};
pub use xlsx::write_xlsx;

/// File and folder names below are a contract with the user's workspace and the matching
/// skill - do not translate.
pub const XLSX_NAME: &str = "JobAlerts.xlsx";
/// The HTML overview next to the Excel file.
pub const HTML_NAME: &str = "JobAlerts.html";
/// Overview of earlier versions; only kept so that "reset everything" takes it along.
const LEGACY_CSV_NAME: &str = "JobAlerts.csv";
/// Subfolder of the workspace for results (as before).
pub const RESULT_DIR: &str = "auswertung";
/// Temporary files of [`write_atomic`] - only left behind after a crash in the middle of
/// writing, and they belong to the app.
const TMP_PREFIX: &str = ".jam-";
const TMP_SUFFIX: &str = ".tmp";

/// One row of the overview as text. Date columns are missing here: Excel gets them as real
/// dates, not as text.
pub(crate) struct Line {
    pub source: &'static str,
    pub title: String,
    pub company: String,
    pub location: String,
    pub url: String,
    pub subject: String,
    pub gmail_url: String,
    pub details: &'static str,
    pub key: String,
}

impl Line {
    pub fn of(job: &JobRow) -> Line {
        let (company, location) = split_company_location(&job.company, &job.location);
        Line {
            source: job.key.portal.label(),
            title: job.title.clone(),
            company,
            location,
            url: job.url.to_string(),
            subject: job.mail_subject.clone(),
            gmail_url: job
                .gmail_id
                .and_then(crate::model::gmail_url)
                .map(|u| u.to_string())
                .unwrap_or_default(),
            details: details_label(job),
            key: job.key.to_string(),
        }
    }
}

/// Path of the overview file in the result folder.
pub fn overview_path(result_dir: &Path) -> PathBuf {
    result_dir.join(XLSX_NAME)
}

/// Path of the HTML overview in the result folder.
pub fn overview_html_path(result_dir: &Path) -> PathBuf {
    result_dir.join(HTML_NAME)
}

/// Writes `bytes` atomically to `path`: first into a temporary file in the same folder, then
/// rename. If the target is locked (e.g. open in Excel), it stays unchanged and the error is
/// `FileLocked`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let long = long_path(path);
    let path = long.as_path();
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    ensure_dir(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(TMP_PREFIX)
        .suffix(TMP_SUFFIX)
        .tempfile_in(dir)
        .map_err(|e| Error::io(dir, e))?;
    tmp.write_all(bytes).map_err(|e| Error::io(tmp.path(), e))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| Error::io(tmp.path(), e))?;
    tmp.persist(path)
        .map_err(|e| Error::replace(path, e.error))?;
    Ok(())
}

/// Creates a folder (with its parents, long paths too).
pub(crate) fn ensure_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(long_path(dir)).map_err(|e| Error::io(dir, e))
}

/// Windows paths from 260 characters on need the long form `\\?\`: `tempfile` passes the
/// path unchanged to Windows when renaming - a long workspace would otherwise make exactly
/// the text files with long titles fail.
fn long_path(path: &Path) -> PathBuf {
    let Ok(absolute) = std::path::absolute(path) else {
        return path.to_path_buf();
    };
    match absolute.to_str() {
        Some(p) if cfg!(windows) && p.len() >= 240 && !p.starts_with(r"\\?\") => {
            match p.strip_prefix(r"\\") {
                Some(unc) => format!(r"\\?\UNC\{unc}").into(),
                None => format!(r"\\?\{p}").into(),
            }
        }
        _ => absolute,
    }
}

/// The app's files in the result folder that exist right now: the overviews, the known text
/// files (plain file names only), leftover temporary files and remains of an earlier reset.
/// The list for resetting - foreign files (script and reports of the matching skill,
/// consultant profiles ...) are never included. One directory listing per folder instead of
/// one query per file (network drive, thousands of text files).
pub fn app_files(result_dir: &Path, txt_names: &[String]) -> Vec<PathBuf> {
    let mut files = files_in(result_dir, |name| {
        name.eq_ignore_ascii_case(XLSX_NAME)
            || name.eq_ignore_ascii_case(HTML_NAME)
            || name.eq_ignore_ascii_case(TOP_MATCHES_NAME)
            || name.eq_ignore_ascii_case(LEGACY_CSV_NAME)
            || is_tmp(name)
    });
    files.extend(txt_files(result_dir, txt_names));
    files
}

/// Only the app's text files in the subfolder `beschreibungen_txt` - the overview stays out.
/// One list for "delete text files" and the display of their number.
pub fn txt_files(result_dir: &Path, txt_names: &[String]) -> Vec<PathBuf> {
    let known: HashSet<&str> = txt_names
        .iter()
        .map(String::as_str)
        .filter(|n| is_plain_file_name(n))
        .collect();
    files_in(&result_dir.join(TXT_DIR), |name| {
        known.contains(name) || is_tmp(name)
    })
}

fn files_in(dir: &Path, ours: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            e.file_name().to_str().is_some_and(|name| {
                ours(
                    name.split_once(crate::reset::LEFTOVER)
                        .map_or(name, |(base, _)| base),
                )
            })
        })
        .map(|e| e.path())
        .collect()
}

fn is_tmp(name: &str) -> bool {
    name.starts_with(TMP_PREFIX) && name.ends_with(TMP_SUFFIX)
}

/// "Delete text files": removes **only** the app's text files ([`txt_files`]) - the Excel
/// overview and foreign files stay.
///
/// Returns the number of deleted files and the names of the files that could not be
/// deleted (e.g. open right now).
pub fn clear_txt_files(result_dir: &Path, txt_names: &[String]) -> (usize, Vec<String>) {
    let mut removed = 0;
    let mut failed = Vec::new();
    for path in txt_files(result_dir, txt_names) {
        match std::fs::remove_file(long_path(&path)) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => failed.push(
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            ),
        }
    }
    // The subfolder only disappears if that made it empty.
    let _ = std::fs::remove_dir(result_dir.join(TXT_DIR));
    (removed, failed)
}

/// Only a file name, no path - protects "clear" against manipulated entries.
pub(crate) fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && name != "."
        && name != ".."
        && Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("x.txt");
        write_atomic(&path, b"eins").unwrap();
        write_atomic(&path, b"zwei").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "zwei");
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(leftovers.len(), 1);
    }

    /// Read-only is not "open in Excel".
    #[cfg(windows)] // On Unix a missing write bit does not prevent replacing.
    #[test]
    fn read_only_target_is_no_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(XLSX_NAME);
        std::fs::write(&path, b"alt").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms.clone()).unwrap();
        let err = write_atomic(&path, b"neu").unwrap_err();
        assert!(matches!(err, Error::Io { .. }), "{err:?}");
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "test: lift the write protection again"
        )]
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    /// Paths above 260 characters (deeply nested workspace).
    #[test]
    fn long_paths_are_written() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a".repeat(120)).join("b".repeat(120));
        let path = deep.join(format!("{}.txt", "c".repeat(100)));
        write_atomic(&path, b"eins").unwrap();
        write_atomic(&path, b"zwei").unwrap();
        assert_eq!(std::fs::read(long_path(&path)).unwrap(), b"zwei");
    }

    /// A file open in Excel (Windows: without delete sharing) stays unchanged, the error
    /// says "locked".
    #[cfg(windows)]
    #[test]
    fn locked_target_is_reported_and_kept() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(XLSX_NAME);
        std::fs::write(&path, b"alt").unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let err = write_atomic(&path, b"neu").unwrap_err();
        assert!(matches!(err, Error::FileLocked(_)), "{err:?}");
        drop(lock);
        assert_eq!(std::fs::read(&path).unwrap(), b"alt");
        let files = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(files, 1, "no temporary leftovers");
    }

    #[test]
    fn clearing_removes_only_the_app_text_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let txt = root.join(TXT_DIR);
        std::fs::create_dir_all(&txt).unwrap();
        std::fs::create_dir_all(root.join("beschreibungen_matching")).unwrap();
        for (path, body) in [
            (root.join(XLSX_NAME), "x"),
            (
                root.join("beschreibungen_matching")
                    .join("20260919_1200_matching.html"),
                "fremd",
            ),
            (txt.join("20260918_LinkedIn_A_4000000001.txt"), "app"),
            (txt.join("notiz.txt"), "fremd"),
            // Remains of an interrupted write.
            (txt.join(".jam-ab12cd.tmp"), "halb"),
            (root.join("fremd.tmp"), "fremd"),
        ] {
            std::fs::write(path, body).unwrap();
        }
        let names = vec![
            "20260918_LinkedIn_A_4000000001.txt".to_string(),
            "..\\..\\evil.txt".to_string(),
            "missing.txt".to_string(),
        ];
        // Counted is exactly what gets deleted.
        assert_eq!(app_files(root, &names).len(), 3);
        assert_eq!(txt_files(root, &names).len(), 2);
        let (removed, failed) = clear_txt_files(root, &names);
        assert_eq!(removed, 2);
        assert!(failed.is_empty());
        assert!(
            root.join(XLSX_NAME).exists(),
            "the overview stays: it is no text file"
        );
        assert!(!txt.join("20260918_LinkedIn_A_4000000001.txt").exists());
        assert!(!txt.join(".jam-ab12cd.tmp").exists());
        assert!(txt.join("notiz.txt").exists(), "a foreign file stays");
        assert!(root.join("fremd.tmp").exists());
        assert!(
            root.join("beschreibungen_matching")
                .join("20260919_1200_matching.html")
                .exists()
        );
    }
}
