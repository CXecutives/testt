//! Safety rules for portal requests - pace, caps, pauses - and their durable state in
//! `policy.json`.
//!
//! Principle: inconspicuous through restraint, not through disguise. **Every** request is
//! counted (failures and sign-in pages too); clicking again circumvents nothing. The file
//! lives next to the database and survives "delete text files" and "reset everything" - a
//! block pause must not be clickable away. It only holds portal names, codes and timestamps.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::export::write_atomic;
use crate::portal::Portal;

/// Pace and caps of a portal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Gap between two requests in milliseconds (random within the range).
    pub pace_ms: RangeInclusive<u64>,
    pub per_hour: usize,
    pub per_day: usize,
}

/// Pace and caps of a portal - from its adapter.
pub fn limits(portal: Portal) -> Limits {
    portal.adapter().limits()
}

/// Session window: dwell time per page (from "fully loaded") like a reader -
/// in addition to the gap, deliberately double restraint.
pub const DWELL_SECS: RangeInclusive<u64> = 8..=20;

/// Pause after throttling (429, server errors, second timeout).
const THROTTLE_PAUSE: SignedDuration = SignedDuration::from_hours(1);
/// Pause after a block signal (999, 403, redirect to sign-in, captcha).
const BLOCK_PAUSE: SignedDuration = SignedDuration::from_hours(24);
/// Pause after the second block signal within [`REPEAT_WINDOW`] - at the same time the
/// longest pause of all.
const REPEAT_BLOCK_PAUSE: SignedDuration = SignedDuration::from_hours(7 * 24);
const REPEAT_WINDOW: SignedDuration = SignedDuration::from_hours(7 * 24);

const HOUR: SignedDuration = SignedDuration::from_hours(1);
const DAY: SignedDuration = SignedDuration::from_hours(24);

/// Pause reasons of earlier versions, which stored only a German text. Only read to
/// derive the code of a pause that is still running - do not translate.
const LEGACY_BREAKER_TEXT: &str = "zwei Seiten ohne Beschreibung in Folge";
const LEGACY_UNREADABLE_TEXT: &str = "Sicherheitsstand war unlesbar";

/// How long a pause lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PauseKind {
    Throttled,
    Blocked,
}

/// Why a portal pauses or stopped - a code for the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum PauseReason {
    /// Rate limited: HTTP 429, server errors, no answer twice.
    Throttled,
    /// Block signal: HTTP 999/403, captcha, redirect to a sign-in wall.
    Blocked,
    /// Two pages without a description in a row - the page layout probably changed.
    LayoutChanged,
    /// The stored safety state was unreadable; every portal rests for 24 hours.
    StateUnreadable,
    /// Network trouble (second failure after a retry) - the rest waits for the next run.
    Network,
    /// Signed in, but the portal showed a security check - the portal rests until the next
    /// run (the app never solves a check itself).
    Challenged,
}

impl From<PauseKind> for PauseReason {
    fn from(kind: PauseKind) -> PauseReason {
        match kind {
            PauseKind::Throttled => PauseReason::Throttled,
            PauseKind::Blocked => PauseReason::Blocked,
        }
    }
}

/// State of a portal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalState {
    pub paused_until: Option<Timestamp>,
    pub pause_kind: Option<PauseKind>,
    /// Code of the last pause (missing in files of earlier versions).
    pub pause_code: Option<PauseReason>,
    /// Detail of the last pause for the log (e.g. "HTTP 999"); never shown as a sentence.
    pub pause_reason: Option<String>,
    /// End of the last block pause - a new block signal within seven days after it counts as
    /// a repetition (otherwise the short pause would start again after every 7-day pause).
    pub block_until: Option<Timestamp>,
    /// Pages without a description in a row - across runs (clicking again does not reset
    /// the breaker).
    pub suspicious_streak: u32,
    /// Requests of the last 24 hours.
    pub accesses: Vec<Timestamp>,
    /// End of the last request - the gap counts from the answer, also across runs.
    pub last_done_at: Option<Timestamp>,
    /// freelance.de: last confirmed session (first job page with a sign-out link).
    pub session_confirmed_at: Option<Timestamp>,
    /// freelance.de: sign-in needed (session expired or never signed in).
    pub login_needed: bool,
}

