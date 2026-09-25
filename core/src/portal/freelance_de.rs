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
    Access, Css, Facts, JobLink, Portal, PortalAdapter, all_digits, host_and_segments, host_is,
    selector,
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
            per_hour: 20,
            per_day: 60,
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
            // `projekt-<ID>`, in old links also `projekt<ID>`.
            let rest = segment.strip_prefix("projekt")?;
            let digits = rest.strip_prefix('-').unwrap_or(rest).split('-').next()?;
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

    /// A redirect the guest client did not follow (another host, a third hop): to a sign-in
    /// or registration page it is a wall, to a check page a check - both stop the portal at
    /// once. Anything else stays suspicious.
    fn redirect_outcome(&self, path: &str) -> PageOutcome {
        wall_path(path).map_or(
            PageOutcome::Suspicious(Cause::RedirectNotFollowed),
            PageOutcome::Blocked,
        )
    }

    /// The public project page: the teaser, rarely the full text. Measured anonymously: an
    /// expired project leads to a project list or category - for a guest that is "gone".
    fn guest_page(&self, html: &str, path: &str, link: &JobLink) -> PageOutcome {
        if let Some(cause) = wall_path(path) {
            return PageOutcome::Blocked(cause);
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
        let own_page = Url::parse("https://www.freelance.de")
            .and_then(|base| base.join(path))
            .is_ok_and(|url| is_project_page(&url, &link.key.id));
        if !own_page && path != "/project/index.php" {
            return PageOutcome::Suspicious(Cause::WrongPage);
        }
        let full = page.panel_html.as_deref().map(html_to_text);
        let text = full.as_deref().map(cut_at_end_markers);
        let fields = page_fields(&page.title, &page.company, &page.location);
        // A guest's description with the registration call is the teaser, however long:
        // only a sign-in brings the rest. Only the EXPERT notice in place of the
        // description, on a project page (its head names the title), is an empty teaser;
        // a page without the description field and without that notice is suspicious.
        let register_call = full.as_deref().is_some_and(|t| t.contains(REGISTER_CALL));
        let expert_only = page.has_expert_marker && full.is_none() && !page.title.is_empty();
        if register_call || expert_only {
            return PageOutcome::Teaser {
                text: text.unwrap_or_default(),
                fields: Some(fields).filter(|f| *f != PageFields::default()),
                facts: facts_of(&page),
            };
        }
        judge(Parsed {
            text,
            fields,
            facts: facts_of(&page),
            ..Parsed::default()
        })
    }

    fn session(&self) -> Option<&'static PortalSite> {
        Some(&SITE)
    }

    fn parser_version(&self) -> u32 {
        PARSER_VERSION
    }

    fn parse_facts(&self, html: &str) -> Facts {
        facts_of(&guest_findings(html))
    }
}

/// Bump whenever the guest page or the probe is read differently (requeues failed jobs).
/// 2: end markers only as headings, the description heading exactly, the EXPERT notice only
/// in place of the description, a guest's register call is a teaser at any length, the
/// rate and more head labels, the status hint when the status is unknown.
const PARSER_VERSION: u32 = 2;

/// The facts of the project head (as the page words them).
fn facts_of(page: &SessionPage) -> Facts {
    let mut facts = Facts {
        start: Facts::value(&page.start),
        duration: Facts::value(&page.duration),
        rate: Facts::value(&page.rate),
        ..Facts::default()
    };
    facts.set_remote(&page.remote);
    facts
}

/// A sign-in, registration or check page by its address (lower case). German path
/// words, do not translate.
fn wall_path(path: &str) -> Option<Cause> {
    let stems: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.split('.').next().unwrap_or_default().replace('_', "-"))
        .collect();
    let any = |words: &[&str]| stems.iter().any(|s| words.contains(&s.as_str()));
    if any(&["captcha", "challenge", "cdn-cgi", "checkpoint"]) {
        Some(Cause::Captcha)
    } else if any(&[
        "login",
        "signin",
        "sign-in",
        "anmelden",
        "anmeldung",
        "register",
        "registrieren",
        "registrierung",
    ]) {
        Some(Cause::LoginWall)
    } else {
        None
    }
}

