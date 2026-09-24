//! User-facing text, German by product decision: every word the exported Excel file shows.
//! The interface has its own catalog; these texts only end up in files the user opens.

use crate::model::DescStatus;
use crate::store::JobRow;

/// Name of the sheet with all jobs.
pub const JOBS_SHEET: &str = "Job-Alerts";
/// Name of the sheet with the run information.
pub const INFO_SHEET: &str = "Info";

/// Column headers of the overview (order as before, plus the job details state). Unlike
/// the text files nobody reads the overview by machine - so it says "Portal" like the
/// interface, not "Quelle" like the skill contract.
pub const COLUMNS: [&str; 12] = [
    "Portal",
    "Mail-Datum",
    "Titel",
    "Unternehmen",
    "Ort",
    "Link",
    "Mail-Betreff",
    "Mail in Gmail",
    "Gespeichert am",
    "Jobdetails",
    "Schlüssel",
    "Passung",
];

/// Label and warning of the last row of the info sheet.
pub const INFO_NOTE_LABEL: &str = "Hinweis";
pub const INFO_NOTE: &str =
    "Diese Datei wird bei jedem Lauf vollständig neu erzeugt – eigene Notizen hier gehen verloren.";

/// Labels of the info sheet (the mail address is deliberately not among them).
pub const INFO_LAST_SCAN: &str = "Letzter Postfach-Abruf";
pub const INFO_SCOPE: &str = "Umfang des letzten Laufs";
pub const INFO_NEW: &str = "Neu (letzter Lauf)";
pub const INFO_KNOWN: &str = "Schon bekannt (letzter Lauf)";
pub const INFO_DUP: &str = "Doppelt in mehreren Mails (letzter Lauf)";
pub const INFO_LAST_RUN: &str = "Letzter Lauf";
pub const INFO_JOBS_TOTAL: &str = "Jobs gesamt";
pub const INFO_PROGRAM: &str = "Programm";
pub const PROGRAM_NAME: &str = "Job-Alert-Monitor";

/// Scope of a mailbox scan in words.
pub const SCOPE_NEW: &str = "Neu seit letztem Lauf";
pub const SCOPE_ALL: &str = "Alle";

/// Words of the HTML overview.
pub const HTML_TITLE: &str = "Job-Alert-Monitor – Übersicht";
pub const HTML_PINNED: &str = "Gemerkte Jobs";
pub const HTML_NEW: &str = "Neue passende Jobs";
pub const HTML_CREATED: &str = "Erstellt am";
pub const HTML_EMPTY: &str = "Keine neuen passenden Jobs.";
pub const HTML_MATCH: &str = "Passung";
pub const HTML_MET: &str = "Erfüllt";
pub const HTML_EXCLUDED: &str = "Ausgeschlossen";
pub const HTML_UNSCORABLE: &str = "Nicht bewertbar";

/// State of the job details in words.
pub fn details_label(job: &JobRow) -> &'static str {
    match job.desc_status {
        DescStatus::Ok if job.desc_closed => "vorhanden (Anzeige geschlossen)",
        DescStatus::Ok if job.desc_short => "vorhanden (kurz)",
        DescStatus::Ok => "vorhanden",
        DescStatus::Missing => "fehlt",
        DescStatus::Failed => "fehlgeschlagen – neuer Versuch folgt",
        DescStatus::Gone => "Anzeige nicht mehr abrufbar",
        DescStatus::Unfetchable => "nicht abrufbar",
    }
}
