//! LinkedIn. Alerts come from linkedin.com; the app reads the public guest view
//! (`/jobs-guest/jobs/api/jobPosting/<ID>`): an HTML fragment with the **full** text of the
//! ad - "show more" is pure CSS there - and the head of the ad. Character-identical to the
//! public page (measured). The mail link itself leads a guest to the sign-in page.

use std::sync::LazyLock;

use regex::Regex;
use scraper::Html;
use url::Url;

use super::{
    Access, Css, Facts, JobLink, Portal, PortalAdapter, Redirects, all_digits, host_and_segments,
    host_is, link, selector,
};
use crate::fetch::policy::Limits;
use crate::fetch::{Cause, PageFields, PageOutcome, Parsed, judge};
use crate::text::{html_to_text, one_line};

pub(super) struct LinkedIn;

impl PortalAdapter for LinkedIn {
    fn portal(&self) -> Portal {
        Portal::LinkedIn
    }
    fn key(&self) -> &'static str {
        "linkedin"
    }
    fn label(&self) -> &'static str {
        "linkedin.com"
    }
    fn file_tag(&self) -> &'static str {
        "LinkedIn"
    }
    fn home_url(&self) -> &'static str {
        "https://www.linkedin.com/jobs/"
    }
    fn sender_domains(&self) -> &'static [&'static str] {
        &["linkedin.com"]
    }
    fn search_terms(&self) -> &'static [&'static str] {
        &["linkedin"]
    }
    fn limits(&self) -> Limits {
        Limits {
            pace_ms: 4_000..=7_000,
            per_hour: 20,
            per_day: 40,
        }
    }
    /// Signed in, LinkedIn shows the same text as to a guest (measured) - a session window
    /// would bring nothing.
    fn access(&self) -> Access {
        Access::Guest
    }

    /// `/jobs/view/<ID>` or `/comm/jobs/view/<ID>`; the segment is the id or ends with
    /// `-<ID>` (`sap-berater-4456653430`). The recommendation and "similar jobs" mails link
    /// a job as a list with it selected: `/jobs/search/?currentJobId=<ID>`,
    /// `/comm/jobs/collections/recommended/?currentJobId=<ID>` - the same job.
    fn job_link(&self, url: &Url) -> Option<JobLink> {
        let (host, segments) = host_and_segments(url)?;
        if !host_is(&host, "linkedin.com") {
            return None;
        }
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        match segments.as_slice() {
            ["jobs", "view", rest @ ..] | ["comm", "jobs", "view", rest @ ..] => {
                let digits = rest.first()?.rsplit('-').next()?;
                if !all_digits(digits, 6) {
                    return None;
                }
                link(Portal::LinkedIn, digits.to_string())
            }
            ["jobs", ..] | ["comm", "jobs", ..] => {
                let (_, id) = url
                    .query_pairs()
                    .find(|(key, _)| key.eq_ignore_ascii_case("currentjobid"))?;
                all_digits(&id, 6).then(|| link(Portal::LinkedIn, id.into_owned()))?
            }
            _ => None,
        }
    }

    fn canonical_url(&self, id: &str) -> Option<Url> {
        all_digits(id, 1)
            .then(|| Url::parse(&format!("https://www.linkedin.com/jobs/view/{id}/")).ok())?
    }

    /// The public guest section (the mail link leads a guest to the sign-in page).
    fn fetch_url(&self, link: &JobLink) -> Url {
        Url::parse(&format!(
            "https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/{}",
            link.key.id
        ))
        .unwrap_or_else(|_| link.url.clone())
    }

    fn redirects(&self) -> Redirects {
        Redirects::Never
    }

    /// Redirects of the guest view almost always lead to the sign-in wall - a block signal.
    fn redirect_outcome(&self, path: &str) -> PageOutcome {
        // Whole path segments: "/loginhilfe" is no sign-in page.
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let wall = matches!(
            segments.as_slice(),
            ["authwall" | "login" | "checkpoint" | "signup", ..] | ["uas", "login", ..]
        );
        if wall {
            PageOutcome::Blocked(Cause::LoginWall)
        } else {
            PageOutcome::Suspicious(Cause::UnexpectedRedirect)
        }
    }

    /// The guest view answered 200. Without the description container the page may be a
    /// sign-in wall or a security check served with 200 (not only as a redirect): that stops
    /// the portal at once, it is no changed layout. Real ads always have the container, and
    /// their sign-in modal is never read as a wall.
    fn guest_page(&self, html: &str, _path: &str, _link: &JobLink) -> PageOutcome {
        let parsed = parse(html);
        if parsed.text.is_none()
            && let Some(cause) = wall(html)
        {
            return PageOutcome::Blocked(cause);
        }
        judge(parsed)
    }

    fn parser_version(&self) -> u32 {
        PARSER_VERSION
    }

    fn parse_facts(&self, html: &str) -> Facts {
        facts(&Html::parse_document(html))
    }
}

