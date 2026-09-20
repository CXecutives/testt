//! Einstellungen der Oberfläche – als JSON im Schlüsselspeicher der Datenbank.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::portal::{LoginMode, Portal};
use crate::store::Store;

const KEY: &str = "settings";

/// `#[serde(default)]` je Feld: Eine ältere Datei ohne die heutigen Felder lädt weiter, und
/// Felder von früher (`format`, `scope`, `firstRunSeen`) werden beim Lesen still übergangen –
/// serde lehnt unbekannte Felder nur mit `deny_unknown_fields` ab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Arbeitsordner; `None` = Standard (`Dokumente\Job-Alert-Monitor`). Nur per Ordnerdialog
    /// änderbar – nie ein vom Frontend geschickter Pfad.
    pub workspace: Option<PathBuf>,
    /// Aktiv abgerufene Portale in Klick-Reihenfolge.
    pub portals: Vec<Portal>,
    /// Portale, die über ein Sitzungsfenster abgerufen werden sollen (leer = keins).
    pub session_portals: Vec<Portal>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            workspace: None,
            portals: Portal::ALL.to_vec(),
            session_portals: Vec::new(),
        }
    }
}

impl Settings {
    /// Gespeicherte Einstellungen; unlesbare werden durch die Voreinstellung ersetzt
    /// (sie sind Bequemlichkeit, keine Daten).
    pub fn load(store: &Store) -> Result<Settings> {
        Ok(match store.kv_get(KEY)? {
            Some(json) => serde_json::from_str::<Settings>(&json).map_or_else(
                |e| {
                    log::warn!("Einstellungen unlesbar ({e}) – Voreinstellung");
                    Settings::default()
                },
                Settings::normalized,
            ),
            None => Settings::default(),
        })
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        let json = serde_json::to_string(&self.clone().normalized()).expect("serialisierbar");
        store.kv_set(KEY, &json)
    }

    /// Jedes Portal höchstens einmal, Reihenfolge wie gewählt. Ein Portal ohne Anmeldung
    /// kennt kein Sitzungsfenster und fällt aus `session_portals` heraus – der Abrufweg wird
    /// so nie von einer alten oder verdrehten Einstellung umgelenkt.
    fn normalized(mut self) -> Settings {
        dedup(&mut self.portals);
        dedup(&mut self.session_portals);
        self.session_portals
            .retain(|p| p.login_mode() != LoginMode::None);
        self
    }

    /// Arbeitsordner (gewählt oder Standard).
    pub fn workspace_or(&self, default: &std::path::Path) -> PathBuf {
        self.workspace
            .clone()
            .unwrap_or_else(|| default.to_path_buf())
    }
}

fn dedup(portals: &mut Vec<Portal>) {
    let mut seen = Vec::new();
    portals.retain(|p| {
        let first = !seen.contains(p);
        seen.push(*p);
        first
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_and_repair() {
        let store = Store::in_memory().unwrap();
        assert_eq!(Settings::load(&store).unwrap(), Settings::default());
        let mut s = Settings::load(&store).unwrap();
        s.portals = vec![
            Portal::Freelancermap,
            Portal::LinkedIn,
            Portal::Freelancermap,
        ];
        // LinkedIn kennt kein Sitzungsfenster und fällt heraus.
        s.session_portals = vec![Portal::Freelancermap, Portal::LinkedIn];
        s.save(&store).unwrap();
        let back = Settings::load(&store).unwrap();
        assert_eq!(back.portals, [Portal::Freelancermap, Portal::LinkedIn]);
        assert_eq!(back.session_portals, [Portal::Freelancermap]);
        // Unbekannte/fehlende Felder: Voreinstellung je Feld; kaputtes JSON: alles Standard.
        store.kv_set(KEY, r#"{"portals":[]}"#).unwrap();
        assert!(Settings::load(&store).unwrap().portals.is_empty());
        store.kv_set(KEY, "{kaputt").unwrap();
        assert_eq!(Settings::load(&store).unwrap(), Settings::default());
    }

    /// Eine Datei aus einer früheren Version trägt Felder, die es nicht mehr gibt: Sie wird
    /// gelesen, die Portalwahl bleibt erhalten.
    #[test]
    fn settings_of_an_older_version_still_load() {
        let store = Store::in_memory().unwrap();
        store
            .kv_set(
                KEY,
                r#"{"workspace":null,"format":"xlsx","scope":"week","portals":["linkedin"],"firstRunSeen":true}"#,
            )
            .unwrap();
        let back = Settings::load(&store).unwrap();
        assert_eq!(back.portals, [Portal::LinkedIn]);
        assert_eq!(back.workspace, None);
        assert!(back.session_portals.is_empty());
    }
}
