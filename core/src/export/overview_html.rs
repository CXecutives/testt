//! `JobAlerts.html`: a small, self-contained overview to open in any browser - the
//! favourites, or else the new matching jobs of the last mailbox run. Title, company,
//! location, portal, link, the score's ring, up to two met requirements and the exclusion
//! reason in words; never the full text.
//! Everything from mails and portals is HTML-escaped; the page loads nothing from outside.

use std::fmt::Write as _;
use std::path::Path;

use jiff::Timestamp;

use super::scale::{SCORE_SCALE, score_step};
use super::texts::Texts;
use crate::error::Result;
use crate::model::{DescStatus, MatchStatus};
use crate::settings::Language;
use crate::store::JobRow;
use crate::text::split_company_location;

/// Colours of the app (tokens of the interface): headings in ink, links in navy, an
/// exclusion in the danger red (coral means act or new, never an exclusion).
const STYLE: &str = "
:root { color-scheme: light; --cream: hsl(32 33% 96%); --ink: hsl(45 7% 17%);
  --navy: hsl(212 34% 37%); --muted: hsl(30 4% 35%); --low: hsl(30 4% 42%);
  --high: hsl(152 50% 31%); --line: hsl(45 22% 90%); --danger: hsl(4 55% 45%);
  --danger-soft: hsl(4 51% 95%); --danger-track: hsl(4 51% 56% / 0.25); }
body { margin: 0; padding: 32px 16px; background: var(--cream); color: var(--ink);
  font: 15px/22px system-ui, -apple-system, 'Segoe UI', sans-serif; }
main { max-width: 880px; margin: 0 auto; }
h1 { font-size: 26px; line-height: 34px; margin: 0 0 4px; color: var(--ink); }
p.meta { margin: 0 0 24px; color: var(--low); font-size: 13px; }
ol { list-style: none; margin: 0; padding: 0; }
li { display: flex; gap: 16px; padding: 16px; margin: 0 0 8px; background: #fff;
  border: 1px solid var(--line); border-radius: 12px; }
.score { position: relative; flex: none; width: 48px; height: 48px; display: grid;
  place-items: center; font-weight: 600; color: var(--ink); }
.ring { position: absolute; inset: 0; width: 100%; height: 100%; transform: rotate(-90deg); }
.ring circle { fill: none; stroke-width: 3; }
.track { stroke: var(--line); }
.value { stroke-linecap: round; }
.provisional .track, .none .track { stroke-dasharray: 2.5 2.5; }
.none, .unscorable { color: var(--low); }
.out { color: var(--danger); }
.out .track { stroke: var(--danger-track); }
.ban { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 2;
  stroke-linecap: round; }
.job a { color: var(--navy); font-weight: 600; text-decoration: none; }
.job a:hover { text-decoration: underline; }
.sub { color: var(--low); font-size: 13px; }
.met, .excluded, p.sub { margin: 6px 0 0; font-size: 13px; }
.tag { display: inline-block; margin: 2px 6px 0 0; padding: 0 8px; border-radius: 10px;
  font-size: 12px; font-weight: 600; background: var(--line); color: var(--ink); }
.met .tag:first-child { background: none; padding-left: 0; color: var(--low); }
.met .tag { background: hsl(152 40% 92%); color: var(--high); font-weight: 400; }
.excluded { color: var(--muted); }
.excluded .tag { background: var(--danger-soft); color: var(--danger); }
";

/// The track of a ring: circumference 100, so an arc's length is the score (like the app's
/// ring).
const TRACK: &str = "<svg class=\"ring\" viewBox=\"0 0 36 36\" aria-hidden=\"true\">\
                     <circle class=\"track\" cx=\"18\" cy=\"18\" r=\"15.9155\"/>";
/// The ban mark of an excluded job's ring: a circle and its diagonal.
const BAN: &str = "<svg class=\"ban\" viewBox=\"0 0 24 24\" aria-hidden=\"true\">\
                   <circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M5.6 5.6 18.4 18.4\"/></svg>";

/// Writes the overview in the app's language; `pinned`: the jobs are the favourites (else
/// the new matches).
pub fn write_overview_html(
    path: &Path,
    jobs: &[JobRow],
    pinned: bool,
    now: Timestamp,
    language: Language,
) -> Result<()> {
    super::write_atomic(
        path,
        render(jobs, pinned, now, Texts::of(language)).as_bytes(),
    )
}

/// The class of each colour step of a score ring (`.s0` ... `.s9`).
const STEP_CLASS: [&str; 10] = ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9"];

