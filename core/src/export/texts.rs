//! User-facing text of the exported files in the app's two languages: every word the Excel
//! file and the HTML overview show. The interface has its own catalogs; these texts only end
//! up in files the user opens. They use the interface's words (glossary in `docs/PLAN.md`)
//! and its style rules - `core/tests/rust_texts.rs` checks both languages. The German words
//! stand at the top level, the English ones in [`en`] under the same names; [`Texts::of`]
//! picks by the app's language. The text files per job are no part of this: they stay
//! German (`job_txt.rs`, a contract with the matching skill).

use jiff::Timestamp;
use serde_json::{Map, Value};

use crate::settings::Language;
use crate::store::JobRow;
use crate::view::DetailState;

// User-facing text, German (the app's first language).

/// Name of the sheet with all jobs.
pub const JOBS_SHEET: &str = "Job-Alerts";
/// Name of the sheet with the run information.
pub const INFO_SHEET: &str = "Info";

/// Column headers of the Excel file (order as before, plus the job details state). Unlike
/// the text files nobody reads it by machine - so it says "Portal" like the interface, not
/// "Quelle" like the skill contract; the first sighting of a job is "Zuerst gesehen" (a
/// "saved" date would read like the favourite).
pub const COLUMNS: [&str; 18] = [
    "Portal",
    "Datum der Alert-Mail",
    "Titel",
    "Unternehmen",
    "Ort",
    "Link",
    "Betreff der Alert-Mail",
    "Alert-Mail in Gmail",
    "Zuerst gesehen",
    "Details",
    "Job-ID",
    "Passung",
    "Ausschluss",
    "Favorit",
    "Tagessatz (€)",
    "Start",
    "Dauer (Monate)",
    "Remote (%)",
];

/// Label and warning of the last row of the info sheet. Not only a fetch writes the file: a
/// rescore, a details run and "Endgültig löschen" do too.
pub const INFO_NOTE_LABEL: &str = "Hinweis";
pub const INFO_NOTE: &str =
    "Die App schreibt diese Datei immer wieder neu, eigene Notizen gehen dabei verloren.";

/// Labels of the info sheet (the mail address is deliberately not among them). The mailbox
/// is "gelesen" like "Ganzes Postfach lesen" in the interface; "Abruf" is the whole run.
pub const INFO_LAST_SCAN: &str = "Postfach zuletzt gelesen";
pub const INFO_SCOPE: &str = "Umfang beim letzten Lesen des Postfachs";
pub const INFO_NEW: &str = "Neu beim letzten Lesen des Postfachs";
pub const INFO_KNOWN: &str = "Schon bekannt beim letzten Lesen des Postfachs";
pub const INFO_DUP: &str = "In mehreren Alert-Mails beim letzten Lesen des Postfachs";
pub const INFO_JOBS_TOTAL: &str = "Jobs gesamt";
pub const INFO_PROGRAM: &str = "Programm";
pub const PROGRAM_NAME: &str = "Job-Alert-Monitor";

/// Scope of a mailbox scan in words.
pub const SCOPE_NEW: &str = "Neu seit dem letzten Abruf";
pub const SCOPE_ALL: &str = "Ganzes Postfach";

/// Words of the HTML overview. "Übersicht" names this file only, like "Übersicht öffnen" in
/// the interface; the favourites are "Favoriten" like its facet, the new matches "Neu und
/// passend" like the day overview's section.
pub const HTML_TITLE: &str = "Übersicht";
pub const HTML_PINNED: &str = "Favoriten";
pub const HTML_NEW: &str = "Neu und passend";
pub const HTML_CREATED: &str = "Erstellt am";
pub const HTML_EMPTY: &str = "Keine neuen passenden Jobs.";

/// Under a cut list of new matches: how many more the app lists.
pub fn html_more(count: usize) -> String {
    if count == 1 {
        "1 weiterer Job in der App.".to_owned()
    } else {
        format!("{} weitere Jobs in der App.", group(count, '.'))
    }
}
pub const HTML_MATCH: &str = "Passung";
pub const HTML_MET: &str = "Erfüllt";
pub const HTML_EXCLUDED: &str = "Ausgeschlossen";
/// A job whose portal showed only the start of its ad (the list's badge in the app).
pub const HTML_TEASER: &str = "Nur Anriss";
pub const HTML_UNSCORABLE: &str = "Nicht bewertbar";
/// A job not scored yet (also one that waits for its details), like the app's ring.
pub const HTML_NONE: &str = "Noch nicht bewertet";

