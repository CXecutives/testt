//! The request a user hands to an AI together with a CV ("Aus Lebenslauf erstellen" and
//! "Aus Lebenslauf aktualisieren" in the Profil view): the AI answers with a profile in exactly
//! the JSON the editor reads, and the user pastes the answer back into the app. In the app's
//! language, because the user sends it as it is; the JSON keys stay German in both (they are
//! the profile format).
//!
//! external contract - do not translate: the German text and the profile JSON keys it names
//! (a test checks that the editor reads every key of the skeleton).

use crate::settings::Language;

/// The skeleton the AI fills: the keys of the form in the order the app writes them, with the
/// career stations (`stationen`, read by the engine, kept in the file) before the wishes and
/// the hard criteria, which only a CV that states them fills.
pub(crate) const SKELETON: &str = r#"{
  "name": "",
  "titel": "",
  "wunschrollen": [],
  "berufserfahrung_jahre": null,
  "ausbildung": [
    { "abschluss": "" }
  ],
  "kernkompetenzen": [
    { "kompetenz": "", "jahre": null, "auch": [] }
  ],
  "schwerpunkte": [],
  "methoden_tools": [
    { "name": "" }
  ],
  "zertifizierungen": [
    { "name": "" }
  ],
  "branchen": [
    { "branche": "" }
  ],
  "sprachen": [
    { "sprache": "", "niveau": "" }
  ],
  "alleinstellungsmerkmale": [],
  "keywords": [],
  "stationen": [
    { "zeitraum": "", "rolle": "", "schwerpunkte": [] }
  ],
  "einsatzpraeferenzen": {
    "tagessatz_wunsch": null,
    "remote": "",
    "regionen": [],
    "branchen": []
  },
  "harte_kriterien": {
    "min_tagessatz": null,
    "laender": [],
    "ausgeschlossene_vertragsarten": [],
    "verfuegbar_ab": ""
  }
}"#;

const RULES: &str = "Erstelle aus meinem angehängten Lebenslauf ein Beraterprofil für den \
Job-Alert-Monitor. Die App vergleicht damit Stellenanzeigen mit meinem Profil.

Regeln
- Übernimm nur, was im Lebenslauf steht. Erfinde nichts und schätze nichts.
- Was der Lebenslauf nicht hergibt, bleibt leer, bei Zahlen null.
- titel ist meine berufliche Rolle in wenigen Worten, etwa Projektleitung.
- wunschrollen sind die Rollen, für die ich laut Lebenslauf gebucht werden will.
- berufserfahrung_jahre sind die Jahre Berufserfahrung insgesamt.
- kernkompetenzen sind meine fachlichen Schwerpunkte, einzeln und kurz benannt, mit den Jahren \
Erfahrung, wenn der Lebenslauf sie belegt. Unter auch stehen andere übliche Begriffe für \
dieselbe Kompetenz, etwa auf Englisch.
- schwerpunkte sind drei bis fünf der kernkompetenzen, die im Lebenslauf am meisten Gewicht haben, genau so geschrieben wie dort.
- methoden_tools sind Software, Systeme und Methoden, zertifizierungen meine Zertifikate.
- niveau ist A1, A2, B1, B2, C1, C2 oder Muttersprache.
- alleinstellungsmerkmale sind bis zu fünf kurze Sätze, was mich auszeichnet.
- keywords sind Fachbegriffe, die in passenden Anzeigen stehen.
- stationen sind meine beruflichen Stationen, je mit zeitraum, rolle und den fachlichen \
Themen der Station unter schwerpunkte.
- einsatzpraeferenzen und harte_kriterien füllst du nur, wenn der Lebenslauf sie nennt. \
Tagessätze sind Euro pro Tag, remote ist voll, ueberwiegend, teilweise oder vor_ort, laender \
sind Ländercodes wie DE, verfuegbar_ab ist ein Datum wie 01.11.2026 oder sofort, \
ausgeschlossene_vertragsarten nennt anue oder festanstellung.
- Keine Kontaktdaten, keine Adresse, kein Geburtsdatum.

Antworte nur mit dem JSON in genau diesem Aufbau, ohne Erklärung davor oder danach.";

/// The same rules in English (the keys they name stay German).
const RULES_EN: &str = "Create a consultant profile for the Job-Alert-Monitor from my attached \
CV. The app compares job ads with my profile.

