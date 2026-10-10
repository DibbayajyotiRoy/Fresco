//! Xfce: the key-colour backdrop, over xfconf.
//!
//! xfdesktop reads its backdrop from the `xfce4-desktop` channel, one
//! `/backdrop/screen<N>/monitor<NAME>/workspace<M>/…` group per monitor and
//! workspace. The mirror needs every pixel of the desktop that is not an icon
//! to be the key colour, so three keys are set in each group:
//!
//! * `image-style` = 0 (none): no picture;
//! * `color-style` = 0 (solid): no gradient, no transparency (transparent
//!   renders black, which is not the key);
//! * `rgba1` = the key, four doubles.
//!
//! Besides every group already in the channel, `workspace0` of each connected
//! monitor is created if absent — by connector name and by index, since which
//! one xfdesktop reads depends on its version — so a desktop that has never
//! had its backdrop configured is keyed too.
//!
//! The user's own values are saved first, one `path<TAB>value` line per key
//! (`ABSENT` for a key that did not exist, which is then removed again instead
//! of being left behind with a made-up value). [`restore`] puts them back — at
//! Stop, and at the next start after a crash.
//!
//! Everything `xfconf-query` prints, and everything read back from the state
//! file, is untrusted: a path must have the exact shape of a backdrop key, a
//! value must parse as the number it should be, and commands are run with an
//! argument vector, never through a shell.

use std::path::Path;
use std::process::{Command, Stdio};

use super::{KEY, KEY_HEX};

/// The xfconf channel xfdesktop keeps its settings in.
const CHANNEL: &str = "xfce4-desktop";
/// Where the user's values are kept while the key colour is set.
const STATE_FILE: &str = "xfce-mirror-background";
/// State-file stand-in for a key that did not exist.
const ABSENT: &str = "ABSENT";
/// The keys set in every group.
const KEYS: [&str; 3] = ["image-style", "color-style", "rgba1"];

/// Runs `xfconf-query -c xfce4-desktop <args>`: its stdout when it exits 0.
type Run<'a> = &'a dyn Fn(&[&str]) -> Option<String>;

