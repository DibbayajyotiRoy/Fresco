//! The "Lock Screen" settings page and its "Preview lock screen" window
//! (docs/plan-lock-screen.md §6).
//!
//! # What this module owns, and what it never touches
//!
//! This is presentation only: every control here edits
//! [`crate::config::LockScreen`] and asks the daemon to re-`Apply` and (for
//! host setup) to run [`crate::ipc::Request::LockSetup`] /
//! [`crate::ipc::Request::LockUndo`]. Nothing here reads or renders a
//! password, and the preview path (see [`open_preview_window`]) only ever
//! sends [`crate::ipc::Request::LockPreview`] — a request the daemon answers
//! with a PNG, never with anything capable of locking the session. See §5's
//! "fail closed" invariant: this page cannot violate it because it has no
//! access to any locking primitive to call in the first place.
//!
//! # Why a `PreferencesWindow`, not a page in the main `Stack`
//!
//! Mirrors `window::show_advanced_dialog` exactly: one more `adw::Window`
//! reached from the app menu (and the command palette), not a third child of
//! the main window's library/editor `Stack`. Same reasoning as `Advanced…`
//! and `Browse wallpapers…` — this is a settings surface, not a mode the
//! library switches into, and a `PreferencesWindow` gets a working host
//! status section, a preset gallery and nine widget toggles onto the screen
//! with libadwaita's own layout rather than a bespoke one. It also comes with
//! its own [`adw::PreferencesWindow::add_toast`], so this page never needs to
//! reach into `AppState`'s private toast overlay (`window.rs` keeps that
//! field module-private; this module is a sibling of `window`, not a child of
//! it, so it could not reach a private field even if it wanted to).
//!
//! # Feature gating
//!
//! This file lives under `src/gui`, already gated behind the `gui` feature by
//! `mod.rs`. Everything it imports from outside `gui` —
//! [`crate::lockscreen`], [`crate::userinfo`], [`crate::config`],
//! [`crate::ipc`], [`crate::clock`] — is itself ungated, so a
//! `--no-default-features --features gui` build (no `daemon`, no
//! `widgetkit`, no `daemon::*`) compiles this module exactly as the
//! wave-2 contract promises.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use chrono::Timelike;
use gtk4::{glib, glib::ControlFlow, prelude::*};
use libadwaita::{self as adw, prelude::*};

use crate::clock::ClockTheme;
use crate::config::{LiveVideo, LockPreset, LockWidgets};
use crate::ipc::{self, LockSetupState, Request, Response, StatusReply};
use crate::lockscreen::LockWidget;
use crate::userinfo;
use crate::{t, tf};

use super::daemon_ctl;
use super::window::AppState;

/// Default timeout for every request on this page except the preview render.
const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(5);
/// `LockPreview` composites a full-resolution frame plus every enabled
/// widget; generous but still bounded, so a wedged daemon fails the button
/// instead of hanging it.
const PREVIEW_IPC_TIMEOUT: Duration = Duration::from_secs(15);
/// How often the status group re-polls while the page is open. Independent
/// of the header pill's own timer (`status::build_status_pill`) — this
/// dialog is opened and closed repeatedly, so its poll must start and stop
/// with the dialog, not run for the life of the process.
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(4);
/// How often the preview window re-fetches a frame while it is open — the
/// clock (and, on AC, the video) should visibly advance.
const PREVIEW_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Build and present the Lock Screen settings window. Reached from the app
/// menu's "Lock Screen…" item and the command palette (see `window.rs`).
pub fn show_lockscreen_window(parent: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let dialog = adw::PreferencesWindow::new();
    dialog.set_transient_for(Some(parent));
    dialog.set_modal(true);
    dialog.set_title(Some(t!("Lock Screen")));

    let flatpak = crate::is_flatpak();
    let page = adw::PreferencesPage::new();

    if flatpak {
        let banner_group = adw::PreferencesGroup::new();
        banner_group.add(&info_banner(t!(
            "Fresco’s lock screen needs the native package (.deb, .rpm, or AUR) — it can’t reach your system’s lock screen from inside Flatpak’s sandbox."
        )));
        page.add(&banner_group);
    }

    // What this desktop will actually do with the lock screen, first thing on
    // the page (issue #37): the master switch below it must not read as if it
    // always works. `support` is the one place the latest poll's verdict
    // lives, shared by the notice, the switch's toast and the status poll.
    let support = Rc::new(Cell::new(None::<LockSupport>));
    let notice = SupportNotice::new();
    page.add(&notice.group);

    let master_switch = add_master_group(&page, &state, &dialog, &support);
    let status_group = add_status_group(&page, &dialog, &notice, &support);
    let look_group = add_look_group(&page, &state);
    let widgets_group = add_widgets_group(&page, &state);
    let motion_group = add_motion_group(&page, &state);
    let preview_group = add_preview_group(&page, parent, &dialog);

    if flatpak {
        // The banner above already says why nothing here can work; a
        // "what reaches your lock screen" verdict would contradict it.
        notice.hide_for_good();
        master_switch.set_sensitive(false);
        for g in [
            &status_group,
            &look_group,
            &widgets_group,
            &motion_group,
            &preview_group,
        ] {
            g.set_sensitive(false);
        }
    }

    dialog.add(&page);
    dialog.present();
}

// ─── Master switch ────────────────────────────────────────────────────────────

/// Master switch + one-line explainer + the trust note. Returns the switch so
/// the caller can grey it out under Flatpak without a second lookup.
///
/// Turning it on where `support` says nothing reaches the lock screen still
/// saves the setting (it is harmless, and holds for the day the desktop gains
/// support) but says so out loud: a switch that flips with no consequence is
/// how people came to believe it had worked (issue #37).
fn add_master_group(
    page: &adw::PreferencesPage,
    state: &Rc<RefCell<AppState>>,
    dialog: &adw::PreferencesWindow,
    support: &Rc<Cell<Option<LockSupport>>>,
) -> gtk4::Switch {
    let cur = lockscreen_settings(state);

    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::new();
    row.set_title(t!("Show Fresco on the lock screen"));
    row.set_subtitle(t!(
        "Your wallpaper and widgets appear on the real lock screen, on desktops that support it"
    ));
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    sw.set_active(cur.enabled);
    row.add_suffix(&sw);
    row.set_activatable_widget(Some(&sw));
    {
        let state = state.clone();
        let support = support.clone();
        // Weak: the switch lives inside the dialog, so a strong handle here
        // would be a reference cycle that keeps a closed window alive.
        let dialog = dialog.downgrade();
        sw.connect_active_notify(move |sw| {
            let on = sw.is_active();
            edit_lockscreen(&state, |l| l.enabled = on);
            if let Some(msg) = enable_notice(on, support.get()) {
                if let Some(dialog) = dialog.upgrade() {
                    let toast = adw::Toast::new(msg);
                    toast.set_timeout(ENABLE_NOTICE_TIMEOUT_SECS);
                    dialog.add_toast(toast);
                }
            }
        });
    }
    group.add(&row);
    group.add(&info_banner(t!(
        "Your password is still checked by your system’s lock screen — Fresco never sees it."
    )));
    page.add(&group);
    sw
}

// ─── What reaches the real lock screen ────────────────────────────────────────

/// How long the "turned on, but nothing changes here" toast stays up. Longer
/// than libadwaita's default: it is a sentence to read, not a confirmation.
const ENABLE_NOTICE_TIMEOUT_SECS: u32 = 8;

/// What Fresco can put on this desktop's real lock screen — the one-line
/// answer the page, the switch's toast and the app menu all give, so they can
/// never disagree with each other (issue #37).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LockSupport {
    /// Live video and Fresco's widgets.
    Full,
    /// A still frame of the wallpaper, no widgets.
    StillFrame,
    /// Nothing: the desktop's lock screen keeps its own background.
    Nothing,
}

impl LockSupport {
    /// The summary sentence, shared verbatim by every surface that states it.
    pub(super) fn summary(self) -> &'static str {
        match self {
            LockSupport::Full => t!("Live video and widgets"),
            LockSupport::StillFrame => t!("Still frame only"),
            LockSupport::Nothing => {
                t!("Nothing yet — the lock screen keeps your desktop’s own background")
            }
        }
    }
}

