//! HTTP-Abruf für LinkedIn (Gastansicht) und freelancermap – ohne Konto, ohne Fenster.
//!
//! Der Client sieht aus wie der installierte Edge: dessen User-Agent, `Accept` und
//! `Accept-Language` wie gemessen, `Accept-Encoding` setzt reqwest selbst. Keine erfundenen
//! Header, keine Rotation. Cookies leben nur für einen Lauf (neuer Client je Lauf) – eine
//! LinkedIn-Anmeldung kann darin strukturell nie vorkommen.

use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, LOCATION};
use reqwest::{Client, Response, StatusCode, redirect};
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{PageFetcher, PageOutcome, Route, freelancermap, judge, linkedin};
use crate::portal::{JobLink, Portal, fetch_url};

/// Edge-Hauptversion, falls die installierte WebView2-Version unbekannt ist.
const FALLBACK_EDGE_MAJOR: &str = "153";
const TIMEOUT: Duration = Duration::from_secs(30);
/// Größer ist keine Anzeige – dann stimmt etwas nicht.
const MAX_BODY: usize = 5 * 1024 * 1024;

/// User-Agent des installierten Edge (WebView2-Version, z. B. „153.0.3405.86“). Wie Edge
/// selbst nennt er nur die Hauptversion („User-Agent Reduction“).
pub fn edge_user_agent(webview_version: Option<&str>) -> String {
    // Nur plausible Versionen übernehmen: WebView2-Hauptversionen sind dreistellig. Was
    // sonst kommt (leer, Buchstaben, eine ganz andere Engine), ergäbe eine Kennung, die es
    // nirgends gibt – genau darauf reagiert Bot-Erkennung.
    let major = webview_version
        .and_then(|v| v.split('.').next())
        .filter(|m| {
            m.bytes().all(|b| b.is_ascii_digit()) && (100..=999).contains(&m.parse().unwrap_or(0))
        })
        .unwrap_or(FALLBACK_EDGE_MAJOR);
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/{major}.0.0.0 Safari/537.36 Edg/{major}.0.0.0"
    )
}

pub struct HttpFetcher {
    linkedin: Client,
    freelancermap: Client,
    /// Nur Tests: alle Anfragen an diesen Server statt an die Portale.
    #[cfg(test)]
    base: Option<Url>,
}

impl HttpFetcher {
    pub fn new(user_agent: &str) -> Result<HttpFetcher, reqwest::Error> {
        let (linkedin, freelancermap) = Self::clients(user_agent)?;
        Ok(HttpFetcher {
            linkedin,
            freelancermap,
            #[cfg(test)]
            base: None,
        })
    }

    /// Wie [`HttpFetcher::new`], aber jede Anfrage geht an `base` (lokaler Testserver).
    #[cfg(test)]
    fn with_base(user_agent: &str, base: Url) -> Result<HttpFetcher, reqwest::Error> {
        let (linkedin, freelancermap) = Self::clients(user_agent)?;
        Ok(HttpFetcher {
            linkedin,
            freelancermap,
            base: Some(base),
        })
    }

