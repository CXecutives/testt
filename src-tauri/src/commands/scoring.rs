//! The matching engine in the app: the compiled profile (kept until its file changes), the
//! matcher of every run and the rescore runs the app starts itself (rules in
//! `pipeline::rescore`). Those runs report to the page like any other run; they never open a
//! window or dialog.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use jobalert_core::error::ErrorKind;
use jobalert_core::pipeline::rescore::{Host, Rescore};
use jobalert_core::pipeline::{LocalMatcher, Matcher, RunEvent, RunKind, RunRequest, demo};
use jobalert_core::profile;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use super::{Activity, AppState, lock};

/// Path, size and change time of the profile file: another stamp means another profile.
type Stamp = (PathBuf, u64, Option<SystemTime>);
/// A compiled profile with the file state it was read from (`None` = no profile or no valid
/// JSON object).
type Cached = (Option<Stamp>, Option<Arc<LocalMatcher>>);

/// The engine state shared by the commands.
#[derive(Default)]
pub struct Scoring {
    /// The compiled profile, until the profile changes.
    cache: Mutex<Option<Cached>>,
    /// Event channel of the page (from its last `app_state`), for runs the app starts.
    page: Mutex<Option<Channel<RunEvent>>>,
    rules: Rescore,
}

impl Scoring {
    /// The page's channel for the runs the app starts; the page attaches to the run anyway
    /// when it loads (again).
    pub(super) fn set_page(&self, channel: Channel<RunEvent>) {
        *lock(&self.page) = Some(channel);
    }

    fn page(&self) -> Channel<RunEvent> {
        lock(&self.page)
            .clone()
            .unwrap_or_else(|| Channel::new(|_| Ok(())))
    }

    /// The app is closing: it starts no run of its own any more.
    pub fn stop(&self) {
        self.rules.stop();
    }
}

impl AppState {
    /// The compiled profile, an empty one too; `None` without a readable profile. The dry run
    /// uses the invented sample profile.
    pub(super) fn compiled_profile(&self) -> Option<Arc<LocalMatcher>> {
        if self.dry_run {
            return Some(demo::matcher());
        }
        let workspace = self.workspace().ok()?;
        let path = profile::profile_path(&workspace);
        let stamp = std::fs::metadata(&path)
            .ok()
            .map(|m| (path, m.len(), m.modified().ok()));
        let mut cache = lock(&self.scoring.cache);
        if let Some((cached, matcher)) = &*cache
            && *cached == stamp
        {
            return matcher.clone();
        }
        let matcher = match stamp.as_ref().map(|_| profile::load(&workspace)) {
            None | Some(Ok(None)) => None,
            Some(Ok(Some(value))) => {
                let matcher = LocalMatcher::from_json(&value);
                log::info!(
                    "profile compiled: {:?}, revision {}",
                    matcher.profile().quality(),
                    matcher.rev()
                );
                Some(Arc::new(matcher))
            }
            Some(Err(e)) => {
                // Not remembered: the next call reads the file again.
                log::warn!("profile not readable: {e}");
                return None;
            }
        };
        *cache = Some((stamp, matcher.clone()));
        matcher
    }

    /// The matcher of the runs and the reader: only a usable profile scores.
    pub(super) fn matcher(&self) -> Option<Arc<LocalMatcher>> {
        self.compiled_profile().filter(|m| m.usable())
    }

    /// Jobs waiting for a score with the current profile (0 without a usable one).
    pub(super) fn match_pending(&self) -> u32 {
        self.matcher().map_or(0, |m| {
            self.store.match_pending(m.rev()).unwrap_or_else(|e| {
                log::warn!("pending scores not counted: {e}");
                0
            })
        })
    }

    /// A run (not a sign-in) is in progress.
    pub(super) fn running(&self) -> bool {
        matches!(*lock(&self.activity), Activity::Run(_))
    }
}

/// The profile changed (chosen, removed, other workspace): the cached one goes, and every job
/// is scored again - now, or right after the run in progress.
pub(super) fn profile_changed(app: &AppHandle, state: &AppState) {
    *lock(&state.scoring.cache) = None;
    state.scoring.rules.profile_changed(&AppHost { app, state });
}

/// At the first page load: jobs that wait for a score are scored in the background.
pub(super) fn rescore_if_pending(app: &AppHandle, state: &AppState) {
    state.scoring.rules.at_start(&AppHost { app, state });
}

/// A run or a sign-in is over: a profile change during it is scored now.
pub(super) fn after_run(app: &AppHandle) {
    let state = app.state::<AppState>();
    state
        .scoring
        .rules
        .run_ended(&AppHost { app, state: &state });
}

struct AppHost<'a> {
    app: &'a AppHandle,
    state: &'a AppState,
}

impl Host for AppHost<'_> {
    fn running(&self) -> bool {
        self.state.running()
    }

    fn usable(&self) -> bool {
        self.state.matcher().is_some()
    }

    fn pending(&self) -> u32 {
        self.state.match_pending()
    }

    fn scored(&self) -> bool {
        self.state.store.has_matches().unwrap_or_else(|e| {
            log::warn!("stored scores not read: {e}");
            false
        })
    }

    fn has_jobs(&self) -> bool {
        self.state.store.job_count().unwrap_or(0) > 0
    }

    fn clear_matches(&self) {
        match self.state.store.clear_matches() {
            Ok(0) => {}
            Ok(n) => log::info!("{n} scores removed: no usable profile"),
            Err(e) => log::warn!("scores not removed: {e}"),
        }
    }

    fn launch_rescore(&self) -> Result<(), ErrorKind> {
        let request = RunRequest {
            kind: RunKind::Rescore,
        };
        super::run::launch(self.app, self.state, request, self.state.scoring.page())
            .map_err(|e| e.kind)
    }
}
