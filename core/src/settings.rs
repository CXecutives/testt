//! Settings of the app - stored as JSON in the key/value table of the database.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::Result;
use crate::portal::{FetchPath, Portal};
use crate::store::Store;

const KEY: &str = "settings";

/// `#[serde(default)]` per field: an older file without today's fields keeps loading, and
/// fields of earlier versions (`format`, `scope`, `firstRunSeen`, `sessionPortals`) are
/// skipped silently - serde only refuses unknown fields with `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Workspace; `None` = default (`Documents\Job-Alert-Monitor`). Only changeable through
    /// the folder dialog - never a path sent by the frontend.
    pub workspace: Option<PathBuf>,
    /// Switches per portal. Earlier versions stored the list of enabled portals here; that
    /// form still loads (listed = enabled, missing = disabled).
    #[serde(deserialize_with = "portals_any_form")]
    pub portals: BTreeMap<Portal, PortalSwitches>,
    /// Start a fetch run at app start (mailbox connected, last fetch older than 6 hours).
    pub auto_fetch_on_start: bool,
    /// Move inbox jobs that are no favourite to the archive this many days after they were
    /// first seen, at the end of every run; 0 = never.
    pub auto_archive_days: u32,
    /// Delete the jobs that lie in the trash this long for good, at the end of every run;
    /// 0 = never.
    pub auto_empty_trash_days: u32,
    /// Language of the interface and of the exported Excel file, HTML overview and prompts;
    /// `None` = the language of the OS ([`Settings::language_or`]). The text files per job
    /// stay German (a contract with the matching skill). A code of a newer version reads as
    /// `None`.
    #[serde(deserialize_with = "known_language")]
    pub language: Option<Language>,
}

/// The app's language. German on a German system, English on any other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Language {
    De,
    En,
}

impl Language {
    /// The app's language until the user picks one in Einstellungen: German, whatever the
    /// OS display language is (the app is written for German-speaking consultants; many of
    /// them run an English Windows or macOS).
    pub const DEFAULT: Language = Language::De;

    /// The language for a BCP 47 tag of the OS (`de-DE`, `de_AT.UTF-8`, `gsw-CH` ...): German
    /// for German, English for everything else (and for no tag at all).
    pub fn from_locale(tag: Option<&str>) -> Language {
        let primary = tag
            .unwrap_or_default()
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        // `gsw` is Swiss German, `nds` Low German: their readers read German.
        if matches!(primary.as_str(), "de" | "gsw" | "nds") {
            Language::De
        } else {
            Language::En
        }
    }

    /// The code of the language (`de`, `en`), as in `<html lang>`.
    pub fn code(self) -> &'static str {
        match self {
            Language::De => "de",
            Language::En => "en",
        }
    }
}

/// Days after which old jobs archive themselves when the switch is on.
pub const AUTO_ARCHIVE_DAYS: u32 = 30;
/// Days after which the trash empties itself when the switch is on.
pub const AUTO_EMPTY_TRASH_DAYS: u32 = 30;

/// The switches of one portal. Safe defaults: active, details fetched, never signed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSwitches {
    /// Alert mails of the portal are read.
    pub enabled: bool,
    /// Job pages are fetched; off = zero requests to the portal.
    pub fetch_details: bool,
    /// The app may sign in (only portals with a sign-in, i.e. freelance.de).
    pub login_enabled: bool,
}

impl Default for PortalSwitches {
    fn default() -> PortalSwitches {
        PortalSwitches {
            enabled: true,
            fetch_details: true,
            login_enabled: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            workspace: None,
            portals: Portal::ALL
                .into_iter()
                .map(|p| (p, PortalSwitches::default()))
                .collect(),
            auto_fetch_on_start: true,
            language: None,
            auto_archive_days: AUTO_ARCHIVE_DAYS,
            auto_empty_trash_days: AUTO_EMPTY_TRASH_DAYS,
        }
    }
}

impl Settings {
    /// Stored settings; unreadable ones are replaced by the defaults (they are convenience,
    /// not data) - except the portal switches, which are safety: a switched-off portal
    /// must never get requests again because the stored JSON broke. Those come back off
    /// ([`Settings::safe_after_damage`]) until the user switches them on again.
    pub fn load(store: &Store) -> Result<Settings> {
        Ok(match store.kv_get(KEY)? {
            Some(json) => serde_json::from_str::<Settings>(&json).map_or_else(
                |e| {
                    log::warn!(
                        "settings unreadable ({e}): defaults, every portal switched off until \
                         the user switches it on again"
                    );
                    Settings::safe_after_damage()
                },
                Settings::normalized,
            ),
            None => Settings::default(),
        })
    }

    /// The defaults with every portal switched off: no mail read, no page requested, no
    /// sign-in.
    fn safe_after_damage() -> Settings {
        let off = PortalSwitches {
            enabled: false,
            fetch_details: false,
            login_enabled: false,
        };
        Settings {
            portals: Portal::ALL.into_iter().map(|p| (p, off)).collect(),
            ..Settings::default()
        }
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        let json = serde_json::to_string(&self.clone().normalized()).expect("serialisable");
        store.kv_set(KEY, &json)
    }

