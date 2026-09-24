//! The request a user hands to Claude together with a CV ("Aus Lebenslauf erstellen" in the
//! Profil view): Claude answers with a profile in exactly the JSON the editor reads, and the
//! user pastes the answer back into the app. German, because the user sends it as it is.
//!
//! external contract - do not translate: the text and the profile JSON keys it names (a
//! test checks that the editor reads every key of the skeleton).

/// The skeleton Claude fills: the keys of the form in the order the app writes them. Wishes,
/// target roles and hard criteria are left out on purpose: they are preferences, not facts
/// of a CV.
pub(crate) const SKELETON: &str = r#"{
  "name": "",
  "titel": "",
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
  "keywords": []
}"#;

const RULES: &str = "Erstelle aus meinem angehängten Lebenslauf ein Beraterprofil für den \
Job-Alert-Monitor. Die App vergleicht damit Stellenanzeigen mit meinem Profil.

Regeln
- Übernimm nur, was im Lebenslauf steht. Erfinde nichts und schätze nichts.
- Was der Lebenslauf nicht hergibt, bleibt leer, bei Zahlen null.
- titel ist meine berufliche Rolle in wenigen Worten, etwa Interim CFO.
- berufserfahrung_jahre sind die Jahre Berufserfahrung insgesamt.
- kernkompetenzen sind meine fachlichen Schwerpunkte, einzeln und kurz benannt, mit den Jahren \
Erfahrung, wenn der Lebenslauf sie belegt. Unter auch stehen andere übliche Begriffe für \
dieselbe Kompetenz, etwa auf Englisch.
- schwerpunkte sind drei bis fünf der kernkompetenzen, die im Lebenslauf am meisten Gewicht haben, genau so geschrieben wie dort.
- methoden_tools sind Software, Systeme und Methoden, zertifizierungen meine Zertifikate.
- niveau ist A1, A2, B1, B2, C1, C2 oder Muttersprache.
- alleinstellungsmerkmale sind bis zu fünf kurze Sätze, was mich auszeichnet.
- keywords sind Fachbegriffe, die in passenden Anzeigen stehen.
- Keine Kontaktdaten, keine Adresse, kein Geburtsdatum.

Antworte nur mit dem JSON in genau diesem Aufbau, ohne Erklärung davor oder danach.";

/// The whole request: rules, then the skeleton.
pub fn text() -> String {
    format!("{RULES}\n\n{SKELETON}\n")
}

#[cfg(test)]
mod tests {
    use super::super::form;
    use super::super::json::Json;
    use super::*;

    /// Every key of the skeleton is one the editor reads: a filled skeleton fills every
    /// field of the form except the hard criteria, and writing that form into an empty
    /// profile gives back exactly the keys of the skeleton.
    #[test]
    fn the_skeleton_names_exactly_the_keys_of_the_form() {
        let filled = SKELETON
            .replacen("\"name\": \"\"", "\"name\": \"Erika Beispiel\"", 1)
            .replace("\"titel\": \"\"", "\"titel\": \"Interim CFO\"")
            .replace(
                "\"berufserfahrung_jahre\": null",
                "\"berufserfahrung_jahre\": 20",
            )
            .replace("\"abschluss\": \"\"", "\"abschluss\": \"Diplom-Kauffrau\"")
            .replace(
                "{ \"kompetenz\": \"\", \"jahre\": null, \"auch\": [] }",
                "{ \"kompetenz\": \"Controlling\", \"jahre\": 12, \"auch\": [\"FP&A\"] }",
            )
            .replace("{ \"name\": \"\" }", "{ \"name\": \"SAP\" }")
            .replace("\"branche\": \"\"", "\"branche\": \"Chemie\"")
            .replace(
                "\"sprache\": \"\", \"niveau\": \"\"",
                "\"sprache\": \"Englisch\", \"niveau\": \"C1\"",
            )
            .replace(
                "\"alleinstellungsmerkmale\": []",
                "\"alleinstellungsmerkmale\": [\"Schnell\"]",
            )
            .replace("\"keywords\": []", "\"keywords\": [\"IFRS\"]")
            .replace(
                "\"schwerpunkte\": []",
                "\"schwerpunkte\": [\"Controlling\"]",
            );
        let doc: Json = serde_json::from_str(&filled).unwrap();
        let read = form::read(&doc);
        let empty = form::ProfileForm::default();
        for (field, is_empty) in [
            ("name", read.name.is_empty()),
            ("title", read.title.is_empty()),
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
        ] {
            assert!(!is_empty, "{field} not read from the skeleton");
        }
        assert_eq!(read.criteria, empty.criteria, "no criteria in a CV");
        assert_eq!(read.wishes, empty.wishes, "no wishes in a CV");
        assert!(read.roles.is_empty(), "no target roles in a CV");

        // Two degrees go into `ausbildung`, as the skeleton has them.
        let mut form = read.clone();
        form.degrees.push("MBA".into());
        let mut written = Json::object();
        form::merge(&mut written, &empty, &form);
        let keys = |doc: &Json| match doc {
            Json::Object(entries) => entries.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        assert_eq!(keys(&written), keys(&doc));
    }

    #[test]
    fn the_text_ends_with_the_skeleton() {
        let text = text();
        assert!(text.contains("nur mit dem JSON"));
        assert!(text.trim_end().ends_with(SKELETON));
    }
}
