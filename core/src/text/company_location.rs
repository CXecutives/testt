//! Firma und Ort: kurz und an der richtigen Stelle.
//!
//! Die Portale schreiben beides uneinheitlich an: freelancermap stellt „von:“ vor die
//! Firma und liefert als Ort die volle Anschrift; freelance.de setzt hinter den Titel nur
//! den Ort („D-20038 Hamburg“), der sonst in der Firmenspalte landet; LinkedIn hängt
//! Länder an („Köln (Deutschland)“). Gespeichert werden die Rohwerte – diese Funktionen
//! bereinigen nur für Anzeige und Export.

use std::sync::LazyLock;

use regex::Regex;

use super::{normalize, strip_chars, truncate_chars};

/// Floskeln vor dem Firmennamen – nur **mit** Doppelpunkt, sonst zerschnitte man echte
/// Namen wie „von Rundstedt & Partner“.
static FIRM_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:von|firma|unternehmen|company|arbeitgeber|auftraggeber|kunde|endkunde|ansprechpartner)\s*:\s*",
    )
    .unwrap()
});

/// Arbeitsform in einer Ortsangabe („Berlin (Remote)“, „Hybrid“, „Vor Ort“).
static WORKPLACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:remote|hybrid|vor ort|on-site|onsite|home ?office)\b").unwrap()
});

/// Ort von der Anzeigenseite, ohne die Arbeitsform der Mail zu verlieren: LinkedIn und
/// freelancermap nennen auf der Seite oft nur den Ort, die Mail aber „Berlin (Remote)“ –
/// für das Matching ist das mitunter das einzige Remote-Signal.
/// Höchstens `max` Zeichen – gekürzt wird der Ort, nie die Arbeitsform.
pub fn page_location(stored: &str, page: &str, max: usize) -> String {
    match WORKPLACE.find(stored) {
        Some(mode) if !page.is_empty() && !WORKPLACE.is_match(page) => {
            let suffix = format!(" ({})", mode.as_str());
            let room = max.saturating_sub(suffix.chars().count());
            format!("{}{suffix}", truncate_chars(page, room))
        }
        _ => truncate_chars(page, max),
    }
}

/// Postleitzahl, auch mit Ländervorsatz („D-68159“, „A-1010“, „CH-4000“).
const POSTCODE: &str = r"(?:D-|A-|CH-)?\d{4,5}";

/// „Am Mühlenweg 68, 27356 Rotenburg Wümme“ → Ortsname.
static ADDRESS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^(?:.*?,\s*)?{POSTCODE}\s+(\S.*)$")).unwrap());

/// Anschrift ohne Postleitzahl: „Am Mühlenweg 68, Rotenburg“ → „Rotenburg“.
static STREET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?\d\s*[a-zA-Z]?,\s*(\S.*)$").unwrap());

/// Postleitzahl + Ortsname am Anfang des Werts.
static POSTCODE_AT_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^{POSTCODE}\s+\p{{L}}")).unwrap());

/// Postleitzahl + Ortsname hinter einem Komma (Anschrift). Nur an diesen beiden Stellen
/// gilt eine Ziffernfolge als Postleitzahl – „Vision 2030 Consulting“ ist kein Ort
/// (früher: jede 4–5-stellige Zahl irgendwo im Wert zählte).
static POSTCODE_AFTER_COMMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r",\s*{POSTCODE}\s+\p{{L}}")).unwrap());

const COUNTRIES: [&str; 8] = [
    "deutschland",
    "germany",
    "österreich",
    "austria",
    "schweiz",
    "switzerland",
    "luxemburg",
    "luxembourg",
];
const COUNTRY_CODES: [&str; 4] = ["de", "at", "ch", "lu"];

/// Land als eigenes Wort im Wert („Hessen Deutschland“, „Berlin (Deutschland)“).
static COUNTRY_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)(?:^|[\s(])(?:{})\b", COUNTRIES.join("|"))).unwrap()
});

/// Rechtsform: Ein Wert damit ist eine Firma. Ausgeschriebene Formen in jeder
/// Schreibweise, die kurzen Kürzel nur in Großbuchstaben – „Muster AG“ ist eine Firma.
static LEGAL_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i:\b(?:gmbh|mbh|ohg|gbr|partg|kgaa|e\.?\s?v\.?|e\.?\s?k\.?|inc|ltd|llc|sarl|plc|corp|s\.?r\.?l|s\.?p\.?a|b\.v|n\.v)\b)|\b(?:AG|SE|KG|UG|S\.A)\b",
    )
    .unwrap()
});

/// Klammerzusätze wie „(AG)“ in „Zug (AG)“ sind Kantone oder Hinweise, keine Rechtsform.
static PARENTHESES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());

const EDGE_TRIM: &str = " -–—,(/|";

fn has_legal_form(value: &str) -> bool {
    LEGAL_FORM.is_match(&PARENTHESES.replace_all(value, " "))
}

fn flat(text: &str) -> String {
    normalize(text).replace('\n', " ")
}

