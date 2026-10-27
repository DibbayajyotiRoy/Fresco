//! Battery status for the lock-screen widget (lock-screen aesthetics feature,
//! wave 1 — the daemon-side glue that turns this into a card lands
//! separately, in wave 2; this module only owns the data).
//!
//! # Why sysfs, not UPower
//!
//! `/sys/class/power_supply` works everywhere this needs to: with no D-Bus
//! session bus at all (a lock-screen context may have none), inside the
//! Flatpak sandbox (the filesystem is exposed read-only there; UPower is a
//! *session* service with no such guarantee), and it is what the daemon
//! already reads — see [`on_battery`], which replaces the identical private
//! helper that used to live in `daemon::mod` (~line 1529). Reading the same
//! files as the code it replaces means there is exactly one place that
//! understands this sysfs layout, instead of two copies drifting apart.
//!
//! # Layering
//!
//! [`read`]/[`read_from`] do the only I/O in this module: one `read_dir` plus
//! a handful of small file reads per entry. Everything they call to interpret
//! those bytes — [`ChargeState`] parsing, the percent/time-remaining
//! aggregation across multiple batteries — is a pure function over
//! already-read strings, exercised below with fake sysfs trees built under
//! `std::env::temp_dir()` (no root, no real battery, no `sudo`).
//!
//! # System battery vs. peripherals
//!
//! `/sys/class/power_supply` also lists **peripheral** batteries: a
//! Bluetooth/USB mouse, keyboard or headset shows up as `hidpp_battery_0` or
//! similar, with the very same `type`/`capacity`/`status` files a laptop
//! battery has. The kernel tags these `scope: Device`; the system's own
//! battery either has no `scope` file at all (the common case) or, on some
//! ACPI backends, `scope: System`. [`read_from`] only aggregates
//! `type == Battery` entries whose scope is **not** `Device` — a lock screen
//! that reported a wireless mouse's charge as the machine's battery would be
//! actively misleading, not just imprecise.
//!
//! [`on_battery`] used to be a **looser**, bug-for-bug port of the daemon's
//! pre-existing quick check that skipped this filtering entirely — see its
//! own doc comment for the regression that shipped and the fix.
//!
//! # Two field families
//!
//! The kernel's battery class exposes energy in one of two unit families,
//! depending on the fuel gauge: `energy_*`/`power_now` in µWh/µW, or
//! `charge_*`/`current_now` in µAh/µA. A given battery only ever reports one
//! family, so every computation here tries the energy family first and falls
//! back to the charge family — never mixes the two, which would need a
//! voltage to convert between them and sysfs does not reliably expose one.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Charge/discharge state, straight off sysfs `status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl ChargeState {
    /// Parse sysfs `status`. Total: an unrecognised or missing value is
    /// [`ChargeState::Unknown`] rather than an error — a lock screen with an
    /// unreadable status is still better shown as "unknown" than not shown.
    fn parse(s: &str) -> ChargeState {
        match s.trim() {
            "Charging" => ChargeState::Charging,
            "Discharging" => ChargeState::Discharging,
            "Full" => ChargeState::Full,
            "Not charging" => ChargeState::NotCharging,
            _ => ChargeState::Unknown,
        }
    }
}

/// One snapshot of the system battery — aggregated across every system
/// battery reported (a dual-battery laptop has BAT0 **and** BAT1) — plus
/// whether the machine is currently on external power.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryStatus {
    /// 0..=100.
    pub percent: u8,
    pub state: ChargeState,
    /// True when any Mains/USB power supply reports `online: 1`.
    pub on_ac: bool,
    /// Estimated time to empty (discharging) or to full (charging).
    /// `None` while full/not-charging/unknown, and whenever sysfs doesn't
    /// publish enough to compute a number — see `hours_to_duration`.
    pub time_remaining: Option<Duration>,
}

/// The system battery, reading real sysfs. `None` on a desktop with no
/// battery at all. See [`read_from`] for the testable/injectable version.
pub fn read() -> Option<BatteryStatus> {
    read_from(Path::new("/sys/class/power_supply"))
}

