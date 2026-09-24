//! Which portal a mail belongs to - and whether it is a job alert at all.

use std::sync::LazyLock;

use regex::Regex;

use super::extract::Found;
use super::parse::ParsedMail;
use crate::portal::Portal;
use crate::text::{html_to_text, one_line};

/// Words an alert subject may carry - enough to load a forwarded mail whole. Not enough for
/// a mail without a single recognised job to count as an alert (`looks_like_alert`): the
/// portals' promo mails speak of projects and agents too. Still no "neue", "new",
/// "passend" or a single "Job" ("... zum neuen Job gratulieren"). German mail patterns,
/// do not translate.
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

/// Subjects that only real alert mails carry - the formats of the checked-in alert
/// fixtures and the real mails they were built from: a count of new jobs or projects,
/// "Anzahl neue Projekte", a search agent's dated report, the weekly suggestions, a job
/// alert named as such. Narrower than `ALERT_SUBJECT` on purpose: this decides whether a
/// mail without a single recognised job is reported as a changed layout, and the portals'
/// promo mails speak of projects and agents too ("Der Projektagent findet für Sie
/// automatisch passende Projekte"). German mail patterns, do not translate.
static ALERT_MARKER_SUBJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
          \b\d+\s+(?:neue|new)\s+(?:jobs?|projekte?|projects?|stellen\w*|aufträge)\b
        | \banzahl\s+neue\s+projekte\b
        | ^(?:\s*(?:fwd?|wg|aw|re)\s*:)*\s*(?:neue|new)\s+(?:jobs|projekte|projects|projektanfragen|project\s+requests|stellenangebote)\s*$
        | \b(?:neue|new)\s+(?:jobs|projekte|projects|projektanfragen|project\s+requests|stellenangebote)\s+(?:für|for|in|zu|matching)\b
        | \bprojektvorschläge\s+der\s+woche\b
        | \bsuchagent\b.*\bvom\s+\d
        | \bjob[\s-]?benachrichtigung(?:en)?\s*(?::|für\b|zu\b)
        | \b(?:ihre?|your)\s+job[\s-]?alerts?\b
        | \bhat\s+eine\s+position\s+als\b
        | \byour\s+project\s+agent\s+found\b
        | \b(?:job\s+recommendations?\s+for|jobempfehlungen\s+für)\b
        ",
    )
    .expect("valid pattern")
});

/// Body sentences only real alert mails carry (header or footer of the alerts behind the
/// checked-in fixtures). German mail patterns, do not translate.
static ALERT_MARKER_BODY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
          \bist\s+fündig\s+geworden\b
        | \bneue\s+projekte\s+für\s+sie\s+zusammengestellt\b
        | \beinstellungen\s+ihres\s+suchagenten\b
        | \bprojektvorschläge\s+der\s+woche\b
        | \bwöchentlichen\s+projektvorschläge\b
        | \bzu\s+ihrer\s+gespeicherten\s+suche\s+gefunden\b
        | \bprojektagent-einstellungen\b
        | \bweil\s+sie\s+einen\s+projektagenten\s+eingerichtet\s+haben\b
        | \bneue\s+projekte\s+für\s+ihr\s+suchprofil\b
        | \bentsprechen\s+ihren\s+einstellungen\b
        | \be-mails\s+zu\s+jobbenachrichtigungen\b
        | \bweil\s+sie\s+job-?alerts\s+abonniert\s+haben\b
        | \bjobs?\s+(?:that\s+)?match(?:es)?\s+your\s+(?:job\s+)?(?:alert|preferences)\b
        ",
    )
    .expect("valid pattern")
});

/// Subjects of onboarding, promo and network mails the portals send from the same
/// addresses as their alerts: never an alert without a job link, whatever else they say.
/// German mail patterns, do not translate.
static PROMO_SUBJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
          \bwillkommen\w* | \bwelcome\b
        | \brabatt\w* | \bdiscount\b | \bgutschein\w*
        | \bregistrierung\b | \bregistration\b | \baktivierung\b | \bactivat\w*
        | \bbestätig\w* | \bconfirm\w* | \byour\s+pin\b
        | \bstudie\b | \bsurvey\b | \bumfrage\b | \bwebinar\w*
        | \bmitgliedschaft\b | \bmembership\b
        | \bveröffentlichen\s+sie\b | \bautomatisch\b
        | \bin\s+your\s+network\b | \byou\s+may\s+know\b | \bis\s+popular\b
        | \bsie\s+wollen\b | \?
        ",
    )
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

