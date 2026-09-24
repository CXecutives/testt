//! `JobAlerts.xlsx`: sheet "Job-Alerts" (all jobs, newest first) and sheet "Info". The file
//! is generated completely anew on every export.

use std::path::Path;

use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook, Worksheet, XlsxError};

use super::Line;
use super::scale::{SCORE_SCALE, score_step};
use super::texts::Texts;
use crate::error::Result;
use crate::model::MatchStatus;
use crate::settings::Language;
use crate::store::JobRow;
use crate::text::truncate_chars;
use crate::time;

/// Excel takes at most this many characters per cell ...
const MAX_CELL_CHARS: usize = 32_767;
/// ... and at most this many links per sheet; above that, URLs stay text (otherwise Excel
/// reports "unreadable content" and removes all links when repairing).
const MAX_LINKS: usize = 65_530;
/// Column widths in characters (order as in `COLUMNS`).
const WIDTHS: [f64; 12] = [
    15.0, 16.0, 50.0, 32.0, 22.0, 45.0, 40.0, 20.0, 16.0, 22.0, 24.0, 10.0,
];
/// Grey of the header row and of excluded jobs.
const HEADER_GREY: u32 = 0x00E7_E6E6;
const EXCLUDED_GREY: u32 = 0x0080_8080;

/// Writes the Excel file in the app's language. `info` are label/value pairs for the sheet
/// "Info" (already in that language).
pub fn write_xlsx(
    path: &Path,
    jobs: &[JobRow],
    info: &[(String, String)],
    language: Language,
) -> Result<()> {
    let texts = Texts::of(language);
    let mut workbook = Workbook::new();
    jobs_sheet(workbook.add_worksheet(), jobs, texts)?;
    info_sheet(workbook.add_worksheet(), info, texts)?;
    let bytes = workbook.save_to_buffer()?;
    super::write_atomic(path, &bytes)
}

fn jobs_sheet(sheet: &mut Worksheet, jobs: &[JobRow], texts: &Texts) -> Result<(), XlsxError> {
    sheet.set_name(texts.jobs_sheet)?;
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(HEADER_GREY))
        .set_border_bottom(FormatBorder::Thin);
    let grey = Format::new().set_font_color(Color::RGB(EXCLUDED_GREY));
    let steps =
        SCORE_SCALE.map(|colour| Format::new().set_background_color(Color::RGB(colour.rgb())));
    let dates = [
        Format::new().set_num_format(texts.excel_moment),
        grey.clone().set_num_format(texts.excel_moment),
    ];
    for (col, (title, width)) in (0u16..).zip(texts.columns.iter().zip(WIDTHS)) {
        sheet.write_string_with_format(0, col, *title, &header)?;
        sheet.set_column_width(col, width)?;
    }
    let mut links = 0;
    for (row, job) in (1u32..).zip(jobs) {
        let line = Line::of(job, texts);
        // Excluded jobs stay in the list, grey, with their domain score.
        let excluded = job
            .match_
            .as_ref()
            .is_some_and(|m| m.status == MatchStatus::Excluded);
        if excluded {
            sheet.set_row_format(row, &grey)?;
        }
        let date = &dates[usize::from(excluded)];
        text(sheet, row, 0, line.source)?;
        if let Some(ts) = job.mail_date {
            sheet.write_datetime_with_format(row, 1, time::local(ts), date)?;
        }
        text(sheet, row, 2, &line.title)?;
        text(sheet, row, 3, &line.company)?;
        text(sheet, row, 4, &line.location)?;
        link(sheet, row, 5, &line.url, &mut links)?;
        text(sheet, row, 6, &line.subject)?;
        link(sheet, row, 7, &line.gmail_url, &mut links)?;
        sheet.write_datetime_with_format(row, 8, time::local(job.first_seen_at), date)?;
        text(sheet, row, 9, line.details)?;
        text(sheet, row, 10, &line.key)?;
        // Unscorable jobs have no number: an empty cell sorts behind every score. A scored
        // job's cell takes the ring colour of the app (ten steps, `scale.rs`); an excluded
        // one stays in the grey of its row.
        if let Some(m) = &job.match_
            && m.status != MatchStatus::Unscorable
        {
            if m.status == MatchStatus::Scored {
                let step = &steps[score_step(m.score)];
                sheet.write_number_with_format(row, 11, f64::from(m.score), step)?;
            } else {
                sheet.write_number(row, 11, f64::from(m.score))?;
            }
        }
    }
    let last_row = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
    sheet.autofilter(
        0,
        0,
        last_row,
        u16::try_from(texts.columns.len() - 1).unwrap_or(0),
    )?;
    sheet.set_freeze_panes(1, 0)?;
    Ok(())
}

