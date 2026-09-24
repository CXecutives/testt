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

/// Version of the mail parser (title, company and location of an entry). A job an older
/// version read takes the current reading the next time a mail names it, and the first scan
/// after an update reads back to the oldest such job once (`scan`).
/// 2: a collection mail never takes the next job's title as company.
/// 3: the plain-text link forms of Outlook, Apple Mail and Gmail ("Title<url>", "<url> Title",
/// "[alt] <url>") give the title; separator lines end a block.
pub const MAIL_PARSER_VERSION: i64 = 3;
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
/// portal's domain; forwarded ones carry a forward prefix, an alert word (every subject form
/// of a real alert among them) or a portal name in the subject or sender. An unreadable
/// head is loaded too (and then counted as defective,
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
/// portals (formerly the second portal got lost). A mail without a single recognised entry
/// stays an alert (layout guard: the portal probably changed its mail layout) only when it
/// carries a real alert's markers - the portals' promo, onboarding and network mails come
/// from the same addresses and are no alerts.
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
    // Without a single job: an alert only with a real alert's markers (layout guard).
    let unlinked_alert = found.is_empty() && classify::looks_like_alert(&mail);
    if found.is_empty() && !unlinked_alert {
        return nothing;
    }
    // With jobs: only an alert brings them in - a newsletter, an InMail or an application
    // confirmation that links a job is none.
    if !found.is_empty() && !classify::vouches_for_links(&mail) {
        return nothing;
    }
    // A forwarded alert whose links are no longer recognised names its portal in the
    // forwarded part ("Von: freelancermap Service") when the head does not.
    let portal = classify::portal_of(&mail, &found, allowed).or_else(|| {
        unlinked_alert
            .then(|| classify::portal_in_body(&mail, allowed))
            .flatten()
    });
    let Some(portal) = portal else {
        return nothing;
    };
    let postings: Vec<Posting> = found
        .into_iter()
        .map(|f| Posting::new(f.link.key, f.link.url, &f.title, &f.company, &f.location))
        .collect();
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
