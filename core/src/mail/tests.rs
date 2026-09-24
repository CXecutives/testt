//! Cases from the old program, with regressions for its bugs.

use super::*;
use crate::model::TITLE_PLACEHOLDER;

const ALL: &[Portal] = &Portal::ALL;

/// Builds a mail like `_mail()` in the old test: text only, HTML only (with a text hint) or
/// both.
fn mail(sender: &str, subject: &str, html: Option<&str>, plain: Option<&str>) -> RawMail {
    let head = format!(
        "From: {sender}\r\nTo: ich@gmail.com\r\nSubject: {subject}\r\n\
         Date: Thu, 03 Sep 2026 08:15:00 +0200\r\nMIME-Version: 1.0\r\n"
    );
    let body = match html {
        None => format!(
            "{head}Content-Type: text/plain; charset=utf-8\r\n\r\n{}",
            plain.unwrap_or_default()
        ),
        Some(html) => format!(
            "{head}Content-Type: multipart/alternative; boundary=\"b\"\r\n\r\n\
             --b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n\
             --b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n--b--\r\n",
            plain.unwrap_or("siehe HTML")
        ),
    };
    RawMail {
        gmail_id: Some(0x1a2b),
        bytes: body.into_bytes(),
    }
}

fn alert(raw: &RawMail, allowed: &[Portal]) -> AlertMail {
    match classify_mail(raw, allowed) {
        MailKind::Alert(alert) => alert,
        other => panic!("no alert: {other:?}"),
    }
}

fn is_other(raw: &RawMail, allowed: &[Portal]) -> bool {
    matches!(classify_mail(raw, allowed), MailKind::Other)
}

/// (title, company, location, URL) per entry - company and location cleaned as list and
/// export show them.
fn rows(alert: &AlertMail) -> Vec<(String, String, String, String)> {
    alert
        .postings
        .iter()
        .map(|p| {
            let (company, location) = crate::text::split_company_location(&p.company, &p.location);
            (p.title.clone(), company, location, p.url.to_string())
        })
        .collect()
}

fn row(title: &str, company: &str, location: &str, url: &str) -> (String, String, String, String) {
    (title.into(), company.into(), location.into(), url.into())
}

const LINKEDIN_HTML: &str = r#"
<html><head><style>.x{color:red}</style></head><body>
<table><tr><td>
  <a href="https://www.linkedin.com/comm/jobs/view/4123456789/?trackingId=abc&amp;refId=x">
     Senior Controller (m/w/d)</a>
  <p>Musterwerke GmbH · Köln, Nordrhein-Westfalen</p>
  <span>vor 2 Tagen</span>
  <a href="https://www.linkedin.com/comm/jobs/view/4123456789/?trackingId=abc">Job ansehen</a>
</td></tr>
<tr><td>
  <a href="https://www.linkedin.com/comm/jobs/view/interim-cfo-4987654321?refId=y">Interim CFO</a>
  <div>Nordlicht AG</div><div>Hamburg (Hybrid)</div>
</td></tr>
<tr><td><a href="https://www.linkedin.com/comm/jobs/search/?keywords=controller">Alle Jobs ansehen</a></td></tr>
</table></body></html>
"#;

const FREELANCERMAP_HTML: &str = r#"
<html><body>
<h2>Neue Projekte für Ihre Suche „SAP“</h2>
<a href="https://www.freelancermap.de/projektboerse/projekte/it/2900001-sap-fi-co-berater.html?utm_source=alert">
  SAP FI/CO Berater (m/w/d)</a><br>
Ferrum Systems SE<br>München<br>
<a href="https://www.freelancermap.de/projektboerse/projekte/it/2900001-sap-fi-co-berater.html">Zum Projekt</a>
<a href="https://www.freelancermap.de/projektboerse/projekte/consulting/2900002-projektleiter.html">Projektleiter Finance</a>
<p>Remote</p>
<a href="https://www.freelancermap.de/login">Login</a>
</body></html>
"#;

