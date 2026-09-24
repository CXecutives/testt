//! The version-4 inputs of the profile, one test per feature: Schwerpunkte (`schwerpunkte`),
//! target roles (`wunschrollen`) and the wishes of `einsatzpraeferenzen` (day rate, remote,
//! regions, industries). Each is off without its key, visible as a reason with code and
//! params, bounded, and never an exclusion.

use std::path::Path;

use jobalert_core::matching::{
    Assessment, JobInput, ProfileWarningCode, Reason, ReasonCode, ReasonKind, TextKind, Verdict,
    WishKey, assess, compile_profile,
};
use jobalert_core::portal::Portal;
use serde_json::{Value, json};

/// An invented interim finance consultant without any version-4 key.
fn base() -> Value {
    json!({
        "kernkompetenzen": [
            {"kompetenz": "Controlling", "jahre": 15},
            {"kompetenz": "Konzernrechnungslegung nach IFRS", "jahre": 12},
            {"kompetenz": "Treasury", "jahre": 10, "auch": ["Cash Management"]},
            {"kompetenz": "Restrukturierung", "jahre": 8},
            {"kompetenz": "Budgetierung", "jahre": 12},
            {"kompetenz": "Forecast", "jahre": 12}
        ],
        "harte_kriterien": {
            "min_tagessatz": 800,
            "laender": ["DE"],
            "ausgeschlossene_vertragsarten": ["anue"]
        }
    })
}

/// The base profile with extra top-level keys (`einsatzpraeferenzen` merged).
fn with(extra: &Value) -> Value {
    let mut profile = base();
    for (key, value) in extra.as_object().expect("object") {
        profile[key] = value.clone();
    }
    profile
}

struct Ad<'a> {
    title: &'a str,
    company: &'a str,
    location: &'a str,
    portal: Portal,
    text: &'a str,
}

const AD: Ad<'static> = Ad {
    title: "Interim Controller (m/w/d)",
    company: "Muster AG",
    location: "Hamburg, Deutschland",
    portal: Portal::LinkedIn,
    text: "",
};

fn run(profile: &Value, ad: &Ad<'_>) -> Assessment {
    let job = JobInput {
        title: ad.title,
        company: ad.company,
        location: ad.location,
        portal: ad.portal,
        text: ad.text,
        facts: None,
        posted: None,
        kind: TextKind::Full,
    };
    assess(&compile_profile(profile), &job, None).expect("assessed")
}

fn reason(a: &Assessment, code: ReasonCode) -> Option<&Reason> {
    a.reasons.iter().find(|r| r.code == code)
}

fn state(a: &Assessment, code: ReasonCode) -> String {
    reason(a, code)
        .and_then(|r| r.params.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("none")
        .to_owned()
}

/// Two musts met, one open: a mid score with room up and down.
const TREASURY_TEXT: &str = "Über uns\n\
Ein Maschinenbauer aus dem Allgäu.\n\n\
Ihr Profil\n\
- Erfahrung im Treasury\n\
- Erfahrung im Controlling\n\
- Kenntnisse in der Zollabwicklung\n\n\
Rahmenbedingungen\n\
Tagessatz 1.100 € pro Tag\n";

const CONTROLLING_TEXT: &str = "Über uns\n\
Ein Mittelständler aus dem Norden.\n\n\
Ihr Profil\n\
- Erfahrung im Controlling\n\
- Erfahrung in der Budgetierung\n\
- Kenntnisse in der Zollabwicklung\n\n\
Rahmenbedingungen\n\
Tagessatz 1.100 € pro Tag\n";

fn ad(text: &str) -> Ad<'_> {
    Ad { text, ..AD }
}

