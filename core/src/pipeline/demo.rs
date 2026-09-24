//! Dry run: a mailbox and portals to look at - without network, without an account. The run
//! behaves like a real one (pace, events, summary) but writes nothing: database and safety
//! state then only live in memory. The sample mails are German like real alert mails.

use std::sync::Arc;
use std::time::Duration;

use jiff::civil::Date;
use tokio_util::sync::CancellationToken;

use super::{Backends, Matcher};
use crate::fetch::{Cause, PageFetcher, PageOutcome};
use crate::mail::RawMail;
use crate::mail::imap::{MailError, MailSource};
use crate::model::{MatchRecord, MatchStatus, Notice};
use crate::portal::{FetchPath, JobLink, Portal};
use crate::store::JobRow;

/// Sample alerts per portal (invented companies).
const MAILS: [(Portal, &str, &str, &str); 3] = [
    (
        Portal::LinkedIn,
        "LinkedIn Job Alerts <jobalerts-noreply@linkedin.com>",
        "Interim CFO: 2 neue Jobs in Hamburg",
        r#"<table><tr><td><a href="https://www.linkedin.com/comm/jobs/view/4999000001/">Interim CFO (m/w/d)</a><p>Nordlicht AG · Hamburg</p></td></tr>
           <tr><td><a href="https://www.linkedin.com/comm/jobs/view/4999000002/">Leiter Controlling (m/w/d)</a><p>Hafenwerke GmbH · Bremen (Hybrid)</p></td></tr></table>"#,
    ),
    (
        Portal::Freelancermap,
        "freelancermap <projekte@freelancermap.de>",
        "Neue Projekte für Ihre Suche „SAP“",
        r#"<a href="https://www.freelancermap.de/nproj/2999001.html">SAP FI/CO Berater (m/w/d)</a><br>Ferrum Systems SE<br>Ort: München // Start: ab sofort<br>
           <a href="https://www.freelancermap.de/nproj/2999002.html">Projektleiter S/4HANA</a><br>Nordwind Consulting<br>Ort: Remote"#,
    ),
    (
        Portal::FreelanceDe,
        "freelance.de <info@freelance.de>",
        "Projektvorschläge der Woche",
        r#"<a href="https://www.freelance.de/project/index.php?id=1999001">PMO Manager (m/w/d)</a><br>Projektbüro Nord GmbH<br>Berlin"#,
    ),
];

#[derive(Default)]
pub struct DemoBackends;

impl Backends for DemoBackends {
    type Mail = DemoMail;
    type Pages = DemoPages;

    async fn connect_mail(&mut self, cancel: &CancellationToken) -> Result<DemoMail, MailError> {
        pause(Duration::from_millis(600), cancel).await?;
        Ok(DemoMail {
            cancel: cancel.clone(),
        })
    }

    fn pages(&mut self, _portal: Portal, _path: FetchPath) -> Result<DemoPages, String> {
        Ok(DemoPages)
    }

    fn matcher(&self) -> Option<Arc<dyn Matcher>> {
        Some(Arc::new(DemoMatcher))
    }
}

/// Scores to look at: one job of each band, one excluded, one without a judgement basis.
pub struct DemoMatcher;

impl Matcher for DemoMatcher {
    fn rev(&self) -> &'static str {
        "demo"
    }

    fn assess(&self, job: &JobRow, _text: Option<&str>) -> Option<MatchRecord> {
        let (status, score, note) = match job.key.id.as_str() {
            "4999000001" => (MatchStatus::Scored, 88, None),
            "4999000002" => (MatchStatus::Scored, 62, None),
            "2999001" => (MatchStatus::Scored, 24, None),
            "2999002" => (
                MatchStatus::Excluded,
                71,
                Some(Notice {
                    code: "hardCriterion".into(),
                    params: serde_json::Map::from_iter([("criterion".into(), "dayRate".into())]),
                }),
            ),
            _ => (MatchStatus::Unscorable, 0, None),
        };
        Some(MatchRecord {
            status,
            score,
            note,
            must_met: 2,
            must_total: 3,
            top: vec![
                "Projektleitung".into(),
                "Abstimmung mit Fachbereichen".into(),
            ],
        })
    }
}

pub struct DemoMail {
    cancel: CancellationToken,
}

impl MailSource for DemoMail {
    async fn search(&mut self, _: Option<Date>, portals: &[Portal]) -> Result<Vec<u32>, MailError> {
        pause(Duration::from_millis(400), &self.cancel).await?;
        Ok(MAILS
            .iter()
            .zip(1u32..)
            .filter(|((portal, ..), _)| portals.contains(portal))
            .map(|(_, uid)| uid)
            .collect())
    }

    async fn fetch(&mut self, uids: &[u32]) -> Result<Vec<RawMail>, MailError> {
        pause(Duration::from_millis(300), &self.cancel).await?;
        // Today's date: the sample jobs always lie within the 30-day window of the fetch.
        let date = jiff::Timestamp::now().strftime("%a, %d %b %Y %H:%M:%S +0000");
        Ok(uids
            .iter()
            .filter_map(|&uid| {
                let (_, from, subject, html) =
                    MAILS.get(usize::try_from(uid.checked_sub(1)?).ok()?)?;
                let bytes = format!(
                    "From: {from}\r\nSubject: {subject}\r\nDate: {date}\r\n\
                     MIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}"
                );
                Some(RawMail {
                    gmail_id: Some(0x1990_0000 + u64::from(uid)),
                    bytes: bytes.into_bytes(),
                })
            })
            .collect())
    }
}

/// Pages to look at. Between the requests the real pace applies (waits with a countdown);
/// freelance.de answers with a throttle, so that a portal stop can be seen too.
pub struct DemoPages;

impl PageFetcher for DemoPages {
    async fn fetch(&mut self, link: &JobLink, cancel: &CancellationToken) -> PageOutcome {
        if pause(Duration::from_millis(500), cancel).await.is_err() {
            return PageOutcome::Cancelled;
        }
        if link.key.portal == Portal::FreelanceDe {
            return PageOutcome::Throttled {
                cause: Cause::DrySample,
                retry_after: None,
            };
        }
        PageOutcome::Text {
            text: format!(
                "Beispieltext (Trockenlauf) für {}.\n\nAufgaben:\n- Projektleitung\n- Abstimmung mit Fachbereichen\n\nProfil:\n- mehrjährige Erfahrung\n- sehr gute Deutschkenntnisse",
                link.key
            ),
            short: false,
            closed: false,
            fields: None,
            facts: crate::portal::Facts::default(),
        }
    }
}

async fn pause(length: Duration, cancel: &CancellationToken) -> Result<(), MailError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(MailError::Cancelled),
        () = tokio::time::sleep(length) => Ok(()),
    }
}