/// Bump whenever the parser reads pages differently (requeues failed jobs).
/// 2: all four criteria, `<br><br>` paragraphs, walls and checks served with 200.
const PARSER_VERSION: u32 = 2;

static CRITERIA: Css = LazyLock::new(|| selector("li.description__job-criteria-item"));
static CRITERION: Css = LazyLock::new(|| selector("h3"));
static CRITERION_VALUE: Css = LazyLock::new(|| selector("span"));

/// The criteria list under the ad: career level, employment type, job function and
/// industries.
fn facts(doc: &Html) -> Facts {
    let mut facts = Facts::default();
    let text = |e: scraper::ElementRef<'_>| one_line(&e.text().collect::<String>());
    for item in doc.select(&CRITERIA) {
        let label = item.select(&CRITERION).next().map(text).unwrap_or_default();
        let Some(value) = item.select(&CRITERION_VALUE).next().map(text) else {
            continue;
        };
        // German and English page labels, do not translate.
        match label.to_lowercase().as_str() {
            "beschäftigungsverhältnis" | "employment type" => {
                facts.employment_type = Facts::value(&value);
            }
            "karrierestufe" | "seniority level" => facts.level = Facts::value(&value),
            "tätigkeitsbereich" | "job function" => facts.function = Facts::value(&value),
            "branchen" | "industries" => facts.industries = Facts::value(&value),
            _ => {}
        }
    }
    facts
}

static PAGE_TITLE: Css = LazyLock::new(|| selector("title"));

/// A sign-in wall or a security check served with 200 instead of the ad: its page title,
/// the sign-in wall's or the check's address in the page, a captcha. `None` for anything
/// else (a page LinkedIn changed is "suspicious", not a wall). German and English page
/// titles, do not translate.
fn wall(html: &str) -> Option<Cause> {
    let doc = Html::parse_document(html);
    let title = doc
        .select(&PAGE_TITLE)
        .next()
        .map(|t| one_line(&t.text().collect::<String>()).to_lowercase())
        .unwrap_or_default();
    let check = [
        "security verification",
        "sicherheitsüberprüfung",
        "security check",
    ];
    if super::has_challenge(&doc, html)
        || html.contains("/checkpoint/challenge")
        || check.iter().any(|t| title.contains(t))
    {
        return Some(Cause::Captcha);
    }
    let sign_in = [
        "sign in",
        "sign up",
        "log in",
        "join linkedin",
        "anmelden",
        "einloggen",
        "registrieren",
        "mitglied werden",
    ];
    (sign_in.iter().any(|t| title.contains(t))
        || html.contains("/authwall")
        || html.contains("authwall?"))
    .then_some(Cause::LoginWall)
}

static MARKUP: Css = LazyLock::new(|| selector("div.show-more-less-html__markup"));
static CLOSED: Css = LazyLock::new(|| selector("figure.closed-job"));
static TITLE: Css = LazyLock::new(|| selector("h2.topcard__title"));
static COMPANY: Css = LazyLock::new(|| selector("a.topcard__org-name-link"));
/// Location; the applicant count carries the same class, but also `--metadata`.
static PLACE: Css = LazyLock::new(|| {
    selector("span.topcard__flavor.topcard__flavor--bullet:not(.topcard__flavor--metadata)")
});

pub(crate) fn parse(html: &str) -> Parsed {
    let doc = Html::parse_document(html);
    let text_of = |sel: &Css| {
        doc.select(sel)
            .next()
            .map(|e| one_line(&e.text().collect::<String>()))
            .unwrap_or_default()
    };
    let closed = doc.select(&CLOSED).next().is_some();
    Parsed {
        text: doc
            .select(&MARKUP)
            .next()
            .map(|e| html_to_text(&e.inner_html())),
        closed,
        fields: PageFields {
            // Closed ads get "(No longer accepting ...)" appended to the title - the suffix
            // goes; if it is unknown, the title from the mail stays.
            title: if closed {
                without_closed_suffix(&text_of(&TITLE))
            } else {
                text_of(&TITLE)
            },
            company: text_of(&COMPANY),
            location: text_of(&PLACE),
        },
        facts: facts(&doc),
    }
}

