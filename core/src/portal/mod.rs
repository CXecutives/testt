//! The portals: one adapter per portal and one registry ([`PORTALS`]).
//!
//! Everything that differs between portals lives in its adapter - keys and labels, alert
//! senders and search terms, pace and caps, how a portal is accessed, which links are jobs,
//! and how a job page reads. The rest of the app asks the registry and knows no single
//! portal: adding one means a variant, an adapter and a registry entry, nothing else.
//!
//! Job ids are only read from **exactly positioned** path parts - never by searching the
//! whole URL (formerly postal codes or norm numbers in a slug became job ids, and so did
//! digits from the query).

mod freelance_de;
mod freelancermap;
mod linkedin;
#[cfg(test)]
mod probe;

#[cfg(test)]
pub(crate) use freelance_de::tests::{
    TEASER as FREELANCE_DE_TEASER, guest_html as freelance_de_page,
};
#[cfg(test)]
pub(crate) use freelancermap::tests::page as freelancermap_page;
#[cfg(test)]
pub(crate) use linkedin::tests::page as linkedin_page;

use std::fmt;
use std::sync::LazyLock;

use scraper::Selector;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::fetch::PageOutcome;
use crate::fetch::policy::Limits;
use crate::fetch::site::PortalSite;

/// A portal. The order is the one of the interface and the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Portal {
    #[serde(rename = "linkedin")]
    LinkedIn,
    #[serde(rename = "freelance")]
    FreelanceDe,
    #[serde(rename = "freelancermap")]
    Freelancermap,
    /// A fourth portal that exists in tests only: it proves that the registry is the only
    /// place that knows the portals.
    #[cfg(test)]
    #[serde(rename = "probe")]
    Probe,
}

/// How a portal's job pages can be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Readable without an account; there is no session window.
    Guest,
    /// Readable in the session window with the user's sign-in. `required: false`: a guest
    /// still sees a part (a teaser) without signing in.
    Session { required: bool },
}

/// The fetch path a run builds for a portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchPath {
    /// HTTP without an account.
    Guest,
    /// The session window with the user's sign-in.
    Session,
}

impl Access {
    /// The path for a run: the session window only where signing in is switched on, the
    /// guest path wherever a guest sees something, otherwise none (zero requests).
    pub fn path(self, login_enabled: bool) -> Option<FetchPath> {
        match self {
            Access::Session { .. } if login_enabled => Some(FetchPath::Session),
            Access::Guest | Access::Session { required: false } => Some(FetchPath::Guest),
            Access::Session { required: true } => None,
        }
    }

    /// Whether the portal offers a sign-in at all.
    pub fn can_sign_in(self) -> bool {
        matches!(self, Access::Session { .. })
    }
}

/// How the guest client treats redirects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redirects {
    /// Judge every redirect itself (it usually leads to a sign-in wall).
    Never,
    /// Follow at most two redirects on the same host and scheme.
    SameOrigin,
}

