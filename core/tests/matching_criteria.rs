//! The hard-criteria strip rests on evidence: a criterion is `Ok` only with the ad's value
//! (and the passage that states it), `NotMentioned` when the ad says nothing, `Inactive`
//! when it does not apply to the contract type. The key facts of the ad come with it.

use jobalert_core::matching::{
    Assessment, CriterionKey, CriterionState, CriterionStatus, JobInput, TextKind, assess,
    compile_profile,
};
use jobalert_core::portal::Portal;
use serde_json::{Value, json};

/// An invented interim finance consultant with every freelance criterion set.
fn profile() -> Value {
    json!({
        "kernkompetenzen": [
            {"kompetenz": "Controlling", "jahre": 15},
            {"kompetenz": "Konzernrechnungslegung nach IFRS", "jahre": 12},
            {"kompetenz": "Budgetierung", "jahre": 12}
        ],
        "harte_kriterien": {
            "min_tagessatz": 800,
            "laender": ["DE"],
            "ausgeschlossene_vertragsarten": ["anue"],
            "verfuegbar_ab": "sofort",
            "min_jahresgehalt": 150_000
        }
    })
}

fn run(location: &str, text: &str) -> Assessment {
    let job = JobInput {
        title: "Interim Controller (m/w/d)",
        company: "Muster AG",
        location,
        portal: Portal::LinkedIn,
        text,
        facts: None,
        posted: None,
        kind: TextKind::Full,
    };
    assess(&compile_profile(&profile()), &job, None).expect("assessed")
}

fn criterion(a: &Assessment, key: CriterionKey) -> &CriterionState {
    a.criteria.iter().find(|c| c.key == key).expect("criterion")
}

fn passage(text: &str, state: &CriterionState) -> String {
    let (start, end) = state.range.expect("a passage");
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16(&units[start as usize..end as usize]).expect("utf-16")
}

const STATED: &str =
    "Für unseren Kunden suchen wir einen Interim Controller (m/w/d) auf freiberuflicher Basis.

Ihre Aufgaben:
- Aufbau des Konzernreportings nach IFRS
- Budgetierung und Forecast

Ihr Profil:
- Erfahrung im Controlling
- Mindestens 10 Jahre Berufserfahrung

Rahmendaten:
- Start: ab sofort
- Laufzeit: 6 Monate
- 60 % remote
- Tagessatz: 950 € pro Tag";

#[test]
fn met_only_with_the_ads_value_and_its_passage() {
    let a = run("Hamburg, Deutschland", STATED);
    let rate = criterion(&a, CriterionKey::MinDayRate);
    assert_eq!(rate.status, CriterionStatus::Ok);
    assert_eq!(rate.params["rate"], 950);
    assert!(passage(STATED, rate).contains("950"));
    let country = criterion(&a, CriterionKey::Countries);
    assert_eq!(country.status, CriterionStatus::Ok);
    assert_eq!(country.params["location"], "Hamburg, Deutschland");
    let anue = criterion(&a, CriterionKey::NoAnue);
    assert_eq!(anue.status, CriterionStatus::Ok);
    assert_eq!(anue.params["contract"], "interim");
    let start = criterion(&a, CriterionKey::Availability);
    assert_eq!(start.status, CriterionStatus::Ok);
    assert_eq!(start.params["start"], "now");
    assert!(passage(STATED, start).contains("ab sofort"));
    // A salary is no criterion of a freelance role.
    let salary = criterion(&a, CriterionKey::MinSalary);
    assert_eq!(salary.status, CriterionStatus::Inactive);

    let facts = &a.facts;
    assert_eq!((facts.rate, facts.hourly), (Some(950), Some(false)));
    assert_eq!(facts.start.as_deref(), Some("now"));
    assert_eq!(facts.months, Some(6));
    assert_eq!((facts.remote_from, facts.remote_to), (Some(60), Some(60)));
    assert_eq!(facts.contract.as_deref(), Some("interim"));
    let json = serde_json::to_string(facts).expect("json");
    assert!(json.len() < 160, "{json}");
}

#[test]
fn not_mentioned_when_the_ad_says_nothing() {
    let text = "Wir suchen einen Interim Controller (m/w/d).

Ihr Profil:
- Erfahrung im Controlling
- Konzernrechnungslegung nach IFRS

Der Tagessatz ist nach Absprache.";
    let a = run("", text);
    let rate = criterion(&a, CriterionKey::MinDayRate);
    assert_eq!(rate.status, CriterionStatus::NotMentioned);
    assert_eq!(rate.params["rateOpen"], true);
    assert!(passage(text, rate).contains("nach Absprache"));
    for key in [CriterionKey::Countries, CriterionKey::Availability] {
        assert_eq!(
            criterion(&a, key).status,
            CriterionStatus::NotMentioned,
            "{key:?}"
        );
    }
    // Only the contract type is stated (an interim role): no temporary agency work.
    let ok: Vec<CriterionKey> = a
        .criteria
        .iter()
        .filter(|c| c.status == CriterionStatus::Ok)
        .map(|c| c.key)
        .collect();
    assert_eq!(ok, [CriterionKey::NoAnue]);
    assert_eq!(a.facts.rate_open, Some(true));
    assert_eq!((a.facts.rate, a.facts.start.as_deref()), (None, None));
}

#[test]
fn a_violation_keeps_the_ads_value() {
    let text = STATED.replace("950 €", "700 €");
    let a = run("Hamburg", &text);
    let rate = criterion(&a, CriterionKey::MinDayRate);
    assert_eq!(rate.status, CriterionStatus::Violated);
    assert_eq!(rate.params["rate"], 700);
    assert!(rate.reason.is_some());
}
