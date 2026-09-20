//! Welchem Portal gehört eine Mail – und ist sie überhaupt ein Job-Alert?

use std::sync::LazyLock;

use regex::Regex;

use super::extract::Found;
use super::parse::ParsedMail;
use crate::portal::Portal;

/// Betreff eines Alerts, in dem kein einziger Stellen-Link erkannt wurde (Layout-Wächter).
/// Enger als früher: „neue“, „new“, „passend“ oder ein einzelnes „Job“ („… zum neuen
/// Job gratulieren“) machten jede Netzwerk-Mail zum Alert ohne Einträge – und am Ende
/// meldete der Lauf fälschlich ein geändertes Mail-Layout.
static ALERT_SUBJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:jobs|job[\s-]?alerts?|jobalerts?|job-?benachrichtigung\w*|job[\s-]?(?:recommendations?|empfehlung\w*)|jobangebot\w*|stellen(?:angebot|anzeige|markt|börse|empfehlung)\w*|projekt(?:e|en|angebot|anfrage|vorschl|agent)\w*|projects?\s+(?:requests?|agent|alerts?)|projects|vakanz\w*)\b",
    )
    .expect("gültiges Muster")
});

/// Portal der Mail unter den gewählten (`allowed`); `found` enthält nur deren Einträge.
///
/// Zuerst der Absender (Original-Alert). Weitergeleitete Alerts kommen vom Nutzer selbst –
/// dann entscheiden die erkannten Stellen-Links, ohne Links nur ein Portalname im Betreff
/// oder Absender (ein „LinkedIn“-Symbol in der Fußzeile eines Newsletters macht noch
/// keinen Alert). Eine Sammelmail zählt zu dem gewählten Portal mit den meisten Einträgen.
pub(crate) fn portal_of(mail: &ParsedMail, found: &[Found], allowed: &[Portal]) -> Option<Portal> {
    let sender = mail.sender_domain().and_then(Portal::from_sender_domain);
    if let Some(portal) = sender.filter(|p| allowed.contains(p)) {
        return Some(portal);
    }
    if found.is_empty() {
        if sender.is_some() {
            return None;
        }
        let head = format!("{} {} {}", mail.subject, mail.sender, mail.sender_address);
        return most_mentioned(&head, allowed.to_vec());
    }
    // Die meisten Stellen-Links gewinnen; bei Gleichstand die häufiger genannte Marke.
    let links = |p: Portal| found.iter().filter(|f| f.link.key.portal == p).count();
    let top = allowed.iter().map(|&p| links(p)).max().unwrap_or(0);
    let leaders: Vec<Portal> = allowed
        .iter()
        .copied()
        .filter(|&p| links(p) == top)
        .collect();
    if let [only] = leaders.as_slice() {
        return Some(*only);
    }
    let everything = [&mail.subject, &mail.sender]
        .into_iter()
        .chain(&mail.html)
        .chain(&mail.text)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    most_mentioned(&everything, leaders.clone()).or(leaders.first().copied())
}

/// Ist eine Mail ohne erkannte Einträge trotzdem ein Alert (Layout geändert)?
pub(crate) fn is_alert_subject(subject: &str) -> bool {
    ALERT_SUBJECT.is_match(subject)
}

