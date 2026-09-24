//! freelancermap. Alerts come from freelancermap.de/.com; the project page carries its data
//! completely in a JSON island (`<script type="application/json"
//! data-component-name="ProjectShow">`) - more reliable than the visible markup and not cut
//! at "apply".

use std::sync::LazyLock;

use scraper::Html;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use super::{
    Access, Css, JobKey, JobLink, Portal, PortalAdapter, all_digits, hex12, host_and_segments,
    host_is, link, selector,
};
use crate::fetch::policy::Limits;
use crate::fetch::{Cause, PageFields, PageOutcome, Parsed, judge};
use crate::text::{html_to_text, one_line};

pub(super) struct Freelancermap;

const DOMAINS: [&str; 2] = ["freelancermap.de", "freelancermap.com"];

impl PortalAdapter for Freelancermap {
    fn portal(&self) -> Portal {
        Portal::Freelancermap
    }
    fn key(&self) -> &'static str {
        "freelancermap"
    }
    fn label(&self) -> &'static str {
        "freelancermap.de"
    }
    fn file_tag(&self) -> &'static str {
        "Freelancermap"
    }
    fn home_url(&self) -> &'static str {
        "https://www.freelancermap.de/"
    }
    fn sender_domains(&self) -> &'static [&'static str] {
        &DOMAINS
    }
    fn search_terms(&self) -> &'static [&'static str] {
        &["freelancermap"]
    }
    fn limits(&self) -> Limits {
        Limits {
            pace_ms: 3_000..=5_000,
            per_hour: 25,
            per_day: 60,
        }
    }
    /// Signed in, freelancermap shows the same text as to a guest (measured).
    fn access(&self) -> Access {
        Access::Guest
    }

    fn job_link(&self, url: &Url) -> Option<JobLink> {
        let (host, segments) = host_and_segments(url)?;
        let domain = DOMAINS.into_iter().find(|d| host_is(&host, d))?;
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        if let Some(id) = project_id(&segments) {
            return link(Portal::Freelancermap, id);
        }
        // /projekt/<slug> (/project/<slug> on .com) carries no id. The identity hangs on
        // domain and path - regardless of subdomain, case and tracking parameters.
        let [kind @ ("projekt" | "project"), slug] = segments.as_slice() else {
            return None;
        };
        if !is_slug(slug) {
            return None;
        }
        let path = format!("/{kind}/{slug}");
        let hash = Sha256::digest(format!("{domain}{path}").as_bytes());
        Some(JobLink {
            key: JobKey {
                portal: Portal::Freelancermap,
                id: format!("u{}", hex12(&hash)),
            },
            url: Url::parse(&format!("https://www.{domain}{path}")).ok()?,
        })
    }

    fn canonical_url(&self, id: &str) -> Option<Url> {
        if !all_digits(id, 1) {
            return None;
        }
        Url::parse(&format!("https://www.freelancermap.de/nproj/{id}.html")).ok()
    }

    /// Not followed (foreign host, third redirect): the target is judged anyway - a sign-in
    /// or check page is a block signal there too.
    fn redirect_outcome(&self, path: &str) -> PageOutcome {
        match landing(path) {
            PageOutcome::Blocked(cause) => PageOutcome::Blocked(cause),
            _ => PageOutcome::Suspicious(Cause::RedirectNotFollowed),
        }
    }

    fn guest_page(&self, html: &str, path: &str, link: &JobLink) -> PageOutcome {
        let expected = link.key.has_portal_id().then_some(link.key.id.as_str());
        match parse(html, expected) {
            // On a sign-in or check page, unrecognisable content is a block signal too.
            Err(_) if !is_project_path(path) => landing(path),
            Err(cause) => PageOutcome::Suspicious(cause),
            Ok(parsed) if parsed.text.is_none() && !is_project_path(path) => landing(path),
            Ok(parsed) => judge(parsed),
        }
    }
}

