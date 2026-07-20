//! Live "now playing" status pill + pause/resume toggle, backed by
//! `ipc::request(&Request::Status)`.

use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{glib, glib::ControlFlow};

use crate::ipc::{self, Request, StatusReply};

/// How often to poll the daemon while the window is open. A live status
/// surface doesn't need sub-second freshness, and this keeps the background
/// thread churn low.
const POLL_INTERVAL_S: u32 = 4;

/// Widgets the polling loop updates in place.
struct PillWidgets {
    dot: gtk4::Label,
    /// Tiny "PLAYING" / "PAUSED" overline above the name.
    overline: gtk4::Label,
    /// Prettified wallpaper name (CPU% lives in the tooltip).
    label: gtk4::Label,
    hwdec: gtk4::Label,
    toggle: gtk4::Button,
    pill: gtk4::Box,
}

/// Build the status pill (dot + wallpaper name + hwdec badge + CPU% + a
/// pause/resume toggle) and start polling the daemon in the background.
/// Returns the root widget to place in the header.
pub fn build_status_pill() -> gtk4::Widget {
    let pill = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    pill.add_css_class("status-pill");

    let dot = gtk4::Label::new(Some("●"));
    dot.add_css_class("dot-off");
    pill.append(&dot);

    // Two stacked lines: a tiny state overline + the wallpaper name.
    let text_col = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    text_col.set_valign(gtk4::Align::Center);
    let overline = gtk4::Label::new(None);
    overline.add_css_class("pill-overline");
    overline.set_xalign(0.0);
    overline.set_visible(false);
    text_col.append(&overline);
    let label = gtk4::Label::new(Some("Not running"));
    label.add_css_class("pill-name");
    label.set_xalign(0.0);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_max_width_chars(24);
    text_col.append(&label);
    pill.append(&text_col);

    let hwdec = gtk4::Label::new(None);
    hwdec.add_css_class("dim");
    hwdec.set_visible(false);
    pill.append(&hwdec);

    let toggle = gtk4::Button::from_icon_name("media-playback-pause-symbolic");
    toggle.add_css_class("flat");
    toggle.set_tooltip_text(Some("Pause"));
    toggle.set_visible(false);
    pill.append(&toggle);

    let widgets = Rc::new(PillWidgets {
        dot,
        overline,
        label,
        hwdec,
        toggle: toggle.clone(),
        pill: pill.clone(),
    });

    {
        let widgets = widgets.clone();
        toggle.connect_clicked(move |btn| {
            let paused = btn.icon_name().as_deref() == Some("media-playback-start-symbolic");
            let req = if paused {
                Request::Resume
            } else {
                Request::Pause
            };
            send_fire_and_forget(req);
            // Re-poll shortly after so the pill reflects the new state without
            // waiting a full interval.
            let widgets = widgets.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || {
                poll_once(widgets);
            });
        });
    }

    poll_once(widgets.clone());
    // Runs for the life of the process: the single window (build_ui guards
    // against duplicates) closing quits the app, taking the timer with it.
    glib::timeout_add_local(Duration::from_secs(POLL_INTERVAL_S as u64), move || {
        poll_once(widgets.clone());
        ControlFlow::Continue
    });

    pill.upcast()
}

/// Fetch `Request::Status` on a background thread and apply the result to the
/// pill once it lands back on the main thread. Mirrors the
/// thread + `async_channel` + `glib::spawn_future_local` pattern used by
/// `poll_notifications` / `check_for_updates` — never blocks the GTK thread.
fn poll_once(widgets: Rc<PillWidgets>) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = ipc::request(&Request::Status);
        let _ = tx.send_blocking(result);
    });

    glib::spawn_future_local(async move {
        let Ok(result) = rx.recv().await else {
            return;
        };
        match result {
            Ok(crate::ipc::Response::Status(status)) => apply_status(&widgets, &status),
            Ok(_) => {}
            Err(e) => {
                // Daemon not running — expected and common, not an error.
                log::debug!("status poll: daemon unreachable: {e:#}");
                apply_off(&widgets);
            }
        }
    });
}

