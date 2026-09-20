//! Eine Mail zerlegen: Kopf (Betreff, Absender, Datum) und alle Textteile – auch die einer
//! als Anhang weitergeleiteten Mail (`message/rfc822`; früher ergaben solche Weiterleitungen
//! null Einträge).

use jiff::Timestamp;
use mail_parser::decoders::base64::base64_decode;
use mail_parser::decoders::charsets::map::charset_decoder;
use mail_parser::decoders::quoted_printable::quoted_printable_decode;
use mail_parser::{Message, MessageParser, MessagePart, MimeHeaders, PartType};

use crate::text::one_line;

/// Wie tief weitergeleitete Mails (Mail in Mail) ausgepackt werden.
const MAX_NESTING: usize = 4;

#[derive(Debug, Default)]
pub struct ParsedMail {
    pub subject: String,
    /// Anzeigename des Absenders, sonst die Adresse.
    pub sender: String,
    /// Absender-Adresse (klein geschrieben).
    pub sender_address: String,
    pub date: Option<Timestamp>,
    pub message_id: Option<String>,
    pub html: Vec<String>,
    pub text: Vec<String>,
    /// Beschädigte Kodierung (fehlende MIME-Grenze, kaputtes Base64 …) – nachsichtig
    /// gelesen; ergibt die Mail trotzdem nichts, zählt sie als unlesbar.
    pub damaged: bool,
}

impl ParsedMail {
    /// Domain der Absender-Adresse.
    pub fn sender_domain(&self) -> Option<&str> {
        self.sender_address.rsplit_once('@').map(|(_, d)| d)
    }
}

/// `None`, wenn die Bytes keine Mail sind.
pub fn parse_mail(raw: &[u8]) -> Option<ParsedMail> {
    let message = MessageParser::default().parse(raw)?;
    let from = message.from().and_then(|a| a.first());
    let address = from
        .and_then(|a| a.address())
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let name = from
        .and_then(|a| a.name())
        .map(one_line)
        .unwrap_or_default();
    let mut mail = ParsedMail {
        subject: message.subject().map(one_line).unwrap_or_default(),
        sender: if name.is_empty() {
            address.clone()
        } else {
            name
        },
        sender_address: address,
        date: message
            .date()
            .and_then(|d| Timestamp::from_second(d.to_timestamp()).ok()),
        message_id: message.message_id().map(str::to_string),
        ..ParsedMail::default()
    };
    collect_bodies(&message, 0, &mut mail);
    Some(mail)
}

fn collect_bodies(message: &Message<'_>, depth: usize, out: &mut ParsedMail) {
    for part in &message.parts {
        let attachment = part
            .content_disposition()
            .is_some_and(mail_parser::ContentType::is_attachment);
        if part.is_encoding_problem && !attachment {
            out.damaged = true;
            if let Some((is_html, text)) = repaired(part) {
                if is_html {
                    out.html.push(text);
                } else {
                    out.text.push(text);
                }
                continue;
            }
        }
        match &part.body {
            PartType::Html(html) if !attachment => out.html.push(html.to_string()),
            PartType::Text(text) if !attachment => out.text.push(text.to_string()),
            PartType::Message(inner) if depth < MAX_NESTING => {
                collect_bodies(inner, depth + 1, out);
            }
            _ => {}
        }
    }
}

