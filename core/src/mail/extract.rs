//! Job-Einträge aus einer Alert-Mail ziehen.
//!
//! Die Portale ändern ihre Mail-Layouts gelegentlich; verlassen wird sich deshalb nicht
//! auf CSS-Klassen, sondern auf das, was stabil bleibt: die Ziel-URLs der Stellen-Links
//! (`portal::job_link`) und den Text der Links bzw. den Text dahinter.
//!
//! Firma und Ort stehen hinter dem Titel. Gesucht wird zuerst in der „Karte“ des Jobs –
//! dem größten Layout-Block um den Link, der keinen anderen Job enthält (Tabellen-Layouts
//! wie bei LinkedIn). So kann Text aus der Fußzeile oder aus dem Block des nächsten Jobs
//! nicht mehr zu Firma oder Ort werden. Gibt es keine solche Karte (flaches Layout mit
//! Zeilenumbrüchen), gilt der Text bis zum nächsten Link.

use std::collections::HashMap;
use std::sync::LazyLock;

use ego_tree::NodeId;
use ego_tree::iter::Edge;
use regex::Regex;
use scraper::{ElementRef, Html, Node, Selector};

use crate::portal::{JobKey, JobLink, job_link};
use crate::text::{one_line, strip_chars};

/// Ein erkannter Job mit Rohwerten aus der Mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub link: JobLink,
    pub title: String,
    pub company: String,
    pub location: String,
}

/// Höchstens so viele Textzeilen hinter dem Titel werden für Firma/Ort betrachtet.
const MAX_TRAILING: usize = 8;

use crate::text::SKIP;

/// Elemente, an deren Grenzen eine Textzeile endet.
const BREAKS: &[&str] = &[
    "address",
    "article",
    "blockquote",
    "br",
    "dd",
    "div",
    "dl",
    "dt",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "ol",
    "p",
    "section",
    "table",
    "tbody",
    "td",
    "th",
    "tr",
    "ul",
];

/// Absatzgrenzen: Im flachen Layout endet dort der Block eines Jobs.
const PARAGRAPH: &[&str] = &[
    "article",
    "blockquote",
    "dl",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "ol",
    "p",
    "section",
    "ul",
];

static A_HREF: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a[href]").expect("gültiger Selektor"));

/// Link-Texte, die keinen Titel darstellen.
const GENERIC: &[&str] = &[
    "job ansehen",
    "jobs ansehen",
    "anzeigen",
    "ansehen",
    "mehr",
    "details",
    "mehr erfahren",
    "jetzt bewerben",
    "bewerben",
    "zum projekt",
    "zum job",
    "projekt ansehen",
    "projektdetails",
    "view job",
    "see job",
    "see jobs",
    "apply",
    "apply now",
    "hier",
    "hier klicken",
    "öffnen",
    "weiter",
    "alle jobs ansehen",
    "alle anzeigen",
    "alle projekte",
    "jetzt ansehen",
    "easy apply",
    "einfach bewerben",
    "mehr anzeigen",
    "details anzeigen",
    "weiter zum projekt",
    "zum projekt »",
    "projekt öffnen",
    "mehr lesen",
];

/// Zusatztexte hinter dem Titel, die weder Firma noch Ort sind. Die Wort-Etiketten gelten
/// nur als ganzes Wort (früher: „Aktiva Consulting“ oder „Neumann GmbH“ fielen weg) –
/// und nur, wenn klein weitergeschrieben wird („Aktiv vor 2 Tagen“), nicht vor einem
/// Namen („Neu Isenburg“, „New York“).
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:(?i:vor \d+|\d+\s*(?:std|min|tag|stunde|minute|day|hour)|\d+ bewerber|be an early|erstellt:|von:$|ab (?:sofort|\w+ \d{4})|start:|beginn:|dauer:|laufzeit:|vertragsart:|/$)|(?i:aktiv|neu|new|promoted|anzeige|gesponsert|sofort|easy apply|einfach bewerben|schnell bewerben|remote möglich|bewerbungsfrist)(?:$|\s*[:!·|]|\s+[\p{Ll}\d]))",
    )
    .expect("gültiges Muster")
});

/// „Ort: Hamburg // Vertragsart: … // Start: …“ (freelancermap).
static PLACE_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Ort|Standort|Location)\s*:\s*(.+?)\s*(?://|\||$)")
        .expect("gültiges Muster")
});

/// URL im Klartext – auch in Klammern, Anführungszeichen oder hinter „Link:“ (ein Zerlegen
/// in Wörter verlöre solche Links).
static PLAIN_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)https?://[^\s<>"'()\[\]{}„“”«»‹›]+"#).expect("gültiges Muster")
});

/// Portal-Adresse im Klartext ohne `https://`.
static BARE_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(?:[a-z0-9-]+\.)*(?:linkedin\.com|freelance\.de|freelancermap\.(?:de|com))/[^\s<>"'()\[\]{}„“”«»‹›]*"#)
        .expect("gültiges Muster")
});

/// „Ort:“ allein in einer Zeile – der Wert steht in der nächsten.
static PLACE_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:Ort|Standort|Location)\s*:?$").expect("gültiges Muster"));

/// „Firma · Ort“ in einer Zeile (LinkedIn).
static SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+[·•|]\s+").expect("gültiges Muster"));

