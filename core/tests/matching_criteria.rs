//! The hard-criteria strip rests on evidence: a criterion is `Ok` only with the ad's value
//! (and the passage that states it), `NotMentioned` when the ad says nothing, `Inactive`
//! when it does not apply to the contract type. The key facts of the ad come with it.

use jobalert_core::matching::{
    Assessment, CompiledProfile, CriterionKey, CriterionState, CriterionStatus, JobInput,
    ReasonCode, ReasonKind, TextKind, Verdict, assess, compile_profile,
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

const PERMANENT: &str = "Wir suchen einen Leiter Controlling (m/w/d) für unsere Holding.

Ihre Aufgaben:
- Aufbau des Konzernreportings nach IFRS
- Budgetierung und Forecast

Ihr Profil:
- Erfahrung im Controlling
- Konzernrechnungslegung nach IFRS

Wir bieten eine unbefristete Festanstellung in Vollzeit.";

/// The profile with these excluded contract types.
fn excluding(kinds: &Value) -> CompiledProfile {
    let mut value = profile();
    value["harte_kriterien"]["ausgeschlossene_vertragsarten"] = kinds.clone();
    compile_profile(&value)
}

fn assess_with(profile: &CompiledProfile, title: &str, text: &str) -> Assessment {
    let job = JobInput {
        title,
        company: "Muster AG",
        location: "Hamburg",
        portal: Portal::LinkedIn,
        text,
        facts: None,
        posted: None,
        kind: TextKind::Full,
    };
    assess(profile, &job, None).expect("assessed")
}

/// Permanent employment as an excluded contract type (`festanstellung`, also `permanent`):
/// a stated permanent role is excluded with the sentence that states it; one inferred from
/// benefits or one that offers freelance work too is a check; an interim role meets the
/// criterion with the ad's contract type. Without the value the permanent role is the
/// check it always was.
#[test]
fn permanent_employment_can_be_excluded() {
    for kinds in [
        json!(["anue", "festanstellung"]),
        json!(["Festanstellung"]),
        json!("permanent"),
    ] {
        let profile = excluding(&kinds);
        let a = assess_with(&profile, "Leiter Controlling (m/w/d)", PERMANENT);
        assert_eq!(a.verdict, Verdict::Excluded, "{kinds}");
        let state = criterion(&a, CriterionKey::NoPermanent);
        assert_eq!(state.status, CriterionStatus::Violated, "{kinds}");
        let reason = a
            .reasons
            .iter()
            .find(|r| r.code == ReasonCode::Permanent)
            .expect("the permanent reason");
        assert_eq!(reason.kind, ReasonKind::Violation);
        assert_eq!(reason.params["stated"], true);
        assert!(
            a.highlights.iter().any(|h| h.reason == reason.id),
            "the stating sentence is marked"
        );
    }
    let excluded = excluding(&json!(["festanstellung"]));
    // Inferred from benefits, or offered next to freelance work: a check only.
    let stated = "Wir bieten eine unbefristete Festanstellung in Vollzeit.";
    let hinted = PERMANENT.replace(
        stated,
        "Why join us\n30 days of vacation and a company pension scheme",
    );
    let both = PERMANENT.replace(
        stated,
        "Festanstellung oder freiberuflich, Tagessatz nach Absprache.",
    );
    for text in [hinted.as_str(), both.as_str()] {
        let a = assess_with(&excluded, "Leiter Controlling (m/w/d)", text);
        assert_ne!(a.verdict, Verdict::Excluded, "{text}");
        assert_eq!(
            criterion(&a, CriterionKey::NoPermanent).status,
            CriterionStatus::Check,
            "{text}"
        );
    }
    // An interim role meets it with the ad's contract type.
    let a = assess_with(&excluded, "Interim Controller (m/w/d)", STATED);
    let state = criterion(&a, CriterionKey::NoPermanent);
    assert_eq!(state.status, CriterionStatus::Ok);
    assert_eq!(state.params["contract"], "interim");
    // Without the value: inactive, and the permanent role is the check it was.
    let a = assess_with(
        &compile_profile(&profile()),
        "Leiter Controlling (m/w/d)",
        PERMANENT,
    );
    assert_ne!(a.verdict, Verdict::Excluded);
    assert_eq!(
        criterion(&a, CriterionKey::NoPermanent).status,
        CriterionStatus::Inactive
    );
    let check = a
        .reasons
        .iter()
        .find(|r| r.code == ReasonCode::Permanent)
        .expect("the permanent check");
    assert_eq!(check.kind, ReasonKind::Check);
    assert!(check.params.is_empty());
    // The profile summary names the criterion, and ANÜ only where the profile says so.
    let set: Vec<CriterionKey> = excluded
        .summary()
        .criteria
        .iter()
        .filter(|c| c.set)
        .map(|c| c.key)
        .collect();
    assert!(set.contains(&CriterionKey::NoPermanent), "{set:?}");
    assert!(!set.contains(&CriterionKey::NoAnue), "{set:?}");
}

/// A short requirement line that starts like a heading of the other listings ("Ähnliche
/// Projekterfahrung von Vorteil") is part of the ad: the hard criteria below it still count,
/// as a plain line and as a list item of a page. A real heading still ends the ad.
#[test]
fn a_requirement_that_starts_like_other_listings_keeps_the_ad_whole() {
    let frame = "Rahmendaten:\n- Einsatz über Arbeitnehmerüberlassung\n- Tagessatz: 600 €";
    for line in [
        "Ähnliche Projekterfahrung von Vorteil",
        "- Weitere Projekterfahrung wünschenswert",
    ] {
        let text = format!("Ihr Profil:\n- Erfahrung im Controlling\n{line}\n\n{frame}");
        let a = run("Hamburg", &text);
        assert_eq!(a.verdict, Verdict::Excluded, "{line}");
        assert_eq!(
            criterion(&a, CriterionKey::NoAnue).status,
            CriterionStatus::Violated
        );
        assert_eq!(
            criterion(&a, CriterionKey::MinDayRate).status,
            CriterionStatus::Violated
        );
    }
    let listings = format!(
        "Ihr Profil:\n- Erfahrung im Controlling\n- Tagessatz: 950 €\n\nÄhnliche Projekte (12)\n{frame}"
    );
    let a = run("Hamburg", &listings);
    assert_ne!(a.verdict, Verdict::Excluded);
}

/// A profile that names its countries (`["Deutschland", "Österreich"]`) keeps the country
/// rule: a job in Hamburg passes, one in Paris is excluded.
#[test]
fn country_names_in_a_profile_keep_the_country_rule() {
    let mut value = profile();
    value["harte_kriterien"]["laender"] = json!(["Deutschland", "Österreich"]);
    let named = compile_profile(&value);
    let at = |location: &str| {
        let job = JobInput {
            title: "Interim Controller (m/w/d)",
            company: "Muster AG",
            location,
            portal: Portal::LinkedIn,
            text: STATED,
            facts: None,
            posted: None,
            kind: TextKind::Full,
        };
        assess(&named, &job, None).expect("assessed")
    };
    assert_eq!(
        criterion(&at("Hamburg"), CriterionKey::Countries).status,
        CriterionStatus::Ok
    );
    let paris = at("Paris, Frankreich");
    assert_eq!(paris.verdict, Verdict::Excluded);
    assert_eq!(
        criterion(&paris, CriterionKey::Countries).status,
        CriterionStatus::Violated
    );
}
