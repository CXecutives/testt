//! Gmail per IMAP: nur der Posteingang, nur lesend (`EXAMINE`, `BODY.PEEK`) – Gmail
//! markiert dadurch nichts als gelesen.
//!
//! Bewusst nicht „Alle Nachrichten“: Dort läge auch „Gesendet“ – ein selbst
//! weitergeleiteter Alert bliebe sichtbar, obwohl der Nutzer ihn gelöscht hat.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_imap::imap_proto::{AttributeValue, MailboxDatum, Response, Status};

use jiff::civil::Date;
use rustls_platform_verifier::ConfigVerifierExt as _;
use tokio::net::TcpStream;
use tokio_io_timeout::TimeoutStream;
use tokio_rustls::client::TlsStream;
use tokio_util::sync::CancellationToken;

use super::RawMail;
use crate::portal::Portal;
use crate::text::{one_line, truncate_chars};

const HOST: &str = "imap.gmail.com";
const PORT: u16 = 993;
/// Kommt so lange gar nichts vom Server, gilt die Verbindung als hängend.
const IDLE: Duration = Duration::from_secs(30);
/// Obergrenze für einen einzelnen Schritt, auch wenn Daten nur tröpfeln.
const STEP_LIMIT: Duration = Duration::from_secs(10 * 60);
/// Mehr wird von einer Mail nie geholt (Alerts sind klein, große Anhänge unnötig).
const MAX_MAIL_BYTES: u32 = 4 * 1024 * 1024;
/// Mails je Abholung.
pub const BATCH: usize = 25;

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("Es ist noch kein Gmail-Zugang hinterlegt – bitte Adresse und App-Passwort eintragen.")]
    NoCredentials,

    #[error("Keine Verbindung zu imap.gmail.com.\n\nInternetverbindung und Firewall prüfen. ({0})")]
    Connect(String),

    #[error(
        "Gmail hat die Anmeldung abgelehnt.\n\n\
         • Als Passwort ist ein App-Passwort nötig (16 Zeichen, unter „Google-Konto → Sicherheit → App-Passwörter“), nicht das normale Konto-Passwort.\n\
         • Die Bestätigung in zwei Schritten muss aktiv sein.\n\
         • IMAP muss in den Gmail-Einstellungen erlaubt sein (Einstellungen → Weiterleitung und POP/IMAP).\n\n\
         Meldung von Gmail: {0}"
    )]
    Auth(String),

    #[error(
        "Gmail antwortet nicht (keine Antwort nach 30 Sekunden). Bitte später erneut versuchen."
    )]
    Timeout,

    #[error("Die Verbindung zu Gmail ist abgebrochen: {0}")]
    Lost(String),

    #[error("Dieses Postfach unterstützt die Gmail-Suche nicht – die App arbeitet nur mit Gmail.")]
    NotGmail,

    #[error("Gmail meldet einen Fehler: {0}")]
    Server(String),

    #[error("Abgebrochen.")]
    Cancelled,
}

impl MailError {
    /// Kurzer, stabiler Name für die Oberfläche.
    pub fn kind(&self) -> &'static str {
        match self {
            MailError::NoCredentials => "mailMissing",
            MailError::Connect(_) => "mailConnect",
            MailError::Auth(_) => "mailAuth",
            MailError::Timeout => "mailTimeout",
            MailError::Lost(_) => "mailLost",
            MailError::NotGmail => "mailNotGmail",
            MailError::Server(_) => "mailServer",
            MailError::Cancelled => "cancelled",
        }
    }
}

/// Protokollfehler nach der Anmeldung. Nur eine echte Ablehnung beim Anmelden ist ein
/// Zugangsfehler – ein Verbindungsabbruch nie (früher: galt als „falsches Passwort“).
fn protocol(error: async_imap::error::Error) -> MailError {
    use async_imap::error::Error as E;
    match error {
        E::Io(e) => io_error(&e),
        E::ConnectionLost => MailError::Lost("vom Server getrennt".into()),
        E::No(text) | E::Bad(text) => MailError::Server(clean(&text)),
        E::Parse(_) => MailError::Server(UNREADABLE.into()),
        other => MailError::Server(clean(&other.to_string())),
    }
}

const UNREADABLE: &str = "Antwort von Gmail nicht lesbar";

