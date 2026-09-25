//! Gmail over IMAP, read-only (`EXAMINE`, `BODY.PEEK`), so Gmail marks nothing as read.
//!
//! The scan reads "All Mail" (the special-use `\All` mailbox, found by its attribute
//! because its name is localized: "[Gmail]/Alle Nachrichten"): archiving a read alert and
//! a filter that skips the inbox for the portals' alerts are common, and the inbox alone
//! would never find those alerts. Trash and spam are not in All Mail; drafts and the
//! user's own sent mails are left out by the search (`-in:drafts (in:inbox OR -in:sent)`),
//! so an alert the user forwarded to someone else never counts. Without an `\All` mailbox
//! the inbox is read.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_imap::imap_proto::{AttributeValue, MailboxDatum, NameAttribute, Response, Status};

use jiff::civil::Date;
use rustls_platform_verifier::ConfigVerifierExt as _;
use tokio::net::TcpStream;
use tokio_io_timeout::TimeoutStream;
use tokio_rustls::client::TlsStream;
use tokio_util::sync::CancellationToken;

use super::{RawHead, RawMail};
use crate::error::ErrorKind;
use crate::portal::Portal;
use crate::text::{one_line, truncate_chars};

const HOST: &str = "imap.gmail.com";
const PORT: u16 = 993;
/// If nothing at all arrives from the server for this long, the connection counts as hung.
const IDLE: Duration = Duration::from_secs(30);
/// Upper bound for a single step, even while data only trickles in.
const STEP_LIMIT: Duration = Duration::from_secs(10 * 60);
/// Never fetch more than this of one mail (alerts are small, large attachments are not needed).
const MAX_MAIL_BYTES: u32 = 4 * 1024 * 1024;
/// Mails per fetch.
pub const BATCH: usize = 25;

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("no Gmail credentials stored")]
    NoCredentials,

    #[error("cannot connect to imap.gmail.com: {0}")]
    Connect(String),

    #[error("Gmail rejected the login: {0}")]
    Auth(String),

    #[error("Gmail did not answer within 30 seconds")]
    Timeout,

    #[error("connection to Gmail lost: {0}")]
    Lost(String),

    #[error("server is not Gmail (no X-GM-EXT-1 capability)")]
    NotGmail,

    #[error("Gmail reported an error: {0}")]
    Server(String),

    #[error("cancelled")]
    Cancelled,
}

impl MailError {
    /// Stable error code for the interface.
    pub fn kind(&self) -> ErrorKind {
        match self {
            MailError::NoCredentials => ErrorKind::MailMissing,
            MailError::Connect(_) => ErrorKind::MailConnect,
            MailError::Auth(_) => ErrorKind::MailAuth,
            MailError::Timeout => ErrorKind::MailTimeout,
            MailError::Lost(_) => ErrorKind::MailLost,
            MailError::NotGmail => ErrorKind::MailNotGmail,
            MailError::Server(_) => ErrorKind::MailServer,
            MailError::Cancelled => ErrorKind::MailCancelled,
        }
    }
}

/// Protocol errors after login. Only a real rejection at login is a credentials error -
/// a dropped connection never is (it used to count as a "wrong password").
fn protocol(error: async_imap::error::Error) -> MailError {
    use async_imap::error::Error as E;
    match error {
        E::Io(e) => io_error(&e),
        E::ConnectionLost => MailError::Lost("disconnected by the server".into()),
        E::No(text) | E::Bad(text) => MailError::Server(clean(&text)),
        E::Parse(_) => MailError::Server(UNREADABLE.into()),
        other => MailError::Server(clean(&other.to_string())),
    }
}

const UNREADABLE: &str = "unreadable reply from Gmail";

/// I/O errors. Parser errors from async-imap carry the whole unread buffer (i.e. mail
/// content) in their text - that never ends up in a message or the log.
fn io_error(error: &std::io::Error) -> MailError {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::TimedOut => MailError::Timeout,
        ErrorKind::Other | ErrorKind::InvalidData => MailError::Server(UNREADABLE.into()),
        kind => MailError::Lost(clean(&kind.to_string())),
    }
}

/// Server texts for messages: one line, at most 200 characters.
fn clean(text: &str) -> String {
    truncate_chars(&one_line(text), 200)
}