/// Text cell; overlong values are cut instead of letting the whole export fail.
fn text(sheet: &mut Worksheet, row: u32, col: u16, value: &str) -> Result<(), XlsxError> {
    sheet.write_string(row, col, truncate_chars(value, MAX_CELL_CHARS))?;
    Ok(())
}

/// Link as a clickable cell; what Excel does not take as a link (length, form, number)
/// stays as text - the export never fails because of it.
fn link(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    url: &str,
    links: &mut usize,
) -> Result<(), XlsxError> {
    if url.is_empty() {
        return Ok(());
    }
    if *links < MAX_LINKS && sheet.write_url_with_text(row, col, url, url).is_ok() {
        *links += 1;
        return Ok(());
    }
    text(sheet, row, col, url)
}

fn info_sheet(
    sheet: &mut Worksheet,
    info: &[(String, String)],
    texts: &Texts,
) -> Result<(), XlsxError> {
    sheet.set_name(texts.info_sheet)?;
    let bold = Format::new().set_bold();
    sheet.set_column_width(0, 48)?;
    sheet.set_column_width(1, 64)?;
    let rows = info
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .chain(std::iter::once((texts.info_note_label, texts.info_note)));
    for (row, (label, value)) in (0u32..).zip(rows) {
        sheet.write_string_with_format(row, 0, truncate_chars(label, MAX_CELL_CHARS), &bold)?;
        text(sheet, row, 1, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use calamine::{Data, Reader, Xlsx, open_workbook};

    use super::*;
    use crate::export::texts::{
        COLUMNS, INFO_LAST_RUN, INFO_NOTE_LABEL, INFO_SHEET, JOBS_SHEET, en,
    };
    use crate::model::DescStatus;
    use crate::portal::job_link;

    fn row(url: &str, title: &str, status: DescStatus) -> JobRow {
        let link = job_link(url).unwrap();
        JobRow {
            key: link.key,
            url: link.url,
            title: title.into(),
            company: "von: Muster GmbH".into(),
            location: "D-68159 Mannheim".into(),
            mail_date: Some("2026-09-18T07:05:00Z".parse().unwrap()),
            mail_subject: "=HYPERLINK(\"http://evil\")".into(),
            gmail_id: Some(0x1a2b),
            first_seen_at: "2026-09-19T08:00:00Z".parse().unwrap(),
            first_seen_run: 1,
            desc_status: status,
            desc_short: false,
            desc_closed: false,
            desc_len: 0,
            desc_fetched_at: None,
            desc_attempts: 0,
            desc_error: None,
            txt_name: None,
            desc_attempted_at: None,
            read_at: None,
            match_: None,
            match_rev: None,
            facts: None,
            pinned_at: None,
            archived_at: None,
            trashed_at: None,
            override_include: false,
        }
    }

    #[test]
    fn workbook_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(super::super::XLSX_NAME);
        let mut jobs = [
            row(
                "https://www.linkedin.com/jobs/view/4000000001/",
                "Interim CFO",
                DescStatus::Ok,
            ),
            row(
                "https://www.freelance.de/projekte/projekt-1288981-SAP",
                "SAP-Berater",
                DescStatus::Missing,
            ),
        ];
        let scored = |status, score| crate::model::MatchRecord {
            status,
            score,
            note: None,
            must_met: 0,
            must_total: 0,
            top: Vec::new(),
            facts: crate::model::KeyFacts::default(),
        };
        jobs[0].match_ = Some(scored(MatchStatus::Scored, 83));
        jobs[1].match_ = Some(scored(MatchStatus::Excluded, 71));
        let info = [(INFO_LAST_RUN.to_string(), "x".to_string())];
        write_xlsx(&path, &jobs, &info, Language::De).unwrap();

        let mut book: Xlsx<_> = open_workbook(&path).unwrap();
        assert_eq!(book.sheet_names(), [JOBS_SHEET, INFO_SHEET]);
        let range = book.worksheet_range(JOBS_SHEET).unwrap();
        let header: Vec<String> = range
            .rows()
            .next()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(header, COLUMNS);
        let first: Vec<&Data> = range.rows().nth(1).unwrap().iter().collect();
        assert_eq!(first[0].to_string(), "linkedin.com");
        assert!(
            matches!(first[1], Data::DateTime(_)),
            "the mail date is an Excel date"
        );
        assert_eq!(first[2].to_string(), "Interim CFO");
        assert_eq!(first[3].to_string(), "Muster GmbH");
        assert_eq!(first[4].to_string(), "Mannheim");
        assert_eq!(
            first[5].to_string(),
            "https://www.linkedin.com/jobs/view/4000000001/"
        );
        // A mail subject with a formula stays text and is never executed.
        assert_eq!(first[6].to_string(), "=HYPERLINK(\"http://evil\")");
        assert_eq!(
            first[7].to_string(),
            "https://mail.google.com/mail/u/0/#all/1a2b"
        );
        assert_eq!(first[9].to_string(), "Vorhanden");
        assert_eq!(first[10].to_string(), "linkedin:4000000001");
        // "Passung" last: a number, for excluded jobs the domain score.
        assert_eq!(first[11], &Data::Float(83.0));
        assert_eq!(range.get((2, 11)), Some(&Data::Float(71.0)));
        assert_eq!(range.rows().count(), 3);
        let info = book.worksheet_range(INFO_SHEET).unwrap();
        assert_eq!(info.get((0, 1)).unwrap().to_string(), "x");
        assert_eq!(info.get((1, 0)).unwrap().to_string(), INFO_NOTE_LABEL);
    }

    /// In English the sheets, headers and word cells are English; data stays as it came.
    #[test]
    fn workbook_in_english() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(super::super::XLSX_NAME);
        let job = row(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "Interim CFO",
            DescStatus::Teaser,
        );
        let info = [(en::INFO_LAST_RUN.to_string(), "x".to_string())];
        write_xlsx(&path, &[job], &info, Language::En).unwrap();

        let mut book: Xlsx<_> = open_workbook(&path).unwrap();
        assert_eq!(book.sheet_names(), [en::JOBS_SHEET, en::INFO_SHEET]);
        let range = book.worksheet_range(en::JOBS_SHEET).unwrap();
        let header: Vec<String> = range
            .rows()
            .next()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(header, en::COLUMNS);
        let first: Vec<&Data> = range.rows().nth(1).unwrap().iter().collect();
        assert_eq!(first[3].to_string(), "Muster GmbH");
        assert_eq!(first[9].to_string(), "Teaser only");
        let info = book.worksheet_range(en::INFO_SHEET).unwrap();
        assert_eq!(info.get((0, 0)).unwrap().to_string(), en::INFO_LAST_RUN);
        assert_eq!(info.get((1, 0)).unwrap().to_string(), en::INFO_NOTE_LABEL);
    }

    #[test]
    fn empty_workbook_has_header_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("leer.xlsx");
        write_xlsx(&path, &[], &[], Language::De).unwrap();
        let mut book: Xlsx<_> = open_workbook(&path).unwrap();
        assert_eq!(book.worksheet_range(JOBS_SHEET).unwrap().rows().count(), 1);
    }

    /// An overlong value cuts the cell instead of letting the export fail (on every run
    /// again).
    #[test]
    fn oversized_cell_is_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lang.xlsx");
        let mut job = row(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "Interim CFO",
            DescStatus::Ok,
        );
        job.mail_subject = "x".repeat(40_000);
        write_xlsx(&path, &[job], &[], Language::De).unwrap();
        let mut book: Xlsx<_> = open_workbook(&path).unwrap();
        let range = book.worksheet_range(JOBS_SHEET).unwrap();
        let subject = range.get((1, 6)).unwrap().to_string();
        assert_eq!(subject.chars().count(), MAX_CELL_CHARS);
    }
}