/// Alle Jobs aus den HTML- und Textteilen einer Mail, in Reihenfolge des ersten
/// Auftretens, ohne Dubletten. Der Klartext ergänzt, was im HTML fehlt (früher: er
/// wurde nur gelesen, wenn das HTML gar nichts lieferte).
pub fn extract(html_parts: &[String], text_parts: &[String]) -> Vec<Found> {
    let mut jobs = Jobs::default();
    for html in html_parts {
        for found in extract_html(html) {
            jobs.add(found);
        }
    }
    // Der Klartext ergänzt nur Jobs, die im HTML fehlen – seine Titel-Heuristik ist
    // schwächer und soll keinen HTML-Eintrag füllen.
    for text in text_parts {
        for found in extract_plain(text) {
            if !jobs.index.contains_key(&found.link.key) {
                jobs.add(found);
            }
        }
    }
    jobs.list
}

/// Sammelt Funde; derselbe Job mehrmals: der erste echte Titel bleibt (früher: ein
/// späterer, längerer Linktext ersetzte einen sauberen Titel). Firma und Ort
/// werden nie überschrieben (früher: eine spätere „Zum Projekt“-Dublette löschte den
/// schon erkannten Ort) und stammen immer gemeinsam aus einer Stelle der Mail – nur wenn
/// die erste Nennung gar keine hatte, zählt die spätere.
#[derive(Default)]
struct Jobs {
    list: Vec<Found>,
    index: HashMap<JobKey, usize>,
}

impl Jobs {
    fn add(&mut self, found: Found) {
        match self.index.get(&found.link.key) {
            None => {
                self.index.insert(found.link.key.clone(), self.list.len());
                self.list.push(found);
            }
            Some(&i) => {
                let known = &mut self.list[i];
                if known.title.is_empty() {
                    known.title = found.title;
                }
                if known.company.is_empty() && known.location.is_empty() {
                    known.company = found.company;
                    known.location = found.location;
                }
            }
        }
    }
}

// ------------------------------------------------------------------------- HTML

enum Token {
    Link {
        job: Option<JobLink>,
        lines: Vec<String>,
        node: NodeId,
    },
    Text {
        line: String,
        node: NodeId,
        /// Vor dieser Zeile lag eine Absatzgrenze (p, h1–h6, hr …).
        para: bool,
    },
}

/// Welche Jobs ein Element enthält – genug für die Karten-Suche.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Holds<'a> {
    One(&'a JobKey),
    Many,
}

/// So viele Tokens hinter einem Link werden höchstens betrachtet (Laufzeitgrenze).
const MAX_SCAN: usize = 200;

fn extract_html(html: &str) -> Vec<Found> {
    let doc = Html::parse_document(html);
    let (tokens, span) = tokenize(&doc);
    let inside = |node: NodeId, container: NodeId| is_inside(&span, node, container);

    // Je Element: enthält es genau einen Job oder mehrere? Linear – der Weg nach oben endet,
    // sobald ein Vorfahr schon Bescheid weiß.
    let mut holds: HashMap<NodeId, Holds<'_>> = HashMap::new();
    for token in &tokens {
        let Token::Link {
            job: Some(link),
            node,
            ..
        } = token
        else {
            continue;
        };
        let Some(n) = doc.tree.get(*node) else {
            continue;
        };
        for ancestor in n.ancestors() {
            match holds.get(&ancestor.id()) {
                None => {
                    holds.insert(ancestor.id(), Holds::One(&link.key));
                }
                Some(Holds::One(k)) if *k == &link.key => break,
                Some(Holds::One(_)) => {
                    holds.insert(ancestor.id(), Holds::Many);
                }
                Some(Holds::Many) => break,
            }
        }
    }
    let card_of = |node: NodeId, key: &JobKey| {
        let mut card = None;
        for ancestor in doc.tree.get(node)?.ancestors() {
            let is_root = matches!(ancestor.value(), Node::Document | Node::Fragment)
                || ancestor
                    .value()
                    .as_element()
                    .is_some_and(|e| matches!(e.name(), "html" | "body"));
            if is_root || holds.get(&ancestor.id()) != Some(&Holds::One(key)) {
                break;
            }
            card = Some(ancestor.id());
        }
        card
    };

    let jobs: Vec<(usize, &JobLink, Option<NodeId>)> = tokens
        .iter()
        .enumerate()
        .filter_map(|(i, t)| match t {
            Token::Link {
                job: Some(link),
                node,
                ..
            } => Some((i, link, card_of(*node, &link.key))),
            _ => None,
        })
        .collect();
    // Karten gelten nur, wenn wenigstens eine Karte Angaben hinter ihrem Link enthält – sonst
    // ist das „Karte“ genannte Element nur eine Hülle um den Link (<b>, <p>, eine Tabellenzeile
    // je Angabe) und die Angaben stehen daneben.
    let card_layout = jobs.iter().any(|&(i, _, card)| {
        let Some(card) = card else { return false };
        let own = matches!(&tokens[i], Token::Link { lines, .. } if lines.iter().filter(|l| !is_generic(l)).count() > 1);
        own || tokens[i + 1..]
            .iter()
            .take(MAX_SCAN)
            .take_while(|t| inside(node_of(t), card))
            .any(|t| match t {
                Token::Text { .. } => true,
                Token::Link { job: None, lines, .. } => lines.iter().any(|l| !is_generic(l)),
                Token::Link { .. } => false,
            })
    });

    let mut out = Vec::new();
    for &(i, link, card) in &jobs {
        let card = card.filter(|_| card_layout);
        let Token::Link { lines, .. } = &tokens[i] else {
            continue;
        };
        let mut texts = lines.iter().map(String::as_str).filter(|l| !is_generic(l));
        let title = texts.next().unwrap_or_default().to_string();
        // Ohne Titel keine Angaben: Im flachen Layout gehörte der Text dahinter meist zum
        // nächsten Job.
        let (company, location) = if title.is_empty() {
            (String::new(), String::new())
        } else {
            // Link um eine ganze Karte: Die weiteren Zeilen im Link sind Firma und Ort.
            details_after(&tokens, i, link, &title, texts.collect(), card, &span)
        };
        out.push(Found {
            link: link.clone(),
            title,
            company,
            location,
        });
    }
    out
}

