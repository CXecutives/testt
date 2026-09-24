//! Does the profile meet one requirement item? (old engine: strict phrase matching)
//!
//! Only complete profile phrases count, never single words from free text:
//! (i) the phrase occurs contiguously in the item's core tokens;
//! (ii) a one-token item equals a one-token phrase or extends it by at most two
//!      characters (`KI-Implementierungen` ~ `KI-Implementierung`);
//! (iii) a phrase of at least three tokens has two thirds of its tokens in the item
//!      (worth half, but it still counts as met).

use super::normalize::char_len;
use super::requirements::core_tokens;

/// A profile phrase in canonical form.
#[derive(Debug, Clone)]
pub(crate) struct Phrase {
    pub key: String,
    pub tokens: Vec<String>,
}

impl Phrase {
    pub(crate) fn new(key: String) -> Self {
        let tokens = key.split_whitespace().map(str::to_owned).collect();
        Self { key, tokens }
    }
}

/// How a phrase met an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    /// Rule (i) or an equal one-token item.
    Exact,
    /// Rule (ii) with a suffix of one or two characters.
    Stem,
    /// Rule (iii): two thirds of the phrase tokens (value one half).
    TwoThirds,
}

/// One phrase that meets an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hit {
    pub phrase: usize,
    pub rule: Rule,
}

/// Python `_match_requirement`.
pub(crate) fn match_item(item: &str, phrases: &[Phrase]) -> Vec<Hit> {
    let kern = core_tokens(item);
    match_tokens(&kern, phrases)
}

/// The matching rules on precomputed core tokens.
pub(crate) fn match_tokens(kern: &[String], phrases: &[Phrase]) -> Vec<Hit> {
    let mut hits = Vec::new();
    if kern.is_empty() {
        return hits;
    }
    let short = kern.len() == 1;
    let kern_text = format!(" {} ", kern.join(" "));
    for (index, phrase) in phrases.iter().enumerate() {
        if phrase.tokens.is_empty() {
            continue;
        }
        if kern_text.contains(&format!(" {} ", phrase.key)) {
            hits.push(Hit {
                phrase: index,
                rule: Rule::Exact,
            });
            continue;
        }
        if short && phrase.tokens.len() == 1 {
            let single = &phrase.tokens[0];
            let token = &kern[0];
            if token == single {
                hits.push(Hit {
                    phrase: index,
                    rule: Rule::Exact,
                });
            } else if char_len(single) >= 3
                && token.starts_with(single.as_str())
                && char_len(token) - char_len(single) <= 2
            {
                hits.push(Hit {
                    phrase: index,
                    rule: Rule::Stem,
                });
            }
            continue;
        }
        if phrase.tokens.len() >= 3 {
            let present = phrase.tokens.iter().filter(|t| kern.contains(t)).count();
            // Python compared `present / n >= 2 / 3` in floats; for these small
            // integers that is exactly `3 * present >= 2 * n`.
            if 3 * present >= 2 * phrase.tokens.len() {
                hits.push(Hit {
                    phrase: index,
                    rule: Rule::TwoThirds,
                });
            }
        }
    }
    hits
}
