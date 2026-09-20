//! Jobdetails holen: Warteschlange, Sicherheitsregeln, Ergebnis-Matrix.
//!
//! Streng nacheinander, nie parallel; jede Beschreibung wird sofort gespeichert. Nach einem
//! Sperrsignal gibt es **keinen** automatischen Ausweichweg – das Portal pausiert.

mod freelance_de;
mod freelancermap;
pub mod http;
mod linkedin;
pub mod policy;
pub mod site;

use std::future::Future;
use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use tokio_util::sync::CancellationToken;

use crate::model::DescStatus;
use crate::portal::{JobKey, JobLink, LoginMode, Portal};
use crate::store::{JobRow, Store};
use crate::time;
use http::HttpFetcher;
use policy::{Allowance, PauseKind, Policy};

/// Ab dieser Länge gilt ein Text ohne Weiteres als vollständig.
const MIN_TEXT_CHARS: usize = 100;
/// Automatisch geholt werden nur Jobs aus Mails der letzten 30 Tage (ältere per Klick).
pub const MAX_AGE: SignedDuration = SignedDuration::from_hours(30 * 24);
/// Ein fehlgeschlagener Abruf wird frühestens nach 12 Stunden wiederholt.
pub const RETRY_AFTER: SignedDuration = SignedDuration::from_hours(12);
/// Nach einem Netzfehler: einmal wiederholen, nach dieser Wartezeit.
const NET_RETRY_DELAY: Duration = Duration::from_secs(30);
/// Längere Wartezeiten meldet der Abruf vorher (die Oberfläche zeigt einen Countdown).
const WAIT_NOTICE: Duration = Duration::from_secs(1);
/// So viele verdächtige Seiten in Folge stoppen ein Portal für den Lauf.
const SUSPICIOUS_STREAK: u32 = 2;
/// Pausengrund nach dem Schutzschalter.
const BREAKER_REASON: &str = "zwei Seiten ohne Beschreibung in Folge – Seitenaufbau geändert?";
/// Reihenfolge der Portale beim Abruf: öffentlich zuerst, die Sitzung zuletzt.
const FETCH_ORDER: [Portal; 3] = [Portal::Freelancermap, Portal::LinkedIn, Portal::FreelanceDe];

/// Strukturierte Angaben einer Seite (verlässlicher als die Mail-Heuristik).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageFields {
    pub title: String,
    pub company: String,
    pub location: String,
}

/// Ergebnis eines Seitenabrufs (Ergebnis-Matrix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageOutcome {
    /// Beschreibung gefunden. `short`: unter 100 Zeichen, aber verifiziert (Container
    /// vorhanden, keine Anmeldewand, richtige Seite).
    Text {
        text: String,
        short: bool,
        closed: bool,
        fields: Option<PageFields>,
    },
    /// Anzeige gibt es nicht mehr.
    Gone,
    /// Seite geladen, aber ohne erkennbare Beschreibung.
    Suspicious(String),
    /// Nur mit (neuer) Anmeldung lesbar; der Text nennt das Anzeichen (fürs Protokoll).
    LoginRequired(String),
    Throttled(String),
    Blocked(String),
    NetError {
        timeout: bool,
        detail: String,
    },
    /// Das Portal leitete einmalig um (nach der Anmeldung) – dieselbe Seite
    /// noch einmal abrufen, als neuer, gezählter Zugriff.
    Retry(String),
    Cancelled,
}

/// Was ein Seitenparser gefunden hat.
#[derive(Debug, Default)]
pub(crate) struct Parsed {
    /// `None`: kein Beschreibungs-Container auf der Seite.
    pub text: Option<String>,
    pub closed: bool,
    pub fields: PageFields,
}

/// Parser-Ergebnis → Ergebnis-Matrix.
pub(crate) fn judge(parsed: Parsed) -> PageOutcome {
    match parsed.text {
        None => {
            PageOutcome::Suspicious("keine Beschreibung gefunden (Seitenaufbau geändert?)".into())
        }
        Some(text) if text.trim().is_empty() => {
            PageOutcome::Suspicious("leere Beschreibung".into())
        }
        Some(text) => {
            let short = text.chars().count() < MIN_TEXT_CHARS;
            let fields = Some(parsed.fields).filter(|f| *f != PageFields::default());
            PageOutcome::Text {
                text,
                short,
                closed: parsed.closed,
                fields,
            }
        }
    }
}