/// Textteil mit Kodierungsfehler nachsichtig dekodieren: fremde Zeichen im Base64
/// überspringen, dann den Zeichensatz anwenden (sonst fehlte der HTML-Teil
/// samt aller Links).
fn repaired(part: &MessagePart<'_>) -> Option<(bool, String)> {
    let content_type = part.content_type()?;
    if !content_type.ctype().eq_ignore_ascii_case("text") {
        return None;
    }
    let is_html = content_type
        .subtype()
        .is_some_and(|s| s.eq_ignore_ascii_case("html"));
    let raw = part.contents();
    let encoding = part
        .content_transfer_encoding()
        .map(str::to_ascii_lowercase);
    let bytes = match encoding.as_deref() {
        Some("base64") => {
            let alphabet: Vec<u8> = raw
                .iter()
                .copied()
                .filter(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/'))
                .collect();
            base64_decode(&alphabet)?
        }
        Some("quoted-printable") => quoted_printable_decode(raw)?,
        _ => raw.to_vec(),
    };
    let text = match content_type
        .attribute("charset")
        .and_then(|c| charset_decoder(c.as_bytes()))
    {
        Some(decode) => decode(&bytes),
        None => String::from_utf8_lossy(&bytes).into_owned(),
    };
    Some((is_html, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALTERNATIVE: &str = "From: =?utf-8?B?RXJpa2EgTcO8bGxlcg==?= <Erika@Example.com>\r\n\
To: ich@gmail.com\r\n\
Subject: =?utf-8?q?K=C3=B6ln=3A_neue_Jobs?=\r\n\
Date: Thu, 03 Sep 2026 08:15:00 +0200\r\n\
Message-ID: <abc@mail>\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"b\"\r\n\
\r\n\
--b\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Klartext\r\n\
--b\r\n\
Content-Type: text/html; charset=iso-8859-1\r\n\
Content-Transfer-Encoding: quoted-printable\r\n\
\r\n\
<p>M=FCnchen</p>\r\n\
--b--\r\n";

    #[test]
    fn header_and_bodies() {
        let mail = parse_mail(ALTERNATIVE.as_bytes()).unwrap();
        assert_eq!(mail.subject, "Köln: neue Jobs");
        assert_eq!(mail.sender, "Erika Müller");
        assert_eq!(mail.sender_address, "erika@example.com");
        assert_eq!(mail.sender_domain(), Some("example.com"));
        assert_eq!(mail.date.unwrap().to_string(), "2026-09-03T06:15:00Z");
        assert_eq!(mail.message_id.as_deref(), Some("abc@mail"));
        assert_eq!(mail.text, ["Klartext"]);
        assert_eq!(mail.html, ["<p>München</p>"]);
    }

    /// Weitergeleitet „als Anhang“: die innere Mail wird mitgelesen, ein echter
    /// Textanhang nicht.
    #[test]
    fn forwarded_as_attachment_is_unpacked() {
        let raw = "From: ich@gmail.com\r\n\
Subject: Fwd: Jobs\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"o\"\r\n\
\r\n\
--o\r\n\
Content-Type: text/plain\r\n\
\r\n\
Siehe Anhang\r\n\
--o\r\n\
Content-Type: text/plain\r\n\
Content-Disposition: attachment; filename=\"notiz.txt\"\r\n\
\r\n\
nicht lesen\r\n\
--o\r\n\
Content-Type: message/rfc822\r\n\
Content-Disposition: attachment\r\n\
\r\n\
From: LinkedIn <jobalerts-noreply@linkedin.com>\r\n\
Subject: 2 neue Jobs\r\n\
Content-Type: text/html\r\n\
\r\n\
<a href=\"https://www.linkedin.com/comm/jobs/view/4123456789/\">Controller</a>\r\n\
--o--\r\n";
        let mail = parse_mail(raw.as_bytes()).unwrap();
        assert_eq!(mail.text, ["Siehe Anhang"]);
        assert_eq!(mail.html.len(), 1);
        assert!(mail.html[0].contains("4123456789"));
    }

    #[test]
    fn garbage_is_no_mail() {
        assert!(parse_mail(b"").is_none());
    }

    /// Base64-HTML mit Fremdzeichen und ohne schließende MIME-Grenze wird
    /// trotzdem gelesen; die Mail ist als beschädigt markiert.
    #[test]
    fn damaged_base64_part_is_repaired() {
        let html = r#"<a href="https://www.linkedin.com/jobs/view/4100000031/">Controller</a>"#;
        // Base64 von `html`, mitten darin ein Fremdzeichen („!“).
        let b64 = "PGEgaHJlZj0iaHR0cHM6Ly93d3cubGlua2VkaW4uY29tL2pvYnMvdmlldy80MTAw!MDAwMDMxLyI+Q29udHJvbGxlcjwvYT4=";
        let raw = [
            "From: jobalerts-noreply@linkedin.com",
            "Subject: Neue Jobs",
            "MIME-Version: 1.0",
            "Content-Type: multipart/alternative; boundary=\"a\"",
            "",
            "--a",
            "Content-Type: text/plain",
            "",
            "Klartext ohne Links",
            "--a",
            "Content-Type: text/html; charset=utf-8",
            "Content-Transfer-Encoding: base64",
            "",
            b64,
            "",
        ]
        .join("\r\n");
        let mail = parse_mail(raw.as_bytes()).unwrap();
        let all: Vec<&String> = mail.html.iter().chain(&mail.text).collect();
        assert!(all.iter().any(|t| t.contains(html)), "{all:?}");
    }
}