    /// Every portal present; sign-in only where the portal has one.
    fn normalized(mut self) -> Settings {
        for portal in Portal::ALL {
            let switches = self.portals.entry(portal).or_default();
            if !portal.access().can_sign_in() {
                switches.login_enabled = false;
            }
        }
        self
    }

    /// The switches of a portal.
    pub fn portal(&self, portal: Portal) -> PortalSwitches {
        self.portals.get(&portal).copied().unwrap_or_default()
    }

    /// Portals whose alert mails are read (in the order of `Portal::ALL`).
    pub fn enabled_portals(&self) -> Vec<Portal> {
        Portal::ALL
            .into_iter()
            .filter(|&p| self.portal(p).enabled)
            .collect()
    }

    /// Portals whose job pages may be fetched: enabled, with details switched on and - for
    /// a portal that is only readable signed in - with the sign-in allowed. Otherwise the
    /// portal gets zero requests, and no sign-in window ever opens unasked.
    pub fn fetch_portals(&self) -> Vec<Portal> {
        Portal::ALL
            .into_iter()
            .filter(|&p| self.fetch_path(p).is_some())
            .collect()
    }

    /// The fetch path of a portal: `None` = zero requests; the session window only with
    /// the sign-in switched on.
    pub fn fetch_path(&self, portal: Portal) -> Option<FetchPath> {
        let switches = self.portal(portal);
        if !(switches.enabled && switches.fetch_details) {
            return None;
        }
        portal.access().path(switches.login_enabled)
    }

    /// The chosen language, else the one of the OS.
    pub fn language_or(&self, system: Language) -> Language {
        self.language.unwrap_or(system)
    }

    /// Workspace (chosen or default).
    pub fn workspace_or(&self, default: &std::path::Path) -> PathBuf {
        self.workspace
            .clone()
            .unwrap_or_else(|| default.to_path_buf())
    }
}

/// A stored language; one this version does not know (a newer version wrote it) is none.
fn known_language<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Language>, D::Error> {
    let code = Option::<String>::deserialize(deserializer)?;
    Ok(code.and_then(|code| serde_json::from_value(serde_json::Value::String(code)).ok()))
}

