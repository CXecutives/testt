//! freelance.de. Alerts come from freelance.de. A guest sees the public project page with a
//! teaser (measured 242-306 characters); the full text is only readable signed in, in the
//! session window - and only if the user switched the sign-in on.
//!
//! The session window only collects findings by script (status, address, sign-in signs,
//! page fields, HTML of the description field); the guest page is read from its HTML with
//! the same rules. Judging them and turning them into text happens here in Rust. Company and
//! location show up for non-EXPERT members only as a placeholder - that one stays out.

use std::sync::LazyLock;

use scraper::{ElementRef, Html};
use url::Url;

use super::{
    Access, Css, JobLink, Portal, PortalAdapter, all_digits, host_and_segments, host_is, selector,
};
use crate::fetch::policy::Limits;
use crate::fetch::site::{PortalSite, SessionPage};
use crate::fetch::{Cause, PageFields, PageOutcome, Parsed, judge};
use crate::text::{html_to_text, one_line};

pub(super) struct FreelanceDe;

/// The session window of freelance.de.
pub static SITE: PortalSite = PortalSite {
    portal: Portal::FreelanceDe,
    login_url: "https://www.freelance.de/login.php",
    logout_url: "https://www.freelance.de/logout.php",
    probe_js: PROBE_JS,
    is_allowed: is_portal_url,
    is_postlogin,
    signed_in,
    judge: judge_page,
};

impl PortalAdapter for FreelanceDe {
    fn portal(&self) -> Portal {
        Portal::FreelanceDe
    }
    fn key(&self) -> &'static str {
        "freelance"
    }
    fn label(&self) -> &'static str {
        "freelance.de"
    }
    fn file_tag(&self) -> &'static str {
        "Freelance"
    }
    fn home_url(&self) -> &'static str {
        "https://www.freelance.de/"
    }
    fn sender_domains(&self) -> &'static [&'static str] {
        &["freelance.de"]
    }
    fn search_terms(&self) -> &'static [&'static str] {
        &["freelance.de"]
    }
    /// On top comes the dwell time in the session window (`DWELL_SECS`). The gap here also
    /// holds across runs, cancellations and restarts.
    fn limits(&self) -> Limits {
        Limits {
            pace_ms: 10_000..=20_000,
            per_hour: 15,
            per_day: 30,
        }
    }
    /// Without a sign-in a guest still gets the teaser.
    fn access(&self) -> Access {
        Access::Session { required: false }
    }

    /// `/project/index.php?id=<ID>`, `/projekte/projekt-<ID>[-slug]` or
    /// `/projekt-<ID>[-slug]` - only at these positions (a blog article
    /// ".../blog/projekt-2025-..." is no project).
    fn job_link(&self, url: &Url) -> Option<JobLink> {
        let (host, segments) = host_and_segments(url)?;
        if !host_is(&host, "freelance.de") {
            return None;
        }
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        let id = if segments == ["project", "index.php"] {
            let (_, id) = url
                .query_pairs()
                .find(|(k, _)| k.eq_ignore_ascii_case("id"))?;
            all_digits(&id, 1).then(|| id.into_owned())?
        } else {
            let (["projekte", segment, ..] | [segment, ..]) = segments.as_slice() else {
                return None;
            };
            let digits = segment.strip_prefix("projekt-")?.split('-').next()?;
            all_digits(digits, 4).then(|| digits.to_string())?
        };
        super::link(Portal::FreelanceDe, id)
    }

    /// Checked: leads without an account to the project page (with the registration wall).
    fn canonical_url(&self, id: &str) -> Option<Url> {
        if !all_digits(id, 1) {
            return None;
        }
        Url::parse(&format!(
            "https://www.freelance.de/project/index.php?id={id}"
        ))
        .ok()
    }

    fn redirect_outcome(&self, _path: &str) -> PageOutcome {
        PageOutcome::Suspicious(Cause::RedirectNotFollowed)
    }

    /// The public project page: the teaser, rarely the full text. Measured anonymously: an
    /// expired project leads to a project list or category - for a guest that is "gone".
    fn guest_page(&self, html: &str, path: &str, link: &JobLink) -> PageOutcome {
        if path.starts_with("/login") {
            return PageOutcome::Blocked(Cause::LoginWall);
        }
        if is_listing(path) {
            return PageOutcome::Gone;
        }
        let page = guest_findings(html);
        if page.has_captcha && page.panel_html.is_none() {
            return PageOutcome::Blocked(Cause::Captcha);
        }
        // The id from the mail is in the path of every project address; the fetch address
        // itself carries it in the query.
        if !path.contains(&link.key.id) && path != "/project/index.php" {
            return PageOutcome::Suspicious(Cause::WrongPage);
        }
        let full = page.panel_html.as_deref().map(html_to_text);
        let text = full.as_deref().map(cut_at_end_markers);
        let fields = page_fields(&page.title, &page.company, &page.location);
        if is_teaser(full.as_deref(), text.as_deref()) || (page.has_expert_marker && full.is_none())
        {
            return PageOutcome::Teaser {
                text: text.unwrap_or_default(),
                fields: Some(fields).filter(|f| *f != PageFields::default()),
            };
        }
        judge(Parsed {
            text,
            fields,
            ..Parsed::default()
        })
    }

    fn session(&self) -> Option<&'static PortalSite> {
        Some(&SITE)
    }
}

