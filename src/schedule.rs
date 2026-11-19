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

use std::path::Path;

use crate::config::{CycleSource, Schedule, ScheduleMode, Wallpaper};

/// Which half of a day/night (or solar) schedule is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Day,
    Night,
}

/// What the schedule shows at some instant: the slot's fixed wallpaper, its
/// pool, or both (the fixed one is the fallback when the pool is empty).
#[derive(Clone, Copy, Debug)]
pub struct Active<'a> {
    /// None for `times` slots, which have no pool.
    pub slot: Option<Slot>,
    pub fixed: Option<&'a Wallpaper>,
    pub source: Option<&'a CycleSource>,
}

/// The slot active at `now` (local wall time). `utc_offset_minutes` is only
/// used by solar mode to convert sunrise/sunset (computed in UTC) into local
/// time. None when the schedule is unusable (no wallpaper or pool configured,
/// unparsable times, missing coords).
pub fn active(s: &Schedule, now: NaiveDateTime, utc_offset_minutes: i32) -> Option<Active<'_>> {
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

/// The fixed wallpaper of the active slot. None when that slot is pool-only or
/// the schedule is unusable. The daemon uses [`active`] so it can draw from the
/// pool; this is the GUI's view of the fixed choice.
pub fn desired(s: &Schedule, now: NaiveDateTime, utc_offset_minutes: i32) -> Option<&Wallpaper> {
    active(s, now, utc_offset_minutes)?.fixed
}

/// Sticky random pick from a day/night slot's pool (#130). The same file is
/// returned until the active slot changes, the pick is cleared (schedule off or
/// no pool), or the file disappears, so the 2 s tick never re-rolls. Lives in
/// the daemon. A daemon restart re-picks, which is acceptable.
#[derive(Default)]
pub struct SlotPick {
    key: Option<(Slot, CycleSource)>,
    pick: Option<Wallpaper>,
}

impl SlotPick {
    pub const fn new() -> Self {
        SlotPick {
            key: None,
            pick: None,
        }
    }

    /// Forget the pick (schedule off, or the active slot has no pool).
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// The wallpaper to show for `slot` drawing from `source`. `pool` lists the
    /// candidates and is only called when a new pick is needed; `rand` chooses
    /// among them. `current` is the configured wallpaper: on the first pick
    /// after the schedule was switched on, a member of the pool is kept, so
    /// turning the schedule on does not re-roll what the user just set. None
    /// when the pool is empty.
    pub fn resolve(
        &mut self,
        slot: Slot,
        source: &CycleSource,
        pool: impl FnOnce() -> Vec<Wallpaper>,
        rand: usize,
        current: Option<&Wallpaper>,
    ) -> Option<Wallpaper> {
        let same = self
            .key
            .as_ref()
            .is_some_and(|(s, src)| *s == slot && src == source);
        if same {
            if let Some(w) = self
                .pick
                .as_ref()
                .filter(|w| w.effective_path().is_some_and(Path::exists))
            {
                return Some(w.clone());
            }
        }
        let first = self.key.is_none();
        let files = pool();
        let adopted = first
            .then(|| {
                current
                    .filter(|c| files.iter().any(|f| f.same_media(c)))
                    .cloned()
            })
            .flatten();
        self.key = Some((slot, source.clone()));
        // ponytail: an empty pool is re-listed on every tick. Cheap next to the
        // 2 s tick; a folder watcher would be the upgrade if it ever matters.
        self.pick =
            adopted.or_else(|| (!files.is_empty()).then(|| files[rand % files.len()].clone()));
        self.pick.clone()
    }
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

/// The next slot change after `now`: when it happens (local wall time,
/// minute precision) and the slot that takes over then. Drives the
/// "Next switch at …" hint in Advanced; None when the schedule is unusable.
pub fn next_switch(
    s: &Schedule,
    now: NaiveDateTime,
    utc_offset_minutes: i32,
) -> Option<(NaiveTime, Active<'_>)> {
    let secs = next_change_secs(s, now, utc_offset_minutes)?;
    let at = now.checked_add_signed(chrono::Duration::seconds(secs as i64))?;
    let a = active(s, at, utc_offset_minutes)?;
    Some((at.time(), a))
}

/// "Next switch at 17:16: name" — pure so the wording is testable.
pub fn next_switch_hint(at: NaiveTime, name: &str) -> String {
    crate::tf!(
        "Next switch at {time}: {name}",
        "time" => at.format("%H:%M").to_string(),
        "name" => name
    )
}

/// Index of `target` among the library's paths, or None when it is no longer
/// there. Callers must NOT default to 0: showing (and later saving) the first
/// library item for a wallpaper that left the library silently rewrites the
/// schedule.
pub fn library_position<'a>(
    paths: impl IntoIterator<Item = Option<&'a std::path::Path>>,
    target: &std::path::Path,
) -> Option<usize> {
    paths.into_iter().position(|p| p == Some(target))
}

