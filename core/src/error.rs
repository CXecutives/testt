//! Fehler der Fachlogik – mit deutschen Nutzertexten an einer Stelle. Für die Oberfläche
//! macht die App-Schicht daraus `{ kind, message }`.

use std::path::{Path, PathBuf};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Die interne Datenbank meldet einen Fehler: {0}")]
    Db(#[from] rusqlite::Error),

    #[error(
        "Die Datei ist gerade geöffnet (z. B. in Excel) und kann nicht ersetzt werden:\n{0}\n\nBitte schließen und erneut versuchen – es geht nichts verloren."
    )]
    FileLocked(PathBuf),

    #[error("Datei „{path}“: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Die Excel-Datei konnte nicht erzeugt werden: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),

    #[error("Gespeicherte Daten sind beschädigt: {0}")]
    Corrupt(String),

    /// Die Datenbank stammt von einer neueren Version – nicht beschädigt, nur nicht lesbar.
    #[error("Die Datenbank stammt von einer neueren Version dieser App (Schema {0}).")]
    NewerSchema(i64),

    /// Eingabe des Nutzers ungültig (Text ist die fertige Meldung).
    #[error("{0}")]
    Invalid(String),
}

impl Error {
    /// Kurzer, stabiler Name für die Oberfläche (reagiert darauf, nicht auf den Text).
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Db(_) => "db",
            Error::FileLocked(_) => "fileLocked",
            Error::Io { .. } => "io",
            Error::Xlsx(_) => "xlsx",
            Error::Corrupt(_) => "corrupt",
            Error::NewerSchema(_) => "newerSchema",
            Error::Invalid(_) => "invalid",
        }
    }

    /// Ein-/Ausgabefehler mit Pfad; eine Freigabe- oder Sperrverletzung heißt „gesperrt“.
    /// Die Fehlernummern 32/33 sind Windows-eigen (anderswo bedeuten sie etwas anderes).
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();
        if cfg!(windows) && matches!(source.raw_os_error(), Some(32 | 33)) {
            Error::FileLocked(path)
        } else {
            Error::Io { path, source }
        }
    }

    /// Fehler beim Ersetzen einer bestehenden Datei. Ist sie in Excel geöffnet, meldet
    /// Windows nur „Zugriff verweigert“ – „gesperrt“ heißt es deshalb nur, wenn die Datei
    /// existiert und nicht schreibgeschützt ist. Fehlendes Schreibrecht bleibt ein
    /// gewöhnlicher Fehler; Excel zu schließen hülfe dort nicht.
    /// „In Excel geöffnet“ ist eine Windows-Eigenheit; anderswo verhindert eine offene
    /// Datei das Ersetzen nicht, und „Zugriff verweigert“ hat dann andere Ursachen.
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
