//! „Alles zurücksetzen“ über einen Neustart: Die App legt einen Auftrag ab und startet neu;
//! der neue Prozess löscht **vor** dem Öffnen von Datenbank und Fenster. So hält nichts mehr
//! eine Datei offen – Prozesse werden nie beendet.
//!
//! Gelöscht werden nur Dinge der App: Datenbank (Jobs, Einstellungen, Scan-Stand), das
//! Profil des Sitzungsfensters (freelance.de-Anmeldung), der Gmail-Zugang im Tresor und im
//! Arbeitsordner die App-Dateien samt `profil/beraterprofil.json`. `policy.json` bleibt –
//! eine Sperrpause darf sich nicht wegklicken lassen. Fremde Dateien bleiben unberührt.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::export::{RESULT_DIR, app_files};
use crate::fetch::policy::Policy;
use crate::portal::Portal;
use crate::profile::{PROFILE_DIR, PROFILE_FILE, profile_path};
use crate::secrets::Vault;
use crate::{DB_FILE, POLICY_FILE, SESSION_DIR};

const MARKER: &str = "reset.pending";
/// Namensteil umbenannter Reste, die sich (noch) nicht löschen ließen.
pub(crate) const LEFTOVER: &str = ".delete-";
const ATTEMPTS: usize = 10;
const PAUSE: Duration = Duration::from_millis(300);

/// Was beim nächsten Start gelöscht wird – nur, was sich nicht aus dem Datenordner ergibt
/// (ein alter oder veränderter Auftrag kann so nie einen beliebigen Ordner treffen).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPlan {
    pub workspace: PathBuf,
    /// Namen der geschriebenen Textdateien (aus der Datenbank, bevor sie weg ist).
    pub txt_names: Vec<String>,
}

/// Ergebnis nach dem Neustart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetReport {
    pub removed: usize,
    /// Was sich nicht löschen ließ (Anzeige: „Zurücksetzen unvollständig“).
    pub failed: Vec<String>,
}

/// Legt den Auftrag ab; danach startet die App neu.
pub fn request(data_dir: &Path, plan: &ResetPlan) -> crate::Result<()> {
    let json = serde_json::to_vec_pretty(plan).expect("serialisierbar");
    crate::export::write_atomic(&data_dir.join(MARKER), &json)
}

/// Beim Start aufrufen, bevor die Datenbank geöffnet wird. `None`: kein Zurücksetzen offen.
///
/// Der Auftrag wird zuerst verbraucht und läuft so genau einmal: Was sich nicht löschen
/// ließ, steht im Bericht – ein zweiter Durchgang bei einem späteren Start löschte
/// inzwischen neu Angelegtes (Datenbank, Anmeldung, Profil, Gmail-Zugang) ungefragt.
pub fn perform_pending(data_dir: &Path, vault: &Vault) -> Option<ResetReport> {
    let marker = data_dir.join(MARKER);
    let bytes = match std::fs::read(&marker) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            log::warn!("Zurücksetzen: Auftrag nicht lesbar ({e})");
            None
        }
    };
    let mut report = ResetReport::default();
    if let Err(e) = std::fs::remove_file(&marker) {
        log::warn!("Zurücksetzen: Auftrag ließ sich nicht entfernen ({e}) – nicht ausgeführt");
        report.failed.push(format!(
            "Zurücksetzen nicht ausgeführt – der Auftrag ließ sich nicht entfernen ({e})"
        ));
        return Some(report);
    }
    let Some(plan) = bytes.and_then(|b| serde_json::from_slice::<ResetPlan>(&b).ok()) else {
        log::warn!("Zurücksetzen: Auftrag unlesbar – verworfen");
        report.failed.push("Auftrag unlesbar".into());
        return Some(report);
    };

    let db = data_dir.join(DB_FILE);
    let journal = journal(&db);
    let profile = profile_path(&plan.workspace);
    let profile_dir = plan.workspace.join(PROFILE_DIR);
    let result_dir = plan.workspace.join(RESULT_DIR);
    let mut targets = vec![db, journal, data_dir.join(SESSION_DIR), profile];
    // Reste früherer Versuche mitnehmen – ein umbenanntes Sitzungsprofil samt Anmelde-Cookie
    // bliebe sonst für immer liegen.
    let journal_name = format!("{DB_FILE}-journal");
    let names_in_data = [DB_FILE, journal_name.as_str(), SESSION_DIR];
    targets.extend(leftovers(data_dir, |base| names_in_data.contains(&base)));
    targets.extend(leftovers(&profile_dir, |base| base == PROFILE_FILE));
    // Übersichten, Textdateien, temporäre Dateien und deren Reste im Ergebnisordner.
    targets.extend(app_files(&result_dir, &plan.txt_names));
    for target in targets {
        match remove(&target) {
            Ok(true) => report.removed += 1,
            Ok(false) => {}
            Err((rest, e)) => {
                log::warn!("Zurücksetzen: {} nicht gelöscht: {e}", rest.display());
                report.failed.push(rest.display().to_string());
            }
        }
    }
    // Leer gewordene App-Ordner mit aufräumen (nie mit Inhalt).
    for dir in [
        result_dir.join(crate::export::TXT_DIR),
        result_dir,
        profile_dir,
    ] {
        let _ = std::fs::remove_dir(dir);
    }
    if let Err(e) = vault.delete_gmail() {
        report.failed.push(format!("Gmail-Zugang im Tresor ({e})"));
    }
    // Die freelance.de-Anmeldung ist mit dem Profil weg – Pausen und Zähler bleiben.
    let mut policy = Policy::load(&data_dir.join(POLICY_FILE), jiff::Timestamp::now());
    policy.forget_session(Portal::FreelanceDe);
    if let Err(e) = policy.save() {
        report
            .failed
            .push(format!("Anmeldestand in {POLICY_FILE} ({e})"));
    }
    Some(report)
}