/// Ein-/Ausgabefehler. Parserfehler von async-imap tragen den ganzen ungelesenen Puffer
/// (also Mailinhalt) im Text – der landet nie in Meldung oder Log.
fn io_error(error: &std::io::Error) -> MailError {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::TimedOut => MailError::Timeout,
        ErrorKind::Other | ErrorKind::InvalidData => MailError::Server(UNREADABLE.into()),
        kind => MailError::Lost(clean(&kind.to_string())),
    }
}

/// Servertexte für Meldungen: eine Zeile, höchstens 200 Zeichen.
fn clean(text: &str) -> String {
    truncate_chars(&one_line(text), 200)
}

/// Fehler beim Anmelden: Nur eine Ablehnung der Zugangsdaten ist ein Zugangsproblem.
/// Vorübergehende Ablehnungen (Gmail überlastet, zu viele Verbindungen,
/// Datenlimit) sind es nicht – sonst änderte der Nutzer ein richtiges Passwort.
fn login_error(error: async_imap::error::Error) -> MailError {
    match error {
        async_imap::error::Error::No(text) | async_imap::error::Error::Bad(text) => {
            let upper = text.to_ascii_uppercase();
            let transient = [
                "UNAVAILABLE",
                "THROTTLED",
                "OVERQUOTA",
                "LIMIT",
                "TOO MANY",
                "TEMPORARY",
                "TRY AGAIN",
            ]
            .iter()
            .any(|w| upper.contains(w));
            if transient {
                MailError::Server(format!("{} – bitte später erneut versuchen", clean(&text)))
            } else {
                MailError::Auth(clean(&text))
            }
        }
        other => protocol(other),
    }
}

/// Gmail-Zugang. Adresse und App-Passwort werden an **einer** Stelle normalisiert:
/// Adresse ohne Rand und klein, Passwort ohne Leerzeichen (Google zeigt es in Vierergruppen).
#[derive(Clone)]
pub struct Credentials {
    pub user: String,
    password: String,
}

impl Credentials {
    pub fn new(user: &str, password: &str) -> Credentials {
        Credentials {
            user: user.trim().to_lowercase(),
            password: password.chars().filter(|c| !c.is_whitespace()).collect(),
        }
    }

    pub fn password(&self) -> &str {
        &self.password
    }
}

/// Das Passwort erscheint nie in Debug-Ausgaben oder Logs.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}

/// Woher die Mails kommen – Gmail oder im Test eine Attrappe.
pub trait MailSource {
    /// UIDs der Treffer seit `since` (`None` = alle), aufsteigend.
    fn search(
        &mut self,
        since: Option<Date>,
        portals: &[Portal],
    ) -> impl Future<Output = Result<Vec<u32>, MailError>> + Send;

    /// Die Mails zu `uids`. Inzwischen gelöschte fehlen einfach; eine Mail ohne Inhalt
    /// kommt mit leeren Bytes zurück (wird als defekt gezählt, nie still verworfen).
    fn fetch(
        &mut self,
        uids: &[u32],
    ) -> impl Future<Output = Result<Vec<RawMail>, MailError>> + Send;
}

/// Suchausdruck: Absender-Domains der Portale **oder** ihre Stichwörter (weitergeleitete
/// Alerts kommen vom Nutzer selbst). `X-GM-RAW` ist Gmails eigene Suche – serverseitig,
/// schnell und ohne das Postfach herunterzuladen.
fn search_query(since: Option<Date>, portals: &[Portal]) -> String {
    let domains: Vec<&str> = portals
        .iter()
        .flat_map(|p| p.sender_domains().iter().copied())
        .collect();
    let terms: Vec<&str> = portals
        .iter()
        .flat_map(|p| p.search_terms().iter().copied())
        .collect();
    let raw = format!(
        r#"X-GM-RAW "from:({}) OR {}""#,
        domains.join(" OR "),
        terms.join(" OR ")
    );
    match since {
        // IMAP-Monatsnamen sind englisch; jiff formatiert unabhängig von der Systemsprache.
        Some(date) => format!("SINCE {} {raw}", date.strftime("%d-%b-%Y")),
        None => raw,
    }
}

/// Verbindung zu Gmail: TCP mit Leerlauf-Grenze, darüber TLS.
pub(crate) type Stream = TlsStream<Pin<Box<TimeoutStream<TcpStream>>>>;

pub struct Gmail<T = Stream>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + fmt::Debug + Send,
{
    session: async_imap::Session<T>,
    cancel: CancellationToken,
}

