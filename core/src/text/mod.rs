//! Textbausteine ohne Netz und ohne Zustand: HTML zu Text, Weißraum, Dateinamen,
//! Firma und Ort.

mod company_location;
mod filename;
mod html_text;

pub use company_location::{page_location, split_company_location};
pub(crate) use filename::job_file_name;
pub(crate) use html_text::{SKIP, html_to_text};

/// Weißraum glätten: Zeilen trimmen, Leerzeichen bündeln, höchstens eine Leerzeile,
/// Rand leer.
///
/// Jede Art Weißraum (geschütztes Leerzeichen, schmale Leerzeichen, Tab) wird zu einem
/// Leerzeichen, Zeilentrenner werden Zeilenumbrüche; unsichtbare Zeichen
/// (Null-Breite-Zeichen, Richtungs-Steuerzeichen, Byte-Order-Mark) und Steuerzeichen
/// fallen weg – sie würden sonst Suchen, Dateinamen und die Excel-Ausgabe stören (ein
/// Rechts-nach-links-Zeichen im Titel könnte etwa einen Dateinamen verfälscht anzeigen).
pub fn normalize(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut pending_blank = false;
    // „\r\n“ ist ein Zeilenende, nicht zwei.
    let text: String = text
        .replace("\r\n", "\n")
        .chars()
        .map(windows_1252)
        .collect();
    // U+0085 fehlt hier bewusst: `windows_1252` hat daraus schon „…“ gemacht.
    for raw in text.split(['\n', '\r', '\u{0B}', '\u{0C}', '\u{2028}', '\u{2029}']) {
        let line = one_line(raw);
        if line.is_empty() {
            pending_blank = !lines.is_empty();
            continue;
        }
        if pending_blank {
            lines.push(String::new());
            pending_blank = false;
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Schneidet nach `max` Zeichen ab (nie mitten in einem Zeichen) und entfernt danach
/// überhängenden Weißraum.
pub fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => text[..cut].trim_end().to_string(),
        None => text.to_string(),
    }
}

/// Entfernt die Zeichen aus `set` an beiden Enden (wie Pythons `str.strip(chars)`).
pub(crate) fn strip_chars<'a>(text: &'a str, set: &str) -> &'a str {
    text.trim_matches(|c| set.contains(c))
}

/// C1-Steuerzeichen (U+0080–U+009F) stammen in Webseiten fast immer aus falsch
/// dekodiertem Windows-1252 – wie der HTML-Standard werden sie als dessen Zeichen gelesen
/// („22000 – aktuell“, „950 €“). Unbelegte bleiben Steuerzeichen.
fn windows_1252(c: char) -> char {
    match c {
        '\u{80}' => '€',
        '\u{82}' => '‚',
        '\u{83}' => 'ƒ',
        '\u{84}' => '„',
        '\u{85}' => '…',
        '\u{86}' => '†',
        '\u{87}' => '‡',
        '\u{88}' => 'ˆ',
        '\u{89}' => '‰',
        '\u{8A}' => 'Š',
        '\u{8B}' => '‹',
        '\u{8C}' => 'Œ',
        '\u{8E}' => 'Ž',
        '\u{91}' => '‘',
        '\u{92}' => '’',
        '\u{93}' => '“',
        '\u{94}' => '”',
        '\u{95}' => '•',
        '\u{96}' => '–',
        '\u{97}' => '—',
        '\u{98}' => '˜',
        '\u{99}' => '™',
        '\u{9A}' => 'š',
        '\u{9B}' => '›',
        '\u{9C}' => 'œ',
        '\u{9E}' => 'ž',
        '\u{9F}' => 'Ÿ',
        other => other,
    }
}

/// Eine Zeile: aller Weißraum (auch Zeilenumbrüche) wird zu einfachen Leerzeichen.
pub fn one_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars().map(windows_1252) {
        if is_invisible(c) {
            continue;
        }
        if c.is_whitespace() || c.is_control() {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    out
}

/// Anzahl mit passender Form – ein Satz sagt nie „1 Mails“. Dieselbe Regel gilt in der
/// Oberfläche (`plural()` in `ui/js/ui.js`), damit Verlauf und Dialog gleich klingen.
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Unsichtbare Format- und Richtungszeichen: kein Weißraum, aber auch nichts Sichtbares.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'                      // weiches Trennzeichen
            | '\u{061C}'                // arabische Richtungsmarke
            | '\u{180E}'                // mongolischer Vokaltrenner
            | '\u{200B}'..='\u{200F}'   // Null-Breite-Zeichen, LRM, RLM
            | '\u{202A}'..='\u{202E}'   // LRE, RLE, PDF, LRO, RLO
            | '\u{2060}'..='\u{2064}'   // Wortverbinder, unsichtbare Operatoren
            | '\u{2066}'..='\u{2069}'   // LRI, RLI, FSI, PDI
            | '\u{FEFF}' // Byte-Order-Mark
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_1252_leftovers_become_their_characters() {
        assert_eq!(
            normalize("FSSC 22000 \u{96} aktuell"),
            "FSSC 22000 – aktuell"
        );
        assert_eq!(
            one_line("Tagessatz 950 \u{80}, \u{84}gut\u{93}"),
            "Tagessatz 950 €, „gut“"
        );
        assert_eq!(one_line("6\u{96}8 Jahre"), "6–8 Jahre");
        assert_eq!(one_line("a\u{81}b"), "a b", "unbelegt: Steuerzeichen");
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("  a\n\n\n\n  b \n\n"), "a\n\nb");
        assert_eq!(normalize("  \n\t "), "");
        assert_eq!(normalize(""), "");
        assert_eq!(
            normalize("Zeile  mit \t  Lücken\r\nzwei\rdrei"),
            "Zeile mit Lücken\nzwei\ndrei"
        );
    }

    /// Früher: geschützte und schmale Leerzeichen blieben stehen, Null-Breite-
    /// Zeichen ebenso – „SAP\u{00A0}FI“ fand die Suche nach „SAP FI“ nicht.
    #[test]
    fn normalize_handles_unicode_whitespace() {
        assert_eq!(normalize("SAP\u{00A0}FI\u{2009}/\u{202F}CO"), "SAP FI / CO");
        assert_eq!(
            normalize("\u{FEFF}Inter\u{200B}im\u{00AD} CFO"),
            "Interim CFO"
        );
        assert_eq!(normalize("eins\u{2028}zwei"), "eins\nzwei");
        // Richtungs-Steuerzeichen (LinkedIn umschließt Nutzertexte damit).
        assert_eq!(normalize("Interim\u{200E} CFO"), "Interim CFO");
        assert_eq!(normalize("\u{2068}SAP FI\u{2069}"), "SAP FI");
        assert_eq!(one_line("Rechnung\u{202E}fdp.exe"), "Rechnungfdp.exe");
        assert_eq!(normalize("Steuer\u{0007}zeichen\u{0001}"), "Steuer zeichen");
    }

    #[test]
    fn one_line_and_truncate() {
        assert_eq!(
            one_line("  Senior\n Controller\t(m/w/d) "),
            "Senior Controller (m/w/d)"
        );
        assert_eq!(truncate_chars("Größenwahn", 4), "Größ");
        assert_eq!(truncate_chars("ab cd", 3), "ab");
        assert_eq!(truncate_chars("kurz", 10), "kurz");
        assert_eq!(truncate_chars("😀😀😀", 2), "😀😀");
    }
}
