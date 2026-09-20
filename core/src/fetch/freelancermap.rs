//! freelancermap-Projektseite: Die Daten stehen vollständig in einer JSON-Insel
//! (`<script type="application/json" data-component-name="ProjectShow">`) – verlässlicher
//! als das sichtbare Markup und ohne Kürzung an „Bewerben“.

use std::sync::LazyLock;

use scraper::{Html, Selector};
use serde::Deserialize;
use url::Url;

use super::site::SessionPage;
use super::{PageFields, PageOutcome, Parsed, judge};
use crate::portal::host_is;
use crate::text::{html_to_text, one_line};

/// Befund-Skript des Sitzungsfensters: die ganze Seite, damit derselbe Parser wie beim
/// Gastweg sie auswertet. In JS auf 2 MB gekappt – ein Befund geht als Zeichenkette über die
/// Fenstergrenze, und so groß ist keine Anzeige.
///
/// `hasAccountMenu` ist die Anmelde-Erkennung und **vermutet, ungeprüft**: an einer echten
/// angemeldeten Seite nachzumessen.
pub const PROBE_JS: &str = r#"(() => { try {
  return JSON.stringify({
    ok: true,
    status: performance.getEntriesByType('navigation')[0]?.responseStatus ?? 0,
    url: location.href,
    hasCaptcha: !!document.querySelector('.g-recaptcha, .h-captcha, [data-sitekey], iframe[src*="captcha"], iframe[src*="challenges.cloudflare"]'),
    hasLoginForm: !!document.querySelector('input[type="password"]'),
    hasAccountMenu: !!document.querySelector('a[href*="logout"], a[href*="abmelden"], .user-menu, .js-user-menu, [data-testid="user-menu"]'),
    html: (document.documentElement ? document.documentElement.outerHTML : '').slice(0, 2 * 1024 * 1024),
  });
} catch (e) { return JSON.stringify({ ok: false, err: String(e), url: String(location.href) }); } })()"#;

/// Eine Seite des Portals: https auf `freelancermap.de`/`.com` oder einer Subdomain.
pub fn is_portal_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some_and(|host| {
            host_is(host, "freelancermap.de") || host_is(host, "freelancermap.com")
        })
}

/// freelancermap kennt keine gemessene Weiterleitung nach der Anmeldung.
pub fn is_postlogin(_url: &str) -> bool {
    false
}

/// **Vermutung, ungeprüft:** Angemeldet zeigt freelancermap ein Konto-Menü mit
/// Abmelde-Eintrag. Nachzumessen an einer echten angemeldeten Seite – bis dahin heißt ein
/// fehlendes Menü nur „Anmeldung nötig“, und das Portal fällt still auf den Gastweg zurück.
pub fn signed_in(page: &SessionPage) -> bool {
    page.has_account_menu
}

/// Befund des Sitzungsfensters → Ergebnis-Matrix. `project_id`: die ID aus der Mail (bei
/// Slug-Links ein Hash, dann ist keine Prüfung möglich).
pub fn judge_page(page: &SessionPage, project_id: &str) -> PageOutcome {
    // Zuerst: Ist das überhaupt eine Seite des Portals? Eine gescheiterte Navigation meldet
    // die WebView ebenfalls als „geladen“ – dann auf ihrer eigenen Fehlerseite.
    if !Url::parse(&page.url).is_ok_and(|u| is_portal_url(&u)) {
        return PageOutcome::NetError {
            timeout: false,
            detail: "Seite nicht geladen".into(),
        };
    }
    if !page.ok {
        return PageOutcome::Suspicious(format!(
            "Seite nicht auswertbar ({})",
            page.err.as_deref().unwrap_or("unbekannt")
        ));
    }
    if page.has_captcha {
        return PageOutcome::Blocked(
            "Sicherheitsprüfung (Captcha) – bitte im Browser öffnen".into(),
        );
    }
    match page.status {
        429 => return PageOutcome::Throttled("HTTP 429 (zu viele Anfragen)".into()),
        403 => return PageOutcome::Blocked("HTTP 403 (Zugriff verweigert)".into()),
        404 | 410 => return PageOutcome::Gone,
        500..=599 => return PageOutcome::Throttled(format!("HTTP {} (Serverfehler)", page.status)),
        _ => {}
    }
    let Some(html) = page.html.as_deref().filter(|h| !h.is_empty()) else {
        return PageOutcome::Suspicious("keine Seite übermittelt".into());
    };
    // Anmeldewand statt Projekt: Bei einem Portal mit optionaler Anmeldung ist das kein
    // Fehler – der Abruf fällt still auf den Gastweg zurück.
    let expected = expected_id(project_id);
    match parse(html, expected) {
        Ok(parsed) if parsed.text.is_some() => judge(parsed),
        _ if !signed_in(page) => PageOutcome::LoginRequired("kein Konto-Menü".into()),
        Ok(_) => PageOutcome::Suspicious("keine Beschreibung gefunden".into()),
        Err(reason) => PageOutcome::Suspicious(reason),
    }
}