/// Probe script: synchronous, in `try/catch`, always returns JSON. No timers, no logic in
/// the page's JS (a hidden web view throttles timers).
///
/// The title sits measured in the `h1` of the project head. Where company and location
/// stand is only **assumed**: the script looks for a labelled row in the project head - to
/// be measured on a real signed-in page. German page words, do not translate.
pub const PROBE_JS: &str = r#"(() => { try {
  const text = (el) => (el && el.textContent || '').replace(/\s+/g, ' ').trim();
  const heading = [...document.querySelectorAll('h1, h2, h3')].find((h) => /Projektbeschreibung/i.test(text(h)));
  const box = heading && (heading.closest('.panel') || heading.parentElement);
  const body = box && (box.querySelector('.panel-body') || box);
  const header = document.querySelector('.panel-body.project-header');
  const labelled = (re) => {
    for (const el of (header ? header.querySelectorAll('*') : [])) {
      if (el.children.length) continue;
      const label = text(el);
      if (!re.test(label)) continue;
      const value = text(el.nextElementSibling) || text(el.parentElement).slice(label.length).trim();
      if (value) return value;
    }
    return '';
  };
  return JSON.stringify({
    ok: true,
    status: performance.getEntriesByType('navigation')[0]?.responseStatus ?? 0,
    url: location.href,
    hasLogout: !!document.querySelector('a[href*="logout.php"]'),
    hasExpertMarker: /für EXPERT-Mitglieder sichtbar/i.test(document.body ? document.body.innerText : ''),
    hasLoginForm: !!document.querySelector('#username') && !!document.querySelector('#password'),
    hasCaptcha: !!document.querySelector('.g-recaptcha, .h-captcha, [data-sitekey], iframe[src*="captcha"], iframe[src*="challenges.cloudflare"]'),
    title: text(header && header.querySelector('h1')),
    company: labelled(/^(?:Firma|Unternehmen|Projektanbieter|Auftraggeber|Kunde)\s*:?$/i),
    location: labelled(/^(?:Ort|Einsatzort|Standort|PLZ\s*\/?\s*Ort)\s*:?$/i),
    panelHtml: body ? body.innerHTML : null,
  });
} catch (e) { return JSON.stringify({ ok: false, err: String(e), url: String(location.href) }); } })()"#;

/// Signed in: there is a sign-out link (measured).
pub fn signed_in(page: &SessionPage) -> bool {
    page.has_logout
}

/// After the sign-in freelance.de redirects the first page once to this address. The portal
/// rule comes first here too: the same path on a foreign host is no redirect of the portal
/// and must not trigger a second access.
pub fn is_postlogin(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| is_portal_url(&u) && u.path().starts_with("/promotion/postlogin"))
}

/// A page of the portal: https on `freelance.de` or a subdomain. Everything else (the web
/// view's error page `chrome-error://...`, `about:blank`, a foreign host) is none.
pub fn is_portal_url(url: &Url) -> bool {
    // The URL parser already lower-cases hosts of http(s) addresses.
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| host_is(host, "freelance.de"))
}

/// Shorter than this and with a registration call: only the teaser (measured 242-306
/// characters).
const TEASER_MAX_CHARS: usize = 500;

/// Registration call of the teaser - also an end marker. German page words, do not
/// translate.
const REGISTER_CALL: &str = "Kostenlos registrieren";

