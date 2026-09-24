//! HTTP fetching of guest pages - no account, no window. Which address, how redirects are
//! treated and how a page reads is the portal adapter's business; this client only speaks
//! HTTP.
//!
//! The client looks like a current browser of the OS the app runs on: the user agent is a
//! plain parameter of [`HttpFetcher::new`], injected by the app (`src-tauri/src/platform.rs`
//! keeps one maintained value per OS); `Accept` and `Accept-Language` as measured,
//! `Accept-Encoding` is set by reqwest itself. No invented headers, no rotation. Cookies
//! live for one run only (new client per run) - a sign-in can structurally never occur in
//! it.

use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, LOCATION};
use reqwest::{Client, Response, StatusCode, redirect};
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{Cause, PageFetcher, PageOutcome};
use crate::portal::{JobLink, Redirects};

const TIMEOUT: Duration = Duration::from_secs(30);
/// Bigger is no ad - then something is wrong.
const MAX_BODY: usize = 5 * 1024 * 1024;

pub struct HttpFetcher {
    /// Judges every redirect itself.
    direct: Client,
    /// Follows at most two redirects on the same host and scheme.
    follow: Client,
    /// Tests only: every request goes to this server instead of the portals.
    #[cfg(test)]
    base: Option<Url>,
}

impl HttpFetcher {
    /// `user_agent` is sent unchanged with every request (see the module docs).
    pub fn new(user_agent: &str) -> Result<HttpFetcher, reqwest::Error> {
        let (direct, follow) = Self::clients(user_agent)?;
        Ok(HttpFetcher {
            direct,
            follow,
            #[cfg(test)]
            base: None,
        })
    }

    /// Like [`HttpFetcher::new`], but every request goes to `base` (local test server).
    #[cfg(test)]
    pub(crate) fn with_base(user_agent: &str, base: Url) -> Result<HttpFetcher, reqwest::Error> {
        let (direct, follow) = Self::clients(user_agent)?;
        Ok(HttpFetcher {
            direct,
            follow,
            base: Some(base),
        })
    }

    /// Two clients - they only differ in how they treat redirects.
    fn clients(user_agent: &str) -> Result<(Client, Client), reqwest::Error> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("de-DE,de;q=0.9"));
        let client = |redirects: redirect::Policy| {
            Client::builder()
                .user_agent(user_agent)
                .default_headers(headers.clone())
                .cookie_store(true)
                // Like a browser after a typed address: no referer, not even on the redirect
                // (only the measured headers).
                .referer(false)
                .timeout(TIMEOUT)
                .redirect(redirects)
                .build()
        };
        Ok((
            client(redirect::Policy::none())?,
            // At most two redirects, only on the same host and scheme (freelancermap:
            // /nproj/<ID>.html leads with 301 to the project page).
            client(redirect::Policy::custom(|attempt| {
                let first = attempt.previous().first();
                let same_origin = first.is_some_and(|f| {
                    f.host_str() == attempt.url().host_str() && f.scheme() == attempt.url().scheme()
                });
                if attempt.previous().len() > 2 || !same_origin {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))?,
        ))
    }

    /// In tests every request goes to the local server, otherwise to the portal itself.
    #[cfg(test)]
    fn target(&self, url: Url) -> Url {
        let Some(base) = &self.base else { return url };
        let mut moved = base.clone();
        moved.set_path(url.path());
        moved.set_query(url.query());
        moved
    }

    #[cfg(not(test))]
    #[expect(clippy::unused_self, reason = "in tests it points to the test server")]
    fn target(&self, url: Url) -> Url {
        url
    }

    async fn page(&self, link: &JobLink) -> PageOutcome {
        let adapter = link.key.portal.adapter();
        let client = match adapter.redirects() {
            Redirects::Never => &self.direct,
            Redirects::SameOrigin => &self.follow,
        };
        let response = match client
            .get(self.target(adapter.fetch_url(link)))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return net_error(&e),
        };
        let status = response.status();
        if status.is_redirection() {
            return adapter.redirect_outcome(&location_path(&response));
        }
        if let Some(outcome) = status_outcome(status) {
            return outcome;
        }
        let path = response.url().path().to_ascii_lowercase();
        match body(response).await {
            Ok(html) => adapter.guest_page(&html, &path, link),
            Err(outcome) => outcome,
        }
    }
}

