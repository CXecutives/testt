//! Erzeugte Dateien im Arbeitsordner: `JobAlerts.xlsx` (Übersicht aller Jobs) und je Job
//! eine Textdatei für das Matching. Alles wird aus der Datenbank erzeugt und atomar
//! geschrieben – eine offene Excel-Datei oder ein Absturz hinterlässt nie eine halbe Datei.

mod job_txt;
mod xlsx;

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::model::DescStatus;
use crate::store::JobRow;
use crate::text::split_company_location;

pub use job_txt::{TXT_DIR, write_job_txt};
pub use xlsx::write_xlsx;

pub const XLSX_NAME: &str = "JobAlerts.xlsx";
/// Übersicht früherer Versionen; nur noch, damit „Alles zurücksetzen“ sie mitnimmt.
const LEGACY_CSV_NAME: &str = "JobAlerts.csv";
/// Unterordner des Arbeitsordners für Ergebnisse (wie bisher).
pub const RESULT_DIR: &str = "auswertung";
/// Temporäre Dateien von [`write_atomic`] – bleiben nur nach einem Abbruch mitten im
/// Schreiben liegen und gehören der App.
const TMP_PREFIX: &str = ".jam-";
const TMP_SUFFIX: &str = ".tmp";

/// Spaltenköpfe der Übersicht (Reihenfolge wie bisher, ergänzt um den Jobdetails-Stand).
/// Anders als die Textdateien liest die Übersicht niemand maschinell – sie heißt deshalb
/// „Portal“ wie die Oberfläche, nicht „Quelle“ wie der Skill-Vertrag.
pub const COLUMNS: [&str; 11] = [
    "Portal",
    "Mail-Datum",
    "Titel",
    "Unternehmen",
    "Ort",
    "Link",
    "Mail-Betreff",
    "Mail in Gmail",
    "Gespeichert am",
    "Jobdetails",
    "Schlüssel",
];

/// Eine Zeile der Übersicht als Text. Datumsspalten fehlen hier: Excel bekommt sie als
/// echtes Datum, nicht als Text.
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

/// Stand der Jobdetails in Worten.
pub fn details_label(job: &JobRow) -> &'static str {
    match job.desc_status {
        DescStatus::Ok if job.desc_closed => "vorhanden (Anzeige geschlossen)",
        DescStatus::Ok if job.desc_short => "vorhanden (kurz)",
        DescStatus::Ok => "vorhanden",
        DescStatus::Missing => "fehlt",
        DescStatus::Failed => "fehlgeschlagen – neuer Versuch folgt",
        DescStatus::Gone => "Anzeige nicht mehr abrufbar",
        DescStatus::Unfetchable => "nicht abrufbar",
    }
}

/// Pfad der Übersichtsdatei im Ergebnisordner.
pub fn overview_path(result_dir: &Path) -> PathBuf {
    result_dir.join(XLSX_NAME)
}

/// Schreibt `bytes` atomar nach `path`: erst in eine temporäre Datei im selben Ordner,
/// dann Umbenennen. Ist das Ziel gesperrt (z. B. in Excel geöffnet), bleibt es
/// unverändert und der Fehler lautet `FileLocked`.
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

/// Legt einen Ordner an (samt Eltern, auch mit langem Pfad).
pub(crate) fn ensure_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(long_path(dir)).map_err(|e| Error::io(dir, e))
}

/// Windows-Pfade ab 260 Zeichen brauchen die Langform `\\?\`: `tempfile` reicht den Pfad
/// beim Umbenennen unverändert an Windows weiter – ein langer Arbeitsordner ließe sonst
/// genau die Textdateien mit langen Titeln scheitern.
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

/// Die Dateien der App im Ergebnisordner, die es gerade gibt: die Übersichten, die
/// bekannten Textdateien (nur reine Dateinamen), liegen gebliebene temporäre Dateien und
/// Reste eines früheren Zurücksetzens. Die Liste fürs Zurücksetzen – fremde Dateien (Skript und Berichte des Matching-Skills, Beraterprofile …)
/// sind nie dabei. Je Ordner eine Verzeichnisabfrage statt einer je Datei (Netzlaufwerk,
/// tausende Textdateien).
pub fn app_files(result_dir: &Path, txt_names: &[String]) -> Vec<PathBuf> {
    let mut files = files_in(result_dir, |name| {
        name.eq_ignore_ascii_case(XLSX_NAME)
            || name.eq_ignore_ascii_case(LEGACY_CSV_NAME)
            || is_tmp(name)
    });
    files.extend(txt_files(result_dir, txt_names));
    files
}

/// Nur die Textdateien der App im Unterordner `beschreibungen_txt` – die Übersicht bleibt
/// außen vor. Eine Liste für „Textdateien löschen“ und die Anzeige ihrer Zahl.
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

/// „Textdateien löschen“: entfernt **nur** die Textdateien der App ([`txt_files`]) – die
/// Excel-Übersicht und fremde Dateien bleiben.
///
/// Liefert die Zahl gelöschter Dateien und die Namen der Dateien, die sich nicht löschen
/// ließen (z. B. gerade geöffnet).
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
    // Der Unterordner verschwindet nur, wenn er dadurch leer geworden ist.
    let _ = std::fs::remove_dir(result_dir.join(TXT_DIR));
    (removed, failed)
}

/// Nur ein Dateiname, kein Pfad – schützt „leeren“ vor manipulierten Einträgen.
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

    /// Schreibgeschützt ist nicht „in Excel geöffnet“.
    #[cfg(windows)] // Unter Unix verhindert ein fehlendes Schreibbit das Ersetzen nicht.
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
            reason = "Test: Schreibschutz wieder aufheben"
        )]
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    /// Pfade über 260 Zeichen (tief verschachtelter Arbeitsordner).
    #[test]
    fn long_paths_are_written() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a".repeat(120)).join("b".repeat(120));
        let path = deep.join(format!("{}.txt", "c".repeat(100)));
        write_atomic(&path, b"eins").unwrap();
        write_atomic(&path, b"zwei").unwrap();
        assert_eq!(std::fs::read(long_path(&path)).unwrap(), b"zwei");
    }

    /// Eine in Excel geöffnete Datei (Windows: ohne Freigabe zum Löschen) bleibt
    /// unverändert, der Fehler heißt „gesperrt“.
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
        assert_eq!(files, 1, "keine temporären Reste");
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
            // Rest eines abgebrochenen Schreibens.
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
        // Gezählt wird genau, was gelöscht wird.
        assert_eq!(app_files(root, &names).len(), 3);
        assert_eq!(txt_files(root, &names).len(), 2);
        let (removed, failed) = clear_txt_files(root, &names);
        assert_eq!(removed, 2);
        assert!(failed.is_empty());
        assert!(
            root.join(XLSX_NAME).exists(),
            "die Übersicht bleibt: sie ist keine Textdatei"
        );
        assert!(!txt.join("20260918_LinkedIn_A_4000000001.txt").exists());
        assert!(!txt.join(".jam-ab12cd.tmp").exists());
        assert!(txt.join("notiz.txt").exists(), "fremde Datei bleibt");
        assert!(root.join("fremd.tmp").exists());
        assert!(
            root.join("beschreibungen_matching")
                .join("20260919_1200_matching.html")
                .exists()
        );
    }
}