const FREELANCE_PLAIN: &str = "Hallo,\r\n\r\nes gibt neue Projekte für Ihr Suchprofil:\r\n\r\n\
Data Engineer (Azure) - Remote\r\n\
https://www.freelance.de/projekte/projekt-1200001-Data-Engineer-Azure?ref=mail\r\n\r\n\
Zum Abbestellen: https://www.freelance.de/newsletter/abmelden\r\n";

const FREELANCE_PROJECT_HTML: &str = r#"
<html><body>
<h2>Projektvorschläge der Woche für Ihr Profil</h2>
<a href="https://www.freelance.de/project/index.php?id=1255067&utm_source=wochenmail&utm_campaign=w1">
  SAP S/4HANA-Projektleiter (m/w/d)</a><br>
Muster Consulting GmbH<br>Remote<br>
<a href="https://www.freelance.de/project/index.php?id=1255068&utm_source=wochenmail">PMO Manager (m/w/d)</a><br>
Projektbüro Nord GmbH<br>Berlin<br>
<a href="https://www.freelance.de/projekte/projekt-1255067-sap-s4hana">Zum Projekt</a>
</body></html>
"#;

const FREELANCERMAP_AGENT_HTML: &str = r#"
<html><body>
<h2>Neue Projektanfragen für Sie</h2>
<a href="https://www.freelancermap.de/nproj/2971857.html?utm_source=agent">
  Senior DevOps Engineer (w/m/d)</a><br>
Ferrum Systems SE<br>
Ort: München // Vertragsart: Freiberuflich // Start: ab sofort<br>
<a href="https://www.freelancermap.de/nproj/2971857.html">Zum Projekt</a><br>
<a href="https://www.freelancermap.de/nproj/2971858.html?utm_source=agent">IT-Architekt (m/w/d)</a><br>
Nordwind Consulting<br>
Ort: Remote // Start: ab sofort
</body></html>
"#;

// ------------------------------------------------------------------ ParseTests

#[test]
fn linkedin_html() {
    let raw = mail(
        "LinkedIn Job Alerts <jobalerts-noreply@linkedin.com>",
        "Controller: 2 neue Jobs in Köln",
        Some(LINKEDIN_HTML),
        None,
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::LinkedIn);
    assert_eq!(
        crate::model::gmail_url(a.gmail_id.unwrap())
            .unwrap()
            .as_str(),
        "https://mail.google.com/mail/u/0/#all/1a2b"
    );
    assert_eq!(a.subject, "Controller: 2 neue Jobs in Köln");
    assert_eq!(a.sender, "LinkedIn Job Alerts");
    assert!(a.date.is_some());
    assert_eq!(a.key, "gm:1a2b");
    assert_eq!(
        rows(&a),
        [
            row(
                "Senior Controller (m/w/d)",
                "Musterwerke GmbH",
                "Köln, Nordrhein-Westfalen",
                "https://www.linkedin.com/jobs/view/4123456789/"
            ),
            row(
                "Interim CFO",
                "Nordlicht AG",
                "Hamburg (Hybrid)",
                "https://www.linkedin.com/jobs/view/4987654321/"
            ),
        ]
    );
}

#[test]
fn freelancermap_html() {
    let raw = mail(
        "freelancermap <projekte@freelancermap.de>",
        "Neue Projekte",
        Some(FREELANCERMAP_HTML),
        None,
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::Freelancermap);
    assert_eq!(
        rows(&a),
        [
            row(
                "SAP FI/CO Berater (m/w/d)",
                "Ferrum Systems SE",
                "München",
                "https://www.freelancermap.de/nproj/2900001.html"
            ),
            // As in the old code: "Remote" stands alone behind the title and stays the company.
            row(
                "Projektleiter Finance",
                "Remote",
                "",
                "https://www.freelancermap.de/nproj/2900002.html"
            ),
        ]
    );
}

