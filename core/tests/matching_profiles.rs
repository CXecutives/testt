//! Profiles beyond the corpus: a field without any domain pack (the general core alone
//! plus the profile's own `auch` terms), the summary of the new profile keys (English
//! aliases, warnings, packs, aliases) and formal duties (licences).

use std::path::Path;

use jobalert_core::matching::{
    Assessment, CriterionKey, JobInput, ProfileWarningCode, ReasonCode, ReasonKind, TextKind,
    Verdict, assess, compile_profile,
};
use jobalert_core::portal::Portal;
use serde_json::{Value, json};

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/matching")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("json")
}

fn run(profile: &Value, title: &str, text: &str) -> Assessment {
    let job = JobInput {
        title,
        location: "Berlin, Deutschland",
        portal: Portal::LinkedIn,
        text,
        facts: None,
        posted: None,
        kind: TextKind::Full,
    };
    assess(&compile_profile(profile), &job, None).expect("assessed")
}

fn has(a: &Assessment, code: ReasonCode, kind: ReasonKind) -> bool {
    a.reasons.iter().any(|r| r.code == code && r.kind == kind)
}

/// An invented clinical project manager: no domain pack, own alternative terms.
fn clinical() -> Value {
    json!({
        "name": "Nora Beispiel",
        "kernkompetenzen": [
            {"kompetenz": "Klinische Studienleitung", "jahre": 12,
             "auch": ["Clinical Trial Management", "Studienmanagement"]},
            {"kompetenz": "Good Clinical Practice", "jahre": 12, "auch": ["GCP"]},
            {"kompetenz": "Monitoring klinischer Prüfungen", "jahre": 8,
             "auch": ["Clinical Monitoring"]},
            {"kompetenz": "Projektmanagement", "jahre": 15},
            {"kompetenz": "Zulassungsstrategie", "jahre": 6, "auch": ["Regulatory Affairs"]},
            {"kompetenz": "Pharmakovigilanz", "jahre": 5}
        ],
        "sprachen": [
            {"sprache": "Deutsch", "niveau": "Muttersprache"},
            {"sprache": "Englisch", "niveau": "C1"}
        ],
        "harte_kriterien": {"laender": ["DE"], "ausgeschlossene_vertragsarten": ["anue"]}
    })
}

const CLINICAL_AD: &str = "About the role\n\
We are looking for an interim clinical project manager for a phase III study.\n\
Requirements\n\
Several years of experience in clinical trial management\n\
Sound knowledge of GCP\n\
Experience in clinical monitoring\n\
Experience in project management\n\
Fluent English\n\
What we offer\n\
Day rate: 1,050 EUR\n";

const FINANCE_AD: &str = "Ihr Profil\n\
Mehrjährige Erfahrung im Controlling\n\
Fundierte Kenntnisse in der Konzernrechnungslegung nach IFRS\n\
Erfahrung mit Monatsabschlüssen nach HGB\n\
Rahmenbedingungen\n\
Tagessatz 1.200 € pro Tag\n";

#[test]
fn the_core_alone_serves_an_unrelated_field() {
    let profile = clinical();
    let compiled = compile_profile(&profile);
    assert!(
        compiled.summary().packs.is_empty(),
        "no pack for clinical work"
    );
    let fit = run(
        &profile,
        "Interim Clinical Project Manager (m/w/d)",
        CLINICAL_AD,
    );
    assert_eq!(fit.verdict, Verdict::Scored);
    assert!(fit.score >= 80, "clinical ad scored {}", fit.score);
    let alias = fit
        .reasons
        .iter()
        .find_map(|r| r.evidence.as_ref().filter(|e| e.path.contains(".auch[")))
        .expect("an alias meets a requirement");
    assert_eq!(alias.path, "kernkompetenzen[0].auch[0]");
    let other = run(&profile, "Interim Konzerncontroller (m/w/d)", FINANCE_AD);
    assert!(other.score <= 25, "finance ad scored {}", other.score);
    assert!(has(&other, ReasonCode::ContractType, ReasonKind::Met));
}

#[test]
fn packs_follow_the_competences() {
    let senior = compile_profile(&fixture("sample_profile_senior.json"));
    assert_eq!(senior.summary().packs, ["finance", "sap"]);
    let sap = compile_profile(&fixture("sample_profile_sap.json"));
    assert_eq!(sap.summary().packs, ["finance", "sap", "itProject"]);
    let it = compile_profile(&fixture("sample_profile_it.json"));
    assert_eq!(it.summary().packs, ["sap", "itProject"]);
}

#[test]
fn the_summary_shows_the_new_keys() {
    let senior = compile_profile(&fixture("sample_profile_senior.json"));
    let summary = senior.summary();
    assert_eq!(summary.years, Some(30));
    assert_eq!(summary.degrees, ["Diplom-Kauffrau (Univ.)"]);
    let criterion = |key| {
        summary
            .criteria
            .iter()
            .find(|c| c.key == key)
            .expect("criterion")
    };
    assert_eq!(criterion(CriterionKey::MinSalary).params["min"], 150_000);
    let region = criterion(CriterionKey::PermanentRegion);
    assert!(region.set);
    assert_eq!(region.params["remoteMin"], 60);
    assert_eq!(criterion(CriterionKey::TargetYears).params["min"], 10);
    assert!(summary.aliases.iter().any(|a| a.competence == "Controlling"
        && a.alias == "FP&A"
        && a.path == "kernkompetenzen[2].auch[1]"));
    assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
    // Aliases are no extra competences.
    assert_eq!(
        senior.quality(),
        jobalert_core::matching::ProfileQuality::Good
    );

    let sap = compile_profile(&fixture("sample_profile_sap.json"));
    let set = |key| sap.summary().criteria.iter().any(|c| c.key == key && c.set);
    assert!(set(CriterionKey::MinSalary) && set(CriterionKey::TargetYears));
    assert!(!set(CriterionKey::PermanentRegion), "no places: rule off");
}

