//! User-facing text, German by product decision: every word the exported Excel file and the
//! HTML overview show. The interface has its own catalog; these texts only end up in files
//! the user opens. They use the interface's words (glossary in `docs/PLAN.md`) and its style
//! rules - `core/tests/rust_texts.rs` checks both.

use serde_json::{Map, Value};

use crate::model::DescStatus;
use crate::store::JobRow;

/// Name of the sheet with all jobs.
pub const JOBS_SHEET: &str = "Job-Alerts";
/// Name of the sheet with the run information.
pub const INFO_SHEET: &str = "Info";

/// Column headers of the Excel file (order as before, plus the job details state). Unlike
/// the text files nobody reads it by machine - so it says "Portal" like the interface, not
/// "Quelle" like the skill contract.
pub const COLUMNS: [&str; 12] = [
    "Portal",
    "Datum der Alert-Mail",
    "Titel",
    "Unternehmen",
    "Ort",
    "Link",
    "Betreff der Alert-Mail",
    "Alert-Mail in Gmail",
    "Gespeichert am",
    "Details",
    "Schlüssel",
    "Passung",
];

/// Label and warning of the last row of the info sheet.
pub const INFO_NOTE_LABEL: &str = "Hinweis";
pub const INFO_NOTE: &str =
    "Diese Datei entsteht bei jedem Abruf neu, eigene Notizen gehen dabei verloren.";

/// Labels of the info sheet (the mail address is deliberately not among them).
pub const INFO_LAST_SCAN: &str = "Letzter Postfach-Abruf";
pub const INFO_SCOPE: &str = "Umfang des letzten Postfach-Abrufs";
pub const INFO_NEW: &str = "Neu beim letzten Postfach-Abruf";
pub const INFO_KNOWN: &str = "Schon bekannt beim letzten Postfach-Abruf";
pub const INFO_DUP: &str = "Doppelt in mehreren Alert-Mails beim letzten Postfach-Abruf";
pub const INFO_LAST_RUN: &str = "Letzter Abruf";
pub const INFO_JOBS_TOTAL: &str = "Jobs gesamt";
pub const INFO_PROGRAM: &str = "Programm";
pub const PROGRAM_NAME: &str = "Job-Alert-Monitor";

/// Scope of a mailbox scan in words.
pub const SCOPE_NEW: &str = "Neu seit dem letzten Abruf";
pub const SCOPE_ALL: &str = "Alle";

/// Info sheet labels and values of earlier versions, stored with the last mailbox scan: they
/// are read in today's words until the next scan stores its own. Do not translate.
pub const LEGACY_INFO: [(&str, &str); 5] = [
    ("Umfang des letzten Laufs", INFO_SCOPE),
    ("Neu (letzter Lauf)", INFO_NEW),
    ("Schon bekannt (letzter Lauf)", INFO_KNOWN),
    ("Doppelt in mehreren Mails (letzter Lauf)", INFO_DUP),
    ("Neu seit letztem Lauf", SCOPE_NEW),
];

/// Words of the HTML overview. "Übersicht" names this file only, like "Übersicht öffnen" in
/// the interface.
pub const HTML_TITLE: &str = "Übersicht";
pub const HTML_PINNED: &str = "Gemerkte Jobs";
pub const HTML_NEW: &str = "Neue passende Jobs";
pub const HTML_CREATED: &str = "Erstellt am";
pub const HTML_EMPTY: &str = "Keine neuen passenden Jobs.";
pub const HTML_MATCH: &str = "Passung";
pub const HTML_MET: &str = "Erfüllt";
pub const HTML_EXCLUDED: &str = "Ausgeschlossen";
pub const HTML_UNSCORABLE: &str = "Nicht bewertbar";

/// Why a job is excluded, by the code of its first violation (the list's `note`), in the
/// words of the interface's criteria. `None` for a code without a text: the overview then
/// says only "Ausgeschlossen" - never the code itself.
pub fn exclusion_reason(code: &str, params: &Map<String, Value>) -> Option<&'static str> {
    Some(match code {
        "dayRate" => "Der Tagessatz liegt unter dem Minimum im Profil.",
        "country" => "Der Einsatzort liegt außerhalb der Länder im Profil.",
        "anue" => "Die Anzeige nennt Arbeitnehmerüberlassung.",
        "availability" => "Der Start passt nicht zur Verfügbarkeit.",
        "salary" => "Das Gehalt liegt unter dem Minimum im Profil.",
        "permanentRegion" => "Die Festanstellung liegt außerhalb der Region im Profil.",
        "tooJunior" => "Die Stelle verlangt deutlich weniger Erfahrung.",
        "formalOpen" if params.get("class").and_then(Value::as_str) == Some("licence") => {
            "Die Anzeige verlangt eine Zulassung, die das Profil nicht nennt."
        }
        "formalOpen" => "Die Anzeige verlangt einen Abschluss, den das Profil nicht nennt.",
        "hardCriterion" => "Ein Ausschlusskriterium greift.",
        _ => return None,
    })
}

/// State of the job details in the words of the interface's badges.
pub fn details_label(job: &JobRow) -> &'static str {
    match job.desc_status {
        DescStatus::Ok if job.desc_closed => "Vorhanden (Anzeige geschlossen)",
        DescStatus::Ok if job.desc_short => "Vorhanden (kurz)",
        DescStatus::Ok => "Vorhanden",
        DescStatus::Teaser => "Nur Anriss",
        DescStatus::Missing => "Details folgen",
        DescStatus::Failed => "Abruf fehlgeschlagen",
        DescStatus::Gone => "Nicht mehr online",
        DescStatus::Unfetchable => "Nicht abrufbar",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::ReasonCode;
    use crate::pipeline::local::code_name;

    /// Every code that can exclude a job (a decided violation) has a text: the overview
    /// never shows an engine code.
    #[test]
    fn every_exclusion_code_has_a_text() {
        let none = Map::new();
        for code in [
            ReasonCode::Anue,
            ReasonCode::DayRate,
            ReasonCode::Availability,
            ReasonCode::Country,
            ReasonCode::Salary,
            ReasonCode::PermanentRegion,
            ReasonCode::TooJunior,
            ReasonCode::FormalOpen,
        ] {
            let name = code_name(&code);
            assert!(exclusion_reason(&name, &none).is_some(), "{name}");
        }
        let mut licence = Map::new();
        licence.insert("class".into(), "licence".into());
        assert_ne!(
            exclusion_reason("formalOpen", &licence),
            exclusion_reason("formalOpen", &none)
        );
        assert_eq!(exclusion_reason("somethingNew", &none), None);
    }
}
