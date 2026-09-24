//! Errors of the core. `Display` texts are English and meant for logs only: the interface
//! receives an [`ErrorInfo`] (`{kind, params}`) and words it from its own catalog.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::mail::imap::MailError;
use crate::portal::Portal;
use crate::secrets::SecretError;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// The file is open elsewhere (typically in Excel on Windows) and cannot be replaced.
    #[error("file is locked by another program: {}", .0.display())]
    FileLocked(PathBuf),

    #[error("file {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot create the Excel file: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),

    #[error("stored data is corrupt: {0}")]
    Corrupt(String),

    /// The database comes from a newer version - not corrupt, just not readable.
    #[error("the database comes from a newer version of this app (schema {0})")]
    NewerSchema(i64),

    /// Input of the user that cannot be used.
    #[error("invalid input: {0}")]
    Invalid(InvalidInput),

    /// The fetch path of a portal could not be created (HTTP client, session window).
    #[error("fetching from {portal} is not possible: {detail}")]
    FetchUnavailable { portal: Portal, detail: String },
}

/// Why an input was refused - a code with data, never a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(
    tag = "reason",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum InvalidInput {
    #[error("no portal is enabled")]
    NoPortal,
    #[error("the profile is not UTF-8 text")]
    ProfileNotUtf8,
    #[error("the profile is not valid JSON (line {line}, column {column})")]
    ProfileNotJson { line: usize, column: usize },
    /// `found`: `array`, `string`, `number`, `boolean` or `null`.
    #[error("the profile must be a JSON object, found {found}")]
    ProfileNotObject { found: String },
    /// A value of the profile form is out of range; `field` names it (`minDayRate`, ...).
    #[error("the profile value {field} is out of range")]
    ProfileValue { field: String },
    /// A pasted answer holds no profile JSON with anything the form can show.
    #[error("the answer holds no profile")]
    ProfileAnswer,
    /// Not a complete e-mail address.
    #[error("not a complete mail address")]
    MailAddress,
    /// An app password is exactly 16 letters (not the normal account password).
    #[error("not an app password")]
    AppPassword,
    #[error("{portal} has no sign-in")]
    NoSignIn { portal: Portal },
}

/// Stable error codes for the interface (it reacts to these, never to a text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ErrorKind {
    Db,
    FileLocked,
    Io,
    Xlsx,
    Corrupt,
    NewerSchema,
    Invalid,
    /// A run or a sign-in window is active.
    Busy,
    NotFound,
    /// Not available in the dry run.
    DryRun,
    MailMissing,
    MailConnect,
    MailAuth,
    MailTimeout,
    MailLost,
    MailNotGmail,
    MailServer,
    MailCancelled,
    SecretStore,
    SecretCorrupt,
    /// The fetch path of a portal could not be created.
    PortalUnavailable,
    /// The portal is paused (`params.until`, `params.reason`).
    PortalPaused,
    /// The hourly or daily cap of the portal is reached (`params.until`).
    PortalQuota,
    /// A crash inside the app; details are in the log.
    Internal,
}

/// An error as the interface sees it: a code plus data (paths, numbers, portal keys).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ErrorInfo {
    pub kind: ErrorKind,
    #[cfg_attr(test, ts(type = "Record<string, string | number | boolean | null>"))]
    pub params: Map<String, Value>,
}

impl ErrorInfo {
    pub fn new(kind: ErrorKind) -> ErrorInfo {
        ErrorInfo {
            kind,
            params: Map::new(),
        }
    }

