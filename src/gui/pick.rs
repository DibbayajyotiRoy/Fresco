//! `fresco pick`: a small window listing the switch pool, for a key bound to
//! it in the desktop's shortcut settings (Wayland does not let an app grab
//! global keys itself).
//!
//! Runs from `cli::dispatch` with its own GTK application, non-unique, so it
//! never hands the request to a running Fresco window. The pool is the one
//! `fresco next` cycles (`switch::load_pool`), and choosing an entry goes
//! through `switch::select` and `daemon_ctl::apply_async`, the gallery's path.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{gdk, gio, glib, prelude::*};
use libadwaita as adw;

use super::library::LibraryEntry;
use super::{daemon_ctl, switch, theme, window};
use crate::config::{Config, Kind};
use crate::t;

/// Case-insensitive substring match. A blank query matches everything.
fn matches(name: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q)
}

fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Video => "video-x-generic-symbolic",
        Kind::Image => "image-x-generic-symbolic",
        Kind::Playlist => "media-playlist-consecutive-symbolic",
        Kind::Slideshow => "folder-pictures-symbolic",
    }
}

struct Picker {
    config: Config,
    entries: Vec<LibraryEntry>,
    /// Indices into `entries`, in the order `fresco next` cycles them.
    pool: Vec<usize>,
}

/// Entry point from `cli::dispatch`: exit 0 after a choice (or Esc), 1 with a
/// message on stderr when nothing can be applied.
pub fn run() -> i32 {
    let (config, entries, pool) = match switch::load_pool() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e:#}");
            return 1;
        }
    };
    if pool.is_empty() {
        eprintln!("error: no wallpapers to pick from — add some in the Fresco app first");
        return 1;
    }
    let state = Rc::new(RefCell::new(Picker {
        config,
        entries,
        pool,
    }));
    let code = Rc::new(Cell::new(0));
    let app = adw::Application::new(None::<&str>, gio::ApplicationFlags::NON_UNIQUE);
    let (state_c, code_c) = (state.clone(), code.clone());
    app.connect_activate(move |app| build(app, state_c.clone(), code_c.clone()));
    app.run_with_args::<&str>(&[]);
    code.get()
}

fn build(app: &adw::Application, state: Rc<RefCell<Picker>>, code: Rc<Cell<i32>>) {
    let cfg = state.borrow().config.clone();
    theme::set_mode(cfg.theme_mode);
    theme::apply(cfg.accent, theme::resolve_dark(cfg.theme_mode));

    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some(t!("Search wallpapers")));

    let list = gtk4::ListBox::new();
    // Enter or a double-click activates a row; a single click only selects.
    list.set_activate_on_single_click(false);
    list.set_placeholder(Some(&gtk4::Label::new(Some(t!("No wallpapers match")))));
    let scroll = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&list)
        .build();

    let vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    vbox.append(&search);
    vbox.append(&scroll);

    let win = gtk4::ApplicationWindow::builder()
        .application(app)
        .title(t!("Pick a wallpaper"))
        .default_width(380)
        .default_height(460)
        .child(&vbox)
        .build();

    let visible: Rc<RefCell<Vec<usize>>> = Rc::default();
    refill(&list, &state.borrow(), &visible, "");

    // Applies the entry shown at `row` (a row index), then closes.
    let choose: Rc<dyn Fn(i32)> = {
        let (app, win, state, code, visible) = (
            app.clone(),
            win.clone(),
            state.clone(),
            code.clone(),
            visible.clone(),
        );
        Rc::new(move |row: i32| {
            let shown = usize::try_from(row)
                .ok()
                .and_then(|r| visible.borrow().get(r).copied());
            if let Some(idx) = shown {
                apply(&app, &win, &state, &code, idx);
            }
        })
    };

    {
        // `changed`, not `search-changed`: the latter is debounced, so Enter
        // within that window would act on the previous query's list.
        let (list, state, visible, scroll) =
            (list.clone(), state.clone(), visible.clone(), scroll.clone());
        search.connect_changed(move |s| {
            refill(&list, &state.borrow(), &visible, &s.text());
            scroll.vadjustment().set_value(0.0);
        });
    }
    {
        let (list, choose) = (list.clone(), choose.clone());
        search.connect_activate(move |_| {
            if let Some(row) = list.selected_row() {
                choose(row.index());
            }
        });
    }
    {
        let choose = choose.clone();
        list.connect_row_activated(move |_, row| choose(row.index()));
    }

    // Capture phase, so Up/Down reach the list even while the search entry
    // has focus, and Esc closes from anywhere in the window.
    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let (win, list, scroll, visible) =
            (win.clone(), list.clone(), scroll.clone(), visible.clone());
        keys.connect_key_pressed(move |_, key, _, _| {
            let step = if key == gdk::Key::Up {
                -1
            } else if key == gdk::Key::Down {
                1
            } else if key == gdk::Key::Escape {
                win.close();
                return glib::Propagation::Stop;
            } else {
                return glib::Propagation::Proceed;
            };
            let len = visible.borrow().len();
            move_selection(&list, &scroll, len, step);
            glib::Propagation::Stop
        });
    }
    win.add_controller(keys);

    win.present();
    search.grab_focus();
}

