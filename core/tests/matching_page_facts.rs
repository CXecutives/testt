//! A page's structured criteria (LinkedIn's criteria list: career level, employment type,
//! industries) reach the engine: an internship or entry-level role is too junior for a
//! senior target, an associate level is a check, a limited employment type makes the ad an
//! interim one, and the stated industries answer the industry wish first.

use jobalert_core::matching::{
    Assessment, JobInput, ReasonCode, ReasonKind, TextKind, Verdict, assess, compile_profile,
};
use jobalert_core::portal::Portal;
use serde_json::{Value, json};

/// An invented interim finance consultant with a senior target and an industry wish.
fn profile() -> Value {
    json!({
        "kernkompetenzen": [
            {"kompetenz": "Controlling", "jahre": 15},
            {"kompetenz": "Konzernrechnungslegung nach IFRS", "jahre": 12},
            {"kompetenz": "Budgetierung", "jahre": 12},
            {"kompetenz": "Forecast", "jahre": 12},
            {"kompetenz": "Reporting", "jahre": 12}
        ],
        "harte_kriterien": {
            "zielprofil_min_jahre": 10
        },
        "einsatzpraeferenzen": {
            "branchen": ["Maschinenbau"]
        }
    })
}

const TEXT: &str = "Über uns\n\
Ein Unternehmen mit Sitz in Köln.\n\n\
Ihre Aufgaben\n\
- Aufbau des Konzernreportings nach IFRS\n\
- Budgetierung und Forecast\n\n\
Ihr Profil\n\
- Erfahrung im Controlling\n\
- Erfahrung in der Konzernrechnungslegung nach IFRS\n";

fn run(facts: &Value) -> Assessment {
    let job = JobInput {
        title: "Controller (m/w/d)",
        company: "Musterwerke GmbH",
        location: "Köln, Deutschland",
        portal: Portal::LinkedIn,
        text: TEXT,
        facts: Some(facts),
        posted: None,
        kind: TextKind::Full,
    };
    assess(&compile_profile(&profile()), &job, None).expect("assessed")
}

fn has(a: &Assessment, code: ReasonCode, kind: ReasonKind) -> bool {
    a.reasons.iter().any(|r| r.code == code && r.kind == kind)
}

#[test]
fn an_internship_or_entry_level_is_too_junior() {
    for facts in [
        json!({ "level": "Praktikum" }),
        json!({ "level": "Berufseinstieg" }),
        json!({ "level": "Entry level" }),
        json!({ "contract": "Praktikum" }),
        json!({ "contract": "Internship" }),
        json!({ "contract": "Ehrenamtlich" }),
    ] {
        let a = run(&facts);
        assert_eq!(a.verdict, Verdict::Excluded, "{facts}");
        assert!(
            has(&a, ReasonCode::TooJunior, ReasonKind::Violation),
            "{facts}"
        );
    }
}

#[test]
fn an_associate_level_is_a_check_and_a_senior_one_nothing() {
    let a = run(&json!({ "level": "Associate" }));
    assert_ne!(a.verdict, Verdict::Excluded);
    assert!(has(&a, ReasonCode::SeniorityUnclear, ReasonKind::Check));
    for level in ["Direktor", "Mid-Senior level", "Geschäftsführung"] {
        let a = run(&json!({ "level": level }));
        assert!(
            a.reasons
                .iter()
                .all(|r| r.code != ReasonCode::TooJunior && r.code != ReasonCode::SeniorityUnclear),
            "{level}"
        );
    }
}

#[test]
fn a_limited_employment_type_is_interim() {
    assert_eq!(
        run(&json!({ "contract": "Befristet" }))
            .facts
            .contract
            .as_deref(),
        Some("interim")
    );
    assert_ne!(
        run(&json!({ "contract": "Vollzeit" }))
            .facts
            .contract
            .as_deref(),
        Some("interim")
    );
}

#[test]
fn the_stated_industries_answer_the_industry_wish() {
    let state = |facts: &Value| {
        run(facts)
            .reasons
            .iter()
            .find(|r| r.code == ReasonCode::IndustryWish)
            .and_then(|r| r.params.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("none")
            .to_owned()
    };
    assert_eq!(state(&json!({})), "unknown");
    assert_eq!(
        state(&json!({ "industries": "Maschinenbau und Metallverarbeitung" })),
        "met"
    );
}
