//! Pull job entries out of an alert mail.
//!
//! Portals change their mail layouts occasionally; this relies not on CSS classes but on
//! what stays stable: the target URLs of the job links (`portal::job_link`) and the text
//! of the links, or the text behind them.
//!
//! Company and location sit behind the title. The search starts in the job's "card" - the
//! largest layout block around the link that holds no other job (table layouts like
//! LinkedIn's). That keeps text from the footer or from the next job's block from ever
//! becoming company or location. Where there is no such card (a flat layout with line
//! breaks), the text up to the next link counts - bounded by the link's own block
//! (`<div>`, `<p>`, `<td>`), so the neighbouring block never supplies company or location.
//!
//! Hand-made collection mails mix both: a titled link, then the next job's title as a
//! plain line above its auto-linked address. A line never becomes company or location
//! when it is another job's title in the same mail or reads like a job title itself
//! (gender tag, role word) - an empty value is better than a wrong one.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use ego_tree::NodeId;
use ego_tree::iter::Edge;
use regex::Regex;
use scraper::{ElementRef, Html, Node, Selector};

use crate::portal::{JobKey, JobLink, Portal, job_link};
use crate::text::{one_line, strip_chars};

/// A recognised job with raw values from the mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub link: JobLink,
    pub title: String,
    pub company: String,
    pub location: String,
}

/// At most this many text lines behind the title are considered for company/location.
const MAX_TRAILING: usize = 8;

use crate::text::SKIP;

/// Elements at whose boundaries a text line ends.
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

/// Paragraph boundaries: in the flat layout a job's block ends there.
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
    LazyLock::new(|| Selector::parse("a[href]").expect("valid selector"));

/// Link texts that are not a title. German and English link texts, do not translate.
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

/// Extra text behind the title that is neither company nor location. The word labels
/// only match as whole words (previously "Aktiva Consulting" or "Neumann GmbH" got
/// dropped) - and only when followed by a lowercase word ("Aktiv vor 2 Tagen"), not
/// before a name ("Neu Isenburg", "New York"). German mail patterns, do not translate.
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:(?i:vor \d+|\d+\s*(?:std|min|tag|stunde|minute|day|hour)|\d+ bewerber|be an early|erstellt:|von:$|ab (?:sofort|\w+ \d{4})|start:|beginn:|dauer:|laufzeit:|vertragsart:|/$)|(?i:aktiv|neu|new|promoted|anzeige|gesponsert|sofort|easy apply|einfach bewerben|schnell bewerben|remote möglich|bewerbungsfrist)(?:$|\s*[:!·|]|\s+[\p{Ll}\d]))",
    )
    .expect("valid pattern")
});

/// Matches "Ort: Hamburg // Vertragsart: ... // Start: ..." style field lines
/// (freelancermap). German field labels, do not translate.
static PLACE_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Ort|Standort|Location)\s*:\s*(.+?)\s*(?://|\||$)").expect("valid pattern")
});

/// A URL in plain text - also in brackets, quotes or behind "Link:" (splitting into
/// words would lose such links).
static PLAIN_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)https?://[^\s<>"'()\[\]{}„“”«»‹›]+"#).expect("valid pattern")
});

/// Portal address in plain text without `https://`.
static BARE_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(?:[a-z0-9-]+\.)*(?:linkedin\.com|freelance\.de|freelancermap\.(?:de|com))/[^\s<>"'()\[\]{}„“”«»‹›]*"#)
        .expect("valid pattern")
});

/// "Ort:" alone on a line - the value is on the next one. German field label, do not
/// translate.
static PLACE_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:Ort|Standort|Location)\s*:?$").expect("valid pattern"));

/// "Company · Location" on a single line (LinkedIn).
static SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+[·•|]\s+").expect("valid pattern"));

