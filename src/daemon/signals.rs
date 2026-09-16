//! SIGTERM, SIGINT and SIGHUP become the daemon's own clean `Stop`.
//!
//! # Why
//!
//! A logout, `pkill frescod`, `systemctl --user stop` and a terminal's Ctrl+C
//! all end the daemon with a signal, and until this module the default action
//! for every one of them killed the process on the spot. That skipped
//! `shutdown()` — the one path that puts the user's desktop back: Fresco's
//! still frame on GNOME and Cinnamon, the transparent wallpaper on Deepin, and
//! on MATE the key colour the icon mirror has Caja paint. Measured on Linux
//! Mint 22 MATE: after a SIGTERM the desktop stayed on the key colour.
//!
//! # How
//!
//! A handler for each signal writes the signal number into a pipe — `write`
//! is one of the few calls that is safe inside a handler — and does nothing
//! else. A thread blocked reading that pipe turns the first signal into an
//! ordinary `Stop` request over the control socket, so each backend shuts down
//! through exactly the path the app's Stop uses: there is no second shutdown
//! sequence to keep in step with the first.
//!
//! Handlers rather than a blocked signal mask and `sigwait`, deliberately.
//! `std::process::Command` passes the parent's signal mask to every child
//! unchanged, so blocking SIGTERM here would leave mpvpaper, ffmpeg and
//! gsettings deaf to it too. A handler is reset to the default in the child by
//! `exec`, so the children behave exactly as before.
//!
//! Before asking for the `Stop`, the thread puts the default disposition back,
//! so a second signal ends the process — Ctrl+C twice still gets out of a
//! shutdown that has hung.

use std::io::Read;
use std::os::fd::{IntoRawFd, OwnedFd};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use nix::fcntl::OFlag;
use nix::sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet, Signal};

use crate::ipc::{self, Request};

/// The signals turned into a clean stop.
const SIGNALS: [Signal; 3] = [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP];

/// How long to keep asking a daemon that is still starting up to stop.
const STOP_ATTEMPTS: u32 = 40;
const STOP_RETRY: Duration = Duration::from_millis(250);

/// Write end of the pipe, for the handler. `-1` until installed.
static WAKE_FD: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(sig: nix::libc::c_int) {
    let fd = WAKE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = sig as u8;
        // SAFETY: `write` is async-signal-safe; `fd` is the pipe's write end,
        // which is never closed once installed.
        unsafe {
            nix::libc::write(fd, (&byte as *const u8).cast(), 1);
        }
    }
}

/// Route termination signals to a clean `Stop`. Call it once, early.
pub fn install() {
    let (read_end, write_end) = match nix::unistd::pipe2(OFlag::O_CLOEXEC) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("signals: no pipe ({e}); a SIGTERM will skip the clean shutdown");
            return;
        }
    };
    let spawned = std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || wait_and_stop(read_end));
    if let Err(e) = spawned {
        log::warn!(
            "signals: could not start the signal thread ({e}); keeping the default handling"
        );
        return;
    }
    // The handler needs a plain descriptor it can reach from any context; the
    // write end is deliberately never closed.
    WAKE_FD.store(write_end.into_raw_fd(), Ordering::Relaxed);
    let action = SigAction::new(
        SigHandler::Handler(on_signal),
        SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    for sig in SIGNALS {
        // SAFETY: the handler only performs an async-signal-safe `write`.
        if let Err(e) = unsafe { signal::sigaction(sig, &action) } {
            log::warn!("signals: could not handle {} ({e})", sig.as_str());
        }
    }
}

fn wait_and_stop(read_end: OwnedFd) {
    let mut pipe = std::fs::File::from(read_end);
    let mut byte = [0u8; 1];
    if pipe.read_exact(&mut byte).is_err() {
        return;
    }
    let name = Signal::try_from(i32::from(byte[0]))
        .map(|s| s.as_str())
        .unwrap_or("signal");
    // A second signal now takes its default action and ends the process —
    // the way out of a shutdown that never finishes.
    for sig in SIGNALS {
        // SAFETY: restoring the default disposition has no preconditions.
        let _ = unsafe { signal::signal(sig, SigHandler::SigDfl) };
    }
    log::info!("{name} received — stopping cleanly");
    for _ in 0..STOP_ATTEMPTS {
        if ipc::request(&Request::Stop).is_ok() {
            return;
        }
        // The control socket may not be up yet if the signal arrived during
        // startup; nothing has been changed on the desktop that early either.
        std::thread::sleep(STOP_RETRY);
    }
    log::warn!("{name}: the daemon never answered a Stop; exiting without cleanup");
    std::process::exit(128 + i32::from(byte[0]));
}
