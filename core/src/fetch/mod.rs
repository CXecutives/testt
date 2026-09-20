//! Jobdetails holen: Warteschlange, Sicherheitsregeln, Ergebnis-Matrix.
//!
//! Die drei Portale laufen nebeneinander – **innerhalb** eines Portals aber streng
//! nacheinander: gleiche Abstände, gleiche Obergrenzen, gleicher Schutzschalter. Das ist
//! eine Sicherheitseigenschaft, keine Beschleunigung. Jede Beschreibung wird sofort
//! gespeichert. Nach einem Sperrsignal gibt es **keinen** automatischen Ausweichweg – das
//! Portal pausiert.
//!
//! Der Sicherheitsstand liegt hinter einer kurzen Sperre; sie wird nie über einen Schlaf
//! oder einen Abruf gehalten, und `policy.json` hat dadurch genau einen Schreiber.

mod freelance_de;
mod freelancermap;
pub mod http;
mod linkedin;
pub mod policy;
pub mod site;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::mpsc;

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

/// Der einzige Router: Sitzungsfenster genau dann, wenn das Portal ohne Anmeldung nichts
/// hergibt. Alles andere geht als Gast.
pub fn route(portal: Portal) -> Route {
    match portal.login_mode() {
        LoginMode::None => Route::Http,
        LoginMode::Required => Route::Session,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchEvent {
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
        key: JobKey,
        status: DescStatus,
    },
    PortalStopped {
        portal: Portal,
        reason: StopReason,
        skipped: usize,
        /// Fertiger Stopptext.
        text: String,
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
    policy: &Mutex<Policy>,
    portal: Portal,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    on_wait: impl FnOnce(Timestamp),
) -> crate::Result<Admission> {
    if cancel.is_cancelled() {
        return Ok(Admission::Cancelled);
    }
    // Die Sperre gilt nur für das Prüfen und Rechnen, nie über den Schlaf.
    let wait = {
        let policy = lock(policy);
        match policy.allowance(portal, clock()) {
            Allowance::Paused { until, reason } => {
                return Ok(Admission::Stop(StopReason::Paused { until, reason }));
            }
            Allowance::Quota { next_at } => {
                return Ok(Admission::Stop(StopReason::Quota { next_at }));
            }
            Allowance::Go => {}
        }
        policy.pace_wait(portal, clock())
    };
    if let Some(wait) = wait {
        if wait > WAIT_NOTICE {
            on_wait(until(clock(), wait));
        }
        if !sleep_for(wait, cancel).await {
            return Ok(Admission::Cancelled);
        }
    }
    let mut policy = lock(policy);
    policy.record_access(portal, clock());
    policy.save()?;
    Ok(Admission::Go)
}

/// Kurzer Zugriff auf den Sicherheitsstand. Ein vergifteter Stand ist kein Grund, den Lauf
/// abzubrechen: Die Zähler darin sind gültig, und ohne sie gäbe es gar keine Grenze mehr.
fn lock(policy: &Mutex<Policy>) -> MutexGuard<'_, Policy> {
    policy.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Was alle Portal-Schleifen teilen.
struct Shared<'a, C: Fn() -> Timestamp> {
    store: &'a Store,
    policy: &'a Mutex<Policy>,
    /// Abbruch **dieses** Abrufs – auch ein Datenbankfehler in einem Portal stoppt so die
    /// übrigen, statt sie ohne gesicherten Stand weiterlaufen zu lassen.
    cancel: &'a CancellationToken,
    clock: &'a C,
}

/// Was eine Portal-Schleife hinterlässt.
struct PortalRun {
    portal: Portal,
    /// `None`: Das Portal hatte in diesem Lauf nichts zu holen.
    counts: Option<PortalCounts>,
    completed: bool,
}

/// Meldung einer Portal-Schleife an den einen Ereignis-Sammler.
enum Note {
    Event(FetchEvent),
    /// Ein Job ist bewertet – der Fortschritt zählt über alle Portale zusammen.
    Done,
}

fn note(notes: &mpsc::UnboundedSender<Note>, event: FetchEvent) {
    let _ = notes.send(Note::Event(event));
}

/// Holt die Jobdetails. `pages` liefert den Abrufweg eines Portals – jedes Portal bekommt
/// seinen eigenen (eigene HTTP-Sitzung, eigenes Fenster), damit die Portale nebeneinander
/// laufen können. `clock` liefert die aktuelle Zeit (in Tests steuerbar).
///
/// Fehler der Datenbank oder beim Sichern von `policy.json` brechen ab – ohne dauerhaften
/// Sicherheitsstand wird kein Portal weiter abgerufen.
#[expect(
    clippy::too_many_arguments,
    reason = "Lauf-Kontext: Speicher, Regeln, Auswahl, Uhr und Ereignisse kommen einzeln (Tests)"
)]
pub async fn fetch_all<F: PageFetcher>(
    mut pages: impl FnMut(Portal) -> Result<F, String>,
    store: &Store,
    policy: &Mutex<Policy>,
    selection: Selection<'_>,
    cancel: &CancellationToken,
    clock: impl Fn() -> Timestamp,
    summary: &mut FetchSummary,
    mut on_event: impl FnMut(FetchEvent),
) -> crate::Result<bool> {
    let queue = queue(store, selection, clock())?;
    let total = queue.len();
    summary.queued = total;
    on_event(FetchEvent::Queued { total });

    let mut by_portal: BTreeMap<Portal, Vec<JobRow>> = BTreeMap::new();
    for job in queue {
        by_portal.entry(job.key.portal).or_default().push(job);
    }
    // Der Abrufweg entsteht vor dem Start: Scheitert er, beginnt kein Portal.
    let mut prepare = |portal: Portal| -> crate::Result<(Portal, Vec<JobRow>, Option<F>)> {
        let jobs = by_portal.remove(&portal).unwrap_or_default();
        let fetcher = if jobs.is_empty() {
            None
        } else {
            Some(
                pages(portal)
                    .map_err(|e| crate::Error::Invalid(format!("Abruf nicht möglich: {e}")))?,
            )
        };
        Ok((portal, jobs, fetcher))
    };
    let first = prepare(FETCH_ORDER[0])?;
    let second = prepare(FETCH_ORDER[1])?;
    let third = prepare(FETCH_ORDER[2])?;

    // Eigenes Abbruch-Signal: Der Nutzer bricht über das übergebene ab, ein Fehler in einem
    // Portal über dieses.
    let inner = cancel.child_token();
    let shared = Shared {
        store,
        policy,
        cancel: &inner,
        clock: &clock,
    };
    let (notes, mut incoming) = mpsc::unbounded_channel::<Note>();
    // Je Schleife ein Absender; sind alle fertig, endet der Sammler von selbst.
    let (n1, n2, n3) = (notes.clone(), notes.clone(), notes);
    let (a, b, c, ()) = tokio::join!(
        fetch_portal(first, &shared, n1),
        fetch_portal(second, &shared, n2),
        fetch_portal(third, &shared, n3),
        async {
            let mut done = 0;
            while let Some(note) = incoming.recv().await {
                match note {
                    Note::Event(event) => on_event(event),
                    Note::Done => {
                        done += 1;
                        on_event(FetchEvent::Progress { done, total });
                    }
                }
            }
        }
    );

    let mut completed = true;
    for result in [a, b, c] {
        let run = result?;
        completed &= run.completed;
        if let Some(counts) = run.counts {
            summary.per_portal.insert(run.portal, counts);
        }
    }
    Ok(completed)
}