/// Everything portal-specific. Adapters are stateless; the registry holds one per portal.
pub trait PortalAdapter: Send + Sync {
    fn portal(&self) -> Portal;
    /// Key for store, settings and interface.
    fn key(&self) -> &'static str;
    /// Display name.
    fn label(&self) -> &'static str;
    /// Tag in the file name of the job details (without ".de") - part of the contract with
    /// the matching skill, which also shows it in the `Quelle:` header line.
    fn file_tag(&self) -> &'static str;
    /// Start page of the portal ("open in the browser" after a block).
    fn home_url(&self) -> &'static str;
    /// Sender domains of the original alerts (subdomains count too).
    fn sender_domains(&self) -> &'static [&'static str];
    /// Keywords that identify forwarded alerts (the sender is then the user).
    fn search_terms(&self) -> &'static [&'static str];
    /// Pace and caps of the portal's requests.
    fn limits(&self) -> Limits;
    fn access(&self) -> Access;
    /// The job behind a link of this portal; `None` for everything else (profile, search,
    /// unsubscribe or sign-in links, other hosts).
    fn job_link(&self, url: &Url) -> Option<JobLink>;
    /// The link shown for a portal id. `None` for ids that are not all digits.
    fn canonical_url(&self, id: &str) -> Option<Url>;
    /// Where the app reads the full text of a job.
    fn fetch_url(&self, link: &JobLink) -> Url {
        link.url.clone()
    }
    /// How the guest client treats redirects.
    fn redirects(&self) -> Redirects {
        Redirects::SameOrigin
    }
    /// A redirect the guest client did not follow; `path` is its target (lower case).
    fn redirect_outcome(&self, path: &str) -> PageOutcome;
    /// A guest page answered with 200; `path` is the final path after redirects (lower
    /// case).
    fn guest_page(&self, html: &str, path: &str, link: &JobLink) -> PageOutcome;
    /// The session window of the portal; `None` for portals without a sign-in.
    fn session(&self) -> Option<&'static PortalSite> {
        None
    }
    /// Version of the page parsers (guest page and session window). Bumped whenever they
    /// read pages differently; stored with every judged page (`parser_version`) - after a
    /// bump the portal's failed jobs are fetched again.
    fn parser_version(&self) -> u32;
    /// The facts a guest page states in structured form (also part of `guest_page`).
    fn parse_facts(&self, html: &str) -> Facts;
}

/// Most skills kept in the facts.
const MAX_SKILLS: usize = 20;
/// Most characters of one fact.
const MAX_FACT_CHARS: usize = 80;

/// Facts a job page states in structured form - stored as JSON in `desc_facts` for the
/// matching engine. Values are the page's own words (external data, never translated);
/// missing ones are left out of the JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Facts {
    /// "Vollzeit", "Freiberuflich", "Contract" ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employment_type: Option<String>,
    /// Career level ("Direktor", "Mid-Senior level").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// Share of remote work in percent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_percent: Option<u8>,
    /// Remote as the page words it, when it gives no percentage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// Rate as the page words it ("95 €/h").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
}

impl Facts {
    pub fn is_empty(&self) -> bool {
        *self == Facts::default()
    }

    /// One fact: one line, at most [`MAX_FACT_CHARS`] characters, `None` when empty.
    pub(crate) fn value(raw: &str) -> Option<String> {
        let value = crate::text::one_line(raw);
        (!value.is_empty()).then(|| crate::text::truncate_chars(&value, MAX_FACT_CHARS))
    }

    /// Remote in the page's words: a percentage becomes `remote_percent`, anything else
    /// stays as text.
    pub(crate) fn set_remote(&mut self, raw: &str) {
        let Some(value) = Facts::value(raw) else {
            return;
        };
        let digits: String = value
            .split('%')
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        match digits.parse::<u8>() {
            Ok(percent) if value.contains('%') && percent <= 100 => {
                self.remote_percent = Some(percent);
            }
            _ => self.remote = Some(value),
        }
    }

    /// Skills: each once, at most [`MAX_SKILLS`].
    pub(crate) fn add_skill(&mut self, raw: &str) {
        if self.skills.len() < MAX_SKILLS
            && let Some(skill) = Facts::value(raw)
            && !self.skills.contains(&skill)
        {
            self.skills.push(skill);
        }
    }
}

/// The registry: every portal once, in the order of the interface.
pub static PORTALS: &[&dyn PortalAdapter] = &[
    &linkedin::LinkedIn,
    &freelance_de::FreelanceDe,
    &freelancermap::Freelancermap,
    #[cfg(test)]
    &probe::Probe,
];

impl Portal {
    /// The product portals (the test-only probe is not among them).
    pub const ALL: [Portal; 3] = [Portal::LinkedIn, Portal::FreelanceDe, Portal::Freelancermap];

