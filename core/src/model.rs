//! Grundtypen, die Mail, Abruf, Speicher, Export und Oberfläche teilen.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::portal::{JobKey, Portal};
use crate::text::{one_line, truncate_chars};

/// Platzhalter, wenn ein Link keinen erkennbaren Titel trägt. Wird später durch einen
/// echten Titel ersetzt (Merge-Regel: nur leere Werte und dieser Platzhalter).
pub const TITLE_PLACEHOLDER: &str = "(Titel nicht erkannt)";

/// Höchstlänge von Titel, Firma und Ort bei der Aufnahme (in Zeichen).
pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_FIELD_CHARS: usize = 120;

/// Ein Job, wie er in einer Alert-Mail steht. Firma und Ort sind Rohwerte – bereinigt
/// wird nur für Anzeige und Export (`text::split_company_location`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posting {
    pub key: JobKey,
    /// Link zur Anzeige (bereinigt, https).
    pub url: Url,
    pub title: String,
    pub company: String,
    pub location: String,
}

impl Posting {
    /// Baut einen Eintrag mit geglätteten und begrenzten Feldern; ein leerer Titel wird
    /// zum Platzhalter.
    pub fn new(key: JobKey, url: Url, title: &str, company: &str, location: &str) -> Self {
        let title = truncate_chars(&one_line(title), MAX_TITLE_CHARS);
        Posting {
            key,
            url,
            title: if title.is_empty() {
                TITLE_PLACEHOLDER.to_string()
            } else {
                title
            },
            company: truncate_chars(&one_line(company), MAX_FIELD_CHARS),
            location: truncate_chars(&one_line(location), MAX_FIELD_CHARS),
        }
    }

    pub fn has_real_title(&self) -> bool {
        !self.title.is_empty() && self.title != TITLE_PLACEHOLDER
    }
}

/// Eine erkannte Alert-Mail samt ihren Einträgen. Eine Mail mit null Einträgen bleibt
/// sichtbar (Layout-Wächter: das Portal hat vermutlich sein Mail-Layout geändert).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertMail {
    /// Dauerhafter Schlüssel der Mail (Gmail-ID, sonst Message-ID, sonst Inhalts-Hash).
    pub key: String,
    /// Portal, dem die Mail zugeordnet ist (Anzeige). Die Einträge tragen ihr eigenes
    /// Portal – eine weitergeleitete Sammelmail kann mehrere enthalten.
    pub portal: Portal,
    pub subject: String,
    pub sender: String,
    pub date: Option<Timestamp>,
    /// Gmail-Nachrichten-ID (`X-GM-MSGID`) für den Direktlink.
    pub gmail_id: Option<u64>,
    pub postings: Vec<Posting>,
}

/// Direktlink auf eine Mail in Gmail (hexadezimale Nachrichten-ID).
pub fn gmail_url(gmail_id: u64) -> Option<Url> {
    (gmail_id != 0)
        .then(|| {
            Url::parse(&format!(
                "https://mail.google.com/mail/u/0/#all/{gmail_id:x}"
            ))
            .ok()
        })
        .flatten()
}

/// Stand der Jobdetails eines Jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DescStatus {
    /// Noch nicht geholt (oder nach portalweitem Abbruch weiter offen).
    Missing,
    /// Volltext liegt vor.
    Ok,
    /// Seite geladen, aber kein gültiger Text – wird später erneut versucht.
    Failed,
    /// Anzeige gibt es nicht mehr.
    Gone,
    /// Nach mehreren Fehlversuchen aufgegeben.
    Unfetchable,
}

impl DescStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            DescStatus::Missing => "missing",
            DescStatus::Ok => "ok",
            DescStatus::Failed => "failed",
            DescStatus::Gone => "gone",
            DescStatus::Unfetchable => "unfetchable",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Missing,
            Self::Ok,
            Self::Failed,
            Self::Gone,
            Self::Unfetchable,
        ]
        .into_iter()
        .find(|s| s.as_str() == text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::job_link;

    #[test]
    fn gmail_link_is_lowercase_hex() {
        assert_eq!(
            gmail_url(0x1A2B).unwrap().as_str(),
            "https://mail.google.com/mail/u/0/#all/1a2b"
        );
        assert_eq!(gmail_url(0), None);
    }

    #[test]
    fn posting_fields_are_flat_and_bounded() {
        let link = job_link("https://www.linkedin.com/jobs/view/4123456789/").unwrap();
        let p = Posting::new(
            link.key.clone(),
            link.url.clone(),
            "  Senior\nController ",
            "A\tB",
            &"x".repeat(500),
        );
        assert_eq!(p.title, "Senior Controller");
        assert_eq!(p.company, "A B");
        assert_eq!(p.location.chars().count(), MAX_FIELD_CHARS);
        let empty = Posting::new(link.key, link.url, " ", "", "");
        assert_eq!(empty.title, TITLE_PLACEHOLDER);
        assert!(!empty.has_real_title());
    }

    #[test]
    fn desc_status_round_trip() {
        for s in [
            DescStatus::Missing,
            DescStatus::Ok,
            DescStatus::Failed,
            DescStatus::Gone,
            DescStatus::Unfetchable,
        ] {
            assert_eq!(DescStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(DescStatus::parse("ok "), None);
    }
}