/// [`read`], but rooted at `root` instead of `/sys/class/power_supply` — the
/// seam the tests use to exercise every sysfs shape without touching real
/// hardware or `/sys` itself.
///
/// `None` means either "this directory has no `type == Battery` entry that
/// isn't a peripheral" (a desktop, or a peripheral-only tree) or "the
/// directory couldn't be read at all". Both are normal, not errors.
pub fn read_from(root: &Path) -> Option<BatteryStatus> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut batteries: Vec<RawBattery> = Vec::new();
    let mut on_ac = false;
    for entry in entries.flatten() {
        let dir = entry.path();
        let Some(kind) = read_trimmed(&dir.join("type")) else {
            continue;
        };
        if is_ac_type(&kind) {
            if read_trimmed(&dir.join("online")).as_deref() == Some("1") {
                on_ac = true;
            }
            continue;
        }
        if kind != "Battery" {
            continue;
        }
        // Peripheral batteries (mice, keyboards, headsets) carry the same
        // files as the system battery but are tagged `scope: Device`. Only
        // that exact tag is excluded — no `scope` file at all, or an explicit
        // `System`, both count as the machine's own battery.
        if read_trimmed(&dir.join("scope")).as_deref() == Some("Device") {
            continue;
        }
        batteries.push(read_battery_dir(&dir));
    }
    if batteries.is_empty() {
        return None;
    }
    let percent = aggregate_percent(&batteries)?;
    let state = aggregate_state(&batteries);
    let time_remaining = aggregate_time_remaining(&batteries, state);
    Some(BatteryStatus {
        percent,
        state,
        on_ac,
        time_remaining,
    })
}

/// Everything read out of one `type == Battery` sysfs directory. Split from
/// [`BatteryStatus`] because a machine can have more than one of these and
/// they must be combined before anything here becomes a fact about "the
/// battery".
struct RawBattery {
    state: ChargeState,
    capacity: Option<u8>,
    energy_now: Option<u64>,
    energy_full: Option<u64>,
    power_now: Option<u64>,
    charge_now: Option<u64>,
    charge_full: Option<u64>,
    current_now: Option<u64>,
}

fn read_battery_dir(dir: &Path) -> RawBattery {
    RawBattery {
        state: read_trimmed(&dir.join("status"))
            .map(|s| ChargeState::parse(&s))
            .unwrap_or(ChargeState::Unknown),
        capacity: read_capacity(&dir.join("capacity")),
        energy_now: read_u64(&dir.join("energy_now")),
        energy_full: read_u64(&dir.join("energy_full")),
        power_now: read_u64(&dir.join("power_now")),
        charge_now: read_u64(&dir.join("charge_now")),
        charge_full: read_u64(&dir.join("charge_full")),
        current_now: read_u64(&dir.join("current_now")),
    }
}