#[test]
fn a_schwerpunkt_the_ad_demands_counts_double_and_raises_relevance() {
    let plain = run(&base(), &ad(TREASURY_TEXT));
    let focus = with(&json!({ "schwerpunkte": ["Treasury"] }));
    let a = run(&focus, &ad(TREASURY_TEXT));
    assert!(a.score > plain.score, "{} vs {}", a.score, plain.score);
    let r = reason(&a, ReasonCode::Focus).expect("focus reason");
    assert_eq!(r.kind, ReasonKind::Met);
    assert_eq!(r.params["focus"], "Treasury");
    assert_eq!(r.params["met"], 1);
    assert_eq!(r.params["relevance"], 100);
    assert_eq!(
        r.evidence.as_ref().expect("evidence").path,
        "schwerpunkte[0]"
    );
    assert!(!r.ranges.is_empty(), "the requirement is highlighted");
    let requirement = a
        .reasons
        .iter()
        .find(|r| r.label.as_deref() == Some("Erfahrung im Treasury"))
        .expect("requirement");
    assert_eq!(requirement.params["focus"], "Treasury");
    // The alternative term of the competence belongs to the Schwerpunkt.
    let cash = run(
        &focus,
        &ad(
            "Ihr Profil\n- Erfahrung im Cash Management\n- Erfahrung im Controlling\n\
             - Kenntnisse in der Zollabwicklung\nRahmenbedingungen\nTagessatz 1.100 € pro Tag\n",
        ),
    );
    assert!(
        reason(&cash, ReasonCode::Focus).is_some(),
        "{:?}",
        cash.reasons
    );
}

#[test]
fn an_ad_that_demands_no_schwerpunkt_gets_nothing() {
    let focus = with(&json!({ "schwerpunkte": ["Treasury", "Restrukturierung"] }));
    let plain = run(&base(), &ad(CONTROLLING_TEXT));
    let a = run(&focus, &ad(CONTROLLING_TEXT));
    assert_eq!(a.score, plain.score);
    assert!(reason(&a, ReasonCode::Focus).is_none());
    // A broader requirement inside the Schwerpunkt does not demand it.
    let ifrs = with(&json!({ "schwerpunkte": ["Konzernrechnungslegung nach IFRS"] }));
    let text = "Ihr Profil\n- Kenntnisse in IFRS\n- Erfahrung im Controlling\n\
                - Kenntnisse in der Zollabwicklung\nRahmenbedingungen\nTagessatz 1.100 € pro Tag\n";
    assert_eq!(run(&ifrs, &ad(text)).score, run(&base(), &ad(text)).score);
    assert!(reason(&run(&ifrs, &ad(text)), ReasonCode::Focus).is_none());
}

#[test]
fn a_schwerpunkt_in_the_title_raises_relevance() {
    let title = Ad {
        title: "Interim Treasury Manager (m/w/d)",
        ..ad(CONTROLLING_TEXT)
    };
    let focus = with(&json!({ "schwerpunkte": ["Treasury"] }));
    let a = run(&focus, &title);
    let r = reason(&a, ReasonCode::Focus).expect("focus from the title");
    assert_eq!(r.params["inTitle"], true);
    assert!(a.score > run(&base(), &title).score);
}

#[test]
fn target_roles_full_half_and_none() {
    let roles = with(&json!({ "wunschrollen": ["Interim CFO", "Head of Finance"] }));
    let titled = |title: &'static str| Ad {
        title,
        ..ad(CONTROLLING_TEXT)
    };
    let plain = run(&base(), &titled("Interim CFO (m/w/d)"));
    let full = run(&roles, &titled("Interim CFO (m/w/d)"));
    let r = reason(&full, ReasonCode::TargetRole).expect("target role");
    assert_eq!(
        (r.kind, r.params["fit"].as_str()),
        (ReasonKind::Met, Some("full"))
    );
    assert_eq!(r.params["points"], 8);
    assert!(
        (7..=9).contains(&(full.score - plain.score)),
        "{} vs {}",
        full.score,
        plain.score
    );
    // The field without the lead level: half.
    let half = run(&roles, &titled("Interim Finance Manager (m/w/d)"));
    let r = reason(&half, ReasonCode::TargetRole).expect("half role");
    assert_eq!(
        (r.kind, r.params["points"].as_i64()),
        (ReasonKind::Partial, Some(4))
    );
    // Another role: nothing.
    let other = run(&roles, &titled("Senior Controller (m/w/d)"));
    assert!(reason(&other, ReasonCode::TargetRole).is_none());
    assert_eq!(
        other.score,
        run(&base(), &titled("Senior Controller (m/w/d)")).score
    );
}

