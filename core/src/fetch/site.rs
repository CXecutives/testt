//! Ein Portal im Sitzungsfenster – alles, was die Fensterschicht über ein Portal wissen
//! muss, an einer Stelle. Das Fenster selbst kennt danach kein einzelnes Portal mehr: Es
//! lädt Adressen, sammelt einen Befund und reicht ihn hierher zurück.

use serde::Deserialize;
use url::Url;

use super::{PageOutcome, freelance_de, freelancermap};
use crate::portal::Portal;

/// Was das Befund-Skript einer Seite meldet – reine Befunde, kein Urteil. Nicht jedes Portal
/// füllt jedes Feld; was es nicht kennt, bleibt leer.
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
    /// Konto-Menü oder Abmelde-Eintrag (freelancermap: Zeichen einer Anmeldung).
    pub has_account_menu: bool,
    pub title: String,
    pub company: String,
    pub location: String,
    /// HTML des Beschreibungsfelds (freelance.de).
    pub panel_html: Option<String>,
    /// HTML der ganzen Seite (freelancermap); in JS auf 2 MB gekappt.
    pub html: Option<String>,
}

/// Ein Portal, das über ein Sitzungsfenster abgerufen werden kann.
pub struct PortalSite {
    pub portal: Portal,
    pub window_title: &'static str,
    pub login_url: &'static str,
    pub logout_url: &'static str,
    /// Befund-Skript: synchron, in `try/catch`, liefert immer JSON ([`SessionPage`]).
    pub probe_js: &'static str,
    /// Adressen, die das Fenster laden darf (Hosts dieses Portals).
    pub is_allowed: fn(&Url) -> bool,
    /// Einmalige Weiterleitung direkt nach der Anmeldung – dieselbe Seite noch einmal holen.
    pub is_postlogin: fn(&str) -> bool,
    /// Zeigt der Befund eine bestehende Anmeldung?
    pub signed_in: fn(&SessionPage) -> bool,
    /// Befund → Ergebnis-Matrix; der zweite Wert ist die Job-ID aus der Mail.
    pub judge: fn(&SessionPage, &str) -> PageOutcome,
}

static FREELANCE_DE: PortalSite = PortalSite {
    portal: Portal::FreelanceDe,
    window_title: "freelance.de – Anmeldung",
    login_url: "https://www.freelance.de/login.php",
    logout_url: "https://www.freelance.de/logout.php",
    probe_js: freelance_de::PROBE_JS,
    is_allowed: freelance_de::is_portal_url,
    is_postlogin: freelance_de::is_postlogin,
    signed_in: freelance_de::signed_in,
    judge: freelance_de::judge_page,
};

static FREELANCERMAP: PortalSite = PortalSite {
    portal: Portal::Freelancermap,
    window_title: "freelancermap.de – Anmeldung",
    login_url: "https://www.freelancermap.de/login.html",
    logout_url: "https://www.freelancermap.de/logout.html",
    probe_js: freelancermap::PROBE_JS,
    is_allowed: freelancermap::is_portal_url,
    is_postlogin: freelancermap::is_postlogin,
    signed_in: freelancermap::signed_in,
    judge: freelancermap::judge_page,
};

impl PortalSite {
    /// Beschreibung eines Portals; `None` für Portale ohne Anmeldung (LinkedIn).
    pub fn of(portal: Portal) -> Option<&'static PortalSite> {
        match portal {
            Portal::FreelanceDe => Some(&FREELANCE_DE),
            Portal::Freelancermap => Some(&FREELANCERMAP),
            Portal::LinkedIn => None,
        }
    }

    /// Fensterkennung – derselbe Name wie der Profilordner.
    pub fn label(&self) -> String {
        crate::session_dir(self.portal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::LoginMode;

    /// Jedes Portal mit Anmeldung hat eine Beschreibung, LinkedIn keine – und jede
    /// beschriebene Adresse gehört zu ihrem eigenen Portal.
    #[test]
    fn every_portal_with_a_login_has_a_site() {
        for portal in Portal::ALL {
            let site = PortalSite::of(portal);
            assert_eq!(
                site.is_some(),
                portal.login_mode() != LoginMode::None,
                "{portal}"
            );
            let Some(site) = site else { continue };
            assert_eq!(site.portal, portal);
            assert_eq!(site.label(), crate::session_dir(portal));
            for url in [site.login_url, site.logout_url] {
                let url = Url::parse(url).expect(url);
                assert!((site.is_allowed)(&url), "{url}");
            }
        }
        // Jedes Fenster bleibt bei seinem Portal.
        let fl = PortalSite::of(Portal::FreelanceDe).unwrap();
        let fm = PortalSite::of(Portal::Freelancermap).unwrap();
        assert!(!(fl.is_allowed)(&Url::parse(fm.login_url).unwrap()));
        assert!(!(fm.is_allowed)(&Url::parse(fl.login_url).unwrap()));
        assert_ne!(fl.label(), fm.label());
    }

    /// Sicherheits-Invariante: Ein Sitzungsfenster lädt nur Adressen seines eigenen Portals –
    /// kein fremder Host, kein anderes Schema, keine App-eigene Adresse.
    #[test]
    fn a_session_window_loads_nothing_but_its_own_portal() {
        let elsewhere = [
            "http://www.freelance.de/login.php",
            "https://www.freelance.de.example.org/login.php",
            "https://notfreelance.de/login.php",
            "https://www.freelancermap.de.evil.example/",
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
}
