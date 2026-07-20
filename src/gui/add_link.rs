//! "Add from link" dialog: paste a Pinterest (pin.it / pinterest.com) or
//! direct media URL, resolve it to a downloadable media URL off the main
//! thread, then pull it into the library through the same downloader and
//! post-add path as "Add from URL". Kept out of window.rs because the
//! resolve→dedupe→download pipeline has enough states to deserve its own file.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use super::library::{self, save_entries};
use super::window::{glass_dialog, show_toast, AppState};
use crate::linkresolve::{MediaKind, ResolvedMedia};

/// Same cap as the "Add from URL" flow: refuse >1 GB outright.
const MAX_BYTES: u64 = 1_000_000_000;

/// Loose pre-check used only to decide whether clipboard text is worth
/// prefilling; the resolver is the real validator.
fn looks_like_supported_url(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("http")
        && (t.contains("pin.it")
            || t.contains("pinterest.com")
            || crate::download::media_filename(t).is_some())
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Rename the downloaded file after the resolved title/pin id. The pin id goes
/// into the name on purpose: it is what the dedupe check looks for next time.
/// Renaming is best-effort — a failure keeps the URL-derived name, which is
/// still a valid library entry.
fn rename_to_resolved(path: PathBuf, resolved: &ResolvedMedia) -> PathBuf {
    let stem = match (&resolved.title, &resolved.pin_id) {
        (Some(t), Some(id)) => format!("{}-{id}", sanitize(t.trim())),
        (Some(t), None) => sanitize(t.trim()),
        (None, Some(id)) => format!("pin-{id}"),
        (None, None) => return path,
    };
    let stem = stem.trim_matches('_');
    if stem.is_empty() {
        return path;
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            match resolved.kind {
                MediaKind::Video => "mp4",
                MediaKind::Image => "jpg",
                MediaKind::Gif => "gif",
            }
            .to_string()
        });
    let dir = path.parent().unwrap_or(Path::new("."));
    let target = (0..)
        .map(|i| {
            if i == 0 {
                dir.join(format!("{stem}.{ext}"))
            } else {
                dir.join(format!("{stem}-{i}.{ext}"))
            }
        })
        .find(|p| !p.exists())
        .expect("some suffix is free");
    match std::fs::rename(&path, &target) {
        Ok(()) => target,
        Err(_) => path,
    }
}