Rules
- Take only what the CV says. Invent nothing and estimate nothing.
- Whatever the CV does not say stays empty, and numbers stay null.
- titel is my professional role in a few words, such as project manager.
- wunschrollen are the roles the CV says I want to be booked for.
- berufserfahrung_jahre is the total number of years of my professional experience.
- kernkompetenzen are my core skills, each named on its own and briefly, with the years of \
experience where the CV proves them. Under auch go other common terms for the same skill, for \
example in German.
- schwerpunkte are three to five of the kernkompetenzen that carry the most weight in the CV, written exactly as they appear there.
- methoden_tools are software, systems and methods, zertifizierungen my certificates.
- niveau is A1, A2, B1, B2, C1, C2 or Muttersprache for a native language.
- alleinstellungsmerkmale are up to five short sentences on what sets me apart.
- keywords are technical terms that appear in matching ads.
- stationen are the stages of my career, each with zeitraum, rolle and the specialist \
topics of the stage under schwerpunkte.
- Fill einsatzpraeferenzen and harte_kriterien only where the CV states them. Day rates are \
euros per day, remote is voll, ueberwiegend, teilweise or vor_ort, laender are country codes \
such as DE, verfuegbar_ab is a date such as 01.11.2026 or sofort, and \
ausgeschlossene_vertragsarten names anue or festanstellung.
- No contact details, no address, no date of birth.
- Keep every key of the JSON exactly as it is written below.

Answer only with the JSON in exactly this structure, without any explanation before or after it.";

/// The whole request in the app's language: rules, then the skeleton.
pub fn text(language: Language) -> String {
    let rules = match language {
        Language::De => RULES,
        Language::En => RULES_EN,
    };
    format!("{rules}\n\n{SKELETON}\n")
}

#[cfg(test)]
mod tests {
    use super::super::form;
    use super::super::json::Json;
    use super::*;
    use crate::error::InvalidInput;
    use crate::matching::{self, ProfileWarningCode};

