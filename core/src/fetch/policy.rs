//! Sicherheitsregeln für Portalzugriffe – Tempo, Obergrenzen, Pausen – und ihr dauerhafter
//! Stand in `policy.json`.
//!
//! Prinzip: unauffällig durch Zurückhaltung, nicht durch Tarnung. Gezählt wird **jeder**
//! Zugriff (auch Fehlschläge und Login-Seiten); erneutes Klicken umgeht nichts. Die Datei
//! liegt neben der Datenbank und überlebt „Ergebnisordner leeren“ und „Alles zurücksetzen“ –
//! eine Sperrpause darf sich nicht wegklicken lassen. Sie enthält nur Portalnamen und
//! Zeitstempel.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::export::write_atomic;
use crate::portal::Portal;

/// Tempo und Obergrenzen eines Portals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Abstand zwischen zwei Abrufen in Millisekunden (zufällig im Bereich).
    pub pace_ms: RangeInclusive<u64>,
    pub per_hour: usize,
    pub per_day: usize,
}

/// Die Regeln an einer Stelle.
pub fn limits(portal: Portal) -> Limits {
    match portal {
        Portal::LinkedIn => Limits {
            pace_ms: 4_000..=7_000,
            per_hour: 20,
            per_day: 40,
        },
        Portal::Freelancermap => Limits {
            pace_ms: 3_000..=5_000,
            per_hour: 25,
            per_day: 60,
        },
        // Dazu kommt die Verweildauer im Sitzungsfenster (`DWELL_SECS`). Der Abstand hier
        // gilt auch über Läufe, Abbrüche und Neustarts hinweg.
        Portal::FreelanceDe => Limits {
            pace_ms: 10_000..=20_000,
            per_hour: 15,
            per_day: 30,
        },
    }
}

/// freelance.de-Sitzungsfenster: Verweildauer je Seite (ab „fertig geladen“) wie beim
/// Lesen – zusätzlich zum Abstand, bewusst doppelte Zurückhaltung.
pub const DWELL_SECS: RangeInclusive<u64> = 8..=20;

/// Pause nach Drosselung (429, Serverfehler, zweite Zeitüberschreitung).
const THROTTLE_PAUSE: SignedDuration = SignedDuration::from_hours(1);
/// Pause nach einem Sperrsignal (999, 403, Umleitung zur Anmeldung, Captcha).
const BLOCK_PAUSE: SignedDuration = SignedDuration::from_hours(24);
/// Pause nach dem zweiten Sperrsignal innerhalb von [`REPEAT_WINDOW`] – zugleich die
/// längste Pause überhaupt.
const REPEAT_BLOCK_PAUSE: SignedDuration = SignedDuration::from_hours(7 * 24);
const REPEAT_WINDOW: SignedDuration = SignedDuration::from_hours(7 * 24);

const HOUR: SignedDuration = SignedDuration::from_hours(1);
const DAY: SignedDuration = SignedDuration::from_hours(24);

/// Warum ein Portal pausiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PauseKind {
    Throttled,
    Blocked,
}

/// Stand eines Portals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalState {
    pub paused_until: Option<Timestamp>,
    pub pause_kind: Option<PauseKind>,
    /// Klartext-Grund der letzten Pause (für die Portal-Ansicht).
    pub pause_reason: Option<String>,
    /// Ende der letzten Sperrpause – ein neues Sperrsignal binnen sieben Tagen danach gilt
    /// als Wiederholung (sonst begänne nach jeder 7-Tage-Pause wieder die kurze).
    pub block_until: Option<Timestamp>,
    /// Seiten ohne Beschreibung in Folge – über Läufe hinweg (erneutes Klicken umgeht den
    /// Schutzschalter nicht).
    pub suspicious_streak: u32,
    /// Zugriffe der letzten 24 Stunden.
    pub accesses: Vec<Timestamp>,
    /// Ende des letzten Abrufs – der Abstand zählt ab der Antwort, auch über Läufe hinweg.
    pub last_done_at: Option<Timestamp>,
    /// freelance.de: zuletzt bestätigte Sitzung (erste Jobseite mit Abmelde-Link).
    pub session_confirmed_at: Option<Timestamp>,
    /// freelance.de: Anmeldung nötig (Sitzung abgelaufen oder nie angemeldet).
    pub login_needed: bool,
}

