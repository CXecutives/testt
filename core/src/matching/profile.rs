//! Consultant profile: competences with their JSON paths, the weighted terms of the old
//! engine and the hard criteria.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value};

use super::lexicon;
use super::normalize::{canonical_phrase, canonical_stream, casefold, char_len, strip, tokens};

/// A core competence text of the profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoreText {
    /// Stripped text as written in the profile.
    pub text: String,
    /// JSON path, e.g. `kernkompetenzen[3].kompetenz`.
    pub path: String,
    /// Found by the explicit list pass (known profile structure) rather than by key name.
    pub explicit: bool,
}

/// Profile texts sorted into the old engine's groups.
#[derive(Debug, Default)]
pub(crate) struct Signals {
    pub core: Vec<CoreText>,
    pub experience: Vec<String>,
    pub soft: Vec<String>,
}

/// Python `analyze_profile` (without the criteria).
pub(crate) fn signals(data: &Value) -> Signals {
    let mut raw_core: Vec<CoreText> = Vec::new();
    let mut signals = Signals::default();
    walk(data, None, "", &mut raw_core, &mut signals);

    if let Value::Object(map) = data {
        for (list_key, sub_keys) in lexicon::PROFILE_LISTS {
            let Some(Value::Array(entries)) = map.get(*list_key) else {
                continue;
            };
            for (index, item) in entries.iter().enumerate() {
                let base = format!("{list_key}[{index}]");
                match item {
                    Value::String(text) => raw_core.push(core_text(text, base, true)),
                    Value::Object(fields) if !sub_keys.is_empty() => {
                        for sub in *sub_keys {
                            if let Some(Value::String(text)) = fields.get(*sub) {
                                raw_core.push(core_text(text, format!("{base}.{sub}"), true));
                            }
                        }
                    }
                    Value::Object(fields) => {
                        for (key, value) in fields {
                            if let Value::String(text) = value {
                                raw_core.push(core_text(text, format!("{base}.{key}"), true));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // `dict.fromkeys(text.strip() ...)`: unique stripped texts, first occurrence wins; a
    // text found by both passes counts as explicit.
    for entry in raw_core {
        if entry.text.is_empty() {
            continue;
        }
        match signals.core.iter_mut().find(|c| c.text == entry.text) {
            Some(existing) => existing.explicit |= entry.explicit,
            None => signals.core.push(entry),
        }
    }
    signals
}

fn core_text(text: &str, path: String, explicit: bool) -> CoreText {
    CoreText {
        text: strip(text).to_owned(),
        path,
        explicit,
    }
}

fn walk(
    node: &Value,
    last: Option<&str>,
    path: &str,
    core: &mut Vec<CoreText>,
    signals: &mut Signals,
) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                let key_cf = casefold(key);
                if lexicon::contains(lexicon::PERSONAL_KEYS, &key_cf) {
                    continue;
                }
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                walk(value, Some(&key_cf), &child, core, signals);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                walk(item, last, &format!("{path}[{index}]"), core, signals);
            }
        }
        Value::String(text) => {
            let last = last.unwrap_or("");
            if lexicon::contains(lexicon::PERSONAL_KEYS, last) {
                return;
            }
            if lexicon::contains(lexicon::CORE_KEYS, last)
                || lexicon::contains(lexicon::SKILL_KEYS, last)
            {
                core.push(core_text(text, path.to_owned(), false));
            } else if !strip(text).is_empty() {
                if lexicon::contains(lexicon::SOFT_KEYS, last) {
                    signals.soft.push(text.clone());
                } else {
                    signals.experience.push(text.clone());
                }
            }
        }
        _ => {}
    }
}

/// A weighted profile term of the old engine (`Term`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub display: String,
    pub phrase: bool,
    /// Base weight in halves (3.0 -> 6).
    pub base_halves: u8,
    pub core: bool,
    pub key: Option<String>,
}

impl Term {
    /// Base weight as the old engine printed it.
    pub fn base(&self) -> f64 {
        f64::from(self.base_halves) / 2.0
    }

    /// Canonical form compared with job texts.
    pub fn match_key(&self) -> String {
        self.key
            .clone()
            .unwrap_or_else(|| canonical_phrase(&self.display))
    }
}

/// Python `build_terms`: phrases and single words of the profile in insertion order.
pub(crate) fn build_terms(signals: &Signals) -> Vec<Term> {
    let mut terms: Vec<Term> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut set_default = |terms: &mut Vec<Term>, id: String, term: Term| {
        index.entry(id).or_insert_with(|| {
            terms.push(term);
            terms.len() - 1
        });
    };
    for core in &signals.core {
        let phrase = canonical_phrase(&core.text);
        if !phrase.is_empty() {
            let term = Term {
                display: core.text.clone(),
                phrase: true,
                base_halves: 6,
                core: true,
                key: Some(phrase.clone()),
            };
            set_default(&mut terms, format!("p:{phrase}"), term);
        }
        for token in canonical_stream(tokens(&core.text)) {
            if char_len(&token) >= 3 {
                let term = Term {
                    display: token.clone(),
                    phrase: false,
                    base_halves: 4,
                    core: true,
                    key: None,
                };
                set_default(&mut terms, format!("w:{token}"), term);
            }
        }
    }
    for (texts, cap, halves_per_count) in [(&signals.experience, 3u8, 2u8), (&signals.soft, 2, 1)] {
        let mut counts: Vec<(String, u8)> = Vec::new();
        for text in texts {
            for token in canonical_stream(tokens(text)) {
                match counts.iter_mut().find(|(t, _)| *t == token) {
                    Some((_, count)) => *count = (*count + 1).min(cap),
                    None => counts.push((token, 1)),
                }
            }
        }
        for (token, count) in counts {
            let term = Term {
                display: token.clone(),
                phrase: false,
                base_halves: count * halves_per_count,
                core: false,
                key: None,
            };
            set_default(&mut terms, format!("w:{token}"), term);
        }
    }
    terms
}

/// Hard criteria as the old engine read them (`_parse_criteria`); `None` = inactive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Criteria {
    pub min_day_rate: Option<i128>,
    pub countries: Option<Vec<String>>,
    pub anue_excluded: Option<bool>,
    pub available_now: bool,
    pub remote_outside_allowed: Option<bool>,
}

/// Python `_parse_criteria`.
pub(crate) fn criteria(data: &Value) -> Criteria {
    let empty = Map::new();
    let section = |key: &str| match data.get(key) {
        Some(Value::Object(map)) => map,
        _ => &empty,
    };
    let hard = section(lexicon::KEY_CRITERIA);
    let prefs = section(lexicon::KEY_PREFERENCES);
    let either = |first: &str, second: &str| match hard.get(first) {
        Some(value) if truthy(value) => Some(value),
        _ => prefs.get(second),
    };
    let min_day_rate = either(lexicon::KEY_MIN_RATE, lexicon::KEY_RATE_FROM).and_then(python_int);
    let countries = match hard.get(lexicon::KEY_COUNTRIES) {
        Some(Value::Array(items)) => {
            Some(items.iter().map(|v| python_str(v).to_uppercase()).collect())
        }
        _ => None,
    };
    let anue_excluded = match hard.get(lexicon::KEY_EXCLUDED_CONTRACTS) {
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .any(|v| python_str(v).to_lowercase() == lexicon::CONTRACT_ANUE),
        ),
        _ => None,
    };
    let available_now = matches!(
        either(lexicon::KEY_AVAILABLE, lexicon::KEY_AVAILABLE),
        Some(Value::String(text)) if text.to_lowercase().contains(lexicon::AVAILABLE_NOW)
    );
    let remote_outside_allowed = match hard.get(lexicon::KEY_REMOTE_OUTSIDE) {
        Some(Value::Bool(allowed)) => Some(*allowed),
        _ => None,
    };
    Criteria {
        min_day_rate,
        countries,
        anue_excluded,
        available_now,
        remote_outside_allowed,
    }
}

