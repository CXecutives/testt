//! Gmail credentials in the system keychain (the Credential Manager on Windows, the
//! keychain elsewhere). The app stores **only** this one credential - LinkedIn and
//! freelancermap need no account, and the freelance.de login lives solely in the
//! session window's profile.

use crate::error::{ErrorKind, InvalidInput};
use crate::mail::imap::Credentials;

/// Service name in the keychain.
pub(crate) const SERVICE: &str = "de.cxecutives.job-alert-monitor";
const GMAIL: &str = "gmail";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("cannot access the system keychain: {0}")]
    Store(String),
    /// Never overwrite silently: the user decides whether to enter it again.
    #[error("stored Gmail credentials are unreadable")]
    Corrupt,
    #[error("invalid credentials: {0}")]
    Invalid(InvalidInput),
}

impl SecretError {
    /// Stable error code for the interface.
    pub fn kind(&self) -> ErrorKind {
        match self {
            SecretError::Store(_) => ErrorKind::SecretStore,
            SecretError::Corrupt => ErrorKind::SecretCorrupt,
            SecretError::Invalid(_) => ErrorKind::Invalid,
        }
    }
}

/// One entry in the keychain; tests use their own service name.
pub struct Vault {
    service: String,
}

impl Vault {
    pub fn app() -> Vault {
        Vault {
            service: SERVICE.into(),
        }
    }

    /// Separate entry for tests only - a test must never touch the app's credentials.
    #[cfg(test)]
    pub(crate) fn for_tests(name: &str) -> Vault {
        Vault {
            service: format!("{SERVICE}.test.{name}.{}", std::process::id()),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(&self.service, GMAIL).map_err(|e| SecretError::Store(e.to_string()))
    }

    /// The stored credentials; `None` if none are stored.
    pub fn load_gmail(&self) -> Result<Option<Credentials>, SecretError> {
        let secret = match self.entry()?.get_password() {
            Ok(secret) => secret,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(SecretError::Store(e.to_string())),
        };
        let value: serde_json::Value =
            serde_json::from_str(&secret).map_err(|_| SecretError::Corrupt)?;
        match (value["user"].as_str(), value["password"].as_str()) {
            (Some(user), Some(password)) => Ok(Some(Credentials::new(user, password))),
            _ => Err(SecretError::Corrupt),
        }
    }

    /// Validates and stores. An app password is 16 letters (Google shows them in groups
    /// of four) - so an account password entered by mistake is caught right away.
    pub fn save_gmail(&self, credentials: &Credentials) -> Result<(), SecretError> {
        let user = &credentials.user;
        let valid_user = user
            .split_once('@')
            .is_some_and(|(name, domain)| !name.is_empty() && domain.contains('.'));
        if !valid_user {
            return Err(SecretError::Invalid(InvalidInput::MailAddress));
        }
        let password = credentials.password();
        if password.chars().count() != 16 || !password.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(SecretError::Invalid(InvalidInput::AppPassword));
        }
        let secret = serde_json::json!({ "user": user, "password": password }).to_string();
        self.entry()?
            .set_password(&secret)
            .map_err(|e| SecretError::Store(e.to_string()))
    }

    /// Removes the credentials; `false` if there were none.
    pub fn delete_gmail(&self) -> Result<bool, SecretError> {
        match self.entry()?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(SecretError::Store(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real keychain with a dedicated test service and dummy values.
    #[cfg(windows)]
    #[test]
    fn round_trip_in_the_vault() {
        let vault = Vault {
            service: format!("{SERVICE}.selftest.{}", std::process::id()),
        };
        assert!(vault.load_gmail().unwrap().is_none());
        let c = Credentials::new(" Test@Example.org ", "abcd efgh ijkl mnop");
        vault.save_gmail(&c).unwrap();
        let back = vault.load_gmail().unwrap().unwrap();
        assert_eq!(
            (back.user.as_str(), back.password()),
            ("test@example.org", "abcdefghijklmnop")
        );
        assert!(vault.delete_gmail().unwrap());
        assert!(!vault.delete_gmail().unwrap());
        assert!(vault.load_gmail().unwrap().is_none());
    }

    #[test]
    fn obviously_wrong_input_is_refused_before_storing() {
        let vault = Vault {
            service: "never-used".into(),
        };
        for (user, password) in [
            ("no-at-sign", "abcdefghijklmnop"),
            ("ich@gmail.com", "MyAccountPassword1!"),
            ("ich@gmail.com", "abcd efgh ijkl"),
        ] {
            assert!(matches!(
                vault.save_gmail(&Credentials::new(user, password)),
                Err(SecretError::Invalid(_))
            ));
        }
    }
}