/// What the key looks like to xfconf, in state-file form: `0` for the two
/// styles, `r,g,b,a` as doubles for `rgba1`.
fn key_value(path: &str) -> String {
    if path.ends_with("/rgba1") {
        let c = f64::from(KEY[0]) / 255.0;
        format!("{c},{c},{c},1")
    } else {
        "0".into()
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Whether `path` is exactly one of the keys this module changes:
/// `/backdrop/screen<N>/monitor<NAME>/workspace<M>/<key>`.
fn is_backdrop_key(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let [empty, backdrop, screen, monitor, workspace, key] = parts[..] else {
        return false;
    };
    empty.is_empty()
        && backdrop == "backdrop"
        && screen.strip_prefix("screen").is_some_and(is_digits)
        && monitor
            .strip_prefix("monitor")
            .is_some_and(|m| !m.is_empty() && !m.chars().any(char::is_control))
        && workspace.strip_prefix("workspace").is_some_and(is_digits)
        && KEYS.contains(&key)
}

/// The backdrop keys in `xfconf-query -l`'s output (one path per line).
fn parse_list(out: &str) -> Vec<String> {
    out.lines()
        .filter(|l| is_backdrop_key(l))
        .map(String::from)
        .collect()
}

/// `xfconf-query -p <path>`'s output as the state file stores it: an integer
/// for the styles, four comma-joined doubles for `rgba1`. `None` when it is
/// not that.
///
/// An array prints as a header line (translated, which is why `LC_ALL=C` is
/// set and the header is not matched), a blank line, then one number per line;
/// any line that is not a number is skipped, so exactly four finite ones must remain.
fn parse_value(path: &str, out: &str) -> Option<String> {
    if path.ends_with("/rgba1") {
        let nums: Vec<f64> = out
            .lines()
            .filter_map(|l| l.trim().parse::<f64>().ok())
            .collect();
        (nums.len() == 4 && nums.iter().all(|n| n.is_finite())).then(|| {
            nums.iter()
                .map(f64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        })
    } else {
        out.trim().parse::<i32>().ok().map(|n| n.to_string())
    }
}

/// The arguments that make `xfconf-query` write `value` (state-file form) to
/// `path`, creating the key if need be. `None` when either is not something
/// this module writes — the arguments are rebuilt from the parsed numbers, so
/// nothing but numbers reaches the command line.
fn set_args(path: &str, value: &str) -> Option<Vec<String>> {
    if !is_backdrop_key(path) {
        return None;
    }
    let mut args: Vec<String> = ["-p", path, "-n"].map(String::from).into();
    if path.ends_with("/rgba1") {
        let nums: Vec<f64> = value
            .split(',')
            .map(|v| v.parse::<f64>().ok().filter(|n| n.is_finite()))
            .collect::<Option<_>>()?;
        if nums.len() != 4 {
            return None;
        }
        for n in nums {
            args.extend(["-t", "double", "-s"].map(String::from));
            args.push(n.to_string());
        }
    } else {
        let n: i32 = value.parse().ok()?;
        args.extend(["-t", "int", "-s"].map(String::from));
        args.push(n.to_string());
    }
    Some(args)
}

/// The groups to key for `monitors` (connector names): `monitor<name>` and
/// `monitor<index>` for each.
fn monitor_groups(monitors: &[String]) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for id in monitors
        .iter()
        .cloned()
        .chain((0..monitors.len()).map(|i| i.to_string()))
    {
        let group = format!("monitor{id}");
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    groups
}

type Saved = Vec<(String, Option<String>)>;

fn read_state(state: &Path) -> Saved {
    let Ok(text) = std::fs::read_to_string(state) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let (path, value) = l.split_once('\t')?;
            is_backdrop_key(path).then(|| {
                (
                    path.to_string(),
                    (value != ABSENT).then(|| value.to_string()),
                )
            })
        })
        .collect()
}

fn write_state(state: &Path, saved: &Saved) -> bool {
    let text: String = saved
        .iter()
        .map(|(p, v)| format!("{p}\t{}\n", v.as_deref().unwrap_or(ABSENT)))
        .collect();
    if let Some(dir) = state.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(state, text).is_ok()
}

fn call(run: Run, args: &[String]) -> Option<String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&args)
}

/// Save the user's backdrop, then set the key colour on every group.
/// False when it cannot be done (nothing is left changed then).
fn apply_with(run: Run, state: &Path, monitors: &[String]) -> bool {
    let listed = parse_list(&run(&["-l"]).unwrap_or_default());
    let mut targets = listed.clone();
    for group in monitor_groups(monitors) {
        for key in KEYS {
            let path = format!("/backdrop/screen0/{group}/workspace0/{key}");
            if is_backdrop_key(&path) && !targets.contains(&path) {
                targets.push(path);
            }
        }
    }
    // A key already in the state file keeps what that run saved: it is the
    // user's, where the channel now holds our key (a crash left it there).
    let mut saved = read_state(state);
    for path in &targets {
        if saved.iter().any(|(p, _)| p == path) {
            continue;
        }
        let original = if listed.contains(path) {
            let Some(out) = run(&["-p", path]) else {
                return false;
            };
            let Some(value) = parse_value(path, &out) else {
                log::warn!("Xfce: cannot save {path}; its value is not what xfdesktop writes");
                return false;
            };
            Some(value)
        } else {
            None
        };
        saved.push((path.clone(), original));
    }
    if !write_state(state, &saved) {
        return false;
    }
    for path in &targets {
        let ok = set_args(path, &key_value(path)).is_some_and(|a| call(run, &a).is_some());
        if !ok {
            log::warn!("Xfce: cannot set {path}; leaving the backdrop as it was");
            restore_with(run, state);
            return false;
        }
    }
    log::info!("Xfce: desktop backdrop set to the icon key colour {KEY_HEX}");
    true
}

