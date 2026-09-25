//! Every domain pack of the engine has a name in both UI catalogs: the Profil view names the
//! packs a profile switches on ("Fachwortschatz für Finanzen und SAP"), and a pack without a
//! name would show its raw id.

use std::path::{Path, PathBuf};

fn repo(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The ids of the packs (`name: "..."` of the files in `lexicon/domains`), checked against the
/// number of packs in `DOMAINS`.
fn pack_ids() -> Vec<String> {
    let dir = repo("core/src/matching/lexicon/domains");
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.file_name()? != "mod.rs").then_some(path)
        })
        .filter_map(|path| {
            let text = read(&path);
            let start = text.find("name: \"")? + "name: \"".len();
            let end = start + text[start..].find('"')?;
            Some(text[start..end].to_owned())
        })
        .collect();
    ids.sort();
    let domains = read(&dir.join("mod.rs"));
    let table = &domains[domains.find("pub(crate) const DOMAINS").expect("DOMAINS")..];
    let table = &table[..table.find("];").expect("end of DOMAINS")];
    assert_eq!(
        table.matches("::DOMAIN").count(),
        ids.len(),
        "one pack per file: {ids:?}"
    );
    ids
}

/// The keys of the `pack: {` table of a UI catalog.
fn labels(catalog: &str) -> Vec<String> {
    let text = read(&repo(catalog));
    let mut keys = Vec::new();
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if !inside {
            inside = line == "pack: {";
            continue;
        }
        if line.starts_with('}') {
            break;
        }
        if let Some((key, value)) = line.split_once(':')
            && value.trim().trim_end_matches(',').len() > 2
        {
            keys.push(key.trim().to_owned());
        }
    }
    keys.sort();
    keys
}

#[test]
fn every_pack_has_a_name_in_both_catalogs() {
    let ids = pack_ids();
    assert!(ids.len() >= 11, "{ids:?}");
    for catalog in ["ui/src/lib/i18n/de.ts", "ui/src/lib/i18n/en.ts"] {
        assert_eq!(labels(catalog), ids, "{catalog}");
    }
}
