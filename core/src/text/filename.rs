//! Dateinamen der Jobdetails-Dateien.
//!
//! Schema (Vertrag mit dem Matching-Skill, der nur `*.txt` im Ordner einliest):
//! `<yyyymmdd>_<Portal>_<Titel>_<Job-ID>.txt`. Die Job-ID macht den Namen eindeutig –
//! zwei gleichnamige Anzeigen am selben Tag überschreiben sich nicht mehr, und
//! gekürzte Titel kollidieren nicht.

use jiff::civil::Date;

use crate::portal::JobKey;

/// Höchstlänge des Titelteils in Zeichen.
const TITLE_CHARS: usize = 80;

/// Dateiname der Jobdetails-Datei. `date` ist das Datum der Alert-Mail (sonst der Tag,
/// an dem der Job erstmals gesehen wurde – beides stabil, nie „heute“).
pub fn job_file_name(date: Date, key: &JobKey, title: &str) -> String {
    format!(
        "{}_{}_{}_{}.txt",
        date.strftime("%Y%m%d"),
        key.portal.file_tag(),
        sanitize_file_part(title, "Job"),
        key.id
    )
}

/// Macht aus beliebigem Text einen sicheren Teil eines Windows-Dateinamens:
/// Weißraum und unzulässige Zeichen (`<>:"/\|?*`, Steuerzeichen) werden `_`, Folgen von
/// `_` gebündelt, Ränder aus `._ ` entfernt, höchstens 80 Zeichen. Pfadtrenner und `..`
/// können daher nie überleben.
pub fn sanitize_file_part(text: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in super::one_line(text).chars() {
        let c = if c.is_whitespace() || c.is_control() || "<>:\"/\\|?*".contains(c) {
            '_'
        } else {
            c
        };
        if c == '_' && out.ends_with('_') {
            continue;
        }
        out.push(c);
    }
    let trimmed = super::strip_chars(&out, "._ ");
    let cut = super::truncate_chars(trimmed, TITLE_CHARS);
    let part = super::strip_chars(&cut, "._ ");
    if part.is_empty() {
        fallback.to_string()
    } else {
        part.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::Portal;

    fn key(portal: Portal, id: &str) -> JobKey {
        JobKey {
            portal,
            id: id.to_string(),
        }
    }

    #[test]
    fn file_name_scheme() {
        let date = Date::new(2026, 9, 2).unwrap();
        assert_eq!(
            job_file_name(
                date,
                &key(Portal::LinkedIn, "4123456789"),
                "Transformation Manager"
            ),
            "20260902_LinkedIn_Transformation_Manager_4123456789.txt"
        );
        assert_eq!(
            job_file_name(
                date,
                &key(Portal::FreelanceDe, "1288981"),
                "Transformation Manager"
            ),
            "20260902_Freelance_Transformation_Manager_1288981.txt"
        );
        assert_eq!(
            job_file_name(
                date,
                &key(Portal::Freelancermap, "u0123456789ab"),
                "Transformation Manager"
            ),
            "20260902_Freelancermap_Transformation_Manager_u0123456789ab.txt"
        );
    }

    #[test]
    fn unsafe_characters_in_title() {
        assert_eq!(
            sanitize_file_part("SAP \"Fi/CO\": Berater (m/w/d)", "Job"),
            "SAP_Fi_CO_Berater_(m_w_d)"
        );
        assert_eq!(sanitize_file_part("  ", "Job"), "Job");
        assert_eq!(sanitize_file_part("...", "Job"), "Job");
    }

    /// Ein Titel aus einer Mail darf nie aus dem Ordner herausführen.
    #[test]
    fn traversal_and_separators_cannot_survive() {
        for title in [
            "../../../../Windows/System32/x",
            "..\\..\\boot.ini",
            "C:\\Temp\\x",
            "a/../b",
        ] {
            let part = sanitize_file_part(title, "Job");
            assert!(!part.contains(['/', '\\', ':']), "{part}");
            assert!(!part.starts_with('.'), "{part}");
        }
    }

    /// Früher: Kürzung nach Zeichen, nicht nach Bytes, und ohne Rest-Unterstrich.
    #[test]
    fn long_and_unicode_titles() {
        let long = "Überlänge ".repeat(20);
        let part = sanitize_file_part(&long, "Job");
        assert!(part.chars().count() <= 80);
        assert!(!part.ends_with('_'));
        assert_eq!(
            sanitize_file_part("🚀 Cloud-Architekt 🚀", "Job"),
            "🚀_Cloud-Architekt_🚀"
        );
    }

    #[test]
    fn same_title_same_day_different_jobs_differ() {
        let date = Date::new(2026, 9, 15).unwrap();
        let a = job_file_name(date, &key(Portal::LinkedIn, "4100000001"), "Interim CFO");
        let b = job_file_name(date, &key(Portal::LinkedIn, "4100000002"), "Interim CFO");
        assert_ne!(a, b);
    }
}