#[test]
fn a_target_role_never_lifts_a_capped_or_excluded_ad() {
    let roles = with(&json!({ "wunschrollen": ["Interim CFO"] }));
    let off_field = Ad {
        title: "Interim CFO (m/w/d) Klinikverbund",
        text: "Ihr Profil\n- Erfahrung in der Krankenhausbuchhaltung\n\
               - Kenntnisse im DRG-System\n- Erfahrung in Pflegesatzverhandlungen\n\
               Rahmenbedingungen\nTagessatz 1.400 € pro Tag\n",
        ..AD
    };
    let a = run(&roles, &off_field);
    assert!(reason(&a, ReasonCode::TargetRole).is_some());
    assert!(a.score <= 25, "off-field cap holds, got {}", a.score);
    let anue = Ad {
        title: "Interim CFO (m/w/d)",
        text: "Ihr Profil\n- Erfahrung im Treasury\n- Erfahrung im Controlling\n\
               Rahmenbedingungen\nDie Besetzung erfolgt im Rahmen der Arbeitnehmerüberlassung.\n",
        ..AD
    };
    assert_eq!(run(&roles, &anue).verdict, Verdict::Excluded);
}

fn wishes(preferences: &Value) -> Value {
    with(&json!({ "einsatzpraeferenzen": preferences }))
}

#[test]
fn day_rate_wish_met_near_missed_unknown() {
    let profile = wishes(&json!({ "tagessatz_wunsch": 1000 }));
    let text = |rate: &str| {
        format!(
            "Ihr Profil\n- Erfahrung im Treasury\n- Erfahrung im Controlling\n\
             - Kenntnisse in der Zollabwicklung\nRahmenbedingungen\n{rate}\n"
        )
    };
    let check = |rate: &str, expected: &str| {
        let body = text(rate);
        let a = run(&profile, &ad(&body));
        assert_eq!(state(&a, ReasonCode::DayRateWish), expected, "{rate}");
        let plain = run(&base(), &ad(&body));
        (i32::from(a.score) - i32::from(plain.score), a)
    };
    let (delta, met) = check("Tagessatz 1.100 € pro Tag", "met");
    assert!(
        (2..=4).contains(&delta),
        "met adds three points, got {delta}"
    );
    let r = reason(&met, ReasonCode::DayRateWish).expect("reason");
    assert_eq!(
        (r.params["wish"].as_u64(), r.params["rate"].as_u64()),
        (Some(1000), Some(1100))
    );
    assert_eq!(check("Tagessatz 960 € pro Tag", "near").0, 0);
    let (delta, _) = check("Tagessatz 850 € pro Tag", "missed");
    assert!(
        (-4..=-2).contains(&delta),
        "missed takes three points, got {delta}"
    );
    assert_eq!(check("Start ab sofort", "unknown").0, 0);
    let (_, chf) = check("Tagessatz CHF 1.200 pro Tag", "unknown");
    assert_eq!(
        reason(&chf, ReasonCode::DayRateWish).unwrap().params["currency"],
        "CHF"
    );
    // Not for permanent roles.
    let body = text("Eine unbefristete Festanstellung mit einem Jahresgehalt von 120.000 €");
    let permanent = Ad {
        title: "Controller (m/w/d)",
        ..ad(&body)
    };
    let permanent = run(&profile, &permanent);
    assert!(reason(&permanent, ReasonCode::DayRateWish).is_none());
}