    /// The adapter of this portal.
    pub fn adapter(self) -> &'static dyn PortalAdapter {
        *PORTALS
            .iter()
            .find(|a| a.portal() == self)
            .expect("every portal is registered")
    }

    pub fn key(self) -> &'static str {
        self.adapter().key()
    }

    pub fn from_key(key: &str) -> Option<Portal> {
        PORTALS.iter().find(|a| a.key() == key).map(|a| a.portal())
    }

    pub fn label(self) -> &'static str {
        self.adapter().label()
    }

    pub fn file_tag(self) -> &'static str {
        self.adapter().file_tag()
    }

    pub fn home_url(self) -> &'static str {
        self.adapter().home_url()
    }

    pub fn sender_domains(self) -> &'static [&'static str] {
        self.adapter().sender_domains()
    }

    pub fn search_terms(self) -> &'static [&'static str] {
        self.adapter().search_terms()
    }

    pub fn access(self) -> Access {
        self.adapter().access()
    }

    /// Portal of a sender domain: exact or as a subdomain - `freelancermap.de` is therefore
    /// never `freelance.de`.
    pub fn from_sender_domain(domain: &str) -> Option<Portal> {
        let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
        PORTALS
            .iter()
            .find(|a| a.sender_domains().iter().any(|d| host_is(&domain, d)))
            .map(|a| a.portal())
    }
}

impl fmt::Display for Portal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Identity of a job across all mails and runs.
///
/// `id` is the portal's job id (digits only) or - only for freelancermap links of the form
/// `/projekt/<slug>` that carry no id - `u` + 12 hex characters of a hash over domain and
/// path. Read from outside (interface), the form is checked: the id ends up in file names
/// and must never contain anything else.
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
            Err(format!("invalid job id `{}`", raw.id))
        }
    }
}

impl JobKey {
    /// Does the key carry a real portal id (instead of a hash)?
    pub fn has_portal_id(&self) -> bool {
        all_digits(&self.id, 1)
    }

    /// `portal:id` - the form of `dup_of`.
    pub fn parse(text: &str) -> Option<JobKey> {
        let (portal, id) = text.split_once(':')?;
        JobKey::try_from(RawJobKey {
            portal: Portal::from_key(portal)?,
            id: id.to_string(),
        })
        .ok()
    }
}

impl fmt::Display for JobKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.portal.key(), self.id)
    }
}

/// A recognised job link: identity and the cleaned link for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobLink {
    pub key: JobKey,
    /// Link to show - without tracking parameters, always https.
    pub url: Url,
}

/// Recognises a job link. `None` for everything that is no job of a registered portal.
///
/// Click trackers that carry the target unencrypted as a parameter are resolved once.
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
    PORTALS.iter().find_map(|a| a.job_link(url))
}

/// Link to show for a portal id.
pub(crate) fn canonical_url(portal: Portal, id: &str) -> Option<Url> {
    portal.adapter().canonical_url(id)
}

/// Where the app reads the full text of a job.
pub fn fetch_url(link: &JobLink) -> Url {
    link.key.portal.adapter().fetch_url(link)
}

