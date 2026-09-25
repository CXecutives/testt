//! Contract type of an ad: interim, permanent, temporary agency work (ANÜ) or unclear.
//! It decides which rules apply: the day rate for interim, salary and region for permanent
//! roles; the location of an interim role is only information.
//!
//! Stated wording first (`Festanstellung`, `Jahresgehalt`, `Werkvertrag`, `Tagessatz`),
//! then indirect hints (benefits, a work-permit question, a career page: permanent), then
//! the portal (freelance portals list projects). A staffing agency without any of it stays
//! unclear and is checked like a permanent role with an ANÜ risk.

use std::ops::Range;

use serde_json::Value;

use super::atoms::fold;
use super::facts::{Finding, JobFacts, Segment, fact, parse_rate};
use super::lexicon::engine as lex;
use crate::portal::Portal;

/// Inferred contract type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractKind {
    Interim,
    Permanent,
    Anue,
    Unclear,
}

impl ContractKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Interim => "interim",
            Self::Permanent => "permanent",
            Self::Anue => "anue",
            Self::Unclear => "unclear",
        }
    }
}

/// The contract type with the cues behind it.
#[derive(Debug, Clone)]
pub(crate) struct Contract {
    pub kind: ContractKind,
    /// Permanent from indirect hints only (rules check instead of deciding).
    pub inferred: bool,
    /// A staffing agency without contract details.
    pub agency: bool,
    /// The ad states a permanent role (also when interim wording contradicts it).
    pub stated_permanent: bool,
    /// Sentences with the deciding cues.
    pub spans: Vec<Range<usize>>,
}

fn any(folded: &str, words: &[&str]) -> bool {
    words.iter().any(|w| folded.contains(w))
}

/// A student or trainee role (`Werkstudent`, `Praktikum` in the title): employment, inferred
/// from the title; only its stated wage decides.
pub(crate) fn student_role(title: &str) -> bool {
    any(&format!("{} ", fold(title)), lex::STUDENT_ROLES)
}

