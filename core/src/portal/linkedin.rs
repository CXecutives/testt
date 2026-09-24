//! LinkedIn. Alerts come from linkedin.com; the app reads the public guest view
//! (`/jobs-guest/jobs/api/jobPosting/<ID>`): an HTML fragment with the **full** text of the
//! ad - "show more" is pure CSS there - and the head of the ad. Character-identical to the
//! public page (measured). The mail link itself leads a guest to the sign-in page.

use std::sync::LazyLock;

use regex::Regex;
use scraper::Html;
use url::Url;

use super::{
    Access, Css, JobLink, Portal, PortalAdapter, Redirects, all_digits, host_and_segments, host_is,
    link, selector,
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
        "LinkedIn"
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
    /// `-<ID>` (`sap-berater-4456653430`).
    fn job_link(&self, url: &Url) -> Option<JobLink> {
        let (host, segments) = host_and_segments(url)?;
        if !host_is(&host, "linkedin.com") {
            return None;
        }
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        let (["jobs", "view", rest @ ..] | ["comm", "jobs", "view", rest @ ..]) =
            segments.as_slice()
        else {
            return None;
        };
        let digits = rest.first()?.rsplit('-').next()?;
        if !all_digits(digits, 6) {
            return None;
        }
        link(Portal::LinkedIn, digits.to_string())
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

    fn guest_page(&self, html: &str, _path: &str, _link: &JobLink) -> PageOutcome {
        judge(parse(html))
    }
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

    /// Layout like the real guest view (shortened, invented content).
    pub(crate) fn page(text: &str, closed: bool) -> String {
        let closed = if closed {
            r#"<figure class="closed-job closed-job__flavor topcard__flavor-row"><figcaption class="closed-job__flavor--closed">Es werden keine Bewerbungen mehr angenommen.</figcaption></figure>"#
        } else {
            ""
        };
        format!(
            r#"<section class="top-card-layout"><a href="/jobs/view/1"><h2 class="top-card-layout__title topcard__title">Interim CFO (m/w/d){}</h2></a>
            <h4><div class="topcard__flavor-row"><span class="topcard__flavor"><a class="topcard__org-name-link topcard__flavor--black-link" href="https://de.linkedin.com/company/x">
              Nordlicht AG
            </a></span><span class="topcard__flavor topcard__flavor--bullet">
              Hamburg, Deutschland
            </span></div><div class="topcard__flavor-row"><span class="posted-time-ago__text topcard__flavor--metadata">vor 2 Tagen</span>
            <span class="num-applicants__caption topcard__flavor--metadata topcard__flavor--bullet">Über 200 Bewerber</span></div>{closed}</h4></section>
            <section class="description"><div class="show-more-less-html__markup show-more-less-html__markup--clamp-after-5
                relative overflow-hidden">{text}</div><button class="show-more-less-html__button">Mehr anzeigen</button></section>
            <ul class="description__job-criteria-list"><li class="description__job-criteria-item"><h3 class="description__job-criteria-subheader">Karrierestufe</h3><span class="description__job-criteria-text description__job-criteria-text--criteria">
              Direktor
            </span></li><li class="description__job-criteria-item"><h3 class="description__job-criteria-subheader">Beschäftigungsverhältnis</h3><span class="description__job-criteria-text description__job-criteria-text--criteria">
              Vollzeit
            </span></li></ul>"#,
            if closed.is_empty() {
                ""
            } else {
                " (No longer accepting applications)"
            }
        )
    }

    #[test]
    fn open_posting() {
        let p = parse(&page(
            "<p><strong>Aufgaben</strong></p><ul><li>Finanzen</li><li>Controlling</li></ul>",
            false,
        ));
        assert_eq!(p.text.as_deref(), Some("Aufgaben\n\nFinanzen\nControlling"));
        assert!(!p.closed);
        assert_eq!(
            p.fields,
            PageFields {
                title: "Interim CFO (m/w/d)".into(),
                company: "Nordlicht AG".into(),
                location: "Hamburg, Deutschland".into(),
            }
        );
    }

    #[test]
    fn closed_posting_keeps_text_and_the_title_without_the_suffix() {
        let p = parse(&page("Text", true));
        assert!(p.closed);
        assert_eq!(p.text.as_deref(), Some("Text"));
        assert_eq!(p.fields.title, "Interim CFO (m/w/d)");
        assert_eq!(p.fields.company, "Nordlicht AG");
        let german = page("Text", true).replace(
            "(No longer accepting applications)",
            "(Bewerbungen werden nicht mehr angenommen)",
        );
        assert_eq!(parse(&german).fields.title, "Interim CFO (m/w/d)");
        let unknown =
            page("Text", true).replace("(No longer accepting applications)", "(geschlossen)");
        assert_eq!(
            parse(&unknown).fields.title,
            "",
            "unknown suffix: the title from the mail"
        );
    }

    #[test]
    fn missing_container_is_no_text() {
        assert_eq!(parse("<html><body>Bitte anmelden</body></html>").text, None);
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

    /// Real pages fetched by hand (private, not checked in): skipped when missing.
    #[test]
    fn real_pages_when_available() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private/pages");
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
        let (Some(open), Some(short), Some(closed)) = (
            read("linkedin-open-4468654483.html"),
            read("linkedin-short-4445179167.html"),
            read("linkedin-closed-4091550784.html"),
        ) else {
            eprintln!("skipped: private LinkedIn pages missing");
            return;
        };
        // No real names in the repository: the test checks that the fields are filled.
        let open = parse(&open);
        assert!(open.text.as_ref().unwrap().chars().count() > 3_000);
        assert!(!open.closed);
        assert!(!open.fields.company.is_empty());
        assert!(!open.fields.location.is_empty());
        assert!(!open.fields.title.is_empty());
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
