//! Was die Oberfläche anzeigt – fertig aufbereitet. Firma und Ort werden hier bereinigt
//! (die Datenbank hält die Rohwerte aus der Mail), Links vollständig gebildet.

use jiff::Timestamp;
use serde::Serialize;

use crate::export::details_label;
use crate::fetch::policy::{Allowance, PauseKind, Policy, limits};
use crate::fetch::{MAX_AGE, RETRY_AFTER};
use crate::model::{DescStatus, gmail_url};
use crate::portal::{JobKey, Portal};
use crate::store::{AlertMailRow, JobRow, Store};
use crate::text::split_company_location;

/// Eine Tabellenzeile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub key: JobKey,
    pub portal_label: &'static str,
    pub title: String,
    pub company: String,
    pub location: String,
    pub url: String,
    pub mail_date: Option<Timestamp>,
    pub mail_subject: String,
    pub gmail_url: Option<String>,
    pub first_seen_at: Timestamp,
    pub first_seen_run: i64,
    pub status: DescStatus,
    /// Stand der Jobdetails in Worten (wie in Excel).
    pub status_text: &'static str,
    pub short: bool,
    pub closed: bool,
    pub desc_len: i64,
    pub desc_error: Option<String>,
    pub txt_name: Option<String>,
}

impl From<&JobRow> for JobView {
    fn from(job: &JobRow) -> JobView {
        let (company, location) = split_company_location(&job.company, &job.location);
        JobView {
            key: job.key.clone(),
            portal_label: job.key.portal.label(),
            title: job.title.clone(),
            company,
            location,
            url: job.url.to_string(),
            mail_date: job.mail_date,
            mail_subject: job.mail_subject.clone(),
            gmail_url: job.gmail_id.and_then(gmail_url).map(|u| u.to_string()),
            first_seen_at: job.first_seen_at,
            first_seen_run: job.first_seen_run,
            status: job.desc_status,
            status_text: details_label(job),
            short: job.desc_short,
            closed: job.desc_closed,
            desc_len: job.desc_len,
            desc_error: job.desc_error.clone(),
            txt_name: job.txt_name.clone(),
        }
    }
}

/// Alert-Mail ohne erkannte Einträge (graue Zeile).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertMailView {
    pub portal: Portal,
    pub portal_label: &'static str,
    pub subject: String,
    pub mail_date: Option<Timestamp>,
    /// Gmail-Nachrichten-ID (hexadezimal) – geöffnet wird sie über `open_target`.
    pub gmail_id: Option<String>,
}

impl From<&AlertMailRow> for AlertMailView {
    fn from(row: &AlertMailRow) -> AlertMailView {
        AlertMailView {
            portal: row.portal,
            portal_label: row.portal.label(),
            subject: row.subject.clone(),
            mail_date: row.mail_date,
            gmail_id: row.gmail_id.map(|id| format!("{id:x}")),
        }
    }
}

/// Zustand eines Portals (Portal-Ansicht).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalView {
    pub portal: Portal,
    pub label: &'static str,
    /// LinkedIn und freelancermap: kein Konto nötig.
    pub needs_account: bool,
    pub paused_until: Option<Timestamp>,
    pub pause_kind: Option<PauseKind>,
    pub pause_reason: Option<String>,
    /// Obergrenze erreicht: ab wann wieder abgerufen werden kann.
    pub next_free_at: Option<Timestamp>,
    pub used_hour: usize,
    pub cap_hour: usize,
    pub used_day: usize,
    pub cap_day: usize,
    pub login_needed: bool,
    pub session_confirmed_at: Option<Timestamp>,
    /// Jetzt automatisch abrufbar (Mails ≤ 30 Tage, Fehlschläge nach 12 h).
    pub due: usize,
    pub open: i64,
    pub failed: i64,
    pub unfetchable: i64,
    pub gone: i64,
    pub ok: i64,
    pub short: i64,
}

