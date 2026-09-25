//! Consultant profile: exactly one JSON file `profil/beraterprofil.json` in the workspace -
//! that is where the matching skill reads it. The Profil view edits it through a form
//! ([`form`]): saving merges the form into the file, so keys the form does not know, their
//! values and the order of the keys stay; the previous file stays next to it as the one
//! backup. Removing the profile makes it that backup, so it can be restored. A file or a
//! pasted answer of an AI ([`prompt`], read by [`answer`]) fills the form first, the user
//! reviews it and saves. No network: everything stays on the computer.

mod answer;
mod form;
mod json;
pub mod prompt;

use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde_json::Value;

use crate::error::{Error, InvalidInput, Result};
use crate::export::write_atomic;
use crate::matching::{self, ProfileQuality, ProfileSummary};
use crate::settings::Language;

pub use form::{
    LanguageLevel, MAX_FOCUS, ProfileAvailability, ProfileCompetence, ProfileCriteria, ProfileForm,
    ProfileLanguage, ProfileWishes, RemoteWish, UnreadableField,
};
use json::Json;

/// Folder and file name are a contract with the matching skill - do not translate.
pub const PROFILE_DIR: &str = "profil";
pub const PROFILE_FILE: &str = "beraterprofil.json";
/// The previous profile, kept next to it on every save (one backup, replaced each time).
pub const BACKUP_FILE: &str = "beraterprofil.json.bak";

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

/// The ISO codes of every country the engine tells apart in a job ad, by a country or a
/// city name of its lexicon (a German city without a country is in Germany), sorted: the
/// countries a profile can choose (`laender`). The UI catalogs name each of them
/// (`profile.country`), which `core/tests/countries.rs` checks.
pub fn country_codes() -> Vec<&'static str> {
    use crate::matching::lexicon::engine as lex;
    let mut codes: Vec<&'static str> = lex::COUNTRIES
        .iter()
        .chain(lex::CITIES)
        .map(|&(_, code)| code)
        .chain(["DE"])
        .collect();
    codes.sort_unstable();
    codes.dedup();
    codes
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

pub fn backup_path(workspace: &Path) -> PathBuf {
    workspace.join(PROFILE_DIR).join(BACKUP_FILE)
}

/// Removes the profile: it becomes the one backup next to where it was (replacing an older
/// one), so [`restore`] can bring it back. `false` if there was none.
pub fn remove(workspace: &Path) -> Result<bool> {
    let path = profile_path(workspace);
    if !path.exists() {
        return Ok(false);
    }
    let backup = backup_path(workspace);
    std::fs::rename(&path, &backup).map_err(|e| Error::io(&path, e))?;
    Ok(true)
}

/// Brings a removed profile back from the backup; `false` when there is a profile already
/// or no backup.
pub fn restore(workspace: &Path) -> Result<bool> {
    let path = profile_path(workspace);
    let backup = backup_path(workspace);
    if path.exists() || !backup.exists() {
        return Ok(false);
    }
    std::fs::rename(&backup, &path).map_err(|e| Error::io(&backup, e))?;
    Ok(true)
}

/// A profile read for the editor: the form, the JSON text it came from (saving merges the
/// form into it) and what the engine understands of it (its warnings show before saving).
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub form: ProfileForm,
    pub source: String,
    pub quality: ProfileQuality,
    pub summary: ProfileSummary,
}

fn draft(doc: &Json, source: &str) -> Draft {
    let compiled = matching::compile_profile(&doc.to_value());
    Draft {
        form: form::read(doc),
        source: source.to_owned(),
        quality: compiled.quality(),
        summary: compiled.summary().clone(),
    }
}

/// A profile file the user chose, for review in the editor (nothing is stored yet).
pub fn draft_from_file(path: &Path) -> Result<Draft> {
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    let text = utf8(&bytes)?;
    Ok(draft(&parse_doc(text)?, text))
}

/// The AI's answer to the [`prompt`] for a new profile: the profile JSON in it (bare, inside
/// a code block or between sentences, repaired and in the skeleton's shape, see [`answer`]),
/// for review in the editor. What the answer leaves empty is dropped, so no empty value
/// becomes a criterion the app cannot read.
pub fn draft_from_answer(answer: &str) -> std::result::Result<Draft, InvalidInput> {
    let (doc, source) = answer::read(answer)?;
    Ok(draft(&doc, &source))
}

