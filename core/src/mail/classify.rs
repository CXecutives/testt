//! Which portal a mail belongs to - and whether it is a job alert at all.

use std::sync::LazyLock;

use regex::Regex;

use super::extract::Found;
use super::parse::ParsedMail;
use crate::portal::Portal;

/// Subject of an alert in which not a single job link was recognised (layout guard).
/// Narrower than before: "neue", "new", "passend" or a single "Job" ("... zum neuen Job
/// gratulieren") made every network mail an alert without entries - and in the end the run
/// wrongly reported a changed mail layout. German mail patterns, do not translate.
static ALERT_SUBJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:jobs|job[\s-]?alerts?|jobalerts?|job-?benachrichtigung\w*|job[\s-]?(?:recommendations?|empfehlung\w*)|jobangebot\w*|stellen(?:angebot|anzeige|markt|börse|empfehlung)\w*|projekt(?:e|en|angebot|anfrage|vorschl|agent)\w*|projects?\s+(?:requests?|agent|alerts?)|projects|vakanz\w*)\b",
    )
    .expect("valid pattern")
});

/// Forward prefixes of mail programs. German mail patterns, do not translate.
static FORWARD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:(?:re|aw|antw)\s*:\s*)*(?:fwd?|wg|weitergeleitet|weiterleitung)\s*:")
        .expect("valid pattern")
});

/// Portal of the mail among the chosen ones (`allowed`); `found` holds only their entries.
///
/// The sender first (original alert). Forwarded alerts come from the user themselves - then
/// the recognised job links decide, without links only a portal name in subject or sender
/// (a "LinkedIn" icon in the footer of a newsletter makes no alert). A collection mail
/// counts for the chosen portal with the most entries.
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
    // The most job links win; on a tie the brand named more often.
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

/// Is a mail without recognised entries an alert all the same (layout changed)?
pub(crate) fn is_alert_subject(subject: &str) -> bool {
    ALERT_SUBJECT.is_match(subject)
}

/// Could the mail be an alert of the chosen portals, judged by its head alone? Mails from a
/// portal's domain always (the body decides which portal's jobs they carry); from anyone
/// else only with a forward prefix, an alert word or a chosen portal's name in subject or
/// sender - a newsletter that mentions a portal in its footer is never loaded whole.
pub(crate) fn head_is_candidate(mail: &ParsedMail, allowed: &[Portal]) -> bool {
    if mail
        .sender_domain()
        .and_then(Portal::from_sender_domain)
        .is_some()
    {
        return true;
    }
    let head = format!("{} {}", mail.subject, mail.sender).to_lowercase();
    FORWARD.is_match(&mail.subject)
        || is_alert_subject(&mail.subject)
        || allowed
            .iter()
            .any(|p| p.search_terms().iter().any(|term| head.contains(term)))
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
        // Sender portal not chosen and no entries of the chosen ones: no portal.
        let m = mail("jobalerts-noreply@linkedin.com", "Neue Jobs", "");
        assert_eq!(portal_of(&m, &[], &[Portal::FreelanceDe]), None);
    }

    /// Forwarded: the job link decides, not the footer "Folgen Sie uns auf LinkedIn".
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

    /// Without a job link only the head counts (formerly a newsletter with a LinkedIn icon
    /// in its footer became a LinkedIn alert).
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
            // real alert subjects that were missing.
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
            // the verb "stellen" and any project words are not.
            "Stellen Sie Ihr Netzwerk vor",
            "Ihr Projektmanagement-Kurs wartet",
        ] {
            assert!(!is_alert_subject(no), "{no}");
        }
    }
}
