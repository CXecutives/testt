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
use std::sync::{Arc, Mutex};

use jobalert_core::error::{ErrorInfo, ErrorKind};
use jobalert_core::reset::ResetReport;
use jobalert_core::secrets::Vault;
use jobalert_core::settings::Settings;
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
pub const COMMANDS: [(&str, &str, &str); 25] = [
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
    ("set_pinned", "{ key: JobKey; on: boolean }", "boolean"),
    (
        "set_app_status",
        "{ key: JobKey; status: AppStatus | null }",
        "boolean",
    ),
    ("set_note", "{ key: JobKey; note: string }", "boolean"),
    ("set_hidden", "{ key: JobKey; hidden: boolean }", "boolean"),
    ("claude_prompt", "{ key: JobKey }", "string"),
    (
        "pick_profile",
        "Record<string, never>",
        "ProfileInfo | null",
    ),
    ("remove_profile", "Record<string, never>", "boolean"),
    (
        "save_profile_template",
        "Record<string, never>",
        "string | null",
    ),
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
        jobs::set_pinned,
        jobs::set_app_status,
        jobs::set_note,
        jobs::set_hidden,
        jobs::claude_prompt,
        profile::pick_profile,
        profile::remove_profile,
        profile::save_profile_template,
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

/// User-facing text, German by product decision: titles of the native file dialogs (the
/// only words the backend shows itself).
mod texts {
    pub const PICK_WORKSPACE: &str = "Arbeitsordner wählen";
    pub const PICK_PROFILE: &str = "Beraterprofil (JSON) wählen";
    pub const PROFILE_FILTER: &str = "Beraterprofil";
    pub const SAVE_TEMPLATE: &str = "Profilvorlage speichern";
    pub const TEMPLATE_NAME: &str = "beraterprofil-vorlage.json";
    // end of user-facing text
}

/// State of the app, shared by all commands.
pub struct AppState {
    pub store: Arc<Store>,
    pub data_dir: PathBuf,
    pub default_workspace: PathBuf,
    pub dry_run: bool,
    pub user_agent: String,
    pub reset_report: Mutex<Option<ResetReport>>,
    /// Gmail address from the vault. The vault (with the password) is thus read only once per
    /// start for the display - otherwise only for the mailbox scan.
    pub gmail_user: Mutex<GmailUser>,
    /// What the app is doing right now. Checked and claimed under the same lock - a run and a
    /// sign-in exclude each other (two writers of `policy.json` would lose requests).
    pub activity: Mutex<Activity>,
    /// The compiled profile and the rescore runs the app starts itself.
    pub scoring: Scoring,
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

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// "Not found" with what was looked for (`job`, `mail`, `folder`, `file`).
fn not_found(what: &str) -> ErrorInfo {
    ErrorInfo::new(ErrorKind::NotFound).with("what", what)
}
