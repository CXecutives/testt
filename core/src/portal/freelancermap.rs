//! freelancermap. Alerts come from freelancermap.de/.com; the project page carries its data
//! completely in a JSON island (`<script type="application/json"
//! data-component-name="ProjectShow">`) - more reliable than the visible markup and not cut
//! at "apply".

use std::sync::LazyLock;

use scraper::Html;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use super::{
    Access, Css, Facts, JobKey, JobLink, Portal, PortalAdapter, all_digits, hex12,
    host_and_segments, host_is, link, selector,
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
        // `/projekt/<slug>-<ID>`: the id at the end of the slug - the same project as
        // `/nproj/<ID>.html`, so the same key.
        if let Some(id) = slug_end_id(slug) {
            return link(Portal::Freelancermap, id);
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
        let parsed = parse(html, expected);
        // No project on the page: a security check served with 200 stops the portal at
        // once (only real check elements count - the island's JSON names a site key on
        // every real page).
        if !matches!(&parsed, Ok(p) if p.text.is_some())
            && super::has_challenge(&Html::parse_document(html), html)
        {
            return PageOutcome::Blocked(Cause::Captcha);
        }
        match parsed {
            // On a sign-in or check page, unrecognisable content is a block signal too.
            Err(_) if !is_project_path(path) => landing(path),
            Err(cause) => PageOutcome::Suspicious(cause),
            Ok(parsed) if parsed.text.is_none() && !is_project_path(path) => landing(path),
            Ok(parsed) => judge(parsed),
        }
    }

    fn parser_version(&self) -> u32 {
        PARSER_VERSION
    }

    fn parse_facts(&self, html: &str) -> Facts {
        parse(html, None).map(|p| p.facts).unwrap_or_default()
    }
}

/// Bump whenever the parser reads pages differently (requeues failed jobs).
/// 2: start, duration, skills, contract type and country as the real island states them;
/// a check page served with 200.
const PARSER_VERSION: u32 = 2;

/// `/nproj/<ID>[.html]` or `/projektboerse/projekte/<category>.../<ID>[-slug][.html]`
/// (also `<slug>-<ID>`) - the forms of the old engine (`legacy-python`, `alerts.py`).
fn project_id(segments: &[&str]) -> Option<String> {
    match segments {
        ["nproj", file] => {
            let digits = file.strip_suffix(".html").unwrap_or(file);
            all_digits(digits, 5).then(|| digits.to_string())
        }
        ["projektboerse", "projekte" | "projekt", .., last] => {
            let stem = last.strip_suffix(".html").unwrap_or(last);
            let first = stem.split('-').next()?;
            if all_digits(first, 5) {
                Some(first.to_string())
            } else {
                slug_end_id(stem)
            }
        }
        _ => None,
    }
}

/// The id at the end of a slug (`sap-fi-co-berater-2971857`). At least six digits: a
/// five-digit postal code at the end of a slug (`...-muenchen-80331`) is no id.
fn slug_end_id(slug: &str) -> Option<String> {
    let (_, digits) = slug.rsplit_once('-')?;
    all_digits(digits, 6).then(|| digits.to_string())
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
    /// The page's own words for its codes (`contract_types_contracting`: "Freiberuflich").
    #[serde(default)]
    translations: serde_json::Map<String, Value>,
}

/// Only the fields needed - names of the contact person are never read.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: Option<u64>,
    title: Option<String>,
    company: Option<String>,
    city: Option<String>,
    country: Option<Country>,
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
    /// Everything else - start, duration, rate and skills are read from here by several
    /// possible names (an alias would refuse the island if two of them appeared).
    #[serde(flatten)]
    rest: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Country {
    iso2: Option<String>,
    localized_name: Option<String>,
}

/// The first of `names` the island carries, as text.
fn first_text(rest: &serde_json::Map<String, Value>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .filter_map(|name| rest.get(*name))
        .find_map(value_text)
}

/// A value as text: a string, a number, or an object's name or label.
fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Facts::value(text),
        Value::Number(number) => Some(number.to_string()),
        Value::Object(object) => ["name", "localizedName", "label", "text"]
            .iter()
            .filter_map(|key| object.get(*key))
            .find_map(value_text),
        _ => None,
    }
}

/// A whole number of the island (`startYear`, `durationInMonths`).
fn number(rest: &serde_json::Map<String, Value>, name: &str) -> Option<u64> {
    rest.get(name).and_then(Value::as_u64)
}