/// Why a job is excluded, by the code of its first violation (the list's `note`), in the
/// words of the interface's criteria. `None` for a code without a text: the overview then
/// says only "Ausgeschlossen" - never the code itself.
pub fn exclusion_reason(code: &str, params: &Map<String, Value>) -> Option<&'static str> {
    Some(match code {
        "dayRate" => "Der Tagessatz liegt unter dem Minimum im Profil.",
        "country" => "Der Einsatzort liegt außerhalb der Länder im Profil.",
        "anue" => "Die Anzeige nennt Arbeitnehmerüberlassung.",
        "permanent" => "Der Job ist eine Festanstellung, das Profil schließt sie aus.",
        "availability" => "Der Start passt nicht zur Verfügbarkeit.",
        "salary" => "Das Gehalt liegt unter dem Minimum im Profil.",
        "permanentRegion" => "Der Ort liegt außerhalb der Orte für Festanstellung.",
        "tooJunior" => "Der Job verlangt deutlich weniger Erfahrung.",
        "formalOpen" if licence(params) => {
            "Die Anzeige verlangt eine Zulassung, die das Profil nicht nennt."
        }
        "formalOpen" => "Die Anzeige verlangt einen Abschluss, den das Profil nicht nennt.",
        "hardCriterion" => "Ein Ausschlusskriterium greift.",
        _ => return None,
    })
}

/// State of the job details (see [`DetailState`]) in the words of the interface's badges
/// (`job.detail`, `job.closed`); a full text has no badge there and is "Vorhanden" here.
pub fn details_label(detail: DetailState, closed: bool, short: bool) -> &'static str {
    match detail {
        DetailState::Ok if closed => "Keine Bewerbung mehr möglich",
        DetailState::Ok if short => "Vorhanden (kurz)",
        DetailState::Ok => "Vorhanden",
        DetailState::Pending { .. } => "Details folgen",
        DetailState::OnRequest => "Details auf Anfrage",
        DetailState::Teaser => "Nur Anriss",
        DetailState::Failed { .. } => "Details fehlen",
        DetailState::Gone => "Nicht mehr online",
        DetailState::Unfetchable => "Nicht erreichbar",
    }
}
// end of user-facing text

/// The English words of the files, under the German names.
pub mod en {
    use serde_json::{Map, Value};

    use super::licence;
    use crate::view::DetailState;

    // User-facing text, English.

    pub const JOBS_SHEET: &str = "Job alerts";
    pub const INFO_SHEET: &str = "Info";

    pub const COLUMNS: [&str; super::COLUMNS.len()] = [
        "Portal",
        "Alert email date",
        "Title",
        "Company",
        "Location",
        "Link",
        "Alert email subject",
        "Alert email in Gmail",
        "First seen",
        "Details",
        "Job ID",
        "Match",
        "Exclusion",
        "Favourite",
        "Day rate (€)",
        "Start",
        "Duration (months)",
        "Remote (%)",
    ];

    pub const INFO_NOTE_LABEL: &str = "Note";
    pub const INFO_NOTE: &str =
        "The app rewrites this file from time to time, so notes added here are lost.";

    pub const INFO_LAST_SCAN: &str = "Mailbox last read";
    pub const INFO_SCOPE: &str = "Scope of the last mailbox read";
    pub const INFO_NEW: &str = "New at the last mailbox read";
    pub const INFO_KNOWN: &str = "Already known at the last mailbox read";
    pub const INFO_DUP: &str = "In several alert emails at the last mailbox read";
    pub const INFO_JOBS_TOTAL: &str = "Jobs in total";
    pub const INFO_PROGRAM: &str = "Program";

    pub const SCOPE_NEW: &str = "New since the last fetch";
    pub const SCOPE_ALL: &str = "Whole mailbox";

