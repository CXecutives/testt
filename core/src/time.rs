//! Zeit: intern UTC-Zeitstempel, für Anzeige, Dateinamen und Export deutsche Ortszeit.

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;

/// Zeitzone für alles, was der Nutzer sieht.
pub fn berlin() -> TimeZone {
    TimeZone::get("Europe/Berlin").unwrap_or_else(|_| TimeZone::system())
}

/// Ortszeit (Europe/Berlin) eines Zeitstempels.
pub fn local(ts: Timestamp) -> DateTime {
    ts.to_zoned(berlin()).datetime()
}

/// Kalendertag (Europe/Berlin) eines Zeitstempels.
pub fn local_date(ts: Timestamp) -> Date {
    local(ts).date()
}

/// „19.09.2026 14:05“.
pub fn display(ts: Timestamp) -> String {
    local(ts).strftime("%d.%m.%Y %H:%M").to_string()
}

/// Sekunden für die Datenbank.
pub fn to_db(ts: Timestamp) -> i64 {
    ts.as_second()
}

pub fn from_db(seconds: i64) -> Option<Timestamp> {
    Timestamp::from_second(seconds).ok()
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
