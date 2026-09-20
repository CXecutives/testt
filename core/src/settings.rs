//! Einstellungen der Oberfläche – als JSON im Schlüsselspeicher der Datenbank.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::export::Format;
use crate::mail::scan::Scope;
use crate::portal::Portal;
use crate::store::Store;

const KEY: &str = "settings";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Arbeitsordner; `None` = Standard (`Dokumente\Job-Alert-Monitor`). Nur per Ordnerdialog
    /// änderbar – nie ein vom Frontend geschickter Pfad.
    pub workspace: Option<PathBuf>,
    pub format: Format,
    pub scope: Scope,
    /// Gewählte Portale in Klick-Reihenfolge.
    pub portals: Vec<Portal>,
    /// Der Erststart-Hinweis wurde gesehen.
    pub first_run_seen: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            workspace: None,
            format: Format::Xlsx,
            scope: Scope::New,
            portals: Portal::ALL.to_vec(),
            first_run_seen: false,
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

    /// Jedes Portal höchstens einmal, Reihenfolge wie gewählt.
    fn normalized(mut self) -> Settings {
        let mut seen = Vec::new();
        self.portals.retain(|p| {
            let first = !seen.contains(p);
            seen.push(*p);
            first
        });
        self
    }

    /// Arbeitsordner (gewählt oder Standard).
    pub fn workspace_or(&self, default: &std::path::Path) -> PathBuf {
        self.workspace
            .clone()
            .unwrap_or_else(|| default.to_path_buf())
    }
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
        s.format = Format::Csv;
        s.save(&store).unwrap();
        let back = Settings::load(&store).unwrap();
        assert_eq!(back.portals, [Portal::Freelancermap, Portal::LinkedIn]);
        assert_eq!(back.format, Format::Csv);
        // Unbekannte/fehlende Felder: Voreinstellung je Feld; kaputtes JSON: alles Standard.
        store.kv_set(KEY, r#"{"format":"none"}"#).unwrap();
        assert_eq!(Settings::load(&store).unwrap().scope, Scope::New);
        store.kv_set(KEY, "{kaputt").unwrap();
        assert_eq!(Settings::load(&store).unwrap(), Settings::default());
    }
}
