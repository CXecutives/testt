//! Gmail-Zugang im Windows-Tresor (Anmeldeinformationsverwaltung). Die App speichert
//! **nur** diesen einen Zugang – LinkedIn und freelancermap brauchen kein Konto, und die
//! freelance.de-Anmeldung lebt allein im Profil des Sitzungsfensters.

use crate::mail::imap::Credentials;

/// Dienstname im Tresor.
pub(crate) const SERVICE: &str = "de.cxecutives.job-alert-monitor";
const GMAIL: &str = "gmail";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("Der Schlüsselspeicher dieses Rechners ist nicht erreichbar: {0}")]
    Store(String),
    /// Nie still überschreiben: Der Nutzer entscheidet, ob neu eingegeben wird.
    #[error("Der gespeicherte Gmail-Zugang ist unlesbar – bitte neu eingeben.")]
    Corrupt,
    #[error("{0}")]
    Invalid(&'static str),
}

/// Ein Eintrag im Tresor; Tests nutzen einen eigenen Dienstnamen.
pub struct Vault {
    service: String,
}

impl Vault {
    pub fn app() -> Vault {
        Vault {
            service: SERVICE.into(),
        }
    }

    /// Eigener Eintrag nur für Tests – ein Test darf den Zugang der App nie berühren.
    #[cfg(test)]
    pub(crate) fn for_tests(name: &str) -> Vault {
        Vault {
            service: format!("{SERVICE}.test.{name}.{}", std::process::id()),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(&self.service, GMAIL).map_err(|e| SecretError::Store(e.to_string()))
    }

    /// Gespeicherter Zugang; `None`, wenn keiner hinterlegt ist.
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

    /// Prüft und speichert. Ein App-Passwort sind 16 Buchstaben (Google zeigt sie in
    /// Vierergruppen) – so fällt das versehentlich eingegebene Konto-Passwort sofort auf.
    pub fn save_gmail(&self, credentials: &Credentials) -> Result<(), SecretError> {
        let user = &credentials.user;
        let valid_user = user
            .split_once('@')
            .is_some_and(|(name, domain)| !name.is_empty() && domain.contains('.'));
        if !valid_user {
            return Err(SecretError::Invalid(
                "Bitte eine vollständige Gmail-Adresse eingeben.",
            ));
        }
        let password = credentials.password();
        if password.chars().count() != 16 || !password.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(SecretError::Invalid(
                "Ein App-Passwort hat genau 16 Buchstaben (nicht das normale Google-Passwort).",
            ));
        }
        let secret = serde_json::json!({ "user": user, "password": password }).to_string();
        self.entry()?
            .set_password(&secret)
            .map_err(|e| SecretError::Store(e.to_string()))
    }

    /// Entfernt den Zugang; `false`, wenn keiner da war.
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

    /// Echter Windows-Tresor mit eigenem Testdienst und Attrappen-Werten.
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
            service: "nie-benutzt".into(),
        };
        for (user, password) in [
            ("ohne-at", "abcdefghijklmnop"),
            ("ich@gmail.com", "MeinKontoPasswort1!"),
            ("ich@gmail.com", "abcd efgh ijkl"),
        ] {
            assert!(matches!(
                vault.save_gmail(&Credentials::new(user, password)),
                Err(SecretError::Invalid(_))
            ));
        }
    }
}