/// Firma und Ort hinter einem Stellen-Link: in der Karte, sonst bis zum nächsten Job,
/// fremden Link oder – nach den ersten Angaben – bis zum nächsten Absatz.
fn details_after<'a>(
    tokens: &'a [Token],
    i: usize,
    link: &JobLink,
    title: &str,
    mut trailing: Vec<&'a str>,
    card: Option<NodeId>,
    span: &HashMap<NodeId, (usize, usize)>,
) -> (String, String) {
    let mut seen_detail = !trailing.is_empty();
    // Absatz je Zeile (Outlook): Dann trennen Absätze keine Jobs – der Satz-Filter hält die
    // Fußzeile fern.
    let mut paragraph_lines = false;
    for next in tokens[i + 1..].iter().take(MAX_SCAN) {
        if trailing.len() >= MAX_TRAILING {
            break;
        }
        if let Some(card) = card
            && !is_inside(span, node_of(next), card)
        {
            break;
        }
        match next {
            Token::Link {
                job: Some(other), ..
            } if other.key != link.key => break,
            // Derselbe Job noch einmal verlinkt (Logo, Firmenzeile als Link …).
            Token::Link {
                job: Some(_),
                lines,
                ..
            } => {
                trailing.extend(
                    lines
                        .iter()
                        .map(String::as_str)
                        .filter(|l| !is_generic(l) && *l != title),
                );
            }
            Token::Link {
                job: None, lines, ..
            } => {
                let useful: Vec<&str> = lines
                    .iter()
                    .map(String::as_str)
                    .filter(|l| !is_generic(l))
                    .collect();
                // Flaches Layout: ein anderer Link mit Text (Login, Abmelden …) beendet den Job.
                if card.is_none() && !useful.is_empty() {
                    break;
                }
                trailing.extend(useful);
            }
            Token::Text { line, para, .. } => {
                // Flaches Layout: Ein neuer Absatz nach den ersten Angaben ist nicht mehr
                // dieser Job (Gruß, Fußzeile).
                if card.is_none() && *para && seen_detail && !paragraph_lines {
                    break;
                }
                if !seen_detail {
                    paragraph_lines = *para;
                }
                trailing.push(line);
                seen_detail = true;
            }
        }
    }
    split_details(&trailing)
}

/// Liegt `node` in `container`? (Lage in Lesereihenfolge, ohne Vorfahren-Suche.)
fn is_inside(span: &HashMap<NodeId, (usize, usize)>, node: NodeId, container: NodeId) -> bool {
    let (Some(&(start, _)), Some(&(from, to))) = (span.get(&node), span.get(&container)) else {
        return false;
    };
    from <= start && start < to
}

fn node_of(token: &Token) -> NodeId {
    match token {
        Token::Link { node, .. } | Token::Text { node, .. } => *node,
    }
}