/// Nur der Firmenname – Floskeln wie „von:“ davor fallen weg (höchstens 80 Zeichen).
pub fn short_company(text: &str) -> String {
    let value = flat(text);
    let value = FIRM_PREFIX.replace(&value, "");
    let value = strip_chars(&value, " -–—·|,");
    truncate_chars(value, 80)
}

/// Landeszusätze entfernen: „Berlin, Deutschland“, „Hessen Deutschland“,
/// „Berlin - Germany, 100% onsite work required (Deutschland)“. Firmen bleiben
/// unangetastet („Telekom Deutschland GmbH“).
fn without_country(value: &str) -> String {
    if has_legal_form(value) {
        return value.to_string();
    }
    let mut value = value.to_string();
    while let Some((head, tail)) = value.rsplit_once(',') {
        let tail = strip_chars(tail.trim(), "()").to_lowercase();
        let is_country =
            COUNTRIES.contains(&tail.as_str()) || COUNTRY_CODES.contains(&tail.as_str());
        if !is_country {
            break;
        }
        value = head.trim().to_string();
    }
    for hit in COUNTRY_WORD.find_iter(&value) {
        let before = strip_chars(&value[..hit.start()], EDGE_TRIM);
        if !before.is_empty() {
            value = before.to_string();
            break;
        }
    }
    strip_chars(&value, EDGE_TRIM).to_string()
}

/// Nur der Ort – eine Anschrift wird auf den Ortsnamen gekürzt (höchstens 60 Zeichen).
pub fn short_location(text: &str) -> String {
    let mut value = without_country(flat(text).trim());
    if let Some(town) = ADDRESS
        .captures(&value)
        .or_else(|| STREET.captures(&value))
        .and_then(|c| c.get(1))
    {
        value = town.as_str().to_string();
    }
    truncate_chars(strip_chars(&value, " -–—,·|"), 60)
}

/// Steht in dem Wert ein Ort statt einer Firma?
///
/// Entschieden wird nach Aufbau, nicht nach Reihenfolge von Verboten: Postleitzahl am
/// Anfang ⇒ Ort (auch „CH-6300 Zug (AG)“); sonst macht eine Rechtsform
/// den Wert zur Firma; sonst sprechen eine Anschrift hinter einem Komma oder ein Land mit
/// Text davor für einen Ort.
pub fn looks_like_location(text: &str) -> bool {
    let value = flat(text);
    let value = value.trim();
    if POSTCODE_AT_START.is_match(value) {
        return true;
    }
    if has_legal_form(value) {
        return false;
    }
    if POSTCODE_AFTER_COMMA.is_match(value) {
        return true;
    }
    COUNTRY_WORD
        .find_iter(value)
        .any(|hit| !strip_chars(&value[..hit.start()], EDGE_TRIM).is_empty())
}

