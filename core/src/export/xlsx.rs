//! `JobAlerts.xlsx`: sheet "Job-Alerts" (all jobs, newest first) and sheet "Info". The file
//! is generated completely anew on every export.

use std::path::Path;

use jiff::Timestamp;
use rust_xlsxwriter::{
    Color, Format, FormatBorder, FormatUnderline, Url, Workbook, Worksheet, XlsxError,
};

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
/// Column widths in characters (order as in `COLUMNS`; a width `w` is `7 w + 5` px at 100 %).
/// Every header keeps clear of its filter button (about 20 px with the cell's padding), with
/// a few pixels to spare, measured in bold Calibri 11: "Datum der Alert-Mail" is 131 px and
/// needs 22, "Alert email in Gmail" 121 px and 21, "Zuerst gesehen" 92 px and 17, "Passung"
/// 49 px and 11. The longest value of the details column, "Keine Bewerbung mehr möglich",
/// is 190 px and needs 28. The exclusion holds a sentence; "Duration (months)" is the widest
/// of the key-fact headers.
const WIDTHS: [f64; 18] = [
    15.0, 22.0, 50.0, 32.0, 22.0, 45.0, 40.0, 21.0, 17.0, 28.0, 24.0, 11.0, 60.0, 13.0, 16.0, 12.0,
    20.0, 13.0,
];
/// Grey of the header row and of excluded jobs.
const HEADER_GREY: u32 = 0x00E7_E6E6;
const EXCLUDED_GREY: u32 = 0x0080_8080;

/// Writes the Excel file in the app's language. `info` are label/value pairs for the sheet
/// "Info" (already in that language). The details column says the state at the moment of
/// writing.
pub fn write_xlsx(
    path: &Path,
    jobs: &[JobRow],
    info: &[(String, String)],
    language: Language,
) -> Result<()> {
    write_xlsx_at(path, jobs, info, language, Timestamp::now())
}

/// [`write_xlsx`] as of `now`.
fn write_xlsx_at(
    path: &Path,
    jobs: &[JobRow],
    info: &[(String, String)],
    language: Language,
    now: Timestamp,
) -> Result<()> {
    let texts = Texts::of(language);
    let mut workbook = Workbook::new();
    jobs_sheet(workbook.add_worksheet(), jobs, texts, now)?;
    info_sheet(workbook.add_worksheet(), info, texts)?;
    let bytes = workbook.save_to_buffer()?;
    super::write_atomic(path, &bytes)
}

