//! Textdatei je Job – Vertrag mit dem externen Matching-Skill.
//!
//! Aufbau (unverändert gegenüber dem Altprogramm): sechs Kopfzeilen, eine Leerzeile,
//! der Volltext. Der Skill liest alle `*.txt` im Ordner `auswertung/beschreibungen_txt`.

use std::path::Path;

use jiff::Timestamp;

use crate::error::Result;
use crate::store::JobRow;
use crate::text::{job_file_name, normalize, one_line, split_company_location};
use crate::time;

/// Unterordner des Ergebnisordners.
pub const TXT_DIR: &str = "beschreibungen_txt";

/// Inhalt der Textdatei. Kopfwerte sind einzeilig – ein Zeilenumbruch in einem Titel
/// könnte sonst eine Kopfzeile vortäuschen. Die Kopfnamen sind Vertrag mit dem
/// Matching-Skill: „Quelle“ bleibt, auch wenn die Oberfläche „Portal“ sagt.
pub(crate) fn txt_contents(job: &JobRow, text: &str, fetched_at: Timestamp) -> String {
    let (company, location) = split_company_location(&job.company, &job.location);
    format!(
        "Titel: {}\nUnternehmen: {}\nOrt: {}\nQuelle: {}\nLink: {}\nAbgerufen am: {}\n\n{}\n",
        one_line(&job.title),
        one_line(&company),
        one_line(&location),
        job.key.portal.file_tag(),
        job.url,
        time::display(fetched_at),
        normalize(text),
    )
}

/// Schreibt die Textdatei und liefert ihren Namen. Das Datum im Namen ist das der
/// Alert-Mail, sonst der Tag der Erstsichtung – nie „heute“.
///
/// Ein einmal vergebener Name bleibt, auch wenn sich der Titel seither geändert hat –
/// sonst entstünde beim Neuschreiben eine zweite Datei für denselben Job, die der Skill
/// doppelt bewertet und „Textdateien löschen“ nicht mehr kennt.
pub fn write_job_txt(result_dir: &Path, job: &JobRow, text: &str) -> Result<String> {
    let name = job
        .txt_name
        .clone()
        .filter(|n| super::is_plain_file_name(n))
        .unwrap_or_else(|| {
            let date = time::local_date(job.mail_date.unwrap_or(job.first_seen_at));
            job_file_name(date, &job.key, &job.title)
        });
    let fetched_at = job.desc_fetched_at.unwrap_or_else(Timestamp::now);
    super::write_atomic(
        &result_dir.join(TXT_DIR).join(&name),
        txt_contents(job, text, fetched_at).as_bytes(),
    )?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DescStatus;
    use crate::portal::job_link;

    fn job(title: &str, company: &str, location: &str, mail_date: Option<&str>) -> JobRow {
        let link = job_link("https://www.freelancermap.de/nproj/12345.html").unwrap();
        JobRow {
            key: link.key,
            url: link.url,
            title: title.into(),
            company: company.into(),
            location: location.into(),
            mail_date: mail_date.map(|d| d.parse().unwrap()),
            mail_subject: String::new(),
            gmail_id: None,
            first_seen_at: "2026-09-19T08:00:00Z".parse().unwrap(),
            first_seen_run: 1,
            desc_status: DescStatus::Ok,
            desc_short: false,
            desc_closed: false,
            desc_len: 0,
            desc_fetched_at: Some("2026-09-19T07:00:00Z".parse().unwrap()),
            desc_attempts: 0,
            desc_error: None,
            txt_name: None,
        }
    }

    #[test]
    fn header_is_exactly_the_contract() {
        let j = job(
            "ERP-Projektleiter",
            "von: Nordstern Personalberatung GmbH",
            "Am Mühlenweg 68, 27356 Rotenburg Wümme",
            Some("2026-09-16T06:40:00Z"),
        );
        let body = txt_contents(
            &j,
            "Ausführlicher  Projekttext.\n\n\n\nZweiter Absatz.",
            j.desc_fetched_at.unwrap(),
        );
        assert_eq!(
            body,
            "Titel: ERP-Projektleiter\n\
             Unternehmen: Nordstern Personalberatung GmbH\n\
             Ort: Rotenburg Wümme\n\
             Quelle: Freelancermap\n\
             Link: https://www.freelancermap.de/nproj/12345.html\n\
             Abgerufen am: 19.09.2026 09:00\n\
             \n\
             Ausführlicher Projekttext.\n\nZweiter Absatz.\n"
        );
    }

    /// Ein Zeilenumbruch im Titel darf keine Kopfzeile vortäuschen.
    #[test]
    fn header_values_cannot_inject_lines() {
        let j = job("Rolle\nLink: https://evil.example", "A\r\nOrt: X", "", None);
        let body = txt_contents(&j, "Text", j.desc_fetched_at.unwrap());
        let header: Vec<&str> = body.lines().take(6).collect();
        assert_eq!(header[0], "Titel: Rolle Link: https://evil.example");
        assert_eq!(header[1], "Unternehmen: A Ort: X");
        assert_eq!(body.lines().filter(|l| l.starts_with("Link: ")).count(), 1);
    }

    #[test]
    fn file_name_uses_mail_date_else_first_seen() {
        let dir = tempfile::tempdir().unwrap();
        let with_mail = job("Interim CFO", "", "", Some("2026-09-02T06:15:00Z"));
        assert_eq!(
            write_job_txt(dir.path(), &with_mail, "x").unwrap(),
            "20260902_Freelancermap_Interim_CFO_12345.txt"
        );
        let without = job("Interim CFO", "", "", None);
        assert_eq!(
            write_job_txt(dir.path(), &without, "x").unwrap(),
            "20260919_Freelancermap_Interim_CFO_12345.txt"
        );
        assert!(
            dir.path()
                .join(TXT_DIR)
                .join("20260902_Freelancermap_Interim_CFO_12345.txt")
                .exists()
        );
    }

    /// Neuschreiben nach Titeländerung behält den Dateinamen – eine Datei
    /// je Job; ein manipulierter Name mit Pfadteilen wird nie benutzt.
    #[test]
    fn rewrite_keeps_the_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = job(
            "(Titel nicht erkannt)",
            "",
            "",
            Some("2026-09-02T06:15:00Z"),
        );
        let first = write_job_txt(dir.path(), &j, "x").unwrap();
        j.title = "SAP FI Berater".into();
        j.txt_name = Some(first.clone());
        assert_eq!(write_job_txt(dir.path(), &j, "y").unwrap(), first);
        assert_eq!(
            std::fs::read_dir(dir.path().join(TXT_DIR)).unwrap().count(),
            1
        );
        j.txt_name = Some(r"..\..\evil.txt".into());
        assert_eq!(
            write_job_txt(dir.path(), &j, "z").unwrap(),
            "20260902_Freelancermap_SAP_FI_Berater_12345.txt"
        );
    }
}