fn journal(database: &Path) -> PathBuf {
    let mut name = database.as_os_str().to_owned();
    name.push("-journal");
    PathBuf::from(name)
}

/// Umbenannte Reste früherer Versuche (`<Name>.delete-<Zeit>`) in `dir`, deren Name zu
/// `ours` passt.
fn leftovers(dir: &Path, ours: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .and_then(|name| name.split_once(LEFTOVER))
                .is_some_and(|(base, _)| ours(base))
        })
        .map(|e| e.path())
        .collect()
}

/// Erst umbenennen (dann ist der Name sofort frei), dann löschen – mit bis zu zehn Versuchen,
/// falls ein gerade beendeter WebView2-Prozess die Datei noch kurz hält. Scheitert es, nennt
/// der Fehler den Rest unter seinem tatsächlichen (neuen) Namen.
fn remove(path: &Path) -> Result<bool, (PathBuf, std::io::Error)> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok(false);
    }
    let already_renamed = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains(LEFTOVER));
    let mut target = path.to_path_buf();
    let mut last = None;
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(PAUSE);
        }
        if target == path && !already_renamed {
            let doomed = doomed_name(path);
            if std::fs::rename(path, &doomed).is_ok() {
                target = doomed;
            }
        }
        let removed = if target.is_dir() {
            std::fs::remove_dir_all(&target)
        } else {
            std::fs::remove_file(&target)
        };
        match removed {
            Ok(()) => return Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(e) => last = Some(e),
        }
    }
    Err((
        target,
        last.unwrap_or_else(|| std::io::Error::other("unbekannt")),
    ))
}