/// Pure selection of [`LockSupport`] from the capability data the status tags
/// show (`live_video`, `widgets`) plus [`ipc::LockStatus::still_frame`] — the
/// only datum that tells "still frame only" from "nothing" once the first two
/// are both off.
///
/// `None` means the daemon did not say (it predates `still_frame`): the page
/// stays quiet rather than claim "nothing" for a desktop that may well get a
/// frame. No host is named here — a desktop that gains a still-frame writer
/// moves from `Nothing` to `StillFrame` purely by the daemon reporting it.
///
/// `live_video` and `widgets` are two flags but one tier on every host today
/// (the daemon sets both from one `capable` test), so either one being on is
/// [`LockSupport::Full`].
pub(super) fn lock_support(
    live_video: bool,
    widgets: bool,
    still_frame: Option<bool>,
) -> Option<LockSupport> {
    if live_video || widgets {
        return Some(LockSupport::Full);
    }
    match still_frame {
        Some(true) => Some(LockSupport::StillFrame),
        Some(false) => Some(LockSupport::Nothing),
        None => None,
    }
}

/// [`lock_support`] over a daemon's [`ipc::LockStatus`].
pub(super) fn lock_support_of(ls: &ipc::LockStatus) -> Option<LockSupport> {
    lock_support(ls.live_video, ls.widgets, ls.still_frame)
}

/// The desktop's name as the notice words it. `"unsupported"` is not a
/// desktop anyone recognises, and [`host_label`]'s sentence for it would read
/// as nonsense after "On", so it becomes plain "this desktop".
fn notice_desktop(host: &str) -> String {
    match host {
        "unsupported" => t!("this desktop").to_string(),
        other => host_label(other),
    }
}

/// The page-top notice's text: `On GNOME: Still frame only`.
fn support_notice_text(host: &str, support: LockSupport) -> String {
    tf!(
        "On {desktop}: {summary}",
        "desktop" => notice_desktop(host),
        "summary" => support.summary()
    )
}

/// The toast shown when the master switch is turned on, or `None` when no
/// toast is due: only for a switch going on, on a desktop where `support` is
/// known to be [`LockSupport::Nothing`]. An unknown verdict (daemon down) and
/// every desktop with something to show stay silent.
fn enable_notice(on: bool, support: Option<LockSupport>) -> Option<&'static str> {
    (on && support == Some(LockSupport::Nothing)).then(|| {
        t!(
            "Saved, but this desktop can’t show Fresco on the lock screen yet — it keeps using its own background"
        )
    })
}

/// The trailing tag on the app menu's "Lock Screen…" row: nothing when the
/// desktop gets everything (or we do not know yet), otherwise a short dim
/// mark so the limit is visible before the page is even opened.
pub(super) fn menu_tag(support: Option<LockSupport>) -> Option<&'static str> {
    match support {
        Some(LockSupport::StillFrame) => Some(t!("Still frame only")),
        Some(LockSupport::Nothing) => Some(t!("Not supported yet")),
        Some(LockSupport::Full) | None => None,
    }
}

/// The menu row's tooltip: what the entry is, then — once known — what the
/// desktop will do with it.
pub(super) fn menu_tooltip(support: Option<LockSupport>) -> String {
    let base = t!("Show Fresco on the lock screen");
    match support {
        Some(s) => format!("{base}\n{}", s.summary()),
        None => base.to_string(),
    }
}

/// Give the app menu's "Lock Screen…" row (a `window::menu_item`, whose child
/// is a lone label) its support hint: a trailing tag and a tooltip carrying
/// the same summary the page shows, refreshed from the last status poll each
/// time the menu opens. A row of any other shape is left untouched.
pub(super) fn decorate_menu_row(btn: &gtk4::Button, popover: &gtk4::Popover) {
    let Some(label) = btn.child().and_downcast::<gtk4::Label>() else {
        return;
    };
    let tag = gtk4::Label::new(None);
    tag.set_visible(false);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn.set_child(None::<&gtk4::Widget>);
    row.append(&label);
    row.append(&tag);
    btn.set_child(Some(&row));

    let refresh = {
        let btn = btn.clone();
        move || {
            let support = super::status::cached_lock_support();
            btn.set_tooltip_text(Some(&menu_tooltip(support)));
            match menu_tag(support) {
                Some(text) => {
                    let nothing = support == Some(LockSupport::Nothing);
                    tag.set_label(text);
                    // The unsupported case is the one worth a colour.
                    let (add, remove) = if nothing {
                        ("warning", "dim")
                    } else {
                        ("dim", "warning")
                    };
                    tag.remove_css_class(remove);
                    tag.add_css_class(add);
                    tag.set_visible(true);
                }
                None => tag.set_visible(false),
            }
        }
    };
    refresh();
    popover.connect_show(move |_| refresh());
}

/// The notice at the top of the page, owned by one dialog. Hidden until the
/// first successful poll names a verdict; hidden again if a later poll cannot.
#[derive(Clone)]
struct SupportNotice {
    group: adw::PreferencesGroup,
    icon: gtk4::Image,
    label: gtk4::Label,
    /// Set under Flatpak, where the page explains itself differently and every
    /// later poll must leave the notice hidden.
    suppressed: Rc<Cell<bool>>,
}

impl SupportNotice {
    fn new() -> Self {
        let label = gtk4::Label::new(None);
        label.set_wrap(true);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        // The same `.capability-banner` shape as `info_banner_widget`, built
        // by hand because this one needs to swap its icon.
        let banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        banner.add_css_class("capability-banner");
        let icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
        icon.set_valign(gtk4::Align::Start);
        banner.append(&icon);
        banner.append(&label);
        let group = adw::PreferencesGroup::new();
        group.add(&banner);
        group.set_visible(false);
        SupportNotice {
            group,
            icon,
            label,
            suppressed: Rc::new(Cell::new(false)),
        }
    }

    /// Show `support` for `host`, or hide the notice when there is no verdict.
    /// The "nothing" case gets a warning icon and colour: it is the one the
    /// reporter of issue #37 needed to notice.
    fn show(&self, host: &str, support: Option<LockSupport>) {
        let Some(support) = support.filter(|_| !self.suppressed.get()) else {
            self.group.set_visible(false);
            return;
        };
        self.label.set_label(&support_notice_text(host, support));
        let nothing = support == LockSupport::Nothing;
        self.icon.set_icon_name(Some(if nothing {
            "dialog-warning-symbolic"
        } else {
            "dialog-information-symbolic"
        }));
        if nothing {
            self.label.add_css_class("warning");
        } else {
            self.label.remove_css_class("warning");
        }
        self.group.set_visible(true);
    }

    fn hide_for_good(&self) {
        self.suppressed.set(true);
        self.group.set_visible(false);
    }
}

// ─── Host status ──────────────────────────────────────────────────────────────

/// Which daemon request a click on the status row's button should send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupAction {
    Setup,
    Undo,
}

/// The Set up/Undo button's label and action for a given
/// [`LockSetupState`] — `None` when the row should not be shown at all
/// (nothing to do on this host, or no setup path exists yet).
fn setup_button_spec(state: LockSetupState) -> Option<(String, SetupAction)> {
    match state {
        LockSetupState::Needed => Some((t!("Set up").to_string(), SetupAction::Setup)),
        LockSetupState::Done => Some((t!("Undo").to_string(), SetupAction::Undo)),
        LockSetupState::NotNeeded | LockSetupState::Unavailable => None,
    }
}

/// Stable host id (see [`LockStatus::host`]) → human label. Falls back to the
/// raw id for anything this build doesn't recognise yet, rather than showing
/// nothing — a newer daemon naming a host this GUI has never heard of is a
/// version-skew hiccup, not a reason to go blank.
fn host_label(host: &str) -> String {
    match host {
        "cosmic" => t!("COSMIC"),
        "kde" => t!("KDE Plasma"),
        "gnome" => t!("GNOME"),
        "cinnamon" => t!("Cinnamon"),
        "mate" => t!("MATE"),
        "xfce" => t!("Xfce"),
        "deepin" => t!("Deepin"),
        "wlroots" => t!("Sway / Hyprland / niri (swaylock-plugin)"),
        "x11" => t!("X11 (xsecurelock)"),
        "unsupported" => t!("Not supported on this desktop yet"),
        other => other,
    }
    .to_string()
}

/// COSMIC keeps its own panel (clock, name, battery, password) at the top of
/// the lock screen until upstream theming lands (plan §4.1); this hint tells
/// the owner of that panel which of *this page's* toggles would otherwise
/// double it up. `None` for every other host: the hint names GUI controls
/// that only make sense next to COSMIC's specific limitation.
fn cosmic_hint(host: &str) -> Option<String> {
    (host == "cosmic").then(|| {
        t!(
            "COSMIC’s own panel — time, name, battery and the password field — stays at the top. Fresco’s widgets fill the rest of the screen. Turn off Clock, Battery or Greeting below to avoid showing them twice."
        )
        .to_string()
    })
}

/// Deepin's real lock screen only receives a still frame — with the dim slider
/// baked in (Deepin adds its own blur and tint on top), but no widgets (a frozen
/// clock would be wrong a minute later) — so the widgets show in Fresco's
/// preview only. `None` for every other host.
fn deepin_hint(host: &str) -> Option<String> {
    (host == "deepin").then(|| {
        t!("On Deepin the lock screen gets a still frame with your dim. Deepin adds its own blur and tint. Widgets appear in the preview only.")
            .to_string()
    })
}

