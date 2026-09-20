//! Fachlogik des Job-Alert-Monitors.
//!
//! Dieses Paket kennt Tauri nicht: Mails lesen, Portale abrufen, speichern und
//! exportieren ist hier vollständig ohne Fenster testbar (`cargo test -p jobalert-core`).
#![forbid(unsafe_code)]

pub mod error;
pub mod export;
pub mod fetch;
pub mod logging;
pub mod mail;
pub mod model;
pub mod pipeline;
pub mod portal;
pub mod profile;
pub mod reset;
pub mod secrets;
pub mod settings;
pub mod store;
pub mod text;
pub mod time;
pub mod view;

pub use error::{Error, Result};

/// Namen im Datenordner der App (`%LOCALAPPDATA%\de.cxecutives.job-alert-monitor`) – eine
/// Stelle für Start, Befehle und Zurücksetzen.
pub const DB_FILE: &str = "jobs.db";
/// Sicherheitsstand der Portale (bleibt beim Zurücksetzen erhalten).
pub const POLICY_FILE: &str = "policy.json";
/// Präfix der Profilordner der Sitzungsfenster (Anmeldung des Nutzers).
pub const SESSION_PREFIX: &str = "session-";
pub const LOG_DIR: &str = "logs";

/// Profilordner des Sitzungsfensters eines Portals. Der Name hängt am Portalschlüssel;
/// `session-freelance` gab es schon, die vorhandene Anmeldung bleibt damit erhalten.
pub fn session_dir(portal: portal::Portal) -> String {
    format!("{SESSION_PREFIX}{}", portal.key())
}

/// Kryptografie für TLS (HTTP und IMAP) einmal einrichten: `ring` – kein aws-lc, kein
/// OpenSSL. Mehrfacher Aufruf schadet nicht.
pub fn install_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