/// Nur eine echte Portal-ID lässt sich auf der Seite wiederfinden; ein Hash aus einem
/// Slug-Link nicht.
fn expected_id(project_id: &str) -> Option<&str> {
    (!project_id.starts_with('u')).then_some(project_id)
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("gültiger Selektor")
}

static ISLAND: LazyLock<Selector> = LazyLock::new(|| {
    selector(r#"script[type="application/json"][data-component-name="ProjectShow"]"#)
});
/// Rückfall, falls die Insel fehlt: das sichtbare Beschreibungsfeld.
static BODY: LazyLock<Selector> =
    LazyLock::new(|| selector("div.project-body-description, div.ql-editor"));

#[derive(Deserialize)]
struct Island {
    project: Project,
}

/// Nur die Felder, die gebraucht werden – Namen des Ansprechpartners werden nie gelesen.
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

/// `expected_id`: die Projekt-ID aus der Mail (bei Slug-Links unbekannt). Eine andere ID
/// auf der Seite heißt: falsche Seite – nie deren Text speichern.
pub(super) fn parse(html: &str, expected_id: Option<&str>) -> Result<Parsed, String> {
    let doc = Html::parse_document(html);
    let island = doc
        .select(&ISLAND)
        .next()
        .and_then(|e| serde_json::from_str::<Island>(&e.text().collect::<String>()).ok());
    let Some(Island { project }) = island else {
        // Ohne die JSON-Insel lässt sich die Seite nur am Rohtext prüfen: Steht die erwartete
        // ID nirgends, ist es nicht diese Anzeige – dann lieber kein Text als ein fremder.
        if expected_id.is_some_and(|id| !html.contains(id)) {
            return Err("Projektseite nicht erkennbar".into());
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
        return Err(format!("Seite zeigt Projekt {found} statt {expected}"));
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
                "contractType": {"contractType": "contracting", "remoteInPercent": 50}
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
        assert!(parse(&page(1, "x", false), Some("2971857")).is_err());
        // Slug-Link ohne ID aus der Mail: keine Prüfung möglich.
        assert!(parse(&page(1, "x", false), None).is_ok());
    }

    #[test]
    fn archived_is_closed() {
        assert!(parse(&page(5, "x", true), Some("5")).unwrap().closed);
    }

    #[test]
    fn without_island_the_visible_body_is_used() {
        // Ohne JSON-Insel zählt der Rohtext: Steht die erwartete ID darin, gilt der sichtbare Text.
        let p = parse(
            r#"<div data-id="5" class="project-body-description"><p>Nur sichtbar</p></div>"#,
            Some("5"),
        )
        .unwrap();
        assert_eq!(p.text.as_deref(), Some("Nur sichtbar"));
        // Ohne Insel und ohne die ID: lieber kein Text als der einer fremden Seite.
        assert!(
            parse(
                r#"<div class="project-body-description"><p>Fremd</p></div>"#,
                Some("5")
            )
            .is_err()
        );
        // Slug-Link ohne bekannte ID: der sichtbare Text bleibt die einzige Quelle.
        assert_eq!(parse("<p>Suche</p>", None).unwrap().text, None);
    }

    #[test]
    fn real_page_when_available() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/private/pages/freelancermap-3049771.html");
        let Ok(html) = std::fs::read_to_string(path) else {
            eprintln!("übersprungen: private freelancermap-Seite fehlt");
            return;
        };
        // Keine echten Namen im Repo: geprüft wird, dass Felder überhaupt gefüllt werden.
        let p = parse(&html, Some("3049771")).unwrap();
        assert!(p.text.unwrap().chars().count() > 1_000);
        assert!(!p.fields.company.is_empty());
        assert!(!p.fields.location.is_empty());
        assert!(!p.closed);
    }
}