/// Facts of the island, measured on a real page: the start as `startYear` and `startMonth`
/// (`startText` null), the duration as `durationInMonths` (`durationText` null), the rate
/// as `budget` (often null), skills as `{enabled: [...], disabled: [...]}`, the contract
/// type as a code. The text forms stay first - a page may carry them.
fn facts(project: &Project, translations: &serde_json::Map<String, Value>) -> Facts {
    let rest = &project.rest;
    let start = first_text(rest, &["start", "startText", "startDate"]).or_else(|| {
        let (year, month) = (number(rest, "startYear")?, number(rest, "startMonth")?);
        (1..=12)
            .contains(&month)
            .then(|| format!("{month:02}.{year}"))
    });
    // German page words (the unit the engine reads), do not translate.
    let duration = first_text(rest, &["duration", "durationText"]).or_else(|| {
        number(rest, "durationInMonths")
            .filter(|n| *n > 0)
            .map(|n| format!("{n} {}", if n == 1 { "Monat" } else { "Monate" }))
    });
    // A budget without a unit (a bare number) is no rate the engine can read: left out
    // rather than given an invented unit.
    let rate = first_text(rest, &["rate", "hourlyRate", "rateText"]).or_else(|| {
        first_text(rest, &["budget"]).filter(|b| crate::matching::facts::readable_rate(b))
    });
    let mut facts = Facts {
        remote_percent: project
            .contract_type
            .as_ref()
            .and_then(|c| c.remote_in_percent)
            .and_then(|p| u8::try_from(p.min(100)).ok()),
        employment_type: project
            .contract_type
            .as_ref()
            .and_then(|c| c.contract_type.as_deref())
            .and_then(|code| contract_label(code, translations)),
        industries: rest
            .get("industry")
            .and_then(|i| {
                ["nameDe", "name", "localizedName"]
                    .iter()
                    .find_map(|k| i.get(*k))
            })
            .and_then(Value::as_str)
            .and_then(Facts::value),
        start,
        duration,
        rate,
        ..Facts::default()
    };
    let skills = match rest.get("skills") {
        Some(Value::Array(skills)) => skills.iter().collect(),
        Some(Value::Object(groups)) => groups
            .get("enabled")
            .and_then(Value::as_array)
            .map(|enabled| enabled.iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    for skill in skills.into_iter().filter_map(value_text) {
        facts.add_skill(&skill);
    }
    facts
}

/// The contract type in the page's own words: its translation of the code, else the fixed
/// labels (the portal's German words, do not translate). The engine reads them:
/// "Festanstellung" is permanent, "Arbeitnehmerüberlassung" is ANÜ, "Freiberuflich" interim.
fn contract_label(code: &str, translations: &serde_json::Map<String, Value>) -> Option<String> {
    let code = code.trim().to_ascii_lowercase();
    let translated = [
        format!("contract_types_{code}"),
        format!("contract_type_{code}"),
    ]
    .iter()
    .filter_map(|key| translations.get(key))
    .find_map(Value::as_str)
    .and_then(Facts::value);
    translated.or_else(|| {
        let fixed = match code.as_str() {
            "contracting" | "freelance" => "Freiberuflich",
            "permanent_position" | "permanent" | "onsite" => "Festanstellung",
            "employee_leasing" | "leasing" => "Arbeitnehmerüberlassung",
            _ => return None,
        };
        Some(fixed.to_string())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    localized_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractType {
    contract_type: Option<String>,
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
    let Some(Island {
        project,
        translations,
    }) = island
    else {
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
    let facts = facts(&project, &translations);
    let remote = facts.remote_percent.is_some_and(|p| p >= 100);
    let city = project
        .city
        .as_deref()
        .map(one_line)
        .filter(|c| !c.is_empty());
    // Several sites: all of them (the city names only one). Outside Germany the country
    // goes with it - the engine's country criterion cannot know every small town.
    let place = match city {
        _ if places.len() > 1 => places.join(", "),
        Some(city) => city,
        None if !places.is_empty() => places.join(", "),
        None => String::new(),
    };
    let foreign = project
        .country
        .as_ref()
        .filter(|c| {
            c.iso2
                .as_deref()
                .is_some_and(|iso| !iso.eq_ignore_ascii_case("DE"))
        })
        .and_then(|c| c.localized_name.as_deref())
        .map(one_line)
        .filter(|name| !name.is_empty() && !place.contains(name.as_str()));
    let location = match (place.is_empty(), foreign) {
        (false, Some(country)) => format!("{place}, {country}"),
        (true, Some(country)) => country,
        (false, None) => place,
        (true, None) if remote => "Remote".into(),
        (true, None) => String::new(),
    };
    Ok(Parsed {
        text: project.description.as_deref().map(html_to_text),
        closed: project.is_archived || !project.active || project.disabled,
        fields: PageFields {
            title: project.title.as_deref().map(one_line).unwrap_or_default(),
            company: project.company.as_deref().map(one_line).unwrap_or_default(),
            location,
        },
        facts,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The island in the shape a real page has it (measured, invented values): start as
    /// year and month, the duration in months, `budget` null, skills in `enabled`, the
    /// contract type as a code, the country as an object, the page's translations.
    pub(crate) fn island(id: u64, description: &str, archived: bool) -> Value {
        serde_json::json!({
            "isHidden": false,
            "reCaptchaSiteKey": "6Lc-invented-site-key",
            "project": {
                "firstName": "Erika", "lastName": "Muster",
                "id": id, "title": "SAP FI/CO Berater (m/w/d)", "company": "Ferrum Systems SE",
                "country": {"country": "/api/countries/1", "id": 1, "iso2": "DE",
                            "nameDe": "Deutschland", "nameEn": "Germany", "localizedName": "Deutschland"},
                "city": null, "locations": [{"localizedName": "München"}, {"localizedName": "Remote"}],
                "description": description, "isArchived": archived, "active": true, "disabled": false,
                "contractType": {"contractType": "contracting", "remoteInPercent": 50},
                "startYear": 2026, "startMonth": 10, "startText": null,
                "durationInMonths": 6, "durationText": null, "budget": null,
                "industry": {"industry": "/api/industries/3", "nameDe": "Finanzwesen", "nameEn": "Finance"},
                "skills": {"enabled": [{"localizedName": "SAP FI"}, {"localizedName": "SAP CO"}], "disabled": []}
            },
            "translations": {
                "contract_types_contracting": "Freiberuflich",
                "contract_types_permanent_position": "Festanstellung",
                "contract_types_employee_leasing": "Arbeitnehmerüberlassung",
                "budget": "Budget"
            }
        })
    }

    fn html_of(island: &Value) -> String {
        format!(
            r#"<html><body><div class="project-body-description"><div class="ql-editor">sichtbar</div></div>
            <script type="application/json" class="js-react-on-rails-component" data-component-name="ProjectShow">{island}</script></body></html>"#
        )
    }

    pub(crate) fn page(id: u64, description: &str, archived: bool) -> String {
        html_of(&island(id, description, archived))
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
        // The facts as the real island states them.
        assert_eq!(
            p.facts,
            Facts {
                employment_type: Some("Freiberuflich".into()),
                industries: Some("Finanzwesen".into()),
                remote_percent: Some(50),
                start: Some("10.2026".into()),
                duration: Some("6 Monate".into()),
                skills: vec!["SAP FI".into(), "SAP CO".into()],
                ..Facts::default()
            }
        );
        // Text forms stay first; plain names and plain strings are read too.
        let mut other = island(7, "x", false);
        other["project"]["startText"] = "ab sofort".into();
        other["project"]["durationText"] = "3 bis 6 Monate".into();
        other["project"]["skills"] = serde_json::json!(["ABAP"]);
        other["project"]["hourlyRate"] = "95 €/h".into();
        let facts = parse(&html_of(&other), Some("7")).unwrap().facts;
        assert_eq!(facts.start.as_deref(), Some("ab sofort"));
        assert_eq!(facts.duration.as_deref(), Some("3 bis 6 Monate"));
        assert_eq!(facts.skills, ["ABAP"]);
        assert_eq!(facts.rate.as_deref(), Some("95 €/h"));
    }

    /// The start and duration read from the numbers reach the engine's readers.
    #[test]
    fn the_numbers_read_as_the_engine_reads_them() {
        let facts = parse(&page(5, "x", false), Some("5")).unwrap().facts;
        assert!(crate::matching::facts::parse_start(facts.start.as_deref().unwrap()).is_some());
        let mut one = island(5, "x", false);
        one["project"]["durationInMonths"] = 1.into();
        one["project"]["startMonth"] = 13.into();
        let facts = parse(&html_of(&one), Some("5")).unwrap().facts;
        assert_eq!(facts.duration.as_deref(), Some("1 Monat"));
        assert_eq!(facts.start, None, "no 13th month");
    }

    /// A budget is a rate only with a unit; a bare number is left out, not given one.
    #[test]
    fn a_budget_needs_a_unit() {
        let rate = |budget: Value| {
            let mut i = island(5, "x", false);
            i["project"]["budget"] = budget;
            parse(&html_of(&i), Some("5")).unwrap().facts.rate
        };
        assert_eq!(rate(Value::Null), None);
        assert_eq!(rate(95.into()), None);
        assert_eq!(rate("950".into()), None);
        assert_eq!(
            rate("950 € pro Tag".into()).as_deref(),
            Some("950 € pro Tag")
        );
    }

    /// Every contract code becomes the page's word for it, so the engine sees a permanent
    /// position or temporary agency work although the description does not repeat it.
    #[test]
    fn contract_codes_in_the_pages_words() {
        let label = |code: &str, translated: bool| {
            let mut i = island(5, "x", false);
            i["project"]["contractType"]["contractType"] = code.into();
            if !translated {
                i["translations"] = serde_json::json!({});
            }
            parse(&html_of(&i), Some("5"))
                .unwrap()
                .facts
                .employment_type
        };
        for translated in [true, false] {
            assert_eq!(
                label("contracting", translated).as_deref(),
                Some("Freiberuflich")
            );
            assert_eq!(
                label("permanent_position", translated).as_deref(),
                Some("Festanstellung")
            );
            assert_eq!(
                label("employee_leasing", translated).as_deref(),
                Some("Arbeitnehmerüberlassung")
            );
        }
        assert_eq!(label("unknown_code", false), None);
    }

    /// A Swiss or Austrian town the country list does not know still names its country.
    #[test]
    fn a_foreign_country_goes_with_the_place() {
        let mut ch = island(5, "x", false);
        ch["project"]["city"] = "Zug".into();
        ch["project"]["locations"] = serde_json::json!([{"localizedName": "Zug"}]);
        ch["project"]["country"] = serde_json::json!({"iso2": "CH", "localizedName": "Schweiz"});
        let p = parse(&html_of(&ch), Some("5")).unwrap();
        assert_eq!(p.fields.location, "Zug, Schweiz");
        // Germany stays the bare place; a city with several sites lists all of them.
        let mut de = island(5, "x", false);
        de["project"]["city"] = "Köln".into();
        de["project"]["locations"] = serde_json::json!([{"localizedName": "Köln"}]);
        assert_eq!(
            parse(&html_of(&de), Some("5")).unwrap().fields.location,
            "Köln"
        );
        de["project"]["locations"] =
            serde_json::json!([{"localizedName": "Köln"}, {"localizedName": "Bonn"}]);
        assert_eq!(
            parse(&html_of(&de), Some("5")).unwrap().fields.location,
            "Köln, Bonn"
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

    /// A check page served with 200 on a project address is a block at once, not a
    /// changed layout; the site key in a real page's island is no check.
    #[test]
    fn a_check_page_on_a_project_address_blocks() {
        let link =
            crate::portal::job_link("https://www.freelancermap.de/nproj/2971857.html").unwrap();
        let judge = |html: &str| Freelancermap.guest_page(html, "/nproj/2971857.html", &link);
        for check in [
            r#"<html><body><div class="g-recaptcha" data-sitekey="x"></div></body></html>"#,
            r#"<html><body><form id="challenge-form" action="/?__cf_chl_f_tk=x"></form></body></html>"#,
            r#"<html><head><script>window._cf_chl_opt={cvId:'3'};</script></head><body></body></html>"#,
            r#"<html><body><iframe src="https://challenges.cloudflare.com/x"></iframe></body></html>"#,
        ] {
            assert_eq!(
                judge(check),
                PageOutcome::Blocked(Cause::Captcha),
                "{check}"
            );
        }
        assert!(matches!(
            judge(&page(
                2_971_857,
                "<p>Aufgaben und Anforderungen.</p>",
                false
            )),
            PageOutcome::Text { .. }
        ));
        assert_eq!(
            judge("<html><body>anders</body></html>"),
            PageOutcome::Suspicious(Cause::PageNotRecognised)
        );
    }

    /// The private real page (not checked in). Ignored by default so that a missing file
    /// shows as "ignored" instead of passing silently; run it with `--ignored` in the main
    /// checkout.
    #[test]
    #[ignore = "needs core/tests/fixtures/private/pages/freelancermap-3049771.html"]
    fn real_page_when_available() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/private/pages/freelancermap-3049771.html");
        let html = std::fs::read_to_string(path).expect("the private page");
        // No real names in the repository: the test checks that fields are filled at all.
        let p = parse(&html, Some("3049771")).unwrap();
        assert!(p.text.unwrap().chars().count() > 1_000);
        assert!(!p.fields.company.is_empty());
        assert!(!p.fields.location.is_empty());
        assert!(!p.closed);
        assert!(p.facts.start.is_some() && p.facts.duration.is_some());
        assert!(p.facts.employment_type.is_some());
        assert!(!p.facts.skills.is_empty());
        assert!(!super::super::has_challenge(
            &Html::parse_document(&html),
            &html
        ));
    }
}