/// The gender tag of a job title: "(m/w/d)", "(w/m/d)", "(m/f/d)", "m/w/x", "(all genders)",
/// "(gn)". Single letters only - "Köln/Bonn" or "D/A/CH" are no tag. German mail patterns,
/// do not translate.
static GENDER_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[\s(\[,])(?:[mwfdx]|div)\s*[/|]\s*(?:[mwfdx]|div)(?:\s*[/|]\s*(?:[mwfdx]|div))?(?:$|[\s)\],])|\((?:all\s+genders?|alle\s+geschlechter|gn\*?)\)",
    )
    .expect("valid pattern")
});

/// Role words of job titles: English ones as whole words, German ones also as the end of a
/// compound ("Projektleiter", "SAP-Berater", "Softwareentwicklerin"). German and English
/// job words, do not translate.
static ROLE_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:engineer|developer|architect|consultant|manager|analyst|administrator|specialist|designer|scientist|tester|programmer|technician|coordinator|controller|accountant|auditor|recruiter|director|officer|product\s+owner|scrum\s+master|head\s+of|team\s*lead|tech\s*lead|lead|werkstudent\w*|praktikant\w*|trainee|intern)\b|\w*(?:leiter|leitung|berater|entwickler|architekt|ingenieur|referent|techniker|spezialist|koordinator|sachbearbeiter|projektmanager|assistent|programmierer|planer|prüfer|kaufmann|kauffrau)(?:in|innen|\(in\))?\b",
    )
    .expect("valid pattern")
});

/// A company's legal form or trade word: such a line names a company, even with a role
/// word in it ("Beispiel Engineering GmbH", "Ingenieurbüro Nord AG"). German and English
/// legal forms, do not translate.
static LEGAL_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:gmbh|mbh|ag|se|kg|kgaa|ohg|gbr|ug|e\.\s?v|ltd|limited|inc|llc|llp|plc|corp|s\.a|b\.v|n\.v|s\.r\.l|sarl|sas|spa|oy|ab|group|gruppe|holding|consulting|partners?|personalberatung|recruiting)\b",
    )
    .expect("valid pattern")
});

/// All jobs from the HTML and text parts of a mail, in order of first appearance,
/// without duplicates. The plain text fills in what the HTML misses (previously it was
/// only read when the HTML delivered nothing at all).
pub fn extract(html_parts: &[String], text_parts: &[String]) -> Vec<Found> {
    let mut jobs = Jobs::default();
    for html in html_parts {
        for found in extract_html(html) {
            jobs.add(found);
        }
    }
    // The plain text fills in jobs missing from the HTML - and an HTML find that was
    // left without a title (an auto-linked address with no line before it). Otherwise
    // it stays out: its title heuristic is weaker and must not overwrite a finished
    // HTML entry.
    for text in text_parts {
        for found in extract_plain(text) {
            let known = jobs.index.get(&found.link.key).copied();
            if known.is_none_or(|i| jobs.list[i].title.is_empty()) {
                jobs.add(found);
            }
        }
    }
    drop_foreign_titles(&mut jobs.list);
    jobs.list
}

/// A job whose company or location is another job's title (the collection mail's next
/// entry, read as this one's details) keeps neither: the pair came from the wrong spot.
fn drop_foreign_titles(list: &mut [Found]) {
    let fold = |text: &str| one_line(text).to_lowercase();
    let titles: HashSet<String> = list
        .iter()
        .map(|f| fold(&f.title))
        .filter(|t| !t.is_empty())
        .collect();
    for found in list {
        let own = fold(&found.title);
        let foreign = |value: &str| {
            let value = fold(value);
            !value.is_empty() && value != own && titles.contains(&value)
        };
        if foreign(&found.company) || foreign(&found.location) {
            found.company.clear();
            found.location.clear();
        }
    }
}

/// Does the text read like a job title - a gender tag, or a role word without a company's
/// legal form? Such a line is never a company or a location ("Senior Requirements
/// Engineer im Bankenumfeld (w/m/d)" behind the job before it).
pub(crate) fn looks_like_job_title(text: &str) -> bool {
    let text = one_line(text);
    GENDER_TAG.is_match(&text) || (ROLE_WORD.is_match(&text) && !LEGAL_FORM.is_match(&text))
}