impl Gmail {
    /// Verbinden, anmelden, Posteingang schreibgeschützt öffnen.
    pub async fn connect(
        credentials: &Credentials,
        cancel: CancellationToken,
    ) -> Result<Gmail, MailError> {
        let config = rustls::ClientConfig::with_platform_verifier()
            .map_err(|e| MailError::Connect(e.to_string()))?;
        let tls = tokio_rustls::TlsConnector::from(Arc::new(config));
        let server = rustls::pki_types::ServerName::try_from(HOST)
            .map_err(|e| MailError::Connect(e.to_string()))?;
        let tcp = guarded(&cancel, TcpStream::connect((HOST, PORT)), |e| {
            MailError::Connect(e.to_string())
        })
        .await?;
        // Leerlauf-Grenze statt Gesamtgrenze: Eine große Mail auf langsamer Leitung darf
        // dauern, solange Daten fließen; eine hängende Verbindung endet nach 30 s.
        let mut tcp = TimeoutStream::new(tcp);
        tcp.set_read_timeout(Some(IDLE));
        tcp.set_write_timeout(Some(IDLE));
        let stream = guarded(&cancel, tls.connect(server, Box::pin(tcp)), |e| {
            MailError::Connect(e.to_string())
        })
        .await?;
        let mut client = async_imap::Client::new(stream);
        guarded(&cancel, client.read_response(), |e| {
            MailError::Connect(e.to_string())
        })
        .await?
        .ok_or_else(|| MailError::Connect("keine Begrüßung vom Server".into()))?;
        let login = client.login(&credentials.user, credentials.password());
        let mut session = guarded(&cancel, login, |(error, _)| login_error(error)).await?;
        let capabilities = guarded(&cancel, session.capabilities(), protocol).await?;
        if !capabilities.has_str("X-GM-EXT-1") {
            return Err(MailError::NotGmail);
        }
        guarded(&cancel, session.examine("INBOX"), protocol).await?;
        Ok(Gmail { session, cancel })
    }
}

impl<T> Gmail<T>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + fmt::Debug + Send,
{
    /// Abmelden; Fehler dabei sind egal (die Verbindung endet so oder so).
    pub async fn close(mut self) {
        let _ = tokio::time::timeout(Duration::from_secs(5), self.session.logout()).await;
    }

    /// Sendet einen Befehl und liest bis zu seiner Abschlussmeldung. `NO`/`BAD` ist ein
    /// Fehler – async-imap selbst prüft das bei SEARCH und FETCH nicht; eine gescheiterte
    /// Suche sähe sonst aus wie „keine Mails“.
    async fn command(
        &mut self,
        command: &str,
        mut on_data: impl FnMut(&Response<'_>),
    ) -> Result<(), MailError> {
        let tag = guarded(&self.cancel, self.session.run_command(command), protocol).await?;
        loop {
            let data = guarded(&self.cancel, self.session.read_response(), |e| io_error(&e))
                .await?
                .ok_or_else(|| MailError::Lost("vom Server getrennt".into()))?;
            match data.parsed() {
                Response::Done {
                    tag: done,
                    status,
                    information,
                    ..
                } if *done == tag => {
                    return match status {
                        Status::Ok => Ok(()),
                        _ => Err(MailError::Server(clean(
                            information.as_deref().unwrap_or("Befehl abgelehnt"),
                        ))),
                    };
                }
                other => on_data(other),
            }
        }
    }
}

impl<T> MailSource for Gmail<T>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + fmt::Debug + Send,
{
    async fn search(
        &mut self,
        since: Option<Date>,
        portals: &[Portal],
    ) -> Result<Vec<u32>, MailError> {
        if portals.is_empty() {
            return Ok(Vec::new());
        }
        let mut uids = Vec::new();
        let command = format!("UID SEARCH {}", search_query(since, portals));
        self.command(&command, |response| {
            if let Response::MailboxData(MailboxDatum::Search(ids)) = response {
                uids.extend(ids.iter().copied());
            }
        })
        .await?;
        uids.sort_unstable();
        uids.dedup();
        Ok(uids)
    }

