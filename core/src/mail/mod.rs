//! Alert-Mails: zerlegen, Jobs herausziehen, dem Portal zuordnen.

mod classify;
mod extract;
pub mod imap;
mod parse;
pub mod scan;

use sha2::{Digest, Sha256};

use crate::model::{AlertMail, MAX_FIELD_CHARS, MAX_TITLE_CHARS, Posting};
use crate::portal::{Portal, hex12};
use crate::text::truncate_chars;

pub(crate) use extract::Found;
pub(crate) use parse::{ParsedMail, parse_mail};

/// Eine Mail, wie sie aus dem Postfach kommt.
#[derive(Debug, Clone, Default)]
pub struct RawMail {
    /// Gmail-Nachrichten-ID (`X-GM-MSGID`).
    pub gmail_id: Option<u64>,
    pub bytes: Vec<u8>,
}

/// Was eine Mail ist.
#[derive(Debug)]
pub enum MailKind {
    Alert(AlertMail),
    /// Keine Job-Alert-Mail (oder ein nicht gewähltes Portal).
    Other,
    /// Nicht lesbar – wird gezählt und gemeldet, nie still verworfen.
    Defective,
}

/// Erkennt eine Job-Alert-Mail der gewählten Portale.
///
/// Einträge tragen ihr eigenes Portal: Eine weitergeleitete Sammelmail kann Jobs mehrerer
/// Portale enthalten (früher: das zweite Portal ging verloren). Ein Alert ohne einen
/// einzigen erkannten Eintrag bleibt als solcher erhalten (Layout-Wächter), wenn sein
/// Betreff nach Alert aussieht.
pub fn classify_mail(raw: &RawMail, allowed: &[Portal]) -> MailKind {
    let Some(mail) = parse_mail(&raw.bytes) else {
        return MailKind::Defective;
    };
    // Ohne Absender, Betreff und Datum ist es keine lesbare Mail (z. B. Binärmüll).
    if mail.sender_address.is_empty() && mail.subject.is_empty() && mail.date.is_none() {
        return MailKind::Defective;
    }
    // Beschädigt und trotzdem nichts erkannt: zählt als unlesbar, nicht still als „kein Alert“.
    let nothing = if mail.damaged {
        MailKind::Defective
    } else {
        MailKind::Other
    };
    let found: Vec<Found> = extract::extract(&mail.html, &mail.text)
        .into_iter()
        .filter(|f| allowed.contains(&f.link.key.portal))
        .collect();
    let Some(portal) = classify::portal_of(&mail, &found, allowed) else {
        return nothing;
    };
    let postings: Vec<Posting> = found
        .into_iter()
        .map(|f| Posting::new(f.link.key, f.link.url, &f.title, &f.company, &f.location))
        .collect();
    if postings.is_empty() && !classify::is_alert_subject(&mail.subject) {
        return nothing;
    }
    MailKind::Alert(AlertMail {
        key: mail_key(raw, &mail),
        portal,
        subject: truncate_chars(&mail.subject, MAX_TITLE_CHARS),
        sender: truncate_chars(&mail.sender, MAX_FIELD_CHARS),
        date: mail.date,
        gmail_id: raw.gmail_id,
        postings,
    })
}

/// Dauerhafter Schlüssel einer Mail: Gmail-ID, sonst Message-ID, sonst Inhalts-Hash.
fn mail_key(raw: &RawMail, mail: &ParsedMail) -> String {
    if let Some(id) = raw.gmail_id {
        return format!("gm:{id:x}");
    }
    if let Some(id) = mail.message_id.as_deref().filter(|id| !id.is_empty()) {
        return format!("mid:{}", truncate_chars(id, 200));
    }
    format!("h:{}", hex12(&Sha256::digest(&raw.bytes)))
}

#[cfg(test)]
mod tests;