/// Infers the contract type; `anue` holds the decided ANÜ findings of the ad.
pub(crate) fn infer(job: &JobFacts<'_>, segments: &[Segment], anue: &[Finding]) -> Contract {
    let contract_fact = fact(job.facts, super::fact_key::CONTRACT)
        .and_then(Value::as_str)
        .map(fold)
        .unwrap_or_default();
    let title = fold(job.title);
    let interim_at = |f: &str| any(f, lex::INTERIM_CUES) || parse_rate(f).is_some();
    // A denied or merely possible later permanent position is no statement of one.
    let stated_at = |f: &str| {
        (any(f, lex::PERMANENT_WORDS) || any(f, lex::PERMANENT_STATED))
            && !any(f, lex::PERMANENT_NEGATED)
            && !any(f, lex::PERMANENT_OPTION)
    };
    // A contract type field (`Vertragsart: Festanstellung`, the page fact) decides
    // before cues elsewhere (a field label `Honorar:`, `Interim ... denkbar`).
    let field_permanent =
        |f: &str| stated_at(f) && !interim_at(f) && !f.contains(" oder ") && !f.contains(" or ");
    let field = field_permanent(&contract_fact)
        || segments.iter().any(|(_, f)| {
            lex::CONTRACT_LINES.iter().any(|l| f.starts_with(l)) && field_permanent(f)
        });
    let hint_at = |f: &str| any(f, lex::PERMANENT_HINTS);
    let spans_of = |pred: &dyn Fn(&str) -> bool| -> Vec<Range<usize>> {
        segments
            .iter()
            .filter(|(_, f)| pred(f))
            .map(|(r, _)| r.clone())
            .take(2)
            .collect()
    };
    // The page's own field, read by its exact value: LinkedIn's "Befristet" or "Contract"
    // is a limited engagement ("Vollzeit" and "Teilzeit" say nothing about it).
    let limited_fact = lex::LIMITED_CONTRACT_VALUES.contains(&contract_fact.trim());
    let interim = interim_at(&title)
        || interim_at(&contract_fact)
        || limited_fact
        || fact(job.facts, super::fact_key::RATE).is_some()
        || segments.iter().any(|(_, f)| interim_at(f));
    let stated = stated_at(&contract_fact) || segments.iter().any(|(_, f)| stated_at(f));
    let hinted = segments.iter().any(|(_, f)| hint_at(f));
    let agency = segments.iter().any(|(_, f)| any(f, lex::AGENCY_CUES));
    let portal_interim = matches!(job.portal, Portal::Freelancermap | Portal::FreelanceDe);
    let decided_anue = anue.iter().any(|f| f.decided);
    let student = student_role(job.title);
    let contract = |kind, inferred, spans| Contract {
        kind,
        inferred,
        agency: agency && kind == ContractKind::Unclear,
        stated_permanent: stated,
        spans,
    };
    if decided_anue {
        let spans = anue.iter().flat_map(|f| f.spans.clone()).collect();
        return contract(ContractKind::Anue, false, spans);
    }
    if field {
        return contract(ContractKind::Permanent, false, spans_of(&stated_at));
    }
    // A student or trainee role (`Werkstudent`, `Praktikum`) is employment, whatever the
    // hourly wage suggests; inferred from the title, so region and salary are checks.
    if student {
        return contract(ContractKind::Permanent, true, spans_of(&stated_at));
    }
    match (stated, interim) {
        (true, true) => contract(ContractKind::Unclear, false, spans_of(&stated_at)),
        (true, false) => contract(ContractKind::Permanent, false, spans_of(&stated_at)),
        (false, true) => contract(ContractKind::Interim, false, spans_of(&interim_at)),
        (false, false) if hinted && !portal_interim => {
            contract(ContractKind::Permanent, true, spans_of(&hint_at))
        }
        (false, false) if portal_interim => contract(ContractKind::Interim, false, Vec::new()),
        (false, false) => {
            let spans = spans_of(&|f: &str| any(f, lex::AGENCY_CUES));
            contract(ContractKind::Unclear, false, spans)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::facts::{anue, segments};

    fn infer_text(title: &str, text: &str, portal: Portal) -> Contract {
        let job = JobFacts {
            title,
            text,
            location: "",
            portal,
            facts: None,
            posted: None,
        };
        let segments = segments(text);
        let anue = anue(&job, &segments);
        infer(&job, &segments, &anue)
    }

    #[test]
    fn contract_types() {
        use ContractKind::{Anue, Interim, Permanent, Unclear};
        let kind = |title, text, portal| infer_text(title, text, portal).kind;
        let li = Portal::LinkedIn;
        assert_eq!(
            kind(
                "Interim Controller (m/w/d)",
                "Erfahrung im Controlling.",
                li
            ),
            Interim
        );
        assert_eq!(
            kind("Controller", "Tagessatz: 1.000 € pro Tag", li),
            Interim
        );
        assert_eq!(
            kind(
                "Controller",
                "Wir bieten eine unbefristete Festanstellung.",
                li
            ),
            Permanent
        );
        let hinted = infer_text(
            "Finance Manager",
            "Why join us\n30 days of vacation and a company pension scheme",
            li,
        );
        assert_eq!((hinted.kind, hinted.inferred), (Permanent, true));
        let agency = infer_text(
            "Controller",
            "Für unseren Kunden suchen wir einen Controller.",
            li,
        );
        assert_eq!((agency.kind, agency.agency), (Unclear, true));
        assert_eq!(
            kind(
                "Controller",
                "Erfahrung im Controlling.",
                Portal::Freelancermap
            ),
            Interim
        );
        assert_eq!(
            kind(
                "Controller",
                "Die Besetzung erfolgt im Rahmen der Arbeitnehmerüberlassung.",
                li
            ),
            Anue
        );
        let both = infer_text(
            "Controller",
            "Festanstellung oder freiberuflich, Tagessatz nach Absprache.",
            li,
        );
        assert_eq!((both.kind, both.stated_permanent), (Unclear, true));
    }

    /// LinkedIn's employment type, read by its exact value: a limited engagement is
    /// interim, a full-time or part-time or internship value says nothing about it.
    #[test]
    fn the_page_employment_type_by_its_value() {
        use ContractKind::{Interim, Unclear};
        let kind = |value: &str| {
            let facts = serde_json::json!({ "contract": value });
            let job = JobFacts {
                title: "Leiter Controlling (m/w/d)",
                text: "Leitung des Controllings.",
                location: "",
                portal: Portal::LinkedIn,
                facts: Some(&facts),
                posted: None,
            };
            let segments = segments(job.text);
            infer(&job, &segments, &[]).kind
        };
        for limited in ["Befristet", "Contract", "Temporary", "Freiberuflich"] {
            assert_eq!(kind(limited), Interim, "{limited}");
        }
        for neutral in [
            "Vollzeit",
            "Teilzeit",
            "Full-time",
            "Praktikum",
            "Sonstiges",
        ] {
            assert_eq!(kind(neutral), Unclear, "{neutral}");
        }
    }

    /// freelancermap's contract type code, in the page's words: a permanent position or
    /// temporary agency work is seen although the description does not repeat it (the
    /// portal alone used to make every project interim).
    #[test]
    fn freelancermap_contract_types_from_the_page() {
        use ContractKind::{Anue, Interim, Permanent};
        let kind = |value: &str| {
            let facts = serde_json::json!({ "contract": value });
            let job = JobFacts {
                title: "Leitung Controlling (m/w/d)",
                text: "Leitung des Controllings im Mittelstand.",
                location: "",
                portal: Portal::Freelancermap,
                facts: Some(&facts),
                posted: None,
            };
            let segments = segments(job.text);
            let anue = anue(&job, &segments);
            infer(&job, &segments, &anue).kind
        };
        assert_eq!(kind("Festanstellung"), Permanent);
        assert_eq!(kind("Arbeitnehmerüberlassung"), Anue);
        assert_eq!(kind("Freiberuflich"), Interim);
    }

    #[test]
    fn stated_denied_optional_and_field_permanent_roles() {
        use ContractKind::{Interim, Permanent};
        let li = Portal::LinkedIn;
        let fm = Portal::FreelanceDe;
        let kind = |title, text, portal| infer_text(title, text, portal).kind;
        assert_eq!(
            kind("Group Accountant", "A permanent full-time position.", li),
            Permanent
        );
        let denied = infer_text(
            "Interim Controller",
            "Daily rate: EUR 1,100. This is not a permanent position.",
            li,
        );
        assert_eq!((denied.kind, denied.stated_permanent), (Interim, false));
        let option = infer_text(
            "Interim Controller",
            "Tagessatz 1.000 €. Perspektivisch ist eine Übernahme in eine Festanstellung denkbar.",
            fm,
        );
        assert_eq!((option.kind, option.stated_permanent), (Interim, false));
        // The contract field decides over a rate label and interim wording elsewhere.
        assert_eq!(
            kind(
                "Leitung Controlling",
                "Vertragsart: Festanstellung\nHonorar: nach Vereinbarung\nAuch ein Interim Manager ist denkbar.",
                fm
            ),
            Permanent
        );
    }
}
