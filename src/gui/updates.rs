use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use super::window::{glass_dialog, show_toast, AppState};

// ─── "What's new" on-update notice ────────────────────────────────────────────

/// The CHANGELOG, embedded at build time so the notes ship inside the binary.
const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// Extract the changelog section for `version` (Keep a Changelog format:
/// `## [x.y.z] …` up to the next `## [`).
fn changelog_for(version: &str) -> Option<String> {
    let header = format!("## [{version}]");
    let start = CHANGELOG.find(&header)?;
    let rest = &CHANGELOG[start..];
    let body = &rest[header.len()..];
    let end = body
        .find("\n## [")
        .map(|i| header.len() + i)
        .unwrap_or(rest.len());
    let section = rest[..end].trim();
    if section.is_empty() {
        None
    } else {
        Some(section.to_string())
    }
}

/// A dismissible banner shown once per version after the app updates. Returns
/// None if the current version's notes have already been seen (or are absent).
pub(crate) fn build_update_banner(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) -> Option<gtk4::Widget> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    if state.borrow().config.last_seen_version == current {
        return None;
    }
    let notes = changelog_for(&current)?;

    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    bar.add_css_class("banner");
    bar.set_margin_start(16);
    bar.set_margin_end(16);
    bar.set_margin_top(8);

    let icon = gtk4::Image::from_icon_name("software-update-available-symbolic");
    bar.append(&icon);

    let label = gtk4::Label::new(Some(&format!("Fresco updated to {current}")));
    label.set_hexpand(true);
    label.set_xalign(0.0);

    let details = gtk4::Button::with_label("See what's new");
    details.add_css_class("suggested-action");

    let close = gtk4::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    close.set_tooltip_text(Some("Dismiss"));

    bar.append(&label);
    bar.append(&details);
    bar.append(&close);

    // Mark the current version seen and persist it.
    let mark_seen = {
        let state = state.clone();
        let current = current.clone();
        move || {
            let mut s = state.borrow_mut();
            if s.config.last_seen_version != current {
                s.config.last_seen_version = current.clone();
                s.config.save().ok();
            }
        }
    };

    {
        let bar = bar.clone();
        let win = window.clone();
        let mark = mark_seen.clone();
        let version = current.clone();
        details.connect_clicked(move |_| {
            show_changelog_modal(&win, &version, &notes);
            mark();
            bar.set_visible(false);
        });
    }
    {
        let bar = bar.clone();
        close.connect_clicked(move |_| {
            mark_seen();
            bar.set_visible(false);
        });
    }

    Some(bar.upcast())
}

/// Modal showing the changelog notes for `version`.
/// One parsed changelog block.
enum Note {
    /// A `### Section` heading.
    Section(String),
    /// A `- ` bullet (may have spanned several wrapped source lines).
    Bullet(String),
    /// A plain paragraph.
    Para(String),
}

/// Parse a Keep-a-Changelog section into blocks, coalescing soft-wrapped
/// source lines back into single bullets/paragraphs and dropping the redundant
/// `## [version]` header (the modal title already shows it).
fn parse_notes(notes: &str) -> Vec<Note> {
    let mut blocks: Vec<Note> = Vec::new();
    let mut cur: Option<Note> = None;
    let flush = |cur: &mut Option<Note>, blocks: &mut Vec<Note>| {
        if let Some(b) = cur.take() {
            blocks.push(b);
        }
    };
    for raw in notes.lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush(&mut cur, &mut blocks);
        } else if let Some(rest) = line.strip_prefix("### ") {
            flush(&mut cur, &mut blocks);
            blocks.push(Note::Section(rest.to_string()));
        } else if line.starts_with("## ") {
            flush(&mut cur, &mut blocks); // version header — title already shows it
        } else if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush(&mut cur, &mut blocks);
            cur = Some(Note::Bullet(rest.to_string()));
        } else {
            match &mut cur {
                Some(Note::Bullet(s)) | Some(Note::Para(s)) => {
                    s.push(' ');
                    s.push_str(line);
                }
                _ => cur = Some(Note::Para(line.to_string())),
            }
        }
    }
    flush(&mut cur, &mut blocks);
    blocks
}

/// Wrap each `delim`-delimited span in `open`/`close` tags, toggling on/off.
fn wrap_pairs(s: &str, delim: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut on = false;
    for (i, seg) in s.split(delim).enumerate() {
        if i > 0 {
            out.push_str(if on { close } else { open });
            on = !on;
        }
        out.push_str(seg);
    }
    if on {
        out.push_str(close);
    }
    out
}