fn doomed_name(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(
        "{LEFTOVER}{}",
        jiff::Timestamp::now().as_millisecond()
    ));
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{TXT_DIR, XLSX_NAME};

    fn setup(root: &Path) -> (PathBuf, ResetPlan) {
        let data = root.join("data");
        let workspace = root.join("ws");
        let txt = workspace.join(RESULT_DIR).join(TXT_DIR);
        std::fs::create_dir_all(&txt).unwrap();
        std::fs::create_dir_all(data.join("session-freelance/Default")).unwrap();
        std::fs::write(data.join("jobs.db"), b"db").unwrap();
        std::fs::write(data.join("policy.json"), b"{}").unwrap();
        std::fs::write(data.join("session-freelance/Default/Cookies"), b"c").unwrap();
        std::fs::write(workspace.join(RESULT_DIR).join(XLSX_NAME), b"x").unwrap();
        std::fs::write(txt.join("20260919_LinkedIn_A_1.txt"), b"t").unwrap();
        std::fs::write(txt.join("fremd.txt"), b"f").unwrap();
        std::fs::write(txt.join(".jam-x1y2z3.tmp"), b"halb").unwrap();
        std::fs::create_dir_all(workspace.join("profil")).unwrap();
        std::fs::write(profile_path(&workspace), b"{}").unwrap();
        std::fs::write(workspace.join("Profil_Erika.json"), b"{}").unwrap();
        let plan = ResetPlan {
            workspace,
            txt_names: vec![
                "20260919_LinkedIn_A_1.txt".into(),
                r"..\..\data\policy.json".into(),
            ],
        };
        (data, plan)
    }

    #[test]
    fn removes_only_app_data_and_keeps_policy_and_foreign_files() {
        let root = tempfile::tempdir().unwrap();
        let (data, plan) = setup(root.path());
        request(&data, &plan).unwrap();
        let report = perform_pending(&data, &Vault::for_tests("reset")).unwrap();
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(!data.join("jobs.db").exists());
        assert!(!data.join("session-freelance").exists());
        assert!(!profile_path(&plan.workspace).exists());
        let txt = plan.workspace.join(RESULT_DIR).join(TXT_DIR);
        assert!(
            !txt.join(".jam-x1y2z3.tmp").exists(),
            "Rest eines abgebrochenen Schreibens"
        );
        // Bleibt: Sicherheitsstand, fremde Dateien (Skill, eigenes Profil), manipulierter Name.
        assert!(data.join("policy.json").exists());
        assert!(txt.join("fremd.txt").exists());
        assert!(plan.workspace.join("Profil_Erika.json").exists());
        assert!(!data.join(MARKER).exists());
        assert!(
            perform_pending(&data, &Vault::for_tests("reset")).is_none(),
            "nur einmal"
        );
    }

    /// Eine gesperrte Datei: Der Rest ist gelöscht, der Bericht nennt sie – und der Auftrag
    /// läuft nie ein zweites Mal (sonst löschte der nächste Start inzwischen neu Angelegtes).
    #[cfg(windows)]
    #[test]
    fn locked_file_is_reported_and_the_plan_runs_once() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let (data, plan) = setup(root.path());
        request(&data, &plan).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(plan.workspace.join(RESULT_DIR).join(XLSX_NAME))
            .unwrap();
        let report = perform_pending(&data, &Vault::for_tests("reset")).unwrap();
        drop(lock);
        assert_eq!(report.failed.len(), 1, "{report:?}");
        assert!(!data.join(MARKER).exists());
        assert!(!data.join("jobs.db").exists());
        // Neu Angelegtes nach dem Zurücksetzen bleibt beim nächsten Start unangetastet.
        std::fs::write(data.join("jobs.db"), b"neu").unwrap();
        assert!(perform_pending(&data, &Vault::for_tests("reset")).is_none());
        assert!(data.join("jobs.db").exists());
    }

    /// Reste früherer Versuche (umbenannt, aber nicht gelöscht) nimmt ein späteres
    /// Zurücksetzen mit – fremde Dateien mit ähnlichem Namen nicht.
    #[test]
    fn leftovers_of_earlier_attempts_are_swept() {
        let root = tempfile::tempdir().unwrap();
        let (data, plan) = setup(root.path());
        let txt = plan.workspace.join(RESULT_DIR).join(TXT_DIR);
        let rests = [
            data.join("jobs.db.delete-111"),
            data.join("session-freelance.delete-222"),
            plan.workspace
                .join(RESULT_DIR)
                .join("JobAlerts.xlsx.delete-333"),
            txt.join("20260919_LinkedIn_A_1.txt.delete-444"),
            plan.workspace
                .join("profil")
                .join("beraterprofil.json.delete-555"),
        ];
        for rest in &rests[..4] {
            std::fs::write(rest, b"rest").unwrap();
        }
        std::fs::remove_file(&rests[1]).unwrap();
        std::fs::create_dir_all(rests[1].join("Default")).unwrap();
        std::fs::write(rests[1].join("Default").join("Cookies"), b"c").unwrap();
        std::fs::write(&rests[4], b"rest").unwrap();
        std::fs::write(txt.join("fremd.txt.delete-1"), b"fremd").unwrap();

        request(&data, &plan).unwrap();
        let report = perform_pending(&data, &Vault::for_tests("reset")).unwrap();
        assert!(report.failed.is_empty(), "{report:?}");
        for rest in &rests {
            assert!(!rest.exists(), "{rest:?}");
        }
        assert!(txt.join("fremd.txt.delete-1").exists());
    }

    /// Scheitert das Löschen, nennt der Fehler den Rest unter seinem tatsächlichen Namen.
    #[cfg(windows)]
    #[test]
    fn a_locked_target_is_reported_by_its_actual_name() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("jobs.db");
        std::fs::write(&file, b"db").unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        let (rest, _) = remove(&file).unwrap_err();
        drop(lock);
        assert!(rest.exists(), "{rest:?}");
        assert!(remove(&rest).unwrap());
    }

    #[test]
    fn an_unreadable_plan_is_discarded_and_reported() {
        let root = tempfile::tempdir().unwrap();
        let (data, _) = setup(root.path());
        std::fs::write(data.join(MARKER), b"{ kaputt").unwrap();
        let report = perform_pending(&data, &Vault::for_tests("reset")).unwrap();
        assert_eq!(report.failed, ["Auftrag unlesbar"]);
        assert!(!data.join(MARKER).exists());
        assert!(data.join("jobs.db").exists(), "nichts gelöscht");
    }

    #[test]
    fn the_freelance_session_is_forgotten_but_pauses_stay() {
        let root = tempfile::tempdir().unwrap();
        let (data, plan) = setup(root.path());
        let now = jiff::Timestamp::now();
        let mut policy = Policy::load(&data.join(POLICY_FILE), now);
        policy.set_session(Portal::FreelanceDe, true, now);
        let until = policy.pause(
            Portal::FreelanceDe,
            crate::fetch::policy::PauseKind::Blocked,
            "Test",
            now,
        );
        policy.save().unwrap();
        request(&data, &plan).unwrap();
        perform_pending(&data, &Vault::for_tests("reset")).unwrap();
        let state = Policy::load(&data.join(POLICY_FILE), now).state(Portal::FreelanceDe);
        assert!(state.login_needed);
        assert_eq!(state.session_confirmed_at, None);
        assert_eq!(state.paused_until, Some(until), "Pause bleibt");
    }
}