/// Zerlegt das Dokument in Link- und Textzeilen in Lesereihenfolge und merkt je Knoten
/// seine Lage in dieser Reihenfolge (für „liegt in der Karte“).
fn tokenize(doc: &Html) -> (Vec<Token>, HashMap<NodeId, (usize, usize)>) {
    let mut tokens = Vec::new();
    let mut span: HashMap<NodeId, (usize, usize)> = HashMap::new();
    let mut order = 0usize;
    let mut buffer = String::new();
    let mut buffer_node: Option<NodeId> = None;
    let mut skip_depth = 0usize;
    let mut inside_link: Option<NodeId> = None;
    let mut para = false;

    let flush = |tokens: &mut Vec<Token>,
                 buffer: &mut String,
                 node: &mut Option<NodeId>,
                 para: &mut bool| {
        let line = one_line(buffer);
        if let (false, Some(n)) = (line.is_empty(), node.take()) {
            tokens.push(Token::Text {
                line,
                node: n,
                para: *para,
            });
            *para = false;
        }
        buffer.clear();
        *node = None;
    };

    for edge in doc.tree.root().traverse() {
        match edge {
            Edge::Open(node) => {
                span.insert(node.id(), (order, usize::MAX));
                order += 1;
                if inside_link.is_some() {
                    continue;
                }
                match node.value() {
                    Node::Element(el) if SKIP.contains(&el.name()) => skip_depth += 1,
                    _ if skip_depth > 0 => {}
                    Node::Element(el) if el.name() == "a" => {
                        let href = el.attr("href").map(str::trim).unwrap_or_default();
                        let job = job_link(href);
                        // Anker ohne Ziel (Textmarken) sind nur Text; ein fremder Link um
                        // Stellen-Links (Klick-Zähler um die ganze Karte) wird durchlaufen.
                        let wraps_jobs = || {
                            ElementRef::wrap(node).is_some_and(|e| {
                                e.select(&A_HREF)
                                    .any(|a| job_link(a.attr("href").unwrap_or_default()).is_some())
                            })
                        };
                        if href.is_empty() || (job.is_none() && wraps_jobs()) {
                            continue;
                        }
                        flush(&mut tokens, &mut buffer, &mut buffer_node, &mut para);
                        let lines = ElementRef::wrap(node).map(link_lines).unwrap_or_default();
                        tokens.push(Token::Link {
                            job,
                            lines,
                            node: node.id(),
                        });
                        para = false;
                        inside_link = Some(node.id());
                    }
                    Node::Element(el) if BREAKS.contains(&el.name()) => {
                        flush(&mut tokens, &mut buffer, &mut buffer_node, &mut para);
                        para |= PARAGRAPH.contains(&el.name());
                    }
                    Node::Text(text) => {
                        buffer.push_str(text);
                        buffer_node.get_or_insert(node.id());
                    }
                    _ => {}
                }
            }
            Edge::Close(node) => {
                if let Some(entry) = span.get_mut(&node.id()) {
                    entry.1 = order;
                }
                if inside_link == Some(node.id()) {
                    inside_link = None;
                    continue;
                }
                if inside_link.is_some() {
                    continue;
                }
                match node.value() {
                    Node::Element(el) if SKIP.contains(&el.name()) => {
                        skip_depth = skip_depth.saturating_sub(1);
                    }
                    Node::Element(el) if skip_depth == 0 && BREAKS.contains(&el.name()) => {
                        flush(&mut tokens, &mut buffer, &mut buffer_node, &mut para);
                        para |= PARAGRAPH.contains(&el.name());
                    }
                    _ => {}
                }
            }
        }
    }
    flush(&mut tokens, &mut buffer, &mut buffer_node, &mut para);
    (tokens, span)
}

/// Text eines Links als Zeilen: Blöcke (div, p, td …) trennen, `<br>` nicht – ein Titel mit
/// Zeilenumbruch bleibt ein Titel („Senior Controller<br>(m/w/d)“).
fn link_lines(link: ElementRef<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut skip_depth = 0usize;
    for edge in link.traverse() {
        let (node, open) = match edge {
            Edge::Open(n) => (n, true),
            Edge::Close(n) => (n, false),
        };
        match node.value() {
            Node::Element(el) if SKIP.contains(&el.name()) => {
                skip_depth = if open {
                    skip_depth + 1
                } else {
                    skip_depth.saturating_sub(1)
                };
            }
            _ if skip_depth > 0 => {}
            Node::Element(el) if el.name() == "br" => current.push(' '),
            Node::Element(el) if el.name() != "a" && BREAKS.contains(&el.name()) => {
                let done = one_line(&current);
                if !done.is_empty() {
                    lines.push(done);
                }
                current.clear();
            }
            Node::Text(text) if open => current.push_str(text),
            _ => {}
        }
    }
    let rest = one_line(&current);
    if !rest.is_empty() {
        lines.push(rest);
    }
    lines
}

// -------------------------------------------------------------------- Klartext

/// Klartext-Mails: jede URL, die ein Job-Link ist (auch ohne `https://`). Der Titel ist die
/// erste Zeile ihres Blocks (seit der letzten Leerzeile oder URL), die Zeilen danach sind
/// Firma und Ort – so wird bei „Titel / Firma / Ort / Link“ nicht der Ort zum Titel.
fn extract_plain(text: &str) -> Vec<Found> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (number, line) in lines.iter().enumerate() {
        for (start, url) in urls(line) {
            let Some(link) = job_link(&url) else { continue };
            let mut first = number;
            while first > 0 && number - first < 6 {
                let previous = lines[first - 1];
                if previous.trim().is_empty() || !urls(previous).is_empty() {
                    break;
                }
                first -= 1;
            }
            let mut block: Vec<String> = lines[first..number]
                .iter()
                .map(|l| one_line(l))
                .filter(|l| !l.is_empty() && !is_generic(l))
                .collect();
            // „SAP Berater: https://…“ – der Titel steht vor dem Link in derselben Zeile.
            if block.is_empty() {
                let before = one_line(line[..start].trim_end_matches([':', '-', '–', ' ']));
                if !before.is_empty() && !is_generic(&before) {
                    block.push(before);
                }
            }
            let title = block.first().cloned().unwrap_or_default();
            let details: Vec<&str> = block.iter().skip(1).map(String::as_str).collect();
            let (company, location) = split_details(&details);
            out.push(Found {
                link,
                title,
                company,
                location,
            });
        }
    }
    out
}

