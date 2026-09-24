//! Consultant profile: exactly one JSON file `profil/beraterprofil.json` in the workspace -
//! that is where the matching skill reads it. Uploading replaces the previous file; the
//! content is checked but taken over unchanged (order and spelling stay the user's). No
//! network: everything stays on the computer.

use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde_json::Value;

use crate::error::{Error, InvalidInput, Result};
use crate::export::write_atomic;

/// Folder and file name are a contract with the matching skill - do not translate.
pub const PROFILE_DIR: &str = "profil";
pub const PROFILE_FILE: &str = "beraterprofil.json";

/// Fill-in template for a new profile with every key the matching engine reads and short
/// example values. The keys are an external contract (profile JSON) - do not translate. Every
/// key is optional: `harte_kriterien` keys that are left out are simply inactive.
const TEMPLATE: &str = r#"{
  "name": "Vorname Nachname",
  "titel": "Interim Manager Finanzen",
  "abschluss": "Diplom-Kaufmann (Univ.)",
  "berufserfahrung_jahre": 15,
  "kernkompetenzen": [
    { "kompetenz": "Controlling", "jahre": 12, "auch": ["Financial Controlling", "Konzerncontrolling"] },
    { "kompetenz": "Konzernrechnungslegung nach IFRS", "jahre": 8, "auch": ["Group Reporting"] },
    { "kompetenz": "Projektmanagement", "jahre": 10 }
  ],
  "methoden_tools": [
    { "name": "SAP S/4HANA" },
    { "name": "Power BI" }
  ],
  "zertifizierungen": [
    { "name": "PMP" }
  ],
  "branchen": [
    { "branche": "Maschinenbau" }
  ],
  "sprachen": [
    { "sprache": "Deutsch", "niveau": "Muttersprache" },
    { "sprache": "Englisch", "niveau": "C1" }
  ],
  "alleinstellungsmerkmale": [
    "Aufbau eines Konzernreportings in 100 Tagen"
  ],
  "keywords": ["IFRS", "HGB", "Transformation"],
  "harte_kriterien": {
    "min_tagessatz": 900,
    "laender": ["DE", "AT", "CH"],
    "ausgeschlossene_vertragsarten": ["anue"],
    "remote_ausserhalb_erlaubt": true,
    "verfuegbar_ab": "sofort",
    "min_jahresgehalt": 120000,
    "festanstellung_orte": ["Hamburg", "Berlin"],
    "festanstellung_remote_min": 50,
    "zielprofil_min_jahre": 8
  }
}
"#;

/// State of the stored profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    pub path: PathBuf,
    pub bytes: u64,
    pub saved_at: Option<Timestamp>,
    /// The file is no valid JSON object any more (edited by hand).
    pub parse_error: Option<InvalidInput>,
}

pub fn profile_path(workspace: &Path) -> PathBuf {
    workspace.join(PROFILE_DIR).join(PROFILE_FILE)
}

/// The stored profile; `None` if there is none.
pub fn info(workspace: &Path) -> Result<Option<ProfileInfo>> {
    let path = profile_path(workspace);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(&path, e)),
    };
    let parse_error = parse_object(&bytes).err();
    let saved_at = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| Timestamp::try_from(t).ok());
    Ok(Some(ProfileInfo {
        bytes: bytes.len() as u64,
        path,
        saved_at,
        parse_error,
    }))
}

/// Checks the chosen file and stores it as the profile (atomically, replacing the old one).
pub fn save_from(workspace: &Path, source: &Path) -> Result<ProfileInfo> {
    let bytes = std::fs::read(source).map_err(|e| Error::io(source, e))?;
    parse_object(&bytes)?;
    let text = utf8(&bytes)?;
    write_atomic(&profile_path(workspace), text.as_bytes())?;
    info(workspace)?.ok_or_else(|| Error::Corrupt("profile unreadable right after saving".into()))
}

/// The stored profile as JSON; `None` if there is none or it is no valid JSON object (the
/// profile info names the error).
pub fn load(workspace: &Path) -> Result<Option<Value>> {
    let path = profile_path(workspace);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(parse_object(&bytes).ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(&path, e)),
    }
}

/// Writes the fill-in template to `target` (a place the user chose).
pub fn write_template(target: &Path) -> Result<()> {
    write_atomic(target, TEMPLATE.as_bytes())
}

