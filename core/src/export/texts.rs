//! User-facing text of the exported files in the app's two languages: every word the Excel
//! file and the HTML overview show. The interface has its own catalogs; these texts only end
//! up in files the user opens. They use the interface's words (glossary in `docs/PLAN.md`)
//! and its style rules - `core/tests/rust_texts.rs` checks both languages. The German words
//! stand at the top level, the English ones in [`en`] under the same names; [`Texts::of`]
//! picks by the app's language. The text files per job are no part of this: they stay
//! German (`job_txt.rs`, a contract with the matching skill).

use serde_json::{Map, Value};

use crate::model::DescStatus;
use crate::settings::Language;
use crate::store::JobRow;

// User-facing text, German (the app's first language).

/// Name of the sheet with all jobs.
pub const JOBS_SHEET: &str = "Job-Alerts";
/// Name of the sheet with the run information.
pub const INFO_SHEET: &str = "Info";

/// Column headers of the Excel file (order as before, plus the job details state, the day
/// of the application and the note). Unlike the text files nobody reads it by machine - so
/// it says "Portal" like the interface, not "Quelle" like the skill contract.
pub const COLUMNS: [&str; 14] = [
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
    "Beworben am",
    "Notiz",
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
        "formalOpen" if licence(params) => {
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
        DescStatus::Failed => "Details fehlen",
        DescStatus::Gone => "Nicht mehr online",
        DescStatus::Unfetchable => "Nicht abrufbar",
    }
}
// end of user-facing text

/// The English words of the files, under the German names.
pub mod en {
    use serde_json::{Map, Value};

    use super::licence;
    use crate::model::DescStatus;
    use crate::store::JobRow;

    // User-facing text, English.

    pub const JOBS_SHEET: &str = "Job alerts";
    pub const INFO_SHEET: &str = "Info";

    pub const COLUMNS: [&str; super::COLUMNS.len()] = [
        "Portal",
        "Alert mail date",
        "Title",
        "Company",
        "Location",
        "Link",
        "Alert mail subject",
        "Alert mail in Gmail",
        "First seen",
        "Details",
        "Key",
        "Match",
        "Applied on",
        "Note",
    ];

    pub const INFO_NOTE_LABEL: &str = "Note";
    pub const INFO_NOTE: &str =
        "This file is written anew at every fetch, so notes added here are lost.";

    pub const INFO_LAST_SCAN: &str = "Last mailbox fetch";
    pub const INFO_SCOPE: &str = "Scope of the last mailbox fetch";
    pub const INFO_NEW: &str = "New at the last mailbox fetch";
    pub const INFO_KNOWN: &str = "Already known at the last mailbox fetch";
    pub const INFO_DUP: &str = "In several alert mails at the last mailbox fetch";
    pub const INFO_LAST_RUN: &str = "Last fetch";
    pub const INFO_JOBS_TOTAL: &str = "Jobs in total";
    pub const INFO_PROGRAM: &str = "Program";

    pub const SCOPE_NEW: &str = "New since the last fetch";
    pub const SCOPE_ALL: &str = "All";

    pub const HTML_TITLE: &str = "Overview";
    pub const HTML_PINNED: &str = "Saved jobs";
    pub const HTML_NEW: &str = "New matching jobs";
    pub const HTML_CREATED: &str = "Created on";
    pub const HTML_EMPTY: &str = "No new matching jobs.";
    pub const HTML_MATCH: &str = "Match";
    pub const HTML_MET: &str = "Met";
    pub const HTML_EXCLUDED: &str = "Excluded";
    pub const HTML_UNSCORABLE: &str = "Not scorable";