/// Behind these headings the description ends (ads, contact, similar projects). German page
/// words, do not translate.
const END_MARKERS: &[&str] = &[
    REGISTER_CALL,
    "Kontaktdaten",
    "Ähnliche Projekte",
    "Kategorien und Skills",
    "Sie suchen Freelancer?",
];

/// Findings -> outcome matrix. `project_id`: the id from the mail.
///
/// "Sign-in needed" only where a sign-in can help (no sign-out link). Whatever is wrong
/// despite a sign-out link is suspicious - signing in again would change nothing and run in
/// circles, the breaker applies instead.
pub fn judge_page(page: &SessionPage, project_id: &str) -> PageOutcome {
    // First: is this a page of the portal at all? The web view reports a failed navigation
    // as "loaded" too - on its own error page.
    let Some(url) = Url::parse(&page.url).ok().filter(is_portal_url) else {
        return PageOutcome::NetError {
            timeout: false,
            cause: Cause::NotLoaded,
        };
    };
    if !page.ok {
        return PageOutcome::Suspicious(Cause::ProbeFailed);
    }
    // A security check blocks - unless the widget only sits passively in a normally loaded
    // project page with a description.
    if page.has_captcha && (page.panel_html.is_none() || page.status != 200) {
        return PageOutcome::Blocked(Cause::Captcha);
    }
    if let Some(outcome) = status_outcome(page.status) {
        return outcome;
    }
    let path = url.path().to_ascii_lowercase();
    if path.starts_with("/login") || (page.has_login_form && !page.has_logout) {
        return PageOutcome::LoginRequired(Cause::LoginPage);
    }
    if !page.has_logout {
        return PageOutcome::LoginRequired(Cause::NoLogoutLink);
    }
    // Expired project: freelance.de redirects to a project list - with a notice
    // (`/projekte?info_message=...`) or to the project's category. Measured only signed out;
    // "gone" therefore only counts in a confirmed session. A project page always has
    // "/projekt-<ID>" in its path.
    if !page.url.contains(project_id) && is_listing(&path) {
        return PageOutcome::Gone;
    }
    // Teaser: a short text with a registration call (measured 277 characters; the call is
    // an end marker too, hence checked before cutting) or only the "EXPERT" notice without a
    // text field. A long text with the call at its end is the full text.
    let full = page.panel_html.as_deref().map(html_to_text);
    let text = full.as_deref().map(cut_at_end_markers);
    if is_teaser(full.as_deref(), text.as_deref()) || (page.has_expert_marker && full.is_none()) {
        return PageOutcome::Suspicious(Cause::TeaserDespiteSession);
    }
    // The right page? The id from the mail is in every form of the project address.
    if !page.url.contains(project_id) {
        return PageOutcome::Suspicious(Cause::WrongPage);
    }
    judge(Parsed {
        text,
        fields: page_fields(&page.title, &page.company, &page.location),
        ..Parsed::default()
    })
}