/// Removes the profile (no error if there is none); the empty folder goes with it.
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

/// Text of the file without a byte order mark.
fn utf8(bytes: &[u8]) -> std::result::Result<&str, InvalidInput> {
    let text = std::str::from_utf8(bytes).map_err(|_| InvalidInput::ProfileNotUtf8)?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

/// The file must be a JSON object.
fn parse_object(bytes: &[u8]) -> std::result::Result<Value, InvalidInput> {
    let value: Value =
        serde_json::from_str(utf8(bytes)?).map_err(|e| InvalidInput::ProfileNotJson {
            line: e.line(),
            column: e.column(),
        })?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(InvalidInput::ProfileNotObject {
            found: json_kind(&value).into(),
        })
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
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
        assert_eq!(info.parse_error, None);
        let stored = std::fs::read_to_string(profile_path(workspace.path())).unwrap();
        assert_eq!(
            stored,
            original.trim_start_matches('\u{feff}'),
            "unchanged, without BOM"
        );
        // The original in the workspace stays untouched (it belongs to the user/skill).
        assert!(source.exists());
        std::fs::write(&source, "{\"neu\": 1}").unwrap();
        let second = save_from(workspace.path(), &source).unwrap();
        assert_eq!(second.bytes, 10);
    }

    #[test]
    fn invalid_files_are_refused_with_a_code() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            (
                &b"{\"name\": }"[..],
                InvalidInput::ProfileNotJson {
                    line: 1,
                    column: 10,
                },
            ),
            (
                &b"[1, 2]"[..],
                InvalidInput::ProfileNotObject {
                    found: "array".into(),
                },
            ),
            (&[0xff, 0xfe, 0x00][..], InvalidInput::ProfileNotUtf8),
        ];
        for (bytes, expected) in cases {
            let source = dir.path().join("x.json");
            std::fs::write(&source, bytes).unwrap();
            let error = save_from(dir.path(), &source).unwrap_err();
            assert!(
                matches!(&error, Error::Invalid(input) if *input == expected),
                "{error:?}"
            );
        }
        assert!(info(dir.path()).unwrap().is_none(), "nothing stored");
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

    /// The template round trip: saved, taken over as the profile, loaded - and the engine
    /// understands all of it (good quality, every hard criterion, no warning).
    #[test]
    fn the_template_is_a_valid_profile() {
        use crate::matching::{self, CriterionKey, ProfileQuality};
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("vorlage.json");
        write_template(&target).unwrap();
        let info = save_from(dir.path(), &target).unwrap();
        assert_eq!(info.parse_error, None);
        let value = load(dir.path()).unwrap().unwrap();
        let profile = matching::compile_profile(&value);
        assert_eq!(profile.quality(), ProfileQuality::Good);
        let summary = profile.summary();
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        let keys: Vec<CriterionKey> = summary
            .criteria
            .iter()
            .filter(|c| c.set)
            .map(|c| c.key)
            .collect();
        assert_eq!(
            keys,
            [
                CriterionKey::MinDayRate,
                CriterionKey::Countries,
                CriterionKey::NoAnue,
                CriterionKey::Availability
            ]
        );
        for path in [
            "kernkompetenzen[].kompetenz",
            "methoden_tools[].name",
            "sprachen[].sprache",
        ] {
            assert!(
                summary.sources.iter().any(|s| s.path == path && !s.guessed),
                "{path}: {:?}",
                summary.sources
            );
        }
        assert!(summary.competences.iter().any(|c| c == "Controlling"));
        assert!(
            !summary.competences.iter().any(|c| c == "Hamburg"),
            "criteria values are no competences"
        );
    }

    #[test]
    fn load_skips_what_is_no_profile() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()).unwrap(), None);
        std::fs::create_dir_all(dir.path().join(PROFILE_DIR)).unwrap();
        std::fs::write(profile_path(dir.path()), "[1]").unwrap();
        assert_eq!(load(dir.path()).unwrap(), None);
        std::fs::write(
            profile_path(dir.path()),
            "\u{feff}{\"keywords\": [\"SAP\"]}",
        )
        .unwrap();
        assert_eq!(load(dir.path()).unwrap().unwrap()["keywords"][0], "SAP");
    }
}
