//! Target roles (`wunschrollen`, `ENGINE_VERSION` 4): job titles the consultant wants. Off
//! while the key is missing.
//!
//! Role and title are compared with the title-fit machinery (folding, stems, concepts,
//! compounds) after a few role paraphrases (`Chief Financial Officer` = `CFO`,
//! `Leiter Finanzen` = `Head of Finance`). A role is its topic atoms plus a lead flag
//! (`Head`, `Leiter`, `Leitung`, `Director`, ...) and contract words (`Interim`):
//! - full: every topic atom is in the title (equal), a lead role meets a lead title, the
//!   contract words are in the title or the contract is interim, and the title is no
//!   junior title;
//! - half: every topic atom is in the title, but one of the other conditions fails or an
//!   atom only fits as a narrower or wider compound;
//! - none otherwise, and always when a role of generic words only (`Head of Finance`)
//!   meets a title that names a specific topic of its own (`S/4HANA Finance Lead`).
//!
//! The best role adds `ROLE_FULL` or `ROLE_HALF` to the shrunk fit before the caps, so it
//! never lifts an off-field ad over its cap and never touches an exclusion.

use serde_json::Value;

use super::atoms::{self, Fit, Vocab, fold, raw_tokens};
use super::contract::ContractKind;
use super::focus::texts;
use super::job::contains_word;
use super::lexicon::{self, wishes as lex};
use super::seniority::junior_title;

/// A target role as the engine compares it.
#[derive(Debug, Clone)]
pub(crate) struct Role {
    /// As written in the profile.
    pub text: String,
    /// JSON path, e.g. `wunschrollen[0]`.
    pub path: String,
    topic: Vec<String>,
    contract: Vec<String>,
    lead: bool,
    /// At least one topic atom is specific (not generic).
    specific: bool,
}

/// The parts of a role or a title.
struct Parts {
    topic: Vec<String>,
    contract: Vec<String>,
    lead: bool,
}

/// Folds a role or title and applies the role paraphrases (whole words).
fn normalise(text: &str) -> String {
    let mut folded = fold(text);
    for (phrase, replacement) in lex::ROLE_PHRASES {
        if contains_word(&folded, phrase) {
            folded = replace_words(&folded, phrase, replacement);
        }
    }
    folded
}