/// Render inline markdown (`**bold**`, `*italic*`) as Pango markup, escaping
/// the rest and normalising em/en dashes to plain hyphens.
fn pango_inline(raw: &str) -> String {
    let normalised = raw.replace(['—', '–'], "-");
    let escaped = glib::markup_escape_text(&normalised).to_string();
    let bolded = wrap_pairs(&escaped, "**", "<b>", "</b>");
    wrap_pairs(&bolded, "*", "<i>", "</i>")
}

/// Modal showing the changelog notes for `version`, rendered as styled widgets
/// rather than raw markdown text.
pub(crate) fn show_changelog_modal(window: &adw::ApplicationWindow, version: &str, notes: &str) {
    let (dialog, content) = glass_dialog(window, &format!("What's new in {version}"), 660, 680);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    list.set_margin_start(28);
    list.set_margin_end(28);
    list.set_margin_top(14);
    list.set_margin_bottom(28);

    // Prominent version title at the top of the notes.
    let title = gtk4::Label::new(Some(&format!("Version {version}")));
    title.add_css_class("changelog-title");
    title.set_xalign(0.0);
    title.set_margin_bottom(2);
    list.append(&title);

    for note in parse_notes(notes) {
        match note {
            Note::Section(text) => {
                let h = gtk4::Label::new(Some(&text.to_uppercase()));
                h.add_css_class("changelog-section");
                h.set_xalign(0.0);
                h.set_halign(gtk4::Align::Start);
                h.set_margin_top(16);
                list.append(&h);
            }
            Note::Bullet(text) => {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                let bullet = gtk4::Label::new(Some("•"));
                bullet.add_css_class("dim");
                bullet.add_css_class("changelog-body");
                bullet.set_valign(gtk4::Align::Start);
                let body = gtk4::Label::new(None);
                body.add_css_class("changelog-body");
                body.set_markup(&pango_inline(&text));
                body.set_wrap(true);
                body.set_xalign(0.0);
                body.set_hexpand(true);
                row.append(&bullet);
                row.append(&body);
                list.append(&row);
            }
            Note::Para(text) => {
                let p = gtk4::Label::new(None);
                p.add_css_class("changelog-body");
                p.set_markup(&pango_inline(&text));
                p.set_wrap(true);
                p.set_xalign(0.0);
                list.append(&p);
            }
        }
    }

    scroll.set_child(Some(&list));
    content.append(&scroll);

    dialog.present();
}

// ─── Self-driven "Update available" check ─────────────────────────────────────

/// Skip a background check if the last one was within this long ago.
const UPDATE_CHECK_INTERVAL_S: u64 = 24 * 60 * 60;

/// One-liner shown when auto-install isn't supported (Flatpak / no apt-get).
const INSTALL_ONELINER: &str =
    "curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | bash";
const RELEASES_URL: &str = "https://github.com/DibbayajyotiRoy/fresco/releases/latest";

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

thread_local! {
    /// True while a release fetch is in flight, so the automatic startup check
    /// and the manual menu action can't race duplicate GitHub calls.
    static CHECK_IN_FLIGHT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Check GitHub Releases for a newer version and, if found, populate the
/// "Update available" banner slot. Mirrors `poll_notifications`'s
/// thread + `async_channel` + `glib::spawn_future_local` pattern so the network
/// call never blocks the GTK main thread.
///
/// `force` bypasses the 24h throttle (used by the manual "Check for updates"
/// menu item) and surfaces an explicit toast either way (latest / offline).
/// The automatic startup check (`force = false`) stays silent on both the
/// throttle skip and any failure.
pub(crate) fn check_for_updates(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    force: bool,
) {
    if !force {
        let last = state.borrow().config.last_update_check;
        if unix_now().saturating_sub(last) < UPDATE_CHECK_INTERVAL_S {
            return;
        }
    }
    if CHECK_IN_FLIGHT.get() {
        return;
    }
    CHECK_IN_FLIGHT.set(true);

    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = crate::update::fetch_latest();
        let _ = tx.send_blocking(result);
    });

    let window = window.clone();
    glib::spawn_future_local(async move {
        let result = rx.recv().await;
        CHECK_IN_FLIGHT.set(false);
        let Ok(result) = result else {
            return;
        };

        {
            let mut s = state.borrow_mut();
            s.config.last_update_check = unix_now();
            s.config.save().ok();
        }

        let latest = match result {
            Ok(latest) => latest,
            Err(e) => {
                log::warn!("update check failed: {e:#}");
                if force {
                    show_toast(
                        &state,
                        "Couldn't check for updates — check your connection.",
                    );
                }
                return;
            }
        };

        let current = crate::update::current_version();
        if !crate::update::is_newer(&latest.version, current) {
            if force {
                show_toast(&state, &format!("You're on the latest version ({current})"));
            }
            return;
        }

        if state.borrow().config.update_skipped_version == latest.version {
            return; // user already dismissed this version with "Later"
        }

        show_update_banner(&window, state, latest);
    });
}

