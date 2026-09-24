//! Alert mails: take them apart, pull out the jobs, assign them to a portal.

mod classify;
pub(crate) mod extract;
pub mod imap;
mod parse;
pub mod scan;

use sha2::{Digest, Sha256};

use crate::model::{AlertMail, MAX_FIELD_CHARS, MAX_TITLE_CHARS, Posting};
use crate::portal::{Portal, hex12};
use crate::text::truncate_chars;

pub(crate) use extract::Found;
pub(crate) use parse::{ParsedMail, parse_mail};

/// A mail as it comes from the mailbox.
#[derive(Debug, Clone, Default)]
pub struct RawMail {
    /// Gmail message id (`X-GM-MSGID`).
    pub gmail_id: Option<u64>,
    pub bytes: Vec<u8>,
}

/// The head of a mail (sender, subject, date, message id) - loaded before any body.
#[derive(Debug, Clone, Default)]
pub struct RawHead {
    pub uid: u32,
    pub bytes: Vec<u8>,
}

/// What a mail is.
#[derive(Debug)]
pub enum MailKind {
    Alert(AlertMail),
    /// No job alert mail (or a portal that is not chosen).
    Other,
    /// Unreadable - counted and reported, never silently dropped.
    Defective,
}

/// Is the mail worth loading whole, judged by its head alone? Original alerts come from a
/// portal's domain; forwarded ones carry a forward prefix, an alert word or a portal name in
/// the subject or sender. An unreadable head is loaded too (and then counted as defective,
/// never silently dropped).
pub fn is_candidate(head: &[u8], allowed: &[Portal]) -> bool {
    let Some(mail) = parse_mail(head) else {
        return true;
    };
    if mail.sender_address.is_empty() && mail.subject.is_empty() && mail.date.is_none() {
        return true;
    }
    classify::head_is_candidate(&mail, allowed)
}

/// The head part of a raw mail (up to and including the blank line).
pub fn head_part(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|at| at + 4)
        .or_else(|| bytes.windows(2).position(|w| w == b"\n\n").map(|at| at + 2))
        .unwrap_or(bytes.len());
    &bytes[..end]
}

/// Recognises a job alert mail of the chosen portals.
///
/// Entries carry their own portal: a forwarded collection mail can hold jobs of several
/// portals (formerly the second portal got lost). An alert without a single recognised
/// entry stays an alert (layout guard) when its subject reads like one.
pub fn classify_mail(raw: &RawMail, allowed: &[Portal]) -> MailKind {
    let Some(mail) = parse_mail(&raw.bytes) else {
        return MailKind::Defective;
    };
    // Without sender, subject and date it is no readable mail (binary garbage, say).
    if mail.sender_address.is_empty() && mail.subject.is_empty() && mail.date.is_none() {
        return MailKind::Defective;
    }
    // Damaged and still nothing recognised: counts as unreadable, not silently as "no
    // alert".
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

/// Lasting key of a mail: Gmail id, else message id, else a hash of the content.
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