#[test]
fn freelance_plain_text() {
    let raw = mail(
        "freelance.de <info@freelance.de>",
        "Neue Projekte für Ihr Profil",
        None,
        Some(FREELANCE_PLAIN),
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::FreelanceDe);
    assert_eq!(
        rows(&a),
        [row(
            "Data Engineer (Azure) - Remote",
            "",
            "",
            "https://www.freelance.de/project/index.php?id=1200001"
        )]
    );
}

/// "Projektvorschläge der Woche": the same job as a second link makes no duplicate and
/// does not erase company and location.
#[test]
fn freelance_project_index_php() {
    let raw = mail(
        "freelance.de <info@freelance.de>",
        "Projektvorschläge der Woche",
        Some(FREELANCE_PROJECT_HTML),
        None,
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::FreelanceDe);
    assert_eq!(
        rows(&a),
        [
            row(
                "SAP S/4HANA-Projektleiter (m/w/d)",
                "Muster Consulting GmbH",
                "Remote",
                "https://www.freelance.de/project/index.php?id=1255067"
            ),
            row(
                "PMO Manager (m/w/d)",
                "Projektbüro Nord GmbH",
                "Berlin",
                "https://www.freelance.de/project/index.php?id=1255068"
            ),
        ]
    );
}

#[test]
fn freelancermap_agent_nproj() {
    let raw = mail(
        "freelancermap Projektagent <projekte@freelancermap.de>",
        "Neue Projektanfragen",
        Some(FREELANCERMAP_AGENT_HTML),
        None,
    );
    let a = alert(&raw, ALL);
    assert_eq!(
        rows(&a),
        [
            row(
                "Senior DevOps Engineer (w/m/d)",
                "Ferrum Systems SE",
                "München",
                "https://www.freelancermap.de/nproj/2971857.html"
            ),
            row(
                "IT-Architekt (m/w/d)",
                "Nordwind Consulting",
                "Remote",
                "https://www.freelancermap.de/nproj/2971858.html"
            ),
        ]
    );
}

#[test]
fn non_alert_mail_is_ignored() {
    let raw = mail(
        "LinkedIn <messages-noreply@linkedin.com>",
        "Max hat dir eine Nachricht geschickt",
        Some("<p>Hallo! <a href='https://www.linkedin.com/messaging/'>Antworten</a></p>"),
        None,
    );
    assert!(is_other(&raw, ALL));
}

/// Layout guard: an alert without recognised links stays an alert without entries.
#[test]
fn alert_without_recognised_links_is_reported() {
    let raw = mail(
        "LinkedIn <jobalerts-noreply@linkedin.com>",
        "5 neue Jobs für dich",
        Some("<p>Layout geändert, keine Links.</p>"),
        None,
    );
    assert!(alert(&raw, ALL).postings.is_empty());
}

#[test]
fn portal_filter() {
    let raw = mail(
        "jobalerts-noreply@linkedin.com",
        "Neue Jobs",
        Some(LINKEDIN_HTML),
        None,
    );
    assert!(is_other(&raw, &[Portal::FreelanceDe]));
}

#[test]
fn encoded_sender_and_subject() {
    let raw = mail(
        "=?utf-8?B?RXJpa2EgTcO8bGxlcg==?= <erika@example.com>",
        "=?utf-8?q?K=C3=B6ln=3A_neue_Jobs?=",
        Some(LINKEDIN_HTML),
        None,
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.sender, "Erika Müller");
    assert_eq!(a.subject, "Köln: neue Jobs");
}

// --------------------------------------------------------------- ForwardedTests