/// The arc colour of each step, from the one table the app uses too.
fn scale_style() -> String {
    let mut css = String::new();
    for (class, colour) in STEP_CLASS.iter().zip(SCORE_SCALE) {
        let _ = writeln!(css, ".{class} .value {{ stroke: {}; }}", colour.css());
    }
    css
}

fn render(jobs: &[JobRow], pinned: bool, now: Timestamp, texts: &Texts) -> String {
    let heading = if pinned {
        texts.html_pinned
    } else {
        texts.html_new
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{}</title><style>{STYLE}{}</style></head>\n<body><main>\
         <h1>{}</h1><p class=\"meta\">{} {}</p>\n",
        texts.language.code(),
        esc(texts.html_title),
        scale_style(),
        esc(heading),
        esc(texts.html_created),
        esc(&texts.moment(now)),
    );
    if jobs.is_empty() {
        let _ = write!(out, "<p>{}</p>", esc(texts.html_empty));
    } else {
        out.push_str("<ol>\n");
        for job in jobs {
            item(&mut out, job, texts);
        }
        out.push_str("</ol>");
    }
    out.push_str("</main></body></html>\n");
    out
}

/// The ring of a job, drawn like the app's: a scored job's arc is its share of 100 in the
/// colour of its step (ten steps by decile, `scale.rs`), on a dashed track while the score
/// comes from a teaser only. An excluded job keeps its score, but its ring is a pale red
/// track with the ban mark and no number; an unscorable job shows the track and a dash, one
/// not scored yet a dashed track and a dash.
fn ring(out: &mut String, job: &JobRow, texts: &Texts) {
    // Class, tooltip, the arc (the score) and what the centre shows.
    let (class, title, arc, centre) = match &job.match_ {
        Some(m) if m.status == MatchStatus::Scored => {
            let step = STEP_CLASS[score_step(m.score)];
            let class = if job.desc_status == DescStatus::Teaser {
                format!("{step} provisional")
            } else {
                step.to_owned()
            };
            (class, texts.html_match, m.score, m.score.to_string())
        }
        Some(m) if m.status == MatchStatus::Excluded => {
            ("out".to_owned(), texts.html_excluded, 0, BAN.to_owned())
        }
        Some(_) => (
            "unscorable".to_owned(),
            texts.html_unscorable,
            0,
            "–".to_owned(),
        ),
        None => ("none".to_owned(), texts.html_match, 0, "–".to_owned()),
    };
    let _ = write!(
        out,
        "<div class=\"score {class}\" title=\"{}\">{TRACK}",
        esc(title)
    );
    // No arc for a score of 0 either: a round cap alone would be a dot.
    if arc > 0 {
        let _ = write!(
            out,
            "<circle class=\"value\" cx=\"18\" cy=\"18\" r=\"15.9155\" \
             stroke-dasharray=\"{arc} 100\"/>"
        );
    }
    let _ = write!(out, "</svg>{centre}</div>");
}

