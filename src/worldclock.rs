//! World-clock logic: which zones to show, and what each one reads right now.
//!
//! Pure functions of a [`WorldClock`] and an instant — no I/O, no `Local`, no
//! rasteriser — so the daemon's widget layer, the floating overlays and the
//! tests all agree on the text. `crate::widgetkit::cards::worldclock` draws
//! these [`Row`]s; this module only decides what they say.
//!
//! Zone names are IANA (`"Asia/Tokyo"`), resolved with `chrono-tz` because
//! neither std nor chrono carries a tz database. A name that does not parse is
//! skipped, never an error: a config hand-edited to a typo, or a zone retired
//! by a newer tzdata, costs one row and not the whole widget.

use chrono::{DateTime, DurationRound, TimeDelta, Timelike, Utc};
use chrono_tz::Tz;

use crate::config::{WorldClock, WorldZone};

/// One line of the widget: a place and the time there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// `"Tokyo"`.
    pub label: String,
    /// `"9:41 PM"`.
    pub time: String,
}

/// The zone for an IANA name, or `None` if it is not one. Case-sensitive, as
/// the tz database is.
pub fn parse_zone(tz: &str) -> Option<Tz> {
    tz.trim().parse().ok()
}

/// What to call a zone: its label, or — when that is blank — the city part of
/// the IANA name with underscores as spaces (`"America/New_York"` ->
/// `"New York"`).
pub fn label_for(zone: &WorldZone) -> String {
    if !zone.label.trim().is_empty() {
        return zone.label.trim().to_string();
    }
    let city = zone.tz.trim().rsplit('/').next().unwrap_or_default();
    city.replace('_', " ")
}

/// `at` as a 12-hour wall time in `tz`: no leading zero, upper-case meridiem.
/// Midnight is `12:00 AM`, noon `12:00 PM`.
pub fn format_12h(at: DateTime<Utc>, tz: Tz) -> String {
    let t = at.with_timezone(&tz);
    let (pm, hour) = t.hour12();
    format!("{hour}:{:02} {}", t.minute(), if pm { "PM" } else { "AM" })
}

/// The rows to draw at `at`, in config order, skipping zones that do not parse.
pub fn rows(cfg: &WorldClock, at: DateTime<Utc>) -> Vec<Row> {
    cfg.zones
        .iter()
        .filter_map(|z| {
            let tz = parse_zone(&z.tz)?;
            Some(Row {
                label: label_for(z),
                time: format_12h(at, tz),
            })
        })
        .collect()
}

/// The next instant any row's text can differ: the top of the next minute.
///
/// Every zone in the IANA database has had a whole-minute UTC offset since long
/// before the clocks it describes, so the displayed minute changes on the same
/// UTC minute boundary everywhere — one sleep serves all rows. Strictly after
/// `at`: called exactly on a boundary it returns the following one.
pub fn next_change(at: DateTime<Utc>) -> DateTime<Utc> {
    let minute = TimeDelta::minutes(1);
    at.duration_trunc(minute)
        .ok()
        .and_then(|t| t.checked_add_signed(minute))
        .unwrap_or(at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 16, h, m, s).unwrap()
    }

    fn zone(tz: &str, label: &str) -> WorldZone {
        WorldZone {
            tz: tz.into(),
            label: label.into(),
        }
    }

    #[test]
    fn twelve_hour_format_has_no_leading_zero_and_a_correct_meridiem() {
        let tokyo = parse_zone("Asia/Tokyo").unwrap();
        assert_eq!(format_12h(utc(12, 41, 0), tokyo), "9:41 PM");
        assert_eq!(format_12h(utc(15, 0, 0), tokyo), "12:00 AM");
        assert_eq!(format_12h(utc(3, 0, 0), tokyo), "12:00 PM");
        assert_eq!(format_12h(utc(0, 5, 0), tokyo), "9:05 AM");
        // A half-hour zone: the minutes are the zone's, not UTC's.
        let kolkata = parse_zone("Asia/Kolkata").unwrap();
        assert_eq!(format_12h(utc(12, 41, 0), kolkata), "6:11 PM");
    }

    #[test]
    fn labels_come_from_the_config_or_the_zone_name() {
        assert_eq!(label_for(&zone("Asia/Tokyo", "Home")), "Home");
        assert_eq!(label_for(&zone("America/New_York", "")), "New York");
        assert_eq!(
            label_for(&zone("America/Argentina/Buenos_Aires", " ")),
            "Buenos Aires"
        );
        assert_eq!(label_for(&zone("UTC", "")), "UTC");
    }

    #[test]
    fn a_bad_zone_costs_one_row_not_the_widget() {
        let cfg = WorldClock {
            zones: vec![
                zone("Asia/Tokyo", ""),
                zone("Mars/Olympus_Mons", ""),
                zone("Europe/London", "UK"),
            ],
            ..WorldClock::default()
        };
        let rows = rows(&cfg, utc(12, 41, 0));
        assert_eq!(
            rows,
            vec![
                Row {
                    label: "Tokyo".into(),
                    time: "9:41 PM".into()
                },
                Row {
                    label: "UK".into(),
                    time: "1:41 PM".into()
                },
            ]
        );
    }

    #[test]
    fn next_change_is_the_next_whole_minute() {
        let at = utc(12, 41, 30) + TimeDelta::milliseconds(500);
        assert_eq!(next_change(at), utc(12, 42, 0));
        // Exactly on a boundary: strictly later, or the caller spins.
        assert_eq!(next_change(utc(12, 42, 0)), utc(12, 43, 0));
        // Rolls over midnight.
        let next_day = Utc.with_ymd_and_hms(2026, 10, 17, 0, 0, 0).unwrap();
        assert_eq!(next_change(utc(23, 59, 59)), next_day);
    }
}
