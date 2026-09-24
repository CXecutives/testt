//! Log file `logs/app.log` (1 MB, one rotation). Never secrets, mail content or page
//! HTML - hence a hard cap per source: async-imap writes every command verbatim at
//! `trace`, including `LOGIN "address" "app-password"`. `trace` is never reachable,
//! not even through a setting or an environment variable.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::text::{one_line, truncate_chars};

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_LINE_CHARS: usize = 2_000;

/// Third-party libraries that may write at most warnings.
const QUIET: &[&str] = &[
    "async_imap",
    "imap_proto",
    "rustls",
    "tokio_rustls",
    "hyper",
    "h2",
    "reqwest",
    "html5ever",
    "selectors",
    "keyring",
    "tao",
    "wry",
    "tauri",
];

/// May a message go into the log? `max` is the chosen level (at most `Debug`).
pub fn allowed(target: &str, level: Level, max: LevelFilter) -> bool {
    let max = max.min(LevelFilter::Debug);
    let quiet = QUIET.iter().any(|q| {
        target == *q
            || target
                .strip_prefix(q)
                .is_some_and(|rest| rest.starts_with("::"))
    });
    let cap = if quiet {
        max.min(LevelFilter::Warn)
    } else {
        max
    };
    level <= cap
}

pub struct FileLogger {
    path: PathBuf,
    max: LevelFilter,
    file: Mutex<Option<File>>,
}

impl FileLogger {
    /// Sets up the log (once per process).
    pub fn install(dir: &Path, max: LevelFilter) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let logger = FileLogger {
            path: dir.join("app.log"),
            max: max.min(LevelFilter::Debug),
            file: Mutex::new(None),
        };
        log::set_max_level(logger.max);
        log::set_boxed_logger(Box::new(logger)).map_err(|e| e.to_string())
    }

    fn open(&self) -> Option<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok()
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        allowed(metadata.target(), metadata.level(), self.max)
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {}: {}\n",
            jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.target(),
            truncate_chars(&one_line(&record.args().to_string()), MAX_LINE_CHARS)
        );
        let mut guard = self
            .file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.is_none() {
            *guard = self.open();
        }
        let too_big = guard
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .is_some_and(|m| m.len() > MAX_BYTES);
        if too_big {
            *guard = None;
            let _ = std::fs::rename(&self.path, self.path.with_extension("log.1"));
            *guard = self.open();
        }
        if let Some(file) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Some(file) = self.file.lock().ok().as_mut().and_then(|g| g.as_mut()) {
            let _ = file.flush();
        }
    }
}

/// The log line of a panic: where it happened and what kind of failure it was - never the
/// message itself, because a slice panic quotes the text it cut (ad text, mail content).
pub fn panic_line(location: Option<&std::panic::Location<'_>>, message: &str) -> String {
    let kind = if message.contains("byte index")
        || message.contains("char boundary")
        || message.starts_with("begin <= end")
        || message.starts_with("begin > end")
    {
        "string slice"
    } else if message.starts_with("index out of bounds")
        || message.contains("out of range for slice")
    {
        "index"
    } else if message.starts_with("attempt to") {
        "arithmetic overflow"
    } else if message.contains("on a `None` value") {
        "unwrap on None"
    } else if message.contains("on an `Err` value") {
        "unwrap on Err"
    } else if message.starts_with("assertion") {
        "assertion"
    } else {
        "other"
    };
    match location {
        Some(at) => format!(
            "panic at {}:{}:{} ({kind})",
            at.file(),
            at.line(),
            at.column()
        ),
        None => format!("panic ({kind})"),
    }
}

/// The log line of a background task that ended without finishing: `what` and whether it
/// panicked or was cancelled - never the error's own text, whose `Display` repeats the panic
/// message (for a slice panic: the ad or mail text it cut). The panic hook logs the place.
pub fn task_failure_line(what: &str, error: &tokio::task::JoinError) -> String {
    let kind = if error.is_panic() {
        "panic"
    } else if error.is_cancelled() {
        "cancelled"
    } else {
        "unknown"
    };
    format!("{what} crashed ({kind})")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crashed run logs its kind only: the panic message of a slice panic quotes ad text.
    #[tokio::test]
    async fn a_crashed_task_logs_no_panic_message() {
        let secret = String::from("Abgeschlossenes Studium, geheimer Anzeigentext");
        let error = tokio::spawn(async move {
            let end = secret.len() + 1;
            secret[..end].len()
        })
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("geheimer Anzeigentext"),
            "the Display of a join error quotes the cut text"
        );
        let line = task_failure_line("run", &error);
        assert_eq!(line, "run crashed (panic)");
    }

    /// The ad text a slice panic quotes never reaches the log.
    #[test]
    fn a_panic_line_names_place_and_kind_only() {
        let at = std::panic::Location::caller();
        let message = "begin > end (55 > 54) when slicing `Abgeschlossenes Studium, oder etwas`";
        let line = panic_line(Some(at), message);
        assert!(line.starts_with("panic at "), "{line}");
        assert!(line.ends_with("(string slice)"), "{line}");
        assert!(!line.contains("Studium"), "{line}");
        assert_eq!(
            panic_line(None, "attempt to multiply with overflow"),
            "panic (arithmetic overflow)"
        );
        assert_eq!(panic_line(None, "secret text"), "panic (other)");
    }

    /// async-imap never below `warn`, `trace` never.
    #[test]
    fn imap_login_can_never_reach_the_log() {
        for max in [LevelFilter::Trace, LevelFilter::Debug, LevelFilter::Info] {
            assert!(!allowed("async_imap::client", Level::Trace, max));
            assert!(!allowed("async_imap::client", Level::Debug, max));
            assert!(!allowed("async_imap", Level::Info, max));
            assert!(allowed("async_imap::client", Level::Warn, max));
            assert!(!allowed("jobalert_core::pipeline", Level::Trace, max));
        }
        assert!(allowed(
            "jobalert_core::pipeline",
            Level::Debug,
            LevelFilter::Trace
        ));
        assert!(!allowed(
            "jobalert_core::pipeline",
            Level::Debug,
            LevelFilter::Info
        ));
        // Only whole module names: "taurus" is not "tauri".
        assert!(allowed("taurus", Level::Info, LevelFilter::Info));
    }
}
