//! An AI's answer to the CV prompt ([`super::prompt`]), read as robustly as chat windows and
//! AIs make it necessary: the profile JSON wherever it stands (a code block, between
//! sentences, after an echoed skeleton), repaired where it is no valid JSON (typographic
//! quotes as delimiters, trailing commas, comments, non-breaking spaces, a byte order mark),
//! then brought into the skeleton's shape: keys under another name (English, with umlauts,
//! in capitals) become the skeleton's keys, a number written as text becomes a number, one
//! text where a list belongs becomes a list, a profile wrapped in one more object is
//! unwrapped, keys outside the skeleton go and empty values are dropped. A chosen file is
//! never reshaped: only an answer is a new profile in the app's own format.
//!
//! external contract - do not translate: the profile JSON keys.

use super::form;
use super::json::Json;
use crate::error::InvalidInput;
use crate::matching::lexicon::{self, engine as lex};

/// How a value of the skeleton is read from an answer.
#[derive(Clone, Copy)]
enum Shape {
    /// A text (a number becomes its text).
    Text,
    /// A whole number (`12`, `20+`, `1.200 €` as text; a fraction is cut).
    Number,
    /// A list of texts; with `split` one text becomes its parts at `,` and `;`.
    Texts { split: bool },
    /// A list of objects with these fields (a text becomes an object with the first one).
    Objects(&'static [Field]),
    /// An object with these fields.
    Section(&'static [Field]),
    /// Kept as it is (the engine reads several forms).
    Any,
}

/// A key of the skeleton: its name first, then the names an AI gives it instead, all
/// written the way [`key`] folds them.
struct Field {
    keys: &'static [&'static str],
    shape: Shape,
}

const TEXTS: Shape = Shape::Texts { split: true };

const DEGREE: &[Field] = &[Field {
    keys: &["abschluss", "degree", "name", "title", "titel"],
    shape: Shape::Text,
}];
const COMPETENCE: &[Field] = &[
    Field {
        keys: &["kompetenz", "competence", "competency", "skill", "name"],
        shape: Shape::Text,
    },
    Field {
        keys: &["jahre", "years", "erfahrung_jahre"],
        shape: Shape::Number,
    },
    Field {
        keys: &["auch", "aliases", "also", "synonyms", "synonyme"],
        shape: TEXTS,
    },
];
const TOOL: &[Field] = &[Field {
    keys: &["name", "tool", "method", "methode", "kompetenz"],
    shape: Shape::Text,
}];
const CERTIFICATE: &[Field] = &[Field {
    keys: &[
        "name",
        "zertifizierung",
        "certification",
        "certificate",
        "zertifikat",
    ],
    shape: Shape::Text,
}];
const INDUSTRY: &[Field] = &[Field {
    keys: &["branche", "industry", "name"],
    shape: Shape::Text,
}];
const LANGUAGE: &[Field] = &[
    Field {
        keys: &["sprache", "language", "name"],
        shape: Shape::Text,
    },
    Field {
        keys: &["niveau", "level"],
        shape: Shape::Text,
    },
];
const STATION: &[Field] = &[
    Field {
        keys: &["zeitraum", "period", "timeframe", "dates", "time", "dauer"],
        shape: Shape::Text,
    },
    Field {
        keys: &["rolle", "role", "position", "title", "titel", "funktion"],
        shape: Shape::Text,
    },
    Field {
        keys: &["schwerpunkte", "focus", "focus_areas", "topics", "themen"],
        shape: TEXTS,
    },
];
const WISHES: &[Field] = &[
    Field {
        keys: &[
            "tagessatz_wunsch",
            "desired_day_rate",
            "day_rate",
            "tagessatz",
        ],
        shape: Shape::Number,
    },
    Field {
        keys: &["remote"],
        shape: Shape::Text,
    },
    Field {
        keys: &["regionen", "regions"],
        shape: TEXTS,
    },
    Field {
        keys: &["branchen", "industries"],
        shape: TEXTS,
    },
    Field {
        keys: lexicon::KEYS_AVAILABLE,
        shape: Shape::Text,
    },
];
/// Every criterion the engine reads, under its German and English names.
const CRITERIA: &[Field] = &[
    Field {
        keys: lexicon::KEYS_MIN_RATE,
        shape: Shape::Number,
    },
    Field {
        keys: lexicon::KEYS_COUNTRIES,
        shape: TEXTS,
    },
    Field {
        keys: lexicon::KEYS_EXCLUDED_CONTRACTS,
        shape: TEXTS,
    },
    Field {
        keys: lexicon::KEYS_AVAILABLE,
        shape: Shape::Text,
    },
    Field {
        keys: lexicon::KEYS_REMOTE_OUTSIDE,
        shape: Shape::Any,
    },
    Field {
        keys: lexicon::KEYS_TARGET_YEARS,
        shape: Shape::Number,
    },
    Field {
        keys: lexicon::KEYS_MIN_SALARY,
        shape: Shape::Number,
    },
    Field {
        keys: lexicon::KEYS_PERMANENT_PLACES,
        shape: TEXTS,
    },
    Field {
        keys: lexicon::KEYS_PERMANENT_REMOTE,
        shape: Shape::Number,
    },
];
/// The skeleton's keys in its order.
const TOP: &[Field] = &[
    Field {
        keys: &["name"],
        shape: Shape::Text,
    },
    Field {
        keys: &["titel", "title", "rolle", "role"],
        shape: Shape::Text,
    },
    Field {
        keys: &["wunschrollen", "target_roles", "desired_roles", "roles"],
        shape: TEXTS,
    },
    Field {
        keys: &[
            "berufserfahrung_jahre",
            "years_of_experience",
            "total_years",
            "experience_years",
            "professional_experience_years",
        ],
        shape: Shape::Number,
    },
    Field {
        keys: &["ausbildung", "abschluss", "education", "degrees", "degree"],
        shape: Shape::Objects(DEGREE),
    },
    Field {
        keys: &[
            "kernkompetenzen",
            "core_competencies",
            "core_competences",
            "competencies",
            "competences",
            "kompetenzen",
            "skills",
        ],
        shape: Shape::Objects(COMPETENCE),
    },
    Field {
        keys: &["schwerpunkte", "focus_areas", "focus"],
        shape: TEXTS,
    },
    Field {
        keys: &[
            "methoden_tools",
            "methods_tools",
            "methods_and_tools",
            "tools_and_methods",
            "tools",
            "methoden",
        ],
        shape: Shape::Objects(TOOL),
    },
    Field {
        keys: &[
            "zertifizierungen",
            "certifications",
            "certificates",
            "zertifikate",
        ],
        shape: Shape::Objects(CERTIFICATE),
    },
    Field {
        keys: &["branchen", "industries"],
        shape: Shape::Objects(INDUSTRY),
    },
    Field {
        keys: &["sprachen", "languages"],
        shape: Shape::Objects(LANGUAGE),
    },
    Field {
        keys: &[
            "alleinstellungsmerkmale",
            "unique_selling_points",
            "usps",
            "key_strengths",
            "strengths",
        ],
        shape: Shape::Texts { split: false },
    },
    Field {
        keys: &["keywords", "stichworte", "schlagworte"],
        shape: TEXTS,
    },
    Field {
        keys: &[
            lex::KEY_STATIONS,
            "stations",
            "career_stations",
            "career",
            "positions",
            "work_experience",
        ],
        shape: Shape::Objects(STATION),
    },
    Field {
        keys: lexicon::KEY_PREFERENCES_ALIASES,
        shape: Shape::Section(WISHES),
    },
    Field {
        keys: lexicon::KEY_CRITERIA_ALIASES,
        shape: Shape::Section(CRITERIA),
    },
];

/// Characters that are never part of a profile: a byte order mark and the zero-width ones.
const INVISIBLE: [char; 5] = ['\u{feff}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}'];
/// Typographic double quotes an AI or a chat window puts where JSON has `"`.
const QUOTES: [char; 6] = [
    '\u{201c}', '\u{201d}', '\u{201e}', '\u{201f}', '\u{2033}', '\u{ff02}',
];

/// The profile of an answer: the first place that parses (as it is, else repaired) and holds
/// something the form can show, in the skeleton's shape, with its source text (the answer's
/// own JSON when nothing had to change, else the reshaped one). An answer whose JSON breaks
/// off says so.
pub(super) fn read(answer: &str) -> Result<(Json, String), InvalidInput> {
    let text: String = answer.chars().filter(|c| !INVISIBLE.contains(c)).collect();
    for candidate in candidates(&text) {
        let (parsed, repaired) = match serde_json::from_str::<Json>(candidate) {
            Ok(doc) => (doc, false),
            Err(_) => match serde_json::from_str::<Json>(&repair(candidate)) {
                Ok(doc) => (doc, true),
                Err(_) => continue,
            },
        };
        if !parsed.is_object() {
            continue;
        }
        let doc = shape(&parsed);
        if !form::read(&doc).has_content() {
            continue;
        }
        let source = if !repaired && doc == parsed {
            candidate.to_owned()
        } else {
            doc.to_pretty().trim_end().to_owned()
        };
        return Ok((doc, source));
    }
    Err(if objects(&text).1 {
        InvalidInput::ProfileAnswerCut
    } else {
        InvalidInput::ProfileAnswer
    })
}

// ------------------------------------------------------------------------------ finding

/// Where a JSON object may stand: every fenced code block (three backticks or three tildes,
/// without its language tag), every object of the text on its own, then the span from the
/// first `{` to the last `}`; each once.
fn candidates(text: &str) -> Vec<&str> {
    let mut spans: Vec<&str> = Vec::new();
    for fence in ["```", "~~~"] {
        let mut rest = text;
        while let Some(start) = rest.find(fence) {
            let opened = &rest[start + fence.len()..];
            let body = &opened[opened.find('\n').map_or(opened.len(), |n| n + 1)..];
            let Some(end) = body.find(fence) else {
                spans.push(body);
                break;
            };
            spans.push(&body[..end]);
            rest = &body[end + fence.len()..];
        }
    }
    spans.extend(objects(text).0);
    if let (Some(open), Some(close)) = (text.find('{'), text.rfind('}'))
        && open < close
    {
        spans.push(&text[open..=close]);
    }
    let mut out: Vec<&str> = Vec::new();
    for span in spans.into_iter().map(str::trim) {
        if span.contains('{') && !out.contains(&span) {
            out.push(span);
        }
    }
    out
}

/// The objects of a text from each `{` outside an object to its closing `}` (strings are
/// skipped), and whether one of them never closes (the answer breaks off).
fn objects(text: &str) -> (Vec<&str>, bool) {
    let mut found = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    let (mut in_string, mut escaped) = (false, false);
    for (at, c) in text.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' if depth > 0 => in_string = true,
            '{' => {
                if depth == 0 {
                    start = at;
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    found.push(&text[start..=at]);
                }
            }
            _ => {}
        }
    }
    (found, depth > 0)
}