/// Firma und Ort getrennt, kurz und an der richtigen Stelle: Steht im Firmenfeld ein
/// Ort und ist das Ortsfeld leer, wandert er hinüber.
pub fn split_company_location(company: &str, location: &str) -> (String, String) {
    let firm = short_company(company);
    let town = short_location(location);
    if town.is_empty() && looks_like_location(&firm) {
        return (String::new(), short_location(&firm));
    }
    (firm, town)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_location_keeps_the_workplace_from_the_mail() {
        let loc = |stored, page| page_location(stored, page, 120);
        assert_eq!(loc("Berlin (Remote)", "Berlin"), "Berlin (Remote)");
        assert_eq!(
            loc("Saarpfalz-Kreis (Hybrid)", "Saarpfalz-Kreis"),
            "Saarpfalz-Kreis (Hybrid)"
        );
        assert_eq!(loc("Remote", "Duisburg"), "Duisburg (Remote)");
        assert_eq!(
            loc("Berlin (Vor Ort)", "Berlin, Deutschland"),
            "Berlin, Deutschland (Vor Ort)"
        );
        assert_eq!(
            loc("Hamburg", "Hamburg, Deutschland"),
            "Hamburg, Deutschland"
        );
        assert_eq!(loc("München (Remote)", "Remote"), "Remote");
        assert_eq!(loc("Berlin (Remote)", ""), "");
        // Ein langer Seitenort wird gekürzt, die Arbeitsform bleibt vollständig.
        let long = "Berlin, Hamburg, München, Köln, Frankfurt am Main, Stuttgart, Düsseldorf";
        let kept = page_location("Berlin (Remote)", long, 40);
        assert!(
            kept.ends_with(" (Remote)") && kept.chars().count() <= 40,
            "{kept}"
        );
    }

    #[test]
    fn company_loses_its_prefix() {
        assert_eq!(
            short_company("von: Nordstern Personalberatung GmbH"),
            "Nordstern Personalberatung GmbH"
        );
        assert_eq!(short_company("von:emagine GmbH"), "emagine GmbH");
        assert_eq!(short_company("Firma: Muster GmbH"), "Muster GmbH");
        assert_eq!(short_company("Unternehmen: X AG"), "X AG");
        assert_eq!(
            short_company("von Rundstedt & Partner"),
            "von Rundstedt & Partner"
        );
        assert_eq!(short_company(""), "");
    }

    #[test]
    fn location_becomes_the_town() {
        assert_eq!(
            short_location("Am Mühlenweg 68, 27356 Rotenburg Wümme"),
            "Rotenburg Wümme"
        );
        assert_eq!(short_location("Am Mühlenweg 68, Rotenburg"), "Rotenburg");
        assert_eq!(short_location("D-68159 Mannheim"), "Mannheim");
        assert_eq!(short_location("68159 Mannheim"), "Mannheim");
    }

    #[test]
    fn location_loses_the_country() {
        assert_eq!(short_location("Berlin, Deutschland"), "Berlin");
        assert_eq!(short_location("Basel, CH"), "Basel");
        assert_eq!(short_location("Hessen Deutschland"), "Hessen");
        assert_eq!(short_location("Köln (Deutschland)"), "Köln");
        assert_eq!(
            short_location("Großraum Köln (Deutschland)"),
            "Großraum Köln"
        );
        assert_eq!(
            short_location("Berlin - Germany, 100% onsite work required (Deutschland)"),
            "Berlin"
        );
        assert_eq!(
            short_location("Berlin (onsite) (Deutschland)"),
            "Berlin (onsite)"
        );
        assert_eq!(short_location("Luxemburg (Luxemburg)"), "Luxemburg");
    }

    #[test]
    fn company_names_with_a_country_stay_whole() {
        for value in [
            "Telekom Deutschland GmbH",
            "Muster Consulting GmbH",
            "Ferrum Systems SE",
            "Nordwind Consulting",
        ] {
            assert_eq!(short_location(value), value);
            assert!(!looks_like_location(value), "{value}");
        }
    }

    #[test]
    fn short_values_of_the_other_portals_stay_untouched() {
        for value in [
            "Berlin (Vor Ort)",
            "Frankfurt am Main",
            "Münster",
            "Neckarsulm",
            "Villingen-Schwenningen (Vor Ort)",
            "Basel (Basel-Stadt)",
            "Zug (AG)",
            "Remote",
            "Hamburg (Hybrid)",
        ] {
            assert_eq!(short_location(value), value);
            assert_eq!(short_company(value), value);
        }
    }

    #[test]
    fn location_in_the_company_field_is_moved() {
        let pair = |c: &str, l: &str| split_company_location(c, l);
        let own = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(pair("D-20038 Hamburg", ""), own("", "Hamburg"));
        assert_eq!(
            pair("CH-4000 Basel (Basel-Stadt)", ""),
            own("", "Basel (Basel-Stadt)")
        );
        assert_eq!(
            pair(
                "Berlin - Germany, 100% onsite work required (Deutschland)",
                ""
            ),
            own("", "Berlin")
        );
        assert_eq!(pair("Luxemburg (Luxemburg)", ""), own("", "Luxemburg"));
        assert_eq!(
            pair("Nordwind Consulting", ""),
            own("Nordwind Consulting", "")
        );
        assert_eq!(
            pair("D-20038 Hamburg", "Hamburg"),
            own("D-20038 Hamburg", "Hamburg")
        );
        assert_eq!(
            pair(
                "von: Nordstern Personalberatung GmbH",
                "Am Mühlenweg 68, 27356 Rotenburg Wümme"
            ),
            own("Nordstern Personalberatung GmbH", "Rotenburg Wümme")
        );
    }

    /// Früher: Die Rechtsform-Sperre kam vor der Postleitzahl – ein Ort mit
    /// Kantonskürzel blieb in der Firmenspalte.
    #[test]
    fn postcode_at_start_wins_over_legal_form_in_parentheses() {
        let own = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            split_company_location("CH-6300 Zug (AG)", ""),
            own("", "Zug (AG)")
        );
        assert!(looks_like_location("A-1010 Wien"));
    }

    /// Früher: Jahreszahlen und Hausnummern galten als Postleitzahl.
    #[test]
    fn numbers_inside_company_names_are_not_postcodes() {
        for value in [
            "Vision 2030 Consulting",
            "Studio 54321 Media",
            "Projekt 2026 Team",
        ] {
            assert!(!looks_like_location(value), "{value}");
            let own = (value.to_string(), String::new());
            assert_eq!(split_company_location(value, ""), own);
        }
        // Firma mit Anschrift bleibt Firma (Rechtsform).
        assert!(!looks_like_location("Muster GmbH, 20038 Hamburg"));
        // Anschrift ohne Firma ist ein Ort.
        assert!(looks_like_location(
            "Am Mühlenweg 68, 27356 Rotenburg Wümme"
        ));
    }

    #[test]
    fn legal_forms() {
        for firm in [
            "Muster e.V.",
            "Handel e.K.",
            "Holding KGaA",
            "Acme Corp",
            "Beta B.V.",
            "Gamma S.A.",
        ] {
            assert!(has_legal_form(firm), "{firm}");
        }
        assert!(!has_legal_form("Zug (AG)"));
        assert!(!has_legal_form("Agentur Nord"));
    }

    #[test]
    fn long_values_are_cut_on_characters() {
        let long = "Ä".repeat(100);
        assert_eq!(short_company(&long).chars().count(), 80);
        assert_eq!(short_location(&long).chars().count(), 60);
    }
}