    /// Ein Client je Portal – sie unterscheiden sich nur darin, wie sie mit Umleitungen
    /// umgehen.
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
                // Wie ein Browser nach einer eingetippten Adresse: kein Referer, auch nicht
                // auf der Umleitung (nur die gemessenen Kopfzeilen).
                .referer(false)
                .timeout(TIMEOUT)
                .redirect(redirects)
                .build()
        };
        Ok((
            // LinkedIn: jede Umleitung selbst beurteilen – sie führt meist zur Anmeldung.
            client(redirect::Policy::none())?,
            // freelancermap: /nproj/<ID>.html leitet (301) auf die Projektseite; höchstens
            // zwei Umleitungen, nur auf demselben Host und Schema.
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

    /// Im Test geht jede Anfrage an den lokalen Server, sonst an das Portal selbst.
    #[cfg(test)]
    fn target(&self, url: Url) -> Url {
        let Some(base) = &self.base else { return url };
        let mut moved = base.clone();
        moved.set_path(url.path());
        moved.set_query(url.query());
        moved
    }

    #[cfg(not(test))]
    #[expect(clippy::unused_self, reason = "im Test lenkt sie auf den Testserver")]
    fn target(&self, url: Url) -> Url {
        url
    }

    async fn linkedin(&self, link: &JobLink) -> PageOutcome {
        let response = match self.linkedin.get(self.target(fetch_url(link))).send().await {
            Ok(r) => r,
            Err(e) => return net_error(&e),
        };
        let status = response.status();
        if status.is_redirection() {
            return linkedin_redirect(&location_path(&response));
        }
        if let Some(outcome) = status_outcome(status) {
            return outcome;
        }
        match body(response).await {
            Ok(html) => judge(linkedin::parse(&html)),
            Err(outcome) => outcome,
        }
    }

    async fn freelancermap(&self, link: &JobLink) -> PageOutcome {
        let response = match self
            .freelancermap
            .get(self.target(fetch_url(link)))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return net_error(&e),
        };
        let status = response.status();
        if status.is_redirection() {
            // Nicht gefolgt (fremder Host, dritte Umleitung): das Ziel trotzdem beurteilen –
            // eine Anmelde- oder Prüfseite ist auch dort ein Sperrsignal.
            return match freelancermap_landing(&location_path(&response)) {
                PageOutcome::Blocked(reason) => PageOutcome::Blocked(reason),
                _ => PageOutcome::Suspicious("Umleitung nicht verfolgt".into()),
            };
        }
        if let Some(outcome) = status_outcome(status) {
            return outcome;
        }
        let path = response.url().path().to_ascii_lowercase();
        let html = match body(response).await {
            Ok(html) => html,
            Err(outcome) => return outcome,
        };
        let expected = link.key.has_portal_id().then_some(link.key.id.as_str());
        match freelancermap::parse(&html, expected) {
            // Auf einer Anmelde- oder Prüfseite ist auch ein unerkennbarer Inhalt ein Sperrsignal.
            Err(_) if !is_project_path(&path) => freelancermap_landing(&path),
            Err(reason) => PageOutcome::Suspicious(reason),
            Ok(parsed) if parsed.text.is_none() && !is_project_path(&path) => {
                freelancermap_landing(&path)
            }
            Ok(parsed) => judge(parsed),
        }
    }
}

impl PageFetcher for HttpFetcher {
    async fn fetch(
        &mut self,
        link: &JobLink,
        _route: Route,
        cancel: &CancellationToken,
    ) -> PageOutcome {
        let work = async {
            match link.key.portal {
                Portal::LinkedIn => self.linkedin(link).await,
                Portal::Freelancermap => self.freelancermap(link).await,
                Portal::FreelanceDe => PageOutcome::Suspicious("nur im Sitzungsfenster".into()),
            }
        };
        tokio::select! {
            biased;
            () = cancel.cancelled() => PageOutcome::Cancelled,
            outcome = work => outcome,
        }
    }
}

/// Statuscodes, die ohne Blick auf den Inhalt entscheiden (`None` = Seite auswerten).
fn status_outcome(status: StatusCode) -> Option<PageOutcome> {
    let code = status.as_u16();
    Some(match code {
        200 => return None,
        404 | 410 => PageOutcome::Gone,
        429 => PageOutcome::Throttled(format!("HTTP {code} (zu viele Anfragen)")),
        // 999 ist LinkedIns eigenes „Bot erkannt“.
        403 | 999 => PageOutcome::Blocked(format!("HTTP {code} (Zugriff verweigert)")),
        500..=599 => PageOutcome::Throttled(format!("HTTP {code} (Serverfehler)")),
        _ => PageOutcome::Suspicious(format!("HTTP {code}")),
    })
}