/// Probe script: synchronous, in `try/catch`, always returns JSON. No timers, no logic in
/// the page's JS (a hidden web view throttles timers).
///
/// The title sits measured in the `h1` of the project head. Where company, location,
/// start, duration, remote share and rate stand is only **assumed**: the script looks for a
/// labelled row in the project head - a text label, or an icon whose tooltip names it - to
/// be measured on a real signed-in page. The description is the panel under the heading
/// that is exactly "Projektbeschreibung" (a project title may contain the word), outside
/// the head; it ends at the first heading of an end section. `status` 0 means the web view
/// knows no status (WebKit): then `statusHint` reads the page's title and first heading.
/// German page words, do not translate.
pub const PROBE_JS: &str = r#"(() => { try {
  const text = (el) => (el && el.textContent || '').replace(/\s+/g, ' ').trim();
  const header = document.querySelector('.panel-body.project-header');
  const inHeader = (el) => !!(header && header.contains(el));
  const heading = [...document.querySelectorAll('h1, h2, h3')]
    .find((h) => !inHeader(h) && /^Projektbeschreibung\s*:?$/i.test(text(h)));
  const box = heading && (heading.closest('.panel') || heading.parentElement);
  const body = box && (box.querySelector('.panel-body') || box);
  let panelHtml = null;
  if (body) {
    const copy = body.cloneNode(true);
    const end = [...copy.querySelectorAll('h2, h3, h4, .panel-heading')]
      .find((h) => /^(?:Kontaktdaten|Ähnliche Projekte|Kategorien und Skills|Sie suchen Freelancer\?)/i.test(text(h)));
    if (end) {
      let node = end;
      while (node) { const next = node.nextSibling; node.remove(); node = next; }
    }
    panelHtml = copy.innerHTML;
  }
  const labelled = (re) => {
    for (const el of (header ? header.querySelectorAll('*') : [])) {
      if (el.children.length) continue;
      const own = text(el);
      const tip = el.getAttribute('title') || el.getAttribute('data-original-title') || '';
      const label = own || tip.trim();
      if (!re.test(label)) continue;
      const value = text(el.nextElementSibling) || text(el.parentElement).slice(own.length).trim();
      if (value) return value;
    }
    return '';
  };
  const expert = /für EXPERT-Mitglieder sichtbar/i;
  const hasExpertMarker = (body && expert.test(text(body)))
    || [...document.querySelectorAll('body *')].some((el) => !el.children.length && !inHeader(el) && expert.test(text(el)));
  const headline = (document.title || '') + ' ' + text(document.querySelector('h1'));
  const statusHint = /too many requests|zu viele anfragen|\b429\b/i.test(headline) ? 'throttled'
    : /access denied|zugriff verweigert|forbidden|attention required|just a moment|\b403\b/i.test(headline) ? 'blocked'
    : /seite nicht gefunden|page not found|nicht gefunden|\b404\b/i.test(headline) ? 'gone' : '';
  return JSON.stringify({
    ok: true,
    status: performance.getEntriesByType('navigation')[0]?.responseStatus ?? 0,
    statusHint,
    url: location.href,
    hasLogout: !!document.querySelector('a[href*="logout.php"]'),
    hasExpertMarker,
    hasLoginForm: !!document.querySelector('#username') && !!document.querySelector('#password'),
    hasCaptcha: !!document.querySelector('.g-recaptcha, .h-captcha, [data-sitekey], iframe[src*="captcha"], iframe[src*="challenges.cloudflare"], #challenge-form, #captcha-internal, form[action*="captcha"], #cf-challenge-running'),
    title: text(header && header.querySelector('h1')),
    company: labelled(/^(?:Firma|Unternehmen|Projektanbieter|Auftraggeber|Kunde)\s*:?$/i),
    location: labelled(/^(?:Ort|Einsatzort|Projektort|Standort|PLZ\s*\/?\s*Ort)\s*:?$/i),
    start: labelled(/^(?:Start|Projektstart|Beginn|Geplanter Start)\s*:?$/i),
    duration: labelled(/^(?:Dauer|Laufzeit|Projektdauer)\s*:?$/i)
      || (labelled(/^(?:Voraussichtliches Ende|Projektende|Ende)\s*:?$/i).replace(/^(?=.)/, 'bis ')),
    remote: labelled(/^(?:Remote|Remoteanteil|Remote-Anteil|Homeoffice)\s*:?$/i),
    rate: labelled(/^(?:Stundensatz|Tagessatz|Honorar|Vergütung)\s*:?$/i),
    panelHtml,
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
    // The web view knows no status (WebKit): the page's title and first heading speak for
    // a rate limit, a block or a missing page.
    if page.status == 0
        && let Some(outcome) = hint_outcome(&page.status_hint)
    {
        return outcome;
    }
    let path = url.path().to_ascii_lowercase();
    if path.starts_with("/login") || (page.has_login_form && !page.has_logout) {
        return PageOutcome::LoginRequired(Cause::LoginPage);
    }
    // An unknown status, no sign-in form and no description: no sign-in is proven missing
    // (an error page has no sign-out link either) - suspicious, so the breaker applies,
    // never a sign-in window on a portal that may be limiting or blocking.
    if page.status == 0 && page.panel_html.is_none() {
        return PageOutcome::Suspicious(Cause::NoDescription);
    }
    if !page.has_logout {
        return PageOutcome::LoginRequired(Cause::NoLogoutLink);
    }
    // Expired project: freelance.de redirects to a project list - with a notice
    // (`/projekte?info_message=...`) or to the project's category. Measured only signed out;
    // "gone" therefore only counts in a confirmed session. A project page always has
    // "/projekt-<ID>" in its path.
    let own_page = is_project_page(&url, project_id);
    if !own_page && is_listing(&path) {
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
    if !own_page {
        return PageOutcome::Suspicious(Cause::WrongPage);
    }
    judge(Parsed {
        text,
        fields: page_fields(&page.title, &page.company, &page.location),
        facts: facts_of(page),
        ..Parsed::default()
    })
}

/// The page is the project `project_id`: its address names exactly that id in one of the
/// project forms - not merely somewhere ("projekt-12550670" or "?ref=1255067" for 1255067).
fn is_project_page(url: &Url, project_id: &str) -> bool {
    FreelanceDe
        .job_link(url)
        .is_some_and(|link| link.key.id == project_id)
}

/// Is the heading the description's own: exactly "Projektbeschreibung" (a colon is fine)?
/// A project title that contains the word (a technical writer for project descriptions)
/// is none. German page word, do not translate.
fn is_description_heading(text: &str) -> bool {
    text.trim()
        .trim_end_matches(':')
        .trim()
        .eq_ignore_ascii_case("projektbeschreibung")
}

/// The EXPERT notice that replaces what a non-member may not see. German page words, do
/// not translate.
fn is_expert_notice(text: &str) -> bool {
    text.to_lowercase()
        .contains("für expert-mitglieder sichtbar")
}

/// A value of the project head: a leaf with one of `labels` - its text, or the tooltip of an
/// icon - and the value in the next element or after the label.
fn labelled(header: ElementRef<'_>, labels: &[&str]) -> String {
    static ANY: Css = LazyLock::new(|| selector("*"));
    let text = |el: ElementRef<'_>| one_line(&el.text().collect::<String>());
    for el in header.select(&ANY) {
        if el.children().any(|c| c.value().is_element()) {
            continue;
        }
        let own = text(el);
        let tip = el
            .value()
            .attr("title")
            .or_else(|| el.value().attr("data-original-title"))
            .map(one_line)
            .unwrap_or_default();
        let label = if own.is_empty() { &tip } else { &own };
        let bare = label.trim_end_matches(':').trim().to_lowercase();
        if !labels.contains(&bare.as_str()) {
            continue;
        }
        let next = el.next_siblings().find_map(ElementRef::wrap).map(text);
        let value = next.filter(|v| !v.is_empty()).unwrap_or_else(|| {
            el.parent()
                .and_then(ElementRef::wrap)
                .and_then(|p| text(p).get(own.len()..).map(|v| v.trim().to_string()))
                .unwrap_or_default()
        });
        if !value.is_empty() {
            return value;
        }
    }
    String::new()
}

/// The findings of the probe script, read from the HTML of a guest page (same rules).
fn guest_findings(html: &str) -> SessionPage {
    static HEADINGS: Css = LazyLock::new(|| selector("h1, h2, h3"));
    static PANEL_BODY: Css = LazyLock::new(|| selector(".panel-body"));
    static HEADER: Css = LazyLock::new(|| selector(".panel-body.project-header"));
    static TITLE: Css = LazyLock::new(|| selector("h1"));
    static ANY: Css = LazyLock::new(|| selector("*"));
    let doc = Html::parse_document(html);
    let text = |el: ElementRef<'_>| one_line(&el.text().collect::<String>());
    let header = doc.select(&HEADER).next();
    let in_header =
        |el: ElementRef<'_>| header.is_some_and(|h| el.ancestors().any(|a| a.id() == h.id()));
    let heading = doc
        .select(&HEADINGS)
        .find(|h| !in_header(*h) && is_description_heading(&text(*h)));
    let panel = heading.map(|h| {
        let parent = h.parent().and_then(ElementRef::wrap).unwrap_or(h);
        let panel = h
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find(|a| a.value().classes().any(|c| c == "panel"))
            .unwrap_or(parent);
        panel.select(&PANEL_BODY).next().unwrap_or(panel)
    });
    let labelled = |labels: &[&str]| header.map(|h| labelled(h, labels)).unwrap_or_default();
    // The notice counts only where it stands for the description: in the description field,
    // or anywhere outside the labelled rows of the project head (there it is the placeholder
    // of company and location every non-member sees).
    let has_expert_marker = panel.is_some_and(|p| is_expert_notice(&text(p)))
        || doc.select(&ANY).any(|el| {
            !el.children().any(|c| c.value().is_element())
                && !in_header(el)
                && is_expert_notice(&text(el))
        });
    // German page labels, do not translate.
    let end = labelled(&["voraussichtliches ende", "projektende", "ende"]);
    let duration = Some(labelled(&["dauer", "laufzeit", "projektdauer"]))
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| {
            if end.is_empty() {
                String::new()
            } else {
                format!("bis {end}")
            }
        });
    SessionPage {
        ok: true,
        status: 200,
        has_expert_marker,
        has_captcha: super::has_challenge(&doc, html),
        title: header
            .and_then(|h| h.select(&TITLE).next())
            .map(text)
            .unwrap_or_default(),
        company: labelled(&[
            "firma",
            "unternehmen",
            "projektanbieter",
            "auftraggeber",
            "kunde",
        ]),
        location: labelled(&[
            "ort",
            "einsatzort",
            "projektort",
            "standort",
            "plz / ort",
            "plz/ort",
        ]),
        start: labelled(&["start", "projektstart", "beginn", "geplanter start"]),
        duration,
        remote: labelled(&["remote", "remoteanteil", "remote-anteil", "homeoffice"]),
        rate: labelled(&["stundensatz", "tagessatz", "honorar", "vergütung"]),
        panel_html: panel.map(|p| p.inner_html()),
        ..SessionPage::default()
    }
}

/// The probe's reading of a page without a known status (`statusHint`).
fn hint_outcome(hint: &str) -> Option<PageOutcome> {
    Some(match hint {
        "throttled" => PageOutcome::Throttled {
            cause: Cause::Http(429),
            retry_after: None,
        },
        "blocked" => PageOutcome::Blocked(Cause::Http(403)),
        "gone" => PageOutcome::Gone,
        _ => return None,
    })
}

/// Status codes that decide without looking at the content.
fn status_outcome(status: u16) -> Option<PageOutcome> {
    Some(match status {
        429 | 500..=599 => PageOutcome::Throttled {
            cause: Cause::Http(status),
            retry_after: None,
        },
        403 => PageOutcome::Blocked(Cause::Http(403)),
        404 | 410 => PageOutcome::Gone,
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

/// Longest line an end marker heads ("Ähnliche Projekte (12)" still is one; a sentence
/// that mentions "Kontaktdaten" is none).
const MAX_MARKER_LINE: usize = 40;

/// The description up to the first end section: a line that is an end marker, or starts
/// with one and is short enough for a heading - or a line that starts with the
/// registration call. A marker word inside a sentence (a task about contact data in the
/// CRM) never cuts.
fn cut_at_end_markers(text: &str) -> String {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let bare = line.trim();
        let heading = END_MARKERS.iter().any(|m| {
            bare == *m || (bare.starts_with(m) && bare.chars().count() <= MAX_MARKER_LINE)
        });
        if heading || bare.starts_with(REGISTER_CALL) {
            return text[..at].trim_end().to_string();
        }
        at += line.len();
    }
    text.trim_end().to_string()
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
            PageOutcome::Teaser {
                text,
                fields,
                facts,
            } => {
                assert_eq!(
                    text,
                    "Derzeit suchen wir für unseren Kunden einen Controller."
                );
                let fields = fields.unwrap();
                assert_eq!(fields.title, "Interim Controller (m/w/d)");
                assert_eq!(fields.location, "Hamburg");
                assert_eq!(fields.company, "");
                // The facts of the project head.
                assert_eq!(
                    facts,
                    Facts {
                        start: Some("01.11.2026".into()),
                        duration: Some("6 Monate".into()),
                        remote_percent: Some(100),
                        ..Facts::default()
                    }
                );
                assert_eq!(FreelanceDe.parse_facts(&teaser), facts);
            }
            other => panic!("{other:?}"),
        }
        // The fetch address itself (no redirect): the id is in the query.
        assert!(matches!(
            guest(&teaser, "/project/index.php"),
            PageOutcome::Teaser { .. }
        ));
        // Only the EXPERT notice in place of the description field, on a project page: an
        // empty teaser - still the right page.
        let expert = guest_html("").replace(
            r#"<div class="panel"><div class="panel-heading"><h2>Projektbeschreibung</h2></div><div class="panel-body"></div></div>"#,
            r#"<div class="panel"><div class="panel-body"><p>Details für EXPERT-Mitglieder sichtbar</p></div></div>"#,
        );
        assert!(matches!(
            guest(&expert, "/project/index.php"),
            PageOutcome::Teaser { text, .. } if text.is_empty()
        ));
        // A notice page with the notice and nothing else is no project page.
        let notice = "<html><body><p>Details für EXPERT-Mitglieder sichtbar</p></body></html>";
        assert_eq!(
            guest(notice, "/project/index.php"),
            PageOutcome::Suspicious(Cause::NoDescription)
        );
    }

    /// The EXPERT placeholder of company and location in the project head is on every
    /// guest page: it never turns a page whose description field is missing (a changed
    /// layout) into an empty teaser that resets the breaker.
    #[test]
    fn the_head_placeholder_hides_no_layout_change() {
        let renamed = guest_html(TEASER)
            .replace("<h2>Projektbeschreibung</h2>", "<h2>Beschreibung</h2>")
            .replace(
                "<li><span>Ort:</span> <span>Hamburg</span></li>",
                "<li><span>Firma:</span> <span>für EXPERT-Mitglieder sichtbar</span></li>",
            );
        assert_eq!(
            guest(&renamed, "/projekte/projekt-1255067-x"),
            PageOutcome::Suspicious(Cause::NoDescription)
        );
        let page = guest_findings(&renamed);
        assert!(!page.has_expert_marker);
    }

    /// A project title that contains "Projektbeschreibung" is not the description's heading:
    /// the head was stored as the full text.
    #[test]
    fn a_title_with_the_heading_word_is_no_description() {
        let long = format!("<p>{}</p>", "Aufgaben und Anforderungen. ".repeat(6));
        let html = guest_html(&long).replace(
            "<h1>Interim Controller (m/w/d)</h1>",
            "<h1>Technischer Redakteur für Projektbeschreibungen (m/w/d)</h1>",
        );
        match guest(&html, "/projekte/projekt-1255067-x") {
            PageOutcome::Text { text, .. } => {
                assert!(text.starts_with("Aufgaben und Anforderungen."), "{text}");
                assert!(!text.contains("Hamburg"), "{text}");
            }
            other => panic!("{other:?}"),
        }
        // Outside the head a colon after the heading is fine.
        let colon = guest_html(&long).replace(
            "<h2>Projektbeschreibung</h2>",
            "<h2>Projektbeschreibung:</h2>",
        );
        assert!(matches!(
            guest(&colon, "/projekte/projekt-1255067-x"),
            PageOutcome::Text { .. }
        ));
    }

    /// End markers cut only where they head a section: a sentence that mentions
    /// "Kontaktdaten" or a requirement about "Ähnliche Projekte" stays in the text.
    #[test]
    fn end_markers_inside_sentences_do_not_cut() {
        let prose = format!(
            "<p>Bereinigung der Adressen und Kontaktdaten im CRM. {}</p>",
            "Weitere Aufgaben im Projekt. ".repeat(14)
        );
        let bullet = "<ul><li>Mehrjährige Erfahrung als Interim Manager</li>\
                      <li>Ähnliche Projekte im Mittelstand erfolgreich umgesetzt</li>\
                      <li>Sicherer Umgang mit SAP</li></ul>"
            .to_string();
        for panel in [prose, bullet] {
            match judge_page(&page(URL, true, Some(&panel)), ID) {
                PageOutcome::Text { text, .. } => {
                    assert!(
                        text.contains("Kontaktdaten im CRM") || text.contains("Sicherer Umgang"),
                        "{text}"
                    );
                }
                other => panic!("{other:?}"),
            }
        }
        // As a heading they still end the description.
        assert_eq!(
            cut_at_end_markers("Aufgaben\n\nÄhnliche Projekte (12)\n\nAnderes"),
            "Aufgaben"
        );
        assert_eq!(
            cut_at_end_markers("Aufgaben\nKostenlos registrieren und alle Details sehen"),
            "Aufgaben"
        );
    }

    /// A guest's description with the registration call is a teaser at any length: only a
    /// sign-in brings the rest, and "ok" would never be fetched again.
    #[test]
    fn a_long_guest_teaser_is_a_teaser() {
        let panel = format!(
            "<p>{}</p><p>Kostenlos registrieren und alle Details sehen</p>",
            "Derzeit suchen wir für unseren Kunden einen Controller. ".repeat(11)
        );
        assert!(html_to_text(&panel).chars().count() > 600);
        match guest(&guest_html(&panel), "/projekte/projekt-1255067-x") {
            PageOutcome::Teaser { text, .. } => assert!(!text.contains("registrieren")),
            other => panic!("{other:?}"),
        }
    }

    /// The rate and more labels of the project head; a label may be an icon's tooltip.
    #[test]
    fn rate_and_more_head_labels() {
        let html = guest_html(TEASER).replace(
            "<ul><li><span>Ort:</span> <span>Hamburg</span></li><li><span>Start:</span> <span>01.11.2026</span></li>\n              <li><span>Dauer:</span> <span>6 Monate</span></li><li><span>Remote:</span> <span>100 %</span></li></ul>",
            r#"<ul><li><i class="fa fa-map-marker" title="Projektort"></i> <span>Köln</span></li>
               <li><span>Geplanter Start:</span> <span>01.12.2026</span></li>
               <li><span>Voraussichtliches Ende:</span> <span>31.05.2027</span></li>
               <li><span>Stundensatz:</span> <span>95 €/h</span></li></ul>"#,
        );
        let page = guest_findings(&html);
        assert_eq!(page.location, "Köln");
        assert_eq!(page.start, "01.12.2026");
        assert_eq!(page.duration, "bis 31.05.2027");
        let facts = FreelanceDe.parse_facts(&html);
        assert_eq!(facts.rate.as_deref(), Some("95 €/h"));
        assert_eq!(facts.start.as_deref(), Some("01.12.2026"));
    }

    /// Without a known status (`WebKit` has no `responseStatus`) the probe's hint from the
    /// title decides; without a hint, a page without a description and without a sign-in
    /// form is suspicious - never a sign-in window on a portal that may be limiting.
    #[test]
    fn an_unknown_status_reads_the_hint() {
        let unknown = |hint: &str, logout: bool| SessionPage {
            status: 0,
            status_hint: hint.into(),
            ..page(URL, logout, None)
        };
        assert!(matches!(
            judge_page(&unknown("throttled", false), ID),
            PageOutcome::Throttled { .. }
        ));
        assert!(matches!(
            judge_page(&unknown("blocked", true), ID),
            PageOutcome::Blocked(_)
        ));
        assert_eq!(judge_page(&unknown("gone", true), ID), PageOutcome::Gone);
        assert_eq!(
            judge_page(&unknown("", false), ID),
            PageOutcome::Suspicious(Cause::NoDescription)
        );
        // A sign-in form still asks for the sign-in.
        let mut form = unknown("", false);
        form.has_login_form = true;
        assert!(is_login(&judge_page(&form, ID)));
    }

    #[test]
    fn a_redirect_to_a_sign_in_is_a_wall() {
        for path in [
            "/login.php",
            "/anmelden",
            "/registrierung/",
            "/users/sign_in",
        ] {
            assert_eq!(
                FreelanceDe.redirect_outcome(path),
                PageOutcome::Blocked(Cause::LoginWall),
                "{path}"
            );
        }
        assert_eq!(
            FreelanceDe.redirect_outcome("/cdn-cgi/challenge-platform/x"),
            PageOutcome::Blocked(Cause::Captcha)
        );
        assert_eq!(
            FreelanceDe.redirect_outcome("/projekte/projekt-1255067-x"),
            PageOutcome::Suspicious(Cause::RedirectNotFollowed)
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

    /// Another project whose id merely contains the searched one is the wrong page.
    #[test]
    fn a_longer_id_is_another_project() {
        let long = format!("<p>{}</p>", "Aufgaben und Anforderungen. ".repeat(6));
        let other = "https://www.freelance.de/projekte/projekt-12550670-sap";
        assert_eq!(
            judge_page(&page(other, true, Some(&long)), ID),
            PageOutcome::Suspicious(Cause::WrongPage)
        );
        let listing = "https://www.freelance.de/projekte?ref=1255067";
        assert_eq!(
            judge_page(&page(listing, true, None), ID),
            PageOutcome::Gone
        );
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
            PageOutcome::Throttled {
                cause: Cause::Http(429),
                ..
            }
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