#[test]
fn remote_wish_levels_and_old_free_text() {
    let case = |wish: Value, frame: &str| {
        let body = format!(
            "Ihr Profil\n- Erfahrung im Treasury\n- Erfahrung im Controlling\n\
             Rahmenbedingungen\nTagessatz 1.100 € pro Tag\n{frame}\n"
        );
        let profile = wishes(&json!({ "remote": wish }));
        let ad = Ad { text: &body, ..AD };
        state(&run(&profile, &ad), ReasonCode::RemoteWish)
    };
    assert_eq!(case(json!("voll"), "Einsatz zu 100 % remote"), "met");
    assert_eq!(case(json!("voll"), "Einsatz zu 80 % remote"), "near");
    assert_eq!(case(json!("voll"), "Einsatz zu 60 % remote"), "missed");
    assert_eq!(
        case(json!("ueberwiegend"), "Zwei Tage pro Woche vor Ort"),
        "met"
    );
    assert_eq!(
        case(json!("ueberwiegend"), "Hybrides Arbeiten nach Absprache"),
        "near"
    );
    assert_eq!(
        case(json!("teilweise"), "Einsatz vor Ort in Hamburg"),
        "missed"
    );
    assert_eq!(case(json!("ueberwiegend"), "Remote-Anteil 40 %"), "near");
    // Old free text reads like the profile editor reads it (up to half: `teilweise`).
    assert_eq!(case(json!("mindestens 50 %"), "Remote-Anteil 40 %"), "met");
    assert_eq!(
        case(json!("mindestens 50 %"), "Einsatz vor Ort in Hamburg"),
        "missed"
    );
    assert_eq!(case(json!("80 % remote"), "Einsatz zu 60 % remote"), "met");
    assert_eq!(case(json!("vor_ort"), "Einsatz zu 100 % remote"), "missed");
    assert_eq!(case(json!("vor_ort"), "Einsatz vor Ort in Hamburg"), "met");
    assert_eq!(case(json!("voll"), "Start ab sofort"), "unknown");
}

#[test]
fn region_wish_places_states_and_remote_only() {
    let profile = wishes(&json!({ "regionen": ["Bayern", "Frankfurt am Main"] }));
    let case = |location: &'static str, frame: &str| {
        let body = format!(
            "Ihr Profil\n- Erfahrung im Treasury\n- Erfahrung im Controlling\n\
             Rahmenbedingungen\nTagessatz 1.100 € pro Tag\n{frame}\n"
        );
        let ad = Ad {
            location,
            text: &body,
            ..AD
        };
        let a = run(&profile, &ad);
        let r = reason(&a, ReasonCode::RegionWish)
            .expect("region reason")
            .clone();
        (state(&a, ReasonCode::RegionWish), r.params)
    };
    let (s, p) = case("München, Bayern, Deutschland", "Einsatz vor Ort");
    assert_eq!(
        (s.as_str(), p["location"].as_str()),
        ("met", Some("München"))
    );
    assert_eq!(case("D-90402 Nürnberg", "Einsatz vor Ort").0, "met");
    assert_eq!(case("60311 Frankfurt am Main", "Einsatz vor Ort").0, "met");
    let (s, p) = case("Hamburg, Deutschland", "Einsatz vor Ort in Hamburg");
    assert_eq!(
        (s.as_str(), p["location"].as_str()),
        ("missed", Some("Hamburg"))
    );
    assert_eq!(
        case("Hamburg, Deutschland", "Einsatz zu 60 % remote").0,
        "near"
    );
    let (s, p) = case("Hamburg, Deutschland", "Einsatz zu 100 % remote");
    assert_eq!((s.as_str(), p["remote"].as_bool()), ("met", Some(true)));
    assert_eq!(case("Deutschland", "Einsatz vor Ort").0, "unknown");
}

