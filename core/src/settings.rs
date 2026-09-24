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
}

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
        }
    }
}

impl Settings {
    /// Stored settings; unreadable ones are replaced by the defaults (they are convenience,
    /// not data).
    pub fn load(store: &Store) -> Result<Settings> {
        Ok(match store.kv_get(KEY)? {
            Some(json) => serde_json::from_str::<Settings>(&json).map_or_else(
                |e| {
                    log::warn!("settings unreadable ({e}), using the defaults");
                    Settings::default()
                },
                Settings::normalized,
            ),
            None => Settings::default(),
        })
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

    /// Workspace (chosen or default).
    pub fn workspace_or(&self, default: &std::path::Path) -> PathBuf {
        self.workspace
            .clone()
            .unwrap_or_else(|| default.to_path_buf())
    }
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
        // Missing fields: default per field; broken JSON: all defaults.
        store
            .kv_set(KEY, r#"{"portals":{"freelance":{"loginEnabled":true}}}"#)
            .unwrap();
        let partial = Settings::load(&store).unwrap();
        assert!(partial.portal(Portal::FreelanceDe).login_enabled);
        assert!(partial.portal(Portal::FreelanceDe).fetch_details);
        assert!(partial.portal(Portal::LinkedIn).enabled);
        assert!(partial.auto_fetch_on_start);
        store.kv_set(KEY, "{kaputt").unwrap();
        assert_eq!(Settings::load(&store).unwrap(), Settings::default());
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

    #[test]
    fn details_off_removes_the_portal_from_fetching_only() {
        let mut s = Settings::default();
        s.portals.get_mut(&Portal::LinkedIn).unwrap().fetch_details = false;
        assert_eq!(s.enabled_portals(), Portal::ALL);
        assert_eq!(s.fetch_portals(), [Portal::Freelancermap]);
    }

    /// freelance.de is only readable signed in: without the sign-in switch it gets no
    /// request at all (no session fetch, no sign-in window).
    #[test]
    fn a_portal_behind_a_sign_in_is_fetched_only_with_the_switch() {
        let mut s = Settings::default();
        assert!(!s.fetch_portals().contains(&Portal::FreelanceDe));
        s.portals
            .get_mut(&Portal::FreelanceDe)
            .unwrap()
            .login_enabled = true;
        assert_eq!(s.fetch_portals(), Portal::ALL);
    }
}