/// The AI's answer to the update prompt: the answer's form (the editor merges it into the
/// stored one, keeping the user's own settings) and, as the JSON the save goes into, the
/// stored profile with the answer's career stations in place of its own (the CV is their
/// source and the form does not show them). Without a readable stored profile it is the
/// draft of a new one.
pub fn update_from_answer(workspace: &Path, answer: &str) -> Result<Draft> {
    let (found, source) = answer::read(answer)?;
    let stored = std::fs::read(profile_path(workspace))
        .ok()
        .and_then(|bytes| {
            let text = utf8(&bytes).ok()?.to_owned();
            Some((parse_doc(&text).ok()?, text))
        });
    let Some((mut doc, text)) = stored else {
        return Ok(draft(&found, &source));
    };
    let source = match found.get(STATIONS) {
        Some(stations) => {
            doc.insert_before(STATIONS, stations.clone(), SETTINGS);
            doc.to_pretty().trim_end().to_owned()
        }
        None => text,
    };
    let compiled = matching::compile_profile(&doc.to_value());
    Ok(Draft {
        form: form::read(&found),
        source,
        quality: compiled.quality(),
        summary: compiled.summary().clone(),
    })
}

/// The career stations (the engine reads them, the form does not show them).
const STATIONS: &str = matching::lexicon::engine::KEY_STATIONS;
/// The sections of the user's settings, German and English: new keys go before them.
const SETTINGS: &[&str] = &[
    matching::lexicon::KEY_PREFERENCES,
    "preferences",
    matching::lexicon::KEY_CRITERIA,
    "hard_criteria",
];

/// The request for an AI in the app's language (see [`prompt`]): for a new profile, or with a
/// `workspace` whose stored profile holds something of a CV, for an update of it.
pub fn cv_prompt(workspace: Option<&Path>, language: Language) -> String {
    let today = crate::time::local_date(Timestamp::now());
    let stored = workspace
        .and_then(stored_form)
        .filter(ProfileForm::has_content);
    prompt::text(language, today, stored.as_ref())
}

/// The form of the stored profile; `None` without a readable one.
pub fn stored_form(workspace: &Path) -> Option<ProfileForm> {
    let bytes = std::fs::read(profile_path(workspace)).ok()?;
    form_of(utf8(&bytes).ok()?)
}

/// The form of a profile text; `None` if it is no JSON object.
pub fn form_of(text: &str) -> Option<ProfileForm> {
    parse_doc(text).ok().map(|doc| form::read(&doc))
}

/// Saves the editor's form. `before` is the form as the editor received it: only fields
/// that differ from it are written. They go into `source` (the JSON of a chosen file or a
/// pasted answer, `{}` for a new profile) or, without one, into the stored profile. The
/// previous file becomes the one backup next to it; the file is replaced atomically.
pub fn save_form(
    workspace: &Path,
    source: Option<&str>,
    before: &ProfileForm,
    after: &ProfileForm,
    clear: &[UnreadableField],
) -> Result<ProfileInfo> {
    let after = form::validate(after)?;
    let path = profile_path(workspace);
    let previous = match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(Error::io(&path, e)),
    };
    let mut doc = match (source, &previous) {
        (Some(text), _) => parse_doc(text.strip_prefix('\u{feff}').unwrap_or(text))?,
        (None, Some(bytes)) => parse_doc(utf8(bytes)?)?,
        (None, None) => Json::object(),
    };
    let unchanged = doc.clone();
    form::merge(&mut doc, before, &after, clear);
    // Nothing to write: the stored file stays exactly as it is (no reformatting).
    let keep = source.is_none() && previous.is_some() && doc == unchanged;
    if !keep {
        if let Some(old) = &previous {
            write_atomic(&backup_path(workspace), old)?;
        }
        write_atomic(&path, doc.to_pretty().as_bytes())?;
    }
    info(workspace)?.ok_or_else(|| Error::Corrupt("profile unreadable right after saving".into()))
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

