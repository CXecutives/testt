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

fn classify(bytes: Vec<u8>) -> MailKind {
    classify_mail(
        &RawMail {
            gmail_id: None,
            bytes,
        },
        &Portal::ALL,
    )
}

/// The `.eml` files of a fixture folder, sorted, with their names.
fn fixtures(folder: &str) -> Vec<(String, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(folder);
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            (name, std::fs::read(&path).unwrap())
        })
        .collect()
}

#[test]
fn checked_in_alert_mails() {
    let mails = fixtures("mails");
    assert!(mails.len() >= 9, "two layouts per portal plus one forward");
    for (name, bytes) in mails {
        insta::assert_snapshot!(name, describe(bytes));
    }
}

/// Every checked-in alert is an alert with jobs; only the `no_alert` mails are none.
#[test]
fn every_alert_fixture_is_an_alert_with_jobs() {
    for (name, bytes) in fixtures("mails") {
        match classify(bytes) {
            MailKind::Alert(alert) => {
                assert!(!name.starts_with("no_alert"), "{name}");
                assert!(!alert.postings.is_empty(), "{name}");
            }
            _ => assert!(name.starts_with("no_alert"), "{name}"),
        }
    }
}

/// Onboarding, promo and network mails from the portals' own addresses (invented, with
/// subjects like the ones a real test mailbox collected): none is an alert - so none can
/// raise the "mail layout changed?" warning, although they speak of projects, agents and
/// jobs.
#[test]
fn promo_and_onboarding_mails_are_no_alerts() {
    let mails = fixtures("promo_mails");
    assert!(mails.len() >= 13, "{}", mails.len());
    let mut out = String::new();
    for (name, bytes) in mails {
        let verdict = describe(bytes.clone());
        assert!(
            matches!(classify(bytes), MailKind::Other),
            "{name}: {verdict}"
        );
        let _ = write!(out, "{name}: {verdict}");
    }
    insta::assert_snapshot!("promo_mails", out);
}

/// The layout guard: a real alert whose job links the app no longer recognises (the
/// portal changed its link form) stays an alert without jobs - that is what the warning is
/// for. Hand-made collection mails carry no alert markers of their own.
#[test]
fn an_alert_with_unrecognised_links_stays_an_alert() {
    const HAND_MADE: &[&str] = &["forward_autolinked", "forward_composite"];
    let mut guarded = 0;
    for (name, bytes) in fixtures("mails") {
        if name.starts_with("no_alert") || HAND_MADE.contains(&name.as_str()) {
            continue;
        }
        let MailKind::Alert(original) = classify(bytes.clone()) else {
            panic!("{name}");
        };
        match classify(unknown_links(&bytes)) {
            MailKind::Alert(alert) => {
                assert!(alert.postings.is_empty(), "{name}");
                assert_eq!(alert.portal, original.portal, "{name}");
            }
            other => panic!("{name}: {other:?}"),
        }
        guarded += 1;
    }
    assert!(guarded >= 11, "{guarded}");
}

/// The mail with its job links rewritten to forms the portals do not use (yet): nothing
/// in it is a job link any more.
fn unknown_links(bytes: &[u8]) -> Vec<u8> {
    [
        ("/jobs/view/", "/jobs/show/"),
        ("/nproj/", "/np/"),
        ("/projektboerse/projekte/", "/pb/"),
        ("freelancermap.de/projekt/", "freelancermap.de/pj/"),
        ("/project/index.php", "/p.php"),
        ("/projekte/projekt-", "/projekte/p-"),
    ]
    .iter()
    .fold(
        String::from_utf8_lossy(bytes).into_owned(),
        |text, (from, to)| text.replace(from, to),
    )
    .into_bytes()
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
        // Print in full (`--nocapture`): real mails show whether title, company and
        // location are right - counting alone doesn't reveal that.
        eprintln!(
            "--- {}\n{}",
            entry.file_name().to_string_lossy(),
            describe(bytes.clone())
        );
        match classify(bytes.clone()) {
            MailKind::Alert(a) => {
                alerts += 1;
                postings += a.postings.len();
                // The layout guard on real mails: with unknown link forms still an alert?
                let guarded = matches!(classify(unknown_links(&bytes)), MailKind::Alert(_));
                eprintln!("layout guard: {guarded}");
            }
            _ => other += 1,
        }
    }
    eprintln!("private mails: {alerts} alerts with {postings} entries, {other} without an alert");
}