// ------------------------------------------------------------------------------ repairing

/// Where the scanner of [`repair`] is.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Outside,
    /// In a string opened by `"`: everything is text up to the next unescaped `"`.
    Straight,
    /// In a string opened by a typographic quote: it ends at a quote followed by what ends a
    /// JSON string (`:`, `,`, `}`, `]` or the end); a quote inside stays text (`„Lean“`).
    Typographic,
}

/// The candidate as valid JSON where AIs and chat windows spoil it: typographic quotes as
/// delimiters become `"`, `//` and `/* */` comments go, other whitespace outside strings
/// becomes a space, control characters inside strings are escaped, and then a comma before a
/// closing bracket goes. Text inside strings stays as it is.
fn repair(candidate: &str) -> String {
    let chars: Vec<char> = candidate.chars().collect();
    let mut out = String::with_capacity(candidate.len());
    let mut place = Place::Outside;
    let mut at = 0;
    let next_solid = |from: usize| chars[from..].iter().copied().find(|c| !c.is_whitespace());
    while at < chars.len() {
        let c = chars[at];
        at += 1;
        match place {
            Place::Outside => match c {
                '"' => {
                    place = Place::Straight;
                    out.push('"');
                }
                c if QUOTES.contains(&c) => {
                    place = Place::Typographic;
                    out.push('"');
                }
                '/' if chars.get(at) == Some(&'/') => {
                    while at < chars.len() && chars[at] != '\n' {
                        at += 1;
                    }
                }
                '/' if chars.get(at) == Some(&'*') => {
                    at += 1;
                    while at < chars.len() && !(chars[at - 1] == '*' && chars[at] == '/') {
                        at += 1;
                    }
                    at += 1;
                }
                c if c.is_whitespace() => out.push(if c == '\n' { '\n' } else { ' ' }),
                c => out.push(c),
            },
            Place::Straight | Place::Typographic => match c {
                '\\' => {
                    out.push('\\');
                    if let Some(&escaped) = chars.get(at) {
                        out.push(escaped);
                        at += 1;
                    }
                }
                '"' if place == Place::Straight => {
                    place = Place::Outside;
                    out.push('"');
                }
                c if place == Place::Typographic
                    && (c == '"' || QUOTES.contains(&c))
                    && matches!(next_solid(at), None | Some(':' | ',' | '}' | ']')) =>
                {
                    place = Place::Outside;
                    out.push('"');
                }
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => out.push(' '),
                c => out.push(c),
            },
        }
    }
    without_trailing_commas(&out)
}