/// A profile text as an ordered document; it must be a JSON object.
fn parse_doc(text: &str) -> std::result::Result<Json, InvalidInput> {
    let doc: Json = serde_json::from_str(text).map_err(|e| InvalidInput::ProfileNotJson {
        line: e.line(),
        column: e.column(),
    })?;
    if doc.is_object() {
        Ok(doc)
    } else {
        Err(InvalidInput::ProfileNotObject {
            found: json_kind(&doc.to_value()).into(),
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
    use crate::matching::CriterionKey;

    fn workspace() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn store(dir: &Path, text: &str) {
        std::fs::create_dir_all(dir.join(PROFILE_DIR)).unwrap();
        std::fs::write(profile_path(dir), text).unwrap();
    }

    fn stored(dir: &Path) -> String {
        std::fs::read_to_string(profile_path(dir)).unwrap()
    }

    fn keys(text: &str) -> Vec<String> {
        match serde_json::from_str::<Json>(text).unwrap() {
            Json::Object(entries) => entries.into_iter().map(|(k, _)| k).collect(),
            _ => Vec::new(),
        }
    }

    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/matching")
            .join(name);
        std::fs::read_to_string(path).unwrap()
    }

    /// Without origins (a form read back from a saved file has them, a new one has not).
    fn plain(form: &ProfileForm) -> ProfileForm {
        let mut form = form.normalized();
        for row in &mut form.competences {
            row.origin = None;
        }
        for row in &mut form.languages {
            row.origin = None;
        }
        form
    }

    /// A hand-made profile: keys the form does not know, in an unusual order, with an
    /// unknown key inside a competence and a criterion under its English key.
    const HAND_MADE: &str = r#"{
  "zeta_notiz": "bleibt",
  "name": "Erika Beispiel",
  "stationen": [{"rolle": "Interim CFO", "jahr": 2020}],
  "kernkompetenzen": [
    {"kompetenz": "Controlling", "jahre": 18, "beschreibung": "Konzern"},
    "Treasury"
  ],
  "keywords": ["IFRS", {"frei": "Objekt"}],
  "hard_criteria": {"min_salary": "150k", "eigene_regel": true},
  "harte_kriterien": {"ausgeschlossene_vertragsarten": ["werkvertrag"], "laender": ["DE"]},
  "einsatzpraeferenzen": {"tagessatz_ab": 950, "remote": "mindestens 50 %"}
}"#;

    #[test]
    fn saving_merges_and_keeps_unknown_keys_values_and_order() {
        let dir = workspace();
        store(dir.path(), HAND_MADE);
        let before = stored_form(dir.path()).unwrap();
        assert_eq!(before.competences.len(), 2);
        assert_eq!(before.competences[0].years, Some(18));
        assert_eq!(before.criteria.min_day_rate, Some(950), "fallback read");
        assert_eq!(
            before.criteria.min_salary,
            Some(150_000),
            "English key read"
        );
        assert_eq!(before.keywords, ["IFRS"]);

        let mut after = before.clone();
        after.competences[0].years = Some(19);
        after.competences[1].aliases = vec!["Cash Management".into()];
        after.keywords.push("HGB".into());
        after.criteria.no_anue = true;
        after.criteria.min_salary = Some(160_000);
        save_form(dir.path(), None, &before, &after, &[]).unwrap();

        let text = stored(dir.path());
        assert_eq!(
            keys(&text),
            [
                "zeta_notiz",
                "name",
                "stationen",
                "kernkompetenzen",
                "keywords",
                "hard_criteria",
                "harte_kriterien",
                "einsatzpraeferenzen"
            ],
            "{text}"
        );
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["zeta_notiz"], "bleibt");
        assert_eq!(value["stationen"][0]["jahr"], 2020);
        let first = &value["kernkompetenzen"][0];
        assert_eq!(first["jahre"], 19);
        assert_eq!(first["beschreibung"], "Konzern", "unknown key in an entry");
        assert_eq!(
            value["kernkompetenzen"][1],
            serde_json::json!({"kompetenz": "Treasury", "auch": ["Cash Management"]}),
            "a text entry with aliases becomes an object"
        );
        assert_eq!(
            value["keywords"],
            serde_json::json!(["IFRS", "HGB", {"frei": "Objekt"}])
        );
        assert_eq!(
            value["hard_criteria"]["min_salary"], 160_000,
            "where it was"
        );
        assert_eq!(value["hard_criteria"]["eigene_regel"], true);
        assert!(value["harte_kriterien"].get("min_jahresgehalt").is_none());
        assert_eq!(
            value["harte_kriterien"]["ausgeschlossene_vertragsarten"],
            serde_json::json!(["werkvertrag", "anue"])
        );
        assert_eq!(
            value["einsatzpraeferenzen"]["tagessatz_ab"], 950,
            "untouched"
        );
        assert_eq!(value["einsatzpraeferenzen"]["remote"], "mindestens 50 %");

        // Read back, the form is what the user left.
        assert_eq!(plain(&stored_form(dir.path()).unwrap()), plain(&after));
    }

    #[test]
    fn the_previous_file_becomes_the_one_backup() {
        let dir = workspace();
        store(dir.path(), HAND_MADE);
        let before = stored_form(dir.path()).unwrap();
        let mut first = before.clone();
        first.title = "Interim CFO".into();
        save_form(dir.path(), None, &before, &first, &[]).unwrap();
        assert_eq!(
            std::fs::read_to_string(backup_path(dir.path())).unwrap(),
            HAND_MADE
        );
        let saved_once = stored(dir.path());
        let mut second = first.clone();
        second.title = "CFO".into();
        save_form(dir.path(), None, &first, &second, &[]).unwrap();
        assert_eq!(
            std::fs::read_to_string(backup_path(dir.path())).unwrap(),
            saved_once
        );
        let mut files: Vec<String> = std::fs::read_dir(dir.path().join(PROFILE_DIR))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(files, [PROFILE_FILE, BACKUP_FILE], "exactly one backup");

        // Removing the profile makes it the backup; restoring brings it back as it was.
        let last = stored(dir.path());
        assert!(remove(dir.path()).unwrap());
        assert!(info(dir.path()).unwrap().is_none(), "no profile");
        assert_eq!(
            std::fs::read_to_string(backup_path(dir.path())).unwrap(),
            last
        );
        assert!(!remove(dir.path()).unwrap(), "nothing left to remove");
        assert!(restore(dir.path()).unwrap());
        assert_eq!(stored(dir.path()), last);
        assert!(!backup_path(dir.path()).exists());
        assert!(!restore(dir.path()).unwrap(), "a profile is there already");

        // "Reset everything" after a removal leaves neither the profile nor its backup.
        assert!(remove(dir.path()).unwrap());
        let data = dir.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let plan = crate::reset::ResetPlan {
            workspace: dir.path().to_path_buf(),
            txt_names: Vec::new(),
        };
        crate::reset::request(&data, &plan).unwrap();
        let vault = crate::secrets::Vault::for_tests("profile-remove-reset");
        let report = crate::reset::perform_pending(&data, &vault).unwrap();
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(!dir.path().join(PROFILE_DIR).exists(), "nothing left");
    }

    #[test]
    fn an_unchanged_form_leaves_the_file_as_written() {
        let dir = workspace();
        let compact = "{\"keywords\":[\"SAP\"],\"name\":\"X\"}";
        store(dir.path(), compact);
        let form = stored_form(dir.path()).unwrap();
        save_form(dir.path(), None, &form, &form, &[]).unwrap();
        assert_eq!(stored(dir.path()), compact, "not even reformatted");
        assert!(!backup_path(dir.path()).exists());
    }

    #[test]
    fn invalid_input_is_refused_with_a_code_and_nothing_is_written() {
        let dir = workspace();
        let empty = ProfileForm::default();
        let refused = |source: Option<&str>, form: &ProfileForm| -> InvalidInput {
            match save_form(dir.path(), source, &empty, form, &[]).unwrap_err() {
                Error::Invalid(input) => input,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(
            refused(Some("{\"name\": }"), &empty),
            InvalidInput::ProfileNotJson {
                line: 1,
                column: 10
            }
        );
        assert_eq!(
            refused(Some("[1, 2]"), &empty),
            InvalidInput::ProfileNotObject {
                found: "array".into()
            }
        );
        let mut rate = empty.clone();
        rate.criteria.min_day_rate = Some(1_000_000);
        assert_eq!(
            refused(Some("{}"), &rate),
            InvalidInput::ProfileValue {
                field: "minDayRate".into(),
                row: None,
                max: Some(100_000)
            }
        );
        let mut date = empty.clone();
        date.criteria.available = ProfileAvailability::From {
            date: "31.02.2026".into(),
        };
        assert_eq!(
            refused(Some("{}"), &date),
            InvalidInput::ProfileValue {
                field: "available".into(),
                row: None,
                max: None
            }
        );
        assert!(info(dir.path()).unwrap().is_none(), "nothing stored");

        // A chosen file that is no profile says why.
        let file = dir.path().join("x.json");
        for (bytes, expected) in [
            (&b"{\"name\": }"[..], "profileNotJson"),
            (&b"[1]"[..], "profileNotObject"),
            (&[0xff, 0xfe, 0x00][..], "profileNotUtf8"),
        ] {
            std::fs::write(&file, bytes).unwrap();
            let error = draft_from_file(&file).unwrap_err();
            let info = crate::error::ErrorInfo::from(&error);
            assert_eq!(info.params["reason"], expected, "{error:?}");
        }
    }

    #[test]
    fn a_chosen_file_fills_the_form_and_saving_keeps_its_keys() {
        let dir = workspace();
        store(dir.path(), "{\"name\": \"Alt\", \"hobbys\": [\"Segeln\"]}");
        let file = dir.path().join("Profil_Neu.json");
        std::fs::write(&file, format!("\u{feff}{HAND_MADE}")).unwrap();
        let draft = draft_from_file(&file).unwrap();
        assert_eq!(draft.form.name, "Erika Beispiel");
        assert_eq!(draft.quality, ProfileQuality::Thin);
        assert!(!draft.source.starts_with('\u{feff}'));
        save_form(
            dir.path(),
            Some(&draft.source),
            &draft.form,
            &draft.form,
            &[],
        )
        .unwrap();
        let text = stored(dir.path());
        assert!(
            !text.contains("hobbys"),
            "the chosen file replaces the old one"
        );
        assert!(text.contains("\"zeta_notiz\": \"bleibt\""));
        assert!(backup_path(dir.path()).exists());
    }

    #[test]
    fn answers_are_read_from_code_blocks_and_chatter() {
        let json = "{\"name\": \"Erika\", \"kernkompetenzen\": [{\"kompetenz\": \"Controlling\"}]}";
        for answer in [
            json.to_owned(),
            format!("```json\n{json}\n```"),
            format!("Hier ist das Profil.\n\n```\n{json}\n```\n\nViel Erfolg."),
            format!("Gern, hier ist es: {json} Sag Bescheid."),
            format!("```json\n{{kaputt\n```\nOder so\n```json\n{json}\n```"),
        ] {
            let draft = draft_from_answer(&answer).unwrap_or_else(|e| panic!("{e:?}: {answer}"));
            assert_eq!(draft.form.competences[0].name, "Controlling");
            assert_eq!(draft.source, json);
        }
        for answer in [
            "",
            "Das kann ich nicht.",
            "```json\n{\"foo\": 1}\n```",
            "[1, 2]",
        ] {
            assert_eq!(
                draft_from_answer(answer),
                Err(InvalidInput::ProfileAnswer),
                "{answer}"
            );
        }
    }

    /// An update from a CV: the form is the answer's (the editor merges it), the JSON to save
    /// into is the stored profile with the answer's career stations in place of its own; the
    /// save keeps the user's settings and every other key.
    #[test]
    fn an_update_takes_the_stations_of_the_answer_into_the_stored_profile() {
        let dir = workspace();
        store(dir.path(), HAND_MADE);
        let answer = "Hier ist das Profil.\n```json\n{\n  \"name\": \"Erika Beispiel\",\n  \
                      \"kernkompetenzen\": [{\"kompetenz\": \"Treasury\", \"jahre\": 9}],\n  \
                      \"stationen\": [{\"zeitraum\": \"01/2021 bis heute\", \"rolle\": \"CFO\", \
                      \"schwerpunkte\": [\"Treasury\"]}]\n}\n```";
        let stations = serde_json::json!([
            {"zeitraum": "01/2021 bis heute", "rolle": "CFO", "schwerpunkte": ["Treasury"]}
        ]);
        let draft = update_from_answer(dir.path(), answer).unwrap();
        assert_eq!(draft.form.competences[0].name, "Treasury");
        assert_eq!(draft.form.competences[0].years, Some(9));
        assert_eq!(keys(&draft.source), keys(HAND_MADE), "the stored order");
        let source: Value = serde_json::from_str(&draft.source).unwrap();
        assert_eq!(source["stationen"], stations);
        assert_eq!(source["zeta_notiz"], "bleibt");

        // The editor's merge (years of a stored competence) saved into it.
        let before = stored_form(dir.path()).unwrap();
        let mut after = before.clone();
        after.competences[1].years = Some(9);
        save_form(dir.path(), Some(&draft.source), &before, &after, &[]).unwrap();
        let saved = load(dir.path()).unwrap().unwrap();
        let original: Value = serde_json::from_str(HAND_MADE).unwrap();
        assert_eq!(saved["stationen"], stations);
        assert_eq!(saved["kernkompetenzen"][1]["jahre"], 9);
        for key in [
            "harte_kriterien",
            "hard_criteria",
            "einsatzpraeferenzen",
            "keywords",
        ] {
            assert_eq!(saved[key], original[key], "{key}");
        }

        // Without stations the stored text stays as it is.
        let text = stored(dir.path());
        let draft = update_from_answer(dir.path(), "{\"kernkompetenzen\": [\"Recht\"]}").unwrap();
        assert_eq!(draft.source, text);
        // A profile without stations gets them before the settings.
        store(
            dir.path(),
            "{\"name\": \"A\", \"einsatzpraeferenzen\": {\"remote\": \"voll\"}}",
        );
        let draft = update_from_answer(dir.path(), answer).unwrap();
        assert_eq!(
            keys(&draft.source),
            ["name", "stationen", "einsatzpraeferenzen"]
        );
        // Without a stored profile it is the draft of a new one.
        assert!(remove(dir.path()).unwrap());
        assert_eq!(
            update_from_answer(dir.path(), answer).unwrap(),
            draft_from_answer(answer).unwrap()
        );
        // An answer without a profile is refused as before.
        assert!(matches!(
            update_from_answer(dir.path(), "Nein."),
            Err(Error::Invalid(InvalidInput::ProfileAnswer))
        ));
    }

    /// The prompt updates only a stored profile that holds something of a CV.
    #[test]
    fn the_update_prompt_needs_a_profile_from_a_cv() {
        let dir = workspace();
        let new = cv_prompt(None, Language::De);
        assert!(new.contains("Bitte erstelle"), "{new}");
        assert_eq!(cv_prompt(Some(dir.path()), Language::De), new, "no profile");
        store(dir.path(), "{\"harte_kriterien\": {\"laender\": [\"DE\"]}}");
        assert_eq!(
            cv_prompt(Some(dir.path()), Language::De),
            new,
            "settings only"
        );
        store(dir.path(), HAND_MADE);
        let update = cv_prompt(Some(dir.path()), Language::En);
        assert!(update.contains("Please update"), "{update}");
        assert!(
            update.contains("{ \"kompetenz\": \"Controlling\", \"jahre\": 18 }"),
            "{update}"
        );
    }

    /// Every field of the form filled.
    fn full_form() -> ProfileForm {
        let row = |name: &str, years: Option<u32>, aliases: &[&str]| ProfileCompetence {
            name: name.into(),
            years,
            aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
            origin: None,
        };
        let texts = |items: &[&str]| items.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>();
        ProfileForm {
            name: "Erika Beispiel".into(),
            title: "Interim Managerin Finanzen".into(),
            competences: vec![
                row("Controlling", Some(12), &["Financial Controlling"]),
                row("Konzernrechnungslegung nach IFRS", Some(8), &[]),
                row("Liquiditätsplanung", None, &[]),
                row("Projektmanagement", Some(10), &[]),
                row("Restrukturierung", None, &["Sanierung"]),
            ],
            strengths: texts(&["Aufbau eines Konzernreportings in 100 Tagen"]),
            keywords: texts(&["IFRS", "HGB"]),
            years: Some(15),
            degrees: texts(&["Diplom-Kauffrau (Univ.)"]),
            industries: texts(&["Maschinenbau"]),
            tools: texts(&["SAP S/4HANA", "Power BI"]),
            certificates: texts(&["PMP"]),
            languages: vec![
                ProfileLanguage {
                    language: "Deutsch".into(),
                    level: Some(LanguageLevel::Native),
                    origin: None,
                },
                ProfileLanguage {
                    language: "Englisch".into(),
                    level: Some(LanguageLevel::C1),
                    origin: None,
                },
            ],
            focus: texts(&["Controlling", "Restrukturierung"]),
            roles: texts(&["Interim CFO", "Head of Controlling"]),
            wishes: ProfileWishes {
                day_rate: Some(1_100),
                remote: Some(RemoteWish::Mostly),
                regions: texts(&["Hamburg"]),
                industries: texts(&["Chemie"]),
            },
            criteria: ProfileCriteria {
                min_day_rate: Some(900),
                countries: texts(&["DE", "AT", "CH"]),
                no_anue: true,
                no_permanent: true,
                available: ProfileAvailability::From {
                    date: "2026-11-01".into(),
                },
                remote_outside: true,
                target_years: Some(8),
                min_salary: Some(120_000),
                permanent_places: texts(&["Hamburg", "Berlin"]),
                permanent_remote_min: Some(50),
            },
        }
    }

    /// A new profile from the empty form: the keys in the order of the skill's template and
    /// an engine that understands all of it (good quality, every criterion, no warning).
    #[test]
    fn a_new_profile_from_the_empty_form() {
        let dir = workspace();
        let after = full_form();
        save_form(dir.path(), Some("{}"), &ProfileForm::default(), &after, &[]).unwrap();
        let text = stored(dir.path());
        assert_eq!(
            keys(&text),
            [
                "name",
                "titel",
                "wunschrollen",
                "berufserfahrung_jahre",
                "abschluss",
                "kernkompetenzen",
                "schwerpunkte",
                "methoden_tools",
                "zertifizierungen",
                "branchen",
                "sprachen",
                "alleinstellungsmerkmale",
                "keywords",
                "einsatzpraeferenzen",
                "harte_kriterien"
            ]
        );
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value["einsatzpraeferenzen"],
            serde_json::json!({
                "tagessatz_wunsch": 1100,
                "remote": "ueberwiegend",
                "regionen": ["Hamburg"],
                "branchen": ["Chemie"]
            }),
            "wishes are plain values, the remote wish one of four words"
        );
        assert_eq!(
            value["branchen"],
            serde_json::json!([{"branche": "Maschinenbau"}])
        );
        assert!(text.contains("\"verfuegbar_ab\": \"01.11.2026\""), "{text}");
        assert!(text.contains("\"niveau\": \"Muttersprache\""), "{text}");
        assert_eq!(plain(&stored_form(dir.path()).unwrap()), plain(&after));

        let value = load(dir.path()).unwrap().unwrap();
        let profile = matching::compile_profile(&value);
        assert_eq!(profile.quality(), ProfileQuality::Good);
        let summary = profile.summary();
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        let set: Vec<CriterionKey> = summary
            .criteria
            .iter()
            .filter(|c| c.set)
            .map(|c| c.key)
            .collect();
        assert_eq!(
            set,
            [
                CriterionKey::MinDayRate,
                CriterionKey::Countries,
                CriterionKey::NoAnue,
                CriterionKey::NoPermanent,
                CriterionKey::Availability,
                CriterionKey::MinSalary,
                CriterionKey::PermanentRegion,
                CriterionKey::TargetYears
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
        assert!(
            !summary.competences.iter().any(|c| c == "Hamburg"),
            "criteria values are no competences"
        );
    }

    /// Every sample profile: the form written into an empty profile reads back the same,
    /// and saving it unchanged into the original changes nothing the engine sees.
    #[test]
    fn every_sample_profile_round_trips() {
        for name in [
            "sample_profile.json",
            "sample_profile_it.json",
            "sample_profile_sap.json",
            "sample_profile_senior.json",
            "legacy_edge_profile.json",
        ] {
            let text = fixture(name);
            let form = form_of(&text).unwrap();
            let mut fresh = Json::object();
            form::merge(&mut fresh, &ProfileForm::default(), &form, &[]);
            assert_eq!(plain(&form::read(&fresh)), plain(&form), "{name}");

            let mut same: Json = serde_json::from_str(&text).unwrap();
            let original = same.clone();
            form::merge(&mut same, &form, &form, &[]);
            assert_eq!(same, original, "{name}: an unchanged form writes nothing");
        }
    }

    #[test]
    fn degrees_live_in_abschluss_and_ausbildung() {
        let merged = |base: &str, degrees: &[&str]| -> Value {
            let mut doc: Json = serde_json::from_str(base).unwrap();
            let before = form::read(&doc);
            let mut after = before.clone();
            after.degrees = degrees.iter().map(|d| (*d).to_owned()).collect();
            form::merge(&mut doc, &before, &after, &[]);
            doc.to_value()
        };
        assert_eq!(
            merged("{}", &["Diplom-Kauffrau"]),
            serde_json::json!({"abschluss": "Diplom-Kauffrau"})
        );
        assert_eq!(
            merged("{\"abschluss\": \"Diplom\"}", &["Diplom", "MBA"]),
            serde_json::json!({"abschluss": "Diplom", "ausbildung": [{"abschluss": "MBA"}]})
        );
        let education = "{\"ausbildung\": [{\"abschluss\": \"Diplom\", \"fach\": \"BWL\"}, \
                         {\"schule\": \"Gymnasium\"}]}";
        assert_eq!(
            merged(education, &["Diplom", "Promotion"]),
            serde_json::json!({"ausbildung": [
                {"abschluss": "Diplom", "fach": "BWL"},
                {"schule": "Gymnasium"},
                {"abschluss": "Promotion"}
            ]}),
            "a kept degree keeps its other keys"
        );
        assert_eq!(
            merged(education, &[]),
            serde_json::json!({"ausbildung": [{"schule": "Gymnasium"}]}),
            "only degree entries go"
        );
    }

    #[test]
    fn clearing_a_criterion_removes_every_key_that_would_bring_it_back() {
        let mut doc: Json = serde_json::from_str(HAND_MADE).unwrap();
        let before = form::read(&doc);
        let mut after = before.clone();
        after.criteria.min_day_rate = None;
        after.criteria.min_salary = None;
        after.criteria.countries.clear();
        form::merge(&mut doc, &before, &after, &[]);
        let value = doc.to_value();
        assert!(value["einsatzpraeferenzen"].get("tagessatz_ab").is_none());
        assert!(value["hard_criteria"].get("min_salary").is_none());
        assert!(value["harte_kriterien"].get("laender").is_none());
        assert_eq!(form::read(&doc).criteria.min_day_rate, None);
        assert_eq!(value["einsatzpraeferenzen"]["remote"], "mindestens 50 %");
    }

    #[test]
    fn the_remote_wish_reads_old_free_text() {
        for (text, wish) in [
            ("voll", Some(RemoteWish::Full)),
            ("ueberwiegend", Some(RemoteWish::Mostly)),
            ("vor_ort", Some(RemoteWish::OnSite)),
            ("mindestens 50 %", Some(RemoteWish::Partly)),
            ("80 % remote", Some(RemoteWish::Mostly)),
            ("100 % Remote", Some(RemoteWish::Full)),
            ("0 %", Some(RemoteWish::OnSite)),
            ("\u{dc}berwiegend remote", Some(RemoteWish::Mostly)),
            ("hybrid", Some(RemoteWish::Partly)),
            ("gern vor Ort", Some(RemoteWish::OnSite)),
            ("Remote", Some(RemoteWish::Full)),
            ("kein Remote", Some(RemoteWish::OnSite)),
            ("no remote", Some(RemoteWish::OnSite)),
            ("egal", None),
        ] {
            assert_eq!(RemoteWish::read(text), wish, "{text}");
        }
        // Unchanged, the old text stays; a new choice writes the word.
        let mut doc: Json = serde_json::from_str(HAND_MADE).unwrap();
        let before = form::read(&doc);
        assert_eq!(before.wishes.remote, Some(RemoteWish::Partly));
        let mut after = before.clone();
        after.wishes.remote = Some(RemoteWish::Full);
        form::merge(&mut doc, &before, &after, &[]);
        assert_eq!(doc.to_value()["einsatzpraeferenzen"]["remote"], "voll");
    }

    #[test]
    fn at_most_five_focus_competences() {
        let dir = workspace();
        let mut form = ProfileForm {
            focus: (1..=6).map(|n| format!("Kompetenz {n}")).collect(),
            ..ProfileForm::default()
        };
        let error = save_form(dir.path(), Some("{}"), &ProfileForm::default(), &form, &[]);
        assert!(
            matches!(error, Err(Error::Invalid(InvalidInput::ProfileValue { ref field, .. })) if field == "focus"),
            "{error:?}"
        );
        form.focus.pop();
        save_form(dir.path(), Some("{}"), &ProfileForm::default(), &form, &[]).unwrap();
        assert_eq!(stored_form(dir.path()).unwrap().focus.len(), MAX_FOCUS);
    }

    #[test]
    fn load_skips_what_is_no_profile() {
        let dir = workspace();
        assert_eq!(load(dir.path()).unwrap(), None);
        store(dir.path(), "[1]");
        assert_eq!(load(dir.path()).unwrap(), None);
        assert_eq!(stored_form(dir.path()), None);
        store(dir.path(), "\u{feff}{\"keywords\": [\"SAP\"]}");
        assert_eq!(load(dir.path()).unwrap().unwrap()["keywords"][0], "SAP");
        store(dir.path(), "{kaputt");
        assert!(info(dir.path()).unwrap().unwrap().parse_error.is_some());
    }
}
