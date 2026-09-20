//! Beraterprofil: genau eine JSON-Datei `profil/beraterprofil.json` im Arbeitsordner – dort
//! liest sie der Matching-Skill. Hochladen ersetzt die bisherige Datei; der Inhalt wird
//! geprüft, aber unverändert übernommen (Reihenfolge und Schreibweise bleiben die des
//! Nutzers). Kein Netzwerk: alles bleibt auf dem Rechner.

use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::export::write_atomic;
use crate::text::plural;

pub const PROFILE_DIR: &str = "profil";
pub const PROFILE_FILE: &str = "beraterprofil.json";

/// Zustand des hinterlegten Profils für die Oberfläche.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub path: PathBuf,
    pub bytes: u64,
    pub saved_at: Option<Timestamp>,
    /// Kurzbeschreibung des Inhalts („Schlüssel: name, skills, …“).
    pub content: String,
    /// Ist die Datei inzwischen kein gültiges JSON mehr (von Hand bearbeitet)?
    pub parse_error: Option<String>,
}

pub fn profile_path(workspace: &Path) -> PathBuf {
    workspace.join(PROFILE_DIR).join(PROFILE_FILE)
}

/// Das hinterlegte Profil; `None`, wenn keins da ist.
pub fn info(workspace: &Path) -> Result<Option<ProfileInfo>> {
    let path = profile_path(workspace);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(&path, e)),
    };
    let (content, parse_error) = match parse(&bytes) {
        Ok(value) => (describe(&value), None),
        Err(message) => (String::new(), Some(message)),
    };
    let saved_at = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| Timestamp::try_from(t).ok());
    Ok(Some(ProfileInfo {
        bytes: bytes.len() as u64,
        path,
        saved_at,
        content,
        parse_error,
    }))
}

/// Prüft die gewählte Datei und legt sie als Profil ab (atomar, ersetzt das bisherige).
pub fn save_from(workspace: &Path, source: &Path) -> Result<ProfileInfo> {
    let bytes = std::fs::read(source).map_err(|e| Error::io(source, e))?;
    let value = parse(&bytes).map_err(Error::Invalid)?;
    if !value.is_object() {
        return Err(Error::Invalid(format!(
            "Das Beraterprofil muss ein JSON-Objekt sein (gefunden: {}).\nBeispiel: {{\"name\": \"…\", \"skills\": […], \"erfahrung\": […] }}",
            describe(&value)
        )));
    }
    let text = utf8(&bytes).map_err(Error::Invalid)?;
    write_atomic(&profile_path(workspace), text.as_bytes())?;
    info(workspace)?.ok_or_else(|| Error::Corrupt("Profil nach dem Speichern nicht lesbar".into()))
}

/// Entfernt das Profil (kein Fehler, wenn keins da ist); der leere Ordner verschwindet mit.
pub fn remove(workspace: &Path) -> Result<bool> {
    let path = profile_path(workspace);
    let removed = match std::fs::remove_file(&path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(Error::io(&path, e)),
    };
    let _ = std::fs::remove_dir(workspace.join(PROFILE_DIR));
    Ok(removed)
}

/// Text der Datei ohne Byte-Order-Mark.
fn utf8(bytes: &[u8]) -> std::result::Result<&str, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| format!("Die Datei ist kein UTF-8-Text (andere Kodierung?):\n{e}"))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

fn parse(bytes: &[u8]) -> std::result::Result<Value, String> {
    let text = utf8(bytes)?;
    serde_json::from_str(text).map_err(|e| {
        format!(
            "Kein gültiges JSON (Zeile {}, Spalte {}):\n{e}",
            e.line(),
            e.column()
        )
    })
}

/// Kurze Beschreibung für die Statusanzeige.
fn describe(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).take(12).collect();
            if map.len() > 12 {
                keys.push("…");
            }
            format!("Schlüssel: {}", keys.join(", "))
        }
        Value::Array(items) => format!("Liste mit {}", plural(items.len(), "Element", "Elementen")),
        Value::String(_) => "Text".into(),
        Value::Number(_) => "Zahl".into(),
        Value::Bool(_) => "Wahrheitswert".into(),
        Value::Null => "null".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_keeps_the_file_as_written_and_replaces_the_old_one() {
        let workspace = tempfile::tempdir().unwrap();
        let source = workspace.path().join("Profil_Test.json");
        let original = "\u{feff}{\n  \"skills\": [\"SAP\"],\n  \"name\": \"Müller\"\n}\n";
        std::fs::write(&source, original).unwrap();
        let info = save_from(workspace.path(), &source).unwrap();
        assert_eq!(info.content, "Schlüssel: name, skills");
        assert_eq!(info.parse_error, None);
        let stored = std::fs::read_to_string(profile_path(workspace.path())).unwrap();
        assert_eq!(
            stored,
            original.trim_start_matches('\u{feff}'),
            "unverändert, ohne BOM"
        );
        // Das Original im Arbeitsordner bleibt unberührt (gehört dem Nutzer/Skill).
        assert!(source.exists());
        std::fs::write(&source, "{\"neu\": 1}").unwrap();
        save_from(workspace.path(), &source).unwrap();
        assert_eq!(info_content(workspace.path()), "Schlüssel: neu");
    }

    fn info_content(workspace: &Path) -> String {
        info(workspace).unwrap().unwrap().content
    }

    #[test]
    fn invalid_files_are_refused_with_a_clear_message() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            (&b"{\"name\": }"[..], "Zeile 1"),
            (&b"[1, 2]"[..], "JSON-Objekt"),
            (&[0xff, 0xfe, 0x00][..], "UTF-8"),
        ];
        for (bytes, expected) in cases {
            let source = dir.path().join("x.json");
            std::fs::write(&source, bytes).unwrap();
            let error = save_from(dir.path(), &source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        assert!(info(dir.path()).unwrap().is_none(), "nichts gespeichert");
    }

    #[test]
    fn remove_and_hand_edited_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!remove(dir.path()).unwrap());
        std::fs::create_dir_all(dir.path().join(PROFILE_DIR)).unwrap();
        std::fs::write(profile_path(dir.path()), "{kaputt").unwrap();
        assert!(info(dir.path()).unwrap().unwrap().parse_error.is_some());
        assert!(remove(dir.path()).unwrap());
        assert!(!dir.path().join(PROFILE_DIR).exists());
    }
}
