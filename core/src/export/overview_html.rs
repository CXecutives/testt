//! `JobAlerts.html`: a small, self-contained overview to open in any browser - the pinned
//! jobs, or else the new matching jobs of the last mailbox run. Title, company, location,
//! portal, link, score, met requirements and the exclusion note; never the full text.
//! Everything from mails and portals is HTML-escaped; the page loads nothing from outside.

use std::fmt::Write as _;
use std::path::Path;

use jiff::Timestamp;

use super::texts;
use crate::error::Result;
use crate::model::{Band, MatchStatus, band};
use crate::store::JobRow;
use crate::text::split_company_location;
use crate::time;

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
  place-items: center; font-weight: 600; border: 3px solid currentColor; }
.high { color: var(--high); } .mid { color: var(--coral-800); } .low { color: var(--low); }
.none { color: var(--low); border-style: dashed; }
.job a { color: var(--slate); font-weight: 600; text-decoration: none; }
.job a:hover { text-decoration: underline; }
.sub { color: var(--low); font-size: 13px; }
.met { margin: 4px 0 0; font-size: 13px; }
.excluded { margin: 4px 0 0; font-size: 13px; color: var(--coral-800); }
";

/// Writes the overview; `pinned`: the jobs are the pinned ones (else the new matches).
pub fn write_overview_html(
    path: &Path,
    jobs: &[JobRow],
    pinned: bool,
    now: Timestamp,
) -> Result<()> {
    super::write_atomic(path, render(jobs, pinned, now).as_bytes())
}

fn render(jobs: &[JobRow], pinned: bool, now: Timestamp) -> String {
    let heading = if pinned {
        texts::HTML_PINNED
    } else {
        texts::HTML_NEW
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"de\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{}</title><style>{STYLE}</style></head>\n<body><main>\
         <h1>{}</h1><p class=\"meta\">{} {}</p>\n",
        esc(texts::HTML_TITLE),
        esc(heading),
        esc(texts::HTML_CREATED),
        esc(&time::display(now)),
    );
    if jobs.is_empty() {
        let _ = write!(out, "<p>{}</p>", esc(texts::HTML_EMPTY));
    } else {
        out.push_str("<ol>\n");
        for job in jobs {
            item(&mut out, job);
        }
        out.push_str("</ol>");
    }
    out.push_str("</main></body></html>\n");
    out
}

fn item(out: &mut String, job: &JobRow) {
    let (company, location) = split_company_location(&job.company, &job.location);
    let (class, score) = match &job.match_ {
        Some(m) if m.status != MatchStatus::Unscorable => {
            let class = match band(m.score) {
                Band::High => "high",
                Band::Mid => "mid",
                Band::Low => "low",
            };
            (class, m.score.to_string())
        }
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
        esc(texts::HTML_MATCH),
        esc(&score),
        esc(job.url.as_str()),
        esc(&job.title),
        esc(&sub.join(" · ")),
    );
    if let Some(m) = &job.match_ {
        let met: Vec<String> = m.top.iter().take(3).map(|t| esc(t)).collect();
        if !met.is_empty() {
            let _ = write!(
                out,
                "<p class=\"met\">{}: {}</p>",
                esc(texts::HTML_MET),
                met.join(" · ")
            );
        }
        match m.status {
            MatchStatus::Excluded => {
                let why = m.note.as_ref().map(|n| n.code.as_str()).unwrap_or_default();
                let _ = write!(
                    out,
                    "<p class=\"excluded\">{} {}</p>",
                    esc(texts::HTML_EXCLUDED),
                    esc(why)
                );
            }
            MatchStatus::Unscorable => {
                let _ = write!(out, "<p class=\"sub\">{}</p>", esc(texts::HTML_UNSCORABLE));
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
            pinned_at: None,
            match_: record,
            match_rev: None,
        }
    }

    #[test]
    fn portal_data_is_escaped_and_no_full_text_appears() {
        let record = MatchRecord {
            status: MatchStatus::Excluded,
            score: 83,
            note: Some(Notice {
                code: "hardCriterion".into(),
                params: serde_json::Map::new(),
            }),
            must_met: 1,
            must_total: 2,
            top: vec!["SAP <FI>".into()],
        };
        let html = render(
            &[job("<script>alert(1)</script>", Some(record))],
            true,
            Timestamp::now(),
        );
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("Muster &lt;GmbH&gt;") && html.contains("SAP &lt;FI&gt;"));
        assert!(html.contains("class=\"score high\"") && html.contains(">83<"));
        assert!(html.contains(texts::HTML_EXCLUDED) && html.contains(texts::HTML_PINNED));
        assert!(!html.contains("Betreff"), "no mail data beyond the listing");
        assert!(
            !html.contains("http://") && !html.contains("<link"),
            "self-contained"
        );
    }

    #[test]
    fn an_empty_overview_says_so() {
        let html = render(&[], false, Timestamp::now());
        assert!(html.contains(texts::HTML_EMPTY) && html.contains(texts::HTML_NEW));
    }
}