fn item(out: &mut String, job: &JobRow, texts: &Texts) {
    let (company, location) = split_company_location(&job.company, &job.location);
    let sub: Vec<&str> = [company.as_str(), location.as_str(), job.key.portal.label()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    out.push_str("<li>");
    ring(out, job, texts);
    // A title opens its ad in a new tab: the overview stays where it is.
    let _ = write!(
        out,
        "<div class=\"job\"><a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{}</a>\
         <div class=\"sub\">{}</div>",
        esc(job.url.as_str()),
        esc(&job.title),
        esc(&sub.join(" · ")),
    );
    if let Some(m) = &job.match_ {
        // The met requirements the list keeps (at most two), each as a tag after a caption.
        if !m.top.is_empty() {
            let _ = write!(
                out,
                "<p class=\"met\"><span class=\"tag\">{}</span>",
                esc(texts.html_met)
            );
            for met in &m.top {
                let _ = write!(out, "<span class=\"tag\">{}</span>", esc(met));
            }
            out.push_str("</p>");
        }
        match m.status {
            MatchStatus::Excluded => {
                let _ = write!(
                    out,
                    "<p class=\"excluded\"><span class=\"tag\">{}</span>",
                    esc(texts.html_excluded)
                );
                // The reason in words - never the engine's code.
                if let Some(why) = m
                    .note
                    .as_ref()
                    .and_then(|n| texts.exclusion_reason(&n.code, &n.params))
                {
                    out.push_str(&esc(why));
                }
                out.push_str("</p>");
            }
            MatchStatus::Unscorable => {
                let _ = write!(out, "<p class=\"sub\">{}</p>", esc(texts.html_unscorable));
            }
            MatchStatus::Scored => {}
        }
    }
    out.push_str("</div></li>\n");
}

/// HTML escaping for text and attribute values.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::texts;
    use crate::model::{MatchRecord, Notice};
    use crate::portal::job_link;

    fn job(title: &str, record: Option<MatchRecord>) -> JobRow {
        let link = job_link("https://www.linkedin.com/jobs/view/4000000001/").unwrap();
        JobRow {
            key: link.key,
            url: link.url,
            title: title.into(),
            company: "Muster <GmbH>".into(),
            location: "Köln".into(),
            mail_date: None,
            mail_subject: "Betreff".into(),
            gmail_id: None,
            first_seen_at: Timestamp::now(),
            first_seen_run: 1,
            desc_status: DescStatus::Ok,
            desc_short: false,
            desc_closed: false,
            desc_len: 0,
            desc_fetched_at: None,
            desc_attempts: 0,
            desc_error: None,
            txt_name: None,
            desc_attempted_at: None,
            read_at: None,
            match_: record,
            match_rev: None,
            facts: None,
            pinned_at: None,
            archived_at: None,
            trashed_at: None,
            override_include: false,
        }
    }

    fn record(status: MatchStatus, score: u8, note: Option<&str>) -> MatchRecord {
        MatchRecord {
            status,
            score,
            note: note.map(|code| Notice {
                code: code.into(),
                params: serde_json::Map::new(),
            }),
            must_met: 1,
            must_total: 2,
            top: vec!["SAP <FI>".into(), "Konzernabschluss".into()],
            facts: crate::model::KeyFacts::default(),
            rank: 0,
        }
    }

    #[test]
    fn portal_data_is_escaped_and_no_full_text_appears() {
        let scored = record(MatchStatus::Scored, 83, None);
        let html = render(
            &[job("<script>alert(1)</script>", Some(scored))],
            true,
            Timestamp::now(),
            &texts::DE,
        );
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("Muster &lt;GmbH&gt;") && html.contains("SAP &lt;FI&gt;"));
        assert!(html.contains("class=\"score s8\"") && html.contains(">83<"));
        assert!(
            html.contains(".s8 .value { stroke: hsl(96 35% 50%); }"),
            "the shared scale"
        );
        assert!(html.contains(texts::HTML_PINNED));
        assert!(!html.contains("Betreff"), "no mail data beyond the listing");
        assert!(
            !html.contains("http://") && !html.contains("<link"),
            "self-contained"
        );
    }

    /// The rings speak like the app's: the arc is the score's share of 100 (a 5 is no full
    /// ring), a score from a teaser stands on a dashed track, an unscorable job shows a dash.
    #[test]
    fn a_ring_shows_the_share_of_its_score() {
        let mut teaser = job("B", Some(record(MatchStatus::Scored, 42, None)));
        teaser.desc_status = DescStatus::Teaser;
        let html = render(
            &[
                job("A", Some(record(MatchStatus::Scored, 83, None))),
                job("C", Some(record(MatchStatus::Scored, 5, None))),
                teaser,
                job("D", Some(record(MatchStatus::Unscorable, 0, None))),
                job("E", None),
            ],
            true,
            Timestamp::now(),
            &texts::DE,
        );
        let rings: Vec<&str> = html
            .split("<div class=\"score ")
            .skip(1)
            .map(|ring| &ring[..ring.find("</div>").unwrap()])
            .collect();
        assert_eq!(rings.len(), 5, "{html}");
        assert!(rings[0].starts_with("s8\"") && rings[0].contains("stroke-dasharray=\"83 100\""));
        assert!(rings[1].starts_with("s0\"") && rings[1].contains("stroke-dasharray=\"5 100\""));
        assert!(rings[2].starts_with("s4 provisional\"") && rings[2].ends_with(">42"));
        assert!(rings[3].starts_with("unscorable\" title=\"Nicht bewertbar\""));
        assert!(rings[4].starts_with("none\""));
        for ring in &rings[3..] {
            assert!(
                ring.ends_with("</svg>–") && !ring.contains("class=\"value\""),
                "{ring}"
            );
        }
        assert!(STYLE.contains(".provisional .track, .none .track { stroke-dasharray"));
        // The line of an unscorable job sits like the others (no paragraph margins of its own).
        assert!(html.contains("<p class=\"sub\">Nicht bewertbar</p>"));
        assert!(STYLE.contains(".met, .excluded, p.sub { margin: 6px 0 0;"));
    }

    /// A title opens the ad in a new tab and leaves the overview open; the heading is ink,
    /// the links navy (as in the app); coral, which means act or new, marks nothing here.
    #[test]
    fn links_open_beside_the_overview() {
        let html = render(
            &[job("A", Some(record(MatchStatus::Scored, 83, None)))],
            false,
            Timestamp::now(),
            &texts::DE,
        );
        assert!(html.contains(
            "<a href=\"https://www.linkedin.com/jobs/view/4000000001/\" target=\"_blank\" \
             rel=\"noopener noreferrer\">A</a>"
        ));
        assert!(STYLE.contains(
            "h1 { font-size: 26px; line-height: 34px; margin: 0 0 4px; color: var(--ink); }"
        ));
        assert!(STYLE.contains(".job a { color: var(--navy);"));
        assert!(
            !STYLE.contains("13 73% 63%") && !STYLE.contains("13 62% 45%"),
            "no coral"
        );
    }

    /// Plain words (CLAUDE.md): no "Erfüllt: A · B", no dash in the title.
    #[test]
    fn the_overview_speaks_plainly() {
        let html = render(
            &[job("A", Some(record(MatchStatus::Scored, 83, None)))],
            false,
            Timestamp::now(),
            &texts::DE,
        );
        assert!(!html.contains(&format!("{}:", texts::HTML_MET)), "{html}");
        assert!(html.contains("<span class=\"tag\">Konzernabschluss</span>"));
        assert!(html.contains(&format!("<title>{}</title>", texts::HTML_TITLE)));
        assert!(!html.contains(" – ") && !html.contains(" - "), "{html}");
    }

    /// An excluded job keeps its score, but the ring shows no number: a pale red track with
    /// the ban mark, unlike a job without a score. The reason is a sentence, never the
    /// engine's code, in the red of an exclusion (not the coral of "act or new").
    #[test]
    fn an_excluded_job_shows_its_reason_in_words() {
        let excluded = record(MatchStatus::Excluded, 86, Some("dayRate"));
        let html = render(
            &[job("A", Some(excluded))],
            true,
            Timestamp::now(),
            &texts::DE,
        );
        assert!(
            html.contains(&format!(
                "<div class=\"score out\" title=\"Ausgeschlossen\">{TRACK}</svg>{BAN}</div>"
            )),
            "{html}"
        );
        assert!(
            !html.contains(">86<") && !html.contains("class=\"value\""),
            "{html}"
        );
        assert!(STYLE.contains(".out .track { stroke: var(--danger-track); }"));
        assert!(
            STYLE.contains(
                ".excluded .tag { background: var(--danger-soft); color: var(--danger); }"
            )
        );
        assert!(html.contains(texts::HTML_EXCLUDED));
        assert!(html.contains("Der Tagessatz liegt unter dem Minimum im Profil."));
        assert!(!html.contains("dayRate"), "{html}");
        let unknown = record(MatchStatus::Excluded, 50, Some("somethingNew"));
        let html = render(
            &[job("A", Some(unknown))],
            true,
            Timestamp::now(),
            &texts::DE,
        );
        assert!(html.contains(texts::HTML_EXCLUDED) && !html.contains("somethingNew"));
    }

    #[test]
    fn an_empty_overview_says_so() {
        let html = render(&[], false, Timestamp::now(), &texts::DE);
        assert!(html.contains(texts::HTML_EMPTY) && html.contains(texts::HTML_NEW));
    }

    /// In English every word of the page is English; the job's own data stays as it came.
    #[test]
    fn the_overview_in_english() {
        let excluded = record(MatchStatus::Excluded, 86, Some("dayRate"));
        let at: Timestamp = "2026-09-19T12:05:00Z".parse().unwrap();
        let html = render(
            &[
                job("Interim CFO", Some(record(MatchStatus::Scored, 83, None))),
                job("B", Some(excluded)),
            ],
            true,
            at,
            &texts::EN,
        );
        assert!(html.contains("<html lang=\"en\">"), "{html}");
        for word in [
            texts::en::HTML_TITLE,
            texts::en::HTML_PINNED,
            texts::en::HTML_MET,
            texts::en::HTML_EXCLUDED,
            "The day rate is below the minimum in the profile.",
            "19/09/2026 14:05",
            "Konzernabschluss",
        ] {
            assert!(html.contains(word), "{word}: {html}");
        }
        for german in [texts::HTML_MET, texts::HTML_PINNED, "Tagessatz", "Erstellt"] {
            assert!(!html.contains(german), "{german}: {html}");
        }
        let html = render(&[], false, at, &texts::EN);
        assert!(html.contains(texts::en::HTML_EMPTY) && html.contains(texts::en::HTML_NEW));
    }
}