/// File names already in the library, so the worker thread can dedupe by pin
/// id without touching GTK state.
fn entry_file_names(state: &Rc<RefCell<AppState>>) -> Vec<String> {
    let s = state.borrow();
    s.entries
        .iter()
        .flat_map(|e| {
            e.path
                .iter()
                .chain(e.paths.iter())
                .chain(e.folder.iter())
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn show_add_link_dialog(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
) {
    let (dialog, content) = glass_dialog(window, "Add from link", 440, -1);
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_start(20);
    inner.set_margin_end(20);
    inner.set_margin_bottom(18);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("Paste a Pinterest or direct video/image link"));
    inner.append(&entry);

    let error = gtk4::Label::new(None);
    error.add_css_class("error");
    error.add_css_class("dim");
    error.set_wrap(true);
    error.set_xalign(0.0);
    error.set_visible(false);
    inner.append(&error);

    let status = gtk4::Label::new(None);
    status.add_css_class("shimmer");
    status.set_xalign(0.0);
    status.set_visible(false);
    inner.append(&status);

    let progress = gtk4::ProgressBar::new();
    progress.add_css_class("update-progress");
    progress.set_hexpand(true);
    progress.set_visible(false);
    inner.append(&progress);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.set_halign(gtk4::Align::End);
    let cancel_btn = gtk4::Button::with_label("Cancel");
    let add_btn = gtk4::Button::with_label("Add");
    add_btn.add_css_class("suggested-action");
    row.append(&cancel_btn);
    row.append(&add_btn);
    inner.append(&row);
    content.append(&inner);

    // Prefill from the clipboard when it already holds a plausible link, so
    // the common copy→open-dialog path is one click.
    {
        let entry = entry.clone();
        gtk4::prelude::WidgetExt::display(window)
            .clipboard()
            .read_text_async(None::<&gio::Cancellable>, move |res| {
                if let Ok(Some(text)) = res {
                    if entry.text().is_empty() && looks_like_supported_url(&text) {
                        entry.set_text(text.trim());
                    }
                }
            });
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let d = dialog.clone();
        let flag = cancel_flag.clone();
        cancel_btn.connect_clicked(move |_| {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            d.close();
        });
    }

    {
        let state = state.clone();
        let dialog = dialog.clone();
        let entry_w = entry.clone();
        let error = error.clone();
        let status = status.clone();
        let progress = progress.clone();
        let flag = cancel_flag;
        add_btn.connect_clicked(move |btn| {
            let url = entry_w.text().trim().to_string();
            if !url.starts_with("http") {
                error.set_text("That doesn\u{2019}t look like a link.");
                error.set_visible(true);
                return;
            }
            // Telemetry label only — the resolver decides how the URL is
            // actually handled.
            let source = if url.contains("pin.it") || url.contains("pinterest.com") {
                "pinterest"
            } else {
                "direct"
            };
            btn.set_sensitive(false);
            entry_w.set_sensitive(false);
            error.set_visible(false);
            status.set_text("Resolving link\u{2026}");
            status.set_visible(true);
            progress.set_visible(true);

            enum Msg {
                Downloading,
                Progress(f64),
                Duplicate,
                Done(Result<PathBuf, String>),
            }
            let (tx, rx) = async_channel::bounded::<Msg>(16);
            let flag_worker = flag.clone();
            let existing = entry_file_names(&state);
            let dest = library::library_dir().join("downloads");
            std::thread::spawn(move || {
                let resolved = match crate::linkresolve::resolve(&url) {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send_blocking(Msg::Done(Err(e.to_string())));
                        return;
                    }
                };
                if let Some(pid) = &resolved.pin_id {
                    if existing.iter().any(|n| n.contains(pid.as_str())) {
                        let _ = tx.send_blocking(Msg::Duplicate);
                        return;
                    }
                }
                let _ = tx.send_blocking(Msg::Downloading);
                let tx_p = tx.clone();
                let result = crate::download::download(
                    &resolved.media_url,
                    &dest,
                    MAX_BYTES,
                    &flag_worker,
                    move |got, total| {
                        if let Some(t) = total {
                            let _ = tx_p.try_send(Msg::Progress(got as f64 / t as f64));
                        }
                    },
                );
                let _ = tx.send_blocking(Msg::Done(
                    result
                        .map(|p| rename_to_resolved(p, &resolved))
                        .map_err(|e| e.to_string()),
                ));
            });

            // Pulse while no byte-level progress is known (resolve phase, or a
            // server that omits Content-Length); the first Progress message
            // switches the bar to determinate.
            let pulsing = Rc::new(std::cell::Cell::new(true));
            {
                let progress = progress.clone();
                let pulsing = pulsing.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
                    if !pulsing.get() || !progress.is_visible() {
                        return glib::ControlFlow::Break;
                    }
                    progress.pulse();
                    glib::ControlFlow::Continue
                });
            }

            let state = state.clone();
            let stack = stack.clone();
            let dialog = dialog.clone();
            let entry_w = entry_w.clone();
            let error = error.clone();
            let status = status.clone();
            let progress = progress.clone();
            let btn = btn.clone();
            glib::spawn_future_local(async move {
                while let Ok(msg) = rx.recv().await {
                    match msg {
                        Msg::Downloading => status.set_text("Downloading\u{2026}"),
                        Msg::Progress(f) => {
                            pulsing.set(false);
                            progress.set_fraction(f.clamp(0.0, 1.0));
                        }
                        Msg::Duplicate => {
                            show_toast(&state, "Already in your library");
                            dialog.close();
                            break;
                        }
                        Msg::Done(Ok(path)) => {
                            crate::telemetry::event(
                                "add_from_link",
                                serde_json::json!({
                                    "ok": true,
                                    "source": source,
                                    // Video vs image split matters for usage
                                    // analytics (and any future Pro limits).
                                    "kind": if library::is_video(&path) { "video" } else { "image" },
                                }),
                            );
                            let mut e = if library::is_video(&path) {
                                library::LibraryEntry::new_video(path)
                            } else {
                                library::LibraryEntry::new_image(path)
                            };
                            e.generate_thumbnail();
                            let name = e.name.clone();
                            let idx = {
                                let mut s = state.borrow_mut();
                                s.entries.push(e);
                                save_entries(&s.entries).ok();
                                s.entries.len() - 1
                            };
                            show_toast(
                                &state,
                                &format!(
                                    "\u{201c}{name}\u{201d} added \u{2014} preview and adjust, then set"
                                ),
                            );
                            let refresh = state.borrow().refresh.clone();
                            if let Some(r) = refresh {
                                r();
                            }
                            dialog.close();
                            // Land in the editor, not the grid: a link-added
                            // wallpaper deserves the same preview/rotate/crop
                            // pass as a file-picked one before it's set.
                            super::window::open_editor(&state, &stack, idx);
                            break;
                        }
                        Msg::Done(Err(msg)) => {
                            crate::telemetry::event(
                                "add_from_link",
                                serde_json::json!({ "ok": false, "source": source }),
                            );
                            pulsing.set(false);
                            error.set_text(&msg);
                            error.set_visible(true);
                            status.set_visible(false);
                            progress.set_visible(false);
                            btn.set_sensitive(true);
                            entry_w.set_sensitive(true);
                            break;
                        }
                    }
                }
            });
        });
    }

    dialog.present();
}
