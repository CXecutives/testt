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

/// The names both catalogs give a country, from the object literal that opens at `start`.
fn names_after(text: &str, start: &str) -> Vec<(String, String)> {
    let at = text
        .find(start)
        .unwrap_or_else(|| panic!("`{start}` not found"));
    let block = &text[at + start.len()..];
    let end = block.find('}').expect("closing brace");
    block[..end]
        .lines()
        .filter_map(|line| {
            let (key, name) = line.trim().split_once(": ")?;
            let name = name.trim_end_matches(',').trim_matches('\'');
            (key.len() == 2).then(|| (key.to_owned(), name.to_owned()))
        })
        .collect()
}

/// A profile may name its countries the way the app names them: every name of both
/// catalogs reads back to its code, so no such name switches the country rule off.
#[test]
fn every_country_name_of_the_app_reads_back_to_its_code() {
    let de = read("ui/src/lib/i18n/de.ts");
    let en = read("ui/src/lib/i18n/en.ts");
    let names = names_after(&de, "\n    country: {")
        .into_iter()
        .chain(names_after(
            &en,
            "const countryName: Record<string, string> = {",
        ));
    let mut seen = 0;
    for (code, name) in names {
        assert_eq!(
            jobalert_core::profile::country_code(&name).as_deref(),
            Some(code.as_str()),
            "{name}"
        );
        seen += 1;
    }
    assert_eq!(seen, 2 * engine().len());
}