/// Errors at login: only a rejection of the credentials is a credentials problem.
/// Transient rejections (Gmail overloaded, too many connections, bandwidth limit)
/// are not - otherwise the user would change a correct password.
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
                MailError::Server(format!("{} (temporary, try again later)", clean(&text)))
            } else {
                MailError::Auth(clean(&text))
            }
        }
        other => protocol(other),
    }
}

/// Gmail credentials. Address and app password are normalised in **one** place: the
/// address trimmed and lowercased, the password without whitespace (Google shows it in
/// groups of four).
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

/// The password never appears in debug output or logs.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}

/// Where the mails come from - Gmail, or a fake in tests.
pub trait MailSource {
    /// UIDs of the matches since `since` (`None` = all), ascending.
    fn search(
        &mut self,
        since: Option<Date>,
        portals: &[Portal],
    ) -> impl Future<Output = Result<Vec<u32>, MailError>> + Send;

    /// The heads of the mails for `uids` (sender, subject, date, message id) - a few hundred
    /// bytes each; the bodies stay on the server. Mails deleted in the meantime are missing.
    fn heads(
        &mut self,
        uids: &[u32],
    ) -> impl Future<Output = Result<Vec<RawHead>, MailError>> + Send;

    /// The whole mails for `uids` - only for mails whose head made them a candidate. Mails
    /// deleted in the meantime are simply missing; a mail without content comes back with
    /// empty bytes (counted as defective, never silently dropped).
    fn fetch(
        &mut self,
        uids: &[u32],
    ) -> impl Future<Output = Result<Vec<RawMail>, MailError>> + Send;

    /// Ends the session after the scan (IMAP `LOGOUT`); errors do not matter. Sources
    /// without a session do nothing.
    fn logout(self) -> impl Future<Output = ()> + Send
    where
        Self: Sized,
    {
        async {}
    }
}

/// The head fields a scan needs to decide whether a mail may be an alert.
const HEAD_FIELDS: &str = "FROM SUBJECT DATE MESSAGE-ID";

/// Search expression: the portals' sender domains **or** their keywords (forwarded
/// alerts come from the user themselves). `X-GM-RAW` is Gmail's own search - server-side,
/// fast and without downloading the mailbox. In All Mail (`all_mail`) drafts and the
/// user's own sent mails stay out - a mail the user sent to themselves is in the inbox
/// and counts.
fn search_query(since: Option<Date>, portals: &[Portal], all_mail: bool) -> String {
    let domains: Vec<&str> = portals
        .iter()
        .flat_map(|p| p.sender_domains().iter().copied())
        .collect();
    let terms: Vec<&str> = portals
        .iter()
        .flat_map(|p| p.search_terms().iter().copied())
        .collect();
    let raw = if all_mail {
        format!(
            r#"X-GM-RAW "(from:({}) OR {}) -in:drafts (in:inbox OR -in:sent)""#,
            domains.join(" OR "),
            terms.join(" OR ")
        )
    } else {
        format!(
            r#"X-GM-RAW "from:({}) OR {}""#,
            domains.join(" OR "),
            terms.join(" OR ")
        )
    };
    match since {
        // IMAP month names are English; jiff formats them regardless of the system language.
        Some(date) => format!("SINCE {} {raw}", date.strftime("%d-%b-%Y")),
        None => raw,
    }
}

/// Connection to Gmail: TCP with an idle limit, TLS on top.
pub(crate) type Stream = TlsStream<Pin<Box<TimeoutStream<TcpStream>>>>;

pub struct Gmail<T = Stream>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + fmt::Debug + Send,
{
    session: async_imap::Session<T>,
    cancel: CancellationToken,
    /// All Mail is open (else the inbox).
    all_mail: bool,
}

impl Gmail {
    /// Connect, log in, open All Mail (else the inbox) read-only.
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
        // Idle limit instead of a total limit: a large mail on a slow line may take its
        // time as long as data flows; a hung connection ends after 30 s.
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
        .ok_or_else(|| MailError::Connect("no greeting from the server".into()))?;
        let login = client.login(&credentials.user, credentials.password());
        let mut session = guarded(&cancel, login, |(error, _)| login_error(error)).await?;
        let capabilities = guarded(&cancel, session.capabilities(), protocol).await?;
        if !capabilities.has_str("X-GM-EXT-1") {
            return Err(MailError::NotGmail);
        }
        let mut gmail = Gmail {
            session,
            cancel,
            all_mail: false,
        };
        gmail.open_mailbox().await?;
        Ok(gmail)
    }
}

