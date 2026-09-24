//! Text building blocks with no network and no state: HTML to text, whitespace, file
//! names, company and location.

mod company_location;
mod filename;
mod html_text;

pub use company_location::{page_location, split_company_location};
pub(crate) use filename::job_file_name;
pub(crate) use html_text::{SKIP, html_to_text};

/// Smooth out whitespace: trim lines, collapse spaces, at most one blank line, no
/// leading/trailing blank.
///
/// Every kind of whitespace (non-breaking space, narrow space, tab) becomes a plain
/// space, line separators become newlines; invisible characters (zero-width
/// characters, directional control characters, byte order mark) and control
/// characters fall away - they would otherwise mess up search, file names and the
/// Excel output (a right-to-left character in a title could, say, make a file name
/// display falsified).
pub fn normalize(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut pending_blank = false;
    // "\r\n" is one line ending, not two.
    let text: String = text
        .replace("\r\n", "\n")
        .chars()
        .map(windows_1252)
        .collect();
    // U+0085 is deliberately missing here: `windows_1252` has already turned it into "…".
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

/// Cuts off after `max` characters (never mid-character) and then removes trailing
/// whitespace.
pub fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => text[..cut].trim_end().to_string(),
        None => text.to_string(),
    }
}

/// Removes the characters in `set` from both ends (like Python's `str.strip(chars)`).
pub(crate) fn strip_chars<'a>(text: &'a str, set: &str) -> &'a str {
    text.trim_matches(|c| set.contains(c))
}

/// C1 control characters (U+0080-U+009F) in web pages almost always come from
/// wrongly decoded Windows-1252 - like the HTML standard, they are read as that
/// encoding's characters ("22000 – aktuell", "950 €"). Unassigned ones stay control
/// characters.
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

/// A single line: all whitespace (including line breaks) becomes plain spaces.
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

/// A count with the matching form - a sentence never says "1 mails". The same rule
/// applies in the UI (`plural()` in `ui/js/ui.js`), so history and dialog sound alike.
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Invisible formatting and directional characters: not whitespace, but not visible
/// either.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'                      // soft hyphen
            | '\u{061C}'                // Arabic letter mark
            | '\u{180E}'                // Mongolian vowel separator
            | '\u{200B}'..='\u{200F}'   // zero-width characters, LRM, RLM
            | '\u{202A}'..='\u{202E}'   // LRE, RLE, PDF, LRO, RLO
            | '\u{2060}'..='\u{2064}'   // word joiner, invisible operators
            | '\u{2066}'..='\u{2069}'   // LRI, RLI, FSI, PDI
            | '\u{FEFF}' // byte order mark
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
        assert_eq!(one_line("a\u{81}b"), "a b", "unassigned: control character");
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

    /// Previously: non-breaking and narrow spaces stayed as they were, zero-width
    /// characters too - a search for "SAP FI" would not find "SAP\u{00A0}FI".
    #[test]
    fn normalize_handles_unicode_whitespace() {
        assert_eq!(normalize("SAP\u{00A0}FI\u{2009}/\u{202F}CO"), "SAP FI / CO");
        assert_eq!(
            normalize("\u{FEFF}Inter\u{200B}im\u{00AD} CFO"),
            "Interim CFO"
        );
        assert_eq!(normalize("eins\u{2028}zwei"), "eins\nzwei");
        // Directional control characters (LinkedIn wraps user text with these).
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