    pub const HTML_TITLE: &str = "Overview";
    pub const HTML_PINNED: &str = "Favourites";
    pub const HTML_NEW: &str = "Best new matches";
    pub const HTML_CREATED: &str = "Created on";
    pub const HTML_EMPTY: &str = "No new matching jobs.";

    pub fn html_more(count: usize) -> String {
        if count == 1 {
            "1 more job in the app.".to_owned()
        } else {
            format!("{} more jobs in the app.", super::group(count, ','))
        }
    }
    pub const HTML_MATCH: &str = "Match";
    pub const HTML_MET: &str = "Met";
    pub const HTML_EXCLUDED: &str = "Excluded";
    pub const HTML_TEASER: &str = "Teaser only";
    pub const HTML_UNSCORABLE: &str = "Not scorable";
    pub const HTML_NONE: &str = "Not scored yet";

    pub fn exclusion_reason(code: &str, params: &Map<String, Value>) -> Option<&'static str> {
        Some(match code {
            "dayRate" => "The day rate is below the minimum in the profile.",
            "country" => "The location is outside the countries in the profile.",
            "anue" => "The ad mentions temporary agency work.",
            "permanent" => "This is a permanent job, which the profile excludes.",
            "availability" => "The start does not fit the availability.",
            "salary" => "The salary is below the minimum in the profile.",
            "permanentRegion" => "The location is outside your locations for permanent jobs.",
            "tooJunior" => "The job asks for much less experience.",
            "formalOpen" if licence(params) => {
                "The ad requires a licence the profile does not name."
            }
            "formalOpen" => "The ad requires a degree the profile does not name.",
            "hardCriterion" => "An exclusion criterion applies.",
            _ => return None,
        })
    }

    pub fn details_label(detail: DetailState, closed: bool, short: bool) -> &'static str {
        match detail {
            DetailState::Ok if closed => "No longer taking applications",
            DetailState::Ok if short => "Available (short)",
            DetailState::Ok => "Available",
            DetailState::Pending { .. } => "Details to come",
            DetailState::OnRequest => "Details on request",
            DetailState::Teaser => "Teaser only",
            DetailState::Failed { .. } => "Details missing",
            DetailState::Gone => "No longer online",
            DetailState::Unfetchable => "Not fetchable",
        }
    }
    // end of user-facing text
}

/// A count with its thousands grouped like the app's numbers (`1.234`, `1,234`).
fn group(count: usize, separator: char) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(separator);
        }
        out.push(digit);
    }
    out
}

/// Does a `formalOpen` violation name a licence (not a degree)?
fn licence(params: &Map<String, Value>) -> bool {
    params.get("class").and_then(Value::as_str) == Some("licence")
}