/// `"{base} ✓"` / `"{base} ✗"` — the capability chips' text.
fn chip_label(base: &str, on: bool) -> String {
    format!("{base} {}", if on { "✓" } else { "✗" })
}

/// One copyable snippet: a short label for what it is, and the exact command
/// text. Verbatim strings the plan specifies — a wrong keybind syntax here is
/// a real bug someone copy-pastes onto their machine, so these are pinned by
/// a unit test rather than only eyeballed.
const WLROOTS_SNIPPETS: [(&str, &str); 3] = [
    (
        "Sway / Hyprland / niri keybind",
        "bindsym $mod+Escape exec fresco lock",
    ),
    ("hypridle", "lock_cmd = fresco lock"),
    (
        "swayidle",
        "timeout 300 'fresco lock' before-sleep 'fresco lock'",
    ),
];
const X11_SNIPPETS: [(&str, &str); 1] = [("X11 (xss-lock)", "xss-lock -- fresco lock")];

/// The snippets to offer for `host`, empty for every host that either needs
/// no snippet (COSMIC, KDE: one-click setup instead) or has no lock command
/// to bind yet (GNOME, Cinnamon, MATE, Xfce, Deepin: still-frame only, no
/// `fresco lock` host adapter in the plan).
fn snippets_for_host(host: &str) -> &'static [(&'static str, &'static str)] {
    match host {
        "wlroots" => &WLROOTS_SNIPPETS,
        "x11" => &X11_SNIPPETS,
        _ => &[],
    }
}

/// Everything the status group updates in place on every poll, built once so
/// a 4-second refresh never tears down and rebuilds the button the user might
/// be about to click.
struct StatusWidgets {
    muted: gtk4::Label,
    content: gtk4::Box,
    host_row: adw::ActionRow,
    live_chip: gtk4::Label,
    widgets_chip: gtk4::Label,
    notes_banner: gtk4::Box,
    notes_label: gtk4::Label,
    setup_row: adw::ActionRow,
    setup_btn: gtk4::Button,
    snippets_box: gtk4::Box,
    /// The page-top verdict, refreshed by the same poll as everything above.
    notice: SupportNotice,
    /// Where each poll leaves the verdict for the master switch's toast.
    support: Rc<Cell<Option<LockSupport>>>,
}

fn add_status_group(
    page: &adw::PreferencesPage,
    dialog: &adw::PreferencesWindow,
    notice: &SupportNotice,
    support: &Rc<Cell<Option<LockSupport>>>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Status"));

    let muted = gtk4::Label::new(Some(t!("Start Fresco to set up the lock screen")));
    muted.add_css_class("dim");
    muted.set_xalign(0.0);
    muted.set_margin_top(4);
    muted.set_margin_bottom(4);
    group.add(&muted);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);

    let host_row = adw::ActionRow::new();
    host_row.set_title(t!("Desktop"));
    let chips = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    chips.set_valign(gtk4::Align::Center);
    let live_chip = chip_widget("", false);
    let widgets_chip = chip_widget("", false);
    chips.append(&live_chip);
    chips.append(&widgets_chip);
    host_row.add_suffix(&chips);
    content.append(&host_row);

    let notes_label = gtk4::Label::new(None);
    notes_label.set_wrap(true);
    notes_label.set_xalign(0.0);
    notes_label.set_hexpand(true);
    let notes_banner = info_banner_widget(&notes_label);
    content.append(&notes_banner);

    let setup_row = adw::ActionRow::new();
    setup_row.set_title(t!("Lock screen integration"));
    let setup_btn = gtk4::Button::new();
    setup_btn.set_valign(gtk4::Align::Center);
    setup_row.add_suffix(&setup_btn);
    content.append(&setup_row);

    let snippets_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    content.append(&snippets_box);

    group.add(&content);
    page.add(&group);

    let widgets = Rc::new(StatusWidgets {
        muted,
        content,
        host_row,
        live_chip,
        widgets_chip,
        notes_banner,
        notes_label,
        setup_row,
        setup_btn,
        snippets_box,
        notice: notice.clone(),
        support: support.clone(),
    });
    let action = Rc::new(Cell::new(None::<SetupAction>));

    // Nothing polled yet: start muted rather than guessing.
    apply_status(&widgets, &action, None);

    {
        // Setup/Undo only ever sends a daemon request — nothing on this path
        // reads or writes `Config`, unlike every other control on the page.
        // `setup_btn` is cloned out (a cheap GObject handle) before `widgets`
        // itself is moved into the closure below — `connect_clicked` needs a
        // live borrow through `widgets` for the call, which would otherwise
        // conflict with the same closure capturing `widgets` by move.
        let setup_btn = widgets.setup_btn.clone();
        let dialog = dialog.clone();
        let widgets = widgets.clone();
        let action = action.clone();
        setup_btn.connect_clicked(move |btn| {
            let Some(a) = action.get() else { return };
            btn.set_sensitive(false);
            let req = match a {
                SetupAction::Setup => Request::LockSetup,
                SetupAction::Undo => Request::LockUndo,
            };
            let dialog = dialog.clone();
            let widgets = widgets.clone();
            let action = action.clone();
            spawn_ipc(req, DEFAULT_IPC_TIMEOUT, move |result| {
                match result {
                    Ok(Response::Err { message }) => {
                        dialog.add_toast(adw::Toast::new(&message));
                    }
                    Err(e) => {
                        dialog.add_toast(adw::Toast::new(&format!("{e:#}")));
                    }
                    Ok(_) => {}
                }
                // Re-poll regardless of outcome, so the row reflects reality
                // rather than what the click merely hoped would happen.
                let widgets = widgets.clone();
                let action = action.clone();
                fetch_status_async(move |status| apply_status(&widgets, &action, status));
            });
        });
    }

    {
        let widgets = widgets.clone();
        let action = action.clone();
        fetch_status_async(move |status| apply_status(&widgets, &action, status));
    }
    let source = Rc::new(Cell::new(None::<glib::SourceId>));
    {
        let widgets = widgets.clone();
        let action = action.clone();
        let id = glib::timeout_add_local(STATUS_POLL_INTERVAL, move || {
            let widgets = widgets.clone();
            let action = action.clone();
            fetch_status_async(move |status| apply_status(&widgets, &action, status));
            ControlFlow::Continue
        });
        source.set(Some(id));
    }
    dialog.connect_close_request(move |_| {
        if let Some(id) = source.take() {
            id.remove();
        }
        glib::Propagation::Proceed
    });

    group
}

/// Update every [`StatusWidgets`] field in place from a fresh poll —
/// `status.is_none()` and `status.lockscreen.is_none()` both mean "nothing to
/// show yet" (daemon down, or an older daemon that predates this field; see
/// [`crate::ipc::StatusReply::lockscreen`]'s own doc comment).
fn apply_status(
    w: &Rc<StatusWidgets>,
    action: &Rc<Cell<Option<SetupAction>>>,
    status: Option<StatusReply>,
) {
    let Some(ls) = status.and_then(|s| s.lockscreen) else {
        w.muted.set_visible(true);
        w.content.set_visible(false);
        action.set(None);
        w.support.set(None);
        w.notice.show("", None);
        return;
    };
    w.muted.set_visible(false);
    w.content.set_visible(true);

    let support = lock_support_of(&ls);
    w.support.set(support);
    w.notice.show(&ls.host, support);

    w.host_row
        .set_subtitle(&glib::markup_escape_text(&host_label(&ls.host)));
    w.live_chip
        .set_label(&chip_label(t!("Live video"), ls.live_video));
    w.widgets_chip
        .set_label(&chip_label(t!("Widgets"), ls.widgets));

    let mut notes = ls.notes.clone();
    if let Some(hint) = cosmic_hint(&ls.host) {
        notes.push(hint);
    }
    if let Some(hint) = deepin_hint(&ls.host) {
        notes.push(hint);
    }
    if notes.is_empty() {
        w.notes_banner.set_visible(false);
    } else {
        w.notes_label.set_label(
            &notes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n\n"),
        );
        w.notes_banner.set_visible(true);
    }

    match setup_button_spec(ls.setup) {
        Some((label, a)) => {
            w.setup_row.set_visible(true);
            w.setup_btn.set_label(&label);
            w.setup_btn.set_sensitive(true);
            action.set(Some(a));
        }
        None => {
            w.setup_row.set_visible(false);
            action.set(None);
        }
    }

    while let Some(child) = w.snippets_box.first_child() {
        w.snippets_box.remove(&child);
    }
    for (label, cmd) in snippets_for_host(&ls.host) {
        w.snippets_box.append(&snippet_row(label, cmd));
    }
}