#[test]
fn industry_wish_from_intro_company_and_title() {
    let profile = wishes(&json!({ "branchen": ["Automobilindustrie", "Versicherungen"] }));
    let case = |ad: &Ad<'_>| state(&run(&profile, ad), ReasonCode::IndustryWish);
    let text = |intro: &str| {
        format!(
            "Über uns\n{intro}\n\nIhre Aufgaben\nGespräche mit den Banken\n\n\
             Ihr Profil\n- Erfahrung im Treasury\n- Erfahrung im Controlling\n\
             Rahmenbedingungen\nTagessatz 1.100 € pro Tag\n"
        )
    };
    let automotive = text("Ein Automobilzulieferer mit 900 Mitarbeitenden.");
    assert_eq!(case(&ad(&automotive)), "met");
    let retail = text("Ein Handelskonzern aus dem Norden.");
    assert_eq!(case(&ad(&retail)), "missed");
    // Tasks say nothing about the client's industry.
    let banks = wishes(&json!({ "branchen": ["Banken"] }));
    let plain = text("Ein Mittelständler.");
    assert_eq!(
        state(&run(&banks, &ad(&plain)), ReasonCode::IndustryWish),
        "unknown"
    );
    // The company counts on LinkedIn; on a freelance portal it is the agency.
    let wiesbaden = text("Ein Unternehmen aus Wiesbaden.");
    let insurer = Ad {
        company: "Rheingau Versicherung AG",
        ..ad(&wiesbaden)
    };
    assert_eq!(case(&insurer), "met");
    let agency = Ad {
        portal: Portal::Freelancermap,
        ..insurer
    };
    assert_eq!(case(&agency), "unknown");
}

#[test]
fn wishes_are_bounded_and_never_exclude() {
    let profile = wishes(&json!({
        "tagessatz_wunsch": 1500,
        "remote": "voll",
        "regionen": ["München"],
        "branchen": ["Automobilindustrie"]
    }));
    let missed = Ad {
        location: "Hamburg, Deutschland",
        text: "Über uns\nEin Handelskonzern.\n\nIhr Profil\n- Erfahrung im Treasury\n\
               - Erfahrung im Controlling\n- Kenntnisse in der Zollabwicklung\n\
               Rahmenbedingungen\nTagessatz 900 € pro Tag\nEinsatz vor Ort in Hamburg\n",
        ..AD
    };
    let plain = run(&base(), &missed);
    let a = run(&profile, &missed);
    assert_eq!(a.verdict, Verdict::Scored, "wishes never exclude");
    let delta = i32::from(plain.score) - i32::from(a.score);
    assert!(
        (9..=11).contains(&delta),
        "all four missed take ten points, got {delta}"
    );
    let points: i64 = a
        .reasons
        .iter()
        .filter_map(|r| r.params.get("points").and_then(Value::as_i64))
        .sum();
    assert_eq!(points, -10);
    for r in &a.reasons {
        assert_ne!(r.kind, ReasonKind::Violation, "{r:?}");
    }
}

#[test]
fn the_summary_and_the_fingerprint_follow_the_keys() {
    let plain = compile_profile(&base());
    assert!(plain.summary().wishes.iter().all(|w| !w.set));
    assert_eq!(plain.summary().wishes.len(), 6);
    let full = wishes(&json!({
        "tagessatz_wunsch": "1.300 €",
        "remote": "mindestens 60 %",
        "regionen": ["München"],
        "branchen": ["Maschinenbau"]
    }));
    let compiled = compile_profile(&full);
    let set = |key| {
        compiled
            .summary()
            .wishes
            .iter()
            .find(|w| w.key == key)
            .expect("wish")
            .clone()
    };
    assert_eq!(set(WishKey::DayRate).params["wish"], 1300);
    assert_eq!(set(WishKey::Remote).params["min"], 60);
    assert!(set(WishKey::Regions).set && set(WishKey::Industries).set);
    let mut seen = vec![plain.fingerprint()];
    for extra in [
        json!({ "schwerpunkte": ["Treasury"] }),
        json!({ "wunschrollen": ["Interim CFO"] }),
        json!({ "einsatzpraeferenzen": { "tagessatz_wunsch": 1200 } }),
        json!({ "einsatzpraeferenzen": { "remote": "voll" } }),
        json!({ "einsatzpraeferenzen": { "regionen": ["Hamburg"] } }),
        json!({ "einsatzpraeferenzen": { "branchen": ["Chemie"] } }),
    ] {
        let fingerprint = compile_profile(&with(&extra)).fingerprint();
        assert!(!seen.contains(&fingerprint), "{extra}");
        seen.push(fingerprint);
    }
}