pub fn portal_views(
    policy: &Policy,
    store: &Store,
    now: Timestamp,
) -> crate::Result<Vec<PortalView>> {
    let counts = store.portal_counts()?;
    let due = store.due_counts(now, MAX_AGE, RETRY_AFTER)?;
    Ok(Portal::ALL
        .into_iter()
        .map(|portal| {
            let state = policy.state(portal);
            let limits = limits(portal);
            let (used_hour, used_day) = policy.usage(portal, now);
            let count = |status: DescStatus| {
                counts
                    .iter()
                    .filter(|c| c.portal == portal && c.status == status)
                    .map(|c| c.count)
                    .sum()
            };
            let short = counts
                .iter()
                .filter(|c| c.portal == portal && c.status == DescStatus::Ok)
                .map(|c| c.short)
                .sum();
            let (paused, next_free_at) = match policy.allowance(portal, now) {
                Allowance::Paused { .. } => (true, None),
                Allowance::Quota { next_at } => (false, Some(next_at)),
                Allowance::Go => (false, None),
            };
            PortalView {
                portal,
                label: portal.label(),
                needs_account: portal == Portal::FreelanceDe,
                paused_until: state.paused_until.filter(|_| paused),
                pause_kind: state.pause_kind.filter(|_| paused),
                pause_reason: state.pause_reason.filter(|_| paused),
                next_free_at,
                used_hour,
                cap_hour: limits.per_hour,
                used_day,
                cap_day: limits.per_day,
                login_needed: state.login_needed,
                session_confirmed_at: state.session_confirmed_at,
                due: due
                    .iter()
                    .find(|(p, _)| *p == portal)
                    .map_or(0, |(_, n)| *n),
                open: count(DescStatus::Missing),
                failed: count(DescStatus::Failed),
                unfetchable: count(DescStatus::Unfetchable),
                gone: count(DescStatus::Gone),
                ok: count(DescStatus::Ok),
                short,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::policy::PauseKind;
    use crate::model::Posting;
    use crate::portal::job_link;
    use crate::store::MailRef;

    #[test]
    fn job_row_is_cleaned_for_display() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let link = job_link("https://www.freelancermap.de/nproj/12345.html").unwrap();
        let posting = Posting::new(
            link.key.clone(),
            link.url,
            "Rolle",
            "von: Muster GmbH",
            "Am Mühlenweg 68, 27356 Rotenburg Wümme",
        );
        let mail = MailRef {
            subject: "Neue Projekte",
            date: None,
            gmail_id: Some(0x1a2b),
        };
        store
            .upsert_posting(run, &posting, mail, Timestamp::now())
            .unwrap();
        let view = JobView::from(&store.job(&link.key).unwrap().unwrap());
        assert_eq!(
            (view.company.as_str(), view.location.as_str()),
            ("Muster GmbH", "Rotenburg Wümme")
        );
        assert_eq!(
            view.gmail_url.as_deref(),
            Some("https://mail.google.com/mail/u/0/#all/1a2b")
        );
        assert_eq!(view.status_text, "fehlt");
    }

    #[test]
    fn portal_state_shows_pause_quota_and_counts() {
        let store = Store::in_memory().unwrap();
        let now = Timestamp::now();
        let mut policy = Policy::in_memory();
        policy.pause(Portal::LinkedIn, PauseKind::Blocked, "HTTP 999", now);
        for _ in 0..25 {
            policy.record_access(Portal::Freelancermap, now);
        }
        let views = portal_views(&policy, &store, now).unwrap();
        let li = views.iter().find(|v| v.portal == Portal::LinkedIn).unwrap();
        assert_eq!(li.pause_reason.as_deref(), Some("HTTP 999"));
        assert!(!li.needs_account);
        let fm = views
            .iter()
            .find(|v| v.portal == Portal::Freelancermap)
            .unwrap();
        assert!(fm.next_free_at.is_some());
        assert_eq!((fm.used_hour, fm.cap_hour), (25, 25));
        assert!(
            views
                .iter()
                .find(|v| v.portal == Portal::FreelanceDe)
                .unwrap()
                .needs_account
        );
    }
}