/// Ergebnis einer Anmeldung durch den Nutzer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Login {
    SignedIn,
    /// Angemeldet, aber das Portal zeigte dabei eine Sicherheitsprüfung: Die Sitzung gilt,
    /// das Portal ruht bis zum nächsten Lauf (die App löst nie selbst eine Prüfung).
    Challenged,
    /// Nicht angemeldet (Fenster geschlossen, Zeit abgelaufen, abgebrochen).
    NotSignedIn,
}

/// Auf welchem Weg eine Seite geholt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Ohne Konto, als Gast.
    Http,
    /// Im Sitzungsfenster, mit der Anmeldung des Nutzers.
    Session,
}

/// Der einzige Router. Sitzungsweg, wenn eine Anmeldung nötig ist – oder wenn sie gewollt
/// **und** bestätigt ist. Ein Portal mit optionaler Anmeldung ohne bestätigte Sitzung geht
/// als Gast: Das ist kein Fehler, nur weniger Text.
pub fn route(portal: Portal, session_portals: &[Portal], policy: &Policy) -> Route {
    match portal.login_mode() {
        LoginMode::None => Route::Http,
        LoginMode::Required => Route::Session,
        LoginMode::Optional => {
            let state = policy.state(portal);
            let signed_in = !state.login_needed && state.session_confirmed_at.is_some();
            if session_portals.contains(&portal) && signed_in {
                Route::Session
            } else {
                Route::Http
            }
        }
    }
}

/// Holt eine Seite – als Gast (HTTP) oder im Sitzungsfenster; den Weg bestimmt [`route`].
pub trait PageFetcher {
    fn fetch(
        &mut self,
        link: &JobLink,
        route: Route,
        cancel: &CancellationToken,
    ) -> impl Future<Output = PageOutcome> + Send;

    /// Lässt den Nutzer sich anmelden (Sitzungsfenster sichtbar). Ohne Anmeldemöglichkeit:
    /// nicht angemeldet.
    fn login(
        &mut self,
        portal: Portal,
        cancel: &CancellationToken,
    ) -> impl Future<Output = Login> + Send {
        let _ = (portal, cancel);
        async { Login::NotSignedIn }
    }
}

/// Die beiden Abrufwege der App nebeneinander.
pub struct Fetchers<S> {
    pub http: HttpFetcher,
    pub session: S,
}

impl<S: PageFetcher + Send> PageFetcher for Fetchers<S> {
    async fn fetch(
        &mut self,
        link: &JobLink,
        route: Route,
        cancel: &CancellationToken,
    ) -> PageOutcome {
        match route {
            Route::Session => self.session.fetch(link, route, cancel).await,
            Route::Http => self.http.fetch(link, route, cancel).await,
        }
    }

    async fn login(&mut self, portal: Portal, cancel: &CancellationToken) -> Login {
        self.session.login(portal, cancel).await
    }
}

/// Warum ein Portal in diesem Lauf nicht (weiter) abgerufen wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Pause aus einem früheren Lauf oder gerade verhängt.
    Paused { until: Timestamp, reason: String },
    /// Obergrenze erreicht – der Rest ist ab `next_at` möglich.
    Quota { next_at: Timestamp },
    /// Zwei verdächtige Seiten in Folge (Schutzschalter) – Portal pausiert bis `until`.
    Breaker { until: Timestamp },
    /// Anmeldung nötig und im Lauf nicht zustande gekommen.
    LoginRequired,
    /// Angemeldet, aber mit Sicherheitsprüfung – das Portal ruht bis zum nächsten Lauf.
    Challenged,
    /// Netz gestört (zweiter Fehler nach Wiederholung).
    Network { detail: String },
}