/// The findings of the probe script, read from the HTML of a guest page (same rules).
fn guest_findings(html: &str) -> SessionPage {
    static HEADINGS: Css = LazyLock::new(|| selector("h1, h2, h3"));
    static PANEL_BODY: Css = LazyLock::new(|| selector(".panel-body"));
    static HEADER: Css = LazyLock::new(|| selector(".panel-body.project-header"));
    static TITLE: Css = LazyLock::new(|| selector("h1"));
    static ANY: Css = LazyLock::new(|| selector("*"));
    static CAPTCHA: Css = LazyLock::new(|| {
        selector(
            r#".g-recaptcha, .h-captcha, [data-sitekey], iframe[src*="captcha"], iframe[src*="challenges.cloudflare"]"#,
        )
    });
    let doc = Html::parse_document(html);
    let text = |el: ElementRef<'_>| one_line(&el.text().collect::<String>());
    let heading = doc
        .select(&HEADINGS)
        .find(|h| text(*h).to_lowercase().contains("projektbeschreibung"));
    let panel = heading.map(|h| {
        let parent = h.parent().and_then(ElementRef::wrap).unwrap_or(h);
        let panel = h
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find(|a| a.value().classes().any(|c| c == "panel"))
            .unwrap_or(parent);
        panel.select(&PANEL_BODY).next().unwrap_or(panel)
    });
    let header = doc.select(&HEADER).next();
    // A leaf with the label, the value in the next element or after the label.
    let labelled = |labels: &[&str]| -> String {
        let Some(header) = header else {
            return String::new();
        };
        for el in header.select(&ANY) {
            if el.children().any(|c| c.value().is_element()) {
                continue;
            }
            let label = text(el);
            let bare = label.trim_end_matches(':').trim().to_lowercase();
            if !labels.contains(&bare.as_str()) {
                continue;
            }
            let next = el.next_siblings().find_map(ElementRef::wrap).map(text);
            let value = next.filter(|v| !v.is_empty()).unwrap_or_else(|| {
                el.parent()
                    .and_then(ElementRef::wrap)
                    .and_then(|p| text(p).get(label.len()..).map(|v| v.trim().to_string()))
                    .unwrap_or_default()
            });
            if !value.is_empty() {
                return value;
            }
        }
        String::new()
    };
    SessionPage {
        ok: true,
        status: 200,
        has_expert_marker: text(doc.root_element())
            .to_lowercase()
            .contains("für expert-mitglieder sichtbar"),
        has_captcha: doc.select(&CAPTCHA).next().is_some(),
        title: header
            .and_then(|h| h.select(&TITLE).next())
            .map(text)
            .unwrap_or_default(),
        // German page labels, do not translate.
        company: labelled(&[
            "firma",
            "unternehmen",
            "projektanbieter",
            "auftraggeber",
            "kunde",
        ]),
        location: labelled(&["ort", "einsatzort", "standort", "plz / ort", "plz/ort"]),
        panel_html: panel.map(|p| p.inner_html()),
        ..SessionPage::default()
    }
}

/// Status codes that decide without looking at the content.
fn status_outcome(status: u16) -> Option<PageOutcome> {
    Some(match status {
        429 => PageOutcome::Throttled(Cause::Http(429)),
        403 => PageOutcome::Blocked(Cause::Http(403)),
        404 | 410 => PageOutcome::Gone,
        500..=599 => PageOutcome::Throttled(Cause::Http(status)),
        _ => return None,
    })
}

/// Short text with the registration call.
fn is_teaser(full: Option<&str>, cut: Option<&str>) -> bool {
    full.is_some_and(|t| t.contains(REGISTER_CALL))
        && cut.is_some_and(|t| t.chars().count() < TEASER_MAX_CHARS)
}

/// Page fields for the table. The store keeps empty values out - so the placeholder "für
/// EXPERT-Mitglieder sichtbar" becomes an empty field instead of a name that is none.
fn page_fields(title: &str, company: &str, location: &str) -> PageFields {
    let value = |raw: &str| {
        let value = one_line(raw);
        if value.to_lowercase().contains("expert-mitglieder") {
            String::new()
        } else {
            value
        }
    };
    PageFields {
        title: value(title),
        company: value(company),
        location: value(location),
    }
}

/// Project list or category (no single project page).
fn is_listing(path: &str) -> bool {
    path == "/projekte"
        || path == "/projekte/"
        || (path.starts_with("/projekte/") && !path.contains("/projekt-"))
}

