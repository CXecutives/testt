//! Checked-in, anonymised alert mails (invented companies and ids, structured like real
//! alerts: two layouts per portal plus one forward as an attachment) - result as a
//! snapshot. When private real mails are present (`fixtures/private/mails`, not checked
//! in), they run through too (no snapshot, just: no crash, counts). The classifier's
//! output strings ("kein Alert", "unlesbar") are snapshot data, do not translate.

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
        // Snapshot data, do not translate.
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
    assert!(names.len() >= 9, "two layouts per portal plus one forward");
    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        insta::assert_snapshot!(name, describe(std::fs::read(&path).unwrap()));
    }
}

#[test]
fn private_real_mails_when_available() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private/mails");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("skipped: no private mails in {}", dir.display());
        return;
    };
    let (mut alerts, mut postings, mut other) = (0, 0, 0);
    for entry in entries.flatten() {
        let bytes = std::fs::read(entry.path()).unwrap();
        match classify_mail(
            &RawMail {
                gmail_id: None,
                bytes: bytes.clone(),
            },
            &Portal::ALL,
        ) {
            MailKind::Alert(a) => {
                alerts += 1;
                postings += a.postings.len();
            }
            _ => other += 1,
        }
        // Print in full (`--nocapture`): real mails show whether title, company and
        // location are right - counting alone doesn't reveal that.
        eprintln!(
            "--- {}\n{}",
            entry.file_name().to_string_lossy(),
            describe(bytes)
        );
    }
    eprintln!("private mails: {alerts} alerts with {postings} entries, {other} without an alert");
}