#[test]
fn english_keys_and_unreadable_values() {
    let profile = json!({
        "kernkompetenzen": [
            {"kompetenz": "Controlling"}, {"kompetenz": "Konsolidierung"},
            {"kompetenz": "Treasury"}, {"kompetenz": "Budgetierung"}, {"kompetenz": "Forecast"}
        ],
        "hard_criteria": {
            "min_annual_salary": "120k",
            "permanent_remote_min": 60,
            "target_min_years": "viele"
        }
    });
    let compiled = compile_profile(&profile);
    let summary = compiled.summary();
    let salary = summary
        .criteria
        .iter()
        .find(|c| c.key == CriterionKey::MinSalary)
        .expect("salary");
    assert_eq!(salary.params["min"], 120_000);
    let codes: Vec<ProfileWarningCode> = summary.warnings.iter().map(|w| w.code).collect();
    assert!(codes.contains(&ProfileWarningCode::RegionWithoutPlaces));
    let unreadable = summary
        .warnings
        .iter()
        .find(|w| w.code == ProfileWarningCode::CriterionNotUnderstood)
        .expect("unreadable value");
    assert_eq!(unreadable.params["key"], "target_min_years");
}

#[test]
fn a_mandatory_licence_excludes_an_optional_one_caps() {
    let profile = fixture("sample_profile_senior.json");
    let ad = |licence: &str| {
        format!(
            "Anforderungen:\n- {licence}\n- Mehrjährige Erfahrung im Controlling\n\
             - Fundierte Kenntnisse in der Konzernrechnungslegung nach IFRS\n\
             - Erfahrung mit Monatsabschlüssen\nRahmenbedingungen:\nTagessatz: 1.300 € pro Tag\n"
        )
    };
    let mandatory = run(
        &profile,
        "Interim Leitung Steuern (m/w/d)",
        &ad("Zulassung als Steuerberater:in zwingend erforderlich"),
    );
    assert_eq!(mandatory.verdict, Verdict::Excluded);
    assert!(has(
        &mandatory,
        ReasonCode::FormalOpen,
        ReasonKind::Violation
    ));
    let optional = run(
        &profile,
        "Interim Leitung Steuern (m/w/d)",
        &ad("Zulassung als Steuerberater:in"),
    );
    assert_eq!(optional.verdict, Verdict::Scored);
    assert!(has(&optional, ReasonCode::FormalOpen, ReasonKind::Check));
    assert!(optional.score <= 40, "capped, got {}", optional.score);
}

/// Ordinary German requirement lines that once panicked the splitter (overlapping ", " and
/// " oder ", "z. B." after a closed bracket) - for every sample profile.
#[test]
fn separator_edge_lines_assess_for_every_sample_profile() {
    let lines = [
        "Studium der Wirtschaftswissenschaften, oder eine vergleichbare Qualifikation",
        "Erfahrung mit einem ERP-System (SAP) z. B. S/4HANA",
    ];
    for name in [
        "sample_profile.json",
        "sample_profile_it.json",
        "sample_profile_sap.json",
        "sample_profile_senior.json",
    ] {
        let profile = fixture(name);
        for line in lines {
            let text = format!(
                "Für ein Transformationsprojekt im Finanzbereich eines Konzerns suchen wir ab \
                 sofort eine erfahrene Unterstützung.\n\nIhr Profil:\n- {line}\n\
                 - Erfahrung im Controlling\n"
            );
            let a = run(&profile, "Interim Controller (m/w/d)", &text);
            assert!(a.score <= 100, "{name}: {line}");
            assert!(
                a.reasons
                    .iter()
                    .filter_map(|r| r.label.as_deref())
                    .any(|label| line.starts_with(label)),
                "{name}: the line is read as a requirement: {:?}",
                a.reasons
            );
        }
    }
}

/// A rate beyond any plausible amount stays far above the minimum (1000 in the sample
/// profile); it neither overflows nor wraps below it into a decided exclusion.
#[test]
fn an_absurd_hourly_rate_is_no_day_rate_violation() {
    let text = "Für ein Transformationsprojekt im Finanzbereich eines Konzerns suchen wir ab \
                sofort eine erfahrene Unterstützung.\n\n\
                Ihr Profil:\n- Erfahrung im Controlling\n\nRahmenbedingungen\n\
                - Stundensatz 99999999999999999999999 € pro Stunde\n";
    let a = run(
        &fixture("sample_profile.json"),
        "Interim Controller (m/w/d)",
        text,
    );
    assert!(
        !has(&a, ReasonCode::DayRate, ReasonKind::Violation),
        "{:?}",
        a.reasons
    );
    assert_ne!(a.verdict, Verdict::Excluded, "{:?}", a.reasons);
}
