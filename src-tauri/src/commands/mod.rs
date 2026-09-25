//! The commands of the interface (IPC v3) - one thin adapter per command onto
//! `jobalert-core`. Paths never come from the page: folders and files are chosen by the user
//! in a dialog, and only what was checked here is opened (`open_target`). Errors reach the
//! page as `ErrorInfo` (`{kind, params}`); their English text goes to the log.
//!
//! Command names live in four places that `core/tests/contract.rs` keeps in agreement: the
//! manifest in `build.rs`, `generate_handler!` below, `capabilities/main.json` and the
//! TypeScript command map generated from [`COMMANDS`].

mod app;
mod files;
mod jobs;
mod mailbox;
mod portals;
mod profile;
mod run;
mod scoring;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use jobalert_core::error::{ErrorInfo, ErrorKind};
use jobalert_core::reset::ResetReport;
use jobalert_core::secrets::Vault;
use jobalert_core::settings::{Language, Settings};
use jobalert_core::store::Store;
use tokio_util::sync::CancellationToken;

pub use run::RunHandle;
pub use scoring::Scoring;

/// Every command: name, arguments and result in TypeScript. `core/tests/contract.rs`
/// writes `ui/src/lib/ipc/types/commands.ts` from this table.
#[expect(
    dead_code,
    reason = "read by core/tests/contract.rs, which generates the TypeScript command map"
)]
pub const COMMANDS: [(&str, &str, &str); 34] = [
    ("app_state", "{ channel: Channel<RunEvent> }", "AppState"),
    (
        "start_run",
        "{ request: RunRequest; channel: Channel<RunEvent> }",
        "null",
    ),
    ("cancel_run", "Record<string, never>", "null"),
    ("list_jobs", "{ query: JobQuery }", "JobPage"),
    ("job_detail", "{ key: JobKey }", "JobDetail"),
    ("mark_read", "{ key: JobKey }", "boolean"),
    (
        "mark_all_read",
        "{ place: Place; search: string | null }",
        "JobKey[]",
    ),
    ("mark_unread", "{ keys: JobKey[] }", "number"),
    ("set_pinned", "{ key: JobKey; on: boolean }", "boolean"),
    ("move_jobs", "{ to: Place; keys: JobKey[] }", "JobKey[]"),
    (
        "set_override",
        "{ key: JobKey; include: boolean }",
        "boolean",
    ),
    ("purge_jobs", "{ keys: JobKey[] }", "Deleted"),
    ("empty_trash", "Record<string, never>", "Deleted"),
    ("ai_prompt", "{ key: JobKey }", "string"),
    ("ai_prompt_top", "{ limit: number }", "string"),
    (
        "pick_profile",
        "Record<string, never>",
        "ProfileDraft | null",
    ),
    ("parse_profile", "{ text: string }", "ProfileDraft"),
    ("profile_prompt", "Record<string, never>", "string"),
    ("save_profile", "{ save: ProfileSave }", "ProfileInfo"),
    ("remove_profile", "Record<string, never>", "boolean"),
    ("restore_profile", "Record<string, never>", "boolean"),
    ("set_unsaved", "{ on: boolean }", "null"),
    ("close_window", "Record<string, never>", "null"),
    (
        "save_mailbox",
        "{ user: string; password: string }",
        "Mailbox",
    ),
    ("remove_mailbox", "Record<string, never>", "boolean"),
    ("portal_login", "{ portal: Portal }", "boolean"),
    ("portal_logout", "{ portal: Portal }", "boolean"),
    ("pick_workspace", "Record<string, never>", "string | null"),
    ("rewrite_txt", "Record<string, never>", "ExportSummary"),
    ("clear_txt", "Record<string, never>", "ClearedTxt"),
    ("open_target", "{ target: OpenTarget }", "null"),
    ("save_settings", "{ patch: SettingsPatch }", "AppState"),
    ("reset_all", "Record<string, never>", "null"),
    (
        "report_ui_error",
        "{ message: string; source: string | null; line: number | null }",
        "null",
    ),
];

/// The command handler for `tauri::Builder::invoke_handler`.
pub fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        app::app_state,
        run::start_run,
        run::cancel_run,
        jobs::list_jobs,
        jobs::job_detail,
        jobs::mark_read,
        jobs::mark_all_read,
        jobs::mark_unread,
        jobs::set_pinned,
        jobs::move_jobs,
        jobs::set_override,
        jobs::purge_jobs,
        jobs::empty_trash,
        jobs::ai_prompt,
        jobs::ai_prompt_top,
        profile::pick_profile,
        profile::parse_profile,
        profile::profile_prompt,
        profile::save_profile,
        profile::remove_profile,
        profile::restore_profile,
        profile::set_unsaved,
        profile::close_window,
        mailbox::save_mailbox,
        mailbox::remove_mailbox,
        portals::portal_login,
        portals::portal_logout,
        app::pick_workspace,
        files::rewrite_txt,
        files::clear_txt,
        files::open_target,
        app::save_settings,
        app::reset_all,
        app::report_ui_error,
    ]
}

/// Titles of the native file dialogs (the only words the backend shows itself), in the
/// app's language.
mod texts {
    use jobalert_core::settings::Language;

    pub struct Dialogs {
        pub pick_workspace: &'static str,
        pub pick_profile: &'static str,
        pub profile_filter: &'static str,
    }