/// Valid JSON but for commas before a closing bracket: without them.
fn without_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut in_string, mut escaped) = (false, false);
    for (at, &c) in chars.iter().enumerate() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if c == '"' {
            in_string = true;
        } else if c == ','
            && matches!(
                chars[at + 1..].iter().find(|c| !c.is_whitespace()),
                Some('}' | ']')
            )
        {
            continue;
        }
        out.push(c);
    }
    out
}

// ------------------------------------------------------------------------------ shaping

/// A key as the tables write it: trimmed, lower case, umlauts as two letters, spaces and
/// hyphens as `_` (`Kern-Kompetenzen` is `kern_kompetenzen`, `Länder` is `laender`).
fn key(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .replace('\u{e4}', "ae")
        .replace('\u{f6}', "oe")
        .replace('\u{fc}', "ue")
        .replace('\u{df}', "ss")
        .replace([' ', '-'], "_")
}

/// The values of `object` under any of `keys`, in the order they stand.
fn values<'a>(object: &'a Json, keys: &[&str]) -> Vec<&'a Json> {
    match object {
        Json::Object(entries) => entries
            .iter()
            .filter(|(k, _)| keys.contains(&key(k).as_str()))
            .map(|(_, value)| value)
            .collect(),
        _ => Vec::new(),
    }
}