    /// The skeleton filled the way an AI would answer (same keys, same order).
    const FILLED: &str = r#"{
  "name": "Erika Beispiel",
  "titel": "Interim CFO",
  "wunschrollen": ["Interim CFO"],
  "berufserfahrung_jahre": 20,
  "ausbildung": [
    { "abschluss": "Diplom-Kauffrau" }
  ],
  "kernkompetenzen": [
    { "kompetenz": "Controlling", "jahre": 12, "auch": ["FP&A"] }
  ],
  "schwerpunkte": ["Controlling"],
  "methoden_tools": [
    { "name": "SAP" }
  ],
  "zertifizierungen": [
    { "name": "PMP" }
  ],
  "branchen": [
    { "branche": "Chemie" }
  ],
  "sprachen": [
    { "sprache": "Englisch", "niveau": "C1" }
  ],
  "alleinstellungsmerkmale": ["Schnell"],
  "keywords": ["IFRS"],
  "stationen": [
    { "zeitraum": "2012 - heute", "rolle": "Interim CFO", "schwerpunkte": ["Restrukturierung"] }
  ],
  "einsatzpraeferenzen": {
    "tagessatz_wunsch": 1200,
    "remote": "ueberwiegend",
    "regionen": ["Hamburg"],
    "branchen": ["Chemie"]
  },
  "harte_kriterien": {
    "min_tagessatz": 1000,
    "laender": ["DE"],
    "ausgeschlossene_vertragsarten": ["anue"],
    "verfuegbar_ab": "sofort"
  }
}"#;

    /// The keys of a document in order, the items of a list by its first one.
    fn shape(value: &Json) -> String {
        match value {
            Json::Object(entries) => entries
                .iter()
                .map(|(key, child)| format!("{key}{{{}}}", shape(child)))
                .collect::<Vec<_>>()
                .join(","),
            Json::Array(items) => items.first().map(shape).unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn keys(doc: &Json) -> Vec<String> {
        match doc {
            Json::Object(entries) => entries.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        }
    }

    /// Every key of the skeleton is one the editor or the engine reads: the filled skeleton
    /// fills every field of the form, its stations (kept in the file, the form does not show
    /// them) give the engine terms, and writing the form into an empty profile gives back the
    /// keys of the skeleton but the stations.
    #[test]
    fn the_skeleton_names_exactly_the_keys_of_the_form() {
        let skeleton: Json = serde_json::from_str(SKELETON).unwrap();
        let doc: Json = serde_json::from_str(FILLED).unwrap();
        assert_eq!(
            shape(&doc),
            shape(&skeleton),
            "the answer keeps the skeleton"
        );
        let read = form::read(&doc);
        let c = &read.criteria;
        let w = &read.wishes;
        for (field, is_empty) in [
            ("name", read.name.is_empty()),
            ("title", read.title.is_empty()),
            ("roles", read.roles.is_empty()),
            ("years", read.years.is_none()),
            ("degrees", read.degrees.is_empty()),
            ("competences", read.competences.is_empty()),
            ("aliases", read.competences[0].aliases.is_empty()),
            ("tools", read.tools.is_empty()),
            ("certificates", read.certificates.is_empty()),
            ("industries", read.industries.is_empty()),
            ("languages", read.languages[0].level.is_none()),
            ("strengths", read.strengths.is_empty()),
            ("keywords", read.keywords.is_empty()),
            ("focus", read.focus.is_empty()),
            ("wishDayRate", w.day_rate.is_none()),
            ("remote", w.remote.is_none()),
            ("regions", w.regions.is_empty()),
            ("wishIndustries", w.industries.is_empty()),
            ("minDayRate", c.min_day_rate.is_none()),
            ("countries", c.countries.is_empty()),
            ("noAnue", !c.no_anue),
            ("available", c.available == form::ProfileAvailability::Unset),
        ] {
            assert!(!is_empty, "{field} not read from the skeleton");
        }
        let summary = matching::compile_profile(&doc.to_value()).summary().clone();
        assert!(
            summary
                .sources
                .iter()
                .any(|s| s.path.starts_with("stationen[]")),
            "{:?}",
            summary.sources
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);

        // Two degrees go into `ausbildung`, as the skeleton has them.
        let mut form = read.clone();
        form.degrees.push("MBA".into());
        let mut written = Json::object();
        form::merge(&mut written, &form::ProfileForm::default(), &form, &[]);
        let mut expected = keys(&skeleton);
        expected.retain(|key| key != "stationen");
        assert_eq!(keys(&written), expected);
    }

    /// What the AI leaves as in the skeleton is dropped: the unfilled skeleton holds no
    /// profile, and a partly filled one leaves no empty criterion the app cannot read.
    #[test]
    fn an_answer_without_values_leaves_no_empty_keys() {
        assert_eq!(
            super::super::draft_from_answer(SKELETON).map(|d| d.form),
            Err(InvalidInput::ProfileAnswer)
        );
        let answer = SKELETON.replacen("\"name\": \"\"", "\"name\": \"Erika Beispiel\"", 1);
        let draft = super::super::draft_from_answer(&answer).unwrap();
        assert_eq!(draft.source, "{\n  \"name\": \"Erika Beispiel\"\n}");
        assert!(
            !draft
                .summary
                .warnings
                .iter()
                .any(|w| w.code == ProfileWarningCode::CriterionNotUnderstood),
            "{:?}",
            draft.summary.warnings
        );
    }

    #[test]
    fn the_text_ends_with_the_skeleton() {
        let german = text(Language::De);
        assert!(german.contains("nur mit dem JSON"));
        assert!(german.trim_end().ends_with(SKELETON));
        let english = text(Language::En);
        assert!(english.contains("only with the JSON"));
        assert!(english.trim_end().ends_with(SKELETON));
        assert!(!english.contains("Lebenslauf") && !english.contains("Antworte"));
        // Both name the same keys of the skeleton (and the level the form reads as native).
        for key in [
            "titel",
            "wunschrollen",
            "berufserfahrung_jahre",
            "kernkompetenzen",
            "auch",
            "schwerpunkte",
            "methoden_tools",
            "zertifizierungen",
            "niveau",
            "Muttersprache",
            "alleinstellungsmerkmale",
            "keywords",
            "stationen",
            "einsatzpraeferenzen",
            "harte_kriterien",
            "anue",
            "festanstellung",
        ] {
            assert!(RULES.contains(key) && RULES_EN.contains(key), "{key}");
        }
    }
}