/// Rebuild the list with the pool entries whose label matches `query`, and
/// select the first one so Enter applies the best match.
fn refill(list: &gtk4::ListBox, p: &Picker, visible: &RefCell<Vec<usize>>, query: &str) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let mut shown = visible.borrow_mut();
    shown.clear();
    for &idx in &p.pool {
        let e = &p.entries[idx];
        let label = window::display_name(&e.name, e.kind);
        if !matches(&label, query) {
            continue;
        }
        list.append(&row(&label, e.kind));
        shown.push(idx);
    }
    if let Some(first) = list.row_at_index(0) {
        list.select_row(Some(&first));
    }
}

fn row(label: &str, kind: Kind) -> gtk4::Box {
    let b = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(10)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(8)
        .margin_end(8)
        .build();
    b.append(&gtk4::Image::from_icon_name(kind_icon(kind)));
    b.append(
        &gtk4::Label::builder()
            .label(label)
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .hexpand(true)
            .build(),
    );
    b
}

/// Select the row `step` away from the current one, clamped to the list, and
/// scroll just enough to keep it in view.
fn move_selection(list: &gtk4::ListBox, scroll: &gtk4::ScrolledWindow, len: usize, step: i32) {
    if len == 0 {
        return;
    }
    let cur = list.selected_row().map_or(-1, |r| r.index());
    let next = (cur + step).clamp(0, len as i32 - 1);
    let Some(row) = list.row_at_index(next) else {
        return;
    };
    list.select_row(Some(&row));
    if let Some(b) = row.compute_bounds(list) {
        let adj = scroll.vadjustment();
        let (top, bottom) = (f64::from(b.y()), f64::from(b.y() + b.height()));
        if top < adj.value() {
            adj.set_value(top);
        } else if bottom > adj.value() + adj.page_size() {
            adj.set_value(bottom - adj.page_size());
        }
    }
}

/// Make `entries[idx]` the wallpaper and hand it to the daemon. The window
/// hides at once; the app quits when the daemon has taken the request, since
/// quitting first would drop it.
fn apply(
    app: &adw::Application,
    win: &gtk4::ApplicationWindow,
    state: &RefCell<Picker>,
    code: &Rc<Cell<i32>>,
    idx: usize,
) {
    let mut guard = state.borrow_mut();
    let p = &mut *guard;
    if let Err(e) = switch::select(&mut p.config, &mut p.entries, idx) {
        eprintln!("error: {e:#}");
        code.set(1);
        app.quit();
        return;
    }
    win.set_visible(false);
    let (app, code) = (app.clone(), code.clone());
    daemon_ctl::apply_async(&p.config, move |outcome| {
        if let Err(e) = outcome.result {
            eprintln!("error: {e}");
            code.set(1);
        }
        app.quit();
    });
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn filter_is_a_case_insensitive_substring_match() {
        assert!(matches("Ocean Waves", "wave"));
        assert!(matches("Ocean Waves", "OCEAN"));
        assert!(matches("Ocean Waves", "  "), "a blank query matches all");
        assert!(matches("Ocean Waves", ""));
        assert!(!matches("Ocean Waves", "forest"));
        assert!(!matches("Ocean", "ocean waves"), "no word-by-word matching");
    }
}