fn jobs_sheet(
    sheet: &mut Worksheet,
    jobs: &[JobRow],
    texts: &Texts,
    now: Timestamp,
) -> Result<(), XlsxError> {
    sheet.set_name(texts.jobs_sheet)?;
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(HEADER_GREY))
        .set_border_bottom(FormatBorder::Thin);
    let grey = Format::new().set_font_color(Color::RGB(EXCLUDED_GREY));
    // A link of an excluded job stays a link, in the grey of its row.
    let grey_link = grey.clone().set_underline(FormatUnderline::Single);
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
        let line = Line::of(job, texts, now);
        // Excluded jobs stay in the list, grey, with their domain score.
        let excluded = job
            .match_
            .as_ref()
            .is_some_and(|m| m.status == MatchStatus::Excluded);
        if excluded {
            sheet.set_row_format(row, &grey)?;
        }
        let date = &dates[usize::from(excluded)];
        let link_format = excluded.then_some(&grey_link);
        text(sheet, row, 0, line.source)?;
        if let Some(ts) = job.mail_date {
            sheet.write_datetime_with_format(row, 1, time::local(ts), date)?;
        }
        text(sheet, row, 2, &line.title)?;
        text(sheet, row, 3, &line.company)?;
        text(sheet, row, 4, &line.location)?;
        link(sheet, row, 5, &line.url, link_format, &mut links)?;
        text(sheet, row, 6, &line.subject)?;
        link(sheet, row, 7, &line.gmail_url, link_format, &mut links)?;
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
        // Why an excluded job is out, the favourite, and the ad's key facts as the engine read
        // them: a day rate in euros (an hourly rate x 8, a rate in another currency left
        // out), the start, the duration and the remote share.
        if excluded {
            let why = job
                .match_
                .as_ref()
                .and_then(|m| m.note.as_ref())
                .and_then(|n| texts.exclusion_reason(&n.code, &n.params))
                .unwrap_or(texts.html_excluded);
            text(sheet, row, 12, why)?;
        }
        if job.pinned_at.is_some() {
            text(sheet, row, 13, texts.cell_yes)?;
        }
        if let Some(facts) = job.match_.as_ref().map(|m| &m.facts) {
            if let Some(rate) = facts.rate
                && facts.currency.as_deref().is_none_or(|c| c == "EUR")
            {
                let day = if facts.hourly == Some(true) {
                    rate.saturating_mul(8)
                } else {
                    rate
                };
                sheet.write_number(row, 14, f64::from(day))?;
            }
            if let Some(start) = facts.start.as_deref() {
                let words = match start {
                    "now" => texts.start_now,
                    "vague" => texts.start_open,
                    date => date,
                };
                text(sheet, row, 15, words)?;
            }
            if let Some(months) = facts.months {
                sheet.write_number(row, 16, f64::from(months))?;
            }
            if let Some(from) = facts.remote_from {
                let to = facts.remote_to.unwrap_or(from);
                if from == to {
                    sheet.write_number(row, 17, f64::from(from))?;
                } else {
                    text(sheet, row, 17, &format!("{from}–{to}"))?;
                }
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

/// Link as a clickable cell, in Excel's link style or in `format`; what Excel does not take
/// as a link (length, form, number) stays as text - the export never fails because of it.
fn link(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    url: &str,
    format: Option<&Format>,
    links: &mut usize,
) -> Result<(), XlsxError> {
    if url.is_empty() {
        return Ok(());
    }
    if *links < MAX_LINKS {
        let link = Url::new(url).set_text(url);
        let written = match format {
            Some(format) => sheet.write_url_with_format(row, col, link, format),
            None => sheet.write_url(row, col, link),
        };
        if written.is_ok() {
            *links += 1;
            return Ok(());
        }
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
        COLUMNS, INFO_LAST_SCAN, INFO_NOTE_LABEL, INFO_SHEET, JOBS_SHEET, en,
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
    fn the_sheet_says_why_and_carries_the_key_facts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(super::super::XLSX_NAME);
        let mut job = row(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "Interim Controller",
            DescStatus::Ok,
        );
        job.pinned_at = Some("2026-09-19T09:00:00Z".parse().unwrap());
        job.match_ = Some(crate::model::MatchRecord {
            status: MatchStatus::Excluded,
            score: 64,
            note: Some(crate::model::Notice {
                code: "dayRate".into(),
                params: serde_json::Map::new(),
            }),
            must_met: 0,
            must_total: 0,
            top: Vec::new(),
            facts: crate::model::KeyFacts {
                rate: Some(95),
                hourly: Some(true),
                start: Some("now".into()),
                months: Some(6),
                remote_from: Some(60),
                remote_to: Some(100),
                ..crate::model::KeyFacts::default()
            },
            rank: 0,
        });
        write_xlsx(&path, &[job], &[], Language::De).unwrap();
        let mut book: Xlsx<_> = open_workbook(&path).unwrap();
        let range = book.worksheet_range(JOBS_SHEET).unwrap();
        let cells: Vec<String> = range.rows().nth(1).unwrap()[12..18]
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            cells,
            [
                "Der Tagessatz liegt unter dem Minimum im Profil.",
                "Ja",
                "760",
                "ab sofort",
                "6",
                "60–100"
            ]
        );
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
            rank: 0,
        };
        jobs[0].match_ = Some(scored(MatchStatus::Scored, 83));
        jobs[1].match_ = Some(scored(MatchStatus::Excluded, 71));
        let info = [(INFO_LAST_SCAN.to_string(), "x".to_string())];
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
        let info = [(en::INFO_LAST_SCAN.to_string(), "x".to_string())];
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
        assert_eq!(info.get((0, 0)).unwrap().to_string(), en::INFO_LAST_SCAN);
        assert_eq!(info.get((1, 0)).unwrap().to_string(), en::INFO_NOTE_LABEL);
    }

    /// The details column says what the list's badge says, as of the moment of writing: a
    /// job whose mail is older than the automatic fetch reaches waits for a request, a closed
    /// ad takes no applications.
    #[test]
    fn the_details_column_speaks_like_the_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(super::super::XLSX_NAME);
        let now: Timestamp = "2026-10-01T08:00:00Z".parse().unwrap();
        let mut old = row(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "Leitung Controlling",
            DescStatus::Missing,
        );
        old.mail_date = Some("2026-08-20T07:05:00Z".parse().unwrap());
        let mut closed = row(
            "https://www.linkedin.com/jobs/view/4000000003/",
            "Interim CFO",
            DescStatus::Ok,
        );
        closed.desc_closed = true;
        let jobs = [
            row(
                "https://www.linkedin.com/jobs/view/4000000001/",
                "SAP-Berater",
                DescStatus::Missing,
            ),
            old,
            closed,
        ];
        for (language, words) in [
            (
                Language::De,
                [
                    "Details folgen",
                    "Details auf Anfrage",
                    "Keine Bewerbung mehr möglich",
                ],
            ),
            (
                Language::En,
                [
                    "Details to come",
                    "Details on request",
                    "No longer taking applications",
                ],
            ),
        ] {
            write_xlsx_at(&path, &jobs, &[], language, now).unwrap();
            let mut book: Xlsx<_> = open_workbook(&path).unwrap();
            let range = book
                .worksheet_range(Texts::of(language).jobs_sheet)
                .unwrap();
            let details: Vec<String> = (1..=3)
                .map(|row| range.get((row, 9)).unwrap().to_string())
                .collect();
            assert_eq!(details, words);
        }
    }

    /// One part of an xlsx file (a zip written as a stream: the deflate data of a part ends
    /// by itself, so its local header is enough).
    fn part(xlsx: &[u8], name: &str) -> String {
        use std::io::Read as _;
        let mut at = 0;
        while let Some(found) = xlsx[at..].windows(4).position(|w| w == b"PK\x03\x04") {
            let head = &xlsx[at + found..];
            if head.len() < 30 {
                break;
            }
            let number = |i: usize| usize::from(u16::from_le_bytes([head[i], head[i + 1]]));
            let (method, name_len, extra_len) = (number(8), number(26), number(28));
            if head.get(30..30 + name_len) == Some(name.as_bytes()) {
                assert_eq!(method, 8, "{name} is deflated");
                let mut xml = String::new();
                flate2::read::DeflateDecoder::new(&head[30 + name_len + extra_len..])
                    .read_to_string(&mut xml)
                    .unwrap();
                return xml;
            }
            at += found + 4;
        }
        panic!("{name} missing")
    }

    /// The value of `attribute` in the first tag of `xml` that starts with `tag`.
    fn attribute<'a>(xml: &'a str, tag: &str, attribute: &str) -> &'a str {
        let open = &xml[xml.find(tag).unwrap_or_else(|| panic!("{tag}"))..];
        let open = &open[..open.find('>').unwrap()];
        let value = open
            .split(&format!(" {attribute}=\""))
            .nth(1)
            .unwrap_or_else(|| panic!("{attribute} in {open}"));
        &value[..value.find('"').unwrap()]
    }

    /// The font of a cell of the first sheet.
    fn font_of(xlsx: &[u8], cell: &str) -> String {
        let sheet = part(xlsx, "xl/worksheets/sheet1.xml");
        let styles = part(xlsx, "xl/styles.xml");
        let style: usize = attribute(&sheet, &format!("<c r=\"{cell}\""), "s")
            .parse()
            .unwrap();
        let formats = &styles[styles.find("<cellXfs").unwrap()..];
        let format = formats.split("<xf ").nth(style + 1).unwrap();
        let font: usize = attribute(&format!("<xf {format}"), "<xf ", "fontId")
            .parse()
            .unwrap();
        let fonts = &styles[styles.find("<fonts").unwrap()..styles.find("</fonts>").unwrap()];
        fonts.split("<font>").nth(font + 1).unwrap().to_owned()
    }

    /// An excluded job's row is grey to its links: they stay links, underlined, but not in
    /// Excel's blue.
    #[test]
    fn an_excluded_row_is_grey_to_its_links() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(super::super::XLSX_NAME);
        let mut jobs = [
            row(
                "https://www.linkedin.com/jobs/view/4000000001/",
                "Interim CFO",
                DescStatus::Ok,
            ),
            row(
                "https://www.linkedin.com/jobs/view/4000000002/",
                "Leitung Controlling",
                DescStatus::Ok,
            ),
        ];
        let record = |status| crate::model::MatchRecord {
            status,
            score: 90,
            note: None,
            must_met: 0,
            must_total: 0,
            top: Vec::new(),
            facts: crate::model::KeyFacts::default(),
            rank: 0,
        };
        jobs[0].match_ = Some(record(MatchStatus::Scored));
        jobs[1].match_ = Some(record(MatchStatus::Excluded));
        write_xlsx(&path, &jobs, &[], Language::De).unwrap();
        let xlsx = std::fs::read(&path).unwrap();
        let grey = format!("{EXCLUDED_GREY:06X}");
        for cell in ["F3", "H3"] {
            let font = font_of(&xlsx, cell);
            assert!(
                font.contains(&grey) && font.contains("<u/>"),
                "{cell}: {font}"
            );
        }
        for cell in ["F2", "H2"] {
            let font = font_of(&xlsx, cell);
            assert!(
                !font.contains(&grey) && font.contains("<u/>"),
                "{cell}: {font}"
            );
        }
        assert!(font_of(&xlsx, "C3").contains(&grey), "the row stays grey");
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
