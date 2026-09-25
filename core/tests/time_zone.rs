//! The zone of everything the user sees is process-wide: Europe/Berlin until the app sets the
//! OS's at its start, then that one for good. A test binary of its own, so setting the zone
//! never reaches the byte tests of the library, which keep Berlin.

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jobalert_core::time;

#[test]
fn the_app_sets_the_zone_once_and_everything_follows_it() {
    let ts: Timestamp = "2026-09-19T23:30:00Z".parse().unwrap();
    assert_eq!(
        time::display(ts),
        "20.09.2026 01:30",
        "Berlin until the start sets one"
    );
    assert_eq!(time::local_date(ts), Date::new(2026, 9, 20).unwrap());

    let new_york = TimeZone::get("America/New_York").unwrap();
    assert!(time::set_zone(new_york));
    assert_eq!(time::display(ts), "19.09.2026 19:30");
    assert_eq!(time::local_date(ts), Date::new(2026, 9, 19).unwrap());
    assert_eq!(time::zone().iana_name(), Some("America/New_York"));

    // Once per process: a second zone, the OS's too, changes nothing.
    assert!(!time::set_zone(TimeZone::UTC));
    time::follow_system_zone();
    assert_eq!(time::display(ts), "19.09.2026 19:30");
}