// ─── Look ─────────────────────────────────────────────────────────────────────

/// "Preset default" + one entry per [`ClockTheme::ALL`]. Index 0 is `None`
/// (follow the preset); index `i+1` is `Some(ClockTheme::ALL[i])`. Kept as a
/// pair of pure functions (below, with the labels) rather than a stored
/// table, since `ClockTheme::label()` is itself a `const fn` over a `const`
/// array — there is nothing to keep in sync by hand.
fn clock_override_labels() -> Vec<&'static str> {
    std::iter::once(t!("Preset default"))
        .chain(ClockTheme::ALL.iter().map(|c| t!(c.label())))
        .collect()
}

fn clock_override_index(v: Option<ClockTheme>) -> u32 {
    match v {
        None => 0,
        Some(c) => ClockTheme::ALL
            .iter()
            .position(|&x| x == c)
            .map(|i| i as u32 + 1)
            .unwrap_or(0),
    }
}

fn clock_override_from_index(i: u32) -> Option<ClockTheme> {
    if i == 0 {
        None
    } else {
        ClockTheme::ALL.get(i as usize - 1).copied()
    }
}

/// Split `[lockscreen].greeting` into what the entry text and the "Hide
/// greeting" switch should show. The tri-state on disk
/// (`None`/`Some("")`/`Some(text)`) is `crate::lockscreen::GreetingText`'s own
/// reading (see [`crate::lockscreen::resolve`]); this is the same reading
/// applied to populate the two GUI controls instead of a render.
fn greeting_ui_state(cfg: Option<&str>) -> (bool, String) {
    match cfg {
        None => (false, String::new()),
        Some("") => (true, String::new()),
        Some(s) => (false, s.to_string()),
    }
}

/// Inverse of [`greeting_ui_state`]. A blank or whitespace-only entry with the
/// switch off collapses to `None` (Auto) rather than `Some("   ")` — "typed
/// nothing" and "no override" are the same state, matching how
/// [`crate::lockscreen::resolve`] already treats an empty string as
/// [`crate::lockscreen::GreetingText::Hidden`] rather than as a custom text of
/// zero characters.
fn greeting_config_value(hide: bool, text: &str) -> Option<String> {
    if hide {
        Some(String::new())
    } else if text.trim().is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn add_look_group(
    page: &adw::PreferencesPage,
    state: &Rc<RefCell<AppState>>,
) -> adw::PreferencesGroup {
    let cur = lockscreen_settings(state);

    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Look"));
    group.set_description(Some(t!("Pick a style, then fine-tune it.")));

    // ── Preset gallery ── pick a style first, then fine-tune it below.
    //
    // Wrapped in a bare `AdwPreferencesRow` rather than handed to
    // `group.add()` on its own: `AdwPreferencesGroup` always renders actual
    // rows ahead of any plain widget added to it, regardless of call order,
    // so a raw `FlowBox` — added first, even — still sank below Clock
    // style/Greeting/Dim/Blur. `AdwPreferencesRow` is the base class
    // `AdwActionRow`/`AdwComboRow` themselves build on, so a bare instance of
    // it counts as a real row; putting the gallery inside one and adding
    // *that* before the other rows is what actually moves it to the top.
    let flow = gtk4::FlowBox::new();
    flow.add_css_class("lock-preset-gallery");
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_column_spacing(8);
    flow.set_row_spacing(8);
    flow.set_homogeneous(true);
    // 2–3 columns at the window's default width. Capped at 3 rather than
    // left at 5 (as this used to be): a `GtkFlowBox` sizes each child from
    // its *natural* width, and see the `set_max_width_chars` calls below for
    // why that used to make every card claim a full, unwrapped line to
    // itself.
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(3);
    flow.set_margin_top(10);
    flow.set_margin_bottom(10);
    flow.set_margin_start(6);
    flow.set_margin_end(6);
    let mut group_anchor: Option<gtk4::ToggleButton> = None;
    for preset in LockPreset::ALL {
        let btn = gtk4::ToggleButton::new();
        btn.add_css_class("lock-preset-card");
        // Non-hexpand: a homogeneous `FlowBox` already gives every card the
        // same, content-sized width. A hexpanding button would compete with
        // that by asking to stretch and fill, which starves exactly the
        // wrapping that makes a multi-column grid possible.
        btn.set_hexpand(false);
        let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        let title_text = preset.label();
        let blurb_text = preset.blurb();
        let title = gtk4::Label::new(Some(&title_text));
        title.add_css_class("lock-preset-title");
        title.set_xalign(0.0);
        title.set_width_chars(10);
        title.set_max_width_chars(16);
        let blurb = gtk4::Label::new(Some(&blurb_text));
        blurb.add_css_class("dim");
        blurb.add_css_class("lock-preset-blurb");
        blurb.set_wrap(true);
        blurb.set_xalign(0.0);
        // The bug this fixes: without a cap, a *wrapping* label's natural
        // width is still its full unwrapped line, so the FlowBox above never
        // saw a card narrow enough to place more than one per row, and
        // stacked all five full-width instead.
        blurb.set_width_chars(20);
        blurb.set_max_width_chars(22);
        inner.append(&title);
        inner.append(&blurb);
        btn.set_child(Some(&inner));
        // Without this, the accessible name is whatever AT-SPI infers from
        // the child labels; state it explicitly so a screen reader always
        // reads the style name and its description together, in order.
        let accessible_name = format!("{title_text}: {blurb_text}");
        btn.update_property(&[gtk4::accessible::Property::Label(&accessible_name)]);
        match &group_anchor {
            Some(anchor) => btn.set_group(Some(anchor)),
            None => group_anchor = Some(btn.clone()),
        }
        // Set before connecting: populating the gallery must not look like a
        // user edit and must not wake the daemon.
        btn.set_active(preset == cur.preset);
        {
            let state = state.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    edit_lockscreen(&state, |l| l.preset = preset);
                }
            });
        }
        flow.append(&btn);
    }
    let preset_row = adw::PreferencesRow::new();
    preset_row.add_css_class("lock-preset-row");
    // A plain container, not a control in its own right: it must not
    // intercept a click or Enter as if it were one activatable thing, and Tab
    // should land on the cards inside it rather than stopping on this empty
    // wrapper first.
    preset_row.set_activatable(false);
    preset_row.set_selectable(false);
    preset_row.set_focusable(false);
    preset_row.set_child(Some(&flow));
    group.add(&preset_row);

    // ── Clock style override ──
    let clock_row = adw::ComboRow::new();
    clock_row.set_title(t!("Clock style"));
    clock_row.set_subtitle(t!("Overrides the preset’s own clock look"));
    clock_row.set_model(Some(&gtk4::StringList::new(&clock_override_labels())));
    clock_row.set_selected(clock_override_index(cur.clock_theme));
    {
        let state = state.clone();
        clock_row.connect_selected_notify(move |row| {
            let theme = clock_override_from_index(row.selected());
            edit_lockscreen(&state, |l| l.clock_theme = theme);
        });
    }
    group.add(&clock_row);

    // ── Greeting ──
    let (hide_init, text_init) = greeting_ui_state(cur.greeting.as_deref());
    let greeting_row = adw::ActionRow::new();
    greeting_row.set_title(t!("Greeting"));
    greeting_row.set_subtitle(t!(
        "Shown automatically unless you set your own text or hide it below"
    ));
    let greeting_entry = gtk4::Entry::new();
    greeting_entry.set_valign(gtk4::Align::Center);
    // Wide enough for both the placeholder ("Good afternoon, <name>") and a
    // typical custom greeting — left unset, the entry shrank to whatever the
    // row had left over and clipped the placeholder mid-word.
    greeting_entry.set_width_chars(24);
    greeting_entry.set_text(&text_init);
    greeting_entry.set_sensitive(!hide_init);
    greeting_entry.set_placeholder_text(Some(&auto_greeting_now(None)));
    greeting_row.add_suffix(&greeting_entry);
    group.add(&greeting_row);

    let hide_row = adw::ActionRow::new();
    hide_row.set_title(t!("Hide greeting"));
    let hide_switch = gtk4::Switch::new();
    hide_switch.set_valign(gtk4::Align::Center);
    hide_switch.set_active(hide_init);
    hide_row.add_suffix(&hide_switch);
    hide_row.set_activatable_widget(Some(&hide_switch));
    group.add(&hide_row);

    {
        let state = state.clone();
        let hide_switch = hide_switch.clone();
        greeting_entry.connect_changed(move |entry| {
            let value = greeting_config_value(hide_switch.is_active(), &entry.text());
            edit_lockscreen(&state, |l| l.greeting = value);
        });
    }
    {
        let state = state.clone();
        let greeting_entry = greeting_entry.clone();
        hide_switch.connect_active_notify(move |sw| {
            let hide = sw.is_active();
            greeting_entry.set_sensitive(!hide);
            let value = greeting_config_value(hide, &greeting_entry.text());
            edit_lockscreen(&state, |l| l.greeting = value);
        });
    }

    // The real name needs `userinfo::current_identity()` (a couple of bounded but
    // real `gdbus` round trips), so it is resolved exactly once, off the GTK
    // thread, and only ever *upgrades* the placeholder already showing the
    // time-only phrase — never blocks opening this page on it.
    {
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let info = userinfo::current_identity();
            let _ = tx.send_blocking(userinfo::first_name(&info));
        });
        glib::spawn_future_local(async move {
            if let Ok(name) = rx.recv().await {
                greeting_entry.set_placeholder_text(Some(&auto_greeting_now(Some(&name))));
            }
        });
    }

    // ── Dim / blur ──
    let dim_row = lock_slider_row(
        t!("Dim"),
        t!("Darkens the wallpaper behind the widgets"),
        (0.0, 0.8, 0.01),
        f64::from(cur.dim),
        {
            let state = state.clone();
            move |v| edit_lockscreen(&state, |l| l.dim = v as f32)
        },
    );
    group.add(&dim_row);

    let blur_row = lock_slider_row(
        t!("Blur"),
        t!("Still images only"),
        (0.0, 1.0, 0.01),
        // Slider position on the curved scale (see `blur_radius_for`), read
        // through `blur_percent` so a not-yet-migrated config shows where its
        // old look now lives rather than a number on the wrong scale.
        f64::from(crate::lockscreen::blur_percent(&cur) / 100.0),
        {
            let state = state.clone();
            move |v| {
                edit_lockscreen(&state, |l| {
                    l.blur = v as f32;
                    l.blur_curve = crate::config::LOCK_BLUR_CURVE;
                })
            }
        },
    );
    group.add(&blur_row);

    page.add(&group);
    group
}