impl StopReason {
    /// Stopptext für Verlauf, Zusammenfassung und Oberfläche – die einzige Quelle.
    pub fn text(&self, portal: Portal, skipped: usize) -> String {
        let label = portal.label();
        let jobs = crate::text::plural(skipped, "Job", "Jobs");
        match self {
            StopReason::Paused { until, reason } => format!(
                "{label}: pausiert bis {} ({reason}) – {jobs} später.",
                time::display(*until)
            ),
            StopReason::Quota { next_at } => format!(
                "{label}: Tagesgrenze/Stundengrenze erreicht – Rest ({skipped}) ab {} möglich.",
                time::display(*next_at)
            ),
            StopReason::Breaker { until } => format!(
                "{label}: zwei Seiten ohne Beschreibung in Folge (Seitenaufbau geändert?) – Pause bis {}, {jobs} später.",
                time::display(*until)
            ),
            StopReason::LoginRequired => format!("{label}: Anmeldung nötig – {jobs} warten."),
            StopReason::Challenged => format!(
                "{label}: Sicherheitsprüfung bei der Anmeldung – bitte später erneut; {jobs} warten bis zum nächsten Lauf."
            ),
            StopReason::Network { detail } => {
                format!("{label}: Netzwerkproblem ({detail}) – {jobs} später.")
            }
        }
    }
}

/// Zähler je Portal.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalCounts {
    pub ok: usize,
    pub short: usize,
    pub closed: usize,
    pub gone: usize,
    pub failed: usize,
    /// Übersprungene bzw. wartende Jobs (Pause, Obergrenze, Anmeldung, Schutzschalter, Netz).
    pub skipped: usize,
    /// Warum das Portal in diesem Lauf stoppte – der fertige Stopptext.
    pub stop: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchSummary {
    pub queued: usize,
    pub per_portal: std::collections::BTreeMap<Portal, PortalCounts>,
}

#[derive(Debug)]
pub enum FetchEvent<'a> {
    Queued {
        total: usize,
    },
    /// Eine Seite des Portals wird gleich abgerufen.
    Fetching {
        portal: Portal,
    },
    /// Die Anmeldung beim Portal beginnt.
    SigningIn {
        portal: Portal,
    },
    /// Bis `until` wird gewartet (Abstand oder Wiederholung nach einem Netzfehler).
    Waiting {
        portal: Portal,
        until: Timestamp,
    },
    /// Ein Job hat einen neuen Stand (Tabelle aktualisieren).
    JobUpdated {
        key: &'a JobKey,
        status: DescStatus,
    },
    PortalStopped {
        portal: Portal,
        reason: &'a StopReason,
        skipped: usize,
        /// Fertiger Stopptext.
        text: &'a str,
    },
    Progress {
        done: usize,
        total: usize,
    },
}

/// Was geholt werden soll.
#[derive(Debug, Clone, Copy)]
pub enum Selection<'a> {
    /// Offenes der gewählten Portale (Mails der letzten 30 Tage).
    Queue(&'a [Portal]),
    /// Genau diese Jobs („Details holen“), auch ältere.
    Jobs(&'a [JobKey]),
}

/// Ergebnis von [`admit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Zugriff gezählt und gesichert – jetzt darf das Portal kontaktiert werden.
    Go,
    /// Pause oder Obergrenze: kein Zugriff.
    Stop(StopReason),
    /// Abgebrochen, bevor etwas gezählt wurde.
    Cancelled,
}

/// Jeder Portalzugriff geht hier durch – Seiten, die Anmeldung im Lauf und An-/Abmelden von
/// Hand: erst Pause und Obergrenzen prüfen (ein pausiertes Portal wartet nicht erst), dann
/// den Abstand abwarten (abbrechbar; `on_wait` erfährt vorher das Ende längerer
/// Wartezeiten), dann zählen und dauerhaft sichern – vor dem Zugriff, damit ein Absturz
/// mittendrin die Obergrenze nicht aushebelt. Ohne gesicherten Stand kein Zugriff.
pub async fn admit(
    policy: &mut Policy,
    portal: Portal,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    on_wait: impl FnOnce(Timestamp),
) -> crate::Result<Admission> {
    if cancel.is_cancelled() {
        return Ok(Admission::Cancelled);
    }
    match policy.allowance(portal, clock()) {
        Allowance::Paused { until, reason } => {
            return Ok(Admission::Stop(StopReason::Paused { until, reason }));
        }
        Allowance::Quota { next_at } => return Ok(Admission::Stop(StopReason::Quota { next_at })),
        Allowance::Go => {}
    }
    if let Some(wait) = policy.pace_wait(portal, clock()) {
        if wait > WAIT_NOTICE {
            on_wait(until(clock(), wait));
        }
        if !sleep_for(wait, cancel).await {
            return Ok(Admission::Cancelled);
        }
    }
    policy.record_access(portal, clock());
    policy.save()?;
    Ok(Admission::Go)
}