/// Sysfs `type` values that describe a charger, not a battery: `Mains` for a
/// wall adapter, everything else by `USB` prefix for a USB-negotiated supply.
/// The kernel reports several USB sub-types (`USB_PD`, `USB_PD_DRP`,
/// `USB_DCP`, `USB_CDP`, `USB_ACA`, plain `USB`) for what is functionally the
/// same thing, so matching the prefix avoids hardcoding each one.
fn is_ac_type(kind: &str) -> bool {
    kind == "Mains" || kind.starts_with("USB")
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

/// `capacity` clamped to a real percentage — some firmware reports slightly
/// over 100 at a full charge, and clamping (rather than rejecting) keeps that
/// still usable instead of throwing the whole reading away.
fn read_capacity(path: &Path) -> Option<u8> {
    let raw: u32 = read_trimmed(path)?.parse().ok()?;
    Some(raw.min(100) as u8)
}

// ---------------------------------------------------------------------------
// Aggregation (pure)
// ---------------------------------------------------------------------------

/// Percent across every battery: the energy ratio when *all* of them report
/// it, else the charge ratio when all report that, else the plain average of
/// whatever `capacity` values exist. `None` only when none of the three is
/// available from any battery.
fn aggregate_percent(batteries: &[RawBattery]) -> Option<u8> {
    ratio_percent(batteries, |b| (b.energy_now, b.energy_full))
        .or_else(|| ratio_percent(batteries, |b| (b.charge_now, b.charge_full)))
        .or_else(|| average_capacity(batteries))
}

/// Sum `pick`'s two fields across every battery and turn the totals into a
/// percentage — but only if *every* battery has both fields; a partial sum
/// would silently understate a battery that just doesn't report this family.
fn ratio_percent(
    batteries: &[RawBattery],
    pick: impl Fn(&RawBattery) -> (Option<u64>, Option<u64>),
) -> Option<u8> {
    let mut now_sum = 0u64;
    let mut full_sum = 0u64;
    for b in batteries {
        let (now, full) = pick(b);
        now_sum = now_sum.checked_add(now?)?;
        full_sum = full_sum.checked_add(full?)?;
    }
    percent_from_ratio(now_sum, full_sum)
}

fn percent_from_ratio(now: u64, full: u64) -> Option<u8> {
    if full == 0 {
        return None;
    }
    let pct = (now.min(full) as f64 / full as f64 * 100.0).round();
    Some(pct.clamp(0.0, 100.0) as u8)
}

fn average_capacity(batteries: &[RawBattery]) -> Option<u8> {
    let caps: Vec<u32> = batteries
        .iter()
        .filter_map(|b| b.capacity.map(u32::from))
        .collect();
    if caps.is_empty() {
        return None;
    }
    let avg = caps.iter().sum::<u32>() as f64 / caps.len() as f64;
    Some(avg.round().clamp(0.0, 100.0) as u8)
}

/// Combine every battery's [`ChargeState`] into one. Real dual-battery
/// laptops keep both in lockstep, so this only matters during the brief
/// windows (or synthetic inputs) where they disagree — priority order is
/// "most action-relevant to a lock screen first": discharging beats
/// charging beats not-charging beats full beats unknown.
fn aggregate_state(batteries: &[RawBattery]) -> ChargeState {
    let states: Vec<ChargeState> = batteries.iter().map(|b| b.state).collect();
    [
        ChargeState::Discharging,
        ChargeState::Charging,
        ChargeState::NotCharging,
        ChargeState::Full,
    ]
    .into_iter()
    .find(|want| states.contains(want))
    .unwrap_or(ChargeState::Unknown)
}

/// Time to empty/full across every battery, or `None` when not charging or
/// discharging, or when sysfs doesn't publish enough of one field family to
/// compute it.
fn aggregate_time_remaining(batteries: &[RawBattery], state: ChargeState) -> Option<Duration> {
    if !matches!(state, ChargeState::Charging | ChargeState::Discharging) {
        return None;
    }
    time_from_energy(batteries, state).or_else(|| time_from_charge(batteries, state))
}

/// Sum one field across every battery, or `None` if any battery lacks it —
/// same all-or-nothing rule as [`ratio_percent`], and for the same reason.
fn sum_all(batteries: &[RawBattery], pick: impl Fn(&RawBattery) -> Option<u64>) -> Option<u64> {
    let mut total = 0u64;
    for b in batteries {
        total = total.checked_add(pick(b)?)?;
    }
    Some(total)
}

fn time_from_energy(batteries: &[RawBattery], state: ChargeState) -> Option<Duration> {
    let power = sum_all(batteries, |b| b.power_now)?;
    let now = sum_all(batteries, |b| b.energy_now)?;
    let full = sum_all(batteries, |b| b.energy_full)?;
    let remaining = match state {
        ChargeState::Discharging => now,
        ChargeState::Charging => full.checked_sub(now)?,
        _ => return None,
    };
    hours_to_duration(remaining, power)
}

fn time_from_charge(batteries: &[RawBattery], state: ChargeState) -> Option<Duration> {
    let current = sum_all(batteries, |b| b.current_now)?;
    let now = sum_all(batteries, |b| b.charge_now)?;
    let full = sum_all(batteries, |b| b.charge_full)?;
    let remaining = match state {
        ChargeState::Discharging => now,
        ChargeState::Charging => full.checked_sub(now)?,
        _ => return None,
    };
    hours_to_duration(remaining, current)
}

/// Largest time-remaining estimate trusted enough to show. A near-zero
/// `power_now`/`current_now` (a battery idling at the top of a charge curve,
/// or a bogus reading) divides out to hundreds of hours, which is not a
/// battery estimate a lock screen should ever print.
const MAX_PLAUSIBLE_HOURS: f64 = 48.0;

/// `remaining / rate` hours, as a [`Duration`] — `None` when `rate` is zero
/// or missing (nothing to divide by) or the result is implausible.
fn hours_to_duration(remaining: u64, rate: u64) -> Option<Duration> {
    if rate == 0 {
        return None;
    }
    let hours = remaining as f64 / rate as f64;
    if !hours.is_finite() || hours > MAX_PLAUSIBLE_HOURS {
        return None;
    }
    Some(Duration::from_secs_f64(hours * 3600.0))
}

// ---------------------------------------------------------------------------
// Quick on/off battery check
// ---------------------------------------------------------------------------

/// Quick "is this machine currently running off battery" check: `true` iff a
/// **system** battery ([`read`]'s rules: `type == Battery`, `scope !=
/// Device`) reports [`ChargeState::Discharging`] and no AC/USB supply is
/// online.
///
/// # The bug this replaced
///
/// Through 1.1.44 this was a straight port of the `fn on_battery()` that used
/// to live in `daemon::mod` (~line 1529), and it was **bug-for-bug identical**
/// to that original rather than a fixed-up version: it did not restrict
/// itself to `type == Battery` or exclude `scope == Device`, so it could
/// answer `true` purely because a Bluetooth/USB peripheral (mouse, keyboard,
/// headset) exposed a `status` file that happened to read `Discharging` — the
/// ordinary state for any battery-powered peripheral that isn't on its
/// charging dock. `daemon::mod`'s `pause_on_battery` was (and is) the one
/// caller, so the user-visible shape of that bug was a live wallpaper on a
/// plugged-in desktop pausing itself because a wireless mouse's battery is
/// discharging, which it almost always is. Fixed here by routing through
/// [`read`]'s already-correct filter instead of re-deriving a looser one —
/// [`read_from`]'s doc comment explains the `type`/`scope` rule this now
/// shares. See the CHANGELOG for the release this landed in.
///
/// Every caller — including `daemon::mod`'s `pause_on_battery` check — goes
/// through this one function, so there is exactly one place that can get the
/// peripheral case wrong again.
///
/// The answer is reused for [`ON_BATTERY_TTL`]: the lock-screen live-video
/// policy (`LockRuntime::forces_pause`) asks this on every run-loop pass, and
/// each uncached call is a full `/sys/class/power_supply` scan — tens of
/// `read(2)`s. Spinning at ~1000 passes a second that was 29,000 reads a second
/// and a core's worth of kernel time while the screen sat locked. A plug or
/// unplug is seen within the TTL, which is far inside every caller's own poll
/// period.
pub fn on_battery() -> bool {
    static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    cached(&mut last, Instant::now(), || on_battery_from(read()))
}

/// How long [`on_battery`] reuses its last answer.
const ON_BATTERY_TTL: Duration = Duration::from_secs(2);

/// [`on_battery`]'s TTL rule with the clock and the sysfs scan handed in.
fn cached(last: &mut Option<(Instant, bool)>, now: Instant, scan: impl FnOnce() -> bool) -> bool {
    match *last {
        Some((at, answer)) if now.saturating_duration_since(at) < ON_BATTERY_TTL => answer,
        _ => {
            let answer = scan();
            *last = Some((now, answer));
            answer
        }
    }
}

/// [`on_battery`]'s formula, pulled out as a pure function of a
/// [`BatteryStatus`] (or its absence) — the seam the tests drive with
/// [`read_from`]'s fixtures instead of duplicating this rule inline.
fn on_battery_from(status: Option<BatteryStatus>) -> bool {
    status.is_some_and(|b| b.state == ChargeState::Discharging && !b.on_ac)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // -- fixture sysfs trees -------------------------------------------------

    /// One sysfs file: (name, content).
    type Kv<'a> = (&'a str, &'a str);
    /// One power-supply directory: (name, files).
    type Supply<'a> = (&'a str, &'a [Kv<'a>]);

    fn fixture_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fresco-battery-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn supply(root: &Path, name: &str, files: &[Kv]) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        for (fname, content) in files {
            std::fs::write(dir.join(fname), content).unwrap();
        }
    }

    // -- read_from: the shapes real machines produce -------------------------

    #[test]
    fn laptop_discharging() {
        let root = fixture_root("laptop-discharging");
        supply(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "55"),
                ("energy_now", "3000000"),
                ("energy_full", "6000000"),
                ("power_now", "1500000"),
            ],
        );
        supply(&root, "AC", &[("type", "Mains"), ("online", "0")]);

        let got = read_from(&root).expect("a system battery is present");
        // The energy ratio (50%) wins over the coarser `capacity` (55).
        assert_eq!(got.percent, 50);
        assert_eq!(got.state, ChargeState::Discharging);
        assert!(!got.on_ac);
        assert_eq!(got.time_remaining, Some(Duration::from_secs(2 * 3600)));
    }

    #[test]
    fn laptop_charging() {
        let root = fixture_root("laptop-charging");
        supply(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Charging"),
                ("energy_now", "3000000"),
                ("energy_full", "6000000"),
                ("power_now", "1000000"),
            ],
        );
        supply(&root, "AC", &[("type", "Mains"), ("online", "1")]);

        let got = read_from(&root).unwrap();
        assert_eq!(got.percent, 50);
        assert_eq!(got.state, ChargeState::Charging);
        assert!(got.on_ac);
        // (full - now) / power = 3,000,000 / 1,000,000 = 3h to full.
        assert_eq!(got.time_remaining, Some(Duration::from_secs(3 * 3600)));
    }

    #[test]
    fn laptop_full() {
        let root = fixture_root("laptop-full");
        supply(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Full"),
                ("energy_now", "6000000"),
                ("energy_full", "6000000"),
            ],
        );
        supply(&root, "AC", &[("type", "Mains"), ("online", "1")]);

        let got = read_from(&root).unwrap();
        assert_eq!(got.percent, 100);
        assert_eq!(got.state, ChargeState::Full);
        assert!(got.on_ac);
        // Full has nothing left to count down, whatever the energy fields say.
        assert_eq!(got.time_remaining, None);
    }

    #[test]
    fn desktop_with_no_battery_is_none() {
        let root = fixture_root("desktop-no-battery");
        supply(&root, "AC", &[("type", "Mains"), ("online", "1")]);
        assert_eq!(read_from(&root), None);
    }

    #[test]
    fn peripheral_only_battery_is_none() {
        let root = fixture_root("peripheral-only");
        supply(
            &root,
            "hidpp_battery_0",
            &[
                ("type", "Battery"),
                ("scope", "Device"),
                ("status", "Discharging"),
                ("capacity", "80"),
            ],
        );
        assert_eq!(read_from(&root), None);
    }

    #[test]
    fn scope_device_is_skipped_scope_system_and_absent_are_kept() {
        let root = fixture_root("scope-handling");
        supply(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("scope", "System"),
                ("status", "Discharging"),
                ("capacity", "40"),
            ],
        );
        supply(
            &root,
            "mouse",
            &[
                ("type", "Battery"),
                ("scope", "Device"),
                ("status", "Discharging"),
                ("capacity", "90"),
            ],
        );
        let got = read_from(&root).unwrap();
        // Only BAT0 (scope: System) counts; the mouse's 90% must not leak in.
        assert_eq!(got.percent, 40);
    }

    #[test]
    fn two_batteries_aggregate_energy_and_power() {
        let root = fixture_root("two-batteries");
        supply(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("energy_now", "2000000"),
                ("energy_full", "4000000"),
                ("power_now", "600000"),
            ],
        );
        supply(
            &root,
            "BAT1",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("energy_now", "1000000"),
                ("energy_full", "2000000"),
                ("power_now", "400000"),
            ],
        );

        let got = read_from(&root).unwrap();
        // (2,000,000+1,000,000) / (4,000,000+2,000,000) = 50%.
        assert_eq!(got.percent, 50);
        assert_eq!(got.state, ChargeState::Discharging);
        // 3,000,000 / 1,000,000 = 3h.
        assert_eq!(got.time_remaining, Some(Duration::from_secs(3 * 3600)));
    }

    #[test]
    fn missing_energy_files_falls_back_to_capacity_and_drops_time_remaining() {
        let root = fixture_root("missing-energy-files");
        supply(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "42"),
            ],
        );
        let got = read_from(&root).unwrap();
        assert_eq!(got.percent, 42);
        assert_eq!(got.time_remaining, None);
    }

    #[test]
    fn garbage_values_fall_back_instead_of_breaking() {
        let root = fixture_root("garbage-values");
        supply(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Zorbing"), // not a real status
                ("capacity", "70"),
                ("energy_now", "not-a-number"), // breaks the energy ratio…
                ("energy_full", "6000000"),
            ],
        );
        let got = read_from(&root).unwrap();
        assert_eq!(got.state, ChargeState::Unknown);
        assert_eq!(got.percent, 70); // …so it falls back to `capacity`.
        assert_eq!(got.time_remaining, None);
    }

    #[test]
    fn capacity_over_100_is_clamped_not_rejected() {
        let root = fixture_root("capacity-over-100");
        supply(
            &root,
            "BAT0",
            &[("type", "Battery"), ("status", "Full"), ("capacity", "150")],
        );
        assert_eq!(read_from(&root).unwrap().percent, 100);
    }

    #[test]
    fn on_battery_scans_sysfs_once_per_ttl() {
        let t0 = Instant::now();
        let mut last = None;
        let mut scans = 0;
        let mut ask = |at: Instant, truth: bool| {
            cached(&mut last, at, || {
                scans += 1;
                truth
            })
        };
        assert!(ask(t0, true));
        // A run loop spinning on the answer reuses it ...
        for ms in [1, 10, 500, 1999] {
            assert!(
                ask(t0 + Duration::from_millis(ms), false),
                "stale at {ms}ms"
            );
        }
        // ... until the TTL lapses, then it looks again.
        assert!(!ask(t0 + ON_BATTERY_TTL, false));
        assert_eq!(scans, 2);
    }

    #[test]
    fn real_queries_do_not_panic() {
        // Whatever this machine actually is, these must return *something*
        // sane rather than panicking — that is the whole point of returning
        // Option/bool instead of asserting a battery exists.
        let _ = read();
        let _ = on_battery();
    }

    // -- on_battery: the peripheral-battery regression ------------------------

    /// Regression test for the bug [`on_battery`]'s doc comment documents:
    /// until 1.1.44 this function was a bug-for-bug port of the daemon's
    /// original, unfiltered sysfs scan, so a wireless mouse/keyboard/headset
    /// reporting `Discharging` — which it does almost all the time — made
    /// `on_battery()` answer `true` on a machine sitting on mains power. The
    /// case tagged `peripheral-only-discharging` is the one that must have
    /// flipped from `true` (the old, buggy answer) to `false` (correct) when
    /// this was fixed; every other case pins the ordinary, non-peripheral
    /// behaviour so a future change here cannot silently reintroduce the bug
    /// from the other direction either.
    #[test]
    fn on_battery_ignores_peripheral_batteries() {
        let cases: [(&str, &[Supply], bool); 7] = [
            (
                "laptop-discharging",
                &[(
                    "BAT0",
                    &[
                        ("type", "Battery"),
                        ("status", "Discharging"),
                        ("capacity", "55"),
                    ],
                )],
                true,
            ),
            (
                "laptop-charging",
                &[(
                    "BAT0",
                    &[
                        ("type", "Battery"),
                        ("status", "Charging"),
                        ("capacity", "55"),
                    ],
                )],
                false,
            ),
            (
                "ac-only-no-status-file",
                &[("AC", &[("type", "Mains"), ("online", "1")])],
                false,
            ),
            (
                // THE regression case: a Bluetooth/USB peripheral's own
                // battery must never make the machine look like it's on
                // battery. `read`/`read_from` already exclude `scope:
                // Device`; `on_battery` must inherit that, not re-derive a
                // looser rule of its own.
                "peripheral-only-discharging",
                &[(
                    "hidpp_battery_0",
                    &[
                        ("type", "Battery"),
                        ("scope", "Device"),
                        ("status", "Discharging"),
                        ("capacity", "80"),
                    ],
                )],
                false,
            ),
            (
                // A discharging peripheral alongside a discharging system
                // battery: still `true`, and for the right reason — the
                // system battery, not the peripheral, which `read_from`'s
                // own `scope_device_is_skipped_scope_system_and_absent_are_kept`
                // test already pins on the `percent` side of this same fixture
                // shape.
                "system-and-peripheral-both-discharging",
                &[
                    (
                        "BAT0",
                        &[
                            ("type", "Battery"),
                            ("status", "Discharging"),
                            ("capacity", "40"),
                        ],
                    ),
                    (
                        "mouse",
                        &[
                            ("type", "Battery"),
                            ("scope", "Device"),
                            ("status", "Discharging"),
                            ("capacity", "80"),
                        ],
                    ),
                ],
                true,
            ),
            (
                // On AC with the system battery still reporting a stale
                // `Discharging` (a brief transitional read right after
                // plugging in) — `on_ac` wins, matching the doc'd formula
                // "Discharging AND NOT on_ac", not "Discharging" alone.
                "discharging_status_but_on_ac",
                &[
                    (
                        "BAT0",
                        &[
                            ("type", "Battery"),
                            ("status", "Discharging"),
                            ("capacity", "60"),
                        ],
                    ),
                    ("AC", &[("type", "Mains"), ("online", "1")]),
                ],
                false,
            ),
            (
                "garbage-status",
                &[(
                    "BAT0",
                    &[
                        ("type", "Battery"),
                        ("status", "banana"),
                        ("capacity", "60"),
                    ],
                )],
                false,
            ),
        ];
        for (tag, entries, want) in cases {
            let root = fixture_root(&format!("onbat-{tag}"));
            for (name, files) in entries {
                supply(&root, name, files);
            }
            assert_eq!(on_battery_from(read_from(&root)), want, "{tag}");
        }

        // An empty (but existing) power_supply directory, and a path that
        // does not exist at all, must both answer `false` rather than panic.
        let empty = fixture_root("onbat-empty-dir");
        assert!(!on_battery_from(read_from(&empty)));
        let missing = fixture_root("onbat-missing-parent").join("does-not-exist");
        assert!(!on_battery_from(read_from(&missing)));
    }

    // -- pure helpers, exercised directly -------------------------------------

    #[test]
    fn charge_state_parse_is_total_and_defaults_to_unknown() {
        assert_eq!(ChargeState::parse("Charging"), ChargeState::Charging);
        assert_eq!(ChargeState::parse("Discharging"), ChargeState::Discharging);
        assert_eq!(ChargeState::parse("Full"), ChargeState::Full);
        assert_eq!(ChargeState::parse("Not charging"), ChargeState::NotCharging);
        assert_eq!(ChargeState::parse("  Full  "), ChargeState::Full);
        assert_eq!(ChargeState::parse(""), ChargeState::Unknown);
        assert_eq!(ChargeState::parse("banana"), ChargeState::Unknown);
    }

    #[test]
    fn is_ac_type_matches_mains_and_every_usb_subtype() {
        for k in [
            "Mains",
            "USB",
            "USB_C",
            "USB_PD",
            "USB_PD_DRP",
            "USB_DCP",
            "USB_CDP",
        ] {
            assert!(is_ac_type(k), "{k}");
        }
        for k in ["Battery", "UPS", "Wireless", ""] {
            assert!(!is_ac_type(k), "{k}");
        }
    }

    #[test]
    fn percent_from_ratio_clamps_and_rejects_zero_denominator() {
        assert_eq!(percent_from_ratio(50, 100), Some(50));
        assert_eq!(percent_from_ratio(0, 100), Some(0));
        assert_eq!(percent_from_ratio(100, 100), Some(100));
        assert_eq!(percent_from_ratio(150, 100), Some(100)); // clamped, not a panic
        assert_eq!(percent_from_ratio(1, 0), None);
    }

    #[test]
    fn hours_to_duration_rejects_zero_rate_and_absurd_results() {
        assert_eq!(hours_to_duration(0, 0), None);
        assert_eq!(hours_to_duration(0, 100), Some(Duration::ZERO));
        assert_eq!(hours_to_duration(100, 100), Some(Duration::from_secs(3600)));
        assert_eq!(hours_to_duration(1000, 1), None); // 1000h is not a real estimate
        assert_eq!(hours_to_duration(u64::MAX, 1), None);
    }

    #[test]
    fn aggregate_state_prioritises_discharging_over_everything() {
        fn raw(state: ChargeState) -> RawBattery {
            RawBattery {
                state,
                capacity: None,
                energy_now: None,
                energy_full: None,
                power_now: None,
                charge_now: None,
                charge_full: None,
                current_now: None,
            }
        }
        assert_eq!(
            aggregate_state(&[raw(ChargeState::Full), raw(ChargeState::Discharging)]),
            ChargeState::Discharging
        );
        assert_eq!(
            aggregate_state(&[raw(ChargeState::Full), raw(ChargeState::Charging)]),
            ChargeState::Charging
        );
        assert_eq!(
            aggregate_state(&[raw(ChargeState::Full)]),
            ChargeState::Full
        );
        assert_eq!(
            aggregate_state(&[raw(ChargeState::Unknown)]),
            ChargeState::Unknown
        );
        assert_eq!(aggregate_state(&[]), ChargeState::Unknown);
    }
}