    pub fn of(language: Language) -> &'static Dialogs {
        match language {
            Language::De => &DE,
            Language::En => &EN,
        }
    }

    // User-facing text, German.
    const DE: Dialogs = Dialogs {
        pick_workspace: "Arbeitsordner wählen",
        pick_profile: "Beraterprofil (JSON) wählen",
        profile_filter: "Beraterprofil",
    };
    // end of user-facing text

    // User-facing text, English.
    const EN: Dialogs = Dialogs {
        pick_workspace: "Choose the work folder",
        pick_profile: "Choose a consultant profile (JSON)",
        profile_filter: "Consultant profile",
    };
    // end of user-facing text
}

/// State of the app, shared by all commands.
pub struct AppState {
    pub store: Arc<Store>,
    pub data_dir: PathBuf,
    pub default_workspace: PathBuf,
    pub dry_run: bool,
    pub user_agent: String,
    /// The app's language until the user chooses one (`Language::DEFAULT`, German).
    pub system_language: Language,
    pub reset_report: Mutex<Option<ResetReport>>,
    /// Gmail address from the vault. The vault (with the password) is thus read only once per
    /// start for the display - otherwise only for the mailbox scan.
    pub gmail_user: Mutex<GmailUser>,
    /// What the app is doing right now. Checked and claimed under the same lock - a run and a
    /// sign-in exclude each other (two writers of `policy.json` would lose requests).
    pub activity: Mutex<Activity>,
    /// The compiled profile and the rescore runs the app starts itself.
    pub scoring: Scoring,
    /// Unsaved changes of the page keep the window from closing until the page has asked.
    pub close_guard: CloseGuard,
}

/// The page holds unsaved changes (the Profil view's form, `set_unsaved`): a close request
/// of the window is held and the page asks the user (save, discard, cancel), then closes the
/// window itself (`close_window`). Nothing unsaved: the window closes at once. Every word
/// from the page counts as an answer; a page that says nothing to a request (gone or stuck)
/// never keeps the window open (main.rs closes it after a moment).
#[derive(Default)]
pub struct CloseGuard {
    unsaved: AtomicBool,
    answers: AtomicU64,
}

impl CloseGuard {
    /// What the page says: it holds unsaved changes or not.
    pub fn set(&self, unsaved: bool) {
        self.unsaved.store(unsaved, Ordering::SeqCst);
        self.answers.fetch_add(1, Ordering::SeqCst);
    }

    pub fn unsaved(&self) -> bool {
        self.unsaved.load(Ordering::SeqCst)
    }

    /// How often the page has spoken (compare before and after a close request).
    pub fn answers(&self) -> u64 {
        self.answers.load(Ordering::SeqCst)
    }
}

/// Stored Gmail address in the cache.
pub enum GmailUser {
    /// Not read from the vault yet.
    Unread,
    Known(Option<String>),
}

/// A run or signing in/out at a portal - never both at once.
pub enum Activity {
    Idle,
    Run(RunHandle),
    /// The session window is in use for signing in or out (cancellable).
    Session(CancellationToken),
}

type CmdResult<T> = Result<T, ErrorInfo>;

impl AppState {
    fn policy_path(&self) -> PathBuf {
        self.data_dir.join(jobalert_core::POLICY_FILE)
    }

    /// Stored Gmail address and, if so, why the vault is unreadable. A read error is not
    /// remembered - the next call tries again.
    fn gmail_user(&self) -> (Option<String>, Option<ErrorInfo>) {
        let mut cached = lock(&self.gmail_user);
        if let GmailUser::Known(user) = &*cached {
            return (user.clone(), None);
        }
        match Vault::app().load_gmail() {
            Ok(credentials) => {
                let user = credentials.map(|c| c.user);
                *cached = GmailUser::Known(user.clone());
                (user, None)
            }
            Err(e) => (None, Some(ErrorInfo::from(e))),
        }
    }

    fn settings(&self) -> CmdResult<Settings> {
        Ok(Settings::load(&self.store)?)
    }

    fn workspace(&self) -> CmdResult<PathBuf> {
        Ok(self.settings()?.workspace_or(&self.default_workspace))
    }

    /// The app's language: the chosen one, else the OS language.
    fn language(&self) -> CmdResult<Language> {
        Ok(self.settings()?.language_or(self.system_language))
    }

    pub fn busy(&self) -> bool {
        !matches!(*lock(&self.activity), Activity::Idle)
    }

    fn ensure_idle(&self) -> CmdResult<()> {
        if self.busy() {
            return Err(ErrorInfo::new(ErrorKind::Busy));
        }
        Ok(())
    }

    /// The dry run changes nothing outside its in-memory database.
    fn ensure_real(&self) -> CmdResult<()> {
        if self.dry_run {
            return Err(ErrorInfo::new(ErrorKind::DryRun));
        }
        Ok(())
    }

    /// Cancels a run or a sign-in/out in progress (idempotent).
    pub fn cancel_run(&self) {
        match &*lock(&self.activity) {
            Activity::Run(run) => run.cancel(),
            Activity::Session(cancel) => cancel.cancel(),
            Activity::Idle => {}
        }
    }
}

/// The app's language for the windows the backend opens itself (the sign-in window).
pub fn language<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Language {
    use tauri::Manager as _;
    app.try_state::<AppState>()
        .and_then(|state| state.language().ok())
        .unwrap_or(Language::DEFAULT)
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// "Not found" with what was looked for (`job`, `mail`, `folder`, `file`).
fn not_found(what: &str) -> ErrorInfo {
    ErrorInfo::new(ErrorKind::NotFound).with("what", what)
}