/// Does the text carry a job title's gender tag ("(m/w/d)")? The sure sign alone: stored
/// values may come from a page, so the store drops only what no company name carries.
pub(crate) fn has_gender_tag(text: &str) -> bool {
    GENDER_TAG.is_match(&one_line(text))
}

/// A line that only names a portal ("freelancermap", "LinkedIn:" as a section heading of a
/// collection mail) is neither company nor location.
fn is_portal_name(text: &str) -> bool {
    let folded = one_line(text).to_lowercase();
    let folded = strip_chars(&folded, " :*-–—·•|");
    Portal::ALL.iter().any(|p| {
        p.search_terms().iter().any(|term| {
            folded == *term
                || folded
                    .strip_prefix(term)
                    .is_some_and(|rest| matches!(rest, ".de" | ".com"))
        })
    })
}

/// Collects finds; the same job seen more than once: the first real title stays
/// (previously a later, longer link text replaced a clean title). Company and location
/// are never overwritten (previously a later "Zum Projekt" duplicate erased an already
/// recognised location) and always come together from one spot in the mail - only when
/// the first mention had neither does the later one count.
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
        /// A paragraph boundary (p, h1-h6, hr, ...) preceded this link.
        para: bool,
        /// Innermost open block (div, p, td, ...) at this point.
        block: Option<NodeId>,
    },
    Text {
        line: String,
        node: NodeId,
        /// A paragraph boundary (p, h1-h6, hr, ...) preceded this line.
        para: bool,
    },
}

/// Which jobs an element holds - enough for the card search.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Holds<'a> {
    One(&'a JobKey),
    Many,
}

/// At most this many tokens behind a link are considered (a runtime bound).
const MAX_SCAN: usize = 200;

