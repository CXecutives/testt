//! Eingecheckte, anonymisierte Alert-Mails (erfundene Firmen und IDs, Aufbau wie echte
//! Alerts: je Portal zwei Layouts und eine Weiterleitung als Anhang) – Ergebnis als
//! Snapshot. Liegen private echte Mails vor (`fixtures/private/mails`, nicht eingecheckt),
//! werden sie zusätzlich durchlaufen (ohne Snapshot, nur: kein Absturz, Zahlen).

use std::fmt::Write as _;
use std::path::Path;

use jobalert_core::mail::{MailKind, RawMail, classify_mail};
use jobalert_core::portal::Portal;
use jobalert_core::text::split_company_location;

fn describe(bytes: Vec<u8>) -> String {
    match classify_mail(
        &RawMail {
            gmail_id: None,
            bytes,
        },
        &Portal::ALL,
    ) {
        MailKind::Other => "kein Alert\n".into(),
        MailKind::Defective => "unlesbar\n".into(),
        MailKind::Alert(alert) => {
            let mut out = format!(
                "{} | {} | {}\n",
                alert.portal.label(),
                alert.subject,
                alert.sender
            );
            for p in &alert.postings {
                let (company, location) = split_company_location(&p.company, &p.location);
                let _ = writeln!(
                    out,
                    "- {} | {} | {} | {} | {}",
                    p.key, p.title, company, location, p.url
                );
            }
            out
        }
    }
}

#[test]
fn checked_in_alert_mails() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mails");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    assert!(
        names.len() >= 9,
        "je Portal zwei Layouts und eine Weiterleitung"
    );
    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        insta::assert_snapshot!(name, describe(std::fs::read(&path).unwrap()));
    }
}

#[test]
fn private_real_mails_when_available() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private/mails");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("übersprungen: keine privaten Mails in {}", dir.display());
        return;
    };
    let (mut alerts, mut postings, mut other) = (0, 0, 0);
    for entry in entries.flatten() {
        match classify_mail(
            &RawMail {
                gmail_id: None,
                bytes: std::fs::read(entry.path()).unwrap(),
            },
            &Portal::ALL,
        ) {
            MailKind::Alert(a) => {
                alerts += 1;
                postings += a.postings.len();
            }
            _ => other += 1,
        }
    }
    eprintln!("private Mails: {alerts} Alerts mit {postings} Einträgen, {other} ohne Alert");
}