/// "Title (No longer accepting applications ...)" -> "Title"; an unknown suffix -> empty.
fn without_closed_suffix(title: &str) -> String {
    // German page wording, do not translate.
    static SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\s*\([^()]*(?:no longer accepting|nicht mehr angenommen)[^()]*\)\s*$")
            .expect("fixed regex")
    });
    SUFFIX
        .find(title)
        .map(|m| title[..m.start()].trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The guest view with LinkedIn's exact element and class skeleton (anonymised: invented
    /// text, company and place), open, closed and legitimately short.
    pub(crate) const OPEN: &str =
        include_str!("../../tests/fixtures/pages/linkedin_guest_open.html");
    const CLOSED: &str = include_str!("../../tests/fixtures/pages/linkedin_guest_closed.html");
    const SHORT: &str = include_str!("../../tests/fixtures/pages/linkedin_guest_short.html");

    /// The checked-in page with `text` as the ad's markup.
    pub(crate) fn page(text: &str, closed: bool) -> String {
        let html = if closed { CLOSED } else { OPEN };
        let open = "relative overflow-hidden\">";
        let start = html.find(open).expect("markup container") + open.len();
        let end = start + html[start..].find("</div>").expect("end of the markup");
        format!("{}{text}{}", &html[..start], &html[end..])
    }

    #[test]
    fn open_posting() {
        let p = parse(OPEN);
        assert!(!p.closed);
        assert_eq!(
            p.fields,
            PageFields {
                title: "Interim Leiter Controlling (m/w/d)".into(),
                company: "Musterwerke GmbH".into(),
                location: "Köln, Nordrhein-Westfalen, Deutschland".into(),
            }
        );
        // All four criteria.
        assert_eq!(
            p.facts,
            Facts {
                employment_type: Some("Befristet".into()),
                level: Some("Direktor".into()),
                function: Some("Finanzen und Rechnungswesen".into()),
                industries: Some("Maschinenbau und Metallverarbeitung".into()),
                ..Facts::default()
            }
        );
        assert_eq!(LinkedIn.parse_facts(OPEN), p.facts);
        // The `<br><br>` paragraphs stay paragraphs, headings stand alone.
        let text = p.text.unwrap();
        assert!(
            text.starts_with(
                "Referenz 16-000001\n\nInterim-Mandat im Mittelstand!\n\nFür unseren Kunden"
            ),
            "{text}"
        );
        assert!(
            text.contains("\n\nIhre Aufgaben\n\nLeitung des Controllings"),
            "{text}"
        );
        assert!(text.contains("\n\nIhr Profil\n\n"), "{text}");
        assert!(!text.contains("Mehr anzeigen"), "the buttons are no text");
        assert!(!text.contains("Passwort"), "the sign-in modal is no text");
        assert!(matches!(
            LinkedIn.guest_page(OPEN, "/", &link()),
            PageOutcome::Text {
                short: false,
                closed: false,
                ..
            }
        ));
    }

    fn link() -> JobLink {
        crate::portal::job_link("https://www.linkedin.com/jobs/view/4199000201/").unwrap()
    }

    #[test]
    fn closed_posting_keeps_text_and_the_title_without_the_suffix() {
        let p = parse(CLOSED);
        assert!(p.closed);
        assert!(p.text.unwrap().contains("Ihre Aufgaben"));
        assert_eq!(p.fields.title, "Interim Leiter Controlling (m/w/d)");
        assert_eq!(p.fields.company, "Musterwerke GmbH");
        let german = CLOSED.replace(
            "(No longer accepting applications as of 11/26)",
            "(Bewerbungen werden nicht mehr angenommen)",
        );
        assert_eq!(
            parse(&german).fields.title,
            "Interim Leiter Controlling (m/w/d)"
        );
        let unknown = CLOSED.replace(
            "(No longer accepting applications as of 11/26)",
            "(geschlossen)",
        );
        assert_eq!(
            parse(&unknown).fields.title,
            "",
            "unknown suffix: the title from the mail"
        );
    }

    #[test]
    fn a_short_ad_is_verified_short() {
        assert!(matches!(
            LinkedIn.guest_page(SHORT, "/", &link()),
            PageOutcome::Text { short: true, .. }
        ));
        // The helper keeps the skeleton.
        assert_eq!(parse(&page("Kurz.", false)).text.as_deref(), Some("Kurz."));
    }

    #[test]
    fn missing_container_is_no_text() {
        assert_eq!(parse("<html><body>Bitte anmelden</body></html>").text, None);
    }

    /// A sign-in wall or a security check served with 200 stops the portal at once (it
    /// used to count as a changed layout: one more request into the wall, a one-hour pause
    /// and a wrong message). A page without the container and without such signs stays
    /// "suspicious"; a real ad with its sign-in modal is never a wall.
    #[test]
    fn walls_and_checks_served_with_200() {
        let judge = |html: &str| LinkedIn.guest_page(html, "/", &link());
        for wall in [
            "<html><head><title>LinkedIn Login, Sign in | LinkedIn</title></head><body><form action=\"/uas/login-submit\"></form></body></html>",
            "<html><head><title>Sign Up | LinkedIn</title></head><body></body></html>",
            "<html><head><title>LinkedIn: Anmelden oder mitmachen</title></head><body></body></html>",
            "<html><body><script>window.location.href = \"https://www.linkedin.com/authwall?trk=gf&sessionRedirect=x\";</script></body></html>",
        ] {
            assert_eq!(
                judge(wall),
                PageOutcome::Blocked(Cause::LoginWall),
                "{wall}"
            );
        }
        for check in [
            "<html><head><title>Security Verification | LinkedIn</title></head><body></body></html>",
            "<html><body><form id=\"captcha-internal\" action=\"/checkpoint/challenge/verify\"></form></body></html>",
            "<html><body><div class=\"g-recaptcha\" data-sitekey=\"x\"></div></body></html>",
            "<html><body><script>window._cf_chl_opt = {};</script></body></html>",
        ] {
            assert_eq!(
                judge(check),
                PageOutcome::Blocked(Cause::Captcha),
                "{check}"
            );
        }
        assert_eq!(
            judge("<html>Bitte anmelden</html>"),
            PageOutcome::Suspicious(Cause::NoDescription)
        );
        assert_eq!(judge(""), PageOutcome::Suspicious(Cause::NoDescription));
        for real in [OPEN, CLOSED, SHORT] {
            assert!(
                matches!(judge(real), PageOutcome::Text { .. }),
                "a real page with its sign-in modal"
            );
        }
    }

    #[test]
    fn redirects_of_the_guest_view() {
        for wall in [
            "/authwall",
            "/uas/login",
            "/checkpoint/challenge/agf",
            "/login",
            "/signup/cold-join",
        ] {
            assert!(
                matches!(
                    LinkedIn.redirect_outcome(wall),
                    PageOutcome::Blocked(Cause::LoginWall)
                ),
                "{wall}"
            );
        }
        for other in ["/jobs/view/4123456789/", "/loginhilfe"] {
            assert!(
                matches!(
                    LinkedIn.redirect_outcome(other),
                    PageOutcome::Suspicious(Cause::UnexpectedRedirect)
                ),
                "{other}"
            );
        }
    }

    /// Real pages fetched by hand (private, not checked in). Ignored by default so a
    /// missing folder shows as "ignored" instead of passing silently; run it with
    /// `cargo test -- --ignored` in the main checkout.
    #[test]
    #[ignore = "needs the private pages in core/tests/fixtures/private/pages"]
    fn real_pages_when_available() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private/pages");
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).expect(name);
        let (open, short, closed) = (
            read("linkedin-open-4468654483.html"),
            read("linkedin-short-4445179167.html"),
            read("linkedin-closed-4091550784.html"),
        );
        // No real names in the repository: the test checks that the fields are filled.
        for html in [&open, &short, &closed] {
            assert!(wall(html).is_none(), "a real ad is no wall");
        }
        let open = parse(&open);
        let text = open.text.as_ref().unwrap();
        assert!(text.chars().count() > 3_000);
        assert!(text.matches("\n\n").count() > 10, "paragraphs kept");
        assert!(!open.closed);
        assert!(!open.fields.company.is_empty());
        assert!(!open.fields.location.is_empty());
        assert!(!open.fields.title.is_empty());
        assert!(open.facts.function.is_some() && open.facts.industries.is_some());
        let short = parse(&short);
        let n = short.text.as_ref().unwrap().chars().count();
        assert!(
            (1..100).contains(&n),
            "legitimately short ad: {n} characters"
        );
        let closed = parse(&closed);
        assert!(closed.closed && closed.text.unwrap().chars().count() > 3_000);
        assert!(!closed.fields.title.contains(" - "), "title without suffix");
        assert!(!closed.fields.title.contains('('));
    }
}
