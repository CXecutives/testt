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
        r"(?ix)
          \b(?:jobs|job[\s-]?alerts?|jobalerts?|job-?benachrichtigung\w*|job[\s-]?(?:recommendations?|empfehlung\w*)|jobangebot\w*|stellen(?:angebot|anzeige|markt|börse|empfehlung)\w*|projekt(?:e|en|angebot|anfrage|vorschl|agent)\w*|projects?\s+(?:requests?|agent|alerts?)|projects|vakanz\w*)\b
        | \bis\s+hiring\b
        | \bsucht\b.*(?:\b[mwfd]\s*/\s*[mwfd]\s*/\s*[mwfd]\b|\b(?:mitarbeiter|leiter|manager|berater|consultant|entwickler|controller|referent|ingenieur|architekt|interim|freelancer|expert)\w*)
        ",
    )
    .expect("valid pattern")
});

/// Forward prefixes of the common mail programs and languages, after optional reply
/// prefixes and the bracket tags a mail gateway puts in front ("[EXTERN] WG: ..."), also
/// the "[Fwd: ...]" form. German mail patterns, do not translate.
static FORWARD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
          ^\s*(?:\[[^\]]{1,24}\]\s*)*
          (?:(?:re|aw|antw|sv|odp)\s*:\s*)*
          (?:\[\s*)?
          (?:fwd?|wg|weitergeleitet|weiterleitung|weitergeleitete\s+nachricht|tr|rv|vs|enc|i|doorgestuurd)\s*:
        ",
    )
    .expect("valid pattern")
});

/// Reply and forward prefixes and bracket tags at the start of a subject: what is left is
/// the subject of the mail that was forwarded. German mail patterns, do not translate.
static PREFIXES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
          ^(?:\s*
            (?: \[\s*(?:fwd?|wg)\s*:
              | \[[^\]]{1,24}\]
              | (?:re|aw|antw|sv|odp|fwd?|wg|weitergeleitet|weiterleitung|weitergeleitete\s+nachricht|tr|rv|vs|enc|i|doorgestuurd)\s*:
            )
          )+\s*
        ",
    )
    .expect("valid pattern")
});

/// Subjects of the portals' mails about the user's own activity: an application, a
/// message, an `InMail`, an invitation. They may link a job, yet they are no alert and their
/// links never become jobs. German mail patterns, do not translate.
static NON_ALERT_SUBJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
          \bbewerbung\w* | \bbeworben\b | \bapplications?\b | \bapplied\b
        | \bnachricht\w* | \bmessages?\b | \binmails?\b
        | \beinladung\w* | \binvitations?\b | \binvited\b
        ",
    )
    .expect("valid pattern")
});

/// Local parts of the portals' senders of messages, `InMail`s and invitations: never an
/// alert, whatever they link.
const NON_ALERT_SENDERS: &[&str] = &[
    "inmail-hit-reply",
    "hit-reply",
    "messages-noreply",
    "messaging-digest-noreply",
    "invitations",
    "invitations-noreply",
];

/// The sender line of a forwarded mail's header block inside the body ("Von: freelancermap
/// <projekte@freelancermap.de>") in the languages of the common mail programs. German mail
/// patterns, do not translate.
static FORWARDED_FROM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)^[\s>*]*(?:von|from|de|van|da|od|från|fra)\s*:[^\n@]{0,200}@([a-z0-9-]+(?:\.[a-z0-9-]+)+)",
    )
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
/// is worth loading whole; `looks_like_alert` decides about a mail without jobs. Every
/// subject that marks a real alert is one too (the candidate test is never narrower than
/// the alert test).
pub(crate) fn is_alert_subject(subject: &str) -> bool {
    ALERT_SUBJECT.is_match(subject) || has_marker_subject(subject)
}

/// The subject without its reply and forward prefixes and bracket tags.
fn bare_subject(subject: &str) -> &str {
    PREFIXES
        .find(subject)
        .map_or(subject, |m| &subject[m.end()..])
}

fn has_marker_subject(subject: &str) -> bool {
    ALERT_MARKER_SUBJECT.is_match(subject) || ALERT_MARKER_SUBJECT.is_match(bare_subject(subject))
}

fn has_marker_body(mail: &ParsedMail) -> bool {
    bodies(mail).any(|body| ALERT_MARKER_BODY.is_match(&one_line(&body)))
}