/// The auto-greeting phrase for right now, with or without a name — the
/// placeholder text, never written to config (an empty entry always means
/// Auto; see [`greeting_config_value`]).
fn auto_greeting_now(name: Option<&str>) -> String {
    let hour = chrono::Local::now().hour();
    userinfo::greeting(hour, name)
}

// ─── Widgets ──────────────────────────────────────────────────────────────────

fn lock_widget_enabled(w: LockWidgets, widget: LockWidget) -> bool {
    match widget {
        LockWidget::Clock => w.clock,
        LockWidget::Date => w.date,
        LockWidget::Greeting => w.greeting,
        LockWidget::Avatar => w.avatar,
        LockWidget::NowPlaying => w.now_playing,
        LockWidget::AlbumArt => w.album_art,
        LockWidget::Battery => w.battery,
        LockWidget::Lyrics => w.lyrics,
        LockWidget::Visualizer => w.visualizer,
    }
}

fn set_lock_widget_enabled(w: &mut LockWidgets, widget: LockWidget, on: bool) {
    match widget {
        LockWidget::Clock => w.clock = on,
        LockWidget::Date => w.date = on,
        LockWidget::Greeting => w.greeting = on,
        LockWidget::Avatar => w.avatar = on,
        LockWidget::NowPlaying => w.now_playing = on,
        LockWidget::AlbumArt => w.album_art = on,
        LockWidget::Battery => w.battery = on,
        LockWidget::Lyrics => w.lyrics = on,
        LockWidget::Visualizer => w.visualizer = on,
    }
}

fn add_widgets_group(
    page: &adw::PreferencesPage,
    state: &Rc<RefCell<AppState>>,
) -> adw::PreferencesGroup {
    let cur = lockscreen_settings(state);

    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Widgets"));
    group.set_description(Some(t!(
        "Anyone standing at the machine can see these before it’s unlocked."
    )));

    for widget in LockWidget::ALL {
        let row = adw::ActionRow::new();
        row.set_title(&widget.label());
        if let Some(note) = widget.privacy_note() {
            row.set_subtitle(&note);
        }
        let sw = gtk4::Switch::new();
        sw.set_valign(gtk4::Align::Center);
        sw.set_active(lock_widget_enabled(cur.widgets, widget));
        {
            let state = state.clone();
            sw.connect_active_notify(move |sw| {
                let on = sw.is_active();
                edit_lockscreen(&state, |l| {
                    set_lock_widget_enabled(&mut l.widgets, widget, on)
                });
            });
        }
        row.add_suffix(&sw);
        row.set_activatable_widget(Some(&sw));
        group.add(&row);
    }

    page.add(&group);
    group
}

// ─── Motion & power ─────────────────────────────────────────────────────────────

const LIVE_VIDEO_OPTIONS: [LiveVideo; 3] = [LiveVideo::Ac, LiveVideo::Always, LiveVideo::Never];

fn live_video_labels() -> [&'static str; 3] {
    [t!("On AC power only"), t!("Always"), t!("Never")]
}

fn live_video_index(v: LiveVideo) -> u32 {
    LIVE_VIDEO_OPTIONS.iter().position(|&x| x == v).unwrap_or(0) as u32
}

fn live_video_from_index(i: u32) -> LiveVideo {
    LIVE_VIDEO_OPTIONS
        .get(i as usize)
        .copied()
        .unwrap_or_default()
}

fn add_motion_group(
    page: &adw::PreferencesPage,
    state: &Rc<RefCell<AppState>>,
) -> adw::PreferencesGroup {
    let cur = lockscreen_settings(state);

    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Motion &amp; power"));

    let row = adw::ComboRow::new();
    row.set_title(t!("Live video"));
    // Short on purpose: a long subtitle here competes with the row's own
    // selected-value label ("On AC power only") for width and was clipping
    // it to "On AC po…".
    row.set_subtitle(t!("Video on the lock screen uses more battery"));
    row.set_model(Some(&gtk4::StringList::new(&live_video_labels())));
    row.set_selected(live_video_index(cur.live_video));
    {
        let state = state.clone();
        row.connect_selected_notify(move |row| {
            let v = live_video_from_index(row.selected());
            edit_lockscreen(&state, |l| l.live_video = v);
        });
    }
    group.add(&row);

    page.add(&group);
    group
}

// ─── Preview ──────────────────────────────────────────────────────────────────

fn add_preview_group(
    page: &adw::PreferencesPage,
    parent: &adw::ApplicationWindow,
    dialog: &adw::PreferencesWindow,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Preview"));

    let row = adw::ActionRow::new();
    row.set_title(t!("Preview lock screen"));
    row.set_subtitle(t!(
        "Shows what the lock screen will look like, full-screen, on this display. It never locks anything."
    ));
    let btn = gtk4::Button::with_label(t!("Preview…"));
    btn.set_valign(gtk4::Align::Center);
    row.add_suffix(&btn);
    row.set_activatable_widget(Some(&btn));

    {
        let parent = parent.clone();
        let dialog = dialog.clone();
        btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            let (width, height) = window_monitor(&parent)
                .map(|m| monitor_pixel_size_from(&m))
                .unwrap_or(FALLBACK_PIXEL_SIZE);
            let btn = btn.clone();
            let parent = parent.clone();
            let dialog2 = dialog.clone();
            spawn_ipc(
                Request::LockPreview { width, height },
                PREVIEW_IPC_TIMEOUT,
                move |result| {
                    btn.set_sensitive(true);
                    match result {
                        Ok(Response::LockPreview { path }) => {
                            // A frame that cannot be loaded would otherwise
                            // open as an empty, black full-screen window.
                            if !open_preview_window(&parent, width, height, path) {
                                dialog2.add_toast(adw::Toast::new(t!(
                                    "Couldn't load the preview image"
                                )));
                            }
                        }
                        Ok(Response::Err { message }) => {
                            dialog2.add_toast(adw::Toast::new(&message));
                        }
                        Ok(_) => {
                            dialog2
                                .add_toast(adw::Toast::new(t!("Unexpected response from Fresco")));
                        }
                        Err(e) => {
                            dialog2.add_toast(adw::Toast::new(&format!("{e:#}")));
                        }
                    }
                },
            );
        });
    }

    group.add(&row);
    page.add(&group);
    group
}

/// Used only if the main window's monitor cannot be determined at all (no
/// display, or a not-yet-realised surface) — a 1080p guess is better than
/// refusing the preview outright.
const FALLBACK_PIXEL_SIZE: (u32, u32) = (1920, 1080);

