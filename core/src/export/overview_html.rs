//! `JobAlerts.html`: a small, self-contained overview to open in any browser - the pinned
//! jobs, or else the new matching jobs of the last mailbox run. Title, company, location,
//! portal, link, score, up to two met requirements and the exclusion reason in words; never
//! the full text.
//! Everything from mails and portals is HTML-escaped; the page loads nothing from outside.

use std::fmt::Write as _;
use std::path::Path;

use jiff::Timestamp;

use super::scale::{SCORE_SCALE, score_step};
use super::texts::Texts;
use crate::error::Result;
use crate::model::MatchStatus;
use crate::settings::Language;
use crate::store::JobRow;
use crate::text::split_company_location;

/// Colours of the app (tokens of the interface).
const STYLE: &str = "
:root { color-scheme: light; --cream: hsl(32 33% 96%); --ink: hsl(45 7% 17%);
  --slate: hsl(212 34% 37%); --coral: hsl(13 73% 63%); --coral-800: hsl(13 62% 45%);
  --high: hsl(152 50% 31%); --low: hsl(30 4% 42%); --line: hsl(32 20% 86%); }
body { margin: 0; padding: 32px 16px; background: var(--cream); color: var(--ink);
  font: 15px/22px system-ui, -apple-system, 'Segoe UI', sans-serif; }
main { max-width: 880px; margin: 0 auto; }
h1 { font-size: 26px; line-height: 34px; margin: 0 0 4px; color: var(--slate); }
p.meta { margin: 0 0 24px; color: var(--low); font-size: 13px; }
ol { list-style: none; margin: 0; padding: 0; }
li { display: flex; gap: 16px; padding: 16px; margin: 0 0 8px; background: #fff;
  border: 1px solid var(--line); border-radius: 12px; }
.score { flex: none; width: 48px; height: 48px; border-radius: 50%; display: grid;
  place-items: center; font-weight: 600; border: 3px solid var(--line); color: var(--ink); }
.none { color: var(--low); border-style: dashed; }
.job a { color: var(--slate); font-weight: 600; text-decoration: none; }
.job a:hover { text-decoration: underline; }
.sub { color: var(--low); font-size: 13px; }
.met, .excluded { margin: 6px 0 0; font-size: 13px; }
.tag { display: inline-block; margin: 2px 6px 0 0; padding: 0 8px; border-radius: 10px;
  font-size: 12px; font-weight: 600; background: var(--line); color: var(--ink); }
.met .tag:first-child { background: none; padding-left: 0; color: var(--low); }
.met .tag { background: hsl(152 40% 92%); color: var(--high); font-weight: 400; }
.excluded { color: var(--coral-800); }
.excluded .tag { background: hsl(13 73% 92%); color: var(--coral-800); }
";

/// Writes the overview in the app's language; `pinned`: the jobs are the pinned ones (else
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

/// The ring colour of each step, from the one table the app uses too.
fn scale_style() -> String {
    let mut css = String::new();
    for (class, colour) in STEP_CLASS.iter().zip(SCORE_SCALE) {
        let _ = writeln!(css, ".{class} {{ border-color: {}; }}", colour.css());
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

fn item(out: &mut String, job: &JobRow, texts: &Texts) {
    let (company, location) = split_company_location(&job.company, &job.location);
    // An excluded job keeps its score, but its ring shows no number (like in the app).
    let (class, score) = match &job.match_ {
        Some(m) if m.status == MatchStatus::Scored => {
            // The ring's colour step of the app (ten steps by decile, `scale.rs`).
            (STEP_CLASS[score_step(m.score)], m.score.to_string())
        }
        Some(m) if m.status == MatchStatus::Excluded => ("none", String::new()),
        _ => ("none", "–".to_string()),
    };
    let sub: Vec<&str> = [company.as_str(), location.as_str(), job.key.portal.label()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let _ = write!(
        out,
        "<li><div class=\"score {class}\" title=\"{}\">{}</div><div class=\"job\">\
         <a href=\"{}\" rel=\"noopener noreferrer\">{}</a><div class=\"sub\">{}</div>",
        esc(texts.html_match),
        esc(&score),
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
    use crate::model::{DescStatus, MatchRecord, Notice};
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
            html.contains(".s8 { border-color: hsl(96 35% 50%); }"),
            "the shared scale"
        );
        assert!(html.contains(texts::HTML_PINNED));
        assert!(!html.contains("Betreff"), "no mail data beyond the listing");
        assert!(
            !html.contains("http://") && !html.contains("<link"),
            "self-contained"
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

    /// An excluded job keeps its score, but the ring shows no number; the reason is a
    /// sentence, never the engine's code.
    #[test]
    fn an_excluded_job_shows_its_reason_in_words() {
        let excluded = record(MatchStatus::Excluded, 86, Some("dayRate"));
        let html = render(
            &[job("A", Some(excluded))],
            true,
            Timestamp::now(),
            &texts::DE,
        );
        assert!(html.contains("<div class=\"score none\" title=\"Passung\"></div>"));
        assert!(
            !html.contains(">86<") && !html.contains("score high"),
            "{html}"
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