/// The portal a forwarded alert without recognised job links names most often in its body
/// (the forwarded header, logo texts). Only for mails that carry a real alert's markers
/// (`looks_like_alert`) - a newsletter's "LinkedIn" icon in the footer never makes a portal.
/// A mail from a portal's own address that is not chosen has no portal here either.
pub(crate) fn portal_in_body(mail: &ParsedMail, allowed: &[Portal]) -> Option<Portal> {
    if mail
        .sender_domain()
        .and_then(Portal::from_sender_domain)
        .is_some()
    {
        return None;
    }
    let body = mail
        .html
        .iter()
        .map(|html| html_to_text(html))
        .chain(mail.text.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");
    most_mentioned(&body, allowed.to_vec())
}

/// Could the subject belong to an alert? Broad - it only decides whether a forwarded mail
/// is worth loading whole; `looks_like_alert` decides about a mail without jobs.
pub(crate) fn is_alert_subject(subject: &str) -> bool {
    ALERT_SUBJECT.is_match(subject)
}

/// Is a mail without a single recognised job an alert all the same - one whose layout
/// changed? Only when it carries a real alert's markers (subject or body, see
/// `ALERT_MARKER_SUBJECT` and `ALERT_MARKER_BODY`) and its subject is no promo, onboarding
/// or network one. Everything else from the portals' addresses (welcome, discount,
/// "your profile", "someone you may know") is no alert and never raises the
/// changed-layout warning.
pub(crate) fn looks_like_alert(mail: &ParsedMail) -> bool {
    if PROMO_SUBJECT.is_match(&mail.subject) {
        return false;
    }
    if ALERT_MARKER_SUBJECT.is_match(&mail.subject) {
        return true;
    }
    mail.html
        .iter()
        .map(|html| html_to_text(html))
        .chain(mail.text.iter().cloned())
        .any(|body| ALERT_MARKER_BODY.is_match(&one_line(&body)))
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

    /// A mail without a recognised job is an alert only with a real alert's markers: the
    /// subject formats and body sentences of the real alerts - never a promo, onboarding or
    /// network subject from the same senders, whatever its body says.
    #[test]
    fn alert_markers() {
        let unlinked = |subject: &str, body: &str| {
            looks_like_alert(&mail("projekte@freelancermap.de", subject, body))
        };
        for subject in [
            "Controller: 2 neue Jobs in Köln",
            "5 neue Jobs für dich",
            "Neue Jobs für Sie",
            "Neue Jobs",
            "WG: Neue Projekte",
            "Neue Projektanfragen",
            "Neue Projekte für Ihre Suche „SAP“",
            "Neue Projekte für Ihr Profil",
            "Change - Anzahl neue Projekte: 2",
            "WG: Suchagent Beispiel vom 18.09.2026",
            "freelance.de – Ihre Projektvorschläge der Woche",
            "Jobbenachrichtigung: Leiter Qualitätsmanagement",
            "Your job alert for controller",
            "Ihre Job-Alerts von LinkedIn",
            "Beispiel AG hat eine Position als Controller (m/w/d) zu besetzen",
            "Your project agent found 3 matches",
            "Job recommendations for you",
        ] {
            assert!(unlinked(subject, "<p>Hallo</p>"), "{subject}");
        }
        for body in [
            "Ihr Suchagent „Test“ ist fündig geworden und hat 3 neue Projekte für Sie zusammengestellt:",
            "unser Projektagent hat neue Aufträge zu Ihrer gespeicherten Suche gefunden:",
            "<p>Neue Jobs in Deutschland</p><p>entsprechen Ihren Einstellungen.</p>",
            "Sie erhalten E-Mails zu Jobbenachrichtigungen.",
            "Sie erhalten diese E-Mail, weil Sie einen Projektagenten eingerichtet haben.",
        ] {
            assert!(unlinked("Fwd: Sammlung", body), "{body}");
        }
        let promo_body = "<p>Der Projektagent findet für Sie automatisch passende Projekte.                           Neue Projekte zu Ihrer gespeicherten Suche gefunden.</p>";
        for subject in [
            "Der freelancermap Projektagent findet für Sie automatisch passende Projekte, Erika",
            "Veröffentlichen Sie jetzt Ihr Profil auf freelancermap, Erika!",
            "Willkommen bei freelancermap!",
            "Aktivierung Ihres freelancermap-Accounts",
            "Willkommen bei freelance.de - 15% Rabatt auf EXPERT Mitgliedschaft",
            "Noch 24 Stunden: Ihr 15% Willkommens-Rabatt läuft morgen ab",
            "Freelancer-Studie 2026: Jetzt Ergebnisse lesen",
            "freelance.de - Vielen Dank für Ihre Registrierung",
            "freelance.de - Registrierung abschließen",
            "Max Muster, is popular in your network",
            "Someone at Beispiel AG you may know",
            "Sie wollen Führungskraft werden? Neue Jobs für Sie",
            "Erika, your pin is 482913. Please confirm your email address",
        ] {
            assert!(!unlinked(subject, promo_body), "{subject}");
            assert!(
                !unlinked(subject, "Sie erhalten E-Mails zu Jobbenachrichtigungen."),
                "{subject}"
            );
        }
        // A hand-made collection is an alert through its job links only.
        assert!(!unlinked(
            "Neue Jobs bei LinkedIn, freelancermap und freelance.de",
            "Hallo"
        ));
    }

    /// A forwarded alert without recognised links: the portal named in the forwarded part.
    #[test]
    fn portal_in_the_forwarded_body() {
        let m = mail(
            "ich@example.org",
            "WG: Beispielsuche - Anzahl neue Projekte: 2",
            r#"<p>Von: freelancermap Service &lt;office@freelancermap.de&gt;</p><img alt="freelancermap">"#,
        );
        assert_eq!(portal_of(&m, &[], &Portal::ALL), None);
        assert_eq!(
            portal_in_body(&m, &Portal::ALL),
            Some(Portal::Freelancermap)
        );
        // Never for a portal's own mail of a portal that is not chosen.
        let m = mail(
            "jobalerts-noreply@linkedin.com",
            "Neue Jobs",
            "freelance.de",
        );
        assert_eq!(portal_in_body(&m, &[Portal::FreelanceDe]), None);
    }
}