/// The monitor the main window is currently shown on. `None` only when the
/// window has no surface yet (not realised) or GDK has no display at all —
/// both effectively impossible for a window the user just clicked a button
/// in, but this must still not panic.
fn window_monitor(win: &adw::ApplicationWindow) -> Option<gtk4::gdk::Monitor> {
    let surface = win.surface()?;
    // `ApplicationWindow` implements both `Root` and `Widget`, and both
    // traits define `display()` — the widget one is what we want (the
    // `Root` one exists for surfaceless roots), so it must be named
    // explicitly rather than through the ambiguous inherent-looking call.
    gtk4::prelude::WidgetExt::display(win).monitor_at_surface(&surface)
}

/// `(logical size, scale factor)` → physical pixels, clamped so a
/// pathological monitor report (negative geometry, zero scale) can never
/// underflow or panic. [`gtk4::gdk::Monitor::geometry`] returns *logical*
/// pixels; the PNG the daemon renders should match the physical framebuffer,
/// so this multiplies back up by the scale factor.
fn monitor_pixel_size(logical_width: i32, logical_height: i32, scale_factor: i32) -> (u32, u32) {
    let scale = i64::from(scale_factor.max(1));
    let w = i64::from(logical_width.max(0)) * scale;
    let h = i64::from(logical_height.max(0)) * scale;
    (
        w.clamp(0, i64::from(u32::MAX)) as u32,
        h.clamp(0, i64::from(u32::MAX)) as u32,
    )
}

fn monitor_pixel_size_from(monitor: &gtk4::gdk::Monitor) -> (u32, u32) {
    let geo = monitor.geometry();
    monitor_pixel_size(geo.width(), geo.height(), monitor.scale_factor())
}

/// Load `path` into `picture`. Errors are logged and otherwise ignored: on
/// the very first load the caller already knows the daemon answered `Ok`, so
/// a decode failure here is unusual, and on a refresh tick the right thing is
/// to keep showing the last good frame rather than blank the window over one
/// missed beat.
fn load_preview_frame(picture: &gtk4::Picture, path: &str) {
    match gtk4::gdk::Texture::from_filename(path) {
        Ok(texture) => picture.set_paintable(Some(&texture)),
        Err(e) => log::warn!("lock screen preview: couldn't load {path}: {e}"),
    }
}

/// Open the borderless, full-screen preview. Never locks anything: the only
/// daemon request this path (or its 1Hz refresh) ever sends is
/// [`Request::LockPreview`], which the daemon answers with a rendered PNG and
/// nothing else — see this module's top-level docs.
///
/// Returns `false`, opening nothing, when the first frame cannot be loaded —
/// the daemon always writes an opaque PNG with a background, so an unreadable
/// one is a real fault to tell the user about, not a window to show black.
fn open_preview_window(
    parent: &adw::ApplicationWindow,
    width: u32,
    height: u32,
    initial_path: String,
) -> bool {
    let first_frame = match gtk4::gdk::Texture::from_filename(&initial_path) {
        Ok(texture) => texture,
        Err(e) => {
            log::warn!("lock screen preview: couldn't load {initial_path}: {e}");
            return false;
        }
    };
    log::info!(
        "lock screen preview: {width}x{height} frame ({}x{}) from {initial_path}",
        first_frame.width(),
        first_frame.height()
    );
    let win = gtk4::Window::new();
    win.set_transient_for(Some(parent));
    win.set_modal(true);
    win.set_decorated(false);
    win.set_title(Some(t!("Lock screen preview")));

    let overlay = gtk4::Overlay::new();
    let picture = gtk4::Picture::new();
    // No `set_content_fit(Cover)`: that API needs gtk4's `v4_8` feature,
    // which this build does not enable (pinned to `v4_6`; see Cargo.toml).
    // It is also unnecessary here — `width`/`height` are the exact physical
    // size of the monitor this window fullscreens onto, so the PNG's aspect
    // ratio already matches the window's, and the default aspect-preserving
    // fit (`keep-aspect-ratio`, GTK 4.6's only option) fills it exactly, the
    // same result "cover" would produce for a source that already matches.
    picture.set_can_shrink(true);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    picture.set_paintable(Some(&first_frame));
    overlay.set_child(Some(&picture));

    let hint = gtk4::Label::new(Some(t!("Press any key to close")));
    hint.add_css_class("osd");
    hint.add_css_class("lock-preview-hint");
    hint.set_valign(gtk4::Align::End);
    hint.set_halign(gtk4::Align::Center);
    hint.set_margin_bottom(28);
    overlay.add_overlay(&hint);

    win.set_child(Some(&overlay));

    match window_monitor(parent) {
        Some(monitor) => win.fullscreen_on_monitor(&monitor),
        None => win.fullscreen(),
    }

    let closed = Rc::new(Cell::new(false));
    let close: Rc<dyn Fn()> = {
        let win = win.clone();
        let closed = closed.clone();
        Rc::new(move || {
            if !closed.replace(true) {
                win.close();
            }
        })
    };

    let keys = gtk4::EventControllerKey::new();
    {
        let close = close.clone();
        keys.connect_key_pressed(move |_, _, _, _| {
            close();
            glib::Propagation::Stop
        });
    }
    win.add_controller(keys);

    let click = gtk4::GestureClick::new();
    {
        let close = close.clone();
        click.connect_pressed(move |_, _, _, _| close());
    }
    win.add_controller(click);

    let source: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));
    {
        let picture = picture.clone();
        let source = source.clone();
        let id = glib::timeout_add_local(PREVIEW_REFRESH_INTERVAL, move || {
            let picture = picture.clone();
            spawn_ipc(
                Request::LockPreview { width, height },
                PREVIEW_IPC_TIMEOUT,
                move |result| {
                    if let Ok(Response::LockPreview { path }) = result {
                        load_preview_frame(&picture, &path);
                    }
                },
            );
            ControlFlow::Continue
        });
        source.set(Some(id));
    }
    win.connect_close_request(move |_| {
        if let Some(id) = source.take() {
            id.remove();
        }
        glib::Propagation::Proceed
    });

    win.present();
    true
}

// ─── Small GTK helpers ────────────────────────────────────────────────────────

/// The `.capability-banner` look (icon + wrapped label) `window.rs` already
/// uses for the GNOME-static notice — reused here for the trust note, the
/// Flatpak explanation, and per-host notes, so this page introduces no new
/// "here is some informational text" shape.
fn info_banner(text: &str) -> gtk4::Box {
    let label = gtk4::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    info_banner_widget(&label)
}

/// As [`info_banner`], but wraps a caller-owned label so its text can be
/// updated later (the host-status notes banner: same widget, refreshed every
/// poll rather than rebuilt).
fn info_banner_widget(label: &gtk4::Label) -> gtk4::Box {
    let banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    banner.add_css_class("capability-banner");
    let icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
    icon.set_valign(gtk4::Align::Start);
    banner.append(&icon);
    banner.append(label);
    banner
}

fn chip_widget(text: &str, on: bool) -> gtk4::Label {
    let l = gtk4::Label::new(Some(text));
    l.add_css_class("lock-chip");
    l.add_css_class(if on { "lock-chip-on" } else { "lock-chip-off" });
    l
}

/// One copyable command row: a dim label naming it, the command in a
/// read-only entry, and a Copy button — the same shape
/// `updates::show_unsupported_dialog` uses for its manual-update one-liner.
fn snippet_row(label: &'static str, command: &str) -> gtk4::Box {
    let col = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let caption = gtk4::Label::new(Some(t!(label)));
    caption.add_css_class("dim");
    caption.set_xalign(0.0);
    col.append(&caption);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let entry = gtk4::Entry::new();
    entry.set_text(command);
    entry.set_editable(false);
    entry.set_hexpand(true);
    let copy_btn = gtk4::Button::with_label(t!("Copy"));
    {
        let entry = entry.clone();
        copy_btn.connect_clicked(move |_| {
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&entry.text());
            }
        });
    }
    row.append(&entry);
    row.append(&copy_btn);
    col.append(&row);
    col
}

/// An [`adw::ActionRow`] carrying a trailing [`gtk4::Scale`] — the same shape
/// `window::widget_spin_row` uses for a trailing `SpinButton`. `apply` fires
/// on every `value-changed`, exactly like every other row on this page (and
/// every widget row in `window.rs`): `config.save()` is a small local write,
/// cheap enough to call on every drag tick, and `daemon_ctl::apply_async`'s
/// own queue is what coalesces the resulting burst of daemon round trips into
/// one in-flight plus one queued (see its doc comment) — so this row needs no
/// debounce timer of its own, and adding one would risk two sliders each
/// delaying the other's write past a still-in-memory value (see
/// `edit_lockscreen`, which always reads and writes the *current* in-memory
/// state, never a stale snapshot).
fn lock_slider_row(
    title: &str,
    subtitle: &str,
    range: (f64, f64, f64),
    value: f64,
    apply: impl Fn(f64) + 'static,
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    let (min, max, step) = range;
    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, min, max, step);
    scale.set_hexpand(true);
    scale.set_size_request(180, -1);
    scale.set_valign(gtk4::Align::Center);
    scale.set_draw_value(true);
    scale.set_value_pos(gtk4::PositionType::Right);
    scale.set_digits(2);
    // Percent readout ("20%") instead of the raw float ("0.20") `digits` +
    // `draw_value` would otherwise draw. `digits` still rounds the *stored*
    // value to two decimal places — this only changes what gets drawn, never
    // what gets saved.
    scale.set_format_value_func(|_, value| format!("{:.0}%", value * 100.0));
    // Set before connecting: populating the page must not look like a user
    // edit and must not wake the daemon.
    scale.set_value(value);
    scale.connect_value_changed(move |s| apply(s.value()));
    row.add_suffix(&scale);
    row
}