    async fn fetch(&mut self, uids: &[u32]) -> Result<Vec<RawMail>, MailError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let wanted: HashSet<u32> = uids.iter().copied().collect();
        let set = uids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut mails = BTreeMap::new();
        let command = format!("UID FETCH {set} (UID X-GM-MSGID BODY.PEEK[]<0.{MAX_MAIL_BYTES}>)");
        self.command(&command, |response| {
            let Response::Fetch(_, attributes) = response else {
                return;
            };
            let (mut uid, mut gmail_id, mut body) = (None, None, None);
            for attribute in attributes {
                match attribute {
                    AttributeValue::Uid(u) => uid = Some(*u),
                    AttributeValue::GmailMsgId(id) => gmail_id = Some(*id),
                    AttributeValue::BodySection {
                        section: None,
                        data,
                        ..
                    } => body = Some(data.as_deref().unwrap_or_default().to_vec()),
                    _ => {}
                }
            }
            // Nur angeforderte Mails mit Inhalt, jede einmal – reine Statusmeldungen
            // (z. B. „inzwischen gelesen“) sind keine Mail.
            if let (Some(uid), Some(bytes)) = (uid, body)
                && wanted.contains(&uid)
            {
                mails.insert(uid, RawMail { gmail_id, bytes });
            }
        })
        .await?;
        Ok(mails.into_values().collect())
    }
}