#[test]
fn unreadable_and_too_many_values_are_warnings() {
    let profile = with(&json!({
        "schwerpunkte": ["Controlling", "Treasury", "Forecast", "Budgetierung",
                         "Restrukturierung", "Konzernrechnungslegung nach IFRS", "Cash Management"],
        "wunschrollen": "CFO",
        "einsatzpraeferenzen": { "tagessatz_wunsch": "viel", "remote": "gern", "regionen": [] }
    }));
    let compiled = compile_profile(&profile);
    let warnings = &compiled.summary().warnings;
    let trimmed = warnings
        .iter()
        .find(|w| w.code == ProfileWarningCode::FocusTrimmed)
        .expect("trimmed");
    assert_eq!(
        (
            trimmed.params["count"].as_u64(),
            trimmed.params["max"].as_u64()
        ),
        (Some(7), Some(5))
    );
    let keys: Vec<&str> = warnings
        .iter()
        .filter(|w| w.code == ProfileWarningCode::CriterionNotUnderstood)
        .filter_map(|w| w.params["key"].as_str())
        .collect();
    for key in ["wunschrollen", "tagessatz_wunsch", "remote", "regionen"] {
        assert!(keys.contains(&key), "{key} in {keys:?}");
    }
    // English keys, and a Schwerpunkt the walk did not find becomes a competence.
    let english = with(&json!({
        "focus_areas": ["Liquiditätsplanung"],
        "target_roles": ["Head of Finance"],
        "preferences": { "desired_day_rate": 1200, "regions": ["Hamburg"], "industries": ["Chemie"] }
    }));
    let compiled = compile_profile(&english);
    assert!(
        compiled
            .summary()
            .wishes
            .iter()
            .all(|w| w.key == WishKey::Remote || w.set)
    );
    let a = run(
        &english,
        &ad(
            "Ihr Profil\n- Erfahrung in der Liquiditätsplanung\n- Erfahrung im Controlling\n\
             Rahmenbedingungen\nTagessatz 1.300 € pro Tag\n",
        ),
    );
    let r = reason(&a, ReasonCode::Focus).expect("focus from focus_areas");
    assert_eq!(r.evidence.as_ref().unwrap().path, "focus_areas[0]");
}

/// Every reason and warning code of the engine has a text in the UI catalog.
#[test]
fn every_engine_code_has_a_catalog_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let types = std::fs::read_to_string(root.join("core/src/matching/types.rs")).expect("types");
    let catalog = std::fs::read_to_string(root.join("ui/src/lib/i18n/de.ts")).expect("de.ts");
    let variants = |name: &str| -> Vec<String> {
        let start = types
            .find(&format!("pub enum {name} {{"))
            .unwrap_or_else(|| panic!("{name}"));
        let body = &types[start..];
        let body = &body[body.find('{').unwrap() + 1..body.find("\n}").unwrap()];
        body.lines()
            .map(str::trim)
            .filter(|l| !l.starts_with("//") && !l.is_empty())
            .filter_map(|l| l.strip_suffix(','))
            .map(|v| {
                let mut camel = v[..1].to_lowercase();
                camel.push_str(&v[1..]);
                camel
            })
            .collect()
    };
    let block = |start: &str| -> &str {
        let at = catalog.find(start).unwrap_or_else(|| panic!("{start}"));
        let rest = &catalog[at..];
        &rest[..rest.find("} satisfies").expect("end of block")]
    };
    for (enum_name, table) in [
        ("ReasonCode", "const reasonCode = {"),
        ("ProfileWarningCode", "const warning = {"),
    ] {
        let block = block(table);
        for code in variants(enum_name) {
            let listed =
                block.contains(&format!("\n  {code}:")) || block.contains(&format!("\n  {code},"));
            assert!(listed, "{enum_name}::{code} has no text in de.ts");
        }
    }
}
