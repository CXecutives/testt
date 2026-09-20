//! Die drei Portale und alles, was sich aus einer Job-URL ableiten lässt.
//!
//! Eine Stelle für alle URL-Regeln: Identität eines Jobs (`JobKey`), der Link, den der
//! Nutzer öffnet (`canonical_url`), und die Adresse, von der die App den Volltext holt
//! (`fetch_url`). Job-IDs werden nur aus **positionsgenau** bestimmten Pfadteilen gelesen –
//! nie per Suchmuster über die ganze URL (früher: Postleitzahlen oder Normnummern im
//! Slug wurden als Job-ID gelesen, Ziffern aus der Query ebenso).

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

/// Ein Portal. Die Reihenfolge ist die der Oberfläche und des Exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Portal {
    #[serde(rename = "linkedin")]
    LinkedIn,
    #[serde(rename = "freelance")]
    FreelanceDe,
    #[serde(rename = "freelancermap")]
    Freelancermap,
}

/// Wie ein Portal abgerufen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoginMode {
    /// Ohne Konto lesbar; ein Sitzungsfenster gibt es nicht.
    None,
    /// Ohne Konto lesbar, angemeldet vollständiger – der Nutzer entscheidet.
    Optional,
    /// Nur angemeldet lesbar.
    Required,
}

impl Portal {
    pub const ALL: [Portal; 3] = [Portal::LinkedIn, Portal::FreelanceDe, Portal::Freelancermap];

    /// Ob und wie eine Anmeldung möglich ist.
    pub const fn login_mode(self) -> LoginMode {
        match self {
            Portal::LinkedIn => LoginMode::None,
            Portal::FreelanceDe => LoginMode::Required,
            Portal::Freelancermap => LoginMode::Optional,
        }
    }

    /// Schlüssel für Speicher, Einstellungen und Oberfläche.
    pub const fn key(self) -> &'static str {
        match self {
            Portal::LinkedIn => "linkedin",
            Portal::FreelanceDe => "freelance",
            Portal::Freelancermap => "freelancermap",
        }
    }

    pub fn from_key(key: &str) -> Option<Portal> {
        Portal::ALL.into_iter().find(|p| p.key() == key)
    }

    /// Anzeigename.
    pub const fn label(self) -> &'static str {
        match self {
            Portal::LinkedIn => "LinkedIn",
            Portal::FreelanceDe => "freelance.de",
            Portal::Freelancermap => "freelancermap.de",
        }
    }

    /// Kürzel im Dateinamen der Jobdetails (ohne „.de“) – Teil des Vertrags mit dem
    /// Matching-Skill, der auch in der Kopfzeile `Quelle:` steht.
    pub const fn file_tag(self) -> &'static str {
        match self {
            Portal::LinkedIn => "LinkedIn",
            Portal::FreelanceDe => "Freelance",
            Portal::Freelancermap => "Freelancermap",
        }
    }

    /// Startseite des Portals („Im Browser öffnen“ nach einer Sperre).
    pub const fn home_url(self) -> &'static str {
        match self {
            Portal::LinkedIn => "https://www.linkedin.com/jobs/",
            Portal::FreelanceDe => "https://www.freelance.de/",
            Portal::Freelancermap => "https://www.freelancermap.de/",
        }
    }

    /// Absender-Domains der Original-Alerts (Subdomains zählen mit).
    pub const fn sender_domains(self) -> &'static [&'static str] {
        match self {
            Portal::LinkedIn => &["linkedin.com"],
            Portal::FreelanceDe => &["freelance.de"],
            Portal::Freelancermap => &["freelancermap.de", "freelancermap.com"],
        }
    }

    /// Stichwörter, an denen weitergeleitete Alerts erkannt werden (Absender ist dann
    /// der Nutzer selbst).
    pub const fn search_terms(self) -> &'static [&'static str] {
        match self {
            Portal::LinkedIn => &["linkedin"],
            Portal::FreelanceDe => &["freelance.de"],
            Portal::Freelancermap => &["freelancermap"],
        }
    }

    /// Portal zu einer Absender-Domain: exakt oder als Subdomain – `freelancermap.de` ist
    /// also nie `freelance.de`.
    pub fn from_sender_domain(domain: &str) -> Option<Portal> {
        let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
        Portal::ALL
            .into_iter()
            .find(|p| p.sender_domains().iter().any(|d| host_is(&domain, d)))
    }
}