/// Resolve the schedule into (start-time, active slot) pairs for `now`'s date.
fn slot_times(
    s: &Schedule,
    now: NaiveDateTime,
    utc_offset_minutes: i32,
) -> Option<Vec<(NaiveTime, Active<'_>)>> {
    let mut slots: Vec<(NaiveTime, Active<'_>)> = match s.mode {
        ScheduleMode::Daynight => vec![
            (parse_hhmm(&s.day_start)?, part(s, Slot::Day)?),
            (parse_hhmm(&s.night_start)?, part(s, Slot::Night)?),
        ],
        ScheduleMode::Times => {
            if s.at.is_empty() {
                return None;
            }
            s.at.iter()
                .map(|slot| {
                    let a = Active {
                        slot: None,
                        fixed: Some(&slot.wallpaper),
                        source: None,
                    };
                    parse_hhmm(&slot.time).map(|t| (t, a))
                })
                .collect::<Option<Vec<_>>>()?
        }
        ScheduleMode::Solar => {
            let (day, night) = (part(s, Slot::Day)?, part(s, Slot::Night)?);
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

/// One half of a day/night schedule. None when it has neither a wallpaper nor
/// a pool, which makes the whole schedule unusable.
fn part(s: &Schedule, slot: Slot) -> Option<Active<'_>> {
    let (fixed, source) = match slot {
        Slot::Day => (s.day.as_ref(), s.day_source.as_ref()),
        Slot::Night => (s.night.as_ref(), s.night_source.as_ref()),
    };
    (fixed.is_some() || source.is_some()).then_some(Active {
        slot: Some(slot),
        fixed,
        source,
    })
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
            day_source: None,
            night_source: None,
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
            day_source: None,
            night_source: None,
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
            day_source: None,
            night_source: None,
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
            day_source: None,
            night_source: None,
            ..daynight()
        };
        assert!(desired(&s, at((2026, 12, 21), (12, 0)), 60).is_none());
    }

    // ── Issue #32: reporter's schedule + edge cases ──

    fn reporter() -> Schedule {
        Schedule {
            night_start: "17:16".into(),
            ..daynight()
        }
    }
    fn at_s(h: u32, m: u32, sec: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 30)
            .unwrap()
            .and_hms_opt(h, m, sec)
            .unwrap()
    }

    #[test]
    fn reporters_case_1715_day_1716_night() {
        let s = reporter();
        assert_eq!(path_of(desired(&s, at_s(17, 15, 0), 0)), "/day.mp4");
        assert_eq!(path_of(desired(&s, at_s(17, 16, 0), 0)), "/night.mp4");
        assert_eq!(next_change_secs(&s, at_s(17, 15, 0), 0), Some(60));
        let (t, w) = next_switch(&s, at_s(17, 15, 0), 0).unwrap();
        assert_eq!(t.format("%H:%M").to_string(), "17:16");
        assert_eq!(path_of(w.fixed), "/night.mp4");
    }

    #[test]
    fn night_before_day_wraps_midnight_both_sides() {
        // Night 01:00–07:00 is unusual but valid: night_start < day_start.
        let s = Schedule {
            day_start: "07:00".into(),
            night_start: "01:00".into(),
            ..daynight()
        };
        assert_eq!(path_of(desired(&s, at_s(0, 30, 0), 0)), "/day.mp4");
        assert_eq!(path_of(desired(&s, at_s(1, 0, 0), 0)), "/night.mp4");
        assert_eq!(path_of(desired(&s, at_s(6, 59, 0), 0)), "/night.mp4");
        assert_eq!(path_of(desired(&s, at_s(23, 59, 0), 0)), "/day.mp4");
        // Before the first slot: next change is today's 01:00.
        assert_eq!(next_change_secs(&s, at_s(0, 30, 0), 0), Some(1800));
        // After the last slot: next change wraps to tomorrow's 01:00.
        assert_eq!(next_change_secs(&s, at_s(23, 0, 0), 0), Some(2 * 3600));
    }

    #[test]
    fn equal_start_times_are_deterministic() {
        let s = Schedule {
            day_start: "08:00".into(),
            night_start: "08:00".into(),
            ..daynight()
        };
        let a = path_of(desired(&s, at_s(3, 0, 0), 0)).to_string();
        let b = path_of(desired(&s, at_s(8, 0, 0), 0)).to_string();
        let c = path_of(desired(&s, at_s(20, 0, 0), 0)).to_string();
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert!(next_change_secs(&s, at_s(8, 0, 0), 0).is_some());
        assert!(next_switch(&s, at_s(8, 0, 0), 0).is_some());
    }

    #[test]
    fn invalid_or_empty_times_yield_none() {
        for bad in ["", "  ", "7", "25:00", "07:60", "ab:cd"] {
            let mut s = daynight();
            s.day_start = bad.into();
            assert!(desired(&s, at_s(12, 0, 0), 0).is_none(), "{bad:?}");
            assert!(next_change_secs(&s, at_s(12, 0, 0), 0).is_none(), "{bad:?}");
            assert!(next_switch(&s, at_s(12, 0, 0), 0).is_none(), "{bad:?}");
            assert!(parse_hhmm(bad).is_none(), "{bad:?}");
        }
        let mut s = daynight();
        s.night_start = String::new();
        assert!(desired(&s, at_s(12, 0, 0), 0).is_none());
    }

    #[test]
    fn library_position_finds_stored_paths_only() {
        use std::path::Path;
        let lib = [Some(Path::new("/a.mp4")), None, Some(Path::new("/b.mp4"))];
        assert_eq!(library_position(lib, Path::new("/b.mp4")), Some(2));
        // A wallpaper that left the library must NOT fall back to slot 0.
        assert_eq!(library_position(lib, Path::new("/gone.mp4")), None);
    }

    #[test]
    fn next_switch_hint_text() {
        let t = NaiveTime::from_hms_opt(17, 16, 40).unwrap();
        assert_eq!(
            next_switch_hint(t, "海屿你"),
            "Next switch at 17:16: 海屿你"
        );
    }
}

