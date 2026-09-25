//! Basic types shared by mail, fetch, store, export and the interface.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::portal::{JobKey, Portal};
use crate::text::{one_line, truncate_chars};

/// Placeholder when a link carries no recognisable title. Replaced later by a real title
/// (merge rule: only empty values and this placeholder). Stored in the database and written
/// to Excel and the TXT files - German by product decision, do not translate.
pub const TITLE_PLACEHOLDER: &str = "(Titel nicht erkannt)";

/// Maximum length of title, company and location when ingested (in characters).
pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_FIELD_CHARS: usize = 120;

/// A job as it appears in an alert mail. Company and location are raw values - they are
/// only cleaned for display and export (`text::split_company_location`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posting {
    pub key: JobKey,
    /// Link to the ad (cleaned, https).
    pub url: Url,
    pub title: String,
    pub company: String,
    pub location: String,
}

impl Posting {
    /// Builds an entry with flattened and bounded fields; an empty title becomes the
    /// placeholder.
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
        is_usable_title(&self.title)
    }
}

/// Is the value usable as a title? Empty and the placeholder are not - and neither is a bare
/// address: when a mail linked the title as a URL, the URL used to stay in the list as the
/// title because it was "not empty".
pub fn is_usable_title(title: &str) -> bool {
    !title.trim().is_empty() && title != TITLE_PLACEHOLDER && !crate::mail::extract::is_url(title)
}

/// A recognised alert mail with its entries. A mail with zero entries stays visible (layout
/// guard: the portal probably changed its mail layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertMail {
    /// Durable key of the mail (Gmail id, else Message-ID, else content hash).
    pub key: String,
    /// Portal the mail is assigned to (display). The entries carry their own portal - a
    /// forwarded digest can contain several.
    pub portal: Portal,
    pub subject: String,
    pub sender: String,
    pub date: Option<Timestamp>,
    /// Gmail message id (`X-GM-MSGID`) for the direct link.
    pub gmail_id: Option<u64>,
    pub postings: Vec<Posting>,
}

/// Direct link to a mail in Gmail (hexadecimal message id) in the browser's first Google
/// account (`u/0`), as the files carry it: they know no mailbox.
pub fn gmail_url(gmail_id: u64) -> Option<Url> {
    gmail_url_for(gmail_id, None)
}

/// Direct link to a mail in Gmail (hexadecimal message id) in the account of `mailbox`, the
/// address the app reads: a browser signed in to several Google accounts opens the right
/// one. Without an address the first account (`u/0`).
pub fn gmail_url_for(gmail_id: u64, mailbox: Option<&str>) -> Option<Url> {
    if gmail_id == 0 {
        return None;
    }
    let account = mailbox
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or("0");
    let mut url = Url::parse("https://mail.google.com/mail/u/").ok()?;
    url.path_segments_mut()
        .ok()?
        .pop_if_empty()
        .push(account)
        .push("");
    url.set_fragment(Some(&format!("all/{gmail_id:x}")));
    Some(url)
}

/// State of the job details of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DescStatus {
    /// Not fetched yet (or still open after a portal-wide stop).
    Missing,
    /// Full text is available.
    Ok,
    /// Only the teaser a guest sees (freelance.de without sign-in) - short, but worth
    /// matching; no text file is written for it.
    Teaser,
    /// Page loaded but no valid text - retried later.
    Failed,
    /// The ad no longer exists.
    Gone,
    /// Given up after several failed attempts.
    Unfetchable,
}

impl DescStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            DescStatus::Missing => "missing",
            DescStatus::Ok => "ok",
            DescStatus::Teaser => "teaser",
            DescStatus::Failed => "failed",
            DescStatus::Gone => "gone",
            DescStatus::Unfetchable => "unfetchable",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Missing,
            Self::Ok,
            Self::Teaser,
            Self::Failed,
            Self::Gone,
            Self::Unfetchable,
        ]
        .into_iter()
        .find(|s| s.as_str() == text)
    }
}

/// How well a job fits the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum MatchStatus {
    /// A score from the profile.
    Scored,
    /// At least one decided violation of a hard criterion; the score is kept.
    Excluded,
    /// Too little text to judge.
    Unscorable,
}

impl MatchStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            MatchStatus::Scored => "scored",
            MatchStatus::Excluded => "excluded",
            MatchStatus::Unscorable => "unscorable",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [Self::Scored, Self::Excluded, Self::Unscorable]
            .into_iter()
            .find(|s| s.as_str() == text)
    }
}

