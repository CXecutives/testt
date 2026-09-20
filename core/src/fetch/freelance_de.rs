//! freelance.de-Projektseite im Sitzungsfenster. Das Fenster sammelt per Skript nur
//! Befunde (Status, Adresse, Anmelde-Zeichen, HTML des Beschreibungsfelds); bewertet und in
//! Text verwandelt wird hier in Rust. Seitenfelder (Firma, Ort) werden bewusst nicht
//! übernommen – für Nicht-EXPERT-Mitglieder stehen dort Platzhalter.

use serde::Deserialize;
use url::Url;

use super::{PageOutcome, Parsed, judge};
use crate::portal::host_is;
use crate::text::html_to_text;

/// Befund-Skript: synchron, in `try/catch`, liefert immer JSON. Keine Timer, keine Logik im
/// Seiten-JS (ein verstecktes WebView drosselt Timer).
pub const PROBE_JS: &str = r#"(() => { try {
  const text = (el) => (el && el.textContent || '').replace(/\s+/g, ' ').trim();
  const heading = [...document.querySelectorAll('h1, h2, h3')].find((h) => /Projektbeschreibung/i.test(text(h)));
  const box = heading && (heading.closest('.panel') || heading.parentElement);
  const body = box && (box.querySelector('.panel-body') || box);
  return JSON.stringify({
    ok: true,
    status: performance.getEntriesByType('navigation')[0]?.responseStatus ?? 0,
    url: location.href,
    hasLogout: !!document.querySelector('a[href*="logout.php"]'),
    hasExpertMarker: /für EXPERT-Mitglieder sichtbar/i.test(document.body ? document.body.innerText : ''),
    hasLoginForm: !!document.querySelector('#username') && !!document.querySelector('#password'),
    hasCaptcha: !!document.querySelector('.g-recaptcha, .h-captcha, [data-sitekey], iframe[src*="captcha"], iframe[src*="challenges.cloudflare"]'),
    panelHtml: body ? body.innerHTML : null,
  });
} catch (e) { return JSON.stringify({ ok: false, err: String(e), url: String(location.href) }); } })()"#;

/// Was das Skript meldet (reine Befunde, daher die vielen Wahrheitswerte).
#[expect(clippy::struct_excessive_bools, reason = "reine Befunde einer Seite")]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionPage {
    pub ok: bool,
    pub err: Option<String>,
    pub status: u16,
    pub url: String,
    pub has_logout: bool,
    pub has_expert_marker: bool,
    pub has_login_form: bool,
    pub has_captcha: bool,
    pub panel_html: Option<String>,
}

/// Nach der Anmeldung leitet freelance.de die erste Seite einmal hierher um. Auch hier gilt
/// die Portal-Regel zuerst: Derselbe Pfad auf einem fremden Host ist keine Weiterleitung des
/// Portals und darf keinen zweiten Zugriff auslösen.
pub fn is_postlogin(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| is_portal_url(&u) && u.path().starts_with("/promotion/postlogin"))
}

/// Eine Seite des Portals: https auf `freelance.de` oder einer Subdomain. Alles andere
/// (Fehlerseite der WebView `chrome-error://…`, `about:blank`, fremder Host) ist keine.
pub fn is_portal_url(url: &Url) -> bool {
    // Der URL-Parser schreibt Hosts von http(s)-Adressen schon klein.
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| host_is(host, "freelance.de"))
}

/// Die Anmeldeseite.
pub const LOGIN_URL: &str = "https://www.freelance.de/login.php";
/// Abmelden.
pub const LOGOUT_URL: &str = "https://www.freelance.de/logout.php";

/// Kürzer als das und mit Registrierungsaufruf: nur der Teaser (gemessen 242–306 Zeichen).
const TEASER_MAX_CHARS: usize = 500;

/// Hinter diesen Überschriften ist die Beschreibung zu Ende (Werbung, Kontakt, Ähnliches).
const END_MARKERS: &[&str] = &[
    "Kostenlos registrieren",
    "Kontaktdaten",
    "Ähnliche Projekte",
    "Kategorien und Skills",
    "Sie suchen Freelancer?",
];

