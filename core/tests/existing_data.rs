//! Existing data of an earlier version: settings with fields that no longer exist, a grown
//! safety state and the existing database. After switching programs everything keeps
//! working - no job, no pause and no sign-in is lost.

use std::path::Path;

use jiff::Timestamp;
use jobalert_core::fetch::PortalHealth;
use jobalert_core::fetch::policy::{Allowance, PauseKind, PauseReason, Policy};
use jobalert_core::model::Posting;
use jobalert_core::portal::{Portal, job_link};
use jobalert_core::settings::Settings;
use jobalert_core::store::{JobFilter, MailRef, Store};
use jobalert_core::view::portal_states;

/// Settings as an earlier version wrote them.
const OLD_SETTINGS: &str = r#"{"workspace":null,"format":"xlsx","scope":"week",
     "portals":["linkedin","freelance"],"firstRunSeen":true}"#;

/// `policy.json` from operation: a running block pause, counted requests and a confirmed
/// freelance.de sign-in. Pause reasons of earlier versions are German texts.
const OLD_POLICY: &str = r#"{
  "portals": {
    "linkedin": {
      "pausedUntil": "2026-09-20T18:00:00Z",
      "pauseKind": "blocked",
      "pauseReason": "HTTP 999 (Zugriff verweigert)",
      "blockUntil": "2026-09-20T18:00:00Z",
      "suspiciousStreak": 1,
      "accesses": ["2026-09-20T07:55:00Z", "2026-09-20T07:56:00Z"],
      "lastDoneAt": "2026-09-20T07:56:10Z"
    },
    "freelance": {
      "accesses": ["2026-09-20T07:40:00Z"],
      "lastDoneAt": "2026-09-20T07:40:30Z",
      "sessionConfirmedAt": "2026-09-20T07:39:00Z",
      "loginNeeded": false
    }
  }
}"#;

fn now() -> Timestamp {
    "2026-09-20T08:00:00Z".parse().unwrap()
}

/// Creates a database as it looks after an earlier run.
fn existing_database(path: &Path) {
    let store = Store::open(path).unwrap();
    let run = store.begin_run().unwrap();
    for url in [
        "https://www.linkedin.com/jobs/view/4123456789/",
        "https://www.freelance.de/project/index.php?id=1255067",
    ] {
        let link = job_link(url).unwrap();
        let posting = Posting::new(link.key.clone(), link.url, "Rolle", "Muster GmbH", "Köln");
        let mail = MailRef {
            subject: "Neue Jobs",
            date: Some(now()),
            gmail_id: Some(0x1a2b),
        };
        store.upsert_posting(run, &posting, mail, now()).unwrap();
        store
            .record_text(&link.key, &"Volltext. ".repeat(20), false, false, now())
            .unwrap();
        store
            .mark_txt_written(
                &link.key,
                &format!("20260920_Rolle_{}.txt", link.key.id),
                now(),
            )
            .unwrap();
    }
    store.kv_set("settings", OLD_SETTINGS).unwrap();
}

#[test]
fn settings_policy_and_database_of_an_earlier_version_keep_working() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join(jobalert_core::DB_FILE);
    let policy_file = dir.path().join(jobalert_core::POLICY_FILE);
    existing_database(&database);
    std::fs::write(&policy_file, OLD_POLICY).unwrap();

    // Restart with the new version: the same file, the same schema.
    let store = Store::open(&database).unwrap();
    assert_eq!(store.job_count().unwrap(), 2);
    assert_eq!(store.jobs(&JobFilter::default()).unwrap().len(), 2);
    assert_eq!(store.txt_names().unwrap().len(), 2, "names stay known");
    // Written text files still count as written.
    assert!(store.txt_jobs(false).unwrap().is_empty());

    // The portal choice survives as the switches; the fields of earlier versions are skipped.
    let settings = Settings::load(&store).unwrap();
    assert_eq!(
        settings.enabled_portals(),
        [Portal::LinkedIn, Portal::FreelanceDe]
    );
    assert!(settings.portal(Portal::LinkedIn).fetch_details);
    assert!(!settings.portal(Portal::FreelanceDe).login_enabled);
    assert!(settings.auto_fetch_on_start);
    assert_eq!(settings.workspace, None);
    settings.save(&store).unwrap();
    assert_eq!(Settings::load(&store).unwrap(), settings);

    // The block pause keeps running - a program update cannot shorten it - and gets its
    // code from the old kind.
    let policy = Policy::load(&policy_file, now());
    let linkedin = policy.state(Portal::LinkedIn);
    assert_eq!(linkedin.pause_kind, Some(PauseKind::Blocked));
    assert!(matches!(
        policy.allowance(Portal::LinkedIn, now()),
        Allowance::Paused {
            reason: PauseReason::Blocked,
            ..
        }
    ));
    assert_eq!(linkedin.accesses.len(), 2, "requests keep counting");
    assert_eq!(policy.usage(Portal::LinkedIn, now()), (2, 2));
    assert_eq!(linkedin.suspicious_streak, 1, "the breaker keeps counting");
    // The confirmed freelance.de sign-in stays.
    let freelance = policy.state(Portal::FreelanceDe);
    assert!(!freelance.login_needed && freelance.session_confirmed_at.is_some());
    assert!(Portal::FreelanceDe.access().can_sign_in());
    // A portal the old file did not know starts unburdened.
    assert_eq!(
        policy.allowance(Portal::Freelancermap, now()),
        Allowance::Go
    );

    // The portal states come from all three without failing.
    let states = portal_states(&policy, &settings, &[], now());
    assert_eq!(states.len(), 3);
    let of = |portal| states.iter().find(|s| s.portal == portal).unwrap();
    assert!(of(Portal::LinkedIn).enabled && of(Portal::FreelanceDe).enabled);
    assert!(!of(Portal::Freelancermap).enabled);
    assert_eq!(of(Portal::FreelanceDe).signed_in, Some(true));
    assert_eq!(of(Portal::Freelancermap).signed_in, None);
    assert!(matches!(
        of(Portal::LinkedIn).health,
        PortalHealth::Paused {
            reason: PauseReason::Blocked,
            ..
        }
    ));
}
