//! When the app scores every job again by itself: after the profile changed (chosen, removed,
//! another workspace) and at the start when jobs wait for a score (new profile, engine
//! update). Only the rules live here; the app supplies the effects through [`Host`].

use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::ErrorKind;

/// What the rules need from the app.
pub trait Host {
    /// A run (not a sign-in) is in progress.
    fn running(&self) -> bool;
    /// A usable profile exists (one that names competences).
    fn usable(&self) -> bool;
    /// Jobs not scored with the current profile yet.
    fn pending(&self) -> u32;
    /// Some job still carries a score (of whatever profile).
    fn scored(&self) -> bool;
    fn has_jobs(&self) -> bool;
    /// Forgets every stored score.
    fn clear_matches(&self);
    /// Starts a rescore run; the error kind if none could start.
    fn launch_rescore(&self) -> Result<(), ErrorKind>;
}

/// The state of the rules: a rescore owed after the current run, and the closing app.
#[derive(Debug, Default)]
pub struct Rescore {
    due: AtomicBool,
    stopped: AtomicBool,
}

impl Rescore {
    /// The profile changed: score everything again - now, or right after the run in progress
    /// (which still scores with the profile it started with).
    ///
    /// The debt is noted before the check: a run that ends right after `running()` said yes
    /// then still finds it in [`Rescore::run_ended`]; whoever takes it first rescores, once.
    pub fn profile_changed(&self, host: &impl Host) {
        self.due.store(true, Ordering::SeqCst);
        if host.running() {
            return;
        }
        if self.due.swap(false, Ordering::SeqCst) {
            self.rescore(host);
        }
    }

    /// The first page load: jobs that wait for a score are scored in the background. A
    /// profile that became unusable while the app was closed (broken by hand, emptied, gone)
    /// is a profile change like any other: its old scores go and the files are written
    /// without them - the list, the counts and the exports never keep the verdicts of a
    /// profile that no longer reads.
    pub fn at_start(&self, host: &impl Host) {
        if !host.usable() {
            if host.scored() {
                log::info!("scores of a profile that no longer reads: removed at the start");
                self.rescore(host);
            }
            return;
        }
        let pending = host.pending();
        if pending > 0 {
            log::info!("{pending} jobs wait for a score: rescore at the start");
            self.start(host);
        }
    }

    /// A run or a sign-in is over (the slot is free again): a profile change during it is
    /// scored now.
    pub fn run_ended(&self, host: &impl Host) {
        if self.due.swap(false, Ordering::SeqCst) {
            self.rescore(host);
        }
    }

    /// The app is closing: it starts no run of its own any more.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    /// Without a usable profile the old scores go (the run then only writes the files again);
    /// with one, every job is scored anew. Without any job nothing runs - a run would also
    /// end the first-run state.
    fn rescore(&self, host: &impl Host) {
        if !host.usable() {
            host.clear_matches();
        }
        if host.has_jobs() {
            self.start(host);
        }
    }

    fn start(&self, host: &impl Host) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        match host.launch_rescore() {
            Ok(()) => log::info!("rescore started"),
            // A sign-in holds the slot: try again when it (or the next run) is over.
            Err(ErrorKind::Busy) => self.due.store(true, Ordering::SeqCst),
            Err(kind) => log::warn!("rescore not started: {kind:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    /// The app as far as the rules see it.
    #[derive(Default)]
    struct Fake {
        running: Cell<bool>,
        usable: bool,
        pending: u32,
        scored: bool,
        jobs: bool,
        busy: Cell<bool>,
        log: RefCell<Vec<&'static str>>,
    }

    impl Host for Fake {
        fn running(&self) -> bool {
            self.running.get()
        }
        fn usable(&self) -> bool {
            self.usable
        }
        fn pending(&self) -> u32 {
            self.pending
        }
        fn scored(&self) -> bool {
            self.scored
        }
        fn has_jobs(&self) -> bool {
            self.jobs
        }
        fn clear_matches(&self) {
            self.log.borrow_mut().push("clear");
        }
        fn launch_rescore(&self) -> Result<(), ErrorKind> {
            if self.busy.get() {
                return Err(ErrorKind::Busy);
            }
            self.log.borrow_mut().push("run");
            Ok(())
        }
    }

    fn fake(usable: bool, jobs: bool) -> Fake {
        Fake {
            usable,
            jobs,
            ..Fake::default()
        }
    }

    fn log(host: &Fake) -> Vec<&'static str> {
        host.log.borrow().clone()
    }

    #[test]
    fn a_new_profile_scores_everything_again() {
        let host = fake(true, true);
        Rescore::default().profile_changed(&host);
        assert_eq!(log(&host), ["run"]);
    }

    #[test]
    fn a_removed_profile_clears_the_scores_first() {
        let host = fake(false, true);
        Rescore::default().profile_changed(&host);
        assert_eq!(log(&host), ["clear", "run"]);
    }

    #[test]
    fn without_jobs_no_run_is_started() {
        let host = fake(true, false);
        Rescore::default().profile_changed(&host);
        assert!(log(&host).is_empty(), "the first-run state stays");
    }

    #[test]
    fn a_change_during_a_run_waits_for_its_end() {
        let rules = Rescore::default();
        let host = fake(true, true);
        host.running.set(true);
        rules.profile_changed(&host);
        assert!(log(&host).is_empty());
        host.running.set(false);
        rules.run_ended(&host);
        assert_eq!(log(&host), ["run"]);
        rules.run_ended(&host);
        assert_eq!(log(&host), ["run"], "only once");
    }

    /// The app whose run ends right after `running()` answered yes: slot free and
    /// `run_ended` done before `profile_changed` goes on.
    struct EndsDuringCheck<'a> {
        fake: &'a Fake,
        rules: &'a Rescore,
    }

