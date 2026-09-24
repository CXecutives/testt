//! Domain logic of the Job-Alert-Monitor.
//!
//! This crate knows no Tauri: reading mails, fetching portals, storing and exporting is
//! fully testable here without a window (`cargo test -p jobalert-core`).
#![forbid(unsafe_code)]

pub mod error;
pub mod export;
pub mod fetch;
pub mod logging;
pub mod mail;
pub mod matching;
pub mod model;
pub mod pipeline;
pub mod portal;
pub mod profile;
pub mod reset;
pub mod secrets;
pub mod settings;
pub mod store;
pub mod sync;
pub mod text;
pub mod time;
pub mod view;

pub use error::{Error, Result};

/// Names in the app's data folder (`%LOCALAPPDATA%\de.cxecutives.job-alert-monitor`) - one
/// place for start, commands and reset.
pub const DB_FILE: &str = "jobs.db";
/// Safety state of the portals (kept on reset).
pub const POLICY_FILE: &str = "policy.json";
/// Prefix of the profile folders of the session windows (the user's sign-in).
pub const SESSION_PREFIX: &str = "session-";
pub const LOG_DIR: &str = "logs";

/// Profile folder of a portal's session window. The name hangs on the portal key;
/// `session-freelance` existed before, so an existing sign-in is kept.
pub fn session_dir(portal: portal::Portal) -> String {
    format!("{SESSION_PREFIX}{}", portal.key())
}

/// Set up cryptography for TLS (HTTP and IMAP) once: `ring` - no aws-lc, no OpenSSL.
/// Calling it again does no harm.
pub fn install_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