#[test]
fn forwarded_linkedin_alert() {
    let html = format!(
        "<p>---------- Weitergeleitete Nachricht ----------<br>Von: LinkedIn Job Alerts</p>{LINKEDIN_HTML}"
    );
    let raw = mail(
        "Erika <erika@example.com>",
        "Fwd: Controller: 2 neue Jobs in Köln",
        Some(&html),
        None,
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::LinkedIn);
    assert_eq!(a.postings.len(), 2);
    assert_eq!(a.postings[0].title, "Senior Controller (m/w/d)");
}

#[test]
fn forwarded_freelancermap_with_linkedin_in_signature() {
    let html = FREELANCERMAP_HTML.replace("</body>", "<p>Folgen Sie uns auf LinkedIn</p></body>");
    let raw = mail(
        "Erika <erika@example.com>",
        "WG: Neue Projekte",
        Some(&html),
        None,
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::Freelancermap);
    assert_eq!(a.postings.len(), 2);
}

#[test]
fn forwarded_plain_text_freelance() {
    let raw = mail(
        "Erika <erika@example.com>",
        "Fwd: Neue Projekte für Ihr Profil",
        None,
        Some(FREELANCE_PLAIN),
    );
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::FreelanceDe);
    assert_eq!(a.postings.len(), 1);
}

#[test]
fn unrelated_mail_is_ignored() {
    let raw = mail(
        "Kollege <k@firma.de>",
        "Neue Termine",
        None,
        Some("Hallo, hier die neuen Termine."),
    );
    assert!(is_other(&raw, ALL));
}

/// Measured on a real newsletter: "linkedin" four times in the footer, no job link - no
/// alert.
#[test]
fn newsletter_footer_is_not_a_portal_alert() {
    let html = r#"<img src="https://assets.example/1746739598700_linkedin-4x_01jtrw.png" alt="linkedin" height="38" width="38"><a href="https://www.linkedin.com/company/flownotes/">linkedin</a>"#;
    let raw = mail(
        "Flow Notes <hello@mail.flownotes.example>",
        "Unlocked: your new notetaker",
        Some(html),
        Some("always inform everyone on a call"),
    );
    assert!(is_other(&raw, ALL));
}

#[test]
fn portal_named_in_the_subject_counts_without_a_job_link() {
    let raw = mail(
        "erika@example.com",
        "Ihre Job-Alerts von LinkedIn",
        None,
        Some("Kein Link erkannt"),
    );
    assert_eq!(alert(&raw, ALL).portal, Portal::LinkedIn);
}

#[test]
fn mail_from_a_portal_domain_still_wins() {
    let raw = mail(
        "LinkedIn <jobalerts-noreply@linkedin.com>",
        "Neue Jobs für Sie",
        None,
        Some("Kein Link erkannt"),
    );
    assert_eq!(alert(&raw, ALL).portal, Portal::LinkedIn);
}

// ------------------------------------------------------------ new regressions

/// A forwarded collection mail with jobs of two portals
/// lost the second. Today every entry carries its own portal.
#[test]
fn collection_mail_keeps_both_portals() {
    let html = format!("{LINKEDIN_HTML}{FREELANCERMAP_AGENT_HTML}");
    let raw = mail("erika@example.com", "Fwd: Sammlung", Some(&html), None);
    let a = alert(&raw, ALL);
    let portals: Vec<Portal> = a.postings.iter().map(|p| p.key.portal).collect();
    assert_eq!(
        portals,
        [
            Portal::LinkedIn,
            Portal::LinkedIn,
            Portal::Freelancermap,
            Portal::Freelancermap
        ]
    );
    // Only the chosen portals make it into the list.
    let only_fm = alert(&raw, &[Portal::Freelancermap]);
    assert_eq!(only_fm.portal, Portal::Freelancermap);
    assert_eq!(only_fm.postings.len(), 2);
}