impl PortalState {
    /// Code of the current pause; files of earlier versions only carry a text.
    pub fn pause_code(&self) -> PauseReason {
        if let Some(code) = self.pause_code {
            return code;
        }
        let text = self.pause_reason.as_deref().unwrap_or_default();
        if text.starts_with(LEGACY_BREAKER_TEXT) {
            PauseReason::LayoutChanged
        } else if text == LEGACY_UNREADABLE_TEXT {
            PauseReason::StateUnreadable
        } else {
            self.pause_kind.map_or(PauseReason::Throttled, Into::into)
        }
    }
}

/// May the portal be requested now?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allowance {
    Go,
    Paused {
        until: Timestamp,
        reason: PauseReason,
        /// For the log only.
        detail: String,
    },
    /// Cap reached; the next request is possible from `next_at`.
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
    /// Reads `policy.json`. If it is missing, an empty state begins. If it is unreadable,
    /// every portal counts as paused for 24 hours to be safe - a damaged file must not lift
    /// a block pause.
    pub fn load(path: &Path, now: Timestamp) -> Policy {
        let mut policy = match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Policy>(&bytes) {
                Ok(mut policy) => {
                    policy.clamp_future(now);
                    policy
                }
                Err(e) => {
                    // Keep a copy for inspection and write the pause over the broken file -
                    // otherwise it would start again on every load and never end. If writing
                    // fails, the broken file stays: the next load pauses again (never a
                    // state without a pause).
                    log::warn!("policy.json unreadable ({e}), all portals paused for 24 h");
                    let aside = path.with_extension(format!("json.bad-{}", now.as_second()));
                    if let Err(e) = std::fs::copy(path, &aside) {
                        log::warn!("policy.json not copied aside: {e}");
                    }
                    let mut paused = Policy::all_paused(now);
                    paused.path = Some(path.to_path_buf());
                    if let Err(e) = paused.save() {
                        log::warn!("safety state not saved: {e}");
                    }
                    paused
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Policy::default(),
            Err(e) => {
                // Only unreadable right now (e.g. locked): pause, but never save - otherwise
                // this stand-in would replace the real file with its longer blocks.
                log::warn!("policy.json not readable ({e}), all portals paused for 24 h");
                return Policy::all_paused(now);
            }
        };
        policy.path = Some(path.to_path_buf());
        policy
    }

    /// State in memory only (dry run, tests): `save` writes nothing.
    pub fn in_memory() -> Policy {
        Policy::default()
    }

    /// Cap timestamps from a wrong clock: no request and no answer lies after "now", no
    /// pause reaches further than the longest one (seven days). Otherwise a date once set
    /// ahead would lock a portal for good - `policy.json` survives "reset everything" too.
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
            state.pause_code = Some(PauseReason::StateUnreadable);
            state.pause_reason = Some("safety state was unreadable".into());
        }
        policy
    }

    /// Writes the state atomically (no partial state after a crash).
    pub fn save(&self) -> crate::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let json = serde_json::to_vec_pretty(self).expect("the policy is always serialisable");
        write_atomic(path, &json)
    }

    pub fn state(&self, portal: Portal) -> PortalState {
        self.portals.get(&portal).cloned().unwrap_or_default()
    }

    /// Pause, hourly cap, daily cap - in this order.
    pub fn allowance(&self, portal: Portal, now: Timestamp) -> Allowance {
        let state = self.portals.get(&portal);
        if let Some(state) = state
            && let Some(until) = state.paused_until.filter(|&until| until > now)
        {
            return Allowance::Paused {
                until,
                reason: state.pause_code(),
                detail: state.pause_reason.clone().unwrap_or_default(),
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

    /// Counts a request and forgets requests older than 24 hours.
    pub fn record_access(&mut self, portal: Portal, now: Timestamp) {
        let state = self.portals.entry(portal).or_default();
        let horizon = now.saturating_sub(DAY).unwrap_or(Timestamp::MIN);
        state.accesses.retain(|&at| at > horizon);
        state.accesses.push(now);
    }

    /// Pauses a portal and returns the end of the pause (reason derived from the kind).
    pub fn pause(
        &mut self,
        portal: Portal,
        kind: PauseKind,
        detail: &str,
        now: Timestamp,
    ) -> Timestamp {
        self.pause_for(portal, kind, kind.into(), detail, now)
    }

    /// Pauses a portal with an explicit reason and returns the end of the pause. A second
    /// block signal within seven days extends to seven days; a running longer pause is
    /// never shortened.
    pub fn pause_for(
        &mut self,
        portal: Portal,
        kind: PauseKind,
        reason: PauseReason,
        detail: &str,
        now: Timestamp,
    ) -> Timestamp {
        let state = self.portals.entry(portal).or_default();
        let length = match kind {
            PauseKind::Throttled => THROTTLE_PAUSE,
            PauseKind::Blocked => {
                // Counted from the end of the last block (it always began before).
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
            state.pause_code = Some(reason);
            state.pause_reason = Some(detail.to_string());
        }
        state.paused_until.unwrap_or(until)
    }

    /// Counts a page without a description and returns the number in a row.
    pub fn count_suspicious(&mut self, portal: Portal) -> u32 {
        let state = self.portals.entry(portal).or_default();
        state.suspicious_streak = state.suspicious_streak.saturating_add(1);
        state.suspicious_streak
    }

    /// A complete text resets the breaker.
    pub fn clear_suspicious(&mut self, portal: Portal) {
        if let Some(state) = self.portals.get_mut(&portal) {
            state.suspicious_streak = 0;
        }
    }

    /// The sign-in is gone (reset): sign-in needed, no confirmed session.
    pub fn forget_session(&mut self, portal: Portal) {
        let state = self.portals.entry(portal).or_default();
        state.login_needed = true;
        state.session_confirmed_at = None;
    }

    /// Requests in the last hour and in the last 24 hours.
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

    /// Last request or last answer of a portal - the gap counts from the later one (also
    /// across runs).
    fn last_access(&self, portal: Portal) -> Option<Timestamp> {
        let state = self.portals.get(&portal)?;
        state.accesses.iter().max().copied().max(state.last_done_at)
    }

    /// How long to wait before the next request: a random gap from the last request or the
    /// last answer (slow answers would otherwise shrink it to zero), at most one gap long -
    /// even if the clock was set back.
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

    /// A request is finished (answer evaluated).
    pub fn record_done(&mut self, portal: Portal, now: Timestamp) {
        self.portals.entry(portal).or_default().last_done_at = Some(now);
    }

    /// Session confirmed (job page with a sign-out link) or sign-in needed.
    pub fn set_session(&mut self, portal: Portal, confirmed: bool, now: Timestamp) {
        let state = self.portals.entry(portal).or_default();
        state.login_needed = !confirmed;
        if confirmed {
            state.session_confirmed_at = Some(now);
        }
    }
}

/// If the cap `cap` is reached within the window, the time from which a request is free
/// again; otherwise `None`.
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
    // The slot frees up as soon as the oldest of the last `cap` requests leaves the window.
    let oldest_counting = recent[recent.len() - cap];
    oldest_counting.checked_add(window).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes: i64) -> Timestamp {
        Timestamp::from_second(1_790_000_000 + minutes * 60).unwrap()
    }

    /// A file that is only locked for a moment must never be replaced by the stand-in when
    /// saving - with its longer blocks and request counters.
    #[test]
    fn a_locked_file_is_never_overwritten_by_the_stand_in() {
        let dir = tempfile::tempdir().unwrap();
        let missing_dir = dir.path().join("policy.json");
        // A directory instead of a file: reading fails (not "missing").
        std::fs::create_dir(&missing_dir).unwrap();
        let mut policy = Policy::load(&missing_dir, at(0));
        assert!(matches!(
            policy.allowance(Portal::LinkedIn, at(0)),
            Allowance::Paused { .. }
        ));
        policy.record_access(Portal::LinkedIn, at(1));
        policy.save().unwrap();
        assert!(missing_dir.is_dir(), "the stand-in is never saved");
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
        // The pause does not start again on every load.
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
        assert!(aside, "the broken file is kept for inspection");
    }

    #[test]
    fn hourly_and_daily_caps() {
        let mut p = Policy::in_memory();
        for i in 0..20 {
            assert_eq!(p.allowance(Portal::LinkedIn, at(i)), Allowance::Go);
            p.record_access(Portal::LinkedIn, at(i));
        }
        // 21st request in the same hour: free as soon as the first leaves the window.
        assert_eq!(
            p.allowance(Portal::LinkedIn, at(30)),
            Allowance::Quota { next_at: at(60) }
        );
        assert_eq!(p.allowance(Portal::LinkedIn, at(61)), Allowance::Go);
        // Other portals are independent.
        assert_eq!(p.allowance(Portal::Freelancermap, at(30)), Allowance::Go);
        // Daily cap 40: after 40 requests over several hours only 24 h later.
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
        // Second block signal within 7 days: 7 days.
        let until = p.pause(
            Portal::LinkedIn,
            PauseKind::Blocked,
            "HTTP 999",
            at(2 * 24 * 60),
        );
        assert_eq!(until, at(9 * 24 * 60));
        // A throttle does not shorten the running block.
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
        // Free again afterwards.
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
        // Only portal names, codes and timestamps in the file.
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

    /// A clock that was ahead left requests and pauses far in the future: after loading they
    /// block at most as long as the longest pause.
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
        // After a week the portal is free again - pause and counters have expired.
        assert_eq!(
            q.allowance(Portal::LinkedIn, at(7 * 24 * 60 + 1)),
            Allowance::Go
        );
        // The gap applies at most once.
        assert!(
            q.pace_wait(Portal::LinkedIn, now)
                .is_some_and(|w| w <= Duration::from_secs(7))
        );
    }

    /// Pause codes: explicit ones are kept, files of earlier versions only carried a German
    /// text - their running pauses still get the right code.
    #[test]
    fn pause_codes_survive_and_legacy_texts_map() {
        let mut p = Policy::in_memory();
        p.pause_for(
            Portal::LinkedIn,
            PauseKind::Throttled,
            PauseReason::LayoutChanged,
            "two pages without description",
            at(0),
        );
        assert!(matches!(
            p.allowance(Portal::LinkedIn, at(1)),
            Allowance::Paused {
                reason: PauseReason::LayoutChanged,
                ..
            }
        ));
        p.pause(Portal::Freelancermap, PauseKind::Blocked, "HTTP 403", at(0));
        assert_eq!(
            p.state(Portal::Freelancermap).pause_code(),
            PauseReason::Blocked
        );
        let legacy = |text: &str, kind| PortalState {
            pause_kind: Some(kind),
            pause_reason: Some(text.into()),
            ..PortalState::default()
        };
        assert_eq!(
            legacy(
                "zwei Seiten ohne Beschreibung in Folge – Seitenaufbau geändert?",
                PauseKind::Throttled
            )
            .pause_code(),
            PauseReason::LayoutChanged
        );
        assert_eq!(
            legacy("Sicherheitsstand war unlesbar", PauseKind::Blocked).pause_code(),
            PauseReason::StateUnreadable
        );
        assert_eq!(
            legacy("HTTP 429", PauseKind::Throttled).pause_code(),
            PauseReason::Throttled
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