/// #130: day/night slots that draw from a pool.
#[cfg(test)]
mod pool_tests {
    use super::*;
    use crate::config::Kind;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 7, 3)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }
    fn fixed(path: &str) -> Wallpaper {
        Wallpaper {
            kind: Kind::Video,
            path: Some(PathBuf::from(path)),
            ..Default::default()
        }
    }
    fn daynight() -> Schedule {
        Schedule {
            mode: ScheduleMode::Daynight,
            day: Some(fixed("/day.mp4")),
            night: Some(fixed("/night.mp4")),
            day_source: None,
            night_source: None,
            day_start: "07:00".into(),
            night_start: "19:00".into(),
            lat: None,
            lon: None,
            at: vec![],
        }
    }
    fn folder(tag: &str, names: &[&str]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fresco-pool-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in names {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        d
    }
    /// What `cycle::slot_pool` would list for `d`: sorted, one wallpaper per file.
    fn listed(d: &Path) -> Vec<Wallpaper> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        files
            .into_iter()
            .map(|p| Wallpaper {
                kind: Kind::Image,
                path: Some(p),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn old_config_without_pools_parses_and_writes_no_pool_keys() {
        let s: Schedule = toml::from_str(
            "mode = \"daynight\"\n[day]\npath = \"/d.mp4\"\n[night]\npath = \"/n.mp4\"\n",
        )
        .unwrap();
        assert!(s.day_source.is_none() && s.night_source.is_none());
        assert!(!toml::to_string(&s).unwrap().contains("source"));
    }

    #[test]
    fn pool_source_parses_from_config() {
        let s: Schedule = toml::from_str(
            "mode = \"daynight\"\n[day]\npath = \"/d.mp4\"\n\
             [night]\npath = \"/n.mp4\"\n\
             [night_source]\nkind = \"folder\"\nvalue = \"/pics\"\n",
        )
        .unwrap();
        assert_eq!(s.day_source, None);
        assert_eq!(
            s.night_source,
            Some(CycleSource::Folder(PathBuf::from("/pics")))
        );
    }

    #[test]
    fn a_pool_alone_makes_a_slot_valid() {
        let src = CycleSource::Folder(PathBuf::from("/pics"));
        let s = Schedule {
            day: None,
            day_source: Some(src.clone()),
            ..daynight()
        };
        let a = active(&s, at(12, 0), 0).expect("pool-only day slot is usable");
        assert_eq!(a.slot, Some(Slot::Day));
        assert!(a.fixed.is_none());
        assert_eq!(a.source.cloned(), Some(src));
        assert!(desired(&s, at(12, 0), 0).is_none());
        assert!(next_change_secs(&s, at(12, 0), 0).is_some());
        // The night slot is still the fixed wallpaper.
        let night = active(&s, at(20, 0), 0).unwrap();
        assert_eq!(
            night.fixed.unwrap().effective_path(),
            Some(Path::new("/night.mp4"))
        );
        assert!(night.source.is_none());
    }

    #[test]
    fn a_slot_with_neither_wallpaper_nor_pool_is_unusable() {
        let s = Schedule {
            day: None,
            ..daynight()
        };
        assert!(active(&s, at(12, 0), 0).is_none());
    }

    #[test]
    fn sticky_pick_holds_until_the_slot_changes() {
        let d = folder("sticky", &["a.png", "b.png", "c.png", "d.png"]);
        let src = CycleSource::Folder(d.clone());
        let mut p = SlotPick::new();
        let first = p.resolve(Slot::Day, &src, || listed(&d), 1, None).unwrap();
        for r in [0, 2, 3, 7] {
            assert_eq!(
                p.resolve(Slot::Day, &src, || listed(&d), r, None),
                Some(first.clone())
            );
        }
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn re_picks_when_the_slot_changes_and_when_the_file_vanishes() {
        let d = folder("reroll", &["a.png", "b.png", "c.png", "d.png"]);
        let src = CycleSource::Folder(d.clone());
        let mut p = SlotPick::new();
        let day = p.resolve(Slot::Day, &src, || listed(&d), 0, None).unwrap();
        let night = p
            .resolve(Slot::Night, &src, || listed(&d), 2, None)
            .unwrap();
        assert_eq!(night.effective_path(), Some(d.join("c.png").as_path()));
        // Back to day with a different roll: a new pick is made.
        let day2 = p.resolve(Slot::Day, &src, || listed(&d), 3, None).unwrap();
        assert_eq!(day2.effective_path(), Some(d.join("d.png").as_path()));
        assert_ne!(day, day2);
        // The picked file is deleted: re-roll instead of returning a dead path.
        std::fs::remove_file(d.join("d.png")).unwrap();
        let again = p.resolve(Slot::Day, &src, || listed(&d), 0, None).unwrap();
        assert!(again.effective_path().unwrap().exists());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn empty_pool_gives_no_pick_and_later_files_are_picked_up() {
        let d = folder("empty", &[]);
        let src = CycleSource::Folder(d.clone());
        let mut p = SlotPick::new();
        assert!(p.resolve(Slot::Day, &src, || listed(&d), 5, None).is_none());
        std::fs::write(d.join("a.png"), b"x").unwrap();
        assert_eq!(
            p.resolve(Slot::Day, &src, || listed(&d), 5, None)
                .unwrap()
                .effective_path(),
            Some(d.join("a.png").as_path())
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn first_pick_keeps_the_configured_member_of_the_pool() {
        let d = folder("adopt", &["a.png", "b.png"]);
        let src = CycleSource::Folder(d.clone());
        let current = listed(&d).remove(1); // b.png, already set by the user
        let mut p = SlotPick::new();
        // Any roll would pick a.png; the schedule was just switched on.
        let got = p
            .resolve(Slot::Day, &src, || listed(&d), 0, Some(&current))
            .unwrap();
        assert_eq!(got.effective_path(), Some(d.join("b.png").as_path()));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn clear_forgets_the_pick() {
        let d = folder("clear", &["a.png", "b.png"]);
        let src = CycleSource::Folder(d.clone());
        let mut p = SlotPick::new();
        p.resolve(Slot::Day, &src, || listed(&d), 0, None);
        p.clear();
        // After a clear the first pick again adopts the configured wallpaper.
        let current = listed(&d).remove(1);
        let got = p
            .resolve(Slot::Day, &src, || listed(&d), 0, Some(&current))
            .unwrap();
        assert_eq!(got.effective_path(), Some(d.join("b.png").as_path()));
        std::fs::remove_dir_all(&d).ok();
    }
}
