//! Time-of-day wallpaper schedule engine (ROADMAP 3.3).
//!
//! Pure functions over local wall-clock time — no I/O, no globals — so every
//! rule (midnight wrap, DST jumps, solar) is unit-testable, and the module
//! stays platform-neutral (macOS readiness principle: this is "brain" code).
//!
//! The daemon evaluates [`desired`] on its coarse tick with
//! `chrono::Local::now().naive_local()` and the current UTC offset; DST is
//! therefore transparent here — we only ever see the wall clock.

use chrono::{NaiveDateTime, NaiveTime, Timelike};

use crate::config::{Schedule, ScheduleMode, Wallpaper};

/// The wallpaper the schedule wants on screen at `now` (local wall time).
/// `utc_offset_minutes` is only used by solar mode to convert sunrise/sunset
/// (computed in UTC) into local time. Returns None when the schedule is
/// unusable (no wallpapers configured, unparsable times, missing coords).
pub fn desired(s: &Schedule, now: NaiveDateTime, utc_offset_minutes: i32) -> Option<&Wallpaper> {
    let slots = slot_times(s, now, utc_offset_minutes)?;
    // The latest slot at or before now wins; before the first slot of the day,
    // wrap to the LAST slot (it has been active since yesterday).
    let t = now.time();
    let mut winner = slots.last()?;
    for slot in &slots {
        if slot.0 <= t {
            winner = slot;
        }
    }
    Some(winner.1)
}

/// Seconds until the next slot boundary after `now` — lets the daemon log or
/// coarsen its polling. None when the schedule is unusable.
pub fn next_change_secs(s: &Schedule, now: NaiveDateTime, utc_offset_minutes: i32) -> Option<u64> {
    let slots = slot_times(s, now, utc_offset_minutes)?;
    let t = now.time();
    let next = slots
        .iter()
        .map(|(st, _)| *st)
        .filter(|st| *st > t)
        .min()
        // No later slot today → first slot tomorrow.
        .or_else(|| slots.iter().map(|(st, _)| *st).min())?;
    let now_s = i64::from(t.num_seconds_from_midnight());
    let next_s = i64::from(next.num_seconds_from_midnight());
    let delta = if next_s > now_s {
        next_s - now_s
    } else {
        next_s + 86_400 - now_s
    };
    Some(delta as u64)
}

/// Resolve the schedule into (start-time, wallpaper) slots for `now`'s date.
fn slot_times(
    s: &Schedule,
    now: NaiveDateTime,
    utc_offset_minutes: i32,
) -> Option<Vec<(NaiveTime, &Wallpaper)>> {
    let mut slots: Vec<(NaiveTime, &Wallpaper)> = match s.mode {
        ScheduleMode::Daynight => {
            let day = s.day.as_ref()?;
            let night = s.night.as_ref()?;
            vec![
                (parse_hhmm(&s.day_start)?, day),
                (parse_hhmm(&s.night_start)?, night),
            ]
        }
        ScheduleMode::Times => {
            if s.at.is_empty() {
                return None;
            }
            s.at.iter()
                .map(|slot| parse_hhmm(&slot.time).map(|t| (t, &slot.wallpaper)))
                .collect::<Option<Vec<_>>>()?
        }
        ScheduleMode::Solar => {
            let day = s.day.as_ref()?;
            let night = s.night.as_ref()?;
            let (rise_utc_h, set_utc_h) = sunrise_sunset_utc(s.lat?, s.lon?, now.date())?;
            let off = f64::from(utc_offset_minutes) / 60.0;
            vec![
                (hours_to_time(rise_utc_h + off), day),
                (hours_to_time(set_utc_h + off), night),
            ]
        }
    };
    slots.sort_by_key(|(t, _)| *t);
    Some(slots)
}

/// "HH:MM" (24h) → NaiveTime.
pub fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// Fractional hours (may be outside 0..24 after tz shift) → wall-clock time.
fn hours_to_time(h: f64) -> NaiveTime {
    let h = h.rem_euclid(24.0);
    let secs = (h * 3600.0).round() as u32 % 86_400;
    NaiveTime::from_num_seconds_from_midnight_opt(secs, 0)
        .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
}