/// Every text part, HTML as text.
fn bodies(mail: &ParsedMail) -> impl Iterator<Item = String> + '_ {
    mail.html
        .iter()
        .map(|html| html_to_text(html))
        .chain(mail.text.iter().cloned())
}

/// A portal mail about the user's own activity (an application, a message, an `InMail`, an
/// invitation) - by its subject, also when forwarded, or by its sender. A subject that
/// marks a real alert is never one.
fn is_activity_mail(mail: &ParsedMail) -> bool {
    if has_marker_subject(&mail.subject) {
        return false;
    }
    let local = |address: &str| {
        address.rsplit_once('@').is_some_and(|(local, domain)| {
            NON_ALERT_SENDERS.contains(&local) && Portal::from_sender_domain(domain).is_some()
        })
    };
    NON_ALERT_SUBJECT.is_match(bare_subject(&mail.subject))
        || local(&mail.sender_address)
        || mail.inner_senders.iter().any(|a| local(a))
}

/// Did the mail forward something from a portal? A forward prefix, a mail from a portal
/// attached whole, or a forwarded header block in the body that names a portal's address.
fn is_forwarded_alert_form(mail: &ParsedMail) -> bool {
    let portal_address = |domain: &str| Portal::from_sender_domain(domain).is_some();
    FORWARD.is_match(&mail.subject)
        || mail
            .inner_senders
            .iter()
            .filter_map(|a| a.rsplit_once('@'))
            .any(|(_, domain)| portal_address(domain))
        || bodies(mail).any(|body| {
            FORWARDED_FROM
                .captures_iter(&body)
                .any(|c| portal_address(&c[1]))
        })
}

/// May the job links of this mail become jobs? Only the user's alert mails bring links
/// in (the hard rule of the fetch): a portal's own mail that is no activity mail, a
/// forwarded one, or a mail with a real alert's markers. A newsletter with a job link, a
/// recruiter's `InMail`, an application confirmation or a portal message is none, whatever
/// it links.
pub(crate) fn vouches_for_links(mail: &ParsedMail) -> bool {
    if is_activity_mail(mail) {
        return false;
    }
    if mail
        .sender_domain()
        .and_then(Portal::from_sender_domain)
        .is_some()
    {
        return true;
    }
    is_forwarded_alert_form(mail) || has_marker_subject(&mail.subject) || has_marker_body(mail)
}

