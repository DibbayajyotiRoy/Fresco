//! In-app wallpaper catalog browser (ROADMAP 3.1). Metadata comes from
//! `crate::catalog`; media downloads ride `crate::download` into the library
//! as ordinary entries — no parallel content system.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::catalog::{self, CatalogItem};
use crate::gui::library;
use crate::gui::window::AppState;

const MAX_MEDIA_BYTES: u64 = 500_000_000;

fn cache_dir() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("gallery")
}

pub fn show_gallery_window(parent: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let win = adw::Window::new();
    win.set_transient_for(Some(parent));
    win.set_modal(false);
    win.set_default_size(760, 560);
    win.set_title(Some("Browse wallpapers"));

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some("Search wallpapers"));
    header.set_title_widget(Some(&search));
    root.append(&header);

    let status = gtk4::Label::new(Some("Loading catalog…"));
    status.add_css_class("dim");
    status.set_margin_top(24);
    root.append(&status);

    let flow = gtk4::FlowBox::new();
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_column_spacing(12);
    flow.set_row_spacing(12);
    flow.set_margin_top(12);
    flow.set_margin_bottom(12);
    flow.set_margin_start(12);
    flow.set_margin_end(12);
    flow.set_homogeneous(true);
    flow.set_valign(gtk4::Align::Start);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&flow));
    root.append(&scroll);
    win.set_content(Some(&root));

    // Fetch (network) with cache fallback, off the main thread.
    let (tx, rx) = async_channel::bounded::<Result<Vec<CatalogItem>, String>>(1);
    std::thread::spawn(move || {
        let result = match catalog::fetch(&catalog::catalog_url()) {
            Ok(items) => {
                let _ = catalog::save_cache(&cache_dir(), &items);
                Ok(items)
            }
            Err(e) => match catalog::load_cache(&cache_dir()) {
                Some(items) => Ok(items), // offline: cached catalog still browses
                None => Err(format!("Couldn’t load the catalog: {e:#}")),
            },
        };
        let _ = tx.send_blocking(result);
    });

    let items_store: Rc<RefCell<Vec<CatalogItem>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let flow = flow.clone();
        let status = status.clone();
        let state = state.clone();
        let items_store = items_store.clone();
        let search = search.clone();
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(Ok(items)) => {
                    log::info!("gallery: {} catalog item(s)", items.len());
                    status.set_visible(items.is_empty());
                    if items.is_empty() {
                        status.set_text("The catalog is empty right now — check back soon.");
                    }
                    *items_store.borrow_mut() = items;
                    render(&flow, &items_store.borrow(), "", &state);
                    let items_store = items_store.clone();
                    let flow2 = flow.clone();
                    let state2 = state.clone();
                    search.connect_search_changed(move |s| {
                        render(
                            &flow2,
                            &items_store.borrow(),
                            &s.text().to_lowercase(),
                            &state2,
                        );
                    });
                }
                Ok(Err(msg)) => status.set_text(&msg),
                Err(_) => status.set_text("Couldn’t load the catalog."),
            }
        });
    }

    win.present();
}

fn render(flow: &gtk4::FlowBox, items: &[CatalogItem], query: &str, state: &Rc<RefCell<AppState>>) {
    while let Some(child) = flow.first_child() {
        flow.remove(&child);
    }
    for item in items {
        if !query.is_empty() {
            let hay = format!(
                "{} {} {}",
                item.title.to_lowercase(),
                item.category.to_lowercase(),
                item.tags.join(" ").to_lowercase()
            );
            if !hay.contains(query) {
                continue;
            }
        }
        flow.insert(&card(item, state), -1);
    }
}

fn card(item: &CatalogItem, state: &Rc<RefCell<AppState>>) -> gtk4::Widget {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    b.add_css_class("card");
    b.set_width_request(200);

    let title = gtk4::Label::new(Some(&item.title));
    title.add_css_class("heading");
    title.set_wrap(true);
    title.set_xalign(0.0);
    title.set_margin_top(10);
    title.set_margin_start(12);
    title.set_margin_end(12);
    b.append(&title);

    // Attribution is a launch requirement: license + author on EVERY card.
    let meta = gtk4::Label::new(Some(&format!(
        "{} · {} · {} MB",
        if item.author.is_empty() {
            "Unknown"
        } else {
            &item.author
        },
        item.license,
        item.size_bytes / 1_048_576
    )));
    meta.add_css_class("dim");
    meta.set_wrap(true);
    meta.set_xalign(0.0);
    meta.set_margin_start(12);
    meta.set_margin_end(12);
    b.append(&meta);

    let progress = gtk4::ProgressBar::new();
    progress.set_visible(false);
    progress.set_margin_start(12);
    progress.set_margin_end(12);
    b.append(&progress);

    let btn = gtk4::Button::with_label("Set as wallpaper");
    btn.add_css_class("suggested-action");
    btn.set_margin_top(4);
    btn.set_margin_bottom(10);
    btn.set_margin_start(12);
    btn.set_margin_end(12);
    b.append(&btn);

    let item = item.clone();
    let state = state.clone();
    btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        progress.set_visible(true);
        install(item.clone(), state.clone(), btn.clone(), progress.clone());
    });

    b.upcast()
}

/// Download → library entry → set as wallpaper (the ≤3-clicks path).
fn install(
    item: CatalogItem,
    state: Rc<RefCell<AppState>>,
    btn: gtk4::Button,
    progress: gtk4::ProgressBar,
) {
    enum Msg {
        Progress(f64),
        Done(Result<std::path::PathBuf, String>),
    }
    let (tx, rx) = async_channel::bounded::<Msg>(16);
    let url = item.media_url.clone();
    let item_id = item.id.clone();
    std::thread::spawn(move || {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let dest = library::library_dir().join("downloads");
        let tx_p = tx.clone();
        let result = crate::download::download(&url, &dest, MAX_MEDIA_BYTES, &cancel, {
            move |got, total| {
                if let Some(t) = total {
                    let _ = tx_p.try_send(Msg::Progress(got as f64 / t as f64));
                }
            }
        });
        if result.is_ok() {
            catalog::record_install(&item_id); // server-side count, no identifiers
        }
        let _ = tx.send_blocking(Msg::Done(result.map_err(|e| e.to_string())));
    });

    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                Msg::Progress(f) => progress.set_fraction(f.clamp(0.0, 1.0)),
                Msg::Done(Ok(path)) => {
                    let mut e = if library::is_video(&path) {
                        library::LibraryEntry::new_video(path)
                    } else {
                        library::LibraryEntry::new_image(path)
                    };
                    e.name = item.title.clone();
                    e.catalog_id = Some(item.id.clone());
                    e.generate_thumbnail();
                    let idx = {
                        let mut s = state.borrow_mut();
                        s.entries.push(e);
                        s.entries.len() - 1
                    };
                    crate::gui::window::apply_entry_by_idx(state.clone(), idx);
                    progress.set_visible(false);
                    btn.set_label("Set ✓");
                    break;
                }
                Msg::Done(Err(msg)) => {
                    progress.set_visible(false);
                    btn.set_sensitive(true);
                    log::warn!("gallery install failed: {msg}");
                    crate::gui::window::show_toast(&state, &msg);
                    break;
                }
            }
        }
    });
}
