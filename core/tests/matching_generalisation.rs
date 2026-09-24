//! Rules found on the unseen held-out set 3, checked on invented ads: they must hold for
//! any ad and any profile, not for the ads they were found on.

use jobalert_core::matching::{
    Assessment, JobInput, ReasonCode, ReasonKind, TextKind, Verdict, assess, compile_profile,
};
use jobalert_core::portal::Portal;
use serde_json::{Value, json};

/// An introduction that makes every test ad long enough to be read.
const INTRO: &str = "Die Muster AG ist ein mittelständisches Unternehmen mit Sitz in Köln und \
                     rund 800 Mitarbeitenden.\n\n";

fn run(profile: &Value, title: &str, text: &str) -> Assessment {
    let text = &format!("{INTRO}{text}");
    let job = JobInput {
        title,
        company: "Muster AG",
        location: "Köln, Deutschland",
        portal: Portal::LinkedIn,
        text,
        facts: None,
        posted: None,
        kind: TextKind::Full,
    };
    assess(&compile_profile(profile), &job, None).expect("assessed")
}

fn finance() -> Value {
    json!({
        "kernkompetenzen": [
            {"kompetenz": "Controlling", "jahre": 15},
            {"kompetenz": "CFO", "jahre": 8},
            {"kompetenz": "Konzernrechnungslegung nach IFRS", "jahre": 12},
            {"kompetenz": "Treasury", "jahre": 10}
        ],
        "sprachen": [
            {"sprache": "Dutch", "niveau": "Native"},
            {"sprache": "English", "niveau": "C2"}
        ],
        "harte_kriterien": {
            "zielprofil_min_jahre": 10,
            "min_jahresgehalt": 95000,
            "festanstellung_orte": ["München"]
        }
    })
}

fn met(a: &Assessment, label: &str) -> bool {
    a.reasons
        .iter()
        .any(|r| r.label.as_deref() == Some(label) && r.kind == ReasonKind::Met)
}

/// Languages under their English names meet the ad's languages.
#[test]
fn languages_under_english_names() {
    let a = run(
        &finance(),
        "Controller (m/w/d)",
        "Ihr Profil\n- Erfahrung im Controlling\n- Fluent Dutch\n- Business fluent English\n",
    );
    assert!(met(&a, "Fluent Dutch"), "{:#?}", a.reasons);
    assert!(met(&a, "Business fluent English"), "{:#?}", a.reasons);
}

/// A text long enough to read but without any requirement is judged from its title: low
/// evidence and at most 60, higher for the profile whose field the title names.
#[test]
fn a_text_without_requirements_is_judged_from_its_title() {
    let text = "Die Muster AG sucht für die Übergangszeit bis zur Nachbesetzung eine erfahrene \
                Persönlichkeit. Vor Ort in Köln, befristet für sechs Monate.";
    let fin = run(&finance(), "Interim CFO (m/w/d)", text);
    assert_eq!(fin.verdict, Verdict::Scored);
    assert!(fin.score <= 60, "{}", fin.score);
    assert!(
        fin.reasons
            .iter()
            .any(|r| r.code == ReasonCode::LowEvidence),
        "{:#?}",
        fin.reasons
    );
    let other = json!({"kernkompetenzen": [
        {"kompetenz": "Recruiting"}, {"kompetenz": "Arbeitsrecht"}, {"kompetenz": "Payroll"}
    ]});
    let hr = run(&other, "Interim CFO (m/w/d)", text);
    assert!(hr.score < fin.score, "{} < {}", hr.score, fin.score);
}

/// The seniority of an ad does not depend on the profile's packs: `Mindestens 5 Jahre` of
/// a pharma role (`QA` in the title is quality assurance for every profile) is too junior
/// for a finance profile that asks for 10.
#[test]
fn seniority_is_the_same_for_every_profile() {
    let a = run(
        &finance(),
        "QA Manager Sterile (m/w/d)",
        "Ihr Profil\n- Mindestens 5 Jahre Erfahrung in der Qualitätssicherung\n\
         - CAPA und Change Control\n- Freiberuflich, Tagessatz 900 €\n",
    );
    assert!(
        a.reasons
            .iter()
            .any(|r| r.code == ReasonCode::TooJunior && r.kind == ReasonKind::Violation),
        "{:#?}",
        a.reasons
    );
}

/// A student role is employment: its stated hourly wage decides the salary criterion, its
/// place is only a check (the role is inferred from the title).
#[test]
fn a_student_role_is_employment_and_its_wage_decides() {
    let a = run(
        &finance(),
        "Werkstudent (m/w/d) Controlling",
        "Dein Profil\n- Studium der Wirtschaftswissenschaften\n- Erste Erfahrung im Controlling\n\
         Das bieten wir\n- 16,50 € pro Stunde\n",
    );
    assert_eq!(a.verdict, Verdict::Excluded);
    let codes: Vec<(ReasonCode, ReasonKind)> = a.reasons.iter().map(|r| (r.code, r.kind)).collect();
    assert!(
        codes.contains(&(ReasonCode::Salary, ReasonKind::Violation)),
        "{codes:?}"
    );
    assert!(
        !codes.contains(&(ReasonCode::PermanentRegion, ReasonKind::Violation)),
        "{codes:?}"
    );
}

/// Equal scores keep an order: the score before the caps.
#[test]
fn the_rank_orders_capped_scores() {
    let weak = run(
        &finance(),
        "Controller (m/w/d)",
        "Ihr Profil\n- Erfahrung im Controlling\n- Python\n- Tableau\n- Snowflake\n",
    );
    let weaker = run(
        &finance(),
        "Data Engineer (m/w/d)",
        "Ihr Profil\n- Kubernetes\n- Python\n- Tableau\n- Snowflake\n",
    );
    assert!(weak.rank > weaker.rank, "{} > {}", weak.rank, weaker.rank);
    assert!(u32::from(weak.rank) >= u32::from(weak.score) * 10 - 5);
}