/// Is a mail without a single recognised job an alert all the same - one whose layout
/// changed? Only when it carries a real alert's markers (subject or body, see
/// `ALERT_MARKER_SUBJECT` and `ALERT_MARKER_BODY`) and its subject is no promo, onboarding,
/// network or activity one. Everything else from the portals' addresses (welcome, discount,
/// "your profile", "someone you may know", a message) is no alert and never raises the
/// changed-layout warning.
pub(crate) fn looks_like_alert(mail: &ParsedMail) -> bool {
    if PROMO_SUBJECT.is_match(&mail.subject) || is_activity_mail(mail) {
        return false;
    }
    has_marker_subject(&mail.subject) || has_marker_body(mail)
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

    /// Subjects of real alerts (the formats behind the checked-in fixtures).
    const MARKER_SUBJECTS: &[&str] = &[
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
        "Suchagent Beispiel vom 18.09.2026",
        "freelance.de – Ihre Projektvorschläge der Woche",
        "Jobbenachrichtigung: Leiter Qualitätsmanagement",
        "Your job alert for controller",
        "Ihre Job-Alerts von LinkedIn",
        "Beispiel AG hat eine Position als Controller (m/w/d) zu besetzen",
        "Your project agent found 3 matches",
        "Job recommendations for you",
    ];

    /// A mail without a recognised job is an alert only with a real alert's markers: the
    /// subject formats and body sentences of the real alerts - never a promo, onboarding or
    /// network subject from the same senders, whatever its body says.
    #[test]
    fn alert_markers() {
        let unlinked = |subject: &str, body: &str| {
            looks_like_alert(&mail("projekte@freelancermap.de", subject, body))
        };
        for subject in MARKER_SUBJECTS {
            assert!(unlinked(subject, "<p>Hallo</p>"), "{subject}");
        }
        // Behind any forward prefix or gateway tag too.
        for subject in [
            "[EXTERN] WG: Neue Jobs",
            "Tr: Neue Jobs",
            "[Fwd: 5 neue Jobs für dich]",
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

    /// The candidate test is never narrower than the alert test: every subject of a real
    /// alert is loaded whole from anyone, without a forward prefix too.
    #[test]
    fn every_marker_subject_is_a_head_candidate() {
        for subject in MARKER_SUBJECTS {
            let m = mail("erika@example.com", subject, "");
            assert!(head_is_candidate(&m, &Portal::ALL), "{subject}");
        }
    }

    /// Forward prefixes of other mail programs and languages, gateway tags and the
    /// portals' own subject forms (measured: all of them were dropped by their head).
    #[test]
    fn forwarded_heads_of_other_mail_programs() {
        for subject in [
            "Beispiel AG hat eine Position als Controller (m/w/d) zu besetzen",
            "Musterwerke is hiring a Senior Controller",
            "Musterwerke sucht Interim Manager Finance (m/w/d)",
            "[EXTERN] WG: Suchagent Beispiel vom 18.09.2026",
            "[EXT] [Extern] Fwd: Irgendwas",
            "Tr: Irgendwas",
            "RV: Irgendwas",
            "VS: Irgendwas",
            "Doorgestuurd: Irgendwas",
            "[Fwd: Irgendwas]",
            "Weitergeleitete Nachricht: Irgendwas",
            "AW: WG: Irgendwas",
        ] {
            let m = mail("erika@example.com", subject, "");
            assert!(head_is_candidate(&m, &Portal::ALL), "{subject}");
        }
        for subject in [
            "Neue Termine",
            "Wir suchen einen Termin",
            "Tragen Sie sich ein",
            "Initiative Nord",
        ] {
            let m = mail("erika@example.com", subject, "");
            assert!(!head_is_candidate(&m, &Portal::ALL), "{subject}");
        }
    }

    /// Only an alert brings job links in: a portal's activity mails and a newsletter with
    /// a job link do not, a forward or a real alert does.
    #[test]
    fn who_vouches_for_job_links() {
        let link =
            r#"<a href="https://www.linkedin.com/comm/jobs/view/4123456789/">Controller</a>"#;
        for (sender, subject) in [
            (
                "jobs-noreply@linkedin.com",
                "Erika, Ihre Bewerbung wurde an Musterwerke GmbH gesendet",
            ),
            (
                "jobs-noreply@linkedin.com",
                "Erika, your application was sent to Musterwerke GmbH",
            ),
            ("inmail-hit-reply@linkedin.com", "Interim CFO Mandat"),
            (
                "noreply@freelancermap.de",
                "Neue Nachricht zu Ihrer Bewerbung",
            ),
            (
                "invitations@linkedin.com",
                "Erika, Max möchte sich vernetzen",
            ),
            (
                "news@firma.example",
                "Newsletter KW 38 - wir stellen ein, Jobs bei uns",
            ),
            ("erika@example.com", "Fwd: Ihre Bewerbung bei Musterwerke"),
        ] {
            let m = mail(sender, subject, link);
            assert!(!vouches_for_links(&m), "{sender} {subject}");
        }
        for (sender, subject, body) in [
            ("jobalerts-noreply@linkedin.com", "Controller in Köln", link),
            ("erika@example.com", "Fwd: Sammlung", link),
            ("erika@example.com", "[EXTERN] WG: Irgendwas", link),
            ("erika@example.com", "Neue Jobs für Sie", link),
            (
                "erika@example.com",
                "Sammlung",
                &format!("<p>Von: LinkedIn &lt;jobalerts-noreply@linkedin.com&gt;</p>{link}"),
            ),
            (
                "erika@example.com",
                "Sammlung",
                &format!("<p>Sie erhalten E-Mails zu Jobbenachrichtigungen.</p>{link}"),
            ),
        ] {
            let m = mail(sender, subject, body);
            assert!(vouches_for_links(&m), "{sender} {subject}");
        }
        // A portal mail attached whole vouches, unless it is an activity mail.
        let mut m = mail("erika@example.com", "Siehe Anhang", link);
        m.inner_senders = vec!["jobalerts-noreply@linkedin.com".into()];
        assert!(vouches_for_links(&m));
        m.inner_senders = vec!["inmail-hit-reply@linkedin.com".into()];
        assert!(!vouches_for_links(&m));
    }
}