/// Darf das Portal jetzt abgerufen werden?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allowance {
    Go,
    Paused {
        until: Timestamp,
        reason: String,
    },
    /// Obergrenze erreicht; der nächste Zugriff ist ab `next_at` möglich.
    Quota {
        next_at: Timestamp,
    },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Policy {
    #[serde(skip)]
    path: Option<PathBuf>,
    #[serde(default)]
    portals: BTreeMap<Portal, PortalState>,
}

impl Policy {
    /// Liest `policy.json`. Fehlt sie, beginnt ein leerer Stand. Ist sie unlesbar, gilt
    /// sicherheitshalber jedes Portal für 24 Stunden als pausiert – eine beschädigte Datei
    /// darf keine Sperrpause aufheben.
    pub fn load(path: &Path, now: Timestamp) -> Policy {
        let mut policy = match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Policy>(&bytes) {
                Ok(mut policy) => {
                    policy.clamp_future(now);
                    policy
                }
                Err(e) => {
                    // Eine Kopie zur Ansicht ablegen und die Pause über die kaputte Datei
                    // schreiben – sonst begänne sie bei jedem Laden neu und endete nie.
                    // Scheitert das Schreiben, bleibt die kaputte Datei: nächstes Laden
                    // pausiert wieder (nie ein Stand ohne Pause).
                    log::warn!("policy.json unlesbar ({e}) – alle Portale 24 h pausiert");
                    let aside = path.with_extension(format!("json.bad-{}", now.as_second()));
                    if let Err(e) = std::fs::copy(path, &aside) {
                        log::warn!("policy.json nicht kopiert: {e}");
                    }
                    let mut paused = Policy::all_paused(now);
                    paused.path = Some(path.to_path_buf());
                    if let Err(e) = paused.save() {
                        log::warn!("Sicherheitsstand nicht gespeichert: {e}");
                    }
                    paused
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Policy::default(),
            Err(e) => {
                // Nur gerade nicht lesbar (z. B. gesperrt): pausieren, aber nie speichern –
                // sonst ersetzte dieser Notstand die echte Datei samt längerer Sperren.
                log::warn!("policy.json nicht lesbar ({e}) – alle Portale 24 h pausiert");
                return Policy::all_paused(now);
            }
        };
        policy.path = Some(path.to_path_buf());
        policy
    }

    /// Stand nur im Arbeitsspeicher (Trockenlauf, Tests): `save` schreibt nichts.
    pub fn in_memory() -> Policy {
        Policy::default()
    }

    /// Zeitstempel aus einer falsch gehenden Uhr kappen: Kein Zugriff und keine Antwort
    /// liegt nach „jetzt“, keine Pause reicht weiter als die längste (sieben Tage). Sonst
    /// sperrte ein einmal vorgestelltes Datum ein Portal dauerhaft – `policy.json` überlebt
    /// ja auch „Alles zurücksetzen“.
    fn clamp_future(&mut self, now: Timestamp) {
        let latest_pause_end = now
            .saturating_add(REPEAT_BLOCK_PAUSE)
            .unwrap_or(Timestamp::MAX);
        for state in self.portals.values_mut() {
            for at in &mut state.accesses {
                *at = (*at).min(now);
            }
            for at in [&mut state.last_done_at, &mut state.session_confirmed_at] {
                *at = at.map(|t| t.min(now));
            }
            for until in [&mut state.paused_until, &mut state.block_until] {
                *until = until.map(|t| t.min(latest_pause_end));
            }
        }
    }

    fn all_paused(now: Timestamp) -> Policy {
        let mut policy = Policy::default();
        for portal in Portal::ALL {
            let state = policy.portals.entry(portal).or_default();
            state.paused_until = now.checked_add(DAY).ok();
            state.pause_kind = Some(PauseKind::Blocked);
            state.pause_reason = Some("Sicherheitsstand war unlesbar".into());
        }
        policy
    }

    /// Schreibt den Stand atomar (kein Teilstand nach einem Absturz).
    pub fn save(&self) -> crate::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let json = serde_json::to_vec_pretty(self).expect("Policy ist immer serialisierbar");
        write_atomic(path, &json)
    }

    pub fn state(&self, portal: Portal) -> PortalState {
        self.portals.get(&portal).cloned().unwrap_or_default()
    }

