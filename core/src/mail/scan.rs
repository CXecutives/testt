//! Postfach durchsuchen und Alert-Mails in den Speicher übernehmen.
//!
//! Jede Mail wird sofort gespeichert – ein Abbruch oder Netzfehler verliert nichts, was
//! schon verarbeitet war. Die Aufnahme ist idempotent: Überlappende Zeiträume schaden
//! nicht, deshalb gibt es keine UID-Zeiger (und keine UIDVALIDITY-Fallen), nur einen
//! Zeitstempel je Portal.

use jiff::civil::Date;
use jiff::{Timestamp, ToSpan as _};
use tokio_util::sync::CancellationToken;

use super::imap::{BATCH, MailError, MailSource};
use super::{MailKind, classify_mail};
use crate::model::AlertMail;
use crate::portal::Portal;
use crate::store::{Seen, Store};
use crate::time::local_date;

/// Welche Mails ein Scan betrachtet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// Seit dem letzten erfolgreichen Scan (einen Tag Überlappung); beim ersten Mal 7 Tage.
    New,
    /// Die letzten 7 Tage.
    Week,
    /// Der ganze Posteingang.
    All,
}

/// Ohne bisherigen Scan: so weit zurück.
const FIRST_SCAN_DAYS: i32 = 7;

/// Zähler eines Scans. Invariante: `postings_total = new + known_before + dup_in_run`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    /// Treffer der Suche.
    pub mails_found: usize,
    /// Davon abgeholt und geprüft.
    pub mails_checked: usize,
    /// Nicht lesbar (gezählt statt still verworfen).
    pub mails_defective: usize,
    pub alert_mails: usize,
    /// Alert-Mails ohne einen erkannten Eintrag (Layout geändert?).
    pub zero_posting_mails: usize,
    pub postings_total: usize,
    pub new: usize,
    pub known_before: usize,
    pub dup_in_run: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(transparent)]
    Mail(#[from] MailError),
    #[error(transparent)]
    Store(#[from] crate::Error),
}

/// Was der Scan unterwegs meldet.
#[derive(Debug)]
pub enum ScanEvent<'a> {
    Found { total: usize },
    Alert(&'a AlertMail),
    Progress { done: usize, total: usize },
}

/// Ab welchem Tag gesucht wird (`None` = alles).
fn scan_since(
    store: &Store,
    scope: Scope,
    portals: &[Portal],
    now: Timestamp,
) -> crate::Result<Option<Date>> {
    let today = local_date(now);
    let week_ago = today.saturating_sub(FIRST_SCAN_DAYS.days());
    Ok(match scope {
        Scope::All => None,
        Scope::Week => Some(week_ago),
        Scope::New => {
            let mut since = today;
            for &portal in portals {
                let from = match store.last_scan(portal)? {
                    // Ein Tag Überlappung: IMAP sucht tageweise, in der Zeitzone des Servers.
                    Some(at) if at <= now => local_date(at).saturating_sub(1.day()),
                    // Stand in der Zukunft (Uhr war falsch gestellt): gilt als unbekannt.
                    _ => week_ago,
                };
                since = since.min(from);
            }
            Some(since)
        }
    })
}

/// Durchsucht das Postfach und übernimmt alle Alert-Mails der gewählten Portale.
///
/// `summary` gehört dem Aufrufer: Auch nach einem Fehler oder Abbruch enthält es, was
/// bis dahin verarbeitet (und schon gespeichert) wurde. Der Scan-Stand je Portal rückt
/// nur nach einem vollständigen Durchlauf vor – und nur für Portale, deren Lücke seit
/// dem letzten Stand der Zeitraum ganz abdeckte.
#[expect(
    clippy::too_many_arguments,
    reason = "Postfach, Speicher, Umfang, Uhr und Ereignisse kommen einzeln (in Tests austauschbar)"
)]
pub async fn scan<S: MailSource>(
    source: &mut S,
    store: &Store,
    run: i64,
    scope: Scope,
    portals: &[Portal],
    started: Timestamp,
    cancel: &CancellationToken,
    summary: &mut ScanSummary,
    mut on_event: impl FnMut(ScanEvent<'_>),
) -> Result<(), ScanError> {
    let since = scan_since(store, scope, portals, started)?;
    let uids = source.search(since, portals).await?;
    summary.mails_found = uids.len();
    on_event(ScanEvent::Found { total: uids.len() });

    let mut done = 0;
    for chunk in uids.chunks(BATCH) {
        if cancel.is_cancelled() {
            return Err(MailError::Cancelled.into());
        }
        for raw in source.fetch(chunk).await? {
            summary.mails_checked += 1;
            match classify_mail(&raw, portals) {
                MailKind::Defective => summary.mails_defective += 1,
                MailKind::Other => {}
                MailKind::Alert(alert) => {
                    take_alert(store, run, &alert, started, summary)?;
                    on_event(ScanEvent::Alert(&alert));
                }
            }
        }
        // Fortschritt je bearbeiteter UID: Inzwischen gelöschte Mails fehlen in der Antwort,
        // der Balken erreicht trotzdem 100 %.
        done += chunk.len();
        on_event(ScanEvent::Progress {
            done,
            total: uids.len(),
        });
    }
    if cancel.is_cancelled() {
        return Err(MailError::Cancelled.into());
    }

    for &portal in portals {
        // Abgedeckt ist die Lücke nur mit einem Tag Überlappung (wie in `scan_since`). Ein
        // Stand in der Zukunft zählt wie „unbekannt“ und wird ersetzt.
        let covered = match (since, store.last_scan(portal)?) {
            (None, _) | (_, None) => true,
            (Some(since), Some(last)) => last > started || since < local_date(last),
        };
        if covered {
            store.set_last_scan(portal, started)?;
        }
    }
    Ok(())
}

fn take_alert(
    store: &Store,
    run: i64,
    alert: &AlertMail,
    now: Timestamp,
    summary: &mut ScanSummary,
) -> crate::Result<()> {
    // Eine Mail ist eine Änderung; gezählt wird erst, wenn sie gespeichert ist – die
    // Invariante gilt auch nach einem Fehler.
    let seen = store.record_alert(run, alert, now)?;
    summary.alert_mails += 1;
    if alert.postings.is_empty() {
        summary.zero_posting_mails += 1;
    }
    for seen in seen {
        match seen {
            Seen::New => summary.new += 1,
            Seen::KnownBefore => summary.known_before += 1,
            Seen::DupInRun => summary.dup_in_run += 1,
        }
        summary.postings_total += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::RawMail;

    /// Postfach-Attrappe: jede Mail hat eine UID; auf Wunsch scheitert das Abholen ab
    /// einer bestimmten UID.
    struct Fake {
        mails: Vec<(u32, RawMail)>,
        fail_at: Option<u32>,
        searched: Vec<Option<Date>>,
    }

    impl MailSource for Fake {
        async fn search(
            &mut self,
            since: Option<Date>,
            _: &[Portal],
        ) -> Result<Vec<u32>, MailError> {
            self.searched.push(since);
            Ok(self.mails.iter().map(|(uid, _)| *uid).collect())
        }

        async fn fetch(&mut self, uids: &[u32]) -> Result<Vec<RawMail>, MailError> {
            if let Some(fail) = self.fail_at.filter(|f| uids.contains(f)) {
                return Err(MailError::Lost(format!("bei {fail}")));
            }
            Ok(self
                .mails
                .iter()
                .filter(|(uid, _)| uids.contains(uid))
                .map(|(_, m)| m.clone())
                .collect())
        }
    }

    fn alert_mail(id: u64, job_ids: &[u64]) -> RawMail {
        let links = job_ids
            .iter()
            .map(|j| {
                format!(r#"<p><a href="https://www.linkedin.com/jobs/view/{j}/">Rolle {j}</a></p>"#)
            })
            .collect::<Vec<_>>()
            .concat();
        let bytes = format!(
            "From: jobalerts-noreply@linkedin.com\r\nSubject: Neue Jobs\r\n\
             Date: Thu, 03 Sep 2026 08:15:00 +0200\r\nContent-Type: text/html\r\n\r\n{links}"
        );
        RawMail {
            gmail_id: Some(id),
            bytes: bytes.into_bytes(),
        }
    }

    fn fake(fail_at: Option<u32>) -> Fake {
        let mut mails = vec![
            (1, alert_mail(1, &[4_000_000_001, 4_000_000_002])),
            (
                2,
                alert_mail(2, &[4_000_000_002, 4_000_000_003, 4_000_000_003]),
            ),
            (3, RawMail::default()),
            (
                4,
                RawMail {
                    gmail_id: Some(4),
                    bytes: b"From: a@b.de\r\nSubject: Hallo\r\n\r\nText".to_vec(),
                },
            ),
        ];
        // Genug Mails für mehrere Abhol-Blöcke.
        for uid in 5..=60 {
            mails.push((uid, alert_mail(u64::from(uid), &[])));
        }
        Fake {
            mails,
            fail_at,
            searched: Vec::new(),
        }
    }

    fn now() -> Timestamp {
        "2026-09-19T08:00:00Z".parse().unwrap()
    }

    async fn run_scan(
        store: &Store,
        source: &mut Fake,
        scope: Scope,
        at: Timestamp,
    ) -> (ScanSummary, Result<(), ScanError>) {
        run_scan_in(store, source, scope, at).await.1
    }

    async fn run_scan_in(
        store: &Store,
        source: &mut Fake,
        scope: Scope,
        at: Timestamp,
    ) -> (i64, (ScanSummary, Result<(), ScanError>)) {
        let run = store.begin_run().unwrap();
        let mut summary = ScanSummary::default();
        let cancel = CancellationToken::new();
        let result = scan(
            source,
            store,
            run,
            scope,
            &[Portal::LinkedIn],
            at,
            &cancel,
            &mut summary,
            |_| {},
        )
        .await;
        (run, (summary, result))
    }

    #[tokio::test]
    async fn counts_and_invariant() {
        let store = Store::in_memory().unwrap();
        let (run, (s, result)) = run_scan_in(&store, &mut fake(None), Scope::New, now()).await;
        result.unwrap();
        assert_eq!(s.mails_found, 60);
        assert_eq!(s.mails_checked, 60);
        assert_eq!(s.mails_defective, 1);
        assert_eq!(s.alert_mails, 58, "2 mit Einträgen + 56 ohne");
        assert_eq!(s.zero_posting_mails, 56);
        // Layout-Wächter: die grauen Zeilen kommen aus der Datenbank (auch nach Neustart).
        assert_eq!(store.zero_posting_mails(run).unwrap().len(), 56);
        // Mail 2 enthält 4000000003 doppelt – innerhalb einer Mail zusammengeführt.
        assert_eq!(
            (s.postings_total, s.new, s.known_before, s.dup_in_run),
            (4, 3, 0, 1)
        );
        assert_eq!(s.postings_total, s.new + s.known_before + s.dup_in_run);
        // Zweiter Lauf: nichts neu.
        let (s, _) = run_scan(&store, &mut fake(None), Scope::New, now()).await;
        assert_eq!((s.new, s.known_before, s.dup_in_run), (0, 3, 1));
    }

    #[tokio::test]
    async fn since_follows_the_scan_state() {
        let store = Store::in_memory().unwrap();
        let mut source = fake(None);
        run_scan(&store, &mut source, Scope::New, now())
            .await
            .1
            .unwrap();
        let later: Timestamp = "2026-09-25T08:00:00Z".parse().unwrap();
        run_scan(&store, &mut source, Scope::New, later)
            .await
            .1
            .unwrap();
        run_scan(&store, &mut source, Scope::All, later)
            .await
            .1
            .unwrap();
        let day = |s: &str| Some(s.parse::<Date>().unwrap());
        assert_eq!(
            source.searched,
            [day("2026-09-12"), day("2026-09-18"), None],
            "Erstlauf 7 Tage; dann letzter Stand − 1 Tag; „Alle“ ohne Grenze"
        );
        assert_eq!(store.last_scan(Portal::LinkedIn).unwrap(), Some(later));
    }

    /// Grenzfall ohne Überlappung und ein Stand in der Zukunft rücken den
    /// Stand nicht vor.
    #[tokio::test]
    async fn boundary_and_future_state_do_not_advance() {
        let store = Store::in_memory().unwrap();
        let mut source = fake(None);
        // Letzter Scan 00:30 Berliner Zeit am 12.09.; „Letzte 7 Tage“ am 19.09. sucht ab 12.09.
        let last: Timestamp = "2026-09-11T22:30:00Z".parse().unwrap();
        store.set_last_scan(Portal::LinkedIn, last).unwrap();
        run_scan(&store, &mut source, Scope::Week, now())
            .await
            .1
            .unwrap();
        assert_eq!(store.last_scan(Portal::LinkedIn).unwrap(), Some(last));
        // Stand in der Zukunft: „Neu“ sucht 7 Tage zurück und überschreibt ihn.
        let future: Timestamp = "2027-01-01T08:00:00Z".parse().unwrap();
        store.set_last_scan(Portal::LinkedIn, future).unwrap();
        run_scan(&store, &mut source, Scope::New, now())
            .await
            .1
            .unwrap();
        assert_eq!(
            source.searched.last().unwrap(),
            &Some("2026-09-12".parse().unwrap())
        );
        assert_eq!(store.last_scan(Portal::LinkedIn).unwrap(), Some(now()));
    }

    /// „Letzte 7 Tage“ nach langer Pause deckt die Lücke nicht – der Stand bleibt stehen,
    /// damit „Neu seit letztem Lauf“ danach nichts überspringt.
    #[tokio::test]
    async fn week_scan_does_not_skip_a_gap() {
        let store = Store::in_memory().unwrap();
        let mut source = fake(None);
        run_scan(&store, &mut source, Scope::New, now())
            .await
            .1
            .unwrap();
        let month_later: Timestamp = "2026-10-19T08:00:00Z".parse().unwrap();
        run_scan(&store, &mut source, Scope::Week, month_later)
            .await
            .1
            .unwrap();
        assert_eq!(store.last_scan(Portal::LinkedIn).unwrap(), Some(now()));
    }

    /// Fehler mitten im Scan: Bis dahin Verarbeitetes ist gespeichert und gezählt, der
    /// Scan-Stand rückt nicht vor.
    #[tokio::test]
    async fn failure_keeps_work_but_not_the_state() {
        let store = Store::in_memory().unwrap();
        let (s, result) = run_scan(&store, &mut fake(Some(30)), Scope::New, now()).await;
        assert!(matches!(result, Err(ScanError::Mail(MailError::Lost(_)))));
        assert_eq!(s.mails_checked, BATCH);
        assert_eq!(store.job_count().unwrap(), 3);
        assert_eq!(store.last_scan(Portal::LinkedIn).unwrap(), None);
    }

    #[tokio::test]
    async fn cancel_stops_before_the_next_batch() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let mut summary = ScanSummary::default();
        let cancel = CancellationToken::new();
        let result = scan(
            &mut fake(None),
            &store,
            run,
            Scope::New,
            &[Portal::LinkedIn],
            now(),
            &cancel,
            &mut summary,
            |event| {
                if matches!(event, ScanEvent::Progress { .. }) {
                    cancel.cancel();
                }
            },
        )
        .await;
        assert!(matches!(result, Err(ScanError::Mail(MailError::Cancelled))));
        assert_eq!(summary.mails_checked, BATCH);
        assert_eq!(store.last_scan(Portal::LinkedIn).unwrap(), None);
    }
}