fn extract_html(html: &str) -> Vec<Found> {
    let doc = Html::parse_document(html);
    let (tokens, span) = tokenize(&doc);
    let inside = |node: NodeId, container: NodeId| is_inside(&span, node, container);

    // Per element: does it hold exactly one job or several? Linear - the walk upward
    // stops as soon as an ancestor already knows the answer.
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
    // Cards only count when at least one card holds details behind its link - otherwise
    // the element called a "card" is just a wrapper around the link (<b>, <p>, one table
    // row per detail) and the details sit beside it, not inside it.
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
        let first = texts.next().unwrap_or_default();
        // Gmail and Outlook turn a bare address in plain text into a link whose text
        // **is** the address - that is not a title. Then the title sits on the line
        // before it, and everything about the job sits before the link: the next block
        // starts right after it (previously its title became the company).
        let (title, company, location) = if is_url(first) {
            (line_before(&tokens, i), String::new(), String::new())
        } else if first.is_empty() {
            // No title, no details: in the flat layout the text behind it usually
            // belonged to the next job.
            (String::new(), String::new(), String::new())
        } else {
            // A link wrapping a whole card: the further lines inside the link are
            // company and location.
            let (company, location) =
                details_after(&tokens, i, link, first, texts.collect(), card, &span);
            (first.to_string(), company, location)
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

/// A text consisting of nothing but an address.
pub(crate) fn is_url(text: &str) -> bool {
    let line = one_line(text);
    let line = line.trim();
    !line.is_empty()
        && PLAIN_URL
            .find(line)
            .or_else(|| BARE_URL.find(line))
            .is_some_and(|m| m.start() == 0 && m.end() == line.len())
}

/// The text line immediately before the link, in the same paragraph - the title of a
/// block whose address got auto-linked. Empty when the link starts a paragraph or
/// nothing usable precedes it; the plain-text part of the mail then supplies the title.
fn line_before(tokens: &[Token], i: usize) -> String {
    if i == 0 || matches!(&tokens[i], Token::Link { para: true, .. }) {
        return String::new();
    }
    match &tokens[i - 1] {
        Token::Text { line, .. } if !is_generic(line) && !is_url(line) => line.clone(),
        _ => String::new(),
    }
}

/// Company and location behind a job link: inside the card, otherwise up to the next
/// job, a foreign link, or - after the first details - up to the next paragraph.
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
    // One paragraph per line (Outlook): then paragraphs don't separate jobs - the
    // sentence filter keeps the footer out.
    let mut paragraph_lines = false;
    let bounds = block_of(tokens, i, span).filter(|_| card.is_none());
    for (at, next) in tokens.iter().enumerate().skip(i + 1).take(MAX_SCAN) {
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
            // The same job linked again (logo, company line as a link, ...).
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
                // Flat layout: another link with text (login, sign out, ...) ends the job.
                if card.is_none() && !useful.is_empty() {
                    break;
                }
                trailing.extend(useful);
            }
            Token::Text { line, para, node } => {
                // Flat layout: whatever sits outside the link's block already belongs
                // to the next job - that applies even before the first detail
                // (previously the fetch read across the `<div>` boundary and took the
                // next block's title as the company).
                if let Some(block) = bounds
                    && !is_inside(span, *node, block)
                {
                    break;
                }
                // The line right above another job's auto-linked address is that job's
                // title (`line_before`), not this job's company - a hand-made collection
                // mail puts titled links and bare addresses in one block.
                if titles_next_job(tokens, at, &link.key) {
                    break;
                }
                // Flat layout: a new paragraph after the first details is no longer
                // this job (sign-off, footer).
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

/// Is the text token at `at` the title of the job linked right after it - a job other
/// than `own` whose link text is its bare address (and so takes the line before it as its
/// title, see `line_before`)?
fn titles_next_job(tokens: &[Token], at: usize, own: &JobKey) -> bool {
    match tokens.get(at + 1) {
        Some(Token::Link {
            job: Some(other),
            lines,
            para: false,
            ..
        }) if other.key != *own => lines
            .iter()
            .find(|l| !is_generic(l))
            .is_some_and(|first| is_url(first)),
        _ => false,
    }
}

/// The block (div, p, td, ...) that bounds the job - but only when it holds more than
/// just the link. When the link is alone in its block (one paragraph per line,
/// Outlook), the block says nothing about the job's boundary and the details sit
/// beside it.
fn block_of(tokens: &[Token], i: usize, span: &HashMap<NodeId, (usize, usize)>) -> Option<NodeId> {
    let Token::Link {
        block: Some(block), ..
    } = &tokens[i]
    else {
        return None;
    };
    let started = i > 0 && is_inside(span, node_of(&tokens[i - 1]), *block);
    started.then_some(*block)
}

/// Is `node` inside `container`? (Position in reading order, no ancestor walk.)
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

/// A block that can bound a job; `br` and `hr` are empty and enclose nothing.
fn is_block(name: &str) -> bool {
    BREAKS.contains(&name) && !matches!(name, "br" | "hr")
}

/// Flush the buffered text out as a line.
fn flush(tokens: &mut Vec<Token>, buffer: &mut String, node: &mut Option<NodeId>, para: &mut bool) {
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
}

/// Splits the document into link and text lines in reading order, and records each
/// node's position in that order (for the "is inside the card" check).
fn tokenize(doc: &Html) -> (Vec<Token>, HashMap<NodeId, (usize, usize)>) {
    let mut tokens = Vec::new();
    let mut span: HashMap<NodeId, (usize, usize)> = HashMap::new();
    let mut order = 0usize;
    let mut buffer = String::new();
    let mut buffer_node: Option<NodeId> = None;
    let mut skip_depth = 0usize;
    let mut inside_link: Option<NodeId> = None;
    let mut para = false;
    let mut blocks: Vec<NodeId> = Vec::new();

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
                        // Anchors without a target (bookmarks) are just text; a foreign
                        // link wrapping job links (a click counter around the whole
                        // card) is walked through.
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
                            para,
                            block: blocks.last().copied(),
                        });
                        para = false;
                        inside_link = Some(node.id());
                    }
                    Node::Element(el) if BREAKS.contains(&el.name()) => {
                        flush(&mut tokens, &mut buffer, &mut buffer_node, &mut para);
                        para |= PARAGRAPH.contains(&el.name());
                        if is_block(el.name()) {
                            blocks.push(node.id());
                        }
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
                        if blocks.last() == Some(&node.id()) {
                            blocks.pop();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    flush(&mut tokens, &mut buffer, &mut buffer_node, &mut para);
    (tokens, span)
}

/// A link's text as lines: blocks (div, p, td, ...) separate them, `<br>` does not - a
/// title with a line break stays one title ("Senior Controller<br>(m/w/d)").
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

// -------------------------------------------------------------------- Plain text

/// Plain-text mails: every URL that is a job link (even without `https://`). The title
/// is the first line of its block (since the last blank line or URL), the lines after
/// it are company and location - so with "title / company / location / link" the
/// location never becomes the title.
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
            // "SAP Berater: https://..." - the title sits before the link on the same line.
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

/// A line's URLs with their start position; portal addresses count even without a scheme.
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

// ---------------------------------------------------------------------- Helpers

fn is_generic(text: &str) -> bool {
    let folded = one_line(text).to_lowercase();
    let folded = folded.trim_end_matches([' ', '.', ':', '>', '»', '›']);
    folded.chars().count() < 3 || GENERIC.contains(&folded)
}

/// A sentence is neither company nor location (footer, sign-off, notice).
fn is_sentence(text: &str) -> bool {
    let words = text.split_whitespace().count();
    words > 8
        || text.chars().count() > 80
        || text.ends_with(['!', '?'])
        || (text.ends_with('.') && words >= 5)
}

/// Company and location (raw values) from the texts behind the title.
///
/// Layouts seen: LinkedIn "Company · Location" on one line; freelance.de "Company",
/// "D-68159 Mannheim"; freelancermap "von:", "Company", "Ort: Hamburg // Vertragsart: ...".
/// Cleaning (dropping "von:", address -> location) only happens for display and export.
/// A sentence ends the search - it belongs to the footer, not the job; so does a job
/// title - it starts the next job. A portal name alone (a section heading) is skipped.
pub(crate) fn split_details(texts: &[&str]) -> (String, String) {
    let mut parts: Vec<String> = Vec::new();
    let mut location = String::new();
    let mut place_next = false;
    for text in texts {
        let text = one_line(text);
        if text.is_empty() {
            continue;
        }
        // "Ort:" alone (e.g. set off in colour), the value follows on the next line.
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
        if is_sentence(&text) || looks_like_job_title(&text) {
            break;
        }
        if text.contains("//") || NOISE.is_match(&text) || is_portal_name(&text) {
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
        // freelancermap: raw values; cleaning happens for display.
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
        // freelance.de: only the location behind the title.
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

    /// Previously: "aktiv", "neu", "new" without a word boundary deleted real names.
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
            "footer is not company/location"
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

    /// Previously: the second mention of the same job erased the location.
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

    /// Gmail and Outlook auto-link a bare address in plain text - the link text is then
    /// the address. Previously it became the title and the next block's title became
    /// the company.
    #[test]
    fn an_auto_linked_url_is_no_title() {
        let block = |title: &str, url: &str| {
            format!(r#"<div dir="ltr">{title}<br><a href="{url}">{url}</a></div><div><br></div>"#)
        };
        let html = format!(
            "{}{}",
            block(
                "Senior Controller (m/w/d)",
                "https://www.linkedin.com/jobs/view/4468805907/"
            ),
            block(
                "Test Automation Engineer Lead",
                "https://www.freelancermap.de/projekt/test-automation-engineer-lead"
            ),
        );
        let found = extract(&[html], &[]);
        assert_eq!(
            titles(&found),
            ["Senior Controller (m/w/d)", "Test Automation Engineer Lead"]
        );
        for f in &found {
            assert_eq!((f.company.as_str(), f.location.as_str()), ("", ""));
        }
    }

    /// Without a usable line before the link the title stays empty in the HTML - then
    /// the plain-text part of the same mail fills it. A finished HTML title, however,
    /// stays as is.
    #[test]
    fn the_plain_text_fills_a_title_the_html_left_empty() {
        let url = "https://www.freelance.de/projekte/projekt-1292304-VMware";
        let text = format!("VMware Lead Solution Architect (m/f/d)\nMuster Consulting GmbH\n{url}");
        let bare = extract(
            &[format!(r#"<div><a href="{url}">{url}</a></div>"#)],
            std::slice::from_ref(&text),
        );
        assert_eq!(bare.len(), 1);
        assert_eq!(bare[0].title, "VMware Lead Solution Architect (m/f/d)");
        assert_eq!(bare[0].company, "Muster Consulting GmbH");
        let titled = extract(
            &[format!(
                r#"<div>Interim CFO<br><a href="{url}">{url}</a></div>"#
            )],
            &[text],
        );
        assert_eq!(titles(&titled), ["Interim CFO"]);
        assert_eq!(titled[0].company, "");
    }

    /// Flat layout: the link's block bounds the job - the neighbouring block supplies
    /// neither company nor location, even when the job had no detail yet.
    #[test]
    fn a_neighbouring_block_is_never_company() {
        let html = format!(
            r#"<div>Projekt 1<br><a href="{FD1}">SAP-Projektleiter</a></div>
               <div>Projekt 2<br><a href="{FD2}">PMO Manager</a><br>Projektbüro Nord GmbH</div>"#
        );
        let found = extract(&[html], &[]);
        assert_eq!(titles(&found), ["SAP-Projektleiter", "PMO Manager"]);
        assert_eq!(
            (found[0].company.as_str(), found[0].location.as_str()),
            ("", "")
        );
        assert_eq!(found[1].company, "Projektbüro Nord GmbH");
    }

    /// Links in brackets, quotes, behind "Link:" or in Markdown form are not lost.
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

    // ------------------------------------------------------ Portal layouts

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

    /// A wrapper around the title link (<b>, one line per detail, Outlook paragraphs,
    /// <h3>): the details sit beside the "card" and are still read.
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
        assert_eq!(details(&bold), expected, "<b> wrapper");
        let outlook = format!(
            r#"<div class="WordSection1"><p class="MsoNormal"><a href="{FM1}">Senior DevOps Engineer</a></p><p class="MsoNormal">von: Ferrum Systems SE</p>
               <p class="MsoNormal">Ort: München // Start: ab sofort</p><p class="MsoNormal"><a href="{FM2}">IT-Architekt</a></p>
               <p class="MsoNormal">von: Nordwind Consulting</p><p class="MsoNormal">Ort: Remote</p></div>"#
        );
        assert_eq!(details(&outlook), expected, "Outlook paragraphs");
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
            "one line per detail"
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

    /// Job links inside a foreign link (a click counter around the card), behind an
    /// unclosed anchor, or behind a bookmark are not lost.
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

    /// A footer and sign-off behind the last (or only) job are neither company nor location.
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

    /// Separators and labels in their own <span>: "·" is not a location, "Ort:" belongs
    /// to the value.
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

    /// A title with a <br> stays one title; company and location stay in their place.
    #[test]
    fn line_break_inside_the_title_link() {
        let html = r#"<table><tr><td><a href="https://www.linkedin.com/comm/jobs/view/4123456789/">Senior Controller<br>(m/w/d)</a>
                     <p>Musterwerke GmbH · Köln</p></td></tr></table>"#;
        assert_eq!(
            details(html),
            [row("Senior Controller (m/w/d)", "Musterwerke GmbH", "Köln")]
        );
    }

    /// Logo, title and company line each linked to the same job.
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

    /// A link without a title ("Zum Projekt" before the next job) takes no details -
    /// in the flat layout they belonged to the next job.
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

    /// Plain text "title / company / location / link": the title is the first line of
    /// the block, and portal addresses without `https://` count too.
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

    #[test]
    fn job_titles_are_recognised() {
        for title in [
            "Senior Requirements Engineer im Bankenumfeld (w/m/d)",
            "VMware Lead Solution Architect (m/f/d)",
            "Controller m/w/d",
            "Project Manager (all genders)",
            "Test Automation Engineer Lead",
            "SAP-Berater",
            "Projektleiterin Kabeltiefbau",
            "Scrum Master",
            "Head of Finance",
        ] {
            assert!(looks_like_job_title(title), "{title}");
        }
        for detail in [
            "Musterwerke GmbH",
            "Beispiel Engineering GmbH",
            "Nordwind Consulting",
            "Ingenieurbüro Nord",
            "Beispiel Personalberatung GmbH",
            "Michael Muster",
            "Köln, Nordrhein-Westfalen",
            "Köln/Bonn",
            "D/A/CH",
            "Hamburg (Hybrid)",
            "D-20038 Hamburg",
            "Remote",
            "Interim CFO",
        ] {
            assert!(!looks_like_job_title(detail), "{detail}");
        }
    }

    /// A hand-made collection mail (Gmail): titled links and bare addresses with the title
    /// on the line above, section headings per portal, all in one block. Previously the
    /// next job's title became the company, and the heading and title the pair.
    #[test]
    fn a_collection_never_takes_the_next_title_as_company() {
        let html = format!(
            r#"<div dir="ltr"><b>LinkedIn</b><br>
            <a href="https://www.linkedin.com/jobs/view/4990000011/">Interim Senior Controller (m/w/d)</a><br>
            Beispiel Personal GmbH · München<br>
            <a href="https://www.linkedin.com/jobs/view/4990000012/">Interim Werkscontroller (m/w/d)</a><br><br>
            <b>freelancermap</b><br>Test Automation Engineer Lead<br>
            <a href="https://www.freelancermap.de/projekt/test-automation-lead">https://www.freelancermap.de/projekt/test-automation-lead</a><br><br>
            <b>freelance.de</b><br>
            <a href="{FD1}">VMware Lead Solution Architect (m/f/d)</a><br>
            Rolle ohne Kennzeichen<br>
            <a href="{FD2}">{FD2}</a></div>"#
        );
        let found = extract(&[html], &[]);
        assert_eq!(
            titles(&found),
            [
                "Interim Senior Controller (m/w/d)",
                "Interim Werkscontroller (m/w/d)",
                "Test Automation Engineer Lead",
                "VMware Lead Solution Architect (m/f/d)",
                "Rolle ohne Kennzeichen",
            ]
        );
        let details: Vec<(&str, &str)> = found
            .iter()
            .map(|f| (f.company.as_str(), f.location.as_str()))
            .collect();
        assert_eq!(
            details,
            [
                ("Beispiel Personal GmbH", "München"),
                ("", ""),
                ("", ""),
                ("", ""),
                ("", "")
            ]
        );
    }

    /// Another job's title is never company or location - also when the plain text part
    /// supplied it and nothing about the line reads like a title.
    #[test]
    fn another_jobs_title_is_no_detail() {
        let mut list = vec![
            Found {
                link: job_link(FD1).unwrap(),
                title: "PMO Manager".into(),
                company: "Rolle ohne Kennzeichen".into(),
                location: "Berlin".into(),
            },
            Found {
                link: job_link(FD2).unwrap(),
                title: "Rolle ohne Kennzeichen".into(),
                company: "Nordlicht AG".into(),
                location: String::new(),
            },
        ];
        drop_foreign_titles(&mut list);
        assert_eq!(
            (list[0].company.as_str(), list[0].location.as_str()),
            ("", "")
        );
        assert_eq!(list[1].company, "Nordlicht AG");
    }

    /// Broken HTML with thousands of nested cards stays fast (linear).
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