fn is_project_path(path: &str) -> bool {
    ["/projekt/", "/project/", "/nproj/"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

/// freelancermap hat auf eine Seite ohne Projekttext umgeleitet. „Gibt es nicht mehr“
/// entscheiden allein 404/410 (nie gemessen, wohin ein gelöschtes Projekt führt): Eine
/// Anmelde- oder Prüfseite ist ein Sperrsignal, alles andere bleibt verdächtig – der Job
/// wird später erneut versucht, und der Schutzschalter greift.
fn freelancermap_landing(path: &str) -> PageOutcome {
    // Ganze Pfadteile, ohne Endung und mit „_“ wie „-“: „/users/sign_in“, „/login.php“.
    let wall = path.split('/').any(|segment| {
        let stem = segment
            .split('.')
            .next()
            .unwrap_or_default()
            .replace('_', "-");
        matches!(
            stem.as_str(),
            "login"
                | "log-in"
                | "signin"
                | "sign-in"
                | "anmelden"
                | "anmeldung"
                | "register"
                | "registrieren"
                | "authwall"
                | "checkpoint"
                | "captcha"
                | "challenge"
                | "cdn-cgi"
        )
    });
    if wall {
        PageOutcome::Blocked("Umleitung zur Anmeldung/Sicherheitsprüfung".into())
    } else {
        PageOutcome::Suspicious(format!("keine Projektseite ({path})"))
    }
}

/// Pfad des Umleitungsziels (klein geschrieben) – auch bei einer relativen Adresse.
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

/// Umleitungen der Gastansicht führen fast immer in die Anmeldewand – ein Sperrsignal.
fn linkedin_redirect(path: &str) -> PageOutcome {
    // Ganze Pfadteile vergleichen: „/loginhilfe“ ist keine Anmeldeseite.
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let wall = matches!(
        segments.as_slice(),
        ["authwall" | "login" | "checkpoint" | "signup", ..] | ["uas", "login", ..]
    );
    if wall {
        PageOutcome::Blocked("Umleitung zur Anmeldung/Sicherheitsprüfung".into())
    } else {
        PageOutcome::Suspicious(format!("unerwartete Umleitung nach {path}"))
    }
}

fn net_error(error: &reqwest::Error) -> PageOutcome {
    PageOutcome::NetError {
        timeout: error.is_timeout(),
        detail: if error.is_timeout() {
            "keine Antwort nach 30 Sekunden".into()
        } else if error.is_connect() {
            "keine Verbindung".into()
        } else {
            "Verbindung abgebrochen".into()
        },
    }
}

async fn body(mut response: Response) -> Result<String, PageOutcome> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| net_error(&e))? {
        if bytes.len() + chunk.len() > MAX_BODY {
            return Err(PageOutcome::Suspicious("Seite ungewöhnlich groß".into()));
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
    use crate::portal::job_link;

    #[test]
    fn user_agent_follows_the_installed_edge() {
        assert_eq!(
            edge_user_agent(Some("140.0.3485.54")),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0"
        );
        assert!(edge_user_agent(None).contains("Edg/153.0.0.0"));
        assert!(edge_user_agent(Some("kaputt")).contains("Edg/153.0.0.0"));
    }

    #[test]
    fn redirects_of_the_guest_view() {
        for wall in [
            "/authwall",
            "/uas/login",
            "/checkpoint/challenge/agf",
            "/login",
            "/signup/cold-join",
        ] {
            assert!(
                matches!(linkedin_redirect(wall), PageOutcome::Blocked(_)),
                "{wall}"
            );
        }
        assert!(matches!(
            linkedin_redirect("/jobs/view/4123456789/"),
            PageOutcome::Suspicious(_)
        ));
        // „/loginhilfe“ ist keine Anmeldeseite.
        assert!(matches!(
            linkedin_redirect("/loginhilfe"),
            PageOutcome::Suspicious(_)
        ));
    }

    const UA: &str = "Mozilla/5.0 Test";

    async fn server() -> (MockServer, HttpFetcher) {
        crate::install_crypto();
        let server = MockServer::start().await;
        let fetcher = HttpFetcher::with_base(UA, server.uri().parse().unwrap()).unwrap();
        (server, fetcher)
    }

    async fn fetch(fetcher: &mut HttpFetcher, url: &str) -> PageOutcome {
        fetcher
            .fetch(
                &job_link(url).unwrap(),
                Route::Http,
                &CancellationToken::new(),
            )
            .await
    }

    /// Antwort des Testservers und erwartetes Ergebnis.
    type Case = (ResponseTemplate, fn(&PageOutcome) -> bool);

    const LI: &str = "https://www.linkedin.com/jobs/view/4123456789/";
    const LI_GUEST: &str = "/jobs-guest/jobs/api/jobPosting/4123456789";

    #[tokio::test]
    async fn linkedin_matrix() {
        let (server, mut f) = server().await;
        let long = "Ausführliche Beschreibung. ".repeat(10);
        let cases: Vec<Case> = vec![
            (
                ResponseTemplate::new(200).set_body_string(linkedin::tests::page(&long, false)),
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
                ResponseTemplate::new(200)
                    .set_body_string(linkedin::tests::page("Kurz, aber echt.", false)),
                |o| matches!(o, PageOutcome::Text { short: true, .. }),
            ),
            (
                ResponseTemplate::new(200).set_body_string(linkedin::tests::page(&long, true)),
                |o| matches!(o, PageOutcome::Text { closed: true, .. }),
            ),
            (
                ResponseTemplate::new(200).set_body_string("<html>Bitte anmelden</html>"),
                |o| matches!(o, PageOutcome::Suspicious(_)),
            ),
            (ResponseTemplate::new(404), |o| {
                matches!(o, PageOutcome::Gone)
            }),
            (ResponseTemplate::new(429), |o| {
                matches!(o, PageOutcome::Throttled(_))
            }),
            (ResponseTemplate::new(503), |o| {
                matches!(o, PageOutcome::Throttled(_))
            }),
            (ResponseTemplate::new(999), |o| {
                matches!(o, PageOutcome::Blocked(_))
            }),
            (ResponseTemplate::new(403), |o| {
                matches!(o, PageOutcome::Blocked(_))
            }),
            (
                ResponseTemplate::new(302)
                    .insert_header("location", "https://www.linkedin.com/authwall?trk=x"),
                |o| matches!(o, PageOutcome::Blocked(_)),
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
            assert!(expected(&outcome), "Fall {i}: {outcome:?}");
            // Genau die gemessenen Header – nichts erfunden, nichts von Hand komprimiert.
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

    /// Komprimierte Antworten entpackt reqwest selbst (Accept-Encoding nie von Hand).
    #[tokio::test]
    async fn gzip_body() {
        use std::io::Write as _;
        let (server, mut f) = server().await;
        let html = linkedin::tests::page(&"Text ".repeat(40), false);
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
                ResponseTemplate::new(200)
                    .set_body_string(freelancermap::tests::page(2_971_857, &long, false)),
            )
            .mount(&server)
            .await;
        let outcome = fetch(&mut f, "https://www.freelancermap.de/nproj/2971857.html").await;
        assert!(
            matches!(&outcome, PageOutcome::Text { short: false, fields: Some(fields), .. } if fields.company == "Ferrum Systems SE"),
            "{outcome:?}"
        );

        // Falsches Projekt hinter der Umleitung.
        Mock::given(path("/nproj/2971858.html"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", "/projekt/sap-fi-co-berater"),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971858.html").await,
            PageOutcome::Suspicious(_)
        ));

        // Umleitung auf einen fremden Host wird nicht verfolgt.
        Mock::given(path("/nproj/2971859.html"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", "https://example.org/x"),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971859.html").await,
            PageOutcome::Suspicious(_)
        ));

        // Umleitung auf die Suche: vielleicht entfernt, vielleicht eine Wand – verdächtig,
        // nie endgültig „gibt es nicht mehr“.
        Mock::given(path("/nproj/2971860.html"))
            .respond_with(ResponseTemplate::new(301).insert_header("location", "/projektboerse"))
            .mount(&server)
            .await;
        Mock::given(path("/projektboerse"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>Suche</html>"))
            .mount(&server)
            .await;
        assert!(matches!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971860.html").await,
            PageOutcome::Suspicious(_)
        ));

        // Umleitung zur Anmeldung: Sperrsignal.
        Mock::given(path("/nproj/2971862.html"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/login?next=x"))
            .mount(&server)
            .await;
        Mock::given(path("/login"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>Anmelden</html>"))
            .mount(&server)
            .await;
        assert!(matches!(
            fetch(&mut f, "https://www.freelancermap.de/nproj/2971862.html").await,
            PageOutcome::Blocked(_)
        ));
        for wall in ["/users/sign_in", "/login.php", "/de/Anmelden/"] {
            assert!(
                matches!(
                    freelancermap_landing(&wall.to_ascii_lowercase()),
                    PageOutcome::Blocked(_)
                ),
                "{wall}"
            );
        }
        assert!(matches!(
            freelancermap_landing("/projektboerse"),
            PageOutcome::Suspicious(_)
        ));

        // Endlose Umleitungen: nach zwei Schritten Schluss.
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

    /// Auch eine Umleitung auf einen fremden Host wird am Ziel gemessen: Anmeldeseite =
    /// Sperrsignal. Der verfolgten Umleitung schickt der Client keinen Referer mit.
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

        // Verfolgte Umleitung auf demselben Host: kein Referer.
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
                ResponseTemplate::new(200)
                    .set_body_string(freelancermap::tests::page(2_971_857, &long, false)),
            )
            .mount(&server)
            .await;
        fetch(&mut f, "https://www.freelancermap.de/nproj/2971857.html").await;
        let followed = &server.received_requests().await.unwrap()[1];
        assert!(followed.headers.get("referer").is_none());
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
        let (outcome, ()) = tokio::join!(f.fetch(&link, Route::Http, &cancel), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel.cancel();
        });
        assert_eq!(outcome, PageOutcome::Cancelled);
    }
}
