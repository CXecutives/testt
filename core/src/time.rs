//! Time: UTC timestamps inside; German local time for display, file names and export.

use std::time::Duration;

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use tokio_util::sync::CancellationToken;

/// Time zone of everything the user sees.
pub fn berlin() -> TimeZone {
    TimeZone::get("Europe/Berlin").unwrap_or_else(|_| TimeZone::system())
}

/// Local time (Europe/Berlin) of a timestamp.
pub fn local(ts: Timestamp) -> DateTime {
    ts.to_zoned(berlin()).datetime()
}

/// Calendar day (Europe/Berlin) of a timestamp.
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
