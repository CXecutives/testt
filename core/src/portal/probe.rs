//! A fourth portal for tests only. It exists nowhere but here and in the registry - the
//! fetch engine, the HTTP client and the mail scan handle it without knowing it.

use url::Url;

use super::{
    Access, Facts, JobLink, Portal, PortalAdapter, all_digits, host_and_segments, host_is,
};
use crate::fetch::policy::Limits;
use crate::fetch::{Cause, PageOutcome, Parsed, judge};
use crate::text::html_to_text;

pub(super) struct Probe;

impl PortalAdapter for Probe {
    fn portal(&self) -> Portal {
        Portal::Probe
    }
    fn key(&self) -> &'static str {
        "probe"
    }
    fn label(&self) -> &'static str {
        "probe.example"
    }
    fn file_tag(&self) -> &'static str {
        "Probe"
    }
    fn home_url(&self) -> &'static str {
        "https://jobs.probe.example/"
    }
    fn sender_domains(&self) -> &'static [&'static str] {
        &["probe.example"]
    }
    fn search_terms(&self) -> &'static [&'static str] {
        &["probe.example"]
    }
    fn limits(&self) -> Limits {
        Limits {
            pace_ms: 1_000..=1_000,
            per_hour: 3,
            per_day: 5,
        }
    }
    fn access(&self) -> Access {
        Access::Guest
    }
    /// `/job/<ID>`.
    fn job_link(&self, url: &Url) -> Option<JobLink> {
        let (host, segments) = host_and_segments(url)?;
        if !host_is(&host, "probe.example") {
            return None;
        }
        match segments.as_slice() {
            [job, id] if job == "job" && all_digits(id, 1) => {
                super::link(Portal::Probe, id.clone())
            }
            _ => None,
        }
    }
    fn canonical_url(&self, id: &str) -> Option<Url> {
        Url::parse(&format!("https://jobs.probe.example/job/{id}")).ok()
    }
    fn redirect_outcome(&self, _path: &str) -> PageOutcome {
        PageOutcome::Suspicious(Cause::RedirectNotFollowed)
    }
    fn parser_version(&self) -> u32 {
        1
    }
    fn parse_facts(&self, _html: &str) -> Facts {
        Facts::default()
    }
    /// The whole body is the description.
    fn guest_page(&self, html: &str, _path: &str, _link: &JobLink) -> PageOutcome {
        judge(Parsed {
            text: Some(html_to_text(html)),
            ..Parsed::default()
        })
    }
}