    /// Pause, Stundengrenze, Tagesgrenze – in dieser Reihenfolge.
    pub fn allowance(&self, portal: Portal, now: Timestamp) -> Allowance {
        let state = self.portals.get(&portal);
        if let Some(state) = state
            && let Some(until) = state.paused_until.filter(|&until| until > now)
        {
            return Allowance::Paused {
                until,
                reason: state.pause_reason.clone().unwrap_or_default(),
            };
        }
        let accesses = state.map_or(&[][..], |s| &s.accesses[..]);
        let limits = limits(portal);
        let next = [(HOUR, limits.per_hour), (DAY, limits.per_day)]
            .into_iter()
            .filter_map(|(window, cap)| quota_free_at(accesses, now, window, cap))
            .max();
        match next {
            Some(next_at) => Allowance::Quota { next_at },
            None => Allowance::Go,
        }
    }

    /// Zählt einen Zugriff und vergisst Zugriffe, die älter als 24 Stunden sind.
    pub fn record_access(&mut self, portal: Portal, now: Timestamp) {
        let state = self.portals.entry(portal).or_default();
        let horizon = now.saturating_sub(DAY).unwrap_or(Timestamp::MIN);
        state.accesses.retain(|&at| at > horizon);
        state.accesses.push(now);
    }

    /// Pausiert ein Portal und liefert das Ende der Pause. Ein zweites Sperrsignal binnen
    /// sieben Tagen verlängert auf sieben Tage; eine laufende längere Pause wird nie
    /// verkürzt.
    pub fn pause(
        &mut self,
        portal: Portal,
        kind: PauseKind,
        reason: &str,
        now: Timestamp,
    ) -> Timestamp {
        let state = self.portals.entry(portal).or_default();
        let length = match kind {
            PauseKind::Throttled => THROTTLE_PAUSE,
            PauseKind::Blocked => {
                // Gezählt ab dem Ende der letzten Sperre (sie begann immer davor).
                let repeat = state
                    .block_until
                    .is_some_and(|end| now.duration_since(end) <= REPEAT_WINDOW);
                if repeat {
                    REPEAT_BLOCK_PAUSE
                } else {
                    BLOCK_PAUSE
                }
            }
        };
        let until = now.saturating_add(length).unwrap_or(Timestamp::MAX);
        if kind == PauseKind::Blocked {
            state.block_until = Some(until);
        }
        if state.paused_until.is_none_or(|current| current < until) {
            state.paused_until = Some(until);
            state.pause_kind = Some(kind);
            state.pause_reason = Some(reason.to_string());
        }
        state.paused_until.unwrap_or(until)
    }

    /// Zählt eine Seite ohne Beschreibung und liefert die Zahl in Folge.
    pub fn count_suspicious(&mut self, portal: Portal) -> u32 {
        let state = self.portals.entry(portal).or_default();
        state.suspicious_streak = state.suspicious_streak.saturating_add(1);
        state.suspicious_streak
    }

    /// Ein vollständiger Text setzt den Schutzschalter zurück.
    pub fn clear_suspicious(&mut self, portal: Portal) {
        if let Some(state) = self.portals.get_mut(&portal) {
            state.suspicious_streak = 0;
        }
    }

    /// Die Anmeldung ist weg (Zurücksetzen): Anmeldung nötig, keine bestätigte Sitzung.
    pub fn forget_session(&mut self, portal: Portal) {
        let state = self.portals.entry(portal).or_default();
        state.login_needed = true;
        state.session_confirmed_at = None;
    }

    /// Zugriffe in der letzten Stunde und in den letzten 24 Stunden.
    pub fn usage(&self, portal: Portal, now: Timestamp) -> (usize, usize) {
        let accesses = self
            .portals
            .get(&portal)
            .map_or(&[][..], |s| &s.accesses[..]);
        let since = |window: SignedDuration| {
            let start = now.saturating_sub(window).unwrap_or(Timestamp::MIN);
            accesses.iter().filter(|&&at| at > start).count()
        };
        (since(HOUR), since(DAY))
    }

    /// Letzter Zugriff oder letzte Antwort eines Portals – ab dem Späteren zählt der Abstand
    /// (auch über Läufe hinweg).
    fn last_access(&self, portal: Portal) -> Option<Timestamp> {
        let state = self.portals.get(&portal)?;
        state.accesses.iter().max().copied().max(state.last_done_at)
    }

    /// Wie lange vor dem nächsten Zugriff noch zu warten ist: ein zufälliger Abstand ab dem
    /// letzten Zugriff bzw. der letzten Antwort (langsame Antworten verkürzten ihn sonst auf
    /// null), höchstens ein Abstand lang – auch wenn die Uhr zurückgestellt wurde.
    pub fn pace_wait(&self, portal: Portal, now: Timestamp) -> Option<Duration> {
        let last = self.last_access(portal)?;
        let pace = SignedDuration::from_millis(
            i64::try_from(fastrand::u64(limits(portal).pace_ms)).unwrap_or(i64::MAX),
        );
        let wait = last
            .saturating_add(pace)
            .unwrap_or(Timestamp::MAX)
            .duration_since(now)
            .min(pace);
        wait.is_positive().then(|| wait.unsigned_abs())
    }

