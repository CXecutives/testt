//! Protokolldatei `logs/app.log` (1 MB, eine Rotation). Nie Geheimnisse, Mailinhalte oder
//! Seiten-HTML – deshalb eine harte Obergrenze je Quelle: async-imap schreibt auf `trace`
//! jeden Befehl wörtlich, also auch `LOGIN "adresse" "app-passwort"`. `trace` ist nie
//! erreichbar, auch nicht per Einstellung oder Umgebungsvariable.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::text::{one_line, truncate_chars};

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_LINE_CHARS: usize = 2_000;

/// Fremdbibliotheken, die höchstens Warnungen schreiben dürfen.
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

/// Darf eine Meldung ins Protokoll? `max` ist die gewählte Stufe (höchstens `Debug`).
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
    /// Richtet das Protokoll ein (einmal je Prozess).
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

#[cfg(test)]
mod tests {
    use super::*;

    /// async-imap nie unter `warn`, `trace` nie.
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
        // Nur ganze Modulnamen: „taurus“ ist nicht „tauri“.
        assert!(allowed("taurus", Level::Info, LevelFilter::Info));
    }
}