/// The answer in the skeleton's shape, its empty values dropped.
fn shape(doc: &Json) -> Json {
    let mut doc = section(unwrap(doc), TOP);
    drop_empty(&mut doc);
    doc
}

/// A profile wrapped in one or two objects of one key (`{"profil": {...}}`) is that profile.
fn unwrap(doc: &Json) -> &Json {
    let known = |doc: &Json| TOP.iter().any(|field| !values(doc, field.keys).is_empty());
    let mut doc = doc;
    for _ in 0..2 {
        match doc {
            Json::Object(entries)
                if !known(doc) && entries.len() == 1 && entries[0].1.is_object() =>
            {
                doc = &entries[0].1;
            }
            _ => break,
        }
    }
    doc
}

/// The fields of an object in the order of `fields`, each in its shape; other keys go. A
/// list or a section gathers what stands under every key it goes by, a single value takes
/// the first.
fn section(object: &Json, fields: &[Field]) -> Json {
    let mut out = Vec::new();
    for field in fields {
        let found = values(object, field.keys);
        let value = match field.shape {
            Shape::Text => found.into_iter().find_map(text),
            Shape::Number => found.into_iter().find_map(number),
            Shape::Any => found.into_iter().next().cloned(),
            Shape::Texts { split } => Some(Json::Array(
                found.into_iter().flat_map(|v| texts(v, split)).collect(),
            )),
            Shape::Objects(fields) => Some(Json::Array(
                found.into_iter().flat_map(|v| entries(v, fields)).collect(),
            )),
            Shape::Section(fields) => {
                let entries: Vec<(String, Json)> = found
                    .into_iter()
                    .filter_map(|v| match v {
                        Json::Object(entries) => Some(entries.iter().cloned()),
                        _ => None,
                    })
                    .flatten()
                    .collect();
                Some(section(&Json::Object(entries), fields))
            }
        };
        if let Some(value) = value {
            out.push((field.keys[0].to_owned(), value));
        }
    }
    Json::Object(out)
}

fn text(value: &Json) -> Option<Json> {
    match value {
        Json::String(text) => Some(Json::text(text.trim())),
        Json::Number(n) => Some(Json::String(n.to_string())),
        _ => None,
    }
}

/// The texts of a list (an object gives the text the engine reads in it); one text is a list
/// of itself, or with `split` of its parts.
fn texts(value: &Json, split: bool) -> Vec<Json> {
    match value {
        Json::String(one) if split => one
            .split([',', ';'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(Json::text)
            .collect(),
        Json::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Json::Object(_) => lexicon::KEYS_ITEM_TEXT
                    .iter()
                    .find_map(|k| item.get(k)?.as_str())
                    .map(Json::text),
                other => text(other),
            })
            .collect(),
        other => text(other).into_iter().collect(),
    }
}

