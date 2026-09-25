//! "Reset everything" through a restart: the app leaves an order and restarts; the new
//! process deletes **before** it opens database and window. So nothing holds a file open
//! any more - processes are never killed.
//!
//! Only things of the app are deleted: the database (jobs, settings, scan state) with its
//! journal, WAL and shared-memory files and its copies (`backups/`), the profiles of the
//! session windows (freelance.de
//! sign-in; on macOS the app removes their `WKWebView` data stores right after the start,
//! which needs the running app), the Gmail access in the keychain and, in the workspace, the
//! app's files including `profil/beraterprofil.json`. `policy.json` stays - a block pause
//! must not be clickable away. Foreign files stay untouched.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::export::{RESULT_DIR, app_files, files_in, is_tmp};
use crate::fetch::policy::Policy;
use crate::portal::Portal;
use crate::profile::{BACKUP_FILE, PROFILE_DIR, PROFILE_FILE, profile_path};
use crate::secrets::Vault;
use crate::store::BACKUP_DIR;
use crate::{DB_FILE, POLICY_FILE, session_dir};

const MARKER: &str = "reset.pending";
/// Name part of renamed leftovers that could not be deleted (yet).
pub(crate) const LEFTOVER: &str = ".delete-";
/// Files `SQLite` keeps next to the database (rollback journal, write-ahead log, shared
/// memory of the log).
const SIDECARS: [&str; 3] = ["-journal", "-wal", "-shm"];
const ATTEMPTS: usize = 10;
const PAUSE: Duration = Duration::from_millis(300);

/// What is deleted at the next start - only what does not follow from the data folder (an
/// old or altered order can thus never hit an arbitrary folder).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPlan {
    pub workspace: PathBuf,
    /// Names of the written text files (from the database, before it is gone).
    pub txt_names: Vec<String>,
}

/// Result after the restart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetReport {
    pub removed: usize,
    /// What could not be deleted (paths and short English notes, for the log).
    pub failed: Vec<String>,
}

/// Leaves the order; afterwards the app restarts.
pub fn request(data_dir: &Path, plan: &ResetPlan) -> crate::Result<()> {
    let json = serde_json::to_vec_pretty(plan).expect("serialisable");
    crate::export::write_atomic(&data_dir.join(MARKER), &json)
}

/// Call at the start, before the database is opened. `None`: no reset pending.
///
/// The order is used up first and so runs exactly once: what could not be deleted is in the
/// report - a second pass at a later start would delete things created in the meantime
/// (database, sign-in, profile, Gmail access) without asking.
pub fn perform_pending(data_dir: &Path, vault: &Vault) -> Option<ResetReport> {
    let marker = data_dir.join(MARKER);
    let bytes = match std::fs::read(&marker) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            log::warn!("reset: order not readable ({e})");
            None
        }
    };
    let mut report = ResetReport::default();
    if let Err(e) = std::fs::remove_file(&marker) {
        log::warn!("reset: order could not be removed ({e}), not performed");
        report
            .failed
            .push(format!("reset not performed: order not removable ({e})"));
        return Some(report);
    }
    let Some(plan) = bytes.and_then(|b| serde_json::from_slice::<ResetPlan>(&b).ok()) else {
        log::warn!("reset: order unreadable, discarded");
        report.failed.push("order unreadable".into());
        return Some(report);
    };

    let db = data_dir.join(DB_FILE);
    let profile = profile_path(&plan.workspace);
    let profile_dir = plan.workspace.join(PROFILE_DIR);
    let result_dir = plan.workspace.join(RESULT_DIR);
    let sessions: Vec<String> = Portal::ALL.into_iter().map(session_dir).collect();
    let sidecars: Vec<String> = SIDECARS.iter().map(|s| format!("{DB_FILE}{s}")).collect();
    let mut targets = vec![db, data_dir.join(BACKUP_DIR), profile];
    targets.extend(sidecars.iter().map(|name| data_dir.join(name)));
    targets.extend(sessions.iter().map(|dir| data_dir.join(dir)));
    // Take along the leftovers of earlier attempts - a renamed session profile with its
    // sign-in cookie would otherwise stay forever.
    let names_in_data = |base: &str| {
        base == DB_FILE
            || base == BACKUP_DIR
            || sidecars.iter().any(|name| name == base)
            || sessions.iter().any(|dir| dir == base)
    };
    targets.extend(leftovers(data_dir, names_in_data));
    // The profile, its backup and remains of an interrupted write - the old profile may carry
    // name and contact data. Foreign files in the folder stay.
    targets.extend(files_in(&profile_dir, |name| {
        name == PROFILE_FILE || name == BACKUP_FILE || is_tmp(name)
    }));
    // Overviews, text files, temporary files and their leftovers in the result folder.
    targets.extend(app_files(&result_dir, &plan.txt_names));
    for target in targets {
        match remove(&target) {
            Ok(true) => report.removed += 1,
            Ok(false) => {}
            Err((rest, e)) => {
                log::warn!("reset: {} not deleted: {e}", rest.display());
                report.failed.push(rest.display().to_string());
            }
        }
    }
    // Clean up app folders that became empty (never with content).
    for dir in [
        result_dir.join(crate::export::TXT_DIR),
        result_dir,
        profile_dir,
    ] {
        let _ = std::fs::remove_dir(dir);
    }
    if let Err(e) = vault.delete_gmail() {
        report
            .failed
            .push(format!("Gmail access in the keychain ({e})"));
    }
    // Every portal sign-in is gone with its profile - pauses and counters stay.
    let mut policy = Policy::load(&data_dir.join(POLICY_FILE), jiff::Timestamp::now());
    for portal in Portal::ALL {
        policy.forget_session(portal);
    }
    if let Err(e) = policy.save() {
        report
            .failed
            .push(format!("sign-in state in {POLICY_FILE} ({e})"));
    }
    Some(report)
}