impl fmt::Display for Portal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Identität eines Jobs über alle Mails und Läufe hinweg.
///
/// `id` ist die Job-ID des Portals (nur Ziffern) oder – nur bei freelancermap-Links der
/// Form `/projekt/<slug>`, die keine ID tragen – `u` + 12 Hex-Zeichen eines Hashes aus
/// Domain und Pfad. Beim Einlesen von außen (Oberfläche) wird die Form geprüft: Die ID
/// landet in Dateinamen und darf deshalb nie etwas anderes enthalten.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "RawJobKey")]
pub struct JobKey {
    pub portal: Portal,
    pub id: String,
}

#[derive(Deserialize)]
struct RawJobKey {
    portal: Portal,
    id: String,
}

impl TryFrom<RawJobKey> for JobKey {
    type Error = String;

    fn try_from(raw: RawJobKey) -> Result<Self, Self::Error> {
        let digits = all_digits(&raw.id, 1);
        let hash = raw.id.len() == 13
            && raw.id.starts_with('u')
            && raw.id[1..]
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if digits || (hash && raw.portal == Portal::Freelancermap) {
            Ok(JobKey {
                portal: raw.portal,
                id: raw.id,
            })
        } else {
            Err(format!("ungültige Job-ID „{}“", raw.id))
        }
    }
}

impl JobKey {
    /// Trägt der Schlüssel eine echte Portal-ID (statt eines Hashes)?
    pub fn has_portal_id(&self) -> bool {
        all_digits(&self.id, 1)
    }
}

impl fmt::Display for JobKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.portal.key(), self.id)
    }
}

/// Ein erkannter Job-Link: Identität und der bereinigte Link für den Nutzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobLink {
    pub key: JobKey,
    /// Link zur Anzeige – ohne Tracking-Parameter, immer https.
    pub url: Url,
}

/// Erkennt einen Job-Link. `None` für alles, was kein Job eines der drei Portale ist
/// (Profil-, Such-, Abmelde- oder Login-Links, fremde Hosts, andere Schemata).
///
/// Klick-Tracker, die das Ziel unverschlüsselt als Parameter tragen, werden einmal
/// aufgelöst.
pub fn job_link(raw: &str) -> Option<JobLink> {
    let url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    direct_job_link(&url).or_else(|| {
        url.query_pairs()
            .filter_map(|(_, value)| Url::parse(&value).ok())
            .filter(|inner| matches!(inner.scheme(), "http" | "https"))
            .find_map(|inner| direct_job_link(&inner))
    })
}

