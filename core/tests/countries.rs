//! The countries a profile can choose are the engine's: the Profil view offers exactly the
//! countries the engine tells apart in a job ad (`jobalert_core::profile::country_codes`),
//! and both UI catalogs name every one of them, so a chip or a violation never shows a bare
//! code for a country the engine knows.

use std::collections::BTreeSet;
use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The two-letter keys of the object literal that opens at `start` (up to its closing
/// brace).
fn keys_after(text: &str, start: &str) -> BTreeSet<String> {
    let at = text
        .find(start)
        .unwrap_or_else(|| panic!("`{start}` not found"));
    let block = &text[at + start.len()..];
    let end = block.find('}').expect("closing brace");
    block[..end]
        .lines()
        .filter_map(|line| {
            let (key, _) = line.trim().split_once(':')?;
            (key.len() == 2 && key.chars().all(|c| c.is_ascii_uppercase())).then(|| key.to_owned())
        })
        .collect()
}

fn engine() -> BTreeSet<String> {
    jobalert_core::profile::country_codes()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_engine_knows_the_common_countries_once_each() {
    let codes = jobalert_core::profile::country_codes();
    for code in ["DE", "AT", "CH", "NL", "GB", "US"] {
        assert!(codes.contains(&code), "{code} missing");
    }
    let mut sorted = codes.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(codes, sorted, "sorted, each code once");
}

#[test]
fn the_german_catalog_names_every_country_of_the_engine() {
    let de = read("ui/src/lib/i18n/de.ts");
    assert_eq!(keys_after(&de, "\n    country: {"), engine());
}

#[test]
fn the_english_catalog_names_every_country_of_the_engine() {
    let en = read("ui/src/lib/i18n/en.ts");
    assert_eq!(
        keys_after(&en, "const countryName: Record<string, string> = {"),
        engine()
    );
}