/// Fire a Pause/Resume request on a background thread; the result isn't
/// awaited (the next poll picks up the new state), matching the plan's
/// "fire-and-forget is fine" guidance for the toggle.
fn send_fire_and_forget(req: Request) {
    std::thread::spawn(move || {
        if let Err(e) = ipc::request(&req) {
            log::warn!("pause/resume request failed: {e:#}");
        }
    });
}

fn apply_off(w: &PillWidgets) {
    w.dot.remove_css_class("dot-ok");
    w.dot.remove_css_class("dot-warn");
    w.dot.add_css_class("dot-off");
    w.overline.set_visible(false);
    w.label.set_label("Not running");
    w.hwdec.set_visible(false);
    w.toggle.set_visible(false);
    w.pill.set_tooltip_text(None);
}

fn apply_status(w: &PillWidgets, status: &StatusReply) {
    if !status.running {
        apply_off(w);
        return;
    }

    let warn = status.paused || status.error.is_some();
    w.dot.remove_css_class("dot-ok");
    w.dot.remove_css_class("dot-warn");
    w.dot.remove_css_class("dot-off");
    w.dot
        .add_css_class(if warn { "dot-warn" } else { "dot-ok" });

    // Presentation-only restyle: overline state + prettified name in the pill,
    // CPU% relegated to the tooltip (with any daemon error).
    w.overline
        .set_label(if status.paused { "PAUSED" } else { "PLAYING" });
    w.overline.set_visible(true);
    let name = status.wallpaper.as_deref().unwrap_or("Wallpaper active");
    w.label.set_label(&pretty_status_name(name));

    match status.hwdec.as_deref() {
        Some(raw) if raw != "no" => {
            w.hwdec.set_label(hwdec_label(raw));
            w.hwdec.set_visible(true);
        }
        _ => w.hwdec.set_visible(false),
    }

    w.toggle.set_visible(true);
    if status.paused {
        w.toggle.set_icon_name("media-playback-start-symbolic");
        w.toggle.set_tooltip_text(Some("Resume"));
    } else {
        w.toggle.set_icon_name("media-playback-pause-symbolic");
        w.toggle.set_tooltip_text(Some("Pause"));
    }

    let mut tip = format!("CPU {:.0}%", status.cpu_percent);
    if let Some(err) = status.error.as_deref() {
        tip.push('\n');
        tip.push_str(err);
    }
    w.pill.set_tooltip_text(Some(&tip));
}

/// Prettify the daemon-reported wallpaper name for the pill: strip a trailing
/// media extension ("CAR.mp4" → "CAR") and middle-truncate very long names.
fn pretty_status_name(raw: &str) -> String {
    let mut n = raw.trim().to_string();
    if let Some((stem, ext)) = n.rsplit_once('.') {
        if !stem.is_empty()
            && matches!(
                ext.to_ascii_lowercase().as_str(),
                "mp4"
                    | "webm"
                    | "mkv"
                    | "avi"
                    | "mov"
                    | "flv"
                    | "gif"
                    | "jpg"
                    | "jpeg"
                    | "png"
                    | "webp"
                    | "bmp"
                    | "tiff"
            )
        {
            n = stem.to_string();
        }
    }
    let chars: Vec<char> = n.chars().collect();
    if chars.len() > 28 {
        let head: String = chars[..18].iter().collect();
        let tail: String = chars[chars.len() - 8..].iter().collect();
        n = format!("{}…{}", head.trim_end(), tail.trim_start());
    }
    n
}

/// Map a raw hwdec value from frescod to a friendly badge label.
fn hwdec_label(raw: &str) -> &str {
    match raw {
        "vaapi" => "VA-API",
        "nvdec" => "NVDEC",
        "vdpau" => "VDPAU",
        "drm" => "DRM",
        _ => raw,
    }
}