/// Put the saved values back and forget them. The state file stays when a
/// command failed, so the next start tries again.
fn restore_with(run: Run, state: &Path) {
    if !state.exists() {
        return;
    }
    let saved = read_state(state);
    let listed = parse_list(&run(&["-l"]).unwrap_or_default());
    let mut all_ok = true;
    for (path, value) in &saved {
        let ok = match value {
            // Gone already (or never created, if the run died first): fine.
            None if !listed.contains(path) => true,
            None => run(&["-p", path, "-r"]).is_some(),
            Some(v) => match set_args(path, v) {
                Some(args) => call(run, &args).is_some(),
                None => {
                    log::warn!("Xfce: ignoring an unreadable saved value for {path}");
                    true
                }
            },
        };
        all_ok &= ok;
    }
    if all_ok {
        std::fs::remove_file(state).ok();
        log::info!("Xfce: desktop backdrop restored");
    } else {
        log::warn!("Xfce: could not restore the desktop backdrop; will retry at the next start");
    }
}

fn xfconf_query(args: &[&str]) -> Option<String> {
    let out = Command::new("xfconf-query")
        // Untranslated output with a '.' decimal point, both ways.
        .env("LC_ALL", "C")
        .args(["-c", CHANNEL])
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn state_file() -> std::path::PathBuf {
    super::bg_state_file().with_file_name(STATE_FILE)
}

/// Save the user's backdrop and switch every group to the key colour.
/// `monitors` are the connector names.
pub(super) fn apply(monitors: &[String]) -> bool {
    apply_with(&xfconf_query, &state_file(), monitors)
}

/// Undo [`apply`]. A no-op, without running anything, when nothing was saved.
pub(super) fn restore() {
    restore_with(&xfconf_query, &state_file());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    /// An in-memory xfce4-desktop channel that speaks `xfconf-query`'s
    /// argument and output formats (`-l`, `-p`, `-p -r`, `-p -n -t -s`).
    #[derive(Default)]
    struct Fake {
        props: RefCell<BTreeMap<String, Vec<String>>>,
        /// Fail every write whose first value is this (the key colour's).
        fail_value: Option<&'static str>,
        calls: RefCell<Vec<String>>,
    }

    impl Fake {
        fn with(props: &[(&str, &[&str])]) -> Fake {
            let f = Fake::default();
            for (p, v) in props {
                f.props
                    .borrow_mut()
                    .insert(p.to_string(), v.iter().map(|s| s.to_string()).collect());
            }
            f
        }

        fn run(&self, args: &[&str]) -> Option<String> {
            self.calls.borrow_mut().push(args.join(" "));
            let mut props = self.props.borrow_mut();
            if args == ["-l"] {
                return Some(props.keys().map(|k| format!("{k}\n")).collect());
            }
            let path = args.get(1).filter(|_| args[0] == "-p")?.to_string();
            if args.len() == 2 {
                // A query: one number for a scalar, the array form otherwise.
                let v = props.get(&path)?;
                return Some(if path.ends_with("/rgba1") {
                    let items: String = v.iter().map(|n| format!("{n}\n")).collect();
                    format!("Value is an array with {} items:\n\n{items}", v.len())
                } else {
                    format!("{}\n", v[0])
                });
            }
            if args[2] == "-r" {
                return props.remove(&path).map(|_| String::new());
            }
            assert_eq!(args[2], "-n", "writes create the key: {args:?}");
            let values: Vec<String> = args[3..]
                .chunks(4)
                .map(|c| {
                    assert!(c[0] == "-t" && (c[1] == "int" || c[1] == "double") && c[2] == "-s");
                    c[3].to_string()
                })
                .collect();
            if self.fail_value.is_some_and(|v| values[0] == v) {
                return None;
            }
            props.insert(path, values);
            Some(String::new())
        }

        fn snapshot(&self) -> BTreeMap<String, Vec<String>> {
            self.props.borrow().clone()
        }
    }

    const G: &str = "/backdrop/screen0/monitorHDMI-1/workspace0";
    /// The key's channel, as `key_value` writes it.
    const K: &str = "0.00392156862745098";

    fn user_backdrop() -> Fake {
        Fake::with(&[
            (&format!("{G}/image-style"), &["5"]),
            (&format!("{G}/color-style"), &["1"]),
            (
                &format!("{G}/rgba1"),
                &["0.100000", "0.200000", "0.300000", "1.000000"],
            ),
            // Not ours to touch.
            (&format!("{G}/last-image"), &["/home/u/a.jpg"]),
            ("/backdrop/single-workspace-mode", &["1"]),
        ])
    }

    fn state_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fresco-xfconf-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(STATE_FILE)
    }

    #[test]
    fn only_exact_backdrop_keys_are_accepted() {
        for ok in [
            "/backdrop/screen0/monitor0/workspace0/rgba1",
            "/backdrop/screen0/monitorHDMI-1/workspace3/image-style",
            "/backdrop/screen12/monitoreDP-1/workspace0/color-style",
        ] {
            assert!(is_backdrop_key(ok), "{ok}");
        }
        for bad in [
            "",
            "/backdrop/screen0/monitor0/workspace0/last-image",
            "/backdrop/screen0/monitor/workspace0/rgba1",
            "/backdrop/screen/monitor0/workspace0/rgba1",
            "/backdrop/screen0/monitor0/workspace/rgba1",
            "/backdrop/screen0/monitor0/workspace0/rgba1/extra",
            "/backdrop/screen0/monitor0/rgba1",
            "backdrop/screen0/monitor0/workspace0/rgba1",
            "/other/screen0/monitor0/workspace0/rgba1",
            "/backdrop/screen0/monitor0\t/workspace0/rgba1",
            "/backdrop/screen0/monitor0\n/workspace0/rgba1",
            "-p /backdrop/screen0/monitor0/workspace0/rgba1",
        ] {
            assert!(!is_backdrop_key(bad), "{bad:?}");
        }
        let listing = "/backdrop/screen0/monitor0/workspace0/rgba1\n/desktop-icons/style\n\
                       /backdrop/screen0/monitor0/workspace0/last-image\n";
        assert_eq!(
            parse_list(listing),
            vec!["/backdrop/screen0/monitor0/workspace0/rgba1"]
        );
    }

    #[test]
    fn values_parse_to_numbers_or_not_at_all() {
        let rgba = "/backdrop/screen0/monitor0/workspace0/rgba1";
        let style = "/backdrop/screen0/monitor0/workspace0/color-style";
        // The array form, in English and in a language that is not.
        let en = "Value is an array with 4 items:\n\n0.100000\n0.200000\n0.300000\n1.000000\n";
        let de = "Wert ist ein Feld mit 4 Elementen:\n\n0.100000\n0.200000\n0.300000\n1.000000\n";
        for out in [en, de] {
            assert_eq!(parse_value(rgba, out).as_deref(), Some("0.1,0.2,0.3,1"));
        }
        assert_eq!(parse_value(rgba, "0.5\n"), None, "a scalar is not rgba1");
        assert_eq!(
            parse_value(rgba, "Value is an array with 3 items:\n\n1\n2\n3\n"),
            None
        );
        assert_eq!(parse_value(rgba, "1\n2\n3\n4\ninf\n"), None);
        assert_eq!(parse_value(style, "1\n").as_deref(), Some("1"));
        assert_eq!(parse_value(style, " 12 \n").as_deref(), Some("12"));
        assert_eq!(parse_value(style, "red\n"), None);
        assert_eq!(parse_value(style, "1; rm -rf ~\n"), None);
        assert_eq!(
            parse_value(style, "Value is an array with 2 items:\n\n1\n2\n"),
            None
        );
    }

    #[test]
    fn set_args_are_built_from_parsed_numbers_only() {
        let style = "/backdrop/screen0/monitor0/workspace0/image-style";
        let rgba = "/backdrop/screen0/monitor0/workspace0/rgba1";
        assert_eq!(
            set_args(style, "0").unwrap(),
            ["-p", style, "-n", "-t", "int", "-s", "0"]
        );
        assert_eq!(
            set_args(rgba, "0.1,0.2,0.3,1").unwrap(),
            [
                "-p", rgba, "-n", "-t", "double", "-s", "0.1", "-t", "double", "-s", "0.2", "-t",
                "double", "-s", "0.3", "-t", "double", "-s", "1"
            ]
        );
        for bad in ["", "x", "0 -r", "0;reboot", "1.5", "99999999999"] {
            assert_eq!(set_args(style, bad), None, "{bad:?}");
        }
        for bad in [
            "",
            "1,2,3",
            "1,2,3,4,5",
            "1,2,3,a",
            "1,2,3,4;reboot",
            "1,2,3,nan",
        ] {
            assert_eq!(set_args(rgba, bad), None, "{bad:?}");
        }
        assert_eq!(
            set_args("/backdrop/screen0/monitor0/workspace0/last-image", "0"),
            None
        );
        assert_eq!(set_args("-r", "0"), None);
    }

    #[test]
    fn the_key_survives_xfdesktop_conversion_to_8_bit() {
        let rgba = "/backdrop/screen0/monitor0/workspace0/rgba1";
        let value = key_value(rgba);
        let nums: Vec<f64> = value.split(',').map(|v| v.parse().unwrap()).collect();
        assert_eq!(nums.len(), 4);
        // Cairo: double -> 16-bit (x * 65535 + 0.5), then the top 8 bits. Also
        // through the six-decimal text xfconf may store it as.
        for n in &nums[..3] {
            for n in [*n, format!("{n:.6}").parse::<f64>().unwrap()] {
                let wide = (n * 65535.0 + 0.5) as u32;
                assert_eq!(wide >> 8, u32::from(KEY[0]), "{n}");
            }
        }
        assert_eq!(nums[3], 1.0);
        assert_eq!(KEY, [1, 1, 1]);
        assert_eq!(key_value("/x/image-style"), "0");
        assert_eq!(key_value("/x/color-style"), "0");
        assert_eq!(value, format!("{K},{K},{K},1"));
    }

    #[test]
    fn apply_then_restore_gives_the_user_their_backdrop_back() {
        let fake = user_backdrop();
        let before = fake.snapshot();
        let state = state_path("roundtrip");
        let monitors = vec!["HDMI-1".to_string(), "DP-2".to_string()];

        assert!(apply_with(&|a| fake.run(a), &state, &monitors));
        let during = fake.snapshot();
        let key =
            |g: &str, k: &str| during[&format!("/backdrop/screen0/{g}/workspace0/{k}")].clone();
        // The user's group, a connected monitor with no group, and its index twin.
        for g in ["monitorHDMI-1", "monitorDP-2", "monitor0", "monitor1"] {
            assert_eq!(key(g, "image-style"), ["0"], "{g}");
            assert_eq!(key(g, "color-style"), ["0"], "{g}");
            let rgba: Vec<f64> = key(g, "rgba1").iter().map(|n| n.parse().unwrap()).collect();
            assert_eq!(rgba.len(), 4, "{g}");
            assert!(rgba[..3].iter().all(|c| (c * 255.0).round() == 1.0), "{g}");
        }
        // What is not a backdrop style is untouched.
        assert_eq!(
            during[&format!("{G}/last-image")],
            before[&format!("{G}/last-image")]
        );
        let text = std::fs::read_to_string(&state).unwrap();
        assert!(text.contains(&format!("{G}/image-style\t5\n")), "{text}");
        assert!(
            text.contains(&format!("{G}/rgba1\t0.1,0.2,0.3,1\n")),
            "{text}"
        );
        assert!(
            text.contains("/backdrop/screen0/monitorDP-2/workspace0/color-style\tABSENT\n"),
            "{text}"
        );

        restore_with(&|a| fake.run(a), &state);
        let after = fake.snapshot();
        // The keys that existed hold their values (rgba1 as xfconf prints a
        // double, which is what the user had); the created ones are gone.
        assert_eq!(
            after.keys().collect::<Vec<_>>(),
            before.keys().collect::<Vec<_>>()
        );
        assert_eq!(after[&format!("{G}/image-style")], ["5"]);
        assert_eq!(after[&format!("{G}/color-style")], ["1"]);
        assert_eq!(after[&format!("{G}/rgba1")], ["0.1", "0.2", "0.3", "1"]);
        assert!(!state.exists());
        std::fs::remove_dir_all(state.parent().unwrap()).ok();
    }

    #[test]
    fn a_second_apply_keeps_the_first_runs_originals() {
        let fake = user_backdrop();
        let state = state_path("twice");
        assert!(apply_with(
            &|a| fake.run(a),
            &state,
            &["HDMI-1".to_string()]
        ));
        // A crashed run and a restart: the channel holds the key now.
        assert!(apply_with(
            &|a| fake.run(a),
            &state,
            &["HDMI-1".to_string()]
        ));
        restore_with(&|a| fake.run(a), &state);
        assert_eq!(fake.snapshot()[&format!("{G}/color-style")], ["1"]);
        assert_eq!(fake.snapshot()[&format!("{G}/image-style")], ["5"]);
        std::fs::remove_dir_all(state.parent().unwrap()).ok();
    }

    #[test]
    fn a_failed_write_leaves_the_backdrop_as_it_was() {
        let fake = Fake {
            fail_value: Some(K),
            ..user_backdrop()
        };
        let before = fake.snapshot();
        let state = state_path("failed");
        assert!(!apply_with(
            &|a| fake.run(a),
            &state,
            &["HDMI-1".to_string()]
        ));
        // The styles were keyed before rgba1 failed; the restore wrote back
        // what the user had, and the keys made for the second group are gone.
        let after = fake.snapshot();
        assert_eq!(
            after.keys().collect::<Vec<_>>(),
            before.keys().collect::<Vec<_>>()
        );
        assert_eq!(after[&format!("{G}/image-style")], ["5"]);
        assert_eq!(after[&format!("{G}/color-style")], ["1"]);
        assert_eq!(after[&format!("{G}/rgba1")], ["0.1", "0.2", "0.3", "1"]);
        assert!(!state.exists());
        std::fs::remove_dir_all(state.parent().unwrap()).ok();
    }

    #[test]
    fn a_value_that_cannot_be_saved_stops_before_anything_changes() {
        let fake = Fake::with(&[(
            "/backdrop/screen0/monitor0/workspace0/color-style",
            &["blue"],
        )]);
        let before = fake.snapshot();
        let state = state_path("unsaveable");
        assert!(!apply_with(&|a| fake.run(a), &state, &[]));
        assert_eq!(fake.snapshot(), before);
        assert!(!state.exists());
        std::fs::remove_dir_all(state.parent().unwrap()).ok();
    }

    #[test]
    fn restore_without_a_state_file_runs_nothing() {
        let fake = user_backdrop();
        let state = state_path("none");
        restore_with(&|a| fake.run(a), &state);
        assert!(fake.calls.borrow().is_empty());
        std::fs::remove_dir_all(state.parent().unwrap()).ok();
    }

    #[test]
    fn a_state_file_with_hostile_lines_restores_only_backdrop_keys() {
        let fake = user_backdrop();
        let before = fake.snapshot();
        let state = state_path("hostile");
        std::fs::write(
            &state,
            format!(
                "/backdrop/screen0/monitorHDMI-1/workspace0/last-image\t0\n\
                 -r\t0\n\
                 {G}/color-style\t0 -r\n\
                 {G}/image-style\t2\n\
                 no tab at all\n"
            ),
        )
        .unwrap();
        restore_with(&|a| fake.run(a), &state);
        let after = fake.snapshot();
        // Only the one well-formed line was applied.
        assert_eq!(after[&format!("{G}/image-style")], ["2"]);
        assert_eq!(
            after[&format!("{G}/color-style")],
            before[&format!("{G}/color-style")]
        );
        assert_eq!(
            after[&format!("{G}/last-image")],
            before[&format!("{G}/last-image")]
        );
        assert!(!state.exists());
        std::fs::remove_dir_all(state.parent().unwrap()).ok();
    }
}