    pub fn exclusion_reason(code: &str, params: &Map<String, Value>) -> Option<&'static str> {
        Some(match code {
            "dayRate" => "The day rate is below the minimum in the profile.",
            "country" => "The location is outside the countries in the profile.",
            "anue" => "The ad mentions temporary agency work.",
            "availability" => "The start does not fit the availability.",
            "salary" => "The salary is below the minimum in the profile.",
            "permanentRegion" => "The permanent role is outside the region in the profile.",
            "tooJunior" => "The role asks for much less experience.",
            "formalOpen" if licence(params) => {
                "The ad requires a licence the profile does not name."
            }
            "formalOpen" => "The ad requires a degree the profile does not name.",
            "hardCriterion" => "An exclusion criterion applies.",
            _ => return None,
        })
    }

    pub fn details_label(job: &JobRow) -> &'static str {
        match job.desc_status {
            DescStatus::Ok if job.desc_closed => "Available (ad closed)",
            DescStatus::Ok if job.desc_short => "Available (short)",
            DescStatus::Ok => "Available",
            DescStatus::Teaser => "Teaser only",
            DescStatus::Missing => "Details to follow",
            DescStatus::Failed => "Details missing",
            DescStatus::Gone => "No longer online",
            DescStatus::Unfetchable => "Not fetchable",
        }
    }
    // end of user-facing text
}

/// Does a `formalOpen` violation name a licence (not a degree)?
fn licence(params: &Map<String, Value>) -> bool {
    params.get("class").and_then(Value::as_str) == Some("licence")
}

/// The words of the files in one language.
pub struct Texts {
    pub language: Language,
    pub jobs_sheet: &'static str,
    pub info_sheet: &'static str,
    pub columns: [&'static str; COLUMNS.len()],
    pub info_note_label: &'static str,
    pub info_note: &'static str,
    pub info_last_scan: &'static str,
    pub info_scope: &'static str,
    pub info_new: &'static str,
    pub info_known: &'static str,
    pub info_dup: &'static str,
    pub info_last_run: &'static str,
    pub info_jobs_total: &'static str,
    pub info_program: &'static str,
    pub scope_new: &'static str,
    pub scope_all: &'static str,
    pub html_title: &'static str,
    pub html_pinned: &'static str,
    pub html_new: &'static str,
    pub html_created: &'static str,
    pub html_empty: &'static str,
    pub html_match: &'static str,
    pub html_met: &'static str,
    pub html_excluded: &'static str,
    pub html_unscorable: &'static str,
    /// A moment as text (`strftime`): `19.09.2026 14:05`, `19/09/2026 14:05`.
    pub moment: &'static str,
    /// The number format of the date cells in Excel.
    pub excel_moment: &'static str,
    exclusion: fn(&str, &Map<String, Value>) -> Option<&'static str>,
    details: fn(&JobRow) -> &'static str,
}

/// The German words.
pub const DE: Texts = Texts {
    language: Language::De,
    jobs_sheet: JOBS_SHEET,
    info_sheet: INFO_SHEET,
    columns: COLUMNS,
    info_note_label: INFO_NOTE_LABEL,
    info_note: INFO_NOTE,
    info_last_scan: INFO_LAST_SCAN,
    info_scope: INFO_SCOPE,
    info_new: INFO_NEW,
    info_known: INFO_KNOWN,
    info_dup: INFO_DUP,
    info_last_run: INFO_LAST_RUN,
    info_jobs_total: INFO_JOBS_TOTAL,
    info_program: INFO_PROGRAM,
    scope_new: SCOPE_NEW,
    scope_all: SCOPE_ALL,
    html_title: HTML_TITLE,
    html_pinned: HTML_PINNED,
    html_new: HTML_NEW,
    html_created: HTML_CREATED,
    html_empty: HTML_EMPTY,
    html_match: HTML_MATCH,
    html_met: HTML_MET,
    html_excluded: HTML_EXCLUDED,
    html_unscorable: HTML_UNSCORABLE,
    moment: "%d.%m.%Y %H:%M",
    excel_moment: "dd.mm.yyyy hh:mm",
    exclusion: exclusion_reason,
    details: details_label,
};

/// The English words.
pub const EN: Texts = Texts {
    language: Language::En,
    jobs_sheet: en::JOBS_SHEET,
    info_sheet: en::INFO_SHEET,
    columns: en::COLUMNS,
    info_note_label: en::INFO_NOTE_LABEL,
    info_note: en::INFO_NOTE,
    info_last_scan: en::INFO_LAST_SCAN,
    info_scope: en::INFO_SCOPE,
    info_new: en::INFO_NEW,
    info_known: en::INFO_KNOWN,
    info_dup: en::INFO_DUP,
    info_last_run: en::INFO_LAST_RUN,
    info_jobs_total: en::INFO_JOBS_TOTAL,
    info_program: en::INFO_PROGRAM,
    scope_new: en::SCOPE_NEW,
    scope_all: en::SCOPE_ALL,
    html_title: en::HTML_TITLE,
    html_pinned: en::HTML_PINNED,
    html_new: en::HTML_NEW,
    html_created: en::HTML_CREATED,
    html_empty: en::HTML_EMPTY,
    html_match: en::HTML_MATCH,
    html_met: en::HTML_MET,
    html_excluded: en::HTML_EXCLUDED,
    html_unscorable: en::HTML_UNSCORABLE,
    moment: "%d/%m/%Y %H:%M",
    excel_moment: "dd/mm/yyyy hh:mm",
    exclusion: en::exclusion_reason,
    details: en::details_label,
};

impl Texts {
    /// The words of a language.
    pub fn of(language: Language) -> &'static Texts {
        match language {
            Language::De => &DE,
            Language::En => &EN,
        }
    }