/// Forwarded as an attachment (`message/rfc822`): formerly that gave 0 entries.
#[test]
fn forwarded_as_attachment() {
    let raw = format!(
        "From: erika@example.com\r\nSubject: Fwd: Jobs\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"o\"\r\n\r\n\
         --o\r\nContent-Type: text/plain\r\n\r\nSiehe Anhang\r\n\
         --o\r\nContent-Type: message/rfc822\r\n\r\n\
         From: LinkedIn <jobalerts-noreply@linkedin.com>\r\nSubject: 2 neue Jobs\r\n\
         Content-Type: text/html; charset=utf-8\r\n\r\n{LINKEDIN_HTML}\r\n--o--\r\n"
    );
    let raw = RawMail {
        bytes: raw.into_bytes(),
        ..RawMail::default()
    };
    let a = alert(&raw, ALL);
    assert_eq!(a.portal, Portal::LinkedIn);
    assert_eq!(a.postings.len(), 2);
    assert!(
        a.key.starts_with("h:"),
        "ohne Gmail-/Message-ID: Inhalts-Hash"
    );
}

/// Plain text adds to the HTML instead of only stepping in when the HTML gives nothing.
#[test]
fn plain_text_adds_what_html_missed() {
    let raw = mail(
        "info@freelance.de",
        "Neue Projekte",
        Some(FREELANCE_PROJECT_HTML),
        Some(FREELANCE_PLAIN),
    );
    let keys: Vec<String> = alert(&raw, ALL)
        .postings
        .iter()
        .map(|p| p.key.id.clone())
        .collect();
    assert_eq!(keys, ["1255067", "1255068", "1200001"]);
}

#[test]
fn defective_and_placeholder() {
    let garbage = RawMail::default();
    assert!(matches!(classify_mail(&garbage, ALL), MailKind::Defective));
    // Binary garbage without head lines is unreadable, not "no alert".
    let junk = RawMail {
        bytes: vec![0x00, 0xff, 0x13, 0x37, 0x0a, 0x80],
        ..RawMail::default()
    };
    assert!(matches!(classify_mail(&junk, ALL), MailKind::Defective));
    // A link without a recognisable title: the placeholder instead of an empty row.
    let raw = mail(
        "jobalerts-noreply@linkedin.com",
        "Neue Jobs",
        Some(r#"<a href="https://www.linkedin.com/jobs/view/4000000009/">Job ansehen</a>"#),
        None,
    );
    assert_eq!(alert(&raw, ALL).postings[0].title, TITLE_PLACEHOLDER);
}

/// Two phases must lose no alert: every checked-in alert mail is a candidate by its head
/// alone, the newsletter is not - so it is never loaded whole.
#[test]
fn every_alert_is_a_candidate_by_its_head() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mails");
    let mut alerts = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        let raw = RawMail {
            gmail_id: None,
            bytes: bytes.clone(),
        };
        let head = head_part(&bytes);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        match classify_mail(&raw, ALL) {
            MailKind::Alert(_) => {
                alerts += 1;
                assert!(is_candidate(head, ALL), "{name}");
            }
            _ if name.starts_with("no_alert") => assert!(!is_candidate(head, ALL), "{name}"),
            _ => {}
        }
    }
    assert!(alerts >= 12, "{alerts}");
    // The unit test mails too: forwarded ones by their prefix, originals by their sender.
    for (sender, subject, expected) in [
        ("Ich <ich@example.org>", "Fwd: Neue Projekte", true),
        ("Ich <ich@example.org>", "WG: Suchagent vom 18.09.", true),
        ("Ich <ich@example.org>", "Mein LinkedIn-Alert", true),
        ("Jobs <jobs-listings@linkedin.com>", "Irgendwas", true),
        ("Kollege <max@firma.de>", "Neue Termine", false),
        ("News <news@example.org>", "Unlocked: your notetaker", false),
    ] {
        let head = format!("From: {sender}\r\nSubject: {subject}\r\n\r\n");
        assert_eq!(is_candidate(head.as_bytes(), ALL), expected, "{subject}");
    }
    // An unreadable head is loaded (and then counted as defective).
    assert!(is_candidate(b"", ALL));
}