/// Build the "Fresco X.Y.Z is available" banner into the already-inserted
/// `update_banner_slot` (see `build_library_view`).
fn show_update_banner(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    latest: crate::update::LatestRelease,
) {
    let Some(slot) = state.borrow().update_banner_slot.clone() else {
        return;
    };
    // Clear anything previously populated (e.g. a re-check while one is shown).
    while let Some(child) = slot.first_child() {
        slot.remove(&child);
    }

    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    bar.add_css_class("banner");
    bar.set_margin_start(16);
    bar.set_margin_end(16);
    bar.set_margin_top(8);

    let icon = gtk4::Image::from_icon_name("software-update-available-symbolic");
    bar.append(&icon);

    let label = gtk4::Label::new(Some(&format!("Fresco {} is available", latest.version)));
    label.set_hexpand(true);
    label.set_xalign(0.0);
    bar.append(&label);

    let whats_new = gtk4::Button::with_label("What's new");
    bar.append(&whats_new);

    let update_now = gtk4::Button::with_label("Update now");
    update_now.add_css_class("suggested-action");
    bar.append(&update_now);

    let later = gtk4::Button::from_icon_name("window-close-symbolic");
    later.add_css_class("flat");
    later.set_tooltip_text(Some("Later"));
    bar.append(&later);

    {
        let win = window.clone();
        let version = latest.version.clone();
        let notes_url = latest.notes_url.clone();
        whats_new.connect_clicked(move |_| {
            if let Some(notes) = changelog_for(&version) {
                show_changelog_modal(&win, &version, &notes);
            } else {
                let _ = gio::AppInfo::launch_default_for_uri(
                    &notes_url,
                    None::<&gio::AppLaunchContext>,
                );
            }
        });
    }
    {
        let bar = bar.clone();
        let state = state.clone();
        let version = latest.version.clone();
        later.connect_clicked(move |_| {
            let mut s = state.borrow_mut();
            s.config.update_skipped_version = version.clone();
            s.config.save().ok();
            bar.set_visible(false);
        });
    }
    {
        let bar = bar.clone();
        let win = window.clone();
        let version = latest.version.clone();
        update_now.connect_clicked(move |_| {
            bar.set_visible(false);
            show_install_dialog(&win, version.clone());
        });
    }

    slot.append(&bar);
}

// ─── Install dialog ────────────────────────────────────────────────────────────

/// Modal driving the actual update: a spinner + staged status label while the
/// updater script runs, then a final Success/Failed/Unsupported state.
fn show_install_dialog(window: &adw::ApplicationWindow, version: String) {
    let (dialog, content) = glass_dialog(window, &format!("Updating to {version}"), 420, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);
    inner.set_halign(gtk4::Align::Center);

    let spinner = gtk4::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(32, 32);
    inner.append(&spinner);

    let status = gtk4::Label::new(Some("Preparing…"));
    status.set_wrap(true);
    status.set_xalign(0.5);
    inner.append(&status);

    content.append(&inner);
    dialog.present();

    // Flatpak installs can never apt-install: go straight to the fallback
    // dialog without ever invoking pkexec.
    if crate::is_flatpak() {
        dialog.close();
        show_unsupported_dialog(window);
        return;
    }

    let (tx, rx) = async_channel::bounded::<UpdateProgress>(8);
    std::thread::spawn(move || {
        let tx_stage = tx.clone();
        let outcome = crate::update::run_updater_with_progress(move |stage| {
            let _ = tx_stage.send_blocking(UpdateProgress::Stage(stage));
        });
        let _ = tx.send_blocking(UpdateProgress::Done(outcome));
    });

    let window = window.clone();
    glib::spawn_future_local(async move {
        while let Ok(progress) = rx.recv().await {
            match progress {
                UpdateProgress::Stage(stage) => {
                    status.set_label(friendly_stage(&stage));
                }
                UpdateProgress::Done(outcome) => {
                    finish_install_dialog(&window, &dialog, &content, outcome);
                    break;
                }
            }
        }
    });
}

/// One message crossing the background→main-thread channel while the updater runs.
enum UpdateProgress {
    Stage(String),
    Done(crate::update::UpdateOutcome),
}

/// Map a raw `STAGE: x` payload to user-facing text.
fn friendly_stage(stage: &str) -> &'static str {
    match stage {
        "downloading" => "Downloading…",
        "installing" => "Installing…",
        "done" => "Done",
        _ => "Working…",
    }
}