/// URLs einer Zeile samt Startposition; Adressen der Portale auch ohne Schema.
fn urls(line: &str) -> Vec<(usize, String)> {
    let trim = |s: &str| {
        s.trim_end_matches(['.', ',', ';', ':', '!', '?'])
            .to_string()
    };
    let mut found: Vec<(usize, usize, String)> = PLAIN_URL
        .find_iter(line)
        .map(|m| (m.start(), m.end(), trim(m.as_str())))
        .collect();
    for m in BARE_URL.find_iter(line) {
        if !found
            .iter()
            .any(|(s, e, _)| m.start() >= *s && m.start() < *e)
        {
            found.push((m.start(), m.end(), format!("https://{}", trim(m.as_str()))));
        }
    }
    found.sort_by_key(|(s, ..)| *s);
    found.into_iter().map(|(s, _, url)| (s, url)).collect()
}

// ---------------------------------------------------------------------- Hilfen

fn is_generic(text: &str) -> bool {
    let folded = one_line(text).to_lowercase();
    let folded = folded.trim_end_matches([' ', '.', ':', '>', '»', '›']);
    folded.chars().count() < 3 || GENERIC.contains(&folded)
}

/// Ein Satz ist weder Firma noch Ort (Fußzeile, Gruß, Hinweis).
fn is_sentence(text: &str) -> bool {
    let words = text.split_whitespace().count();
    words > 8
        || text.chars().count() > 80
        || text.ends_with(['!', '?'])
        || (text.ends_with('.') && words >= 5)
}