/// Where a job is, like a mail: in the inbox ("Eingang", the active jobs), the archive or the
/// trash ("Papierkorb"). A job is in exactly one place; the favourite (the star) is a flag of
/// its own. Runs never move a job, except that old ones archive themselves and an old trash
/// empties itself (settings).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Place {
    #[default]
    Inbox,
    Archive,
    Trash,
}

/// Score band of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Band {
    High,
    Mid,
    Low,
}

/// Minimum score of the high band (the store counts "high" with it).
pub const HIGH_FROM: u8 = 80;
/// Minimum score of the mid band.
pub const MID_FROM: u8 = 40;

/// The band of a score - the only place with the thresholds (80 and 40).
pub const fn band(score: u8) -> Band {
    if score >= HIGH_FROM {
        Band::High
    } else if score >= MID_FROM {
        Band::Mid
    } else {
        Band::Low
    }
}

/// The stored match of a job - what a matcher says about it.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchRecord {
    pub status: MatchStatus,
    /// 0-100; kept for excluded jobs too.
    pub score: u8,
    /// The one statement the list shows (e.g. why excluded).
    pub note: Option<Notice>,
    pub must_met: u16,
    pub must_total: u16,
    /// At most two met requirements, quoted from the ad.
    pub top: Vec<String>,
    /// Rate, start, duration, remote share and contract type as the engine read them.
    pub facts: KeyFacts,
    /// Per-mille score before caps and rounding: orders equal scores in the list.
    pub rank: u16,
}

/// The key facts of an ad as the engine read them (the page facts first, then the text):
/// numbers and codes for the list row and the reader, `null` when the ad says nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KeyFacts {
    /// Highest rate amount stated, per day or per hour (`hourly`).
    pub rate: Option<u32>,
    pub hourly: Option<bool>,
    /// Currency code of a rate not in EUR (`CHF`).
    pub currency: Option<String>,
    /// The ad names a rate to be agreed (`nach Absprache`) without an amount.
    pub rate_open: Option<bool>,
    /// Start: `now`, `vague` or an ISO date (`2026-11-01`).
    pub start: Option<String>,
    /// Duration in months.
    pub months: Option<u16>,
    /// Remote share in percent, from and to (equal when the ad states one share).
    pub remote_from: Option<u8>,
    pub remote_to: Option<u8>,
    /// Contract type: `interim`, `permanent` or `anue` (`null` when unclear).
    pub contract: Option<String>,
}

impl KeyFacts {
    pub fn is_empty(&self) -> bool {
        *self == KeyFacts::default()
    }
}

/// A statement for the interface as a code with data - the core never sends prose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Notice {
    pub code: String,
    #[cfg_attr(test, ts(type = "Record<string, string | number | boolean | null>"))]
    pub params: serde_json::Map<String, serde_json::Value>,
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

    /// In the app a mail opens in the account of the mailbox the app reads, not in whatever
    /// account the browser has first; without an address it stays `u/0`.
    #[test]
    fn gmail_link_names_the_mailbox() {
        assert_eq!(
            gmail_url_for(0x1A2B, Some("erika.muster+jobs@gmail.com"))
                .unwrap()
                .as_str(),
            "https://mail.google.com/mail/u/erika.muster+jobs@gmail.com/#all/1a2b"
        );
        assert_eq!(gmail_url_for(0x1A2B, None), gmail_url(0x1A2B));
        assert_eq!(gmail_url_for(0x1A2B, Some("  ")), gmail_url(0x1A2B));
        assert_eq!(
            gmail_url_for(0x1A2B, Some("a/b#c")).unwrap().as_str(),
            "https://mail.google.com/mail/u/a%2Fb%23c/#all/1a2b",
            "an address never leaves its path segment"
        );
        assert_eq!(gmail_url_for(0, Some("erika@gmail.com")), None);
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
            DescStatus::Teaser,
            DescStatus::Failed,
            DescStatus::Gone,
            DescStatus::Unfetchable,
        ] {
            assert_eq!(DescStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(DescStatus::parse("ok "), None);
    }

    #[test]
    fn bands_split_at_80_and_40() {
        let bands = [0, 39, 40, 79, 80, 100].map(band);
        assert_eq!(
            bands,
            [
                Band::Low,
                Band::Low,
                Band::Mid,
                Band::Mid,
                Band::High,
                Band::High
            ]
        );
        for s in [
            MatchStatus::Scored,
            MatchStatus::Excluded,
            MatchStatus::Unscorable,
        ] {
            assert_eq!(MatchStatus::parse(s.as_str()), Some(s));
        }
    }
}