/// Replace the install dialog's content with its final state once the updater
/// process exits.
fn finish_install_dialog(
    window: &adw::ApplicationWindow,
    dialog: &adw::Window,
    content: &gtk4::Box,
    outcome: crate::update::UpdateOutcome,
) {
    match outcome {
        crate::update::UpdateOutcome::Success => {
            replace_dialog_body(content, |inner| {
                let heading = gtk4::Label::new(Some("Updated — restart to apply"));
                heading.add_css_class("dialog-heading");
                heading.set_wrap(true);
                inner.append(&heading);

                let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                buttons.set_margin_top(6);
                let later = gtk4::Button::with_label("Later");
                later.add_css_class("flat");
                let restart = gtk4::Button::with_label("Restart now");
                restart.add_css_class("suggested-action");
                buttons.append(&later);
                buttons.append(&restart);
                inner.append(&buttons);

                {
                    let d = dialog.clone();
                    later.connect_clicked(move |_| d.close());
                }
                {
                    let win = window.clone();
                    let d = dialog.clone();
                    restart.connect_clicked(move |_| {
                        relaunch_app(&win);
                        d.close();
                    });
                }
            });
        }
        crate::update::UpdateOutcome::AlreadyUpToDate => {
            replace_dialog_body(content, |inner| {
                let heading = gtk4::Label::new(Some("You're already on the latest version"));
                heading.add_css_class("dialog-heading");
                heading.set_wrap(true);
                inner.append(&heading);

                let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                buttons.set_margin_top(6);
                buttons.set_halign(gtk4::Align::End);
                let close = gtk4::Button::with_label("Close");
                close.add_css_class("flat");
                buttons.append(&close);
                inner.append(&buttons);

                let d = dialog.clone();
                close.connect_clicked(move |_| d.close());
            });
        }
        crate::update::UpdateOutcome::Unsupported => {
            dialog.close();
            show_unsupported_dialog(window);
        }
        crate::update::UpdateOutcome::Failed(msg) => {
            log::warn!("update install failed: {msg}");
            replace_dialog_body(content, |inner| {
                let heading = gtk4::Label::new(Some("Update failed"));
                heading.add_css_class("dialog-heading");
                inner.append(&heading);

                let body = gtk4::Label::new(Some(&msg));
                body.set_wrap(true);
                body.set_xalign(0.0);
                body.set_selectable(true);
                inner.append(&body);

                let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                buttons.set_margin_top(6);
                buttons.set_halign(gtk4::Align::End);
                let close = gtk4::Button::with_label("Close");
                close.add_css_class("flat");
                buttons.append(&close);
                inner.append(&buttons);

                let d = dialog.clone();
                close.connect_clicked(move |_| d.close());
            });
        }
    }
}

/// Remove the dialog's content (below the header bar) and let `build` add a
/// fresh body, reusing the existing dialog window/header instead of opening a
/// second modal.
fn replace_dialog_body(content: &gtk4::Box, build: impl FnOnce(&gtk4::Box)) {
    // Keep the first child (the HeaderBar), drop everything after it.
    let mut child = content.first_child();
    let mut first = true;
    while let Some(c) = child {
        let next = c.next_sibling();
        if !first {
            content.remove(&c);
        }
        first = false;
        child = next;
    }
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);
    build(&inner);
    content.append(&inner);
}

/// Relaunch the app as a new detached process, then quit this one so the
/// freshly-installed binary takes over.
fn relaunch_app(window: &adw::ApplicationWindow) {
    if let Ok(exe) = std::env::current_exe() {
        if let Err(e) = std::process::Command::new(exe).spawn() {
            log::warn!("failed to relaunch fresco: {e}");
            return;
        }
    }
    if let Some(app) = window.application() {
        app.quit();
    }
}

/// Fallback shown when auto-install isn't supported here (Flatpak sandbox, or
/// no apt-get): a copyable one-liner plus a link to the releases page.
fn show_unsupported_dialog(window: &adw::ApplicationWindow) {
    let (dialog, content) = glass_dialog(window, "Update manually", 460, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    let body = gtk4::Label::new(Some(
        "This install can't be updated automatically. Run this command in a terminal, or grab the latest release directly:",
    ));
    body.set_wrap(true);
    body.set_xalign(0.0);
    inner.append(&body);

    let copy_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let entry = gtk4::Entry::new();
    entry.set_text(INSTALL_ONELINER);
    entry.set_editable(false);
    entry.set_hexpand(true);
    let copy_btn = gtk4::Button::with_label("Copy");
    {
        let entry = entry.clone();
        copy_btn.connect_clicked(move |_| {
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&entry.text());
            }
        });
    }
    copy_row.append(&entry);
    copy_row.append(&copy_btn);
    inner.append(&copy_row);

    let releases_link = gtk4::LinkButton::with_label(RELEASES_URL, "Open releases page");
    releases_link.set_halign(gtk4::Align::Start);
    inner.append(&releases_link);

    content.append(&inner);
    dialog.present();
}