/// Firma und Ort (Rohwerte) aus den Texten hinter dem Titel.
///
/// Gesehene Layouts: LinkedIn „Firma · Ort“ in einer Zeile; freelance.de „Firma“,
/// „D-68159 Mannheim“; freelancermap „von:“, „Firma“, „Ort: Hamburg // Vertragsart: …“.
/// Bereinigt (ohne „von:“, Anschrift → Ort) wird erst für Anzeige und Export. Ein Satz
/// beendet die Suche – er gehört zur Fußzeile, nicht zum Job.
pub(crate) fn split_details(texts: &[&str]) -> (String, String) {
    let mut parts: Vec<String> = Vec::new();
    let mut location = String::new();
    let mut place_next = false;
    for text in texts {
        let text = one_line(text);
        if text.is_empty() {
            continue;
        }
        // „Ort:“ allein (z. B. farbig abgesetzt), der Wert folgt in der nächsten Zeile.
        if place_next {
            place_next = false;
            if location.is_empty() {
                let value = text.split("//").next().unwrap_or_default();
                location = strip_chars(value, " -–—").to_string();
            }
            continue;
        }
        if PLACE_LABEL.is_match(&text) {
            place_next = true;
            continue;
        }
        if let Some(place) = PLACE_FIELD.captures(&text).and_then(|c| c.get(1)) {
            if location.is_empty() {
                location = strip_chars(place.as_str(), " -–—").to_string();
            }
            continue;
        }
        if is_sentence(&text) {
            break;
        }
        if text.contains("//") || NOISE.is_match(&text) {
            continue;
        }
        for piece in SEPARATOR.split(&text) {
            let piece = strip_chars(piece, " -–—·•|/");
            if !piece.is_empty() && !NOISE.is_match(piece) {
                parts.push(piece.to_string());
            }
        }
        if parts.len() >= 2 {
            break;
        }
    }
    let company = parts.first().cloned().unwrap_or_default();
    if location.is_empty() {
        location = parts.get(1).cloned().unwrap_or_default();
    }
    (company, location)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::split_company_location;

    fn titles(found: &[Found]) -> Vec<&str> {
        found.iter().map(|f| f.title.as_str()).collect()
    }

    #[test]
    fn details_from_the_layouts_seen() {
        assert_eq!(
            split_details(&["Nordwind Consulting · Berlin (Vor Ort)"]),
            ("Nordwind Consulting".into(), "Berlin (Vor Ort)".into())
        );
        // freelancermap: Rohwerte; bereinigt wird für die Anzeige.
        let raw = split_details(&[
            "von: Nordstern Personalberatung GmbH",
            "Ort: Am Mühlenweg 68, 27356 Rotenburg Wümme // Start: ab sofort",
        ]);
        assert_eq!(
            split_company_location(&raw.0, &raw.1),
            (
                "Nordstern Personalberatung GmbH".into(),
                "Rotenburg Wümme".into()
            )
        );
        // freelance.de: hinter dem Titel nur der Ort.
        let raw = split_details(&["D-20038 Hamburg"]);
        assert_eq!(
            split_company_location(&raw.0, &raw.1),
            (String::new(), "Hamburg".into())
        );
    }

    #[test]
    fn place_field() {
        assert_eq!(split_details(&["Ort: Hamburg"]).1, "Hamburg");
        assert_eq!(
            split_details(&["Standort: Köln // Dauer: 6 Monate"]).1,
            "Köln"
        );
        assert_eq!(
            split_details(&["Ortsangabe: Berlin"]).0,
            "Ortsangabe: Berlin"
        );
    }

    /// Früher: „aktiv“, „neu“, „new“ ohne Wortgrenze löschten echte Namen.
    #[test]
    fn noise_labels_only_as_whole_words() {
        for keep in [
            "Aktiva Consulting",
            "Neumann GmbH",
            "New York",
            "Neu Isenburg",
            "Neu-Ulm",
            "Sofortis AG",
            "Anzeigenblatt Verlag",
        ] {
            assert!(!NOISE.is_match(keep), "{keep}");
        }
        for drop in [
            "Neu",
            "Aktiv vor 2 Tagen",
            "Promoted",
            "Easy Apply",
            "vor 3 Tagen",
            "2 Tagen",
            "50 Bewerber",
            "Erstellt: 12.09.2026",
            "Ab März 2026",
            "Gesponsert",
        ] {
            assert!(NOISE.is_match(drop), "{drop}");
        }
    }

    #[test]
    fn plain_text_finds_the_url_word_not_the_line_start() {
        let text = "Übersicht: https://www.freelance.de/newsletter/x Projekt https://www.freelance.de/projekte/projekt-1200001-Data?ref=mail.";
        let found = extract(&[], &[text.to_string()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].link.key.id, "1200001");
    }

    #[test]
    fn card_keeps_footer_text_out() {
        let html = r#"<table>
            <tr><td><a href="https://www.linkedin.com/jobs/view/4000000001/">Rolle A</a><p>Firma A · Köln</p></td></tr>
            <tr><td><a href="https://www.linkedin.com/jobs/view/4000000002/">Rolle B</a></td></tr>
            <tr><td><p>Impressum · Datenschutz</p></td></tr>
        </table>"#;
        let found = extract(&[html.to_string()], &[]);
        assert_eq!(titles(&found), ["Rolle A", "Rolle B"]);
        assert_eq!(
            (found[0].company.as_str(), found[0].location.as_str()),
            ("Firma A", "Köln")
        );
        assert_eq!(
            (found[1].company.as_str(), found[1].location.as_str()),
            ("", ""),
            "Fußzeile ist nicht Firma/Ort"
        );
    }

    #[test]
    fn link_around_a_whole_card() {
        let html = r#"<div><a href="https://www.freelancermap.de/nproj/2971857.html">
              <div>Senior DevOps Engineer</div><div>Ferrum Systems SE</div><div>München</div></a></div>
            <div><a href="https://www.freelancermap.de/nproj/2971858.html"><div>IT-Architekt</div></a></div>"#;
        let found = extract(&[html.to_string()], &[]);
        assert_eq!(titles(&found), ["Senior DevOps Engineer", "IT-Architekt"]);
        assert_eq!(
            (found[0].company.as_str(), found[0].location.as_str()),
            ("Ferrum Systems SE", "München")
        );
    }

    /// Früher: Die zweite Nennung desselben Jobs löschte den Ort.
    #[test]
    fn duplicate_link_does_not_erase_known_place() {
        let html = r#"<body>
            <a href="https://www.freelance.de/project/index.php?id=1255067">SAP-Projektleiter</a><br>D-20038 Hamburg<br>
            <a href="https://www.freelance.de/project/index.php?id=1255068">PMO</a><br>Nord GmbH<br>Berlin<br>
            <a href="https://www.freelance.de/projekte/projekt-1255067-sap">Zum Projekt</a><br>Andere Firma<br>Ort: X
        </body>"#;
        let found = extract(&[html.to_string()], &[]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].company, "D-20038 Hamburg");
        assert_eq!(found[0].location, "");
    }

    #[test]
    fn first_real_title_stays() {
        let html = r#"<a href="https://www.linkedin.com/jobs/view/4000000001/"><img src="logo.png"></a>
            <a href="https://www.linkedin.com/jobs/view/4000000001/">Controller (m/w/d)</a>
            <a href="https://www.linkedin.com/jobs/view/4000000001/">Controller (m/w/d) – jetzt mit einem Klick bewerben</a>"#;
        assert_eq!(
            titles(&extract(&[html.to_string()], &[])),
            ["Controller (m/w/d)"]
        );
    }

    /// Links in Klammern, Anführungszeichen, hinter „Link:“ oder in
    /// Markdown-Form gehen nicht verloren.
    #[test]
    fn plain_links_in_brackets_and_quotes() {
        let text = [
            "Data Engineer",
            "[https://www.freelance.de/projekte/projekt-1200001-Data]",
            "SAP Berater",
            "(https://www.freelance.de/projekte/projekt-1200002-SAP)",
            "PMO",
            "Link:https://www.freelance.de/projekte/projekt-1200003-PMO",
            "Architekt",
            "„https://www.freelance.de/projekte/projekt-1200004-A“",
            "Tester",
            "[Projekt](https://www.freelance.de/projekte/projekt-1200005-T).",
        ]
        .join(
            "
",
        );
        let ids: Vec<String> = extract(&[], &[text])
            .into_iter()
            .map(|f| f.link.key.id)
            .collect();
        assert_eq!(ids, ["1200001", "1200002", "1200003", "1200004", "1200005"]);
    }

    // ------------------------------------------------------ Layouts der Portale

    fn details(html: &str) -> Vec<(String, String, String)> {
        extract(&[html.to_string()], &[])
            .into_iter()
            .map(|f| {
                let (company, location) = split_company_location(&f.company, &f.location);
                (f.title, company, location)
            })
            .collect()
    }

    fn row(title: &str, company: &str, location: &str) -> (String, String, String) {
        (title.into(), company.into(), location.into())
    }

    const FM1: &str = "https://www.freelancermap.de/nproj/2971857.html";
    const FM2: &str = "https://www.freelancermap.de/nproj/2971858.html";
    const FD1: &str = "https://www.freelance.de/project/index.php?id=1255067";
    const FD2: &str = "https://www.freelance.de/project/index.php?id=1255068";

    /// Hülle um den Titel-Link (<b>, eine Zeile je Angabe, Outlook-Absätze, <h3>): Die
    /// Angaben stehen neben der „Karte“ und werden trotzdem gelesen.
    #[test]
    fn wrapper_layouts_keep_company_and_place() {
        let expected = [
            row("Senior DevOps Engineer", "Ferrum Systems SE", "München"),
            row("IT-Architekt", "Nordwind Consulting", "Remote"),
        ];
        let bold = format!(
            r#"<table><tr><td><b><a href="{FM1}">Senior DevOps Engineer</a></b><br>von: Ferrum Systems SE<br>Ort: München // Start: ab sofort<br>
               <b><a href="{FM2}">IT-Architekt</a></b><br>von: Nordwind Consulting<br>Ort: Remote // Start: ab sofort</td></tr></table>"#
        );
        assert_eq!(details(&bold), expected, "<b>-Hülle");
        let outlook = format!(
            r#"<div class="WordSection1"><p class="MsoNormal"><a href="{FM1}">Senior DevOps Engineer</a></p><p class="MsoNormal">von: Ferrum Systems SE</p>
               <p class="MsoNormal">Ort: München // Start: ab sofort</p><p class="MsoNormal"><a href="{FM2}">IT-Architekt</a></p>
               <p class="MsoNormal">von: Nordwind Consulting</p><p class="MsoNormal">Ort: Remote</p></div>"#
        );
        assert_eq!(details(&outlook), expected, "Outlook-Absätze");
        let rows = format!(
            r#"<table><tr><td><a href="{FD1}">SAP S/4HANA-Projektleiter</a></td></tr><tr><td>Muster Consulting GmbH</td></tr><tr><td>D-20038 Hamburg</td></tr>
               <tr><td><a href="{FD2}">PMO Manager</a></td></tr><tr><td>Projektbüro Nord GmbH</td></tr><tr><td>Berlin</td></tr></table>"#
        );
        assert_eq!(
            details(&rows),
            [
                row(
                    "SAP S/4HANA-Projektleiter",
                    "Muster Consulting GmbH",
                    "Hamburg"
                ),
                row("PMO Manager", "Projektbüro Nord GmbH", "Berlin")
            ],
            "eine Zeile je Angabe"
        );
        let heading = format!(
            r#"<h3><a href="{FD1}">SAP S/4HANA-Projektleiter</a></h3><p>Muster Consulting GmbH<br>D-20038 Hamburg</p>
               <h3><a href="{FD2}">PMO Manager</a></h3><p>Projektbüro Nord GmbH<br>Berlin</p>"#
        );
        assert_eq!(
            details(&heading)[1],
            row("PMO Manager", "Projektbüro Nord GmbH", "Berlin")
        );
    }

    /// Stellen-Links in einem fremden Link (Klick-Zähler um die Karte), hinter einem nicht
    /// geschlossenen Anker oder einer Textmarke gehen nicht verloren.
    #[test]
    fn nested_and_broken_anchors() {
        let tracker = format!(
            r#"<a href="https://click.freelancermap.de/ls/click?upn=abc"><table><tr><td><a href="{FM1}">Senior DevOps Engineer</a><p>Ferrum Systems SE · München</p></td></tr></table></a>
               <a href="https://click.freelancermap.de/ls/click?upn=def"><table><tr><td><a href="{FM2}">IT-Architekt</a></td></tr></table></a>"#
        );
        assert_eq!(
            titles(&extract(&[tracker], &[])),
            ["Senior DevOps Engineer", "IT-Architekt"]
        );
        let unclosed = String::from(
            r#"<p>Ihre Alerts – <a href="https://www.linkedin.com/comm/jobs/alerts">Einstellungen</p>
               <table><tr><td><a href="https://www.linkedin.com/comm/jobs/view/4123456789/">Senior Controller</a><p>Musterwerke GmbH · Köln</p></td></tr></table>"#,
        );
        assert_eq!(
            details(&unclosed),
            [row("Senior Controller", "Musterwerke GmbH", "Köln")]
        );
        let bookmark = format!(
            r#"<a href="{FM1}">Senior DevOps Engineer</a><br><a name="_Hlk180000001"></a>von: Ferrum Systems SE<br>Ort: München // Start: ab sofort<br>
               <a href="{FM2}">IT-Architekt</a><br>von: Nordwind Consulting"#
        );
        assert_eq!(
            details(&bookmark)[0],
            row("Senior DevOps Engineer", "Ferrum Systems SE", "München")
        );
    }

    /// Fußzeile und Gruß hinter dem letzten (oder einzigen) Job sind weder Firma noch Ort.
    #[test]
    fn footer_after_the_last_job_is_ignored() {
        let single = format!(
            r#"<table><tr><td><a href="{FD1}">SAP S/4HANA-Projektleiter (m/w/d)</a><br>D-20038 Hamburg</td></tr>
               <tr><td>Sie erhalten diese E-Mail, weil Sie Projektvorschläge abonniert haben.</td></tr></table>"#
        );
        assert_eq!(
            details(&single),
            [row("SAP S/4HANA-Projektleiter (m/w/d)", "", "Hamburg")]
        );
        let flat = format!(
            r#"<a href="{FD1}">SAP-Projektleiter</a><br>D-20038 Hamburg<br><a href="{FD2}">PMO Manager</a><br>D-10115 Berlin
               <p>Viel Erfolg bei der Projektsuche wünscht Ihnen Ihr freelance.de-Team</p><a href="https://www.freelance.de/logout.php">Abmelden</a>"#
        );
        assert_eq!(details(&flat)[1], row("PMO Manager", "", "Berlin"));
    }

    /// Trenner und Etiketten in eigenen <span>: „·“ ist kein Ort, „Ort:“ gehört zum Wert.
    #[test]
    fn inline_spans_are_not_lines() {
        let li = r#"<table><tr><td><a href="https://www.linkedin.com/jobs/view/4123456789/">Senior Controller</a>
                   <p>Musterwerke GmbH<span> · </span>Köln, Nordrhein-Westfalen</p></td></tr></table>"#;
        assert_eq!(
            details(li),
            [row(
                "Senior Controller",
                "Musterwerke GmbH",
                "Köln, Nordrhein-Westfalen"
            )]
        );
        let fm = format!(
            r#"<a href="{FM1}">Senior DevOps Engineer</a><br>Ferrum Systems SE<br><span style="color:#888">Ort:</span> München // Start: ab sofort"#
        );
        assert_eq!(
            details(&fm),
            [row(
                "Senior DevOps Engineer",
                "Ferrum Systems SE",
                "München"
            )]
        );
    }

    /// Ein Titel mit <br> bleibt ein Titel; Firma und Ort bleiben an ihrem Platz.
    #[test]
    fn line_break_inside_the_title_link() {
        let html = r#"<table><tr><td><a href="https://www.linkedin.com/comm/jobs/view/4123456789/">Senior Controller<br>(m/w/d)</a>
                     <p>Musterwerke GmbH · Köln</p></td></tr></table>"#;
        assert_eq!(
            details(html),
            [row("Senior Controller (m/w/d)", "Musterwerke GmbH", "Köln")]
        );
    }

    /// Logo, Titel und Firmenzeile jeweils als Link auf denselben Job.
    #[test]
    fn same_job_linked_three_times() {
        let html = r#"<table><tr><td><a href="https://www.linkedin.com/jobs/view/4123456789/?trk=logo"><img alt=""></a></td>
                     <td><a href="https://www.linkedin.com/jobs/view/4123456789/?trk=title">Senior Controller (m/w/d)</a><br>
                     <a href="https://www.linkedin.com/jobs/view/4123456789/?trk=company">Musterwerke GmbH · Köln</a></td></tr></table>"#;
        assert_eq!(
            details(html),
            [row("Senior Controller (m/w/d)", "Musterwerke GmbH", "Köln")]
        );
    }

    /// Ein Link ohne Titel („Zum Projekt“ vor dem nächsten Job) nimmt keine Angaben – die
    /// gehörten im flachen Layout zum nächsten Job.
    #[test]
    fn untitled_link_takes_no_details() {
        let html = format!(
            r#"<h3>SAP-Projektleiter</h3><p>Muster Consulting GmbH<br>Hamburg</p><a href="{FD1}">Zum Projekt</a>
               <h3>PMO Manager</h3><p>Projektbüro Nord GmbH<br>Berlin</p><a href="{FD2}">Zum Projekt</a>"#
        );
        for (_, company, location) in details(&html) {
            assert_eq!((company.as_str(), location.as_str()), ("", ""));
        }
    }

    /// Klartext „Titel / Firma / Ort / Link“: Titel ist die erste Zeile des Blocks, und
    /// Portal-Adressen ohne `https://` zählen auch.
    #[test]
    fn plain_text_blocks_and_bare_urls() {
        let text = "Senior Controller (m/w/d)\nMusterwerke GmbH\nKöln, Nordrhein-Westfalen\nDiesen Job anzeigen: https://www.linkedin.com/comm/jobs/view/4123456789/\n\n\
                    Data Engineer (Azure)\nwww.freelancermap.de/projektboerse/projekte/it/2971857-data-engineer.html\n\n\
                    SAP Berater: freelancermap.de/nproj/2971858.html";
        let found = extract(&[], &[text.to_string()]);
        let got: Vec<(&str, &str, &str)> = found
            .iter()
            .map(|f| (f.link.key.id.as_str(), f.title.as_str(), f.company.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "4123456789",
                    "Senior Controller (m/w/d)",
                    "Musterwerke GmbH"
                ),
                ("2971857", "Data Engineer (Azure)", ""),
                ("2971858", "SAP Berater", "")
            ]
        );
    }

    /// Kaputtes HTML mit tausenden verschachtelten Karten bleibt schnell (linear).
    #[test]
    fn deeply_nested_mail_stays_fast() {
        let jobs = 4_000;
        let html: String = (0..jobs)
            .map(|i| format!(r#"<div><a href="https://www.linkedin.com/jobs/view/{}/">Rolle</a><p>Firma · Ort</p>"#, 4_000_000_000_u64 + i))
            .collect::<Vec<_>>()
            .concat();
        let started = std::time::Instant::now();
        assert_eq!(extract(&[html], &[]).len(), usize::try_from(jobs).unwrap());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }
}