    /// See [`exclusion_reason`].
    pub fn exclusion_reason(
        &self,
        code: &str,
        params: &Map<String, Value>,
    ) -> Option<&'static str> {
        (self.exclusion)(code, params)
    }

    /// See [`details_label`].
    pub fn details_label(&self, job: &JobRow) -> &'static str {
        (self.details)(job)
    }

    /// A moment in local time as the files show it.
    pub fn moment(&self, ts: jiff::Timestamp) -> String {
        crate::time::local(ts).strftime(self.moment).to_string()
    }

    /// Rows of the info sheet that an earlier version stored in German: the same row in this
    /// language (labels and the scope; numbers and dates stay).
    pub fn from_german(&self, word: &str) -> Option<&'static str> {
        let german = DE.stored_words();
        let index = german.iter().position(|w| *w == word)?;
        Some(self.stored_words()[index])
    }

    /// The words of the info sheet that are stored with the last mailbox scan.
    fn stored_words(&self) -> [&'static str; 7] {
        [
            self.info_last_scan,
            self.info_scope,
            self.info_new,
            self.info_known,
            self.info_dup,
            self.scope_new,
            self.scope_all,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::ReasonCode;
    use crate::pipeline::local::code_name;

    /// Every code that can exclude a job (a decided violation) has a text in both languages:
    /// the overview never shows an engine code.
    #[test]
    fn every_exclusion_code_has_a_text() {
        let none = Map::new();
        let mut licence = Map::new();
        licence.insert("class".into(), "licence".into());
        for texts in [&DE, &EN] {
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
                assert!(texts.exclusion_reason(&name, &none).is_some(), "{name}");
            }
            assert_ne!(
                texts.exclusion_reason("formalOpen", &licence),
                texts.exclusion_reason("formalOpen", &none)
            );
            assert_eq!(texts.exclusion_reason("somethingNew", &none), None);
        }
    }

    /// Both languages say the same things: each word has its counterpart, none is left
    /// untranslated, and German stored words find their English row.
    #[test]
    fn the_languages_match() {
        assert_eq!(Texts::of(Language::De).language, Language::De);
        assert_eq!(Texts::of(Language::En).language, Language::En);
        for (de, en) in DE.stored_words().into_iter().zip(EN.stored_words()) {
            assert_eq!(EN.from_german(de), Some(en));
            assert_eq!(DE.from_german(de), Some(de));
        }
        assert_eq!(EN.from_german("3"), None);
        for (de, en) in DE.columns.iter().zip(EN.columns) {
            // Product and loan words are the same in both.
            if !["Portal", "Link", "Details"].contains(de) {
                assert_ne!(*de, en);
            }
        }
        let ts: jiff::Timestamp = "2026-09-19T12:05:00Z".parse().unwrap();
        assert_eq!(DE.moment(ts), "19.09.2026 14:05");
        assert_eq!(EN.moment(ts), "19/09/2026 14:05");
    }
}