/// Reads the portal switches in today's map form or the list form of earlier versions.
/// Unknown portal keys are skipped.
fn portals_any_form<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<Portal, PortalSwitches>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum AnyForm {
        List(Vec<String>),
        Map(BTreeMap<String, PortalSwitches>),
    }
    Ok(match AnyForm::deserialize(deserializer)? {
        AnyForm::List(keys) => Portal::ALL
            .into_iter()
            .map(|p| {
                let enabled = keys.iter().any(|k| k == p.key());
                (
                    p,
                    PortalSwitches {
                        enabled,
                        ..PortalSwitches::default()
                    },
                )
            })
            .collect(),
        AnyForm::Map(map) => map
            .into_iter()
            .filter_map(|(key, switches)| Some((Portal::from_key(&key)?, switches)))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_and_repair() {
        let store = Store::in_memory().unwrap();
        assert_eq!(Settings::load(&store).unwrap(), Settings::default());
        let mut s = Settings::load(&store).unwrap();
        s.portals.insert(
            Portal::LinkedIn,
            PortalSwitches {
                enabled: false,
                fetch_details: false,
                login_enabled: true,
            },
        );
        s.auto_fetch_on_start = false;
        s.save(&store).unwrap();
        let back = Settings::load(&store).unwrap();
        assert_eq!(
            back.portal(Portal::LinkedIn),
            PortalSwitches {
                enabled: false,
                fetch_details: false,
                // LinkedIn has no sign-in: the switch cannot be on.
                login_enabled: false,
            }
        );
        assert!(!back.auto_fetch_on_start);
        assert_eq!(
            back.enabled_portals(),
            [Portal::FreelanceDe, Portal::Freelancermap]
        );
        // Missing fields: default per field.
        store
            .kv_set(KEY, r#"{"portals":{"freelance":{"loginEnabled":true}}}"#)
            .unwrap();
        let partial = Settings::load(&store).unwrap();
        assert!(partial.portal(Portal::FreelanceDe).login_enabled);
        assert!(partial.portal(Portal::FreelanceDe).fetch_details);
        assert!(partial.portal(Portal::LinkedIn).enabled);
        assert!(partial.auto_fetch_on_start);
    }

    /// Broken JSON gives the defaults for convenience, but never switches a portal on:
    /// portals the user switched off must get zero requests (the hard rule), so all of them
    /// come back off until the user switches them on again.
    #[test]
    fn broken_settings_switch_no_portal_on() {
        let store = Store::in_memory().unwrap();
        let mut s = Settings::default();
        for switches in s.portals.values_mut() {
            switches.enabled = false;
        }
        s.save(&store).unwrap();
        let paths = crate::pipeline::stored_paths(std::sync::Arc::new(Store::in_memory().unwrap()));
        assert!(Portal::ALL.iter().all(|&p| paths(p).is_some()), "defaults");
        let json = store.kv_get(KEY).unwrap().unwrap();
        store.kv_set(KEY, &json[..json.len() - 1]).unwrap();
        let back = Settings::load(&store).unwrap();
        assert!(back.fetch_portals().is_empty());
        assert!(back.enabled_portals().is_empty());
        assert_eq!(
            back.auto_archive_days, AUTO_ARCHIVE_DAYS,
            "the rest: defaults"
        );
        let shared = std::sync::Arc::new(store);
        shared.kv_set(KEY, "{kaputt").unwrap();
        let paths = crate::pipeline::stored_paths(shared);
        assert!(Portal::ALL.iter().all(|&p| paths(p).is_none()));
    }

    /// A file of an earlier version carries fields that no longer exist and the list form of
    /// the portal choice: it loads, and the choice survives as the `enabled` switch.
    #[test]
    fn settings_of_an_older_version_still_load() {
        let store = Store::in_memory().unwrap();
        store
            .kv_set(
                KEY,
                r#"{"workspace":null,"format":"xlsx","scope":"week","portals":["linkedin"],"sessionPortals":["freelance"],"firstRunSeen":true}"#,
            )
            .unwrap();
        let back = Settings::load(&store).unwrap();
        assert_eq!(back.enabled_portals(), [Portal::LinkedIn]);
        assert_eq!(back.fetch_portals(), [Portal::LinkedIn]);
        assert_eq!(back.workspace, None);
        assert!(back.auto_fetch_on_start);
        store.kv_set(KEY, r#"{"portals":[]}"#).unwrap();
        assert!(Settings::load(&store).unwrap().enabled_portals().is_empty());
    }

    /// Without a choice the app follows the OS: German only on a German system. A stored
    /// choice wins and survives a restart; an unknown code falls back to the OS.
    #[test]
    fn the_language_follows_the_system_until_chosen() {
        for (tag, language) in [
            (Some("de-DE"), Language::De),
            (Some("de-AT"), Language::De),
            (Some("de_CH.UTF-8"), Language::De),
            (Some("gsw-CH"), Language::De),
            (Some("DE"), Language::De),
            (Some("en-US"), Language::En),
            (Some("fr-FR"), Language::En),
            (Some("nl"), Language::En),
            (Some("dev"), Language::En),
            (Some(""), Language::En),
            (None, Language::En),
        ] {
            assert_eq!(Language::from_locale(tag), language, "{tag:?}");
        }

        let store = Store::in_memory().unwrap();
        let mut s = Settings::load(&store).unwrap();
        assert_eq!(s.language, None);
        assert_eq!(s.language_or(Language::De), Language::De);
        assert_eq!(s.language_or(Language::En), Language::En);
        s.language = Some(Language::En);
        s.save(&store).unwrap();
        let back = Settings::load(&store).unwrap();
        assert_eq!(back.language_or(Language::De), Language::En);
        assert!(
            store
                .kv_get(KEY)
                .unwrap()
                .unwrap()
                .contains(r#""language":"en""#)
        );
        // A language of a newer version: the settings stay, the language follows the OS.
        store
            .kv_set(KEY, r#"{"language":"fr","autoFetchOnStart":false}"#)
            .unwrap();
        let newer = Settings::load(&store).unwrap();
        assert!(!newer.auto_fetch_on_start);
        assert_eq!(newer.language, None);
    }

    #[test]
    fn details_off_removes_the_portal_from_fetching_only() {
        let mut s = Settings::default();
        s.portals.get_mut(&Portal::LinkedIn).unwrap().fetch_details = false;
        assert_eq!(s.enabled_portals(), Portal::ALL);
        assert_eq!(
            s.fetch_portals(),
            [Portal::FreelanceDe, Portal::Freelancermap]
        );
    }

    /// freelance.de without the sign-in switch goes as a guest (the public teaser) - never
    /// in the session window; with the switch in the session window.
    #[test]
    fn a_portal_with_a_sign_in_goes_as_a_guest_until_the_switch() {
        let mut s = Settings::default();
        assert_eq!(s.fetch_portals(), Portal::ALL);
        assert_eq!(s.fetch_path(Portal::FreelanceDe), Some(FetchPath::Guest));
        s.portals
            .get_mut(&Portal::FreelanceDe)
            .unwrap()
            .login_enabled = true;
        assert_eq!(s.fetch_path(Portal::FreelanceDe), Some(FetchPath::Session));
        assert_eq!(s.fetch_path(Portal::LinkedIn), Some(FetchPath::Guest));
        s.portals
            .get_mut(&Portal::FreelanceDe)
            .unwrap()
            .fetch_details = false;
        assert_eq!(s.fetch_path(Portal::FreelanceDe), None);
    }
}
