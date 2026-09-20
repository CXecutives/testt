//! LinkedIn-Gastansicht (`/jobs-guest/jobs/api/jobPosting/<ID>`): ein HTML-Fragment mit dem
//! **vollen** Ausschreibungstext – „mehr anzeigen“ ist dort reines CSS – und dem Kopf der
//! Anzeige. Zeichengleich zur öffentlichen Seite (gemessen).

use std::sync::LazyLock;

use regex::Regex;
use scraper::{Html, Selector};

use super::{PageFields, Parsed};
use crate::text::{html_to_text, one_line};

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("gültiger Selektor")
}

static MARKUP: LazyLock<Selector> = LazyLock::new(|| selector("div.show-more-less-html__markup"));
static CLOSED: LazyLock<Selector> = LazyLock::new(|| selector("figure.closed-job"));
static TITLE: LazyLock<Selector> = LazyLock::new(|| selector("h2.topcard__title"));
static COMPANY: LazyLock<Selector> = LazyLock::new(|| selector("a.topcard__org-name-link"));
/// Ort; die Bewerberzahl trägt dieselbe Klasse, aber zusätzlich `--metadata`.
static PLACE: LazyLock<Selector> = LazyLock::new(|| {
    selector("span.topcard__flavor.topcard__flavor--bullet:not(.topcard__flavor--metadata)")
});

pub(super) fn parse(html: &str) -> Parsed {
    let doc = Html::parse_document(html);
    let text_of = |sel: &Selector| {
        doc.select(sel)
            .next()
            .map(|e| one_line(&e.text().collect::<String>()))
            .unwrap_or_default()
    };
    let closed = doc.select(&CLOSED).next().is_some();
    Parsed {
        text: doc
            .select(&MARKUP)
            .next()
            .map(|e| html_to_text(&e.inner_html())),
        closed,
        fields: PageFields {
            // Bei geschlossenen Anzeigen hängt LinkedIn „(No longer accepting …)“ an den
            // Titel – der Zusatz fällt weg; ist er unbekannt, bleibt der Titel aus der Mail.
            title: if closed {
                without_closed_suffix(&text_of(&TITLE))
            } else {
                text_of(&TITLE)
            },
            company: text_of(&COMPANY),
            location: text_of(&PLACE),
        },
    }
}

/// „Titel (No longer accepting applications …)“ → „Titel“; ein unbekannter Zusatz → leer.
fn without_closed_suffix(title: &str) -> String {
    static SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\s*\([^()]*(?:no longer accepting|nicht mehr angenommen)[^()]*\)\s*$")
            .expect("feste Regex")
    });
    SUFFIX
        .find(title)
        .map(|m| title[..m.start()].trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Aufbau wie die echte Gastansicht (gekürzt, erfundene Inhalte).
    pub(crate) fn page(text: &str, closed: bool) -> String {
        let closed = if closed {
            r#"<figure class="closed-job closed-job__flavor topcard__flavor-row"><figcaption class="closed-job__flavor--closed">Es werden keine Bewerbungen mehr angenommen.</figcaption></figure>"#
        } else {
            ""
        };
        format!(
            r#"<section class="top-card-layout"><a href="/jobs/view/1"><h2 class="top-card-layout__title topcard__title">Interim CFO (m/w/d){}</h2></a>
            <h4><div class="topcard__flavor-row"><span class="topcard__flavor"><a class="topcard__org-name-link topcard__flavor--black-link" href="https://de.linkedin.com/company/x">
              Nordlicht AG
            </a></span><span class="topcard__flavor topcard__flavor--bullet">
              Hamburg, Deutschland
            </span></div><div class="topcard__flavor-row"><span class="posted-time-ago__text topcard__flavor--metadata">vor 2 Tagen</span>
            <span class="num-applicants__caption topcard__flavor--metadata topcard__flavor--bullet">Über 200 Bewerber</span></div>{closed}</h4></section>
            <section class="description"><div class="show-more-less-html__markup show-more-less-html__markup--clamp-after-5
                relative overflow-hidden">{text}</div><button class="show-more-less-html__button">Mehr anzeigen</button></section>
            <ul class="description__job-criteria-list"><li class="description__job-criteria-item"><h3>Beschäftigungsverhältnis</h3><span>Vollzeit</span></li></ul>"#,
            if closed.is_empty() {
                ""
            } else {
                " (No longer accepting applications)"
            }
        )
    }

    #[test]
    fn open_posting() {
        let p = parse(&page(
            "<p><strong>Aufgaben</strong></p><ul><li>Finanzen</li><li>Controlling</li></ul>",
            false,
        ));
        assert_eq!(p.text.as_deref(), Some("Aufgaben\n\nFinanzen\nControlling"));
        assert!(!p.closed);
        assert_eq!(
            p.fields,
            PageFields {
                title: "Interim CFO (m/w/d)".into(),
                company: "Nordlicht AG".into(),
                location: "Hamburg, Deutschland".into(),
            }
        );
    }

    #[test]
    fn closed_posting_keeps_text_and_the_title_without_the_suffix() {
        let p = parse(&page("Text", true));
        assert!(p.closed);
        assert_eq!(p.text.as_deref(), Some("Text"));
        assert_eq!(p.fields.title, "Interim CFO (m/w/d)");
        assert_eq!(p.fields.company, "Nordlicht AG");
        let german = page("Text", true).replace(
            "(No longer accepting applications)",
            "(Bewerbungen werden nicht mehr angenommen)",
        );
        assert_eq!(parse(&german).fields.title, "Interim CFO (m/w/d)");
        let unknown =
            page("Text", true).replace("(No longer accepting applications)", "(geschlossen)");
        assert_eq!(
            parse(&unknown).fields.title,
            "",
            "unbekannter Zusatz: Titel aus der Mail"
        );
    }

    #[test]
    fn missing_container_is_no_text() {
        assert_eq!(parse("<html><body>Bitte anmelden</body></html>").text, None);
    }

    /// Echte, selbst geholte Seiten (privat, nicht eingecheckt): fehlen sie, wird übersprungen.
    #[test]
    fn real_pages_when_available() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private/pages");
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
        let (Some(open), Some(short), Some(closed)) = (
            read("linkedin-open-4468654483.html"),
            read("linkedin-short-4445179167.html"),
            read("linkedin-closed-4091550784.html"),
        ) else {
            eprintln!("übersprungen: private LinkedIn-Seiten fehlen");
            return;
        };
        // Keine echten Namen im Repo: geprüft wird, dass die Felder gefüllt werden.
        let open = parse(&open);
        assert!(open.text.as_ref().unwrap().chars().count() > 3_000);
        assert!(!open.closed);
        assert!(!open.fields.company.is_empty());
        assert!(!open.fields.location.is_empty());
        assert!(!open.fields.title.is_empty());
        let short = parse(&short);
        let n = short.text.as_ref().unwrap().chars().count();
        assert!((1..100).contains(&n), "legitim kurze Anzeige: {n} Zeichen");
        let closed = parse(&closed);
        assert!(closed.closed && closed.text.unwrap().chars().count() > 3_000);
        assert!(!closed.fields.title.contains(" - "), "Titel ohne Zusatz");
        assert!(!closed.fields.title.contains('('));
    }
}