/// Befund → Ergebnis-Matrix. `project_id`: die ID aus der Mail.
///
/// „Anmeldung nötig“ nur, wenn eine Anmeldung helfen kann (kein Abmelde-Link). Was trotz
/// Abmelde-Link nicht stimmt, ist verdächtig – eine erneute Anmeldung änderte nichts und
/// liefe im Kreis, der Schutzschalter dagegen greift.
pub fn judge_page(page: &SessionPage, project_id: &str) -> PageOutcome {
    // Zuerst: Ist das überhaupt eine Seite des Portals? Eine gescheiterte Navigation meldet
    // die WebView ebenfalls als „geladen“ – dann auf ihrer eigenen Fehlerseite.
    let Some(url) = Url::parse(&page.url).ok().filter(is_portal_url) else {
        return PageOutcome::NetError {
            timeout: false,
            detail: "Seite nicht geladen".into(),
        };
    };
    if !page.ok {
        return PageOutcome::Suspicious(format!(
            "Seite nicht auswertbar ({})",
            page.err.as_deref().unwrap_or("unbekannt")
        ));
    }
    // Eine Sicherheitsprüfung sperrt – außer das Widget steckt nur passiv in einer normal
    // geladenen Projektseite mit Beschreibung.
    if page.has_captcha && (page.panel_html.is_none() || page.status != 200) {
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
    let path = url.path().to_ascii_lowercase();
    if path.starts_with("/login") || (page.has_login_form && !page.has_logout) {
        return PageOutcome::LoginRequired("Anmeldeseite".into());
    }
    if !page.has_logout {
        return PageOutcome::LoginRequired("kein Abmelde-Link".into());
    }
    // Abgelaufenes Projekt: freelance.de leitet auf eine Projektliste um – mit Hinweis
    // (`/projekte?info_message=…`) oder auf die Kategorie des Projekts. Gemessen ist das nur
    // abgemeldet; „weg“ gilt deshalb erst in einer bestätigten Sitzung. Eine Projektseite hat
    // immer „/projekt-<ID>“ im Pfad.
    if !page.url.contains(project_id) && is_listing(&path) {
        return PageOutcome::Gone;
    }
    // Teaser: kurzer Text mit Registrierungsaufruf (gemessen 277 Zeichen; der Aufruf ist
    // zugleich Endmarke, darum vor dem Kürzen geprüft) oder nur der „EXPERT“-Hinweis ohne
    // Textfeld. Ein langer Text mit Aufruf am Ende ist dagegen der Volltext.
    let full = page.panel_html.as_deref().map(html_to_text);
    let text = full.as_deref().map(cut_at_end_markers);
    let teaser = full
        .as_deref()
        .is_some_and(|t| t.contains("Kostenlos registrieren"))
        && text
            .as_deref()
            .is_some_and(|t| t.chars().count() < TEASER_MAX_CHARS);
    if teaser || (page.has_expert_marker && full.is_none()) {
        return PageOutcome::Suspicious("nur Teaser trotz Anmeldung".into());
    }
    // Richtige Seite? Die ID aus der Mail steht in jeder Form der Projektadresse.
    if !page.url.contains(project_id) {
        return PageOutcome::Suspicious("andere Seite als erwartet".into());
    }
    judge(Parsed {
        text,
        ..Parsed::default()
    })
}

/// Projektliste oder Kategorie (keine einzelne Projektseite).
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
mod tests {
    use super::*;

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

    /// Gemessen am 19.09.2026 (anonym): abgelaufenes Projekt → Kategorieseite; laufendes
    /// Projekt → Teaser (277 Zeichen) mit Registrierungsaufruf auf der Projektadresse.
    #[test]
    fn measured_expired_project_and_teaser() {
        let expired = page(
            "https://www.freelance.de/projekte/IT-Entwicklung-Projekte/Berater-Und-Spezialisten-Projekte",
            false,
            None,
        );
        // Abgemeldet hilft erst die Anmeldung; „weg“ nur in einer bestätigten Sitzung.
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
        // Teaser trotz Abmelde-Link: Eine erneute Anmeldung hülfe nicht – verdächtig, damit
        // der Schutzschalter greift statt einer Anmeldeschleife.
        let teaser_logged_in = SessionPage {
            has_logout: true,
            ..teaser
        };
        assert!(is_suspicious(&judge_page(&teaser_logged_in, "1287763")));
    }

    /// Eine gescheiterte Navigation (offline, DNS, TLS) meldet die WebView ebenfalls als
    /// geladen – auf ihrer eigenen Fehlerseite. Das ist ein Netzfehler, keine Anmeldewand.
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
        // Auch ein gescheitertes Befund-Skript auf der Fehlerseite.
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
                    "Ende an der Marke"
                );
            }
            other => panic!("{other:?}"),
        }
        // Kurz, aber angemeldet und im Beschreibungsfeld: verifiziert kurz.
        assert!(matches!(
            judge_page(&page(URL, true, Some("<p>Kurzprojekt SAP.</p>")), ID),
            PageOutcome::Text { short: true, .. }
        ));
    }

    /// „Anmeldung nötig“ nur, wo eine Anmeldung helfen kann.
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
        // Abgemeldet und nur der EXPERT-Hinweis: Anmeldung.
        let mut anonymous_teaser = page(URL, false, None);
        anonymous_teaser.has_expert_marker = true;
        assert!(is_login(&judge_page(&anonymous_teaser, ID)));
    }

    /// Mit Abmelde-Link ist ein Teaser verdächtig, nie „Anmeldung nötig“.
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
        // Langer Text mit Aufruf am Ende: Volltext, der Aufruf wird abgeschnitten.
        let long = format!(
            "<p>{}</p><p>Kostenlos registrieren und mehr lesen</p>",
            "Aufgaben und Anforderungen des Projekts. ".repeat(20)
        );
        match judge_page(&page(URL, true, Some(&long)), ID) {
            PageOutcome::Text { text, .. } => assert!(!text.contains("registrieren"), "{text}"),
            other => panic!("{other:?}"),
        }
    }

    /// Ein Captcha sperrt, außer es steckt passiv in einer normal geladenen Seite mit
    /// Beschreibung.
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
        assert!(matches!(judge_page(&wall, ID), PageOutcome::Blocked(_)));
        // Mit Fehlercode ist es immer eine Sperre – auch mit Textfeld.
        let mut with_error = page(URL, true, Some(&long));
        with_error.has_captcha = true;
        with_error.status = 503;
        assert!(matches!(
            judge_page(&with_error, ID),
            PageOutcome::Blocked(_)
        ));
        // Auch abgemeldet: eine Prüfseite ist eine Sperre, keine Anmeldung.
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
            PageOutcome::Throttled(_)
        ));
        assert!(is_suspicious(&judge_page(
            &page(
                "https://www.freelance.de/projekte/projekt-9999999-x",
                true,
                Some("<p>x</p>")
            ),
            ID
        )));
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
        // Derselbe Pfad woanders ist keine Weiterleitung des Portals: kein zweiter Zugriff.
        assert!(!is_postlogin("https://example.com/promotion/postlogin.php"));
        assert!(!is_postlogin(
            "http://www.freelance.de/promotion/postlogin.php"
        ));
    }
}