/// Lower-case host and non-empty lower-case path segments of a URL. Path words are compared
/// regardless of case (like the old code with `re.I`): portals link in lower case,
/// forwarded links not always.
pub(crate) fn host_and_segments(url: &Url) -> Option<(String, Vec<String>)> {
    let host = url.host_str()?.to_ascii_lowercase();
    let segments = url
        .path_segments()
        .map(|s| {
            s.filter(|seg| !seg.is_empty())
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_default();
    Some((host, segments))
}

/// A job link with a digit id and its canonical URL.
pub(crate) fn link(portal: Portal, id: String) -> Option<JobLink> {
    let url = canonical_url(portal, &id)?;
    Some(JobLink {
        key: JobKey { portal, id },
        url,
    })
}

/// `host` is `domain` or a subdomain of it.
pub(crate) fn host_is(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

/// Longer digit runs are no job id: the id ends up in file names and comes back through the
/// interface, where longer ones are refused.
const MAX_ID_DIGITS: usize = 20;

pub(crate) fn all_digits(text: &str, min_len: usize) -> bool {
    (min_len..=MAX_ID_DIGITS).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit())
}

/// The first 6 bytes as 12 hex characters.
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

/// A CSS selector of a page parser (fixed and valid).
pub(crate) fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("valid selector")
}

/// A lazily built selector.
pub(crate) type Css = LazyLock<Selector>;

#[cfg(test)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "helpers return an Option so that assert_eq! stays readable"
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
            "https://www.linkedin.com/jobs/view/12345/", // too short
            // too long: no id, otherwise a row that could never be opened or fetched
            "https://www.linkedin.com/jobs/view/412345678901234567890/",
            "",
        ] {
            assert_eq!(key(raw), None, "{raw}");
        }
    }

    /// Formerly the old pattern took the first run of 6+ digits after a dash - here
    /// "20260915" instead of the job id.
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

    /// Two link forms of the same project give the same key.
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

    /// Formerly digits from the query, the slug or a foreign host became the id.
    /// `/projekt/<slug>` without an id at its end carries none - a postal code or a norm
    /// number in the slug must not become the id.
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
        assert_ne!(b.key, c.key, "two projects, two keys");
        // Tracking parameters do not change the key.
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
        // The link shown is always https, even when the mail links http.
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

    /// The id is only read at fixed positions.
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

    /// Case in path and parameter does not matter (old code: re.I).
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

    /// Subdomains gave a non-existent address and another key.
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

    /// A job key from the interface is checked at the border - the id ends up in file
    /// names.
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
        assert_eq!(JobKey::parse(&ok.to_string()), Some(ok));
        assert_eq!(JobKey::parse("linkedin:../x"), None);
        assert_eq!(JobKey::parse("xing:123"), None);
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
        // freelancermap.de is never freelance.de, foreign domains with the same ending
        // neither.
        assert_ne!(
            Portal::from_sender_domain("freelancermap.de"),
            Some(Portal::FreelanceDe)
        );
        assert_eq!(Portal::from_sender_domain("notfreelance.de"), None);
        assert_eq!(Portal::from_sender_domain("gmail.com"), None);
    }

    #[test]
    fn keys_labels_access_and_serde() {
        assert_eq!(
            Portal::ALL.map(Portal::key),
            ["linkedin", "freelance", "freelancermap"]
        );
        assert_eq!(
            Portal::ALL.map(Portal::access),
            [
                Access::Guest,
                Access::Session { required: false },
                Access::Guest
            ]
        );
        assert_eq!(
            Portal::ALL.map(Portal::file_tag),
            ["LinkedIn", "Freelance", "Freelancermap"]
        );
        for p in Portal::ALL {
            assert_eq!(Portal::from_key(p.key()), Some(p));
            assert_eq!(p.adapter().portal(), p);
            assert_eq!(
                serde_json::to_string(&p).unwrap(),
                format!("\"{}\"", p.key())
            );
            // A session window exactly where the portal offers a sign-in.
            assert_eq!(p.adapter().session().is_some(), p.access().can_sign_in());
        }
        // An unknown portal from the interface is refused.
        assert!(serde_json::from_str::<Portal>("\"../../x\"").is_err());
        assert_eq!(Portal::from_key("xing"), None);
        // Every registered portal exactly once.
        for (i, a) in PORTALS.iter().enumerate() {
            assert!(PORTALS[..i].iter().all(|b| b.portal() != a.portal()));
        }
    }

    /// The path of a run: a session window only with the sign-in switched on.
    #[test]
    fn fetch_paths() {
        let required = Access::Session { required: true };
        let optional = Access::Session { required: false };
        assert_eq!(Access::Guest.path(false), Some(FetchPath::Guest));
        assert_eq!(Access::Guest.path(true), Some(FetchPath::Guest));
        assert_eq!(required.path(false), None);
        assert_eq!(required.path(true), Some(FetchPath::Session));
        assert_eq!(optional.path(false), Some(FetchPath::Guest));
        assert_eq!(optional.path(true), Some(FetchPath::Session));
    }

    /// Every link form of the old engine (`legacy-python`, `alerts.py`) keeps its id - and
    /// the forms of one project share one key, so no job appears twice.
    #[test]
    fn every_legacy_url_form_keeps_its_id() {
        let forms: [(&str, Option<(Portal, String)>); 17] = [
            // LinkedIn: /jobs/view/<ID>, /comm/jobs/view/<ID>, a slug before the id.
            (
                "https://www.linkedin.com/jobs/view/4123456789",
                li("4123456789"),
            ),
            (
                "https://www.linkedin.com/comm/jobs/view/4123456789/",
                li("4123456789"),
            ),
            (
                "https://www.linkedin.com/jobs/view/cfo-4123456789",
                li("4123456789"),
            ),
            // freelance.de: index.php?...id=, /projekte/projekt-<ID>, /projekt-<ID>,
            // /projekt<ID> (optional dash).
            (
                "https://www.freelance.de/project/index.php?a=1&id=1255067",
                fl("1255067"),
            ),
            (
                "https://www.freelance.de/projekte/projekt-1255067-sap",
                fl("1255067"),
            ),
            ("https://www.freelance.de/projekt-1255067", fl("1255067")),
            ("https://www.freelance.de/projekt1255067", fl("1255067")),
            // freelancermap: /nproj/<ID>, /projektboerse/projekte?/<categories>/<ID>-slug,
            // /projekt/<slug>-<ID>, /project/<slug>-<ID> (.com).
            ("https://www.freelancermap.de/nproj/2971857", fm("2971857")),
            (
                "https://www.freelancermap.de/nproj/2971857.html",
                fm("2971857"),
            ),
            (
                "https://www.freelancermap.de/projektboerse/projekte/it/sap/2971857-sap.html",
                fm("2971857"),
            ),
            (
                "https://www.freelancermap.de/projektboerse/projekt/2971857-sap-fi",
                fm("2971857"),
            ),
            (
                "https://www.freelancermap.de/projektboerse/projekte/it/2971857.html",
                fm("2971857"),
            ),
            (
                "https://www.freelancermap.de/projektboerse/projekte/it/sap-fi-2971857.html",
                fm("2971857"),
            ),
            (
                "https://www.freelancermap.de/projekt/sap-fi-co-berater-m-w-d-2971857",
                fm("2971857"),
            ),
            (
                "https://www.freelancermap.com/project/sap-fi-co-consultant-2971857",
                fm("2971857"),
            ),
            // A postal code at the end of a slug is no id (the old engine read one).
            (
                "https://www.freelancermap.de/projekt/sap-berater-muenchen-80331",
                None,
            ),
            (
                "https://www.freelancermap.de/projektboerse/projekte/it",
                None,
            ),
        ];
        for (raw, expected) in forms {
            match expected {
                Some(_) => assert_eq!(key(raw), expected, "{raw}"),
                // Without an id: a hash key, never a digit id.
                None => assert!(
                    job_link(raw).is_none_or(|l| !l.key.has_portal_id()),
                    "{raw}"
                ),
            }
        }
        // The slug form with an id and the agent link are one job.
        let slug =
            job_link("https://www.freelancermap.de/projekt/sap-fi-co-berater-2971857").unwrap();
        let agent = job_link("https://www.freelancermap.de/nproj/2971857.html").unwrap();
        assert_eq!(slug.key, agent.key);
        assert_eq!(slug.url, agent.url);
    }

    /// The test portal is recognised through the registry alone.
    #[test]
    fn a_fourth_portal_through_the_registry() {
        let probe = job_link("https://jobs.probe.example/job/4711").unwrap();
        assert_eq!(probe.key.portal, Portal::Probe);
        assert_eq!(Portal::from_key("probe"), Some(Portal::Probe));
        assert_eq!(
            Portal::from_sender_domain("alerts.probe.example"),
            Some(Portal::Probe)
        );
        assert!(!Portal::ALL.contains(&Portal::Probe));
    }
}
