//! Company and location: short and in the right field.
//!
//! Portals write both inconsistently: freelancermap puts "von:" before the company and
//! gives the full address as the location; freelance.de puts only the location behind
//! the title ("D-20038 Hamburg"), which would otherwise land in the company column;
//! LinkedIn appends countries ("Köln (Deutschland)"). The raw values get stored - these
//! functions only clean up for display and export.

use std::sync::LazyLock;

use regex::Regex;

use super::{normalize, strip_chars, truncate_chars};

/// Filler words before the company name - only **with** a colon, otherwise real names
/// like "von Rundstedt & Partner" would get cut apart. German field labels, do not
/// translate.
static FIRM_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:von|firma|unternehmen|company|arbeitgeber|auftraggeber|kunde|endkunde|ansprechpartner)\s*:\s*",
    )
    .unwrap()
});

/// Work mode in a location ("Berlin (Remote)", "Hybrid", "Vor Ort"). German and English
/// terms, do not translate.
static WORKPLACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:remote|hybrid|vor ort|on-site|onsite|home ?office)\b").unwrap()
});

/// Location from the job page, without losing the work mode from the mail: LinkedIn and
/// freelancermap often name only the location on the page, but the mail says
/// "Berlin (Remote)" - for matching that is sometimes the only remote signal.
/// At most `max` characters - the location gets truncated, never the work mode.
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

/// A postcode, also with a country prefix ("D-68159", "A-1010", "CH-4000").
const POSTCODE: &str = r"(?:D-|A-|CH-)?\d{4,5}";

/// "Am Mühlenweg 68, 27356 Rotenburg Wümme" -> town name.
static ADDRESS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^(?:.*?,\s*)?{POSTCODE}\s+(\S.*)$")).unwrap());

/// An address without a postcode: "Am Mühlenweg 68, Rotenburg" -> "Rotenburg".
static STREET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?\d\s*[a-zA-Z]?,\s*(\S.*)$").unwrap());

/// Postcode plus town name at the start of the value.
static POSTCODE_AT_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^{POSTCODE}\s+\p{{L}}")).unwrap());

/// Postcode plus town name behind a comma (an address). A digit sequence only counts as
/// a postcode in these two spots - "Vision 2030 Consulting" is not a location
/// (previously any 4-5 digit number anywhere in the value counted).
static POSTCODE_AFTER_COMMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r",\s*{POSTCODE}\s+\p{{L}}")).unwrap());

/// Countries that show up behind the location in alert mails. Goes beyond the
/// German-speaking area because freelance.de also writes "Krakow (Polen)" (observed in
/// real mails). German and English country names, do not translate.
const COUNTRIES: [&str; 30] = [
    "deutschland",
    "germany",
    "österreich",
    "austria",
    "schweiz",
    "switzerland",
    "luxemburg",
    "luxembourg",
    "polen",
    "poland",
    "tschechien",
    "czechia",
    "niederlande",
    "netherlands",
    "belgien",
    "belgium",
    "frankreich",
    "france",
    "italien",
    "italy",
    "spanien",
    "spain",
    "portugal",
    "dänemark",
    "denmark",
    "schweden",
    "sweden",
    "norwegen",
    "norway",
    "finnland",
];
const COUNTRY_CODES: [&str; 4] = ["de", "at", "ch", "lu"];

/// A shape that only a location has: a country prefix without a postcode
/// ("D-D8/D9 D8/D9") or an empty pair of parentheses at the end ("D1 ()"). Both occur
/// like this in real freelance.de mails; no company name looks like that.
static PLACE_SHAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:D|A|CH)-\S|\(\s*\)$").unwrap());

/// "Bayern (Bayern)", "Genf (Genf)": a location with itself as the region in brackets.
fn repeats_itself_in_brackets(value: &str) -> bool {
    value
        .strip_suffix(')')
        .and_then(|v| v.rsplit_once('('))
        .is_some_and(|(head, tail)| {
            !head.trim().is_empty() && head.trim().eq_ignore_ascii_case(tail.trim())
        })
}

/// A country as its own word in the value ("Hessen Deutschland", "Berlin (Deutschland)").
static COUNTRY_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)(?:^|[\s(])(?:{})\b", COUNTRIES.join("|"))).unwrap()
});

/// Legal form: a value with one is a company. Spelled-out forms in any casing, the short
/// abbreviations only in upper case - "Muster AG" is a company.
static LEGAL_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i:\b(?:gmbh|mbh|ohg|gbr|partg|kgaa|e\.?\s?v\.?|e\.?\s?k\.?|inc|ltd|llc|sarl|plc|corp|s\.?r\.?l|s\.?p\.?a|b\.v|n\.v)\b)|\b(?:AG|SE|KG|UG|S\.A)\b",
    )
    .unwrap()
});

