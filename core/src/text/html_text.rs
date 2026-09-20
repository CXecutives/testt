//! HTML zu Fließtext: Blockelemente werden Zeilen, Skripte und Stile fallen weg.

use ego_tree::iter::Edge;
use scraper::{Html, Node};

use super::normalize;

/// Elemente, deren Inhalt nie Text ist.
pub(crate) const SKIP: &[&str] = &[
    "script", "style", "head", "title", "noscript", "template", "svg", "iframe", "object",
];

/// Absatz-Elemente: davor und danach eine Leerzeile – so bleiben Abschnitte einer
/// Stellenbeschreibung auch in der Textdatei als Abschnitte lesbar.
const PARAGRAPH: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "dl",
    "fieldset",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "ul",
];

/// Zeilen-Elemente: eigene Zeile ohne Leerzeile. Tabellenzellen und Definitionslisten
/// zählen mit – sonst klebten Eckdaten wie „Berlin“ und „Vollzeit“ zu `BerlinVollzeit`
/// zusammen.
const LINE: &[&str] = &[
    "br",
    "dd",
    "div",
    "dt",
    "figcaption",
    "li",
    "td",
    "th",
    "tr",
];

/// Wandelt HTML (ganze Seite oder Ausschnitt) in normalisierten Text.
pub fn html_to_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let mut out = String::with_capacity(html.len() / 2);
    let mut skip_depth = 0usize;
    for edge in fragment.tree.root().traverse() {
        let (node, opening) = match edge {
            Edge::Open(node) => (node, true),
            Edge::Close(node) => (node, false),
        };
        match node.value() {
            Node::Element(el) if SKIP.contains(&el.name()) => {
                if opening {
                    skip_depth += 1;
                } else {
                    skip_depth = skip_depth.saturating_sub(1);
                }
            }
            _ if skip_depth > 0 => {}
            Node::Element(el) if PARAGRAPH.contains(&el.name()) => break_lines(&mut out, 2),
            Node::Element(el) if LINE.contains(&el.name()) => break_lines(&mut out, 1),
            // Am Zeilenanfang zählt Quelltext-Einrückung nicht: Sonst erzeugt hübsch
            // formatiertes HTML Leerzeilen zwischen Listenpunkten und Tabellenzellen.
            Node::Text(text) if opening => {
                let at_line_start = {
                    let tail = out.trim_end_matches([' ', '\t']);
                    tail.is_empty() || tail.ends_with('\n')
                };
                out.push_str(if at_line_start {
                    text.trim_start()
                } else {
                    text
                });
            }
            _ => {}
        }
    }
    normalize(&out)
}

/// Sorgt dafür, dass der Text mit mindestens `count` Zeilenumbrüchen endet (am Anfang
/// nichts). Leerzeichen am Zeilenende zählen dabei nicht als Inhalt.
fn break_lines(out: &mut String, count: usize) {
    out.truncate(out.trim_end_matches([' ', '\t']).len());
    if out.is_empty() {
        return;
    }
    let present = out.chars().rev().take_while(|&c| c == '\n').count();
    for _ in present..count {
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_and_lines() {
        let text = html_to_text(
            "<div><p>Erste Zeile</p><p>Zweite Zeile</p><ul><li>Punkt A</li><li>Punkt B</li></ul></div>",
        );
        assert_eq!(text, "Erste Zeile\n\nZweite Zeile\n\nPunkt A\nPunkt B");
    }

    #[test]
    fn source_whitespace_between_blocks_is_ignored() {
        let text = html_to_text("<ul>\n  <li>A</li>\n  <li>B</li>\n</ul>\n<p>\n  Text\n</p>");
        assert_eq!(text, "A\nB\n\nText");
    }

    /// formatiertes HTML (Einrückung, Zeilenumbrüche im Quelltext) ergibt
    /// denselben Text wie kompaktes.
    #[test]
    fn pretty_printed_html_equals_compact_html() {
        let pretty = "<ul>\n  <li>\n    <a href=\"/p\">Projekte finden</a>\n  </li>\n  <li>\n    <a href=\"/q\">Preise</a>\n  </li>\n</ul>";
        assert_eq!(html_to_text(pretty), "Projekte finden\nPreise");
        let table =
            "<table>\n<tr>\n<td>\n  Berlin\n</td>\n<td>\n  Vollzeit\n</td>\n</tr>\n</table>";
        assert_eq!(html_to_text(table), "Berlin\nVollzeit");
        assert_eq!(html_to_text("Zeile 1<br>\n  Zeile 2"), "Zeile 1\nZeile 2");
        assert_eq!(
            html_to_text("<p>SAP <b>S/4</b> <i>HANA</i></p>"),
            "SAP S/4 HANA"
        );
    }

    #[test]
    fn script_and_style_are_removed() {
        let text =
            html_to_text("<style>p {color:red}</style><p>Inhalt</p><script>alert(1)</script>");
        assert_eq!(text, "Inhalt");
    }

    #[test]
    fn entities_and_br() {
        assert_eq!(html_to_text("A &amp; B<br>Zeile 2"), "A & B\nZeile 2");
        assert_eq!(
            html_to_text("M&uuml;nchen&nbsp;&ndash;&#160;Remote"),
            "München – Remote"
        );
    }

    #[test]
    fn inline_tags_do_not_split_words() {
        assert_eq!(
            html_to_text("<p>SAP <b>S/4</b><i>HANA</i> Berater</p>"),
            "SAP S/4HANA Berater"
        );
    }

    /// Zellen einer Tabellenzeile bleiben getrennt.
    #[test]
    fn table_cells_are_separated() {
        assert_eq!(
            html_to_text("<table><tr><td>Berlin</td><td>Vollzeit</td></tr></table>"),
            "Berlin\nVollzeit"
        );
        assert_eq!(
            html_to_text("<dl><dt>Start</dt><dd>ab sofort</dd></dl>"),
            "Start\nab sofort"
        );
    }

    #[test]
    fn broken_html_still_yields_text() {
        assert_eq!(html_to_text("<div><p>offen<p>zweiter"), "offen\n\nzweiter");
        assert_eq!(html_to_text(""), "");
        assert_eq!(html_to_text("nur Text"), "nur Text");
    }

    #[test]
    fn deep_nesting_does_not_overflow() {
        let html = "<div>".repeat(5_000) + "tief" + &"</div>".repeat(5_000);
        assert_eq!(html_to_text(&html), "tief");
    }
}