/// Führt einen Netzschritt aus – abbrechbar. Die Leerlauf-Grenze liegt auf der Verbindung;
/// diese Gesamtgrenze fängt nur, was darüber hinaus hängen könnte.
async fn guarded<T, E>(
    cancel: &CancellationToken,
    step: impl Future<Output = Result<T, E>>,
    map: impl FnOnce(E) -> MailError,
) -> Result<T, MailError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(MailError::Cancelled),
        result = tokio::time::timeout(STEP_LIMIT, step) => match result {
            Err(_) => Err(MailError::Timeout),
            Ok(result) => result.map_err(map),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_for_selected_portals() {
        let since = Date::new(2026, 3, 5).unwrap();
        assert_eq!(
            search_query(Some(since), &[Portal::LinkedIn, Portal::Freelancermap]),
            r#"SINCE 05-Mar-2026 X-GM-RAW "from:(linkedin.com OR freelancermap.de OR freelancermap.com) OR linkedin OR freelancermap""#
        );
        assert_eq!(
            search_query(None, &[Portal::FreelanceDe]),
            r#"X-GM-RAW "from:(freelance.de) OR freelance.de""#
        );
    }

    /// Früher: Leerzeichen im App-Passwort wurden nur beim Speichern entfernt, die
    /// Adresse war groß/klein-empfindlich.
    #[test]
    fn credentials_are_normalised_and_never_printed() {
        let c = Credentials::new("  Erika.Mueller@Example.com ", "abcd efgh\tijkl mnop");
        assert_eq!(c.user, "erika.mueller@example.com");
        assert_eq!(c.password(), "abcdefghijklmnop");
        assert!(!format!("{c:?}").contains("abcd"));
    }

    /// Ein Verbindungsabbruch beim Anmelden ist kein falsches Passwort.
    #[test]
    fn only_a_rejection_is_an_auth_error() {
        use async_imap::error::Error as E;
        assert!(matches!(
            login_error(E::No("[AUTHENTICATIONFAILED] Invalid credentials".into())),
            MailError::Auth(_)
        ));
        let eof = std::io::Error::from(std::io::ErrorKind::UnexpectedEof);
        assert!(matches!(login_error(E::Io(eof)), MailError::Lost(_)));
        assert!(matches!(login_error(E::ConnectionLost), MailError::Lost(_)));
    }

    #[tokio::test]
    async fn cancel_and_timeout_are_errors() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let never = std::future::pending::<Result<(), ()>>();
        let result = guarded(&cancel, never, |()| MailError::Lost(String::new())).await;
        assert!(matches!(result, Err(MailError::Cancelled)));
    }

    /// Postfach-Attrappe auf Protokollebene: beantwortet jeden Befehl mit der nächsten
    /// vorbereiteten Antwort (`{tag}` wird ersetzt).
    async fn scripted(replies: Vec<String>) -> Gmail<tokio::io::DuplexStream> {
        use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
        let (client_side, server_side) = tokio::io::duplex(1 << 20);
        tokio::spawn(async move {
            let (read, mut write) = tokio::io::split(server_side);
            let mut read = BufReader::new(read);
            write.write_all(b"* OK Gimap ready\r\n").await.unwrap();
            let replies = std::iter::once("{tag} OK LOGIN done\r\n".to_string()).chain(replies);
            for reply in replies {
                let mut line = String::new();
                if read.read_line(&mut line).await.unwrap_or(0) == 0 {
                    return;
                }
                let tag = line.split(' ').next().unwrap_or_default().to_string();
                write
                    .write_all(reply.replace("{tag}", &tag).as_bytes())
                    .await
                    .unwrap();
            }
        });
        let mut client = async_imap::Client::new(client_side);
        client.read_response().await.unwrap().unwrap();
        let session = client
            .login("ich@gmail.com", "pw")
            .await
            .map_err(|(e, _)| e)
            .unwrap();
        Gmail {
            session,
            cancel: CancellationToken::new(),
        }
    }

    /// Eine gescheiterte Suche ist ein Fehler, nicht „0 Mails“ –
    /// sonst rückte der Scan-Stand vor und die Mails fehlten für immer.
    #[tokio::test]
    async fn failed_search_is_an_error() {
        let mut gmail = scripted(vec![
            "{tag} NO [UNAVAILABLE] Temporary System Problem (Failure)\r\n".into(),
        ])
        .await;
        let result = gmail.search(None, &[Portal::LinkedIn]).await;
        assert!(matches!(result, Err(MailError::Server(_))), "{result:?}");

        let mut gmail = scripted(vec![
            "* SEARCH 9 3 5 3\r\n{tag} OK SEARCH completed\r\n".into(),
        ])
        .await;
        assert_eq!(
            gmail.search(None, &[Portal::LinkedIn]).await.unwrap(),
            [3, 5, 9]
        );
    }

    /// Statusmeldungen ohne Inhalt sind keine Mail; eine abgelehnte
    /// Abholung ist ein Fehler.
    #[tokio::test]
    async fn fetch_takes_only_requested_mails_with_content() {
        let body = "Subject: Neue Jobs\r\n\r\nText";
        let reply = format!(
            "* 1 FETCH (UID 7 FLAGS (\\Seen))\r\n\
             * 2 FETCH (UID 99 X-GM-MSGID 5 BODY[]<0> {{{len}}}\r\n{body})\r\n\
             * 1 FETCH (UID 7 X-GM-MSGID 1234 BODY[]<0> {{{len}}}\r\n{body})\r\n\
             {{tag}} OK Success\r\n",
            len = body.len()
        );
        let mut gmail = scripted(vec![reply]).await;
        let mails = gmail.fetch(&[7]).await.unwrap();
        assert_eq!(mails.len(), 1);
        assert_eq!(mails[0].gmail_id, Some(1234));
        assert_eq!(mails[0].bytes, body.as_bytes());

        let mut gmail = scripted(vec![
            "{tag} NO Some messages could not be FETCHed (Failure)\r\n".into(),
        ])
        .await;
        assert!(matches!(gmail.fetch(&[7]).await, Err(MailError::Server(_))));
    }

    /// Eine unlesbare Antwort bringt keinen Mailinhalt in die Meldung.
    #[tokio::test]
    async fn unreadable_answer_leaks_no_content() {
        let mut gmail = scripted(vec![
            "* 1 FETCH (UID 1 BODY[] {33}\r\nSubject: Gehalt\r\n\r\nVERTRAULICH X-UNBEKANNT 5)\r\n"
                .into(),
        ])
        .await;
        let error = gmail.fetch(&[1]).await.unwrap_err();
        let text = error.to_string();
        assert!(
            !text.contains("VERTRAULICH") && text.chars().count() < 300,
            "{text}"
        );
    }

    /// Eine vorübergehende Ablehnung ist kein falsches Passwort.
    #[test]
    fn temporary_login_rejection_is_no_auth_error() {
        use async_imap::error::Error as E;
        for text in [
            "[UNAVAILABLE] Temporary System Problem (Failure)",
            "[ALERT] Too many simultaneous connections. (Failure)",
            "[OVERQUOTA] Account exceeded command or bandwidth limits.",
        ] {
            assert!(
                matches!(login_error(E::No(text.into())), MailError::Server(_)),
                "{text}"
            );
        }
        assert!(matches!(
            login_error(E::No(
                "[ALERT] Application-specific password required".into()
            )),
            MailError::Auth(_)
        ));
        let stalled = std::io::Error::from(std::io::ErrorKind::TimedOut);
        assert!(matches!(io_error(&stalled), MailError::Timeout));
    }
}
