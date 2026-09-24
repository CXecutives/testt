//! The one contact-data filter for everything that sends the profile to an AI: the app's
//! prompts and the job-matching skill's brief read the same rules
//! (`tools/job-matching-skill/personal_data.json`) and pass the same cases
//! (`tools/job-matching-skill/tests/personal_cases.json`, checked here and in the skill's
//! Python test).

use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use serde_json::{Map, Value};

/// The rules, shared with the skill.
const RULES_JSON: &str = include_str!("../../../tools/job-matching-skill/personal_data.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rules {
    key_parts: Vec<String>,
    key_tokens: Vec<String>,
    name_keys: Vec<String>,
    personal_sections: Vec<String>,
    prose_parts: Vec<String>,
    value_patterns: Vec<String>,
}

struct Filter {
    rules: Rules,
    patterns: Vec<Regex>,
}

static FILTER: LazyLock<Filter> = LazyLock::new(|| {
    let rules: Rules = serde_json::from_str(RULES_JSON).expect("valid personal_data.json");
    let patterns = rules
        .value_patterns
        .iter()
        .map(|p| Regex::new(p).expect("valid pattern in personal_data.json"))
        .collect();
    Filter { rules, patterns }
});

/// The profile without contact data, the consultant's name and testimonial prose.
pub fn scrub_profile(profile: &Value) -> Value {
    match profile {
        Value::Object(map) => Value::Object(scrub_map(map, Place::Top)),
        other => scrub_value(other, Place::Inside),
    }
}

/// Where a key stands: the name is left out at the top and in a personal section.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Top,
    Personal,
    Inside,
}

/// Lower case, umlauts spelled out, every other character an underscore (runs collapsed).
fn normalise(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for c in key.trim().to_lowercase().chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            c if c.is_ascii_alphanumeric() => out.push(c),
            _ => {
                if !out.is_empty() && !out.ends_with('_') {
                    out.push('_');
                }
            }
        }
    }
    out.trim_end_matches('_').to_owned()
}

fn is_personal_key(key: &str, place: Place) -> bool {
    let rules = &FILTER.rules;
    let key = normalise(key);
    rules
        .key_parts
        .iter()
        .any(|part| key.contains(part.as_str()))
        || key
            .split('_')
            .any(|token| rules.key_tokens.iter().any(|t| t == token))
        || rules
            .prose_parts
            .iter()
            .any(|part| key.contains(part.as_str()))
        || (place != Place::Inside && rules.name_keys.contains(&key))
}

fn is_personal_section(key: &str) -> bool {
    let key = format!("_{}", normalise(key));
    FILTER
        .rules
        .personal_sections
        .iter()
        .any(|section| key.contains(&format!("_{section}")))
}

fn scrub_map(map: &Map<String, Value>, place: Place) -> Map<String, Value> {
    map.iter()
        .filter(|(key, _)| !is_personal_key(key, place))
        .map(|(key, value)| {
            let inner = if place == Place::Personal || is_personal_section(key) {
                Place::Personal
            } else {
                Place::Inside
            };
            (key.clone(), scrub_value(value, inner))
        })
        .collect()
}

fn scrub_value(value: &Value, place: Place) -> Value {
    match value {
        Value::Object(map) => Value::Object(scrub_map(map, place)),
        Value::Array(items) => Value::Array(items.iter().map(|v| scrub_value(v, place)).collect()),
        Value::String(text) => Value::String(scrub_text(text)),
        other => other.clone(),
    }
}

/// A text without mail addresses, links and phone numbers; where something went, the
/// spaces are collapsed.
pub fn scrub_text(text: &str) -> String {
    let mut out = text.to_owned();
    let mut changed = false;
    for pattern in &FILTER.patterns {
        if pattern.is_match(&out) {
            out = pattern.replace_all(&out, "").into_owned();
            changed = true;
        }
    }
    if changed {
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        out.trim().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: &str = include_str!("../../../tools/job-matching-skill/tests/personal_cases.json");

    /// The shared cases: the skill's Python test checks the same file.
    #[test]
    fn the_shared_cases_hold() {
        let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
        assert!(cases.len() >= 2);
        for case in cases {
            assert_eq!(
                scrub_profile(&case["input"]),
                case["expected"],
                "{}",
                case["about"]
            );
        }
    }

    #[test]
    fn keys_are_read_in_every_spelling() {
        assert_eq!(normalise("E-Mail-Adresse"), "e_mail_adresse");
        assert_eq!(normalise(" Geburtsdatum (TT.MM.) "), "geburtsdatum_tt_mm");
        assert_eq!(normalise("Straße"), "strasse");
        for key in [
            "E-Mail-Adresse",
            "emailadresse",
            "Telefonnummer",
            "mobilnummer",
            "Handy",
            "wohnadresse",
            "geburtstag",
            "linkedin_profil",
            "xing-profil",
            "Kontakt",
            "contact_info",
            "links",
            "web",
            "PLZ",
        ] {
            assert!(is_personal_key(key, Place::Inside), "{key}");
        }
        for key in [
            "ausbildung",
            "mobilitaet",
            "einsatzort",
            "harte_kriterien",
            "tagessatz_wunsch",
            "kernkompetenzen",
            "titel",
        ] {
            assert!(!is_personal_key(key, Place::Top), "{key}");
        }
        assert!(is_personal_key("name", Place::Top));
        assert!(is_personal_key("vorname", Place::Personal));
        assert!(
            !is_personal_key("name", Place::Inside),
            "a tool is called name too"
        );
    }

    #[test]
    fn free_text_loses_contact_data_only() {
        assert_eq!(
            scrub_text("Mail an erika@example.org, Tel. +49 171 1234567 oder 040 123456"),
            "Mail an , Tel. oder"
        );
        assert_eq!(
            scrub_text("Profil auf linkedin.com/in/erika und https://example.org/cv"),
            "Profil auf und"
        );
        for kept in [
            "ASP.NET und S/4HANA seit 2019 - 2023",
            "Node.js, 0,5 FTE ab 01.11.2026",
            "Tagessatz 1.200 EUR",
        ] {
            assert_eq!(scrub_text(kept), kept);
        }
    }
}