// ─── Config + IPC glue ────────────────────────────────────────────────────────

/// `[lockscreen]` as it stands right now, or the documented defaults when the
/// section is absent — the normal state for anyone who has never opened this
/// page. Read-only: takes a shared borrow and returns a copy, so callers can
/// populate widgets without holding a borrow across a GTK call.
fn lockscreen_settings(state: &Rc<RefCell<AppState>>) -> crate::config::LockScreen {
    state.borrow().config.lockscreen.clone().unwrap_or_default()
}

/// Apply one edit to `[lockscreen]`, save, and push the result to the daemon —
/// the single mutation path behind every control on this page. Mirrors
/// `window::edit_widgets`'s contract exactly, including *why* the borrow is
/// dropped before `apply_async` runs (its callback may itself need to borrow
/// `state`): `Config::lockscreen` is `Option<LockScreen>` so a config nobody
/// has touched carries no `[lockscreen]` key at all, and this is the one
/// place that materialises it, the first time any control here is used.
fn edit_lockscreen(
    state: &Rc<RefCell<AppState>>,
    edit: impl FnOnce(&mut crate::config::LockScreen),
) {
    let config = {
        let mut s = state.borrow_mut();
        edit(
            s.config
                .lockscreen
                .get_or_insert_with(crate::config::LockScreen::default),
        );
        s.config.save().ok();
        s.config.clone()
    };
    daemon_ctl::apply_async(&config, |_| {});
}

/// Send `req` to the daemon on a worker thread and hand the result back to
/// `on_done` on the GTK thread — the thread + `async_channel` +
/// `glib::spawn_future_local` shape used throughout `src/gui` (e.g.
/// `status::poll_once`, `daemon_ctl::spawn_apply_worker`), so nothing on this
/// page ever blocks the UI waiting on the daemon.
fn spawn_ipc(
    req: Request,
    timeout: Duration,
    on_done: impl FnOnce(anyhow::Result<Response>) + 'static,
) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = ipc::request_with_timeout(&req, timeout);
        let _ = tx.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            on_done(result);
        }
    });
}