/// Eine Portal-Schleife: streng nacheinander, mit allen Regeln. Ein Fehler stoppt auch die
/// übrigen Portale – ohne gesicherten Sicherheitsstand ruft niemand weiter ab.
async fn fetch_portal<F: PageFetcher, C: Fn() -> Timestamp>(
    work: (Portal, Vec<JobRow>, Option<F>),
    shared: &Shared<'_, C>,
    notes: mpsc::UnboundedSender<Note>,
) -> crate::Result<PortalRun> {
    let portal = work.0;
    let result = portal_loop(work, shared, &notes).await;
    if result.is_err() {
        shared.cancel.cancel();
    }
    result.map(|(counts, completed)| PortalRun {
        portal,
        counts,
        completed,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "die Ergebnis-Matrix liest sich am besten an einem Stück"
)]
async fn portal_loop<F: PageFetcher, C: Fn() -> Timestamp>(
    (portal, jobs, fetcher): (Portal, Vec<JobRow>, Option<F>),
    shared: &Shared<'_, C>,
    notes: &mpsc::UnboundedSender<Note>,
) -> crate::Result<(Option<PortalCounts>, bool)> {
    let Some(mut fetcher) = fetcher else {
        return Ok((None, true));
    };
    let (store, policy, cancel, clock) = (shared.store, shared.policy, shared.cancel, shared.clock);
    let mut counts = PortalCounts::default();
    let route = route(portal);
    // Eine Anmeldung je Portal und Lauf; danach wird derselbe Job erneut versucht.
    let mut login_tried = false;
    // Eine optionale Anmeldung geht höchstens einmal je Lauf verloren – danach wäre es
    // keine verlorene Sitzung mehr, sondern eine Anmeldewand auch für Gäste.
    // Job, dessen Seite schon einmal wiederholt wurde (höchstens eine Wiederholung).
    let mut retried: Option<usize> = None;
    let mut index = 0;
    while index < jobs.len() {
        let job = &jobs[index];
        let remaining = jobs.len() - index;
        let link = JobLink {
            key: job.key.clone(),
            url: job.url.clone(),
        };
        let mut outcome =
            match access(&mut fetcher, policy, &link, route, cancel, clock, notes).await? {
                Ok(outcome) => outcome,
                Err(reason) => {
                    stop(&mut counts, &reason, remaining, portal, notes);
                    break;
                }
            };
        if let PageOutcome::NetError { .. } = outcome {
            // Einmal wiederholen – nach 30 s, wieder mit allen Regeln.
            note(
                notes,
                FetchEvent::Waiting {
                    portal,
                    until: until(clock(), NET_RETRY_DELAY),
                },
            );
            if !sleep_for(NET_RETRY_DELAY, cancel).await {
                return Ok((Some(counts), false));
            }
            outcome = match access(&mut fetcher, policy, &link, route, cancel, clock, notes).await?
            {
                Ok(PageOutcome::NetError { timeout: true, .. }) => {
                    PageOutcome::Throttled("zweimal keine Antwort".into())
                }
                Ok(outcome) => outcome,
                Err(reason) => {
                    stop(&mut counts, &reason, remaining, portal, notes);
                    break;
                }
            };
        }
        // Leitet dieselbe Seite ein zweites Mal um, stimmt etwas nicht: verdächtig, damit
        // der Schutzschalter greift.
        let outcome = match outcome {
            PageOutcome::Retry(reason) if retried == Some(index) => PageOutcome::Suspicious(reason),
            other => other,
        };

        let now = clock();
        lock(policy).record_done(portal, now);
        let stop_reason = match outcome {
            PageOutcome::Cancelled => {
                // Die Antwortzeit gilt auch nach einem Abbruch für den nächsten Abstand.
                lock(policy).save()?;
                return Ok((Some(counts), false));
            }
            PageOutcome::Retry(_) => {
                retried = Some(index);
                lock(policy).save()?;
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
                {
                    let mut policy = lock(policy);
                    // Eine gelesene Seite im Sitzungsfenster bestätigt die Anmeldung; über
                    // den Gastweg sagt sie darüber nichts.
                    if route == Route::Session {
                        policy.set_session(portal, true, now);
                    }
                    // Nur ein zweifelsfrei vollständiger Text setzt den Schutzschalter zurück.
                    if !short {
                        policy.clear_suspicious(portal);
                    }
                }
                counts.ok += 1;
                counts.short += usize::from(short);
                counts.closed += usize::from(closed);
                note(
                    notes,
                    FetchEvent::JobUpdated {
                        key: job.key.clone(),
                        status: DescStatus::Ok,
                    },
                );
                None
            }
            PageOutcome::Gone => {
                store.record_gone(&job.key, now)?;
                counts.gone += 1;
                note(
                    notes,
                    FetchEvent::JobUpdated {
                        key: job.key.clone(),
                        status: DescStatus::Gone,
                    },
                );
                None
            }
            PageOutcome::Suspicious(reason) => {
                let status = store.record_failed(&job.key, &reason, now)?;
                counts.failed += 1;
                note(
                    notes,
                    FetchEvent::JobUpdated {
                        key: job.key.clone(),
                        status,
                    },
                );
                // Zwei in Folge – auch über Läufe hinweg – stoppen das Portal und
                // pausieren es eine Stunde: vermutlich hat sich der Seitenaufbau geändert.
                let mut policy = lock(policy);
                (policy.count_suspicious(portal) >= SUSPICIOUS_STREAK).then(|| {
                    StopReason::Breaker {
                        until: policy.pause(portal, PauseKind::Throttled, BREAKER_REASON, now),
                    }
                })
            }
            PageOutcome::LoginRequired(sign) => {
                log::info!("{portal}: Anmeldung nötig ({sign})");
                {
                    let mut policy = lock(policy);
                    policy.set_session(portal, false, now);
                    policy.save()?;
                }
                if login_tried {
                    Some(StopReason::LoginRequired)
                } else {
                    login_tried = true;
                    // Die Anmeldeseite ist ein Portalzugriff wie jeder andere.
                    let admission = admit(policy, portal, cancel, clock, |until| {
                        note(notes, FetchEvent::Waiting { portal, until });
                    })
                    .await?;
                    match admission {
                        Admission::Stop(reason) => Some(reason),
                        Admission::Cancelled => {
                            lock(policy).save()?;
                            return Ok((Some(counts), false));
                        }
                        Admission::Go => {
                            note(notes, FetchEvent::SigningIn { portal });
                            let login = fetcher.login(portal, cancel).await;
                            // Auch nach der Anmeldung gilt der Abstand zur nächsten Seite.
                            lock(policy).record_done(portal, clock());
                            match login {
                                Login::SignedIn => {
                                    let mut policy = lock(policy);
                                    policy.set_session(portal, true, clock());
                                    policy.save()?;
                                    continue;
                                }
                                Login::Challenged => {
                                    lock(policy).set_session(portal, true, clock());
                                    Some(StopReason::Challenged)
                                }
                                Login::NotSignedIn if cancel.is_cancelled() => {
                                    lock(policy).save()?;
                                    return Ok((Some(counts), false));
                                }
                                Login::NotSignedIn => Some(StopReason::LoginRequired),
                            }
                        }
                    }
                }
            }
            PageOutcome::Throttled(reason) => {
                let until = lock(policy).pause(portal, PauseKind::Throttled, &reason, now);
                Some(StopReason::Paused { until, reason })
            }
            PageOutcome::Blocked(reason) => {
                let until = lock(policy).pause(portal, PauseKind::Blocked, &reason, now);
                Some(StopReason::Paused { until, reason })
            }
            PageOutcome::NetError { detail, .. } => Some(StopReason::Network { detail }),
        };
        lock(policy).save()?;
        let _ = notes.send(Note::Done);
        if let Some(reason) = stop_reason {
            // Der aktuelle Job zählt mit, wenn er nicht bewertet wurde.
            let skipped = if matches!(reason, StopReason::Breaker { .. }) {
                remaining - 1
            } else {
                remaining
            };
            stop(&mut counts, &reason, skipped, portal, notes);
            break;
        }
        index += 1;
    }
    Ok((Some(counts), true))
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
    policy: &Mutex<Policy>,
    link: &JobLink,
    route: Route,
    cancel: &CancellationToken,
    clock: &impl Fn() -> Timestamp,
    notes: &mpsc::UnboundedSender<Note>,
) -> crate::Result<Result<PageOutcome, StopReason>> {
    let portal = link.key.portal;
    let admission = admit(policy, portal, cancel, clock, |until| {
        note(notes, FetchEvent::Waiting { portal, until });
    })
    .await?;
    match admission {
        Admission::Go => {
            note(notes, FetchEvent::Fetching { portal });
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
    notes: &mpsc::UnboundedSender<Note>,
) {
    let text = reason.text(portal, skipped);
    counts.skipped += skipped;
    note(
        notes,
        FetchEvent::PortalStopped {
            portal,
            reason: reason.clone(),
            skipped,
            text: text.clone(),
        },
    );
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