fn most_mentioned(text: &str, candidates: Vec<Portal>) -> Option<Portal> {
    let text = text.to_lowercase();
    let hits = |p: Portal| {
        p.search_terms()
            .iter()
            .map(|term| text.matches(term).count())
            .sum::<usize>()
    };
    let best = candidates
        .iter()
        .map(|&p| hits(p))
        .max()
        .filter(|&n| n > 0)?;
    candidates.into_iter().find(|&p| hits(p) == best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::job_link;

    fn mail(sender: &str, subject: &str, body: &str) -> ParsedMail {
        ParsedMail {
            subject: subject.into(),
            sender: sender.into(),
            sender_address: sender.to_lowercase(),
            html: vec![body.into()],
            ..ParsedMail::default()
        }
    }

    fn found(urls: &[&str]) -> Vec<Found> {
        urls.iter()
            .map(|u| Found {
                link: job_link(u).unwrap(),
                title: String::new(),
                company: String::new(),
                location: String::new(),
            })
            .collect()
    }

    #[test]
    fn sender_domain_wins() {
        let m = mail("jobalerts-noreply@linkedin.com", "Neue Jobs", "");
        assert_eq!(portal_of(&m, &[], &Portal::ALL), Some(Portal::LinkedIn));
        let m = mail("alert@mail.freelancermap.de", "x", "");
        assert_eq!(
            portal_of(&m, &[], &Portal::ALL),
            Some(Portal::Freelancermap)
        );
        let m = mail("kollege@firma.de", "Neue Termine", "Hallo");
        assert_eq!(portal_of(&m, &[], &Portal::ALL), None);
        // Absender-Portal nicht gewählt und keine Einträge der gewählten: keine Zuordnung.
        let m = mail("jobalerts-noreply@linkedin.com", "Neue Jobs", "");
        assert_eq!(portal_of(&m, &[], &[Portal::FreelanceDe]), None);
    }

    /// Weitergeleitet: Der Stellen-Link entscheidet, nicht die Fußzeile „Folgen Sie uns
    /// auf LinkedIn“.
    #[test]
    fn job_links_beat_brand_names() {
        let m = mail(
            "erika@example.com",
            "Projektvorschläge der Woche",
            "Folgen Sie uns auf LinkedIn. LinkedIn!",
        );
        let f = found(&["https://www.freelancermap.de/nproj/2971857.html"]);
        assert_eq!(portal_of(&m, &f, &Portal::ALL), Some(Portal::Freelancermap));
    }

    #[test]
    fn tie_on_links_goes_to_the_more_mentioned_portal() {
        let m = mail(
            "erika@example.com",
            "Fwd",
            "freelance.de freelance.de linkedin",
        );
        let f = found(&[
            "https://www.linkedin.com/jobs/view/4012345678/",
            "https://www.freelance.de/project/index.php?id=1255067",
        ]);
        assert_eq!(portal_of(&m, &f, &Portal::ALL), Some(Portal::FreelanceDe));
    }

    /// Ohne Stellen-Link zählt nur der Kopf (früher: ein Newsletter mit LinkedIn-Symbol
    /// in der Fußzeile wurde zum LinkedIn-Alert).
    #[test]
    fn without_links_only_the_head_counts() {
        let m = mail(
            "hello@mail.flownotes.example",
            "Unlocked: your new notetaker",
            r#"<img alt="linkedin"><a href="https://www.linkedin.com/company/flownotes/">linkedin</a>"#,
        );
        assert_eq!(portal_of(&m, &[], &Portal::ALL), None);
        let m = mail(
            "erika@example.com",
            "Ihre Job-Alerts von LinkedIn",
            "Kein Link",
        );
        assert_eq!(portal_of(&m, &[], &Portal::ALL), Some(Portal::LinkedIn));
    }

    #[test]
    fn alert_subjects() {
        for yes in [
            "5 neue Jobs für dich",
            "Neue Jobs für Sie",
            "Ihre Job-Alerts von LinkedIn",
            "Projektvorschläge der Woche",
            "Neue Projekte für Ihr Profil",
            "Neue Stellenangebote",
            "Ihre Jobempfehlungen",
            "New projects for you",
            // echte Alert-Betreffe, die fehlten.
            "Your job alert for controller",
            "Jobbenachrichtigung: Controller in Köln",
            "New project requests for you",
            "Your project agent found 3 matches",
            "Job recommendations for you",
        ] {
            assert!(is_alert_subject(yes), "{yes}");
        }
        for no in [
            "Max hat dir eine Nachricht geschickt",
            "Gratulieren Sie Max zum neuen Job",
            "Sie wurden in 5 Suchen gefunden",
            "Neue Termine",
            // das Verb „stellen“ und beliebige Projekt-Wörter nicht.
            "Stellen Sie Ihr Netzwerk vor",
            "Ihr Projektmanagement-Kurs wartet",
        ] {
            assert!(!is_alert_subject(no), "{no}");
        }
    }
}
