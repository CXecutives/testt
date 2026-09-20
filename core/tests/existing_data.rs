//! Bestandsdaten einer früheren Version: Einstellungen mit Feldern, die es nicht mehr gibt,
//! ein gewachsener Sicherheitsstand und die bestehende Datenbank. Nach einem Programmwechsel
//! läuft alles weiter – kein Job, keine Pause und keine Anmeldung geht verloren.

use std::path::Path;

use jiff::Timestamp;
use jobalert_core::fetch::policy::{Allowance, PauseKind, Policy};
use jobalert_core::fetch::{Route, route};
use jobalert_core::model::Posting;
use jobalert_core::portal::{Portal, job_link};
use jobalert_core::settings::Settings;
use jobalert_core::store::{JobFilter, MailRef, Store};
use jobalert_core::view::portal_views;

/// Einstellungen, wie sie eine frühere Version geschrieben hat.
const OLD_SETTINGS: &str = r#"{"workspace":null,"format":"xlsx","scope":"week",
     "portals":["linkedin","freelance"],"firstRunSeen":true}"#;

/// `policy.json` aus dem Betrieb: eine laufende Sperrpause, gezählte Zugriffe und eine
/// bestätigte freelance.de-Anmeldung.
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

/// Legt eine Datenbank an, wie sie nach einem früheren Lauf aussieht.
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

    // Neustart mit der neuen Version: dieselbe Datei, dasselbe Schema.
    let store = Store::open(&database).unwrap();
    assert_eq!(store.job_count().unwrap(), 2);
    assert_eq!(store.jobs(&JobFilter::default()).unwrap().len(), 2);
    assert_eq!(store.txt_names().unwrap().len(), 2, "Namen bleiben bekannt");
    // Geschriebene Textdateien gelten weiter als geschrieben.
    assert!(store.txt_jobs(false).unwrap().is_empty());

    // Die Portalwahl überlebt; die Felder von früher werden still übergangen.
    let settings = Settings::load(&store).unwrap();
    assert_eq!(settings.portals, [Portal::LinkedIn, Portal::FreelanceDe]);
    assert_eq!(settings.workspace, None);
    assert!(settings.session_portals.is_empty());
    settings.save(&store).unwrap();
    assert_eq!(Settings::load(&store).unwrap(), settings);

    // Die Sperrpause läuft weiter – sie lässt sich durch ein Programmupdate nicht abkürzen.
    let policy = Policy::load(&policy_file, now());
    let linkedin = policy.state(Portal::LinkedIn);
    assert_eq!(linkedin.pause_kind, Some(PauseKind::Blocked));
    assert!(matches!(
        policy.allowance(Portal::LinkedIn, now()),
        Allowance::Paused { .. }
    ));
    assert_eq!(linkedin.accesses.len(), 2, "Zugriffe zählen weiter mit");
    assert_eq!(policy.usage(Portal::LinkedIn, now()), (2, 2));
    assert_eq!(linkedin.suspicious_streak, 1, "Schutzschalter zählt weiter");
    // Die bestätigte freelance.de-Anmeldung bleibt bestehen.
    let freelance = policy.state(Portal::FreelanceDe);
    assert!(!freelance.login_needed && freelance.session_confirmed_at.is_some());
    assert_eq!(
        route(Portal::FreelanceDe, &settings.session_portals, &policy),
        Route::Session
    );
    // Ein Portal, das die alte Datei gar nicht kannte, beginnt unbelastet.
    assert_eq!(
        policy.allowance(Portal::Freelancermap, now()),
        Allowance::Go
    );

    // Die Portal-Ansicht entsteht aus allen dreien, ohne zu scheitern.
    let views = portal_views(&policy, &store, &settings, now()).unwrap();
    assert_eq!(views.len(), 3);
    let of = |portal| views.iter().find(|v| v.portal == portal).unwrap();
    assert!(of(Portal::LinkedIn).enabled && of(Portal::FreelanceDe).enabled);
    assert!(!of(Portal::Freelancermap).enabled);
    assert_eq!(of(Portal::FreelanceDe).signed_in, Some(true));
    assert_eq!(of(Portal::Freelancermap).signed_in, None);
    assert_eq!(of(Portal::LinkedIn).ok, 1);
}
