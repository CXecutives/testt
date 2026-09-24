//! Dry run: a mailbox and portals to look at - without network, without an account. The run
//! behaves like a real one (pace, events, summary) but writes nothing: database and safety
//! state then only live in memory. The sample mails and ads are German like real ones; the
//! jobs are scored by the real engine against the invented sample profile of the matching
//! corpus, so the list shows real rings (high, mid, low, excluded and one without details).

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use jiff::civil::Date;
use tokio_util::sync::CancellationToken;

use super::{Backends, LocalMatcher, Matcher};
use crate::fetch::{Cause, PageFetcher, PageOutcome};
use crate::mail::imap::{MailError, MailSource};
use crate::mail::{RawHead, RawMail, head_part};
use crate::portal::{FetchPath, JobLink, Portal};

/// The profile of the dry run: the invented interim finance profile of the matching corpus.
pub const PROFILE_JSON: &str = include_str!("../../tests/fixtures/matching/sample_profile.json");
/// File name the dry run shows for it (a name, German like the profile keys).
pub const PROFILE_NAME: &str = "beispielprofil.json";

static MATCHER: LazyLock<Arc<LocalMatcher>> = LazyLock::new(|| {
    let value = serde_json::from_str(PROFILE_JSON).unwrap_or_default();
    Arc::new(LocalMatcher::from_json(&value))
});

/// The engine with the sample profile.
pub fn matcher() -> Arc<LocalMatcher> {
    MATCHER.clone()
}

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
        Some(matcher())
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

    async fn heads(&mut self, uids: &[u32]) -> Result<Vec<RawHead>, MailError> {
        pause(Duration::from_millis(100), &self.cancel).await?;
        Ok(uids
            .iter()
            .filter_map(|&uid| {
                Some(RawHead {
                    uid,
                    bytes: head_part(&sample(uid)?.bytes).to_vec(),
                })
            })
            .collect())
    }

    async fn fetch(&mut self, uids: &[u32]) -> Result<Vec<RawMail>, MailError> {
        pause(Duration::from_millis(300), &self.cancel).await?;
        Ok(uids.iter().filter_map(|&uid| sample(uid)).collect())
    }
}

/// Sample mail `uid` (1-based).
fn sample(uid: u32) -> Option<RawMail> {
    let (_, from, subject, html) = MAILS.get(usize::try_from(uid.checked_sub(1)?).ok()?)?;
    // Today's date: the sample jobs always lie within the 30-day window of the fetch.
    let date = jiff::Timestamp::now().strftime("%a, %d %b %Y %H:%M:%S +0000");
    let bytes = format!(
        "From: {from}\r\nSubject: {subject}\r\nDate: {date}\r\n\
         MIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}"
    );
    Some(RawMail {
        gmail_id: Some(0x1990_0000 + u64::from(uid)),
        bytes: bytes.into_bytes(),
    })
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
            text: ad(&link.key.id).to_owned(),
            short: false,
            closed: false,
            fields: None,
            facts: crate::portal::Facts::default(),
        }
    }
}

/// The sample ad of a job: one fits the profile well, one partly, one hardly, one breaks a
/// hard criterion (day rate below the minimum).
fn ad(id: &str) -> &'static str {
    match id {
        "4999000001" => AD_HIGH,
        "4999000002" => AD_MID,
        "2999002" => AD_EXCLUDED,
        _ => AD_LOW,
    }
}

const AD_HIGH: &str = "Beispielanzeige (Trockenlauf)

Für die Nordlicht AG suchen wir ab sofort einen Interim CFO (m/w/d) für neun Monate.

Aufgaben:
- Leitung von Controlling und Konzernrechnungslegung nach IFRS
- Monatsabschlüsse und Jahresabschlüsse im Konzern
- Budgetierung, Forecast und Liquiditätsplanung
- Begleitung der Restrukturierung

Anforderungen:
- Mehrjährige Erfahrung im Interim Management
- Fundierte Kenntnisse in Konsolidierung und IFRS
- Erfahrung mit SAP S/4HANA
- Sehr gute Deutschkenntnisse und gute Englischkenntnisse

Rahmenbedingungen:
- Tagessatz 1.200 €
- Einsatzort Hamburg, 60 % remote";

const AD_MID: &str = "Beispielanzeige (Trockenlauf)

Die Hafenwerke GmbH sucht für ein Interim-Mandat von sechs Monaten eine Leitung Controlling (m/w/d).

Aufgaben:
- Führung des Controlling-Teams
- Aufbau eines Hafenlogistik-Controllings

Anforderungen:
- Erfahrung im Controlling
- Erfahrung mit Power BI
- Budgetierung und Forecast
- Kenntnisse in Zollabwicklung
- Erfahrung in der Tarifkalkulation für Terminals
- Staplerschein

Rahmenbedingungen:
- Einsatzort Bremen, zwei Tage remote";

const AD_LOW: &str = "Beispielanzeige (Trockenlauf)

Für ein Entwicklungsprojekt suchen wir Unterstützung (m/w/d).

Anforderungen:
- ABAP-Entwicklung
- SAP BTP und SAP Fiori
- Schnittstellen mit IDoc und OData
- Erfahrung mit Java und Kubernetes

Rahmenbedingungen:
- Einsatzort München, remote möglich";

const AD_EXCLUDED: &str = "Beispielanzeige (Trockenlauf)

Für die Einführung von SAP S/4HANA im Finanzbereich suchen wir eine Projektleitung (m/w/d).

Anforderungen:
- Projektmanagement in SAP-Einführungen
- Erfahrung mit SAP S/4HANA und SAP FI/CO
- Prozessoptimierung im Finanzbereich

Rahmenbedingungen:
- Tagessatz bis 800 €
- 100 % remote";

async fn pause(length: Duration, cancel: &CancellationToken) -> Result<(), MailError> {
    if crate::time::sleep_cancellable(length, cancel).await {
        Ok(())
    } else {
        Err(MailError::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::{self, JobInput, TextKind, Verdict};
    use crate::model::{Band, band};

    /// The sample ads give every ring the list knows, judged by the real engine.
    #[test]
    fn the_sample_ads_show_every_ring() {
        let matcher = matcher();
        assert!(matcher.usable());
        let judge = |id: &str, title: &str, location: &str| {
            let job = JobInput {
                title,
                location,
                portal: Portal::LinkedIn,
                text: ad(id),
                facts: None,
                posted: None,
                kind: TextKind::Full,
            };
            let a = matching::assess(matcher.profile(), &job, None).unwrap();
            (a.verdict, band(a.score))
        };
        assert_eq!(
            judge("4999000001", "Interim CFO (m/w/d)", "Hamburg"),
            (Verdict::Scored, Band::High)
        );
        assert_eq!(
            judge(
                "4999000002",
                "Leiter Controlling (m/w/d)",
                "Bremen (Hybrid)"
            ),
            (Verdict::Scored, Band::Mid)
        );
        assert_eq!(
            judge("2999001", "SAP FI/CO Berater (m/w/d)", "München"),
            (Verdict::Scored, Band::Low)
        );
        assert_eq!(
            judge("2999002", "Projektleiter S/4HANA", "Remote").0,
            Verdict::Excluded
        );
    }
}