/// Python truthiness of a JSON value.
pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Python `int(value)` for `isinstance(value, (int, float))` (bool is an int).
fn python_int(value: &Value) -> Option<i128> {
    match value {
        Value::Bool(b) => Some(i128::from(*b)),
        Value::Number(n) => n
            .as_i64()
            .map(i128::from)
            .or_else(|| n.as_u64().map(i128::from))
            .or_else(|| {
                // Truncation toward zero; values beyond i128 cannot come from a profile.
                #[allow(clippy::cast_possible_truncation)]
                n.as_f64().map(|f| f.trunc() as i128)
            }),
        _ => None,
    }
}

/// Python `str(value)` for JSON values.
pub(crate) fn python_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => python_repr(other),
    }
}

/// Python `repr` of a JSON value (non-printable non-ASCII characters are shown as they
/// are, which only matters for absurd profile values).
fn python_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => i.to_string(),
            (None, Some(u), _) => u.to_string(),
            (None, None, Some(f)) => python_float_repr(f),
            _ => n.to_string(),
        },
        Value::String(s) => {
            let quote = if s.contains('\'') && !s.contains('"') {
                '"'
            } else {
                '\''
            };
            let mut out = String::from(quote);
            for c in s.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if c == quote => {
                        out.push('\\');
                        out.push(c);
                    }
                    c if c < ' ' || c == '\x7f' => {
                        let _ = write!(out, "\\x{:02x}", u32::from(c));
                    }
                    c => out.push(c),
                }
            }
            out.push(quote);
            out
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(python_repr).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!(
                    "{}: {}",
                    python_repr(&Value::String(k.clone())),
                    python_repr(v)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Python `repr(float)`: shortest round-trip digits, scientific below 1e-4 and from 1e16.
fn python_float_repr(value: f64) -> String {
    let sci = format!("{value:e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let sign = if negative { "-" } else { "" };
    if (-4..16).contains(&exponent) {
        let point = usize::try_from(exponent + 1).unwrap_or(0);
        if exponent < 0 {
            let zeros = "0".repeat(usize::try_from(-exponent - 1).unwrap_or(0));
            return format!("{sign}0.{zeros}{digits}");
        }
        let int_part = if digits.len() > point {
            digits[..point].to_owned()
        } else {
            format!("{digits:0<point$}")
        };
        let frac = if digits.len() > point {
            &digits[point..]
        } else {
            "0"
        };
        return format!("{sign}{int_part}.{frac}");
    }
    let rest = &digits[1..];
    let mantissa = if rest.is_empty() {
        digits[..1].to_owned()
    } else {
        format!("{}.{rest}", &digits[..1])
    };
    let exp_sign = if exponent < 0 { '-' } else { '+' };
    format!("{sign}{mantissa}e{exp_sign}{:02}", exponent.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn python_str_of_values() {
        assert_eq!(python_str(&json!(1.0)), "1.0");
        assert_eq!(python_str(&json!(1e16)), "1e+16");
        assert_eq!(python_str(&json!(0.00015)), "0.00015");
        assert_eq!(python_str(&json!(1.5e-5)), "1.5e-05");
        assert_eq!(python_str(&json!(123.25)), "123.25");
        assert_eq!(python_str(&json!(null)), "None");
        assert_eq!(python_str(&json!(["de", 1, true])), "['de', 1, True]");
    }

    #[test]
    fn criteria_fallbacks() {
        let c = criteria(&json!({
            "harte_kriterien": {"min_tagessatz": 0, "laender": ["de", "at"], "ausgeschlossene_vertragsarten": ["ANUE"], "verfuegbar_ab": ""},
            "einsatzpraeferenzen": {"tagessatz_ab": 999.9, "verfuegbar_ab": "Sofort verfügbar"}
        }));
        assert_eq!(c.min_day_rate, Some(999));
        assert_eq!(c.countries, Some(vec!["DE".to_owned(), "AT".to_owned()]));
        assert_eq!(c.anue_excluded, Some(true));
        assert!(c.available_now);
        assert_eq!(c.remote_outside_allowed, None);
    }
}