impl PageFetcher for HttpFetcher {
    async fn fetch(&mut self, link: &JobLink, cancel: &CancellationToken) -> PageOutcome {
        tokio::select! {
            biased;
            () = cancel.cancelled() => PageOutcome::Cancelled,
            outcome = self.page(link) => outcome,
        }
    }
}

/// Status codes that decide without looking at the content (`None` = judge the page).
fn status_outcome(status: StatusCode) -> Option<PageOutcome> {
    let code = status.as_u16();
    Some(match code {
        200 => return None,
        404 | 410 => PageOutcome::Gone,
        429 | 500..=599 => PageOutcome::Throttled(Cause::Http(code)),
        // 999 is LinkedIn's own "bot detected".
        403 | 999 => PageOutcome::Blocked(Cause::Http(code)),
        _ => PageOutcome::Suspicious(Cause::Http(code)),
    })
}

/// Path of the redirect target (lower case) - for a relative address too.
fn location_path(response: &Response) -> String {
    let target = response
        .headers()
        .get(LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    Url::parse(target)
        .or_else(|_| response.url().join(target))
        .map(|u| u.path().to_ascii_lowercase())
        .unwrap_or_default()
}

fn net_error(error: &reqwest::Error) -> PageOutcome {
    PageOutcome::NetError {
        timeout: error.is_timeout(),
        cause: if error.is_timeout() {
            Cause::Timeout
        } else if error.is_connect() {
            Cause::NoConnection
        } else {
            Cause::ConnectionLost
        },
    }
}

async fn body(mut response: Response) -> Result<String, PageOutcome> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| net_error(&e))? {
        if bytes.len() + chunk.len() > MAX_BODY {
            return Err(PageOutcome::Suspicious(Cause::PageTooLarge));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::portal::freelancermap_page as fm_page;
    use crate::portal::job_link;
    use crate::portal::linkedin_page as li_page;
    use crate::portal::{FREELANCE_DE_TEASER, freelance_de_page as fl_page};

    const UA: &str = "Mozilla/5.0 Test";

    async fn server() -> (MockServer, HttpFetcher) {
        crate::install_crypto();
        let server = MockServer::start().await;
        let fetcher = HttpFetcher::with_base(UA, server.uri().parse().unwrap()).unwrap();
        (server, fetcher)
    }

    async fn fetch(fetcher: &mut HttpFetcher, url: &str) -> PageOutcome {
        fetcher
            .fetch(&job_link(url).unwrap(), &CancellationToken::new())
            .await
    }

    /// Answer of the test server and the expected outcome.
    type Case = (ResponseTemplate, fn(&PageOutcome) -> bool);

    const LI: &str = "https://www.linkedin.com/jobs/view/4123456789/";
    const LI_GUEST: &str = "/jobs-guest/jobs/api/jobPosting/4123456789";

    #[tokio::test]
    async fn linkedin_matrix() {
        let (server, mut f) = server().await;
        let long = "Ausführliche Beschreibung. ".repeat(10);
        let cases: Vec<Case> = vec![
            (
                ResponseTemplate::new(200).set_body_string(li_page(&long, false)),
                |o| {
                    matches!(
                        o,
                        PageOutcome::Text {
                            short: false,
                            closed: false,
                            fields: Some(_),
                            ..
                        }
                    )
                },
            ),
            (
                ResponseTemplate::new(200).set_body_string(li_page("Kurz, aber echt.", false)),
                |o| matches!(o, PageOutcome::Text { short: true, .. }),
            ),
            (
                ResponseTemplate::new(200).set_body_string(li_page(&long, true)),
                |o| matches!(o, PageOutcome::Text { closed: true, .. }),
            ),
            (
                ResponseTemplate::new(200).set_body_string("<html>Bitte anmelden</html>"),
                |o| matches!(o, PageOutcome::Suspicious(Cause::NoDescription)),
            ),
            (ResponseTemplate::new(404), |o| {
                matches!(o, PageOutcome::Gone)
            }),
            (ResponseTemplate::new(429), |o| {
                matches!(o, PageOutcome::Throttled(Cause::Http(429)))
            }),
            (ResponseTemplate::new(503), |o| {
                matches!(o, PageOutcome::Throttled(_))
            }),
            (ResponseTemplate::new(999), |o| {
                matches!(o, PageOutcome::Blocked(Cause::Http(999)))
            }),
            (ResponseTemplate::new(403), |o| {
                matches!(o, PageOutcome::Blocked(_))
            }),
            (
                ResponseTemplate::new(302)
                    .insert_header("location", "https://www.linkedin.com/authwall?trk=x"),
                |o| matches!(o, PageOutcome::Blocked(Cause::LoginWall)),
            ),
        ];
        for (i, (response, expected)) in cases.into_iter().enumerate() {
            server.reset().await;
            Mock::given(method("GET"))
                .and(path(LI_GUEST))
                .respond_with(response)
                .expect(1)
                .mount(&server)
                .await;
            let outcome = fetch(&mut f, LI).await;
            assert!(expected(&outcome), "case {i}: {outcome:?}");
            // Exactly the measured headers - nothing invented, nothing compressed by hand.
            let request = &server.received_requests().await.unwrap()[0];
            let head = |name: &str| {
                request
                    .headers
                    .get(name)
                    .map(|v| v.to_str().unwrap().to_string())
            };
            assert_eq!(head("user-agent").as_deref(), Some(UA));
            assert_eq!(head("accept-language").as_deref(), Some("de-DE,de;q=0.9"));
            assert!(head("accept").unwrap().starts_with("text/html"));
            assert!(head("cookie").is_none());
        }
    }

    /// reqwest unpacks compressed answers itself (Accept-Encoding never by hand).
    #[tokio::test]
    async fn gzip_body() {
        use std::io::Write as _;
        let (server, mut f) = server().await;
        let html = li_page(&"Text ".repeat(40), false);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(html.as_bytes()).unwrap();
        Mock::given(path(LI_GUEST))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-encoding", "gzip")
                    .set_body_bytes(gz.finish().unwrap()),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            fetch(&mut f, LI).await,
            PageOutcome::Text { short: false, .. }
        ));
    }

    #[tokio::test]
    async fn freelancermap_redirects_and_checks() {
        let (server, mut f) = server().await;
        let long = "Projektbeschreibung mit allen Details. ".repeat(5);
        Mock::given(path("/nproj/2971857.html"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", "/projekt/sap-fi-co-berater"),
            )
            .mount(&server)
            .await;
        Mock::given(path("/projekt/sap-fi-co-berater"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fm_page(2_971_857, &long, false)),
            )
            .mount(&server)
            .await;
        let outcome = fetch(&mut f, "https://www.freelancermap.de/nproj/2971857.html").await;
        assert!(
            matches!(&outcome, PageOutcome::Text { short: false, fields: Some(fields), .. } if fields.company == "Ferrum Systems SE"),
            "{outcome:?}"
        );

        // The wrong project behind the redirect.
        Mock::given(path("/nproj/2971858.html"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", "/projekt/sap-fi-co-berater"),
            )
            .mount(&server)
            .await;
        assert_eq!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971858.html").await,
            PageOutcome::Suspicious(Cause::WrongPage)
        );

        // A redirect to a foreign host is not followed.
        Mock::given(path("/nproj/2971859.html"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", "https://example.org/x"),
            )
            .mount(&server)
            .await;
        assert_eq!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971859.html").await,
            PageOutcome::Suspicious(Cause::RedirectNotFollowed)
        );

        // A redirect to the search: maybe removed, maybe a wall - suspicious, never finally
        // "no longer exists".
        Mock::given(path("/nproj/2971860.html"))
            .respond_with(ResponseTemplate::new(301).insert_header("location", "/projektboerse"))
            .mount(&server)
            .await;
        Mock::given(path("/projektboerse"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>Suche</html>"))
            .mount(&server)
            .await;
        assert_eq!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971860.html").await,
            PageOutcome::Suspicious(Cause::NotAProjectPage)
        );

        // A redirect to the sign-in: a block signal.
        Mock::given(path("/nproj/2971862.html"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/login?next=x"))
            .mount(&server)
            .await;
        Mock::given(path("/login"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>Anmelden</html>"))
            .mount(&server)
            .await;
        assert_eq!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971862.html").await,
            PageOutcome::Blocked(Cause::LoginWall)
        );

        // Endless redirects: done after two steps.
        for (from, to) in [("/nproj/2971861.html", "/a"), ("/a", "/b"), ("/b", "/c")] {
            Mock::given(path(from))
                .respond_with(ResponseTemplate::new(301).insert_header("location", to))
                .mount(&server)
                .await;
        }
        assert!(matches!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971861.html").await,
            PageOutcome::Suspicious(_)
        ));
    }

    /// A redirect to a foreign host is judged at its target too: a sign-in page = a block
    /// signal. The followed redirect carries no referer.
    #[tokio::test]
    async fn a_redirect_to_a_foreign_login_is_a_block() {
        let (server, mut f) = server().await;
        Mock::given(path(LI_GUEST))
            .respond_with(
                ResponseTemplate::new(302).insert_header("location", "https://example.org/login"),
            )
            .mount(&server)
            .await;
        assert!(matches!(fetch(&mut f, LI).await, PageOutcome::Blocked(_)));

        server.reset().await;
        Mock::given(path("/nproj/2971863.html"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", "https://example.org/signin"),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971863.html").await,
            PageOutcome::Blocked(_)
        ));

        // A followed redirect on the same host: no referer.
        server.reset().await;
        let long = "Projektbeschreibung mit allen Details. ".repeat(5);
        Mock::given(path("/nproj/2971857.html"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", "/projekt/sap-fi-co-berater"),
            )
            .mount(&server)
            .await;
        Mock::given(path("/projekt/sap-fi-co-berater"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fm_page(2_971_857, &long, false)),
            )
            .mount(&server)
            .await;
        fetch(&mut f, "https://www.freelancermap.de/nproj/2971857.html").await;
        let followed = &server.received_requests().await.unwrap()[1];
        assert!(followed.headers.get("referer").is_none());
    }

    /// The test-only fourth portal goes through the same client - address, redirects and
    /// page come from its adapter alone.
    #[tokio::test]
    async fn a_fourth_portal_through_the_same_client() {
        let (server, mut f) = server().await;
        Mock::given(path("/job/4711"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(format!("<p>{}</p>", "Probe text. ".repeat(20))),
            )
            .expect(1)
            .mount(&server)
            .await;
        assert!(matches!(
            fetch(&mut f, "https://jobs.probe.example/job/4711").await,
            PageOutcome::Text { short: false, .. }
        ));
    }

    /// freelance.de without the sign-in: the public page as a guest gives the teaser; an
    /// expired project leads to a project list and is gone.
    #[tokio::test]
    async fn freelance_de_as_a_guest_gives_the_teaser() {
        let (server, mut f) = server().await;
        Mock::given(path("/project/index.php"))
            .respond_with(
                ResponseTemplate::new(301)
                    .insert_header("location", "/projekte/projekt-1255067-interim"),
            )
            .mount(&server)
            .await;
        Mock::given(path("/projekte/projekt-1255067-interim"))
            .respond_with(ResponseTemplate::new(200).set_body_string(fl_page(FREELANCE_DE_TEASER)))
            .mount(&server)
            .await;
        let outcome = fetch(
            &mut f,
            "https://www.freelance.de/project/index.php?id=1255067",
        )
        .await;
        assert!(
            matches!(&outcome, PageOutcome::Teaser { text, .. } if text.contains("Controller")),
            "{outcome:?}"
        );

        server.reset().await;
        Mock::given(path("/project/index.php"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/projekte/it"))
            .mount(&server)
            .await;
        Mock::given(path("/projekte/it"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>Liste</html>"))
            .mount(&server)
            .await;
        assert_eq!(
            fetch(
                &mut f,
                "https://www.freelance.de/project/index.php?id=1255068"
            )
            .await,
            PageOutcome::Gone
        );
    }

    #[tokio::test]
    async fn cancel_ends_a_hanging_request() {
        let (server, mut f) = server().await;
        Mock::given(path(LI_GUEST))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(20)))
            .mount(&server)
            .await;
        let cancel = CancellationToken::new();
        let link = job_link(LI).unwrap();
        let (outcome, ()) = tokio::join!(f.fetch(&link, &cancel), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel.cancel();
        });
        assert_eq!(outcome, PageOutcome::Cancelled);
    }
}