/// NOAA sunrise/sunset for a date, in fractional hours UTC. None inside polar
/// day/night (no event that date). Accuracy ≈ ±1 minute — the standard NOAA
/// solar-position algorithm, kept dependency-free on purpose.
fn sunrise_sunset_utc(lat: f64, lon: f64, date: chrono::NaiveDate) -> Option<(f64, f64)> {
    use chrono::Datelike;
    let rad = std::f64::consts::PI / 180.0;

    // Fractional year (radians) at solar noon-ish.
    let leap = date.leap_year();
    let days = if leap { 366.0 } else { 365.0 };
    let doy = f64::from(date.ordinal());
    let gamma = 2.0 * std::f64::consts::PI / days * (doy - 1.0 + 0.5);

    // Equation of time (minutes) and solar declination (radians) — NOAA.
    let eqtime = 229.18
        * (0.000075 + 0.001868 * gamma.cos()
            - 0.032077 * gamma.sin()
            - 0.014615 * (2.0 * gamma).cos()
            - 0.040849 * (2.0 * gamma).sin());
    let decl = 0.006918 - 0.399912 * gamma.cos() + 0.070257 * gamma.sin()
        - 0.006758 * (2.0 * gamma).cos()
        + 0.000907 * (2.0 * gamma).sin()
        - 0.002697 * (3.0 * gamma).cos()
        + 0.00148 * (3.0 * gamma).sin();

    // Hour angle for the official sunrise zenith 90.833° (refraction + disc).
    let zenith = 90.833 * rad;
    let cos_ha = (zenith.cos() - (lat * rad).sin() * decl.sin()) / ((lat * rad).cos() * decl.cos());
    if !(-1.0..=1.0).contains(&cos_ha) {
        return None; // polar day or night
    }
    let ha_deg = cos_ha.acos() / rad;

    // Minutes UTC: 720 − 4·(lon + ha) − eqtime  (lon east-positive).
    let sunrise_min = 720.0 - 4.0 * (lon + ha_deg) - eqtime;
    let sunset_min = 720.0 - 4.0 * (lon - ha_deg) - eqtime;
    Some((sunrise_min / 60.0, sunset_min / 60.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Kind, TimeSlot};
    use chrono::NaiveDate;
    use std::path::PathBuf;

    fn wp(path: &str) -> Wallpaper {
        Wallpaper {
            kind: Kind::Video,
            path: Some(PathBuf::from(path)),
            ..Default::default()
        }
    }
    fn daynight() -> Schedule {
        Schedule {
            mode: ScheduleMode::Daynight,
            day: Some(wp("/day.mp4")),
            night: Some(wp("/night.mp4")),
            day_start: "07:00".into(),
            night_start: "19:00".into(),
            lat: None,
            lon: None,
            at: vec![],
        }
    }
    fn at(date: (i32, u32, u32), time: (u32, u32)) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(date.0, date.1, date.2)
            .unwrap()
            .and_hms_opt(time.0, time.1, 0)
            .unwrap()
    }
    fn path_of(w: Option<&Wallpaper>) -> &str {
        w.unwrap().path.as_ref().unwrap().to_str().unwrap()
    }

    #[test]
    fn daynight_basic_and_midnight_wrap() {
        let s = daynight();
        assert_eq!(
            path_of(desired(&s, at((2026, 7, 3), (12, 0)), 0)),
            "/day.mp4"
        );
        assert_eq!(
            path_of(desired(&s, at((2026, 7, 3), (20, 0)), 0)),
            "/night.mp4"
        );
        // 02:00 — before day_start: last slot (night, from yesterday) wraps over.
        assert_eq!(
            path_of(desired(&s, at((2026, 7, 3), (2, 0)), 0)),
            "/night.mp4"
        );
        // Boundaries are inclusive.
        assert_eq!(
            path_of(desired(&s, at((2026, 7, 3), (7, 0)), 0)),
            "/day.mp4"
        );
        assert_eq!(
            path_of(desired(&s, at((2026, 7, 3), (19, 0)), 0)),
            "/night.mp4"
        );
    }

    #[test]
    fn dst_jumps_are_transparent_wall_clock() {
        let s = daynight();
        // Spring forward (EU 2026-03-29: 02:00→03:00). The wall clock never
        // shows 02:30; evaluating right before and after the jump stays night.
        assert_eq!(
            path_of(desired(&s, at((2026, 3, 29), (1, 59)), 60)),
            "/night.mp4"
        );
        assert_eq!(
            path_of(desired(&s, at((2026, 3, 29), (3, 0)), 120)),
            "/night.mp4"
        );
        // Fall back (2026-10-25: 03:00→02:00): 02:30 occurs twice; both are night.
        assert_eq!(
            path_of(desired(&s, at((2026, 10, 25), (2, 30)), 120)),
            "/night.mp4"
        );
        assert_eq!(
            path_of(desired(&s, at((2026, 10, 25), (2, 30)), 60)),
            "/night.mp4"
        );
        // And the day boundary still lands normally that day.
        assert_eq!(
            path_of(desired(&s, at((2026, 10, 25), (7, 1)), 60)),
            "/day.mp4"
        );
    }

    #[test]
    fn times_mode_latest_slot_wins() {
        let s = Schedule {
            mode: ScheduleMode::Times,
            at: vec![
                TimeSlot {
                    time: "06:00".into(),
                    wallpaper: wp("/a.mp4"),
                },
                TimeSlot {
                    time: "12:00".into(),
                    wallpaper: wp("/b.mp4"),
                },
                TimeSlot {
                    time: "22:30".into(),
                    wallpaper: wp("/c.mp4"),
                },
            ],
            day: None,
            night: None,
            day_start: default_start("07:00"),
            night_start: default_start("19:00"),
            lat: None,
            lon: None,
        };
        assert_eq!(
            path_of(desired(&s, at((2026, 7, 3), (11, 59)), 0)),
            "/a.mp4"
        );
        assert_eq!(path_of(desired(&s, at((2026, 7, 3), (12, 0)), 0)), "/b.mp4");
        assert_eq!(path_of(desired(&s, at((2026, 7, 3), (23, 0)), 0)), "/c.mp4");
        assert_eq!(path_of(desired(&s, at((2026, 7, 4), (0, 30)), 0)), "/c.mp4"); // wrap
        assert_eq!(
            next_change_secs(&s, at((2026, 7, 3), (11, 59)), 0),
            Some(60)
        );
    }

    fn default_start(v: &str) -> String {
        v.into()
    }

    #[test]
    fn unusable_schedules_yield_none() {
        let mut s = daynight();
        s.day = None;
        assert!(desired(&s, at((2026, 7, 3), (12, 0)), 0).is_none());
        let mut s2 = daynight();
        s2.day_start = "25:99".into();
        assert!(desired(&s2, at((2026, 7, 3), (12, 0)), 0).is_none());
        let s3 = Schedule {
            mode: ScheduleMode::Times,
            at: vec![],
            ..daynight()
        };
        assert!(desired(&s3, at((2026, 7, 3), (12, 0)), 0).is_none());
    }

    /// NOAA fixtures within ±2 minutes of published values (UTC).
    #[test]
    fn noaa_sunrise_fixtures() {
        let cases = [
            // (lat, lon, y, m, d, sunrise_utc_min, sunset_utc_min) — NOAA calculator values.
            (
                51.4769,
                0.0,
                2024,
                6,
                21,
                3.0 * 60.0 + 43.0,
                20.0 * 60.0 + 21.0,
            ), // Greenwich, June solstice
            (
                51.4769,
                0.0,
                2024,
                3,
                20,
                6.0 * 60.0 + 2.0,
                18.0 * 60.0 + 14.0,
            ), // Greenwich, equinox
            (
                40.7128,
                -74.006,
                2024,
                12,
                21,
                12.0 * 60.0 + 16.0,
                21.0 * 60.0 + 32.0,
            ), // NYC, Dec solstice
        ];
        for (lat, lon, y, m, d, rise_min, set_min) in cases {
            let date = NaiveDate::from_ymd_opt(y, m, d).unwrap();
            let (r, s) = sunrise_sunset_utc(lat, lon, date).unwrap();
            let (r_min, s_min) = (r * 60.0, s * 60.0);
            assert!(
                (r_min - rise_min).abs() <= 2.0,
                "sunrise {lat},{lon} {y}-{m}-{d}: got {r_min:.1}, want {rise_min:.1}"
            );
            assert!(
                (s_min - set_min).abs() <= 2.0,
                "sunset {lat},{lon} {y}-{m}-{d}: got {s_min:.1}, want {set_min:.1}"
            );
        }
    }

    #[test]
    fn solar_mode_uses_local_offset() {
        let s = Schedule {
            mode: ScheduleMode::Solar,
            lat: Some(51.4769),
            lon: Some(0.0),
            ..daynight()
        };
        // Greenwich June solstice, BST (+60): sunrise ≈ 04:43, sunset ≈ 21:21 local.
        assert_eq!(
            path_of(desired(&s, at((2024, 6, 21), (12, 0)), 60)),
            "/day.mp4"
        );
        assert_eq!(
            path_of(desired(&s, at((2024, 6, 21), (22, 0)), 60)),
            "/night.mp4"
        );
        assert_eq!(
            path_of(desired(&s, at((2024, 6, 21), (4, 0)), 60)),
            "/night.mp4"
        );
        assert_eq!(
            path_of(desired(&s, at((2024, 6, 21), (5, 0)), 60)),
            "/day.mp4"
        );
    }

    #[test]
    fn polar_night_yields_none() {
        let s = Schedule {
            mode: ScheduleMode::Solar,
            lat: Some(78.0), // Svalbard
            lon: Some(15.0),
            ..daynight()
        };
        assert!(desired(&s, at((2026, 12, 21), (12, 0)), 60).is_none());
    }
}