fn cut_at_end_markers(text: &str) -> String {
    let end = END_MARKERS
        .iter()
        .filter_map(|m| text.find(m))
        .min()
        .unwrap_or(text.len());
    text[..end].trim_end().to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::portal::job_link;

    /// A public project page as a guest sees it (invented content, layout of the probe).
    pub(crate) fn guest_html(panel: &str) -> String {
        format!(
            r#"<html><body>
            <div class="panel"><div class="panel-body project-header"><h1>Interim Controller (m/w/d)</h1>
              <ul><li><span>Ort:</span> <span>Hamburg</span></li><li><span>Start:</span> <span>01.11.2026</span></li>
              <li><span>Dauer:</span> <span>6 Monate</span></li><li><span>Remote:</span> <span>100 %</span></li></ul></div></div>
            <div class="panel"><div class="panel-heading"><h2>Projektbeschreibung</h2></div><div class="panel-body">{panel}</div></div>
            </body></html>"#
        )
    }

    pub(crate) const TEASER: &str = "<p>Derzeit suchen wir für unseren Kunden einen Controller.</p><p>Kostenlos registrieren und alle Details sehen</p>";

    fn guest(html: &str, path: &str) -> PageOutcome {
        let link = job_link("https://www.freelance.de/project/index.php?id=1255067").unwrap();
        FreelanceDe.guest_page(html, path, &link)
    }

    #[test]
    fn a_guest_gets_the_teaser_with_the_page_fields() {
        let teaser = guest_html(TEASER);
        match guest(&teaser, "/projekte/projekt-1255067-interim-controlling") {
            PageOutcome::Teaser { text, fields } => {
                assert_eq!(
                    text,
                    "Derzeit suchen wir für unseren Kunden einen Controller."
                );
                let fields = fields.unwrap();
                assert_eq!(fields.title, "Interim Controller (m/w/d)");
                assert_eq!(fields.location, "Hamburg");
                assert_eq!(fields.company, "");
            }
            other => panic!("{other:?}"),
        }
        // The fetch address itself (no redirect): the id is in the query.
        assert!(matches!(
            guest(&teaser, "/project/index.php"),
            PageOutcome::Teaser { .. }
        ));
        // Only the EXPERT notice, no text field: an empty teaser - still the right page.
        let expert = "<html><body><p>Details für EXPERT-Mitglieder sichtbar</p></body></html>";
        assert_eq!(
            guest(expert, "/project/index.php"),
            PageOutcome::Teaser {
                text: String::new(),
                fields: None
            }
        );
    }

    /// What a guest sees in full is the full text.
    #[test]
    fn a_full_text_for_a_guest_is_the_full_text() {
        let long = format!("<p>{}</p>", "Aufgaben und Anforderungen. ".repeat(6));
        assert!(matches!(
            guest(&guest_html(&long), "/projekte/projekt-1255067-x"),
            PageOutcome::Text { short: false, .. }
        ));
    }

    #[test]
    fn guest_walls_lists_and_wrong_pages() {
        let teaser = guest_html(TEASER);
        assert_eq!(
            guest(&teaser, "/login.php"),
            PageOutcome::Blocked(Cause::LoginWall)
        );
        assert_eq!(
            guest("<html>Liste</html>", "/projekte/it-entwicklung-projekte"),
            PageOutcome::Gone
        );
        assert_eq!(
            guest(&teaser, "/projekte/projekt-9999999-x"),
            PageOutcome::Suspicious(Cause::WrongPage)
        );
        assert_eq!(
            guest(r#"<div class="g-recaptcha"></div>"#, "/project/index.php"),
            PageOutcome::Blocked(Cause::Captcha)
        );
        assert_eq!(
            guest("<html>nichts</html>", "/project/index.php"),
            PageOutcome::Suspicious(Cause::NoDescription)
        );
    }

    fn page(url: &str, logout: bool, panel: Option<&str>) -> SessionPage {
        SessionPage {
            ok: true,
            status: 200,
            url: url.into(),
            has_logout: logout,
            panel_html: panel.map(str::to_string),
            ..SessionPage::default()
        }
    }

    const URL: &str = "https://www.freelance.de/projekte/projekt-1255067-sap-s4hana";
    const ID: &str = "1255067";

    fn is_login(outcome: &PageOutcome) -> bool {
        matches!(outcome, PageOutcome::LoginRequired(_))
    }

    fn is_suspicious(outcome: &PageOutcome) -> bool {
        matches!(outcome, PageOutcome::Suspicious(_))
    }

    /// Measured on 2026-09-19 (anonymous): expired project -> category page; running project
    /// -> teaser (277 characters) with a registration call on the project address.
    #[test]
    fn measured_expired_project_and_teaser() {
        let expired = page(
            "https://www.freelance.de/projekte/IT-Entwicklung-Projekte/Berater-Und-Spezialisten-Projekte",
            false,
            None,
        );
        // Signed out, only a sign-in helps; "gone" only in a confirmed session.
        assert!(is_login(&judge_page(&expired, "1242180")));
        let logged_in = SessionPage {
            has_logout: true,
            ..expired
        };
        assert_eq!(judge_page(&logged_in, "1242180"), PageOutcome::Gone);

        let teaser = SessionPage {
            has_expert_marker: true,
            ..page(
                "https://www.freelance.de/projekte/projekt-1287763-Interim-Controlling",
                false,
                Some(
                    "<p>Derzeit suchen wir für unseren Kunden einen Controller.</p><p>Kostenlos registrieren und alle Details sehen</p>",
                ),
            )
        };
        assert!(is_login(&judge_page(&teaser, "1287763")));
        // A teaser despite a sign-out link: signing in again would not help - suspicious, so
        // that the breaker applies instead of a sign-in loop.
        let teaser_logged_in = SessionPage {
            has_logout: true,
            ..teaser
        };
        assert_eq!(
            judge_page(&teaser_logged_in, "1287763"),
            PageOutcome::Suspicious(Cause::TeaserDespiteSession)
        );
    }

    /// A failed navigation (offline, DNS, TLS) is reported as loaded too - on the web view's
    /// own error page. That is a network error, not a sign-in wall.
    #[test]
    fn a_page_outside_the_portal_is_a_network_error() {
        for url in [
            "chrome-error://chromewebdata/",
            "about:blank",
            "http://www.freelance.de/projekte/projekt-1255067-sap",
            "https://www.example.org/projekte/projekt-1255067",
            "https://freelance.de.example.org/projekte/projekt-1255067",
            "https://notfreelance.de/projekte/projekt-1255067",
            "",
        ] {
            let mut error_page = page(url, false, None);
            error_page.status = 0;
            assert!(
                matches!(
                    judge_page(&error_page, ID),
                    PageOutcome::NetError { timeout: false, .. }
                ),
                "{url}"
            );
        }
        // A failed probe on the error page too.
        let failed = SessionPage {
            ok: false,
            err: Some("TypeError".into()),
            url: "chrome-error://chromewebdata/".into(),
            ..SessionPage::default()
        };
        assert!(matches!(
            judge_page(&failed, ID),
            PageOutcome::NetError { .. }
        ));
        assert!(is_portal_url(&Url::parse("https://freelance.de/").unwrap()));
        assert!(is_portal_url(
            &Url::parse("https://WWW.Freelance.DE/x").unwrap()
        ));
    }

    /// Title, company and location of the page replace the mail heuristics - the EXPERT
    /// placeholder never: there the field stays empty, the store keeps the mail value.
    #[test]
    fn page_fields_replace_the_mail_guess_but_never_the_placeholder() {
        let long = format!("<p>{}</p>", "Aufgaben und Anforderungen. ".repeat(6));
        let full = SessionPage {
            title: "  Interim\n Controller (m/w/d) ".into(),
            company: "Muster Consulting GmbH".into(),
            location: "D-20038 Hamburg".into(),
            ..page(URL, true, Some(&long))
        };
        match judge_page(&full, ID) {
            PageOutcome::Text { fields, .. } => assert_eq!(
                fields,
                Some(PageFields {
                    title: "Interim Controller (m/w/d)".into(),
                    company: "Muster Consulting GmbH".into(),
                    location: "D-20038 Hamburg".into(),
                })
            ),
            other => panic!("{other:?}"),
        }
        let hidden = SessionPage {
            company: "für EXPERT-Mitglieder sichtbar".into(),
            location: "Für EXPERT-Mitglieder sichtbar".into(),
            ..full
        };
        match judge_page(&hidden, ID) {
            PageOutcome::Text { fields, .. } => assert_eq!(
                fields,
                Some(PageFields {
                    title: "Interim Controller (m/w/d)".into(),
                    company: String::new(),
                    location: String::new(),
                })
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn logged_in_project_page() {
        let long = format!(
            "<p>{}</p><h3>Kontaktdaten</h3><p>Muster</p>",
            "Aufgaben und Anforderungen. ".repeat(6)
        );
        match judge_page(&page(URL, true, Some(&long)), ID) {
            PageOutcome::Text {
                text,
                short,
                fields,
                ..
            } => {
                assert!(!short && fields.is_none());
                assert!(
                    !text.contains("Kontaktdaten") && !text.contains("Muster"),
                    "ends at the marker"
                );
            }
            other => panic!("{other:?}"),
        }
        // Short, but signed in and in the description field: verified short.
        assert!(matches!(
            judge_page(&page(URL, true, Some("<p>Kurzprojekt SAP.</p>")), ID),
            PageOutcome::Text { short: true, .. }
        ));
    }

    /// "Sign-in needed" only where a sign-in can help.
    #[test]
    fn login_only_when_it_can_help() {
        assert!(is_login(&judge_page(
            &page("https://www.freelance.de/login.php", false, None),
            ID
        )));
        let mut form = page(URL, false, None);
        form.has_login_form = true;
        assert!(is_login(&judge_page(&form, ID)));
        assert!(is_login(&judge_page(
            &page(URL, false, Some("<p>Text</p>")),
            ID
        )));
        // Signed out and only the EXPERT notice: sign-in.
        let mut anonymous_teaser = page(URL, false, None);
        anonymous_teaser.has_expert_marker = true;
        assert!(is_login(&judge_page(&anonymous_teaser, ID)));
    }

    /// With a sign-out link a teaser is suspicious, never "sign-in needed".
    #[test]
    fn a_teaser_despite_a_session_is_suspicious() {
        let mut expert_only = page(URL, true, None);
        expert_only.has_expert_marker = true;
        assert!(is_suspicious(&judge_page(&expert_only, ID)));
        let cta = page(
            URL,
            true,
            Some("<p>Kurz.</p><p>Kostenlos registrieren und mehr lesen</p>"),
        );
        assert!(is_suspicious(&judge_page(&cta, ID)));
        let only_cta = page(URL, true, Some("<p>Kostenlos registrieren</p>"));
        assert!(is_suspicious(&judge_page(&only_cta, ID)));
        // A long text with the call at its end: the full text, the call is cut off.
        let long = format!(
            "<p>{}</p><p>Kostenlos registrieren und mehr lesen</p>",
            "Aufgaben und Anforderungen des Projekts. ".repeat(20)
        );
        match judge_page(&page(URL, true, Some(&long)), ID) {
            PageOutcome::Text { text, .. } => assert!(!text.contains("registrieren"), "{text}"),
            other => panic!("{other:?}"),
        }
    }

    /// A captcha blocks, unless it sits passively in a normally loaded page with a
    /// description.
    #[test]
    fn a_captcha_blocks_only_without_a_description() {
        let long = format!("<p>{}</p>", "Aufgaben und Anforderungen. ".repeat(6));
        let mut passive = page(URL, true, Some(&long));
        passive.has_captcha = true;
        assert!(matches!(
            judge_page(&passive, ID),
            PageOutcome::Text { short: false, .. }
        ));
        let mut wall = page(URL, true, None);
        wall.has_captcha = true;
        assert!(matches!(
            judge_page(&wall, ID),
            PageOutcome::Blocked(Cause::Captcha)
        ));
        // With an error code it is always a block - even with a text field.
        let mut with_error = page(URL, true, Some(&long));
        with_error.has_captcha = true;
        with_error.status = 503;
        assert!(matches!(
            judge_page(&with_error, ID),
            PageOutcome::Blocked(_)
        ));
        // Signed out too: a check page is a block, not a sign-in.
        let mut anonymous = page(URL, false, None);
        anonymous.has_captcha = true;
        assert!(matches!(
            judge_page(&anonymous, ID),
            PageOutcome::Blocked(_)
        ));
    }

    #[test]
    fn gone_throttled_and_wrong_page() {
        let dead = page(
            "https://www.freelance.de/projekte?info_message=Projekt+nicht+gefunden",
            true,
            None,
        );
        assert_eq!(judge_page(&dead, ID), PageOutcome::Gone);
        let dead_anonymous = SessionPage {
            has_logout: false,
            ..dead
        };
        assert!(is_login(&judge_page(&dead_anonymous, ID)));
        let mut limited = page(URL, true, None);
        limited.status = 429;
        assert!(matches!(
            judge_page(&limited, ID),
            PageOutcome::Throttled(Cause::Http(429))
        ));
        assert_eq!(
            judge_page(
                &page(
                    "https://www.freelance.de/projekte/projekt-9999999-x",
                    true,
                    Some("<p>x</p>")
                ),
                ID
            ),
            PageOutcome::Suspicious(Cause::WrongPage)
        );
        assert!(is_suspicious(&judge_page(
            &SessionPage {
                ok: false,
                err: Some("TypeError".into()),
                url: URL.into(),
                ..SessionPage::default()
            },
            ID
        )));
        assert!(is_postlogin(
            "https://www.freelance.de/promotion/postlogin.php?x=1"
        ));
        assert!(!is_postlogin(URL));
        // The same path elsewhere is no redirect of the portal: no second access.
        assert!(!is_postlogin("https://example.com/promotion/postlogin.php"));
        assert!(!is_postlogin(
            "http://www.freelance.de/promotion/postlogin.php"
        ));
    }
}
