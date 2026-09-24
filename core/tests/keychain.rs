//! Real round trip through the OS keychain (Windows Credential Manager, macOS keychain), the
//! store that holds the Gmail app password. It uses its own unique service name and a dummy
//! value - never the app's entry. Runs on the Windows and macOS CI runners; skipped elsewhere
//! (no OS keychain there).
#![cfg(any(windows, target_os = "macos"))]

use std::time::{SystemTime, UNIX_EPOCH};

use keyring::{Entry, Error};

const USER: &str = "round-trip";
const DUMMY: &str = "dummy value - not a secret";

/// Removes the test item even if an assertion fails half way.
struct Cleanup(String);

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Ok(entry) = Entry::new(&self.0, USER) {
            let _ = entry.delete_credential();
        }
    }
}

#[test]
fn a_value_survives_a_round_trip_through_the_os_keychain() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let service = format!(
        "de.cxecutives.job-alert-monitor.keychain-test.{}.{nanos}",
        std::process::id()
    );
    let _cleanup = Cleanup(service.clone());

    let entry = Entry::new(&service, USER).unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)));
    entry.set_password(DUMMY).unwrap();
    assert_eq!(entry.get_password().unwrap(), DUMMY);
    // A second handle finds the same item: it lives in the OS, not in this process.
    assert_eq!(
        Entry::new(&service, USER).unwrap().get_password().unwrap(),
        DUMMY
    );
    entry.delete_credential().unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)));
    assert!(matches!(entry.delete_credential(), Err(Error::NoEntry)));
}