/// Bracketed extras like "(AG)" in "Zug (AG)" are cantons or notes, not a legal form.
static PARENTHESES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());

const EDGE_TRIM: &str = " -–—,(/|";

fn has_legal_form(value: &str) -> bool {
    LEGAL_FORM.is_match(&PARENTHESES.replace_all(value, " "))
}

fn flat(text: &str) -> String {
    normalize(text).replace('\n', " ")
}

/// Just the company name - filler like "von:" before it falls away (at most 80
/// characters).
pub fn short_company(text: &str) -> String {
    let value = flat(text);
    let value = FIRM_PREFIX.replace(&value, "");
    let value = strip_chars(&value, " -–—·|,");
    truncate_chars(value, 80)
}

/// Remove country suffixes: "Berlin, Deutschland", "Hessen Deutschland",
/// "Berlin - Germany, 100% onsite work required (Deutschland)". Companies stay
/// untouched ("Telekom Deutschland GmbH").
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

/// Just the location - an address gets shortened to the town name (at most 60
/// characters).
pub fn short_location(text: &str) -> String {
    let mut value = without_country(flat(text).trim());
    if let Some(town) = ADDRESS
        .captures(&value)
        .or_else(|| STREET.captures(&value))
        .and_then(|c| c.get(1))
    {
        value = town.as_str().to_string();
    }
    // "D1 ()": freelance.de appends an empty pair of parentheses when the region is
    // missing.
    if let Some(head) = value.strip_suffix(')')
        && let Some((head, tail)) = head.rsplit_once('(')
        && tail.trim().is_empty()
    {
        value = head.to_string();
    }
    truncate_chars(strip_chars(&value, " -–—,·|"), 60)
}

/// Does the value hold a location instead of a company?
///
/// Decided by shape, not by a priority order of bans: a postcode at the start means a
/// location (even "CH-6300 Zug (AG)"); otherwise a legal form makes the value a
/// company; otherwise an address behind a comma, a country with text before it, or a
/// pure location shape speak for a location.
pub fn looks_like_location(text: &str) -> bool {
    let value = flat(text);
    let value = value.trim();
    if POSTCODE_AT_START.is_match(value) {
        return true;
    }
    if has_legal_form(value) {
        return false;
    }
    if POSTCODE_AFTER_COMMA.is_match(value)
        || PLACE_SHAPE.is_match(value)
        || repeats_itself_in_brackets(value)
    {
        return true;
    }
    COUNTRY_WORD
        .find_iter(value)
        .any(|hit| !strip_chars(&value[..hit.start()], EDGE_TRIM).is_empty())
}

/// Company and location split apart, short, in the right field: if the company field
/// holds a location and the location field is empty, it moves over.
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
        // A long page location gets truncated, the work mode stays whole.
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

    /// Previously: the legal-form block came before the postcode check - a location
    /// with a canton abbreviation stayed in the company column.
    #[test]
    fn postcode_at_start_wins_over_legal_form_in_parentheses() {
        let own = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            split_company_location("CH-6300 Zug (AG)", ""),
            own("", "Zug (AG)")
        );
        assert!(looks_like_location("A-1010 Wien"));
    }

    /// Previously: years and house numbers counted as a postcode.
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
        // A company with an address stays a company (legal form).
        assert!(!looks_like_location("Muster GmbH, 20038 Hamburg"));
        // An address without a company is a location.
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
    /// Locations from real freelance.de mails that used to pass as a company: their
    /// company column is always empty, the value behind the title is the location.
    #[test]
    fn odd_freelance_places_are_places_not_firms() {
        for place in [
            "D-D8/D9 D8/D9",
            "D1 ()",
            "D76 ()",
            "Krakow (Polen)",
            "Bayern (Bayern)",
            "D-20038 Hamburg",
            "CH-4000 Basel (Basel-Stadt)",
            "D-80331 München, D-50667 Köln, D-10115 Berlin",
        ] {
            let (company, location) = split_company_location(place, "");
            assert_eq!(company, "", "{place} is not a company");
            assert!(!location.is_empty(), "{place} has a location");
        }
        // The empty pair of parentheses disappears, the region stays.
        assert_eq!(split_company_location("D1 ()", "").1, "D1");
        assert_eq!(split_company_location("Krakow (Polen)", "").1, "Krakow");
        // A company stays a company, even with a country in parentheses.
        let (company, location) = split_company_location("Muster (Deutschland) GmbH", "");
        assert_eq!(
            (company.as_str(), location.as_str()),
            ("Muster (Deutschland) GmbH", "")
        );
    }
}