    /// Ein Abruf ist beendet (Antwort ausgewertet).
    pub fn record_done(&mut self, portal: Portal, now: Timestamp) {
        self.portals.entry(portal).or_default().last_done_at = Some(now);
    }

    /// Sitzung bestätigt (Jobseite mit Abmelde-Link) bzw. Anmeldung nötig.
    pub fn set_session(&mut self, portal: Portal, confirmed: bool, now: Timestamp) {
        let state = self.portals.entry(portal).or_default();
        state.login_needed = !confirmed;
        if confirmed {
            state.session_confirmed_at = Some(now);
        }
    }
}

/// Ist die Grenze `cap` im Fenster erreicht, der Zeitpunkt, ab dem wieder ein Zugriff frei
/// ist; sonst `None`.
fn quota_free_at(
    accesses: &[Timestamp],
    now: Timestamp,
    window: SignedDuration,
    cap: usize,
) -> Option<Timestamp> {
    let start = now.saturating_sub(window).unwrap_or(Timestamp::MIN);
    let mut recent: Vec<Timestamp> = accesses.iter().copied().filter(|&at| at > start).collect();
    if recent.len() < cap {
        return None;
    }
    recent.sort_unstable();
    // Frei wird der Platz, sobald der älteste der letzten `cap` Zugriffe aus dem Fenster fällt.
    let oldest_counting = recent[recent.len() - cap];
    oldest_counting.checked_add(window).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes: i64) -> Timestamp {
        Timestamp::from_second(1_790_000_000 + minutes * 60).unwrap()
    }

    /// Eine nur kurz gesperrte Datei darf beim Speichern nie die echte ersetzen – samt
    /// längerer Sperren und Zugriffszählern.
    #[test]
    fn a_locked_file_is_never_overwritten_by_the_stand_in() {
        let dir = tempfile::tempdir().unwrap();
        let missing_dir = dir.path().join("policy.json");
        // Ein Verzeichnis statt einer Datei: Lesen scheitert (nicht „fehlt“).
        std::fs::create_dir(&missing_dir).unwrap();
        let mut policy = Policy::load(&missing_dir, at(0));
        assert!(matches!(
            policy.allowance(Portal::LinkedIn, at(0)),
            Allowance::Paused { .. }
        ));
        policy.record_access(Portal::LinkedIn, at(1));
        policy.save().unwrap();
        assert!(missing_dir.is_dir(), "Notstand wird nie gespeichert");
    }

    #[test]
    fn an_unreadable_file_is_set_aside_and_the_pause_is_anchored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        std::fs::write(&path, b"{ kaputt").unwrap();
        let policy = Policy::load(&path, at(0));
        assert!(matches!(
            policy.allowance(Portal::LinkedIn, at(0)),
            Allowance::Paused { .. }
        ));
        // Die Pause beginnt nicht bei jedem Laden neu.
        let later = Policy::load(&path, at(600));
        assert_eq!(
            later.state(Portal::LinkedIn).paused_until,
            policy.state(Portal::LinkedIn).paused_until
        );
        let aside = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("policy.json.bad-")
            });
        assert!(aside, "kaputte Datei bleibt zur Ansicht erhalten");
    }

    #[test]
    fn hourly_and_daily_caps() {
        let mut p = Policy::in_memory();
        for i in 0..20 {
            assert_eq!(p.allowance(Portal::LinkedIn, at(i)), Allowance::Go);
            p.record_access(Portal::LinkedIn, at(i));
        }
        // 21. Zugriff in derselben Stunde: frei, sobald der erste aus dem Fenster fällt.
        assert_eq!(
            p.allowance(Portal::LinkedIn, at(30)),
            Allowance::Quota { next_at: at(60) }
        );
        assert_eq!(p.allowance(Portal::LinkedIn, at(61)), Allowance::Go);
        // Andere Portale sind unabhängig.
        assert_eq!(p.allowance(Portal::Freelancermap, at(30)), Allowance::Go);
        // Tagesgrenze 40: nach 40 Zugriffen über mehrere Stunden erst 24 h später.
        for i in 0..20 {
            p.record_access(Portal::LinkedIn, at(120 + i));
        }
        assert_eq!(
            p.allowance(Portal::LinkedIn, at(300)),
            Allowance::Quota {
                next_at: at(24 * 60)
            }
        );
    }

    #[test]
    fn pauses_escalate_and_never_shrink() {
        let mut p = Policy::in_memory();
        let until = p.pause(Portal::LinkedIn, PauseKind::Blocked, "HTTP 999", at(0));
        assert_eq!(until, at(24 * 60));
        assert!(matches!(
            p.allowance(Portal::LinkedIn, at(60)),
            Allowance::Paused { .. }
        ));
        // Zweites Sperrsignal binnen 7 Tagen: 7 Tage.
        let until = p.pause(
            Portal::LinkedIn,
            PauseKind::Blocked,
            "HTTP 999",
            at(2 * 24 * 60),
        );
        assert_eq!(until, at(9 * 24 * 60));
        // Eine Drosselung verkürzt die laufende Sperre nicht.
        let until = p.pause(
            Portal::LinkedIn,
            PauseKind::Throttled,
            "HTTP 429",
            at(3 * 24 * 60),
        );
        assert_eq!(until, at(9 * 24 * 60));
        assert_eq!(
            p.state(Portal::LinkedIn).pause_kind,
            Some(PauseKind::Blocked)
        );
        // Nach Ablauf wieder frei.
        assert_eq!(
            p.allowance(Portal::LinkedIn, at(9 * 24 * 60 + 1)),
            Allowance::Go
        );
    }

    #[test]
    fn state_survives_a_restart_and_a_broken_file_pauses_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        let mut p = Policy::load(&path, at(0));
        p.record_access(Portal::Freelancermap, at(0));
        p.pause(Portal::LinkedIn, PauseKind::Throttled, "HTTP 429", at(0));
        p.set_session(Portal::FreelanceDe, false, at(0));
        p.save().unwrap();
        let q = Policy::load(&path, at(1));
        assert_eq!(q.state(Portal::Freelancermap).accesses, [at(0)]);
        assert_eq!(q.state(Portal::LinkedIn).paused_until, Some(at(60)));
        assert!(q.state(Portal::FreelanceDe).login_needed);
        // Nur Portalnamen und Zeitstempel in der Datei.
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"linkedin\"") && !json.contains('@'));

        std::fs::write(&path, b"{kaputt").unwrap();
        let broken = Policy::load(&path, at(0));
        for portal in Portal::ALL {
            assert!(matches!(
                broken.allowance(portal, at(1)),
                Allowance::Paused { .. }
            ));
        }
    }

    /// Eine vorgestellte Uhr hinterließ Zugriffe und Pausen weit in der Zukunft: Nach dem
    /// Laden sperren sie höchstens so lange wie die längste Pause.
    #[test]
    fn a_clock_that_was_ahead_locks_no_portal_for_good() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        let future = at(60 * 24 * 365 * 70);
        let mut p = Policy::load(&path, future);
        for _ in 0..40 {
            p.record_access(Portal::LinkedIn, future);
        }
        p.record_done(Portal::LinkedIn, future);
        p.pause(Portal::LinkedIn, PauseKind::Blocked, "HTTP 999", future);
        p.pause(Portal::LinkedIn, PauseKind::Blocked, "HTTP 999", future);
        p.save().unwrap();

        let now = at(0);
        let q = Policy::load(&path, now);
        let state = q.state(Portal::LinkedIn);
        assert!(state.accesses.iter().all(|&a| a <= now));
        assert!(state.last_done_at.is_some_and(|d| d <= now));
        let week = now.saturating_add(REPEAT_BLOCK_PAUSE).unwrap();
        assert!(state.paused_until.is_some_and(|u| u <= week));
        assert!(state.block_until.is_some_and(|u| u <= week));
        // Nach einer Woche ist das Portal wieder frei – Pause und Zähler sind abgelaufen.
        assert_eq!(
            q.allowance(Portal::LinkedIn, at(7 * 24 * 60 + 1)),
            Allowance::Go
        );
        // Der Abstand gilt höchstens einmal.
        assert!(
            q.pace_wait(Portal::LinkedIn, now)
                .is_some_and(|w| w <= Duration::from_secs(7))
        );
    }

    #[test]
    fn old_accesses_are_forgotten() {
        let mut p = Policy::in_memory();
        p.record_access(Portal::LinkedIn, at(0));
        p.record_access(Portal::LinkedIn, at(25 * 60));
        assert_eq!(p.state(Portal::LinkedIn).accesses, [at(25 * 60)]);
    }
}