fn direct_job_link(url: &Url) -> Option<JobLink> {
    let host = url.host_str()?.to_ascii_lowercase();
    // Pfadteile ohne Rücksicht auf Groß/Klein vergleichen (wie der Altcode mit re.I);
    // Portale verlinken durchweg klein, weitergeleitete Links nicht immer.
    let lowered: Vec<String> = url
        .path_segments()
        .map(|s| {
            s.filter(|seg| !seg.is_empty())
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_default();
    let segments: Vec<&str> = lowered.iter().map(String::as_str).collect();

    if host_is(&host, "linkedin.com") {
        let id = linkedin_id(&segments)?;
        return Some(link(Portal::LinkedIn, id));
    }
    if host_is(&host, "freelance.de") {
        let id = freelance_id(url, &segments)?;
        return Some(link(Portal::FreelanceDe, id));
    }
    let domain = ["freelancermap.de", "freelancermap.com"]
        .into_iter()
        .find(|d| host_is(&host, d))?;
    if let Some(id) = freelancermap_id(&segments) {
        return Some(link(Portal::Freelancermap, id));
    }
    // /projekt/<slug> (bzw. /project/<slug> auf .com) trägt keine ID. Die Identität hängt
    // an Domain und Pfad – unabhängig von Subdomain, Groß/Klein und Tracking-Parametern.
    let [kind @ ("projekt" | "project"), slug] = segments.as_slice() else {
        return None;
    };
    if !is_slug(slug) {
        return None;
    }
    let path = format!("/{kind}/{slug}");
    let hash = Sha256::digest(format!("{domain}{path}").as_bytes());
    Some(JobLink {
        key: JobKey {
            portal: Portal::Freelancermap,
            id: format!("u{}", hex12(&hash)),
        },
        url: Url::parse(&format!("https://www.{domain}{path}")).ok()?,
    })
}

/// `/jobs/view/<ID>` bzw. `/comm/jobs/view/<ID>`; das Segment ist die ID oder endet auf
/// `-<ID>` (`sap-berater-4456653430`).
fn linkedin_id(segments: &[&str]) -> Option<String> {
    let (["jobs", "view", rest @ ..] | ["comm", "jobs", "view", rest @ ..]) = segments else {
        return None;
    };
    let digits = rest.first()?.rsplit('-').next()?;
    all_digits(digits, 6).then(|| digits.to_string())
}

/// `/project/index.php?id=<ID>`, `/projekte/projekt-<ID>[-slug]` oder `/projekt-<ID>[-slug]`
/// – nur an diesen Stellen (ein Blog-Artikel „…/blog/projekt-2025-…“ ist kein Projekt).
fn freelance_id(url: &Url, segments: &[&str]) -> Option<String> {
    if segments == ["project", "index.php"] {
        let (_, id) = url
            .query_pairs()
            .find(|(k, _)| k.eq_ignore_ascii_case("id"))?;
        return all_digits(&id, 1).then(|| id.into_owned());
    }
    let (["projekte", segment, ..] | [segment, ..]) = segments else {
        return None;
    };
    let digits = segment.strip_prefix("projekt-")?.split('-').next()?;
    all_digits(digits, 4).then(|| digits.to_string())
}

/// `/nproj/<ID>.html` oder `/projektboerse/projekte/…/<ID>-slug.html`.
fn freelancermap_id(segments: &[&str]) -> Option<String> {
    match segments {
        ["nproj", file] => {
            let digits = file.strip_suffix(".html").unwrap_or(file);
            all_digits(digits, 5).then(|| digits.to_string())
        }
        ["projektboerse", "projekte" | "projekt", .., last] => {
            let digits = last.split('-').next()?;
            (last.contains('-') && all_digits(digits, 5)).then(|| digits.to_string())
        }
        _ => None,
    }
}

fn link(portal: Portal, id: String) -> JobLink {
    let url = canonical_url(portal, &id).expect("kanonische URL aus Ziffern-ID ist immer gültig");
    JobLink {
        key: JobKey { portal, id },
        url,
    }
}

/// Link zur Anzeige für eine Portal-ID. Die Formen sind geprüft: jede führt ohne Konto
/// zur Anzeige (freelance.de: zur Projektseite mit Registrierungswand).
pub(crate) fn canonical_url(portal: Portal, id: &str) -> Option<Url> {
    if !all_digits(id, 1) {
        return None;
    }
    let url = match portal {
        Portal::LinkedIn => format!("https://www.linkedin.com/jobs/view/{id}/"),
        Portal::FreelanceDe => format!("https://www.freelance.de/project/index.php?id={id}"),
        Portal::Freelancermap => format!("https://www.freelancermap.de/nproj/{id}.html"),
    };
    Url::parse(&url).ok()
}

/// Adresse, von der die App den Volltext holt. LinkedIn: der öffentliche Gast-Abschnitt
/// (der Mail-Link selbst führt Gäste auf die Anmeldeseite). Übrige: der Anzeige-Link.
pub fn fetch_url(link: &JobLink) -> Url {
    match (link.key.portal, link.key.has_portal_id()) {
        (Portal::LinkedIn, true) => Url::parse(&format!(
            "https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/{}",
            link.key.id
        ))
        .expect("Gast-URL aus Ziffern-ID ist immer gültig"),
        _ => link.url.clone(),
    }
}

/// `host` ist `domain` oder eine Subdomain davon.
pub(crate) fn host_is(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

/// Längere Ziffernfolgen sind keine Job-ID: Die ID landet in Dateinamen und kommt über die
/// Oberfläche zurück, wo längere abgelehnt werden.
const MAX_ID_DIGITS: usize = 20;

fn all_digits(text: &str, min_len: usize) -> bool {
    (min_len..=MAX_ID_DIGITS).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit())
}

fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 300
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Die ersten 6 Bytes als 12 Hex-Zeichen.
pub(crate) fn hex12(bytes: &[u8]) -> String {
    use fmt::Write as _;
    bytes
        .iter()
        .take(6)
        .fold(String::with_capacity(12), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Helfer geben Option zurück, damit assert_eq! lesbar bleibt"
)]
mod tests {
    use super::*;

    fn key(raw: &str) -> Option<(Portal, String)> {
        job_link(raw).map(|l| (l.key.portal, l.key.id))
    }

    fn li(id: &str) -> Option<(Portal, String)> {
        Some((Portal::LinkedIn, id.to_string()))
    }
    fn fl(id: &str) -> Option<(Portal, String)> {
        Some((Portal::FreelanceDe, id.to_string()))
    }
    fn fm(id: &str) -> Option<(Portal, String)> {
        Some((Portal::Freelancermap, id.to_string()))
    }

    #[test]
    fn linkedin_links() {
        assert_eq!(
            key("https://www.linkedin.com/jobs/view/4123456789/"),
            li("4123456789")
        );
        assert_eq!(
            key("https://de.linkedin.com/jobs/view/4100000001/?refId=abc"),
            li("4100000001")
        );
        assert_eq!(
            key("https://www.linkedin.com/comm/jobs/view/4123456789/?trackingId=abc&refId=x"),
            li("4123456789")
        );
        assert_eq!(
            key("https://www.linkedin.com/comm/jobs/view/interim-cfo-4987654321?refId=y"),
            li("4987654321")
        );
        assert_eq!(
            key(
                "https://www.linkedin.com/jobs/view/sap-production-support-at-siemens-4456653430?position=1&pageNum=0"
            ),
            li("4456653430")
        );
    }

    #[test]
    fn linkedin_non_job_links() {
        for raw in [
            "https://www.linkedin.com/comm/jobs/search/?keywords=controller",
            "https://www.linkedin.com/company/flownotes/",
            "https://www.linkedin.com/messaging/",
            "https://www.linkedin.com/jobs/view/12345/", // zu kurz
            // zu lang: keine ID, sonst eine Zeile, die sich nie öffnen oder abrufen ließe
            "https://www.linkedin.com/jobs/view/412345678901234567890/",
            "",
        ] {
            assert_eq!(key(raw), None, "{raw}");
        }
    }

    /// Früher: Das alte Muster nahm die erste 6+-stellige Ziffernfolge nach
    /// einem Bindestrich – hier „20260915“ statt der Job-ID.
    #[test]
    fn linkedin_id_is_the_end_of_the_segment() {
        assert_eq!(
            key("https://www.linkedin.com/jobs/view/start-20260915-berater-4456653430/"),
            li("4456653430")
        );
    }

    #[test]
    fn freelance_links() {
        assert_eq!(
            key("https://www.freelance.de/projekte/projekt-1288981-SAP-Transformation"),
            fl("1288981")
        );
        assert_eq!(
            key("https://www.freelance.de/project/index.php?id=1255067&utm_source=x"),
            fl("1255067")
        );
        assert_eq!(
            key("https://www.freelance.de/project/index.php?utm_source=wochenmail&id=1255068"),
            fl("1255068")
        );
        assert_eq!(
            key("https://www.freelance.de/projekte/projekt-1200001-Data-Engineer-Azure?ref=mail"),
            fl("1200001")
        );
        assert_eq!(key("https://www.freelance.de/newsletter/abmelden"), None);
        assert_eq!(key("https://www.freelance.de/projekte"), None);
        assert_eq!(
            key(&format!(
                "https://www.freelance.de/project/index.php?id={}",
                "1".repeat(300)
            )),
            None
        );
        assert_eq!(
            key("https://www.freelance.de/project/index.php?id=12345678901234567890"),
            fl("12345678901234567890")
        );
    }

    /// Zwei Link-Formen desselben Projekts ergeben denselben Schlüssel.
    #[test]
    fn same_project_same_key() {
        let a =
            job_link("https://www.freelance.de/project/index.php?id=1255067&utm_source=w").unwrap();
        let b = job_link("https://www.freelance.de/projekte/projekt-1255067-sap-s4hana").unwrap();
        assert_eq!(a.key, b.key);
        assert_eq!(a.url, b.url);
        let c =
            job_link("https://www.freelancermap.de/nproj/2971857.html?utm_source=agent").unwrap();
        let d = job_link(
            "https://www.freelancermap.de/projektboerse/projekte/it/2971857-performance.html",
        )
        .unwrap();
        assert_eq!(c.key, d.key);
        assert_eq!(
            c.url.as_str(),
            "https://www.freelancermap.de/nproj/2971857.html"
        );
    }

    #[test]
    fn freelancermap_links() {
        assert_eq!(
            key("https://www.freelancermap.de/nproj/2971857.html?utm=x"),
            fm("2971857")
        );
        assert_eq!(
            key(
                "https://www.freelancermap.de/projektboerse/projekte/it/2900001-sap-fi-co-berater.html?utm_source=alert"
            ),
            fm("2900001")
        );
        assert_eq!(
            key(
                "https://www.freelancermap.de/projektboerse/projekte/consulting/2900002-projektleiter.html"
            ),
            fm("2900002")
        );
        assert_eq!(key("https://www.freelancermap.de/"), None);
        assert_eq!(key("https://www.freelancermap.de/login"), None);
    }

    /// Früher: Ziffern aus Query, Slug oder fremdem Host wurden als ID
    /// gelesen. `/projekt/<slug>` trägt keine ID – Postleitzahl und Normnummer im Slug
    /// dürfen nicht zur ID werden.
    #[test]
    fn freelancermap_slug_links_never_yield_an_id() {
        let a = job_link("https://www.freelancermap.de/projekt/sap-berater-m-w-d-80331-muenchen")
            .unwrap();
        let b = job_link("https://www.freelancermap.de/projekt/iso-27001-auditor-remote").unwrap();
        let c =
            job_link("https://www.freelancermap.de/projekt/iso-27001-lead-implementer").unwrap();
        for l in [&a, &b, &c] {
            assert!(!l.key.has_portal_id(), "{:?}", l.key);
            assert!(l.key.id.starts_with('u') && l.key.id.len() == 13);
        }
        assert_ne!(b.key, c.key, "zwei Projekte, zwei Schlüssel");
        // Tracking-Parameter ändern den Schlüssel nicht.
        let b2 = job_link(
            "https://freelancermap.de/projekt/iso-27001-auditor-remote?utm_campaign=alert-8871234",
        )
        .unwrap();
        assert_eq!(b.key, b2.key);
        assert_eq!(
            b2.url.as_str(),
            "https://www.freelancermap.de/projekt/iso-27001-auditor-remote"
        );
        assert_eq!(key("https://example.com/foo/123456"), None);
        assert_eq!(key("https://example.com/nproj/2971857.html"), None);
    }

    #[test]
    fn only_http_and_known_hosts() {
        for raw in [
            "javascript:alert(1)//www.linkedin.com/jobs/view/4123456789/",
            "data:text/html,https://www.freelance.de/projekte/projekt-1288981",
            "file:///C:/www.linkedin.com/jobs/view/4123456789/",
            "https://linkedin.com.evil.example/jobs/view/4123456789/",
            "https://evillinkedin.com/jobs/view/4123456789/",
        ] {
            assert_eq!(key(raw), None, "{raw}");
        }
        // Die Anzeige-URL ist immer https, auch wenn die Mail http verlinkt.
        let l = job_link("http://www.linkedin.com/jobs/view/4123456789").unwrap();
        assert_eq!(
            l.url.as_str(),
            "https://www.linkedin.com/jobs/view/4123456789/"
        );
    }

    #[test]
    fn click_tracker_is_resolved_once() {
        let l = job_link(
            "https://click.example-mailer.com/r?u=https%3A%2F%2Fwww.linkedin.com%2Fcomm%2Fjobs%2Fview%2F4123456789%2F&x=1",
        )
        .unwrap();
        assert_eq!(l.key.id, "4123456789");
    }

    #[test]
    fn fetch_urls() {
        let l =
            job_link("https://www.linkedin.com/comm/jobs/view/4123456789/?trackingId=abc").unwrap();
        assert_eq!(
            fetch_url(&l).as_str(),
            "https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/4123456789"
        );
        let m = job_link("https://www.freelancermap.de/nproj/2971857.html").unwrap();
        assert_eq!(
            fetch_url(&m).as_str(),
            "https://www.freelancermap.de/nproj/2971857.html"
        );
        let f = job_link("https://www.freelance.de/projekte/projekt-1288981-SAP").unwrap();
        assert_eq!(
            fetch_url(&f).as_str(),
            "https://www.freelance.de/project/index.php?id=1288981"
        );
    }

    /// Die ID wird nur an festen Stellen gelesen.
    #[test]
    fn freelance_id_only_at_fixed_positions() {
        assert_eq!(
            key("https://www.freelance.de/blog/projekt-2025-jahresrueckblick"),
            None
        );
        assert_eq!(
            key("https://www.freelance.de/freelancer/projekt-1234-referenz"),
            None
        );
        assert_eq!(
            key("https://www.freelance.de/projekt-1288981-SAP"),
            fl("1288981")
        );
    }

    /// Groß/Klein im Pfad und im Parameter spielt keine Rolle (Altcode: re.I).
    #[test]
    fn path_keywords_ignore_case() {
        assert_eq!(
            key("https://www.linkedin.com/Jobs/View/4123456789/"),
            li("4123456789")
        );
        assert_eq!(
            key("https://www.freelance.de/Projekte/Projekt-1288981-SAP-Transformation"),
            fl("1288981")
        );
        assert_eq!(
            key("https://www.freelance.de/project/index.php?ID=1255067"),
            fl("1255067")
        );
        assert_eq!(
            key("https://www.freelancermap.de/NPROJ/2971857.HTML"),
            fm("2971857")
        );
        let upper = job_link("https://www.freelancermap.de/Projekt/SAP-Berater-m-w-d").unwrap();
        let lower = job_link("https://www.freelancermap.de/projekt/sap-berater-m-w-d").unwrap();
        assert_eq!(upper.key, lower.key);
    }

    /// Subdomains ergaben eine nicht existierende Adresse und einen
    /// anderen Schlüssel.
    #[test]
    fn freelancermap_subdomains_share_key_and_url() {
        let m = job_link("https://m.freelancermap.de/projekt/sap-berater-m-w-d").unwrap();
        let www = job_link("https://www.freelancermap.de/projekt/sap-berater-m-w-d").unwrap();
        let bare = job_link("https://freelancermap.de/projekt/sap-berater-m-w-d").unwrap();
        assert_eq!(m.key, www.key);
        assert_eq!(bare.key, www.key);
        assert_eq!(
            m.url.as_str(),
            "https://www.freelancermap.de/projekt/sap-berater-m-w-d"
        );
        let com = job_link("https://www.freelancermap.com/project/sap-consultant").unwrap();
        assert_eq!(
            com.url.as_str(),
            "https://www.freelancermap.com/project/sap-consultant"
        );
    }

    /// Ein Job-Schlüssel aus der Oberfläche wird an der Grenze geprüft – die ID landet in
    /// Dateinamen.
    #[test]
    fn job_key_from_outside_is_validated() {
        let ok: JobKey =
            serde_json::from_str(r#"{"portal":"linkedin","id":"4123456789"}"#).unwrap();
        assert!(ok.has_portal_id());
        let hash: JobKey =
            serde_json::from_str(r#"{"portal":"freelancermap","id":"u0123456789ab"}"#).unwrap();
        assert!(!hash.has_portal_id());
        for bad in [
            r#"{"portal":"linkedin","id":"..\\..\\Windows\\x"}"#,
            r#"{"portal":"linkedin","id":""}"#,
            r#"{"portal":"linkedin","id":"u0123456789ab"}"#,
            r#"{"portal":"freelancermap","id":"u0123456789AB"}"#,
            r#"{"portal":"linkedin","id":"123456789012345678901"}"#,
        ] {
            assert!(serde_json::from_str::<JobKey>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn sender_domains() {
        assert_eq!(
            Portal::from_sender_domain("linkedin.com"),
            Some(Portal::LinkedIn)
        );
        assert_eq!(
            Portal::from_sender_domain("e.LinkedIn.com"),
            Some(Portal::LinkedIn)
        );
        assert_eq!(
            Portal::from_sender_domain("mail.freelancermap.de"),
            Some(Portal::Freelancermap)
        );
        assert_eq!(
            Portal::from_sender_domain("freelancermap.com"),
            Some(Portal::Freelancermap)
        );
        assert_eq!(
            Portal::from_sender_domain("freelance.de"),
            Some(Portal::FreelanceDe)
        );
        assert_eq!(
            Portal::from_sender_domain("news.freelance.de"),
            Some(Portal::FreelanceDe)
        );
        // freelancermap.de ist nie freelance.de, fremde Domains mit gleichem Ende auch nicht.
        assert_ne!(
            Portal::from_sender_domain("freelancermap.de"),
            Some(Portal::FreelanceDe)
        );
        assert_eq!(Portal::from_sender_domain("notfreelance.de"), None);
        assert_eq!(Portal::from_sender_domain("gmail.com"), None);
    }

    #[test]
    fn keys_labels_and_serde() {
        assert_eq!(
            Portal::ALL.map(Portal::key),
            ["linkedin", "freelance", "freelancermap"]
        );
        assert_eq!(
            Portal::ALL.map(Portal::login_mode),
            [LoginMode::None, LoginMode::Required, LoginMode::Optional]
        );
        assert_eq!(
            Portal::ALL.map(Portal::file_tag),
            ["LinkedIn", "Freelance", "Freelancermap"]
        );
        for p in Portal::ALL {
            assert_eq!(Portal::from_key(p.key()), Some(p));
            assert_eq!(
                serde_json::to_string(&p).unwrap(),
                format!("\"{}\"", p.key())
            );
        }
        // Ein unbekanntes Portal aus der Oberfläche wird abgelehnt.
        assert!(serde_json::from_str::<Portal>("\"../../x\"").is_err());
        assert_eq!(Portal::from_key("xing"), None);
    }
}