/// Replaces whole-word occurrences of `phrase`.
fn replace_words(text: &str, phrase: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(phrase) {
        let end = at + phrase.len();
        let before = rest[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after = rest[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        out.push_str(&rest[..at]);
        out.push_str(if before && after { replacement } else { phrase });
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn parts(text: &str, vocab: &Vocab) -> Parts {
    let folded = normalise(text);
    let tokens: Vec<&str> = raw_tokens(&folded).collect();
    let lead = tokens
        .iter()
        .any(|t| lex::LEAD_WORDS.contains(t) || lex::LEAD_ENDINGS.iter().any(|e| t.ends_with(e)));
    let contract: Vec<String> = tokens
        .iter()
        .filter(|t| lex::ROLE_CONTRACT_WORDS.contains(t))
        .map(|t| (*t).to_owned())
        .collect();
    let rest: Vec<&str> = tokens
        .iter()
        .filter(|t| {
            !lex::LEAD_WORDS.contains(t)
                && !lex::LEVEL_WORDS.contains(t)
                && !lex::ROLE_CONTRACT_WORDS.contains(t)
                && !lexicon::engine::JUNIOR_TITLES.contains(t)
        })
        .copied()
        .collect();
    Parts {
        topic: atoms::atoms(&rest.join(" "), vocab),
        contract,
        lead,
    }
}

/// The target roles of a profile and the key with its value when none can be read.
pub(crate) fn read(data: &Value, vocab: &Vocab) -> (Vec<Role>, Vec<(&'static str, String)>) {
    let Some((key, value)) = lexicon::KEYS_TARGET_ROLES
        .iter()
        .find_map(|key| data.get(*key).filter(|v| !v.is_null()).map(|v| (*key, v)))
    else {
        return (Vec::new(), Vec::new());
    };
    let list = texts(value).unwrap_or_default();
    if list.is_empty() {
        return (Vec::new(), vec![(key, value.to_string())]);
    }
    let mut roles = Vec::new();
    let mut unreadable = Vec::new();
    for (i, text) in list.into_iter().enumerate() {
        let p = parts(&text, vocab);
        if p.topic.is_empty() {
            unreadable.push((key, text));
            continue;
        }
        roles.push(Role {
            specific: p.topic.iter().any(|a| !atoms::is_generic(a)),
            path: format!("{key}[{i}]"),
            text,
            topic: p.topic,
            contract: p.contract,
            lead: p.lead,
        });
    }
    (roles, unreadable)
}

/// The best target role for a title: its index and whether it fits in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoleFit {
    pub role: usize,
    pub full: bool,
}

/// How one role fits a title: `None`, `Some(false)` half, `Some(true)` full.
fn role_fit(role: &Role, title: &Parts, junior: bool, interim: bool) -> Option<bool> {
    let mut full = true;
    for atom in &role.topic {
        let best = title
            .topic
            .iter()
            .map(|t| atoms::fit(t, atom))
            .max()
            .unwrap_or(Fit::None);
        match best {
            Fit::Equal => {}
            Fit::General | Fit::Specific => full = false,
            Fit::None => return None,
        }
    }
    if role
        .contract
        .iter()
        .any(|c| !interim && !title.contract.contains(c))
    {
        full = false;
    }
    if (role.lead && !title.lead) || junior {
        full = false;
    }
    // A role of generic words only (`Head of Finance`) is another role when the title
    // names a specific topic of its own (`S/4HANA Finance Lead`, `FP&A`).
    let extra = title.topic.iter().any(|t| {
        !atoms::is_generic(t) && !role.topic.iter().any(|r| atoms::fit(t, r) != Fit::None)
    });
    if !role.specific && extra {
        return None;
    }
    Some(full)
}

/// The topic atoms of every target role (what a title may name).
pub(crate) fn topics(roles: &[Role]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for role in roles {
        for atom in &role.topic {
            if !atoms::is_generic(atom) && !out.contains(atom) {
                out.push(atom.clone());
            }
        }
    }
    out
}

/// The best target role for a job title (full before half, profile order on a tie).
pub(crate) fn best(
    roles: &[Role],
    title: &str,
    vocab: &Vocab,
    contract: ContractKind,
) -> Option<RoleFit> {
    if roles.is_empty() {
        return None;
    }
    let parts = parts(title, vocab);
    let junior = junior_title(title);
    let interim = contract == ContractKind::Interim;
    let fits: Vec<(usize, bool)> = roles
        .iter()
        .enumerate()
        .filter_map(|(i, role)| role_fit(role, &parts, junior, interim).map(|full| (i, full)))
        .collect();
    fits.iter()
        .find(|(_, full)| *full)
        .or_else(|| fits.first())
        .map(|&(role, full)| RoleFit { role, full })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fit(roles: &[&str], title: &str, contract: ContractKind) -> Option<(String, bool)> {
        let vocab = Vocab::all();
        let (roles, unreadable) = read(&json!({ "wunschrollen": roles }), &vocab);
        assert!(unreadable.is_empty(), "{unreadable:?}");
        best(&roles, title, &vocab, contract).map(|f| (roles[f.role].text.clone(), f.full))
    }

    #[test]
    fn titles_against_target_roles() {
        use ContractKind::{Interim, Permanent};
        let roles = [
            "Interim CFO",
            "Head of Finance",
            "Leitung Konzernrechnungswesen",
        ];
        let full = |title, contract| fit(&roles, title, contract).filter(|(_, full)| *full);
        let half = |title, contract| fit(&roles, title, contract).filter(|(_, full)| !*full);
        assert_eq!(
            full("Interim CFO (m/w/d)", Interim).unwrap().0,
            "Interim CFO"
        );
        // A freelance portal's CFO project is interim without the word.
        assert!(full("CFO (m/w/d)", Interim).is_some());
        // A permanent CFO is half of an interim CFO.
        assert!(half("Chief Financial Officer (m/w/d)", Permanent).is_some());
        assert_eq!(
            full("Head of Finance (m/w/d)", Permanent).unwrap().0,
            "Head of Finance"
        );
        assert!(full("Leiter Finanzen (m/w/d)", Permanent).is_some());
        assert!(full("Leitung Konzernrechnungswesen (m/w/d)", Permanent).is_some());
        // The field without the lead level, or a junior title.
        assert!(half("Projekt im Konzernrechnungswesen", Interim).is_some());
        assert!(half("Interim Finance Manager", Interim).is_some());
        assert!(half("Junior Head of Finance", Permanent).is_some());
        // A generic role is another role when the title names a specific topic.
        let other = |title| fit(&roles, title, Interim);
        assert_eq!(other("Interim SAP S/4HANA Finance Lead (Contract)"), None);
        assert_eq!(other("Director Finance Transformation (m/w/d)"), None);
        assert_eq!(
            fit(&roles, "Head of Group Controlling (m/w/d)", Permanent),
            None
        );
        assert_eq!(
            fit(&roles, "Leiter Konzerncontrolling (Interim)", Interim),
            None
        );
        assert_eq!(fit(&roles, "SAP MM Berater (m/w/d)", Interim), None);
    }

    #[test]
    fn specific_roles_allow_more_words_in_the_title() {
        let roles = ["SAP FI/CO Berater", "S/4HANA Finance Lead"];
        let full = |title| {
            fit(&roles, title, ContractKind::Interim)
                .filter(|(_, full)| *full)
                .map(|(role, _)| role)
        };
        assert_eq!(
            full("SAP FI/CO Berater (m/w/d) für S/4HANA-Migration").as_deref(),
            Some("SAP FI/CO Berater")
        );
        assert_eq!(
            full("Interim SAP S/4HANA Finance Lead (Contract)").as_deref(),
            Some("S/4HANA Finance Lead")
        );
        assert_eq!(
            fit(
                &roles,
                "SAP SD Inhouse-Berater (m/w/d)",
                ContractKind::Interim
            ),
            None
        );
    }

    #[test]
    fn unreadable_roles_are_reported() {
        let vocab = Vocab::all();
        let (roles, unreadable) = read(&json!({ "wunschrollen": ["Head of", "CFO"] }), &vocab);
        assert_eq!(roles.len(), 1);
        assert_eq!(unreadable, [("wunschrollen", "Head of".to_owned())]);
        let (roles, unreadable) = read(&json!({ "target_roles": "CFO" }), &vocab);
        assert!(roles.is_empty());
        assert_eq!(unreadable[0].0, "target_roles");
        assert!(read(&json!({}), &vocab).0.is_empty());
    }
}