/// Renamed leftovers of earlier attempts (`<name>.delete-<time>`) in `dir` whose name fits
/// `ours`.
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

/// First rename (then the name is free at once), then delete - with up to ten attempts in
/// case a WebView2 process that just ended still holds the file for a moment. If it fails,
/// the error names the leftover by its actual (new) name.
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
        last.unwrap_or_else(|| std::io::Error::other("unknown")),
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
        std::fs::write(data.join("jobs.db-wal"), b"wal").unwrap();
        std::fs::write(data.join("jobs.db-shm"), b"shm").unwrap();
        std::fs::write(data.join("policy.json"), b"{}").unwrap();
        std::fs::write(data.join("session-freelance/Default/Cookies"), b"c").unwrap();
        std::fs::create_dir_all(data.join("backups")).unwrap();
        std::fs::write(data.join("backups/jobs-2026-09-19.db"), b"copy").unwrap();
        std::fs::write(workspace.join(RESULT_DIR).join(XLSX_NAME), b"x").unwrap();
        std::fs::write(txt.join("20260919_LinkedIn_A_1.txt"), b"t").unwrap();
        std::fs::write(txt.join("fremd.txt"), b"f").unwrap();
        std::fs::write(txt.join(".jam-x1y2z3.tmp"), b"halb").unwrap();
        std::fs::create_dir_all(workspace.join("profil")).unwrap();
        std::fs::write(profile_path(&workspace), b"{}").unwrap();
        std::fs::write(crate::profile::backup_path(&workspace), b"{}").unwrap();
        std::fs::write(workspace.join("profil").join(".jam-p1q2r3.tmp"), b"halb").unwrap();
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
        assert!(!data.join("jobs.db-wal").exists() && !data.join("jobs.db-shm").exists());
        assert!(
            !data.join("backups").exists(),
            "the copies go with the database"
        );
        assert!(!data.join("session-freelance").exists());
        assert!(!profile_path(&plan.workspace).exists());
        assert!(
            !plan.workspace.join("profil").exists(),
            "profile, its backup and a half-written file go, and the folder with them"
        );
        let txt = plan.workspace.join(RESULT_DIR).join(TXT_DIR);
        assert!(
            !txt.join(".jam-x1y2z3.tmp").exists(),
            "remains of an interrupted write"
        );
        // Stays: safety state, foreign files (skill, own profile), manipulated name.
        assert!(data.join("policy.json").exists());
        assert!(txt.join("fremd.txt").exists());
        assert!(plan.workspace.join("Profil_Erika.json").exists());
        assert!(!data.join(MARKER).exists());
        assert!(
            perform_pending(&data, &Vault::for_tests("reset")).is_none(),
            "only once"
        );
    }

    /// A locked file: the rest is deleted, the report names it - and the order never runs a
    /// second time (otherwise the next start would delete things created in the meantime).
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
        // What is created after the reset stays untouched at the next start.
        std::fs::write(data.join("jobs.db"), b"neu").unwrap();
        assert!(perform_pending(&data, &Vault::for_tests("reset")).is_none());
        assert!(data.join("jobs.db").exists());
    }

    /// Leftovers of earlier attempts (renamed but not deleted) are taken along by a later
    /// reset - foreign files with a similar name are not.
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
            data.join("jobs.db-wal.delete-666"),
        ];
        for rest in &rests[..4] {
            std::fs::write(rest, b"rest").unwrap();
        }
        std::fs::remove_file(&rests[1]).unwrap();
        std::fs::create_dir_all(rests[1].join("Default")).unwrap();
        std::fs::write(rests[1].join("Default").join("Cookies"), b"c").unwrap();
        std::fs::write(&rests[4], b"rest").unwrap();
        std::fs::write(&rests[5], b"rest").unwrap();
        std::fs::write(txt.join("fremd.txt.delete-1"), b"fremd").unwrap();

        request(&data, &plan).unwrap();
        let report = perform_pending(&data, &Vault::for_tests("reset")).unwrap();
        assert!(report.failed.is_empty(), "{report:?}");
        for rest in &rests {
            assert!(!rest.exists(), "{rest:?}");
        }
        assert!(txt.join("fremd.txt.delete-1").exists());
    }

    /// If deleting fails, the error names the leftover by its actual name.
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
        assert_eq!(report.failed, ["order unreadable"]);
        assert!(!data.join(MARKER).exists());
        assert!(data.join("jobs.db").exists(), "nothing deleted");
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
        assert_eq!(state.paused_until, Some(until), "the pause stays");
    }
}