    impl Host for EndsDuringCheck<'_> {
        fn running(&self) -> bool {
            let was = self.fake.running.replace(false);
            if was {
                self.rules.run_ended(self.fake);
            }
            was
        }
        fn usable(&self) -> bool {
            self.fake.usable()
        }
        fn pending(&self) -> u32 {
            self.fake.pending()
        }
        fn scored(&self) -> bool {
            self.fake.scored()
        }
        fn has_jobs(&self) -> bool {
            self.fake.has_jobs()
        }
        fn clear_matches(&self) {
            self.fake.clear_matches();
        }
        fn launch_rescore(&self) -> Result<(), ErrorKind> {
            self.fake.launch_rescore()
        }
    }

    #[test]
    fn a_change_is_not_lost_when_the_run_ends_during_the_check() {
        let rules = Rescore::default();
        let host = fake(true, true);
        host.running.set(true);
        rules.profile_changed(&EndsDuringCheck {
            fake: &host,
            rules: &rules,
        });
        assert_eq!(log(&host), ["run"], "scored with the new profile");
        rules.run_ended(&host);
        assert_eq!(log(&host), ["run"], "only once");
    }

    #[test]
    fn a_busy_slot_retries_after_the_next_run() {
        let rules = Rescore::default();
        let host = fake(true, true);
        host.busy.set(true);
        rules.profile_changed(&host);
        assert!(log(&host).is_empty());
        host.busy.set(false);
        rules.run_ended(&host);
        assert_eq!(log(&host), ["run"]);
    }

    #[test]
    fn the_start_rescores_only_when_jobs_wait() {
        let idle = fake(true, true);
        Rescore::default().at_start(&idle);
        assert!(log(&idle).is_empty(), "nothing pending");
        let waiting = Fake {
            pending: 3,
            ..fake(true, true)
        };
        Rescore::default().at_start(&waiting);
        assert_eq!(log(&waiting), ["run"]);
    }

    /// A profile broken (or emptied) while the app was closed: at the start its old scores
    /// go and a run writes the files without them - once; with nothing scored, nothing runs.
    #[test]
    fn an_unusable_profile_at_the_start_clears_the_old_scores() {
        let stale = Fake {
            scored: true,
            pending: 0,
            ..fake(false, true)
        };
        Rescore::default().at_start(&stale);
        assert_eq!(log(&stale), ["clear", "run"]);
        let clean = fake(false, true);
        Rescore::default().at_start(&clean);
        assert!(log(&clean).is_empty(), "no scores, nothing to do");
        let usable = Fake {
            scored: true,
            ..fake(true, true)
        };
        Rescore::default().at_start(&usable);
        assert!(log(&usable).is_empty(), "a usable profile keeps its scores");
    }

    #[test]
    fn a_closing_app_starts_nothing() {
        let rules = Rescore::default();
        let host = fake(true, true);
        host.running.set(true);
        rules.profile_changed(&host);
        rules.stop();
        host.running.set(false);
        rules.run_ended(&host);
        rules.profile_changed(&host);
        assert!(log(&host).is_empty());
    }
}