/// Words of the info sheet an earlier version stored with the last mailbox scan in a wording
/// of this file that changed since, and the German word of that row today - do not
/// translate.
const FORMER_WORDS: [(&str, &str); 7] = [
    (
        "Doppelt in mehreren Alert-Mails beim letzten Postfach-Abruf",
        INFO_DUP,
    ),
    ("Letzter Postfach-Abruf", INFO_LAST_SCAN),
    ("Umfang des letzten Postfach-Abrufs", INFO_SCOPE),
    ("Neu beim letzten Postfach-Abruf", INFO_NEW),
    ("Schon bekannt beim letzten Postfach-Abruf", INFO_KNOWN),
    (
        "In mehreren Alert-Mails beim letzten Postfach-Abruf",
        INFO_DUP,
    ),
    ("Alle", SCOPE_ALL),
];

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
    pub info_jobs_total: &'static str,
    pub info_program: &'static str,
    pub scope_new: &'static str,
    pub scope_all: &'static str,
    pub html_title: &'static str,
    pub html_pinned: &'static str,
    pub html_new: &'static str,
    pub html_created: &'static str,
    pub html_empty: &'static str,
    /// Under a cut list of new matches: how many more the app lists.
    pub html_more: fn(usize) -> String,
    pub html_match: &'static str,
    pub html_met: &'static str,
    pub html_excluded: &'static str,
    pub html_teaser: &'static str,
    pub html_unscorable: &'static str,
    pub html_none: &'static str,
    /// A moment as text (`strftime`): `19.09.2026 14:05`, `19/09/2026 14:05`.
    pub moment: &'static str,
    /// The number format of the date cells in Excel.
    pub excel_moment: &'static str,
    /// The Excel cells of a favourite and of an ad's start (`now`, `vague`).
    pub cell_yes: &'static str,
    pub start_now: &'static str,
    pub start_open: &'static str,
    exclusion: fn(&str, &Map<String, Value>) -> Option<&'static str>,
    details: fn(DetailState, bool, bool) -> &'static str,
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
    info_jobs_total: INFO_JOBS_TOTAL,
    info_program: INFO_PROGRAM,
    scope_new: SCOPE_NEW,
    scope_all: SCOPE_ALL,
    html_title: HTML_TITLE,
    html_pinned: HTML_PINNED,
    html_new: HTML_NEW,
    html_created: HTML_CREATED,
    html_empty: HTML_EMPTY,
    html_more,
    html_match: HTML_MATCH,
    html_met: HTML_MET,
    html_excluded: HTML_EXCLUDED,
    html_teaser: HTML_TEASER,
    html_unscorable: HTML_UNSCORABLE,
    html_none: HTML_NONE,
    moment: "%d.%m.%Y %H:%M",
    excel_moment: "dd.mm.yyyy hh:mm",
    cell_yes: "Ja",
    start_now: "ab sofort",
    start_open: "offen",
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
    info_jobs_total: en::INFO_JOBS_TOTAL,
    info_program: en::INFO_PROGRAM,
    scope_new: en::SCOPE_NEW,
    scope_all: en::SCOPE_ALL,
    html_title: en::HTML_TITLE,
    html_pinned: en::HTML_PINNED,
    html_new: en::HTML_NEW,
    html_created: en::HTML_CREATED,
    html_empty: en::HTML_EMPTY,
    html_more: en::html_more,
    html_match: en::HTML_MATCH,
    html_met: en::HTML_MET,
    html_excluded: en::HTML_EXCLUDED,
    html_teaser: en::HTML_TEASER,
    html_unscorable: en::HTML_UNSCORABLE,
    html_none: en::HTML_NONE,
    moment: "%d/%m/%Y %H:%M",
    excel_moment: "dd/mm/yyyy hh:mm",
    cell_yes: "Yes",
    start_now: "now",
    start_open: "open",
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

    /// The state of a job's details at `now` (see [`details_label`]): like the list, a job
    /// whose mail is older than the automatic fetch reaches waits for a request.
    pub fn details_label(&self, job: &JobRow, now: Timestamp) -> &'static str {
        (self.details)(DetailState::at(job, now), job.desc_closed, job.desc_short)
    }

    /// A moment in local time as the files show it.
    pub fn moment(&self, ts: jiff::Timestamp) -> String {
        crate::time::local(ts).strftime(self.moment).to_string()
    }

    /// Rows of the info sheet that an earlier version stored in German: the same row in this
    /// language (labels and the scope; numbers and dates stay), also where this file words
    /// them otherwise now.
    pub fn from_german(&self, word: &str) -> Option<&'static str> {
        let word = FORMER_WORDS
            .iter()
            .find(|(former, _)| *former == word)
            .map_or(word, |(_, today)| *today);
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
                ReasonCode::Permanent,
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
        // A row stored in a wording this file used before reads as today's row.
        for (former, today) in FORMER_WORDS {
            assert_eq!(DE.from_german(former), Some(today));
            let index = DE.stored_words().iter().position(|w| *w == today).unwrap();
            assert_eq!(EN.from_german(former), Some(EN.stored_words()[index]));
        }
        assert_eq!(
            EN.from_german("Letzter Postfach-Abruf"),
            Some(en::INFO_LAST_SCAN)
        );
        assert_eq!(EN.from_german("Alle"), Some(en::SCOPE_ALL));
        for (de, en) in DE.columns.iter().zip(EN.columns) {
            // Product and loan words are the same in both.
            if !["Portal", "Link", "Details", "Start", "Remote (%)"].contains(de) {
                assert_ne!(*de, en);
            }
        }
        let ts: jiff::Timestamp = "2026-09-19T12:05:00Z".parse().unwrap();
        assert_eq!(DE.moment(ts), "19.09.2026 14:05");
        assert_eq!(EN.moment(ts), "19/09/2026 14:05");
    }
}