    /// Adds a parameter (only strings, numbers, booleans or null belong here).
    #[must_use]
    pub fn with(mut self, name: &str, value: impl Into<Value>) -> ErrorInfo {
        self.params.insert(name.to_string(), value.into());
        self
    }
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Db(_) => ErrorKind::Db,
            Error::FileLocked(_) => ErrorKind::FileLocked,
            Error::Io { .. } => ErrorKind::Io,
            Error::Xlsx(_) => ErrorKind::Xlsx,
            Error::Corrupt(_) => ErrorKind::Corrupt,
            Error::NewerSchema(_) => ErrorKind::NewerSchema,
            Error::Invalid(_) => ErrorKind::Invalid,
            Error::FetchUnavailable { .. } => ErrorKind::PortalUnavailable,
        }
    }

    /// Input/output error with path; a sharing or lock violation counts as "locked".
    /// The error numbers 32/33 are Windows-specific (they mean something else elsewhere).
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();
        if cfg!(windows) && matches!(source.raw_os_error(), Some(32 | 33)) {
            Error::FileLocked(path)
        } else {
            Error::Io { path, source }
        }
    }

    /// Error while replacing an existing file. If it is open in Excel, Windows only says
    /// "access denied" - so it counts as locked only if the file exists and is not
    /// read-only. Missing write permission stays an ordinary error; closing Excel would not
    /// help there. "Open in Excel" is a Windows peculiarity; elsewhere an open file does not
    /// prevent replacing it, and "access denied" has other causes.
    pub(crate) fn replace(path: &Path, source: std::io::Error) -> Self {
        let open_elsewhere = cfg!(windows)
            && source.kind() == std::io::ErrorKind::PermissionDenied
            && std::fs::metadata(path).is_ok_and(|m| m.is_file() && !m.permissions().readonly());
        if open_elsewhere {
            Error::FileLocked(path.to_path_buf())
        } else {
            Error::io(path, source)
        }
    }
}

impl From<&Error> for ErrorInfo {
    fn from(error: &Error) -> ErrorInfo {
        let info = ErrorInfo::new(error.kind());
        match error {
            Error::FileLocked(path) | Error::Io { path, .. } => {
                info.with("path", path.display().to_string())
            }
            Error::NewerSchema(version) => info.with("schema", *version),
            Error::Invalid(input) => ErrorInfo::from(input),
            Error::FetchUnavailable { portal, .. } => info.with("portal", portal.key()),
            Error::Db(_) | Error::Xlsx(_) | Error::Corrupt(_) => info,
        }
    }
}

impl From<&InvalidInput> for ErrorInfo {
    fn from(input: &InvalidInput) -> ErrorInfo {
        let params = match serde_json::to_value(input) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        ErrorInfo {
            kind: ErrorKind::Invalid,
            params,
        }
    }
}

impl From<&MailError> for ErrorInfo {
    fn from(error: &MailError) -> ErrorInfo {
        ErrorInfo::new(error.kind())
    }
}

impl From<&SecretError> for ErrorInfo {
    fn from(error: &SecretError) -> ErrorInfo {
        match error {
            SecretError::Invalid(input) => ErrorInfo::from(input),
            other => ErrorInfo::new(other.kind()),
        }
    }
}

/// Handing an error to the interface: its English text goes to the log, the code to the
/// page.
impl From<Error> for ErrorInfo {
    fn from(error: Error) -> ErrorInfo {
        log::warn!("{error}");
        ErrorInfo::from(&error)
    }
}

impl From<MailError> for ErrorInfo {
    fn from(error: MailError) -> ErrorInfo {
        log::warn!("{error}");
        ErrorInfo::from(&error)
    }
}

impl From<SecretError> for ErrorInfo {
    fn from(error: SecretError) -> ErrorInfo {
        log::warn!("{error}");
        ErrorInfo::from(&error)
    }
}

impl From<InvalidInput> for Error {
    fn from(input: InvalidInput) -> Error {
        Error::Invalid(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infos_are_codes_with_data() {
        let locked = ErrorInfo::from(&Error::FileLocked(PathBuf::from("a.xlsx")));
        assert_eq!(
            serde_json::to_value(&locked).unwrap(),
            serde_json::json!({"kind": "fileLocked", "params": {"path": "a.xlsx"}})
        );
        let invalid = ErrorInfo::from(&Error::Invalid(InvalidInput::ProfileNotJson {
            line: 3,
            column: 7,
        }));
        assert_eq!(
            serde_json::to_value(&invalid).unwrap(),
            serde_json::json!({"kind": "invalid",
                "params": {"reason": "profileNotJson", "line": 3, "column": 7}})
        );
        let portal = ErrorInfo::from(&Error::FetchUnavailable {
            portal: Portal::LinkedIn,
            detail: "no client".into(),
        });
        assert_eq!(portal.kind, ErrorKind::PortalUnavailable);
        assert_eq!(portal.params["portal"], "linkedin");
    }
}