/// The entries of a list of objects; a text is an entry with the first field.
fn entries(value: &Json, fields: &[Field]) -> Vec<Json> {
    let items: Vec<&Json> = match value {
        Json::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    items
        .into_iter()
        .filter_map(|item| match item {
            Json::Object(_) => Some(section(item, fields)),
            other => text(other).map(|t| Json::Object(vec![(fields[0].keys[0].to_owned(), t)])),
        })
        .collect()
}

/// A whole number from a number (a fraction is cut) or a text such as `12`, `20+`,
/// `1.200 €` or `7,5 Jahre`; `None` for anything else (`ca. 10`, `nach Absprache`, `-3`).
fn number(value: &Json) -> Option<Json> {
    let n = match value {
        Json::Number(n) => n.as_u64().or_else(|| {
            let f = n.as_f64().filter(|f| *f >= 0.0)?;
            // A profile number is small; the fraction is cut.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(f.trunc() as u64)
        }),
        Json::String(text) => whole(text),
        _ => None,
    }?;
    Some(Json::Number(n.into()))
}

/// Marks inside a number written as text: thousands (`1.200`, `1 200`) or a fraction (`7,5`).
const MARKS: [char; 6] = ['.', ',', '\'', ' ', '\u{a0}', '\u{202f}'];

fn whole(text: &str) -> Option<u64> {
    let text = text.trim();
    let end = text
        .find(|c: char| !c.is_ascii_digit() && !MARKS.contains(&c))
        .unwrap_or(text.len());
    let (digits, unit) = text.split_at(end);
    // After the number only a plus or a unit (`Jahre`, `€ pro Tag`), no second number.
    if unit.chars().any(|c| c.is_ascii_digit() || c == '-') {
        return None;
    }
    let digits = digits.trim_end_matches(MARKS);
    let parts: Vec<&str> = digits.split(MARKS).collect();
    match parts.as_slice() {
        [single] => single.parse().ok(),
        [head, tail] if (1..=2).contains(&tail.len()) => head.parse().ok(),
        [head, groups @ ..]
            if (1..=3).contains(&head.len()) && groups.iter().all(|g| g.len() == 3) =>
        {
            parts.concat().parse().ok()
        }
        // Thousands and a fraction (`1.200,50`): the fraction is cut.
        [head, groups @ .., tail]
            if (1..=3).contains(&head.len())
                && groups.iter().all(|g| g.len() == 3)
                && (1..=2).contains(&tail.len()) =>
        {
            format!("{head}{}", groups.concat()).parse().ok()
        }
        _ => None,
    }
}

/// Removes empty values from objects and lists, deepest first (a list of empty objects goes
/// as a whole); `true` if anything went.
pub(super) fn drop_empty(value: &mut Json) -> bool {
    let empty = |value: &Json| match value {
        Json::Null => true,
        Json::String(text) => text.trim().is_empty(),
        Json::Array(items) => items.is_empty(),
        Json::Object(entries) => entries.is_empty(),
        Json::Bool(_) | Json::Number(_) => false,
    };
    let mut dropped = false;
    match value {
        Json::Object(entries) => {
            for (_, child) in entries.iter_mut() {
                dropped |= drop_empty(child);
            }
            let before = entries.len();
            entries.retain(|(_, child)| !empty(child));
            dropped |= entries.len() != before;
        }
        Json::Array(items) => {
            for child in items.iter_mut() {
                dropped |= drop_empty(child);
            }
            let before = items.len();
            items.retain(|child| !empty(child));
            dropped |= items.len() != before;
        }
        _ => {}
    }
    dropped
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const JSON: &str =
        "{\"name\": \"Erika\", \"kernkompetenzen\": [{\"kompetenz\": \"Controlling\"}]}";

    /// The profile an answer gives, and its source text.
    fn profile(answer: &str) -> (Value, String) {
        let (doc, source) = read(answer).unwrap_or_else(|e| panic!("{e:?}: {answer}"));
        (doc.to_value(), source)
    }

    fn json_of(text: &str) -> Json {
        serde_json::from_str(text).unwrap()
    }

    /// The skeleton's JSON is found wherever an AI puts it; valid JSON keeps its text.
    #[test]
    fn the_profile_is_found_between_prose_and_other_json() {
        let skeleton = super::super::prompt::SKELETON;
        for answer in [
            JSON.to_owned(),
            format!("```json\n{JSON}\n```"),
            format!("~~~json\n{JSON}\n~~~"),
            format!("\u{feff}Hier ist das Profil.\n\n```\n{JSON}\n```\n\nViel Erfolg."),
            format!("Gern, hier ist es: {JSON} Sag Bescheid."),
            // Braces in the prose around it.
            format!("Ich habe {{Platzhalter}} ersetzt.\n{JSON}\nFrag, wenn {{etwas}} fehlt."),
            // The skeleton echoed first, then the answer, without code blocks.
            format!("Der Aufbau war\n{skeleton}\nund das Profil ist\n{JSON}"),
            format!("```json\n{{kaputt\n```\nOder so\n```json\n{JSON}\n```"),
        ] {
            let (value, source) = profile(&answer);
            assert_eq!(
                value["kernkompetenzen"][0]["kompetenz"], "Controlling",
                "{answer}"
            );
            assert_eq!(source, JSON, "{answer}");
        }
    }

    /// What chat windows and AIs spoil is repaired; text inside the values stays as it is.
    #[test]
    fn broken_json_is_repaired() {
        let lean = "Einführung \u{201e}Lean\u{201c} im Werk";
        for answer in [
            // Typographic quotes as delimiters, German quotes inside a value.
            "{\u{201c}name\u{201d}: \u{201c}Erika\u{201d}, \u{201c}kernkompetenzen\u{201d}: \
             [{\u{201c}kompetenz\u{201d}: \u{201c}Einführung \u{201e}Lean\u{201c} im Werk\u{201d}}]}",
            // German quotes around the keys, straight ones around the values.
            "{\u{201e}name\u{201c}: \"Erika\", \u{201e}kernkompetenzen\u{201c}: \
             [{\"kompetenz\": \"Einführung \u{201e}Lean\u{201c} im Werk\"}]}",
            // Trailing commas and comments.
            "{\n  \"name\": \"Erika\", // wie im Lebenslauf\n  \"kernkompetenzen\": [\n    \
             {\"kompetenz\": \"Einführung \u{201e}Lean\u{201c} im Werk\",},\n  ],\n  \
             /* Rest leer */\n}",
            // Non-breaking spaces and zero-width characters between the tokens.
            "```json\n\u{feff}{\u{a0}\"name\":\u{a0}\"Erika\",\u{200b}\n\u{202f}\
             \"kernkompetenzen\": [\"Einführung \u{201e}Lean\u{201c} im Werk\"]}\n```",
        ] {
            let (value, source) = profile(answer);
            assert_eq!(value["name"], "Erika", "{answer}");
            assert_eq!(
                value["kernkompetenzen"],
                json!([{ "kompetenz": lean }]),
                "{answer}"
            );
            // The source is valid JSON for the save.
            assert_eq!(serde_json::from_str::<Value>(&source).unwrap(), value);
        }
        // A line break inside a value; a link is no comment.
        let (value, _) = profile(
            "{\"name\": \"Erika\",\n\"keywords\": [\"https://example.invalid/a\",],\n\
             \"alleinstellungsmerkmale\": [\"Zeile eins\nZeile zwei\"]}",
        );
        assert_eq!(value["keywords"], json!(["https://example.invalid/a"]));
        assert_eq!(
            value["alleinstellungsmerkmale"],
            json!(["Zeile eins\nZeile zwei"])
        );
    }

    /// Keys under another name become the skeleton's, in its order; keys outside it go.
    #[test]
    fn other_keys_become_the_skeletons() {
        let answer = json!({
            "Profile": {
                "Name": "Erika Beispiel",
                "email": "erika@example.invalid",
                "Title": "Interim CFO",
                "core competencies": [
                    {
                        "skill": "Controlling",
                        "years": "12",
                        "aliases": "FP&A, Financial Controlling",
                        "note": "x"
                    },
                    "Treasury"
                ],
                "Education": [{"degree": "Diplom-Kauffrau (Univ.)", "school": "Uni"}],
                "abschluss": "MBA",
                "tools": ["SAP S/4HANA FI", {"name": "Power BI"}],
                "certifications": "PMP",
                "industries": [{"industry": "Chemie"}],
                "Languages": [{"language": "Englisch", "level": "C1"}],
                "strengths": "Schnell, gründlich und ruhig.",
                "Hinweis": "bleibt weg",
                "stations": [{
                    "period": "01/2020 bis heute",
                    "role": "CFO",
                    "topics": "Treasury; Controlling",
                    "company": "Beispiel AG"
                }],
                "focus_areas": ["Controlling"],
                "target_roles": "Interim CFO, Head of Controlling",
                "years_of_experience": 20.7,
                "preferences": {"desired_day_rate": "1.200 €", "Regions": "Hamburg; Berlin", "extra": 1},
                "hard_criteria": {"min_day_rate": "1000", "countries": "DE, AT", "eigene_regel": true},
                "Harte Kriterien": {"Länder": ["CH"]}
            }
        })
        .to_string();
        let (value, _) = profile(&answer);
        assert_eq!(
            value,
            json!({
                "name": "Erika Beispiel",
                "titel": "Interim CFO",
                "wunschrollen": ["Interim CFO", "Head of Controlling"],
                "berufserfahrung_jahre": 20,
                "ausbildung": [{"abschluss": "Diplom-Kauffrau (Univ.)"}, {"abschluss": "MBA"}],
                "kernkompetenzen": [
                    {"kompetenz": "Controlling", "jahre": 12, "auch": ["FP&A", "Financial Controlling"]},
                    {"kompetenz": "Treasury"}
                ],
                "schwerpunkte": ["Controlling"],
                "methoden_tools": [{"name": "SAP S/4HANA FI"}, {"name": "Power BI"}],
                "zertifizierungen": [{"name": "PMP"}],
                "branchen": [{"branche": "Chemie"}],
                "sprachen": [{"sprache": "Englisch", "niveau": "C1"}],
                "alleinstellungsmerkmale": ["Schnell, gründlich und ruhig."],
                "stationen": [{
                    "zeitraum": "01/2020 bis heute",
                    "rolle": "CFO",
                    "schwerpunkte": ["Treasury", "Controlling"]
                }],
                "einsatzpraeferenzen": {"tagessatz_wunsch": 1200, "regionen": ["Hamburg", "Berlin"]},
                // Two sections of criteria are one (`json!` sorts the keys).
                "harte_kriterien": {"min_tagessatz": 1000, "laender": ["CH", "DE", "AT"]}
            })
        );
        // The form reads all of it.
        let (doc, _) = read(&answer).unwrap();
        let form = form::read(&doc);
        assert_eq!(
            form.competences[0].aliases,
            ["FP&A", "Financial Controlling"]
        );
        assert_eq!(form.criteria.countries, ["CH", "DE", "AT"]);
        assert_eq!(form.wishes.day_rate, Some(1200));
    }

    #[test]
    fn numbers_written_as_text() {
        for (text, expected) in [
            ("12", Some(12)),
            (" 20+ ", Some(20)),
            ("20+ Jahre", Some(20)),
            ("1.200", Some(1200)),
            ("1 200 €", Some(1200)),
            ("1.200,50 € pro Tag", Some(1200)),
            ("100.000", Some(100_000)),
            ("7,5 Jahre", Some(7)),
            ("12.5", Some(12)),
            ("ca. 10", None),
            ("-3", None),
            ("10-15", None),
            ("12 (seit 2014)", None),
            ("nach Absprache", None),
            ("", None),
        ] {
            assert_eq!(whole(text), expected, "{text}");
        }
        assert_eq!(number(&json_of("7.9")), Some(Json::number(7)));
        assert_eq!(number(&json_of("-2")), None);
        assert_eq!(number(&json_of("true")), None);
    }

    /// An answer without a profile says so; one whose JSON breaks off says that instead.
    #[test]
    fn an_answer_without_a_whole_profile_says_why() {
        for answer in [
            "",
            "Das kann ich nicht.",
            "```json\n{\"foo\": 1}\n```",
            "[1, 2]",
            "{\"name\": }",
            "Ersetze {x} und {y}.",
        ] {
            assert_eq!(
                read(answer).err(),
                Some(InvalidInput::ProfileAnswer),
                "{answer}"
            );
        }
        let cut = &JSON[..JSON.len() - 10];
        for answer in [
            cut.to_owned(),
            format!("```json\n{cut}"),
            format!("Hier ist es.\n```json\n{cut}\n```"),
            format!("{}\n{cut}", super::super::prompt::SKELETON),
        ] {
            assert_eq!(
                read(&answer).err(),
                Some(InvalidInput::ProfileAnswerCut),
                "{answer}"
            );
        }
    }
}
