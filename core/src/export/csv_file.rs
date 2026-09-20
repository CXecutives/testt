//! `JobAlerts.csv`: dieselben Spalten wie die Excel-Datei; Semikolon und UTF-8 mit BOM,
//! damit ein deutsches Excel die Datei ohne Import-Dialog richtig öffnet.

use std::path::Path;

use super::{COLUMNS, Line};
use crate::error::{Error, Result};
use crate::store::JobRow;

pub fn write_csv(path: &Path, jobs: &[JobRow]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_writer(b"\xEF\xBB\xBF".to_vec());
    writer.write_record(COLUMNS)?;
    for job in jobs {
        let line = Line::of(job);
        let cells = [
            line.source.to_string(),
            line.mail_date,
            line.title,
            line.company,
            line.location,
            line.url,
            line.subject,
            line.gmail_url,
            line.saved_at,
            line.details.to_string(),
            line.key,
        ];
        writer.write_record(cells.iter().map(|c| defuse(c)))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| Error::Csv(e.into_error().into()))?;
    super::write_atomic(path, &bytes)
}

/// Excel führt Zellen, die mit `= + - @` (oder Tab/Wagenrücklauf) beginnen, als Formel aus
/// – Titel und Betreffs stammen aus fremden Mails. Ein vorangestelltes `'` macht sie zu Text.
fn defuse(cell: &str) -> String {
    if cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{cell}")
    } else {
        cell.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DescStatus;
    use crate::portal::job_link;

    #[test]
    fn bom_semicolon_and_formula_defused() {
        let link = job_link("https://www.linkedin.com/jobs/view/4000000001/").unwrap();
        let job = JobRow {
            key: link.key,
            url: link.url,
            title: "=cmd|' /C calc'!A1".into(),
            company: "Müller; Söhne".into(),
            location: "+49 Remote".into(),
            mail_date: None,
            mail_subject: "@SUM(1)".into(),
            gmail_id: None,
            first_seen_at: "2026-09-19T08:00:00Z".parse().unwrap(),
            first_seen_run: 1,
            desc_status: DescStatus::Missing,
            desc_short: false,
            desc_closed: false,
            desc_len: 0,
            desc_fetched_at: None,
            desc_attempts: 0,
            desc_error: None,
            txt_name: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("JobAlerts.csv");
        write_csv(&path, &[job]).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"\xEF\xBB\xBF"));
        let text = String::from_utf8(bytes[3..].to_vec()).unwrap();
        let mut lines = text.lines();
        assert_eq!(lines.next().unwrap(), COLUMNS.join(";"));
        let row = lines.next().unwrap();
        assert!(
            row.starts_with("LinkedIn;;'=cmd|' /C calc'!A1;\"Müller; Söhne\";'+49 Remote;"),
            "{row}"
        );
        assert!(row.contains(";'@SUM(1);"), "{row}");
        assert!(
            row.contains(";19.09.2026 10:00;fehlt;linkedin:4000000001"),
            "{row}"
        );
    }
}