/// `/nproj/<ID>.html` or `/projektboerse/projekte/.../<ID>-slug.html`.
fn project_id(segments: &[&str]) -> Option<String> {
    match segments {
        ["nproj", file] => {
            let digits = file.strip_suffix(".html").unwrap_or(file);
            all_digits(digits, 5).then(|| digits.to_string())
        }
        ["projektboerse", "projekte" | "projekt", .., last] => {
            let digits = last.split('-').next()?;
            (last.contains('-') && all_digits(digits, 5)).then(|| digits.to_string())
        }
        _ => None,
    }
}

fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 300
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_project_path(path: &str) -> bool {
    ["/projekt/", "/project/", "/nproj/"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

/// freelancermap redirected to a page without a project text. Only 404/410 decide "no
/// longer exists" (where a deleted project leads was never measured): a sign-in or check
/// page is a block signal, everything else stays suspicious - the job is tried again later,
/// and the breaker applies.
fn landing(path: &str) -> PageOutcome {
    // Whole path segments, without extension and with "_" as "-": "/users/sign_in",
    // "/login.php". German path words, do not translate.
    let wall = path.split('/').any(|segment| {
        let stem = segment
            .split('.')
            .next()
            .unwrap_or_default()
            .replace('_', "-");
        matches!(
            stem.as_str(),
            "login"
                | "log-in"
                | "signin"
                | "sign-in"
                | "anmelden"
                | "anmeldung"
                | "register"
                | "registrieren"
                | "authwall"
                | "checkpoint"
                | "captcha"
                | "challenge"
                | "cdn-cgi"
        )
    });
    if wall {
        PageOutcome::Blocked(Cause::LoginWall)
    } else {
        PageOutcome::Suspicious(Cause::NotAProjectPage)
    }
}

static ISLAND: Css = LazyLock::new(|| {
    selector(r#"script[type="application/json"][data-component-name="ProjectShow"]"#)
});
/// Fallback when the island is missing: the visible description field.
static BODY: Css = LazyLock::new(|| selector("div.project-body-description, div.ql-editor"));

#[derive(Deserialize)]
struct Island {
    project: Project,
}

/// Only the fields needed - names of the contact person are never read.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: Option<u64>,
    title: Option<String>,
    company: Option<String>,
    city: Option<String>,
    #[serde(default)]
    locations: Vec<Location>,
    description: Option<String>,
    #[serde(default)]
    is_archived: bool,
    #[serde(default = "yes")]
    active: bool,
    #[serde(default)]
    disabled: bool,
    contract_type: Option<ContractType>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    localized_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractType {
    remote_in_percent: Option<u32>,
}

fn yes() -> bool {
    true
}

/// `expected_id`: the project id from the mail (unknown for slug links). Another id on the
/// page means: the wrong page - never store its text.
pub(crate) fn parse(html: &str, expected_id: Option<&str>) -> Result<Parsed, Cause> {
    let doc = Html::parse_document(html);
    let island = doc
        .select(&ISLAND)
        .next()
        .and_then(|e| serde_json::from_str::<Island>(&e.text().collect::<String>()).ok());
    let Some(Island { project }) = island else {
        // Without the JSON island the page can only be checked by its raw text: if the
        // expected id is nowhere, it is not this ad - better no text than a foreign one.
        if expected_id.is_some_and(|id| !html.contains(id)) {
            return Err(Cause::PageNotRecognised);
        }
        return Ok(Parsed {
            text: doc
                .select(&BODY)
                .next()
                .map(|e| html_to_text(&e.inner_html())),
            ..Parsed::default()
        });
    };
    if let (Some(expected), Some(found)) = (expected_id, project.id)
        && expected != found.to_string()
    {
        return Err(Cause::WrongPage);
    }
    let places: Vec<String> = project
        .locations
        .iter()
        .filter_map(|l| l.localized_name.as_deref().map(one_line))
        .filter(|l| !l.is_empty())
        .collect();
    let remote = project
        .contract_type
        .and_then(|c| c.remote_in_percent)
        .is_some_and(|p| p >= 100);
    let location = match project
        .city
        .as_deref()
        .map(one_line)
        .filter(|c| !c.is_empty())
    {
        Some(city) => city,
        None if !places.is_empty() => places.join(", "),
        None if remote => "Remote".into(),
        None => String::new(),
    };
    Ok(Parsed {
        text: project.description.as_deref().map(html_to_text),
        closed: project.is_archived || !project.active || project.disabled,
        fields: PageFields {
            title: project.title.as_deref().map(one_line).unwrap_or_default(),
            company: project.company.as_deref().map(one_line).unwrap_or_default(),
            location,
        },
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn page(id: u64, description: &str, archived: bool) -> String {
        let island = serde_json::json!({
            "isHidden": false,
            "project": {
                "firstName": "Erika", "lastName": "Muster",
                "id": id, "title": "SAP FI/CO Berater (m/w/d)", "company": "Ferrum Systems SE",
                "city": null, "locations": [{"localizedName": "München"}, {"localizedName": "Remote"}],
                "description": description, "isArchived": archived, "active": true, "disabled": false,
                "contractType": {"contractType": "contracting", "remoteInPercent": 50},
                "startText": "ab sofort", "durationText": "6 Monate", "hourlyRate": "95 €/h",
                "skills": [{"name": "SAP FI"}, {"name": "SAP CO"}]
            }
        });
        format!(
            r#"<html><body><div class="project-body-description"><div class="ql-editor">sichtbar</div></div>
            <script type="application/json" class="js-react-on-rails-component" data-component-name="ProjectShow">{island}</script></body></html>"#
        )
    }

    #[test]
    fn island_gives_text_and_fields() {
        let p = parse(
            &page(
                2_971_857,
                "<div class=\"ql-editor\"><p>Aufgaben:</p><ul><li>SAP</li></ul></div>",
                false,
            ),
            Some("2971857"),
        )
        .unwrap();
        assert_eq!(p.text.as_deref(), Some("Aufgaben:\n\nSAP"));
        assert!(!p.closed);
        assert_eq!(
            p.fields,
            PageFields {
                title: "SAP FI/CO Berater (m/w/d)".into(),
                company: "Ferrum Systems SE".into(),
                location: "München, Remote".into(),
            }
        );
    }

    #[test]
    fn wrong_project_is_rejected() {
        assert_eq!(
            parse(&page(1, "x", false), Some("2971857")).err(),
            Some(Cause::WrongPage)
        );
        // Slug link without an id from the mail: no check possible.
        assert!(parse(&page(1, "x", false), None).is_ok());
    }

    #[test]
    fn archived_is_closed() {
        assert!(parse(&page(5, "x", true), Some("5")).unwrap().closed);
    }

    #[test]
    fn without_island_the_visible_body_is_used() {
        // Without the JSON island the raw text counts: with the expected id in it, the
        // visible text counts.
        let p = parse(
            r#"<div data-id="5" class="project-body-description"><p>Nur sichtbar</p></div>"#,
            Some("5"),
        )
        .unwrap();
        assert_eq!(p.text.as_deref(), Some("Nur sichtbar"));
        // Without the island and without the id: better no text than a foreign page's.
        assert!(
            parse(
                r#"<div class="project-body-description"><p>Fremd</p></div>"#,
                Some("5")
            )
            .is_err()
        );
        // Slug link without a known id: the visible text stays the only source.
        assert_eq!(parse("<p>Suche</p>", None).unwrap().text, None);
    }

    #[test]
    fn landing_pages() {
        for wall in ["/users/sign_in", "/login.php", "/de/anmelden/"] {
            assert!(
                matches!(landing(wall), PageOutcome::Blocked(Cause::LoginWall)),
                "{wall}"
            );
        }
        assert!(matches!(
            landing("/projektboerse"),
            PageOutcome::Suspicious(Cause::NotAProjectPage)
        ));
    }

    #[test]
    fn real_page_when_available() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/private/pages/freelancermap-3049771.html");
        let Ok(html) = std::fs::read_to_string(path) else {
            eprintln!("skipped: private freelancermap page missing");
            return;
        };
        // No real names in the repository: the test checks that fields are filled at all.
        let p = parse(&html, Some("3049771")).unwrap();
        assert!(p.text.unwrap().chars().count() > 1_000);
        assert!(!p.fields.company.is_empty());
        assert!(!p.fields.location.is_empty());
        assert!(!p.closed);
    }
}
