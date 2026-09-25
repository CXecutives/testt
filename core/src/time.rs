//! Time: UTC timestamps inside; local time for display, file names and export. Local is the
//! time zone of the OS, like the interface's dates: the app sets it once at its start
//! ([`follow_system_zone`]). Until then, and so in every test, it is Europe/Berlin: the TXT
//! byte tests and the golden digests stay the same on any machine.

use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use tokio_util::sync::CancellationToken;

/// The zone the app set at its start ([`set_zone`]).
static ZONE: OnceLock<TimeZone> = OnceLock::new();
/// The zone until then.
static BERLIN: LazyLock<TimeZone> =
    LazyLock::new(|| TimeZone::get("Europe/Berlin").unwrap_or_else(|_| TimeZone::system()));

/// Time zone of everything the user sees: the OS's once the app set it, else Europe/Berlin.
pub fn zone() -> &'static TimeZone {
    ZONE.get().unwrap_or(&BERLIN)
}

/// Sets the zone of everything the user sees, once per process; `false` if it was set before.
pub fn set_zone(zone: TimeZone) -> bool {
    ZONE.set(zone).is_ok()
}

/// At the start of the app: local time is the OS's, like the interface's. A zone the OS does
/// not name keeps Europe/Berlin.
pub fn follow_system_zone() {
    match TimeZone::try_system() {
        Ok(system) => {
            let name = system.iana_name().unwrap_or("unnamed").to_owned();
            if set_zone(system) {
                log::info!("time zone {name}");
            }
        }
        Err(e) => log::warn!("time zone of the system unknown, Europe/Berlin kept: {e}"),
    }
}

/// Local time of a timestamp.
pub fn local(ts: Timestamp) -> DateTime {
    zone().to_datetime(ts)
}

/// Calendar day (local) of a timestamp.
pub fn local_date(ts: Timestamp) -> Date {
    local(ts).date()
}

/// "19.09.2026 14:05".
pub fn display(ts: Timestamp) -> String {
    local(ts).strftime("%d.%m.%Y %H:%M").to_string()
}

/// Seconds for the database.
pub fn to_db(ts: Timestamp) -> i64 {
    ts.as_second()
}

pub fn from_db(seconds: i64) -> Option<Timestamp> {
    Timestamp::from_second(seconds).ok()
}

/// Waits `wait`; `false` if `cancel` came first - the one cancellable sleep of the app.
pub async fn sleep_cancellable(wait: Duration, cancel: &CancellationToken) -> bool {
    tokio::select! {
        biased;
        () = cancel.cancelled() => false,
        () = tokio::time::sleep(wait) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn berlin_summer_and_winter_time() {
        let summer: Timestamp = "2026-09-19T12:05:00Z".parse().unwrap();
        assert_eq!(display(summer), "19.09.2026 14:05");
        let winter: Timestamp = "2026-01-10T23:30:00Z".parse().unwrap();
        assert_eq!(display(winter), "11.01.2026 00:30");
        assert_eq!(local_date(winter), Date::new(2026, 1, 11).unwrap());
        assert_eq!(from_db(to_db(summer)), Some(summer));
    }
}