/// Holt die Jobdetails. `clock` liefert die aktuelle Zeit (in Tests steuerbar).
///
/// Fehler der Datenbank oder beim Sichern von `policy.json` brechen ab – ohne dauerhaften
/// Sicherheitsstand wird kein Portal weiter abgerufen.
#[expect(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "die Ergebnis-Matrix liest sich am besten an einem Stück; Uhr und Ereignisse kommen einzeln (Tests)"
)]
pub async fn fetch_all<F: PageFetcher>(
    fetcher: &mut F,
    store: &Store,
    policy: &mut Policy,
    selection: Selection<'_>,
    session_portals: &[Portal],
    cancel: &CancellationToken,
    clock: impl Fn() -> Timestamp,
    summary: &mut FetchSummary,
    mut on_event: impl FnMut(FetchEvent<'_>),
) -> crate::Result<bool> {
    let queue = queue(store, selection, clock())?;
    summary.queued = queue.len();
    on_event(FetchEvent::Queued { total: queue.len() });
    let mut done = 0;

    for portal in FETCH_ORDER {
        let jobs: Vec<&JobRow> = queue.iter().filter(|j| j.key.portal == portal).collect();
        if jobs.is_empty() {
            continue;
        }
        let counts = summary.per_portal.entry(portal).or_default();
        let mut route = route(portal, session_portals, policy);
        // Eine Anmeldung je Portal und Lauf; danach wird derselbe Job erneut versucht.
        let mut login_tried = false;
        // Eine optionale Anmeldung geht höchstens einmal je Lauf verloren – danach wäre es
        // keine verlorene Sitzung mehr, sondern eine Anmeldewand auch für Gäste.
        let mut fell_back = false;
        // Job, dessen Seite schon einmal wiederholt wurde (höchstens eine Wiederholung).
        let mut retried: Option<usize> = None;
        let mut index = 0;
        while index < jobs.len() {
            let job = jobs[index];
            let remaining = jobs.len() - index;
            let link = JobLink {
                key: job.key.clone(),
                url: job.url.clone(),
            };
            let mut outcome =
                match access(fetcher, policy, &link, route, cancel, &clock, &mut on_event).await? {
                    Ok(outcome) => outcome,
                    Err(reason) => {
                        stop(counts, &reason, remaining, portal, &mut on_event);
                        break;
                    }
                };
            if let PageOutcome::NetError { .. } = outcome {
                // Einmal wiederholen – nach 30 s, wieder mit allen Regeln.
                on_event(FetchEvent::Waiting {
                    portal,
                    until: until(clock(), NET_RETRY_DELAY),
                });
                if !sleep_for(NET_RETRY_DELAY, cancel).await {
                    return Ok(false);
                }
                outcome = match access(fetcher, policy, &link, route, cancel, &clock, &mut on_event)
                    .await?
                {
                    Ok(PageOutcome::NetError { timeout: true, .. }) => {
                        PageOutcome::Throttled("zweimal keine Antwort".into())
                    }
                    Ok(outcome) => outcome,
                    Err(reason) => {
                        stop(counts, &reason, remaining, portal, &mut on_event);
                        break;
                    }
                };
            }
            // Leitet dieselbe Seite ein zweites Mal um, stimmt etwas nicht: verdächtig, damit
            // der Schutzschalter greift.
            let outcome = match outcome {
                PageOutcome::Retry(reason) if retried == Some(index) => {
                    PageOutcome::Suspicious(reason)
                }
                other => other,
            };

            let now = clock();
            policy.record_done(portal, now);
            let stop_reason = match outcome {
                PageOutcome::Cancelled => {
                    // Die Antwortzeit gilt auch nach einem Abbruch für den nächsten Abstand.
                    policy.save()?;
                    return Ok(false);
                }
                PageOutcome::Retry(_) => {
                    retried = Some(index);
                    policy.save()?;
                    continue;
                }
                PageOutcome::Text {
                    text,
                    short,
                    closed,
                    fields,
                } => {
                    store.record_text(&job.key, &text, short, closed, now)?;
                    // Nur nicht-leere Felder überschreiben die Mail-Heuristik: Was die Seite
                    // verbirgt („für EXPERT-Mitglieder sichtbar“), kommt leer an.
                    if let Some(f) = fields {
                        store.record_page_fields(&job.key, &f.title, &f.company, &f.location)?;
                    }
                    // Eine gelesene Seite im Sitzungsfenster bestätigt die Anmeldung; über den
                    // Gastweg sagt sie darüber nichts.
                    if route == Route::Session {
                        policy.set_session(portal, true, now);
                    }
                    counts.ok += 1;
                    counts.short += usize::from(short);
                    counts.closed += usize::from(closed);
                    // Nur ein zweifelsfrei vollständiger Text setzt den Schutzschalter zurück.
                    if !short {
                        policy.clear_suspicious(portal);
                    }
                    on_event(FetchEvent::JobUpdated {
                        key: &job.key,
                        status: DescStatus::Ok,
                    });
                    None
                }
                PageOutcome::Gone => {
                    store.record_gone(&job.key, now)?;
                    counts.gone += 1;
                    on_event(FetchEvent::JobUpdated {
                        key: &job.key,
                        status: DescStatus::Gone,
                    });
                    None
                }
                PageOutcome::Suspicious(reason) => {
                    let status = store.record_failed(&job.key, &reason, now)?;
                    counts.failed += 1;
                    on_event(FetchEvent::JobUpdated {
                        key: &job.key,
                        status,
                    });
                    // Zwei in Folge – auch über Läufe hinweg – stoppen das Portal und
                    // pausieren es eine Stunde: vermutlich hat sich der Seitenaufbau geändert.
                    (policy.count_suspicious(portal) >= SUSPICIOUS_STREAK).then(|| {
                        StopReason::Breaker {
                            until: policy.pause(portal, PauseKind::Throttled, BREAKER_REASON, now),
                        }
                    })
                }
                PageOutcome::LoginRequired(sign) => {
                    log::info!("{portal}: Anmeldung nötig ({sign})");
                    policy.set_session(portal, false, now);
                    policy.save()?;
                    // Optionale Anmeldung verloren: still zurück auf den Gastweg – keine
                    // Pause, kein Fehlversuch, derselbe Job wird gleich als Gast geholt.
                    if portal.login_mode() == LoginMode::Optional && !fell_back {
                        log::info!("{portal}: ohne Anmeldung weiter (Gastweg)");
                        fell_back = true;
                        route = Route::Http;
                        continue;
                    }
                    if login_tried {
                        Some(StopReason::LoginRequired)
                    } else {
                        login_tried = true;
                        // Die Anmeldeseite ist ein Portalzugriff wie jeder andere.
                        let admission = admit(policy, portal, cancel, &clock, |until| {
                            on_event(FetchEvent::Waiting { portal, until });
                        })
                        .await?;
                        match admission {
                            Admission::Stop(reason) => Some(reason),
                            Admission::Cancelled => {
                                policy.save()?;
                                return Ok(false);
                            }
                            Admission::Go => {
                                on_event(FetchEvent::SigningIn { portal });
                                let login = fetcher.login(portal, cancel).await;
                                // Auch nach der Anmeldung gilt der Abstand zur nächsten Seite.
                                policy.record_done(portal, clock());
                                match login {
                                    Login::SignedIn => {
                                        policy.set_session(portal, true, clock());
                                        policy.save()?;
                                        continue;
                                    }
                                    Login::Challenged => {
                                        policy.set_session(portal, true, clock());
                                        Some(StopReason::Challenged)
                                    }
                                    Login::NotSignedIn if cancel.is_cancelled() => {
                                        policy.save()?;
                                        return Ok(false);
                                    }
                                    Login::NotSignedIn => Some(StopReason::LoginRequired),
                                }
                            }
                        }
                    }
                }
                PageOutcome::Throttled(reason) => {
                    let until = policy.pause(portal, PauseKind::Throttled, &reason, now);
                    Some(StopReason::Paused { until, reason })
                }
                PageOutcome::Blocked(reason) => {
                    let until = policy.pause(portal, PauseKind::Blocked, &reason, now);
                    Some(StopReason::Paused { until, reason })
                }
                PageOutcome::NetError { detail, .. } => Some(StopReason::Network { detail }),
            };
            policy.save()?;
            done += 1;
            on_event(FetchEvent::Progress {
                done,
                total: queue.len(),
            });
            if let Some(reason) = stop_reason {
                // Der aktuelle Job zählt mit, wenn er nicht bewertet wurde.
                let skipped = if matches!(reason, StopReason::Breaker { .. }) {
                    remaining - 1
                } else {
                    remaining
                };
                stop(counts, &reason, skipped, portal, &mut on_event);
                break;
            }
            index += 1;
        }
    }
    Ok(true)
}

/// Offene Jobs: automatisch nur aus den letzten 30 Tagen; gezielt gewählte Jobs auch
/// älter – nie aber ein schon erfolgreich geholter (Erfolgreiches wird nie erneut geholt).
fn queue(store: &Store, selection: Selection<'_>, now: Timestamp) -> crate::Result<Vec<JobRow>> {
    Ok(match selection {
        Selection::Queue(portals) => store
            .fetch_queue(now, MAX_AGE, RETRY_AFTER)?
            .into_iter()
            .filter(|j| portals.contains(&j.key.portal))
            .collect(),
        Selection::Jobs(keys) => {
            let mut jobs = Vec::new();
            for key in keys {
                if let Some(job) = store.job(key)?
                    && job.desc_status != DescStatus::Ok
                    && !jobs.iter().any(|j: &JobRow| j.key == job.key)
                {
                    jobs.push(job);
                }
            }
            jobs
        }
    })
}

/// Eine Seite mit allen Regeln abrufen. `Err` = Portal darf gerade nicht; ein Abbruch vor
/// dem Zugriff kommt als `PageOutcome::Cancelled`.
async fn access<F: PageFetcher>(
    fetcher: &mut F,
    policy: &mut Policy,
    link: &JobLink,
    route: Route,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    on_event: &mut impl FnMut(FetchEvent<'_>),
) -> crate::Result<Result<PageOutcome, StopReason>> {
    let portal = link.key.portal;
    let admission = admit(policy, portal, cancel, clock, |until| {
        on_event(FetchEvent::Waiting { portal, until });
    })
    .await?;
    match admission {
        Admission::Go => {
            on_event(FetchEvent::Fetching { portal });
            Ok(Ok(fetcher.fetch(link, route, cancel).await))
        }
        Admission::Stop(reason) => Ok(Err(reason)),
        Admission::Cancelled => Ok(Ok(PageOutcome::Cancelled)),
    }
}

fn stop(
    counts: &mut PortalCounts,
    reason: &StopReason,
    skipped: usize,
    portal: Portal,
    on_event: &mut impl FnMut(FetchEvent<'_>),
) {
    let text = reason.text(portal, skipped);
    counts.skipped += skipped;
    on_event(FetchEvent::PortalStopped {
        portal,
        reason,
        skipped,
        text: &text,
    });
    counts.stop = Some(text);
}

/// Ende einer Wartezeit ab `now`.
fn until(now: Timestamp, wait: Duration) -> Timestamp {
    SignedDuration::try_from(wait)
        .ok()
        .and_then(|wait| now.checked_add(wait).ok())
        .unwrap_or(Timestamp::MAX)
}

/// Wartet `wait`; `false`, wenn vorher abgebrochen wurde.
async fn sleep_for(wait: Duration, cancel: &CancellationToken) -> bool {
    tokio::select! {
        biased;
        () = cancel.cancelled() => false,
        () = tokio::time::sleep(wait) => true,
    }
}

#[cfg(test)]
mod tests;