impl<T> Gmail<T>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + fmt::Debug + Send,
{
    /// Opens All Mail read-only - the mailbox with the special-use attribute `\All`, whose
    /// name Gmail localizes - or the inbox when there is none.
    async fn open_mailbox(&mut self) -> Result<(), MailError> {
        let mut all = None;
        self.command(r#"LIST "" "*""#, |response| {
            if let Response::MailboxData(MailboxDatum::List {
                name_attributes,
                name,
                ..
            }) = response
                && all.is_none()
                && name_attributes.contains(&NameAttribute::All)
            {
                all = Some(name.to_string());
            }
        })
        .await?;
        let cancel = self.cancel.clone();
        self.all_mail = all.is_some();
        if let Some(name) = all {
            guarded(&cancel, self.session.examine(&name), protocol).await?;
        } else {
            guarded(&cancel, self.session.examine("INBOX"), protocol).await?;
        }
        Ok(())
    }

    /// Log out; errors here do not matter (the connection ends either way).
    pub async fn close(mut self) {
        let _ = tokio::time::timeout(Duration::from_secs(5), self.session.logout()).await;
    }

    /// Sends a command and reads up to its completion response. `NO`/`BAD` is an error -
    /// async-imap itself does not check this for SEARCH and FETCH; a failed search would
    /// otherwise look like "no mails".
    async fn command(
        &mut self,
        command: &str,
        mut on_data: impl FnMut(&Response<'_>),
    ) -> Result<(), MailError> {
        let tag = guarded(&self.cancel, self.session.run_command(command), protocol).await?;
        loop {
            let data = guarded(&self.cancel, self.session.read_response(), |e| io_error(&e))
                .await?
                .ok_or_else(|| MailError::Lost("disconnected by the server".into()))?;
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
                            information.as_deref().unwrap_or("command rejected"),
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
    async fn logout(self) {
        self.close().await;
    }

    async fn search(
        &mut self,
        since: Option<Date>,
        portals: &[Portal],
    ) -> Result<Vec<u32>, MailError> {
        if portals.is_empty() {
            return Ok(Vec::new());
        }
        let mut uids = Vec::new();
        let command = format!("UID SEARCH {}", search_query(since, portals, self.all_mail));
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

    async fn heads(&mut self, uids: &[u32]) -> Result<Vec<RawHead>, MailError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let wanted: HashSet<u32> = uids.iter().copied().collect();
        let mut heads = BTreeMap::new();
        let command = format!(
            "UID FETCH {} (UID BODY.PEEK[HEADER.FIELDS ({HEAD_FIELDS})])",
            uid_set(uids)
        );
        self.command(&command, |response| {
            let Response::Fetch(_, attributes) = response else {
                return;
            };
            let (mut uid, mut bytes) = (None, None);
            for attribute in attributes {
                match attribute {
                    AttributeValue::Uid(u) => uid = Some(*u),
                    AttributeValue::BodySection { data, .. } => {
                        bytes = Some(data.as_deref().unwrap_or_default().to_vec());
                    }
                    _ => {}
                }
            }
            if let (Some(uid), Some(bytes)) = (uid, bytes)
                && wanted.contains(&uid)
            {
                heads.insert(uid, RawHead { uid, bytes });
            }
        })
        .await?;
        Ok(heads.into_values().collect())
    }

    async fn fetch(&mut self, uids: &[u32]) -> Result<Vec<RawMail>, MailError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let wanted: HashSet<u32> = uids.iter().copied().collect();
        let set = uid_set(uids);
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
            // Only requested mails with content, each once - pure status updates
            // (e.g. "read in the meantime") are not a mail.
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

/// An IMAP UID set: `7,9,12`.
fn uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Runs one network step, cancellable. The idle limit sits on the connection; this
/// overall limit only catches whatever could hang beyond that.
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
            search_query(
                Some(since),
                &[Portal::LinkedIn, Portal::Freelancermap],
                false
            ),
            r#"SINCE 05-Mar-2026 X-GM-RAW "from:(linkedin.com OR freelancermap.de OR freelancermap.com) OR linkedin OR freelancermap""#
        );
        assert_eq!(
            search_query(None, &[Portal::FreelanceDe], false),
            r#"X-GM-RAW "from:(freelance.de) OR freelance.de""#
        );
        // All Mail: without drafts and the user's own sent mails.
        assert_eq!(
            search_query(None, &[Portal::FreelanceDe], true),
            r#"X-GM-RAW "(from:(freelance.de) OR freelance.de) -in:drafts (in:inbox OR -in:sent)""#
        );
    }

    /// All Mail is found by its special-use attribute, whatever its localized name; the
    /// search then leaves drafts and sent mails out. Without it the inbox is read.
    #[tokio::test]
    async fn all_mail_by_its_attribute_else_the_inbox() {
        let list = "* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n\
                    * LIST (\\HasChildren \\Noselect) \"/\" \"[Gmail]\"\r\n\
                    * LIST (\\All \\HasNoChildren) \"/\" \"[Gmail]/Alle Nachrichten\"\r\n\
                    * LIST (\\HasNoChildren \\Sent) \"/\" \"[Gmail]/Gesendet\"\r\n\
                    {tag} OK Success\r\n";
        let examine =
            "* FLAGS (\\Seen)\r\n* 3 EXISTS\r\n{tag} OK [READ-ONLY] EXAMINE completed\r\n";
        let (mut gmail, sent) = scripted_logging(vec![
            list.into(),
            examine.into(),
            "* SEARCH 7\r\n{tag} OK SEARCH completed\r\n".into(),
        ])
        .await;
        gmail.open_mailbox().await.unwrap();
        assert!(gmail.all_mail);
        gmail.search(None, &[Portal::LinkedIn]).await.unwrap();
        let commands = sent.lock().unwrap().join("\n");
        assert!(
            commands.contains(r#"EXAMINE "[Gmail]/Alle Nachrichten""#),
            "{commands}"
        );
        assert!(commands.contains("(in:inbox OR -in:sent)"), "{commands}");

        let list = "* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n{tag} OK Success\r\n";
        let (mut gmail, sent) = scripted_logging(vec![list.into(), examine.into()]).await;
        gmail.open_mailbox().await.unwrap();
        assert!(!gmail.all_mail);
        let commands = sent.lock().unwrap().join("\n");
        assert!(commands.contains(r#"EXAMINE "INBOX""#), "{commands}");
    }

    /// Formerly: whitespace in the app password was only removed when saving, and the
    /// address was case-sensitive.
    #[test]
    fn credentials_are_normalised_and_never_printed() {
        let c = Credentials::new("  Erika.Mueller@Example.com ", "abcd efgh\tijkl mnop");
        assert_eq!(c.user, "erika.mueller@example.com");
        assert_eq!(c.password(), "abcdefghijklmnop");
        assert!(!format!("{c:?}").contains("abcd"));
    }

    /// A dropped connection at login is not a wrong password.
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

    /// Mailbox fake at protocol level: answers every command with the next prepared
    /// reply (`{tag}` is replaced).
    async fn scripted(replies: Vec<String>) -> Gmail<tokio::io::DuplexStream> {
        scripted_logging(replies).await.0
    }

    /// Like [`scripted`], but also returns every command the client sent.
    async fn scripted_logging(
        replies: Vec<String>,
    ) -> (
        Gmail<tokio::io::DuplexStream>,
        Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
        let (client_side, server_side) = tokio::io::duplex(1 << 20);
        let sent: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
        let log = sent.clone();
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
                if let Ok(mut log) = log.lock() {
                    log.push(line.trim_end().to_string());
                }
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
        (
            Gmail {
                session,
                cancel: CancellationToken::new(),
                all_mail: false,
            },
            sent,
        )
    }

    /// Safety invariant: Gmail is only ever read. The mailbox is opened with `EXAMINE`
    /// (never `SELECT`), mails are fetched with `BODY.PEEK` - so Gmail marks nothing as
    /// read, and no command ever changes anything.
    #[tokio::test]
    async fn the_mailbox_is_only_ever_read() {
        let body = "Subject: Neue Jobs\r\n\r\nText";
        let reply = format!(
            "* 1 FETCH (UID 7 X-GM-MSGID 1234 BODY[]<0> {{{len}}}\r\n{body})\r\n{{tag}} OK Success\r\n",
            len = body.len()
        );
        let (mut gmail, sent) = scripted_logging(vec![
            "* SEARCH 7\r\n{tag} OK SEARCH completed\r\n".into(),
            reply,
        ])
        .await;
        gmail.search(None, &[Portal::LinkedIn]).await.unwrap();
        gmail.fetch(&[7]).await.unwrap();
        let commands = sent.lock().unwrap().join("\n").to_uppercase();
        assert!(commands.contains("BODY.PEEK["), "{commands}");
        for forbidden in [
            "SELECT", "STORE", "EXPUNGE", "DELETE", "APPEND", "COPY", "MOVE", "\\SEEN",
        ] {
            assert!(!commands.contains(forbidden), "{forbidden}: {commands}");
        }
        // The mailbox itself is opened on connect - that code says `examine`, never `select`.
        // The search text is assembled here, otherwise the test would find itself.
        let source = include_str!("imap.rs");
        assert!(source.contains(&format!("session{}", r#".examine("INBOX")"#)));
        assert!(source.contains(&format!("session{}", ".examine(&name)")));
        assert!(!source.contains(&format!("session{}", ".select(")));
    }

    /// Two phases over IMAP: the heads of all matches, then the whole mail only for the
    /// candidates - a newsletter the keyword search found is never loaded whole.
    #[tokio::test]
    async fn only_candidate_bodies_are_fetched() {
        use crate::mail::scan::{ScanSummary, Scope, scan};
        let alert = "From: LinkedIn <jobalerts-noreply@linkedin.com>\r\nSubject: Neue Jobs\r\n\r\n";
        let news = "From: News <news@example.org>\r\nSubject: Unlocked: your notetaker\r\n\r\n";
        let field = format!("BODY[HEADER.FIELDS ({HEAD_FIELDS})]");
        let heads = format!(
            "* 1 FETCH (UID 7 {field} {{{}}}\r\n{alert})\r\n\
             * 2 FETCH (UID 8 {field} {{{}}}\r\n{news})\r\n{{tag}} OK Success\r\n",
            alert.len(),
            news.len()
        );
        let body =
            format!("{alert}<a href=\"https://www.linkedin.com/jobs/view/4123456789/\">Rolle</a>");
        let bodies = format!(
            "* 1 FETCH (UID 7 X-GM-MSGID 1234 BODY[]<0> {{{}}}\r\n{body})\r\n{{tag}} OK Success\r\n",
            body.len()
        );
        let (mut gmail, sent) = scripted_logging(vec![
            "* SEARCH 7 8\r\n{tag} OK SEARCH completed\r\n".into(),
            heads,
            bodies,
        ])
        .await;
        let store = crate::store::Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let mut summary = ScanSummary::default();
        scan(
            &mut gmail,
            &store,
            run,
            Scope::All,
            &[Portal::LinkedIn],
            "2026-09-19T08:00:00Z".parse().unwrap(),
            &CancellationToken::new(),
            &mut summary,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            (summary.mails_checked, summary.alert_mails, summary.new),
            (2, 1, 1)
        );
        let commands = sent.lock().unwrap().clone();
        let fetches: Vec<&String> = commands
            .iter()
            .filter(|c| c.contains("UID FETCH"))
            .collect();
        assert_eq!(fetches.len(), 2, "{commands:?}");
        assert!(fetches[0].contains("UID FETCH 7,8 (UID BODY.PEEK[HEADER.FIELDS"));
        assert!(fetches[1].contains("UID FETCH 7 ("), "only the candidate");
        assert!(fetches[1].contains("BODY.PEEK[]"));
    }

    /// A failed search is an error, not "0 mails" - otherwise the scan state would
    /// advance and the mails would be missing forever.
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

    /// Status updates without content are not a mail; a rejected fetch is an error.
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

    /// An unreadable reply brings no mail content into the message.
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

    /// A temporary rejection is not a wrong password.
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