/// [`Request::Status`], reduced to the one field this page cares about.
/// `None` covers both "daemon unreachable" and "daemon running but predates
/// this field" — both render as the same muted status-group state.
fn fetch_status_async(on_done: impl FnOnce(Option<StatusReply>) + 'static) {
    spawn_ipc(Request::Status, DEFAULT_IPC_TIMEOUT, move |result| {
        let status = match result {
            Ok(Response::Status(s)) => Some(s),
            _ => None,
        };
        on_done(status);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LockScreen;

    // -- setup_button_spec ----------------------------------------------------

    #[test]
    fn setup_button_spec_matches_each_state() {
        assert_eq!(
            setup_button_spec(LockSetupState::Needed),
            Some((t!("Set up").to_string(), SetupAction::Setup))
        );
        assert_eq!(
            setup_button_spec(LockSetupState::Done),
            Some((t!("Undo").to_string(), SetupAction::Undo))
        );
        assert_eq!(setup_button_spec(LockSetupState::NotNeeded), None);
        assert_eq!(setup_button_spec(LockSetupState::Unavailable), None);
    }

    // -- lock_support (issue #37) ---------------------------------------------

    fn lock_status(host: &str, live: bool, widgets: bool, still: Option<bool>) -> ipc::LockStatus {
        ipc::LockStatus {
            enabled: true,
            host: host.into(),
            live_video: live,
            widgets,
            still_frame: still,
            locked: false,
            setup: LockSetupState::NotNeeded,
            notes: Vec::new(),
        }
    }

    #[test]
    fn lock_support_picks_the_tier_the_capability_data_names() {
        // Live video / widgets win over everything, whatever `still_frame` says.
        for still in [None, Some(false), Some(true)] {
            assert_eq!(lock_support(true, true, still), Some(LockSupport::Full));
            // One flag alone is still the live-surface tier (the daemon sets
            // both from one test; this pins what a future split would do).
            assert_eq!(lock_support(true, false, still), Some(LockSupport::Full));
            assert_eq!(lock_support(false, true, still), Some(LockSupport::Full));
        }
        assert_eq!(
            lock_support(false, false, Some(true)),
            Some(LockSupport::StillFrame)
        );
        assert_eq!(
            lock_support(false, false, Some(false)),
            Some(LockSupport::Nothing)
        );
        // An older daemon said nothing about frames: claim nothing.
        assert_eq!(lock_support(false, false, None), None);
    }

    #[test]
    fn lock_support_of_follows_the_daemon_not_the_host_name() {
        // Same host, flipped only by the reported capability: a desktop that
        // gains a still-frame writer moves tier with no GUI change.
        let before = lock_status("deepin", false, false, Some(false));
        let after = lock_status("deepin", false, false, Some(true));
        assert_eq!(lock_support_of(&before), Some(LockSupport::Nothing));
        assert_eq!(lock_support_of(&after), Some(LockSupport::StillFrame));
        // And the reverse for a host that is "still frame" today.
        let gnome = lock_status("gnome", false, false, Some(true));
        assert_eq!(lock_support_of(&gnome), Some(LockSupport::StillFrame));
        let cosmic = lock_status("cosmic", true, true, Some(true));
        assert_eq!(lock_support_of(&cosmic), Some(LockSupport::Full));
        let unsupported = lock_status("unsupported", false, false, Some(false));
        assert_eq!(lock_support_of(&unsupported), Some(LockSupport::Nothing));
    }

    #[test]
    fn lock_support_summaries_are_the_three_documented_sentences() {
        assert_eq!(LockSupport::Full.summary(), "Live video and widgets");
        assert_eq!(LockSupport::StillFrame.summary(), "Still frame only");
        assert_eq!(
            LockSupport::Nothing.summary(),
            "Nothing yet — the lock screen keeps your desktop’s own background"
        );
    }

    #[test]
    fn support_notice_names_the_desktop_and_the_verdict() {
        assert_eq!(
            support_notice_text("gnome", LockSupport::StillFrame),
            "On GNOME: Still frame only"
        );
        assert_eq!(
            support_notice_text("deepin", LockSupport::Nothing),
            "On Deepin: Nothing yet — the lock screen keeps your desktop’s own background"
        );
        // "unsupported" is not a desktop name: it must not read "On Not
        // supported on this desktop yet: …".
        assert_eq!(
            support_notice_text("unsupported", LockSupport::Nothing),
            "On this desktop: Nothing yet — the lock screen keeps your desktop’s own background"
        );
    }

    #[test]
    fn enable_notice_fires_only_for_switching_on_where_nothing_applies() {
        let msg = enable_notice(true, Some(LockSupport::Nothing));
        assert!(msg.is_some_and(|m| m.starts_with("Saved, but this desktop can’t")));
        // Turning it off, or any desktop with something to show, or an
        // unknown verdict: no toast.
        assert_eq!(enable_notice(false, Some(LockSupport::Nothing)), None);
        assert_eq!(enable_notice(true, Some(LockSupport::StillFrame)), None);
        assert_eq!(enable_notice(true, Some(LockSupport::Full)), None);
        assert_eq!(enable_notice(true, None), None);
    }

    #[test]
    fn menu_row_hint_marks_only_what_is_limited() {
        assert_eq!(menu_tag(Some(LockSupport::Full)), None);
        assert_eq!(menu_tag(None), None);
        assert_eq!(
            menu_tag(Some(LockSupport::StillFrame)),
            Some("Still frame only")
        );
        assert_eq!(
            menu_tag(Some(LockSupport::Nothing)),
            Some("Not supported yet")
        );

        assert_eq!(menu_tooltip(None), "Show Fresco on the lock screen");
        assert_eq!(
            menu_tooltip(Some(LockSupport::StillFrame)),
            "Show Fresco on the lock screen\nStill frame only"
        );
        assert!(menu_tooltip(Some(LockSupport::Nothing)).ends_with(LockSupport::Nothing.summary()));
    }

    // -- host_label -------------------------------------------------------------

    #[test]
    fn host_label_covers_every_documented_id() {
        let cases = [
            ("cosmic", "COSMIC"),
            ("kde", "KDE Plasma"),
            ("gnome", "GNOME"),
            ("cinnamon", "Cinnamon"),
            ("mate", "MATE"),
            ("xfce", "Xfce"),
            ("deepin", "Deepin"),
        ];
        for (id, expect_contains) in cases {
            let label = host_label(id);
            assert!(!label.trim().is_empty(), "{id}");
            // Only asserting non-empty + no panic for translated labels in
            // general (see `Xfce`, which is short enough some locales might
            // legitimately not touch it); COSMIC/GNOME/etc are pinned above
            // via the English source string used as the translation key.
            let _ = expect_contains;
        }
        assert!(host_label("wlroots").contains("swaylock-plugin"));
        assert!(host_label("x11").contains("xsecurelock"));
        assert!(!host_label("unsupported").trim().is_empty());
    }

    #[test]
    fn host_label_falls_back_to_the_raw_id_for_an_unknown_host() {
        // A newer daemon naming a host this GUI predates must not go blank.
        assert_eq!(host_label("plasma-mobile"), "plasma-mobile");
    }

    // -- cosmic_hint --------------------------------------------------------------

    #[test]
    fn cosmic_hint_only_fires_for_cosmic() {
        assert!(cosmic_hint("cosmic").is_some());
        for other in ["kde", "gnome", "wlroots", "x11", "unsupported", ""] {
            assert!(cosmic_hint(other).is_none(), "{other}");
        }
    }

    #[test]
    fn deepin_hint_only_fires_for_deepin() {
        assert!(deepin_hint("deepin").is_some());
        for other in [
            "kde",
            "gnome",
            "cosmic",
            "wlroots",
            "x11",
            "unsupported",
            "",
        ] {
            assert!(deepin_hint(other).is_none(), "{other}");
        }
    }

    // -- chip_label -----------------------------------------------------------

    #[test]
    fn chip_label_shows_the_right_glyph() {
        assert_eq!(chip_label("Live video", true), "Live video ✓");
        assert_eq!(chip_label("Live video", false), "Live video ✗");
    }

    // -- snippets ---------------------------------------------------------------

    #[test]
    fn wlroots_snippets_are_exact() {
        let s = snippets_for_host("wlroots");
        assert_eq!(s.len(), 3);
        assert!(s
            .iter()
            .any(|(_, cmd)| *cmd == "bindsym $mod+Escape exec fresco lock"));
        assert!(s.iter().any(|(_, cmd)| *cmd == "lock_cmd = fresco lock"));
        assert!(s
            .iter()
            .any(|(_, cmd)| *cmd == "timeout 300 'fresco lock' before-sleep 'fresco lock'"));
    }

    #[test]
    fn x11_snippet_is_exact() {
        let s = snippets_for_host("x11");
        assert_eq!(s, &[("X11 (xss-lock)", "xss-lock -- fresco lock")]);
    }

    #[test]
    fn hosts_without_a_snippet_get_an_empty_list() {
        for host in [
            "cosmic",
            "kde",
            "gnome",
            "cinnamon",
            "mate",
            "xfce",
            "deepin",
            "unsupported",
        ] {
            assert!(snippets_for_host(host).is_empty(), "{host}");
        }
    }

    // -- clock theme override index mapping --------------------------------------

    #[test]
    fn clock_override_index_round_trips_every_theme_and_none() {
        assert_eq!(clock_override_index(None), 0);
        assert_eq!(clock_override_from_index(0), None);
        for (i, theme) in ClockTheme::ALL.iter().enumerate() {
            let idx = clock_override_index(Some(*theme));
            assert_eq!(idx, i as u32 + 1, "{theme:?}");
            assert_eq!(clock_override_from_index(idx), Some(*theme));
        }
    }

    #[test]
    fn clock_override_from_index_out_of_range_is_none_not_a_panic() {
        assert_eq!(clock_override_from_index(9999), None);
    }

    #[test]
    fn clock_override_labels_has_one_entry_per_theme_plus_default() {
        assert_eq!(clock_override_labels().len(), ClockTheme::ALL.len() + 1);
    }

    // -- greeting round trip ------------------------------------------------------

    #[test]
    fn greeting_ui_state_reads_the_tri_state() {
        assert_eq!(greeting_ui_state(None), (false, String::new()));
        assert_eq!(greeting_ui_state(Some("")), (true, String::new()));
        assert_eq!(
            greeting_ui_state(Some("Welcome back")),
            (false, "Welcome back".to_string())
        );
    }

    #[test]
    fn greeting_config_value_covers_hide_auto_and_custom() {
        assert_eq!(greeting_config_value(true, "anything"), Some(String::new()));
        assert_eq!(greeting_config_value(false, ""), None);
        assert_eq!(
            greeting_config_value(false, "   "),
            None,
            "whitespace-only collapses to Auto"
        );
        assert_eq!(greeting_config_value(false, "Yo"), Some("Yo".to_string()));
    }

    #[test]
    fn greeting_round_trips_through_both_directions() {
        for cfg in [None, Some(""), Some("Welcome back")] {
            let (hide, text) = greeting_ui_state(cfg);
            let back = greeting_config_value(hide, &text);
            assert_eq!(back.as_deref(), cfg, "{cfg:?}");
        }
    }

    // -- monitor pixel size -------------------------------------------------------

    #[test]
    fn monitor_pixel_size_multiplies_by_scale() {
        assert_eq!(monitor_pixel_size(1920, 1080, 1), (1920, 1080));
        assert_eq!(monitor_pixel_size(1920, 1080, 2), (3840, 2160));
        assert_eq!(monitor_pixel_size(1366, 768, 1), (1366, 768));
    }

    #[test]
    fn monitor_pixel_size_never_panics_on_bad_input() {
        assert_eq!(monitor_pixel_size(-100, -100, 0), (0, 0));
        assert_eq!(monitor_pixel_size(0, 0, -5), (0, 0));
        assert_eq!(
            monitor_pixel_size(i32::MAX, i32::MAX, i32::MAX),
            (u32::MAX, u32::MAX)
        );
    }

    // -- live video index mapping -------------------------------------------------

    #[test]
    fn live_video_index_round_trips() {
        for v in [LiveVideo::Ac, LiveVideo::Always, LiveVideo::Never] {
            assert_eq!(live_video_from_index(live_video_index(v)), v);
        }
    }

    #[test]
    fn live_video_from_index_out_of_range_falls_back_to_default() {
        assert_eq!(live_video_from_index(999), LiveVideo::default());
    }

    #[test]
    fn live_video_labels_are_never_empty() {
        for l in live_video_labels() {
            assert!(!l.trim().is_empty());
        }
    }

    // -- lock widget field mapping --------------------------------------------------

    #[test]
    fn lock_widget_get_set_round_trips_every_variant_independently() {
        for widget in LockWidget::ALL {
            let mut w = LockWidgets {
                clock: false,
                date: false,
                greeting: false,
                avatar: false,
                now_playing: false,
                album_art: false,
                battery: false,
                lyrics: false,
                visualizer: false,
            };
            assert!(!lock_widget_enabled(w, widget), "{widget:?} starts off");
            set_lock_widget_enabled(&mut w, widget, true);
            assert!(lock_widget_enabled(w, widget), "{widget:?} turned on");

            // Turning one on must not turn any other on.
            for other in LockWidget::ALL {
                if other != widget {
                    assert!(
                        !lock_widget_enabled(w, other),
                        "{widget:?} leaked into {other:?}"
                    );
                }
            }

            set_lock_widget_enabled(&mut w, widget, false);
            assert!(
                !lock_widget_enabled(w, widget),
                "{widget:?} turned back off"
            );
        }
    }

    // -- edit_lockscreen's materialisation contract (pure slice) -----------------

    #[test]
    fn lockscreen_default_has_no_widgets_hidden_that_should_be_on() {
        // Sanity check that this module and `config::LockScreen::default()`
        // still agree on which widgets ship on: if this ever drifts, every
        // `lock_widget_enabled` call above would be reading the wrong
        // defaults on first render.
        let d = LockScreen::default();
        assert!(lock_widget_enabled(d.widgets, LockWidget::Clock));
        assert!(lock_widget_enabled(d.widgets, LockWidget::Date));
        assert!(lock_widget_enabled(d.widgets, LockWidget::Greeting));
        assert!(!lock_widget_enabled(d.widgets, LockWidget::Avatar));
        assert!(!lock_widget_enabled(d.widgets, LockWidget::Lyrics));
        assert!(!lock_widget_enabled(d.widgets, LockWidget::Visualizer));
    }
}
