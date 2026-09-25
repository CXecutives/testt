//! A portal in the session window - everything the window layer needs to know about a
//! portal, in one place. The window itself knows no single portal: it loads addresses,
//! collects findings and hands them back here.

use serde::Deserialize;
use url::Url;

use super::PageOutcome;
use crate::portal::Portal;

/// What the probe script of a page reports - findings only, no judgement. Not every portal
/// fills every field; what it does not know stays empty.
#[expect(clippy::struct_excessive_bools, reason = "plain findings of a page")]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionPage {
    pub ok: bool,
    pub err: Option<String>,
    /// The HTTP status of the navigation; 0 = unknown (a web view without
    /// `responseStatus`: `WebKit` on macOS) - then `status_hint` speaks.
    pub status: u16,
    /// What the page's title or first heading says when the status is unknown:
    /// `throttled` (429, too many requests), `blocked` (403, access denied, a check),
    /// `gone` (404, not found) or empty.
    pub status_hint: String,
    pub url: String,
    pub has_logout: bool,
    pub has_expert_marker: bool,
    pub has_login_form: bool,
    pub has_captcha: bool,
    pub title: String,
    pub company: String,
    pub location: String,
    /// Facts of the page head, as the page words them.
    pub start: String,
    pub duration: String,
    pub remote: String,
    /// The rate (hourly or daily) as the page words it.
    pub rate: String,
    /// HTML of the description field.
    pub panel_html: Option<String>,
}

/// A portal that can be read in a session window.
pub struct PortalSite {
    pub portal: Portal,
    pub login_url: &'static str,
    pub logout_url: &'static str,
    /// Probe script: synchronous, in `try/catch`, always returns JSON ([`SessionPage`]).
    pub probe_js: &'static str,
    /// Addresses the window may load (hosts of this portal).
    pub is_allowed: fn(&Url) -> bool,
    /// One-off redirect right after the sign-in - fetch the same page again.
    pub is_postlogin: fn(&str) -> bool,
    /// Do the findings show an existing sign-in?
    pub signed_in: fn(&SessionPage) -> bool,
    /// Findings -> outcome matrix; the second value is the job id from the mail.
    pub judge: fn(&SessionPage, &str) -> PageOutcome,
}

impl PortalSite {
    /// The session window of a portal (from its adapter); `None` for portals without a
    /// sign-in.
    pub fn of(portal: Portal) -> Option<&'static PortalSite> {
        portal.adapter().session()
    }

    /// Window label - the same name as the profile folder.
    pub fn label(&self) -> String {
        crate::session_dir(self.portal)
    }
}

/// The result of a web view `eval`: JSON-encoded, so a JSON text inside a JSON string. A
/// raw value (not a string) is returned as it came.
pub fn eval_result(raw: String) -> String {
    serde_json::from_str(&raw).unwrap_or(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every portal with a sign-in has a session site, the others none - and every address a
    /// site names belongs to its own portal.
    #[test]
    fn every_portal_with_a_login_has_a_site() {
        for portal in Portal::ALL {
            let site = PortalSite::of(portal);
            assert_eq!(site.is_some(), portal.access().can_sign_in(), "{portal}");
            let Some(site) = site else { continue };
            assert_eq!(site.portal, portal);
            assert_eq!(site.label(), crate::session_dir(portal));
            for url in [site.login_url, site.logout_url] {
                let url = Url::parse(url).expect(url);
                assert!((site.is_allowed)(&url), "{url}");
            }
        }
    }

    /// Safety invariant: a session window loads nothing but addresses of its own portal - no
    /// foreign host, no other scheme, no address of the app itself.
    #[test]
    fn a_session_window_loads_nothing_but_its_own_portal() {
        let elsewhere = [
            "http://www.freelance.de/login.php",
            "https://www.freelance.de.example.org/login.php",
            "https://notfreelance.de/login.php",
            "https://www.freelancermap.de.evil.example/",
            // Another portal is as foreign as any other address.
            "https://www.freelancermap.de/login.html",
            "https://accounts.google.com/",
            "tauri://localhost/index.html",
            "file:///C:/Windows/system.ini",
            "data:text/html,<script>fetch('/')</script>",
            "javascript:fetch('https://www.freelance.de/')",
        ];
        for portal in Portal::ALL {
            let Some(site) = PortalSite::of(portal) else {
                continue;
            };
            for raw in elsewhere {
                let Ok(url) = Url::parse(raw) else { continue };
                assert!(!(site.is_allowed)(&url), "{portal}: {raw}");
            }
        }
    }

    #[test]
    fn eval_results_are_unwrapped_once() {
        assert_eq!(eval_result(r#""{\"ok\":true}""#.into()), r#"{"ok":true}"#);
        assert_eq!(eval_result(r#"{"ok":true}"#.into()), r#"{"ok":true}"#);
        assert_eq!(eval_result("not json".into()), "not json");
    }
}
