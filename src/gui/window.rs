use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};
use gtk4::{FileChooserAction, GestureClick, PolicyType, ResponseType};
use libadwaita::{self as adw, prelude::*};

use super::theme;
use super::{
    daemon_ctl,
    library::{self, load_entries, save_entries, LibraryEntry},
};
use crate::{
    autostart,
    config::{Accent, Config, Fit, Kind, Scaling, ThemeMode, Transition},
    APP_ID,
};

pub struct FrescoApplication {
    pub app: adw::Application,
}

impl FrescoApplication {
    pub fn new() -> Self {
        let app = adw::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE);
        app.connect_activate(build_ui);
        FrescoApplication { app }
    }

    pub fn run(&self, args: &[String]) -> i32 {
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let code = self.app.run_with_args(&argv);
        i32::from(code)
    }
}

impl Default for FrescoApplication {
    fn default() -> Self {
        Self::new()
    }
}

// ─── App state ────────────────────────────────────────────────────────────────

struct AppState {
    config: Config,
    entries: Vec<LibraryEntry>,
    editing_idx: Option<usize>,
    /// Keeps the native file/folder chooser alive until it responds. Without
    /// this, the local `FileChooserNative` is dropped when the open function
    /// returns, so the portal's reply never reaches our handler.
    current_picker: Option<gtk4::FileChooserNative>,
    /// Floating toast host that wraps the whole window (set once in build_ui).
    toast: adw::ToastOverlay,
    /// Rebuilds the library grid in place; installed by build_library_view so
    /// the active-wallpaper highlight can update without a view switch.
    refresh: Option<Rc<dyn Fn()>>,
}

// ─── Main window ─────────────────────────────────────────────────────────────

fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Fresco"));
    window.set_default_size(880, 660);
    window.set_size_request(420, 480);
    window.set_icon_name(Some(APP_ID));

    let config = Config::load().unwrap_or_default();

    // Install + apply the theme before first paint so there is no flash.
    theme::install();
    theme::set_mode(config.theme_mode);
    theme::apply(config.accent, theme::is_dark());

    // Wayland guard.
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        let status = adw::StatusPage::new();
        status.set_icon_name(Some("dialog-information-symbolic"));
        status.set_title("X11 session required");
        status.set_description(Some(
            "Fresco currently works on X11 sessions only. Log out and select the Xorg session at login.",
        ));
        window.set_content(Some(&status));
        window.present();
        return;
    }

    // Mark broken entries (missing source files) before showing the library.
    let mut entries = load_entries().unwrap_or_default();
    for e in &mut entries {
        e.check_health();
    }

    let toast = adw::ToastOverlay::new();

    let state = Rc::new(RefCell::new(AppState {
        config,
        entries,
        editing_idx: None,
        current_picker: None,
        toast: toast.clone(),
        refresh: None,
    }));

    // Re-apply the palette when the system light/dark resolution flips.
    {
        let state = state.clone();
        adw::StyleManager::default().connect_dark_notify(move |sm| {
            theme::apply(state.borrow().config.accent, sm.is_dark());
        });
    }

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
    stack.set_transition_duration(220);

    let library_view = build_library_view(&window, state.clone(), &stack);
    let editor_view = build_editor_view(state.clone(), &stack);

    stack.add_named(&library_view, Some("library"));
    stack.add_named(&editor_view, Some("editor"));

    toast.set_child(Some(&stack));
    window.set_content(Some(&toast));
    window.present();

    // Anonymous opt-in feedback + admin-pushed notifications (Supabase).
    run_startup_checks(&window, state);
}

// ─── Library view ─────────────────────────────────────────────────────────────

fn build_library_view(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: &gtk4::Stack,
) -> gtk4::Box {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // ── Header bar ──
    // Deliberately no pause/stop buttons: setting a wallpaper just runs it, and
    // picking another switches it. A stray "Stop" only created a confusing
    // dead/stopped state, so the model is kept dead-simple.
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Fresco", "Live wallpapers")));

    let menu_btn = gtk4::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.add_css_class("flat");
    menu_btn.set_popover(Some(&build_menu_popover(window, state.clone())));
    header.pack_end(&menu_btn);
    root.append(&header);

    // ── "What's new" banner (shown once per version after an update) ──
    if let Some(banner) = build_update_banner(window, state.clone()) {
        root.append(&banner);
    }

    // ── Search ──
    let search = gtk4::SearchEntry::new();
    search.add_css_class("wp-search");
    search.set_placeholder_text(Some("Search wallpapers…"));
    search.set_margin_start(16);
    search.set_margin_end(16);
    search.set_margin_top(12);
    search.set_margin_bottom(4);
    root.append(&search);

    // ── Scrollable content ──
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_bottom(8);

    // Recent row.
    let recent_label = overline("Recent");
    recent_label.set_margin_top(14);
    recent_label.set_margin_bottom(8);
    content.append(&recent_label);

    let recent_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    recent_box.set_margin_bottom(10);
    content.append(&recent_box);

    // Per-type sections (Images / Videos / GIFs); rebuilt by populate_library.
    let sections_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sections_box.set_margin_bottom(12);
    content.append(&sections_box);

    // Welcome page (shown when library is empty).
    let welcome = adw::StatusPage::new();
    welcome.set_icon_name(Some("video-display-symbolic"));
    welcome.set_title("No wallpapers yet");
    welcome.set_description(Some(
        "Add a video, GIF, image, or a folder of images to begin",
    ));
    welcome.set_vexpand(true);
    let welcome_btn = gtk4::Button::with_label("Add your first wallpaper");
    welcome_btn.add_css_class("suggested-action");
    welcome_btn.add_css_class("pill");
    welcome_btn.add_css_class("welcome-cta");
    welcome_btn.set_halign(gtk4::Align::Center);
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let win2 = window.clone();
        welcome_btn.connect_clicked(move |_| {
            open_file_picker(&win2, state2.clone(), stack2.clone(), None);
        });
    }
    welcome.set_child(Some(&welcome_btn));
    content.append(&welcome);

    scroll.set_child(Some(&content));
    root.append(&scroll);

    // ── Footer: status pill (left) + add actions (right) ──
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    footer.set_margin_start(16);
    footer.set_margin_end(16);
    footer.set_margin_top(8);
    footer.set_margin_bottom(14);

    let status_pill = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    status_pill.add_css_class("status-pill");
    status_pill.set_valign(gtk4::Align::Center);
    let status_dot = gtk4::Label::new(Some("●"));
    status_dot.add_css_class("dot-off");
    let status_label = gtk4::Label::new(None);
    status_pill.append(&status_dot);
    status_pill.append(&status_label);
    footer.append(&status_pill);

    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    footer.append(&spacer);

    let add_folder_btn = gtk4::Button::new();
    add_folder_btn.set_child(Some(&button_content("folder-new-symbolic", "Add folder")));
    add_folder_btn.set_tooltip_text(Some("Create an image slideshow from a folder"));
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let win2 = window.clone();
        add_folder_btn.connect_clicked(move |_| {
            open_folder_picker(&win2, state2.clone(), stack2.clone());
        });
    }
    footer.append(&add_folder_btn);

    let add_btn = gtk4::Button::new();
    add_btn.set_child(Some(&button_content("list-add-symbolic", "Add")));
    add_btn.add_css_class("suggested-action");
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let win2 = window.clone();
        add_btn.connect_clicked(move |_| {
            open_file_picker(&win2, state2.clone(), stack2.clone(), None);
        });
    }
    footer.append(&add_btn);
    root.append(&footer);

    // ── Live-updating sectioned library ──
    let home_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let refresh: Rc<dyn Fn()> = {
        let state = state.clone();
        let sections_box = sections_box.clone();
        let recent_box = recent_box.clone();
        let recent_label = recent_label.clone();
        let welcome = welcome.clone();
        let stack = stack.clone();
        let home_query = home_query.clone();
        let search = search.clone();
        Rc::new(move || {
            // Searching an empty library is pointless: hide the field until
            // there's something to search.
            search.set_visible(!state.borrow().entries.is_empty());
            let q = home_query.borrow();
            populate_library(
                &state,
                &sections_box,
                &recent_box,
                &recent_label,
                &welcome,
                &stack,
                q.as_str(),
            );
        })
    };
    state.borrow_mut().refresh = Some(refresh.clone());
    refresh();

    // Search re-runs populate with the query (rebuilds the matching sections).
    {
        let home_query = home_query.clone();
        let refresh = refresh.clone();
        search.connect_search_changed(move |entry| {
            *home_query.borrow_mut() = entry.text().to_string();
            refresh();
        });
    }

    // Repopulate whenever we return to the library view (e.g. after editing).
    {
        let refresh = refresh.clone();
        stack.connect_visible_child_name_notify(move |s| {
            if s.visible_child_name().as_deref() == Some("library") {
                refresh();
            }
        });
    }

    // Poll daemon status every 3s. Quiet by design: the pill is hidden while
    // stopped, and only shows a single semantic dot + concise decode mode when
    // a wallpaper is actually playing (no CPU/RAM churn, no "Checking…").
    {
        let status_label = status_label.clone();
        let status_dot = status_dot.clone();
        let status_pill = status_pill.clone();
        let tick = move || {
            let status = daemon_ctl::get_status();
            for c in ["dot-ok", "dot-warn", "dot-off"] {
                status_dot.remove_css_class(c);
            }
            match status.as_ref() {
                None => status_pill.set_visible(false),
                Some(s) if s.paused => {
                    status_pill.set_visible(true);
                    status_dot.add_css_class("dot-warn");
                    status_label.set_text("Paused");
                    status_label.set_tooltip_text(None);
                }
                Some(s) if matches!(s.hwdec.as_deref(), Some("no") | None) => {
                    status_pill.set_visible(true);
                    status_dot.add_css_class("dot-warn");
                    status_label.set_text("Software decode");
                    status_label
                        .set_tooltip_text(daemon_ctl::hwdec_hint(status.as_ref()).as_deref());
                }
                Some(_) => {
                    status_pill.set_visible(true);
                    status_dot.add_css_class("dot-ok");
                    status_label.set_text("GPU decode");
                    status_label.set_tooltip_text(None);
                }
            }
        };
        tick();
        glib::timeout_add_local(Duration::from_secs(3), move || {
            tick();
            glib::ControlFlow::Continue
        });
    }

    root
}

/// Header menu: appearance (theme mode + accent) and behavior switches.
fn build_menu_popover(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) -> gtk4::Popover {
    let popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    popover_box.set_margin_top(10);
    popover_box.set_margin_bottom(10);
    popover_box.set_margin_start(10);
    popover_box.set_margin_end(10);
    popover_box.set_width_request(252);

    // ── Appearance ──
    popover_box.append(&overline("Appearance"));

    let seg = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    seg.add_css_class("linked");
    seg.add_css_class("seg");
    seg.set_homogeneous(true);
    let b_sys = gtk4::ToggleButton::with_label("System");
    let b_light = gtk4::ToggleButton::with_label("Light");
    let b_dark = gtk4::ToggleButton::with_label("Dark");
    b_light.set_group(Some(&b_sys));
    b_dark.set_group(Some(&b_sys));
    match state.borrow().config.theme_mode {
        ThemeMode::System => b_sys.set_active(true),
        ThemeMode::Light => b_light.set_active(true),
        ThemeMode::Dark => b_dark.set_active(true),
    }
    for (btn, mode) in [
        (&b_sys, ThemeMode::System),
        (&b_light, ThemeMode::Light),
        (&b_dark, ThemeMode::Dark),
    ] {
        let state2 = state.clone();
        btn.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            let accent = {
                let mut s = state2.borrow_mut();
                s.config.theme_mode = mode;
                s.config.save().ok();
                s.config.accent
            };
            theme::set_mode(mode);
            theme::apply(accent, theme::is_dark());
        });
    }
    seg.append(&b_sys);
    seg.append(&b_light);
    seg.append(&b_dark);
    popover_box.append(&seg);

    let accent_lbl = overline("Accent");
    accent_lbl.set_margin_top(8);
    popover_box.append(&accent_lbl);

    let dot_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    dot_row.set_margin_top(2);
    dot_row.set_margin_bottom(2);
    let dot_btns: Rc<RefCell<Vec<(Accent, gtk4::Button)>>> = Rc::new(RefCell::new(Vec::new()));
    for (acc, cls) in [
        (Accent::Blue, "accent-blue"),
        (Accent::Teal, "accent-teal"),
        (Accent::Green, "accent-green"),
        (Accent::Amber, "accent-amber"),
        (Accent::Coral, "accent-coral"),
        (Accent::Graphite, "accent-graphite"),
    ] {
        let b = gtk4::Button::new();
        b.add_css_class("accent-dot");
        b.add_css_class(cls);
        b.set_tooltip_text(Some(accent_name(acc)));
        if state.borrow().config.accent == acc {
            b.add_css_class("selected");
        }
        {
            let state2 = state.clone();
            let dots = dot_btns.clone();
            b.connect_clicked(move |_| {
                {
                    let mut s = state2.borrow_mut();
                    s.config.accent = acc;
                    s.config.save().ok();
                }
                theme::apply(acc, theme::is_dark());
                for (a, btn) in dots.borrow().iter() {
                    if *a == acc {
                        btn.add_css_class("selected");
                    } else {
                        btn.remove_css_class("selected");
                    }
                }
            });
        }
        dot_row.append(&b);
        dot_btns.borrow_mut().push((acc, b));
    }
    popover_box.append(&dot_row);

    let sep1 = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    sep1.set_margin_top(6);
    sep1.set_margin_bottom(2);
    popover_box.append(&sep1);

    // ── Behavior ──
    popover_box.append(&overline("Behavior"));
    popover_box.append(&switch_row(
        "Restore on login",
        state.borrow().config.autostart,
        {
            let state2 = state.clone();
            move |active| {
                {
                    let mut s = state2.borrow_mut();
                    s.config.autostart = active;
                    s.config.save().ok();
                }
                if active {
                    autostart::enable().ok();
                } else {
                    autostart::disable().ok();
                }
            }
        },
    ));
    popover_box.append(&switch_row(
        "Pause on battery",
        state.borrow().config.pause_on_battery,
        {
            let state2 = state.clone();
            move |active| {
                let mut s = state2.borrow_mut();
                s.config.pause_on_battery = active;
                s.config.save().ok();
            }
        },
    ));

    let sep2 = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    sep2.set_margin_top(2);
    sep2.set_margin_bottom(2);
    popover_box.append(&sep2);

    let advanced_btn = gtk4::Button::with_label("Advanced…");
    advanced_btn.add_css_class("flat");
    advanced_btn.set_halign(gtk4::Align::Start);
    {
        let state_adv = state.clone();
        let win_adv = window.clone();
        advanced_btn.connect_clicked(move |_| {
            show_advanced_dialog(&win_adv, state_adv.clone());
        });
    }
    popover_box.append(&advanced_btn);

    let popover = gtk4::Popover::new();
    popover.set_child(Some(&popover_box));
    popover
}

// Orchestrates a coherent bundle of library-view widgets; splitting them into a
// struct would add ceremony without clarifying this single-caller helper.
#[allow(clippy::too_many_arguments)]
fn populate_library(
    state: &Rc<RefCell<AppState>>,
    sections_box: &gtk4::Box,
    recent_box: &gtk4::Box,
    recent_label: &gtk4::Label,
    welcome: &adw::StatusPage,
    stack: &gtk4::Stack,
    query: &str,
) {
    // Clear.
    while let Some(c) = sections_box.first_child() {
        sections_box.remove(&c);
    }
    while let Some(c) = recent_box.first_child() {
        recent_box.remove(&c);
    }

    let (entries, cfg) = {
        let s = state.borrow();
        (s.entries.clone(), s.config.clone())
    };

    if entries.is_empty() {
        welcome.set_visible(true);
        recent_label.set_visible(false);
        recent_box.set_visible(false);
        return;
    }
    welcome.set_visible(false);

    let q = query.to_lowercase();
    let searching = !q.is_empty();

    // Recents (hidden while searching, to focus on the matches).
    {
        let recents = if searching {
            Vec::new()
        } else {
            library::recent_entries(&entries, 6)
        };
        let show = !recents.is_empty();
        recent_label.set_visible(show);
        recent_box.set_visible(show);
        for e in recents {
            let idx = entries.iter().position(|x| x.id == e.id).unwrap_or(0);
            let active = entry_is_active(e, &cfg);
            recent_box.append(&build_mini_card(
                e,
                idx,
                state.clone(),
                stack.clone(),
                active,
            ));
        }
    }

    // One section per non-empty category: Images, Videos, GIFs.
    let mut first_section = true;
    for cat in CATEGORY_ORDER {
        let matches: Vec<(usize, &LibraryEntry)> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                entry_category(e) == cat && (q.is_empty() || e.name.to_lowercase().contains(&q))
            })
            .collect();
        if matches.is_empty() {
            continue;
        }

        let section = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let header = overline(category_label(cat));
        header.set_margin_top(if first_section { 12 } else { 16 });
        header.set_margin_bottom(10);
        first_section = false;
        section.append(&header);

        let flow = gtk4::FlowBox::new();
        flow.set_homogeneous(true);
        flow.set_max_children_per_line(6);
        flow.set_min_children_per_line(2);
        flow.set_selection_mode(gtk4::SelectionMode::None);
        flow.set_valign(gtk4::Align::Start);
        flow.set_row_spacing(14);
        flow.set_column_spacing(14);
        flow.set_margin_bottom(8);
        for (idx, entry) in matches {
            let active = entry_is_active(entry, &cfg);
            let card = build_library_card(entry, idx, state.clone(), stack.clone(), active);
            flow.append(&card);
        }
        section.append(&flow);
        sections_box.append(&section);
    }
}

/// Compact recent-row card: thumbnail + title scrim, click to apply.
fn build_mini_card(
    entry: &LibraryEntry,
    idx: usize,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    active: bool,
) -> gtk4::Overlay {
    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("wp-mini");
    if active {
        overlay.add_css_class("active");
    }
    overlay.set_overflow(gtk4::Overflow::Hidden);
    overlay.set_size_request(150, 84);

    let pic = gtk4::Picture::new();
    pic.add_css_class("wp-thumb");
    pic.set_size_request(150, 84);
    pic.set_can_shrink(true);
    pic.set_keep_aspect_ratio(true);
    if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
        pic.set_file(Some(&gio::File::for_path(thumb)));
    }
    overlay.set_child(Some(&pic));

    let scrim = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    scrim.add_css_class("wp-scrim");
    scrim.set_valign(gtk4::Align::End);
    let title = gtk4::Label::new(Some(&entry.name));
    title.add_css_class("wp-title");
    title.set_xalign(0.0);
    title.set_halign(gtk4::Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    scrim.append(&title);
    overlay.add_overlay(&scrim);

    let click = GestureClick::new();
    {
        let state_c = state.clone();
        let stack_c = stack.clone();
        click.connect_released(move |_, n_press, _, _| {
            if n_press == 1 {
                apply_entry_by_idx(state_c.clone(), idx);
            } else if n_press == 2 {
                state_c.borrow_mut().editing_idx = Some(idx);
                stack_c.set_visible_child_name("editor");
            }
        });
    }
    overlay.add_controller(click);

    overlay
}

/// Cinematic 16:9 library card: poster thumbnail, gradient title scrim, kind
/// badge, active-wallpaper accent ring + pill, and a hover-revealed Edit button.
fn build_library_card(
    entry: &LibraryEntry,
    idx: usize,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    active: bool,
) -> gtk4::Overlay {
    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("wp-card");
    if active {
        overlay.add_css_class("active");
    }
    overlay.set_overflow(gtk4::Overflow::Hidden);
    // Fixed 16:9 poster footprint, top-aligned so the FlowBox never stretches
    // cards to fill the viewport (vexpand on the child would propagate upward).
    overlay.set_size_request(230, 130);
    overlay.set_valign(gtk4::Align::Start);

    let pic = gtk4::Picture::new();
    pic.add_css_class("wp-thumb");
    pic.set_size_request(230, 130);
    pic.set_can_shrink(true);
    pic.set_keep_aspect_ratio(true);
    if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
        pic.set_file(Some(&gio::File::for_path(thumb)));
    }
    overlay.set_child(Some(&pic));

    // Bottom gradient scrim + title.
    let scrim = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    scrim.add_css_class("wp-scrim");
    scrim.set_valign(gtk4::Align::End);
    scrim.set_hexpand(true);
    let title = gtk4::Label::new(Some(&entry.name));
    title.add_css_class("wp-title");
    title.set_xalign(0.0);
    title.set_halign(gtk4::Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    scrim.append(&title);
    overlay.add_overlay(&scrim);

    // Kind badge (top-left).
    let badge = gtk4::Label::new(Some(kind_badge(entry.kind)));
    badge.add_css_class("wp-badge");
    badge.set_halign(gtk4::Align::Start);
    badge.set_valign(gtk4::Align::Start);
    overlay.add_overlay(&badge);

    // Active pill (top-right).
    if active {
        let pill = gtk4::Label::new(Some("ACTIVE"));
        pill.add_css_class("wp-active-pill");
        pill.set_halign(gtk4::Align::End);
        pill.set_valign(gtk4::Align::Start);
        overlay.add_overlay(&pill);
    }

    // Missing-source warning.
    if entry.broken {
        let warn = gtk4::Label::new(Some("MISSING"));
        warn.add_css_class("wp-badge");
        warn.add_css_class("warning");
        warn.set_halign(gtk4::Align::Start);
        warn.set_valign(gtk4::Align::End);
        warn.set_tooltip_text(entry.error.as_deref().or(Some("Source file not found")));
        overlay.add_overlay(&warn);
        overlay.set_opacity(0.65);
    }

    // Hover-revealed Edit button (bottom-right).
    let edit = gtk4::Button::from_icon_name("document-edit-symbolic");
    edit.add_css_class("wp-edit");
    edit.add_css_class("circular");
    edit.set_halign(gtk4::Align::End);
    edit.set_valign(gtk4::Align::End);
    edit.set_visible(false);
    edit.set_tooltip_text(Some("Edit & crop"));
    {
        let state_e = state.clone();
        let stack_e = stack.clone();
        edit.connect_clicked(move |_| {
            state_e.borrow_mut().editing_idx = Some(idx);
            stack_e.set_visible_child_name("editor");
        });
    }
    overlay.add_overlay(&edit);

    let motion = gtk4::EventControllerMotion::new();
    {
        let edit = edit.clone();
        motion.connect_enter(move |_, _, _| edit.set_visible(true));
    }
    {
        let edit = edit.clone();
        motion.connect_leave(move |_| edit.set_visible(false));
    }
    overlay.add_controller(motion);

    // Single click = apply; double click = open editor.
    let click = GestureClick::new();
    {
        let state_c = state.clone();
        let stack_c = stack.clone();
        click.connect_released(move |_, n_press, _, _| {
            if n_press == 1 {
                apply_entry_by_idx(state_c.clone(), idx);
            } else if n_press == 2 {
                state_c.borrow_mut().editing_idx = Some(idx);
                stack_c.set_visible_child_name("editor");
            }
        });
    }
    overlay.add_controller(click);

    // Right click = context menu (Set / Edit / Rename / Remove).
    let rclick = GestureClick::new();
    rclick.set_button(gtk4::gdk::BUTTON_SECONDARY);
    {
        let state_c = state.clone();
        let stack_c = stack.clone();
        let overlay_c = overlay.clone();
        rclick.connect_pressed(move |_, _, x, y| {
            show_card_menu(&overlay_c, state_c.clone(), stack_c.clone(), idx, x, y);
        });
    }
    overlay.add_controller(rclick);

    // Video/GIF cards play a muted, looping preview while hovered.
    if let Some(video) = preview_video_path(entry) {
        super::hover_preview::attach(&overlay, &pic, video, entry.thumbnail.clone());
    }

    overlay
}

/// Home sections, in display order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Images,
    Videos,
    Gifs,
}

const CATEGORY_ORDER: [Category; 3] = [Category::Images, Category::Videos, Category::Gifs];

fn category_label(c: Category) -> &'static str {
    match c {
        Category::Images => "Images",
        Category::Videos => "Videos",
        Category::Gifs => "GIFs",
    }
}

fn is_gif(p: &std::path::Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gif"))
        .unwrap_or(false)
}

fn entry_category(entry: &LibraryEntry) -> Category {
    match entry.kind {
        Kind::Image | Kind::Slideshow => Category::Images,
        Kind::Playlist => Category::Videos,
        Kind::Video => {
            if entry.path.as_deref().map(is_gif).unwrap_or(false) {
                Category::Gifs
            } else {
                Category::Videos
            }
        }
    }
}

/// The video file to preview on hover, if this entry is a (non-slideshow) video
/// or GIF. Images and slideshows have nothing to play.
fn preview_video_path(entry: &LibraryEntry) -> Option<PathBuf> {
    match entry.kind {
        Kind::Video => entry.path.clone(),
        Kind::Playlist => entry.paths.first().cloned(),
        _ => None,
    }
}

/// Right-click context menu for a library card.
fn show_card_menu(
    parent: &gtk4::Overlay,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    idx: usize,
    x: f64,
    y: f64,
) {
    let pop = gtk4::Popover::new();
    pop.set_parent(parent);
    pop.set_has_arrow(false);
    pop.set_halign(gtk4::Align::Start);
    pop.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

    let menu = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    menu.set_margin_top(4);
    menu.set_margin_bottom(4);
    menu.set_margin_start(4);
    menu.set_margin_end(4);

    let item = |label: &str| {
        let b = gtk4::Button::with_label(label);
        b.add_css_class("flat");
        if let Some(lbl) = b.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
            lbl.set_xalign(0.0);
        }
        b
    };

    let set = item("Set as wallpaper");
    {
        let s = state.clone();
        let p = pop.clone();
        set.connect_clicked(move |_| {
            apply_entry_by_idx(s.clone(), idx);
            p.popdown();
        });
    }
    menu.append(&set);

    let edit = item("Edit / Crop…");
    {
        let s = state.clone();
        let st = stack.clone();
        let p = pop.clone();
        edit.connect_clicked(move |_| {
            s.borrow_mut().editing_idx = Some(idx);
            st.set_visible_child_name("editor");
            p.popdown();
        });
    }
    menu.append(&edit);

    let rename = item("Rename…");
    {
        let s = state.clone();
        let p = pop.clone();
        let parent = parent.clone();
        rename.connect_clicked(move |_| {
            p.popdown();
            rename_entry(&parent, s.clone(), idx);
        });
    }
    menu.append(&rename);

    menu.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let remove = item("Remove from library");
    remove.add_css_class("destructive-action");
    {
        let s = state.clone();
        let p = pop.clone();
        remove.connect_clicked(move |_| {
            remove_entry_by_idx(s.clone(), idx);
            p.popdown();
        });
    }
    menu.append(&remove);

    pop.set_child(Some(&menu));
    pop.connect_closed(|p| p.unparent());
    pop.popup();
}

/// Remove a library entry (and its cached thumbnail). Does not touch the
/// original media file. Refreshes the grid afterwards.
fn remove_entry_by_idx(state: Rc<RefCell<AppState>>, idx: usize) {
    {
        let mut s = state.borrow_mut();
        if idx >= s.entries.len() {
            return;
        }
        let entry = s.entries.remove(idx);
        if let Some(thumb) = &entry.thumbnail {
            std::fs::remove_file(thumb).ok();
        }
        save_entries(&s.entries).ok();
    }
    show_toast(&state, "Removed from library");
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// Small inline popover to rename a library entry.
fn rename_entry(parent: &gtk4::Overlay, state: Rc<RefCell<AppState>>, idx: usize) {
    let current = state
        .borrow()
        .entries
        .get(idx)
        .map(|e| e.name.clone())
        .unwrap_or_default();

    let pop = gtk4::Popover::new();
    pop.set_parent(parent);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row.set_margin_top(6);
    row.set_margin_bottom(6);
    row.set_margin_start(6);
    row.set_margin_end(6);

    let entry = gtk4::Entry::new();
    entry.set_text(&current);
    entry.set_hexpand(true);
    let save = gtk4::Button::from_icon_name("emblem-ok-symbolic");
    save.add_css_class("suggested-action");
    row.append(&entry);
    row.append(&save);
    pop.set_child(Some(&row));

    {
        let state = state.clone();
        let entry = entry.clone();
        let pop = pop.clone();
        save.connect_clicked(move |_| {
            commit_rename(&state, idx, &entry.text());
            pop.popdown();
        });
    }
    {
        let state = state.clone();
        let pop = pop.clone();
        entry.connect_activate(move |e| {
            commit_rename(&state, idx, &e.text());
            pop.popdown();
        });
    }

    pop.connect_closed(|p| p.unparent());
    pop.popup();
    entry.grab_focus();
}

fn commit_rename(state: &Rc<RefCell<AppState>>, idx: usize, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    {
        let mut s = state.borrow_mut();
        if let Some(e) = s.entries.get_mut(idx) {
            e.name = name.to_string();
        }
        save_entries(&s.entries).ok();
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

fn apply_entry_by_idx(state: Rc<RefCell<AppState>>, idx: usize) {
    let name = {
        let mut s = state.borrow_mut();
        let Some(entry) = s.entries.get_mut(idx) else {
            return;
        };
        if entry.broken {
            return;
        }
        entry.touch();
        let wallpaper = entry.to_wallpaper();
        let name = entry.name.clone();
        s.config.wallpaper = wallpaper;
        s.config.enabled = true;
        name
    };
    let ok = {
        let s = state.borrow();
        let r = daemon_ctl::ensure_daemon_and_apply(&s.config);
        save_entries(&s.entries).ok();
        if let Err(e) = &r {
            log::error!("failed to apply wallpaper: {e}");
        }
        r.is_ok()
    };
    if ok {
        show_toast(&state, &format!("“{name}” set as wallpaper"));
    } else {
        show_toast(&state, "Couldn’t start the wallpaper. Run frescod --check");
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

// ─── Editor view ──────────────────────────────────────────────────────────────

fn build_editor_view(state: Rc<RefCell<AppState>>, stack: &gtk4::Stack) -> gtk4::Box {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let header = adw::HeaderBar::new();
    let title_widget = adw::WindowTitle::new("Edit wallpaper", "");
    header.set_title_widget(Some(&title_widget));
    let back = gtk4::Button::from_icon_name("go-previous-symbolic");
    back.add_css_class("flat");
    back.set_tooltip_text(Some("Back to library"));
    {
        let stack2 = stack.clone();
        back.connect_clicked(move |_| {
            stack2.set_visible_child_name("library");
        });
    }
    header.pack_start(&back);
    root.append(&header);

    // Full-width two-pane editor: a large preview on the left, controls on the
    // right. Slideshows show a looping transition preview; other media show the
    // crop editor.
    let split = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    split.set_vexpand(true);

    let preview_pane = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    preview_pane.set_hexpand(true);
    preview_pane.set_vexpand(true);
    preview_pane.set_valign(gtk4::Align::Center);
    preview_pane.set_margin_start(20);
    preview_pane.set_margin_end(16);
    preview_pane.set_margin_top(20);
    preview_pane.set_margin_bottom(20);

    // Looping transition preview (shown for slideshows in place of the crop tool).
    let transition_preview = Rc::new(super::transition_preview::TransitionPreview::new());
    let tp_frame = gtk4::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
    tp_frame.set_child(Some(&transition_preview.root));
    tp_frame.set_vexpand(false);
    tp_frame.set_hexpand(true);
    tp_frame.set_visible(false);

    let controls = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    controls.set_width_request(360);
    controls.set_valign(gtk4::Align::Center);
    controls.set_margin_start(8);
    controls.set_margin_end(20);
    controls.set_margin_top(20);
    controls.set_margin_bottom(20);

    // Crop editor, framed at the desktop's 16:9 aspect so the preview reads as a
    // monitor — not a tall, phone-shaped box. AspectFrame locks the ratio at any
    // window height; vexpand(false) stops the inner Picture's vexpand from
    // stretching it vertically. The crop rectangle is constrained to 16:9 too, so
    // what you frame matches what fills the screen.
    let crop_editor = super::preview::CropEditor::new(Some(16.0 / 9.0));
    crop_editor.overlay.add_css_class("wp-thumb");
    crop_editor.overlay.add_css_class("crop-frame");
    crop_editor.overlay.set_overflow(gtk4::Overflow::Hidden);
    let crop_frame = gtk4::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
    crop_frame.set_child(Some(&crop_editor.overlay));
    crop_frame.set_vexpand(false);
    crop_frame.set_hexpand(true);
    preview_pane.append(&crop_frame);
    preview_pane.append(&tp_frame);

    let reset_crop = gtk4::Button::with_label("Reset crop");
    reset_crop.add_css_class("flat");
    reset_crop.set_halign(gtk4::Align::End);
    reset_crop.set_margin_top(6);
    {
        let ce = crop_editor.clone();
        reset_crop.connect_clicked(move |_| ce.reset());
    }
    preview_pane.append(&reset_crop);

    // Preferences group.
    let prefs = adw::PreferencesGroup::new();
    prefs.set_margin_top(14);

    let fit_row = adw::ComboRow::new();
    fit_row.set_title("Fit");
    fit_row.set_subtitle("How the media fills the screen");
    fit_row.set_model(Some(&gtk4::StringList::new(&[
        "Cover", "Contain", "Stretch",
    ])));
    prefs.add(&fit_row);

    let mute_row = adw::ActionRow::new();
    mute_row.set_title("Muted");
    let mute_sw = gtk4::Switch::new();
    mute_sw.set_active(true);
    mute_sw.set_valign(gtk4::Align::Center);
    mute_row.add_suffix(&mute_sw);
    mute_row.set_activatable_widget(Some(&mute_sw));
    prefs.add(&mute_row);

    let vol_row = adw::ActionRow::new();
    vol_row.set_title("Volume");
    let vol_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 5.0);
    vol_scale.set_value(50.0);
    vol_scale.set_hexpand(true);
    vol_scale.set_size_request(180, -1);
    vol_scale.set_valign(gtk4::Align::Center);
    vol_row.add_suffix(&vol_scale);
    prefs.add(&vol_row);

    // Slideshow cadence (shown only for slideshows; see the on-enter handler).
    let interval_row = adw::ComboRow::new();
    interval_row.set_title("Interval");
    interval_row.set_subtitle("How often the slideshow advances");
    interval_row.set_model(Some(&gtk4::StringList::new(&[
        "5 seconds",
        "15 seconds",
        "30 seconds",
        "1 minute",
        "5 minutes",
        "10 minutes",
    ])));
    interval_row.set_selected(2);
    prefs.add(&interval_row);

    // Slideshow transition effect (shown only for slideshows).
    let transition_row = adw::ComboRow::new();
    transition_row.set_title("Transition");
    transition_row.set_subtitle("Effect when the image changes");
    transition_row.set_model(Some(&gtk4::StringList::new(&[
        "None",
        "Crossfade",
        "Fade to black",
        "Ken Burns",
    ])));
    transition_row.set_selected(1);
    prefs.add(&transition_row);

    controls.append(&prefs);

    // Set Wallpaper button.
    let set_btn = gtk4::Button::with_label("Set as wallpaper");
    set_btn.add_css_class("suggested-action");
    set_btn.add_css_class("pill");
    set_btn.add_css_class("set-btn");
    set_btn.set_margin_top(18);

    {
        let state_set = state.clone();
        let stack_set = stack.clone();
        let crop_ref = crop_editor.clone();
        let fit_ref = fit_row.clone();
        let mute_ref = mute_sw.clone();
        let vol_ref = vol_scale.clone();
        let interval_ref = interval_row.clone();
        let transition_ref = transition_row.clone();
        set_btn.connect_clicked(move |_| {
            let crop = crop_ref.crop();
            let fit = match fit_ref.selected() {
                1 => Fit::Contain,
                2 => Fit::Stretch,
                _ => Fit::Cover,
            };
            let interval = interval_secs(interval_ref.selected());
            let transition = transition_from_index(transition_ref.selected());
            let name = {
                let mut s = state_set.borrow_mut();
                s.config.wallpaper.crop = crop;
                s.config.wallpaper.fit = fit;
                s.config.wallpaper.mute = mute_ref.is_active();
                s.config.wallpaper.volume = vol_ref.value() as u8;
                s.config.enabled = true;
                if let Some(ss) = s.config.wallpaper.slideshow.as_mut() {
                    ss.interval_s = interval;
                    ss.transition = transition;
                }
                let idx = s.editing_idx;
                if let Some(e) = idx.and_then(|i| s.entries.get_mut(i)) {
                    if e.kind == Kind::Slideshow {
                        e.interval_s = Some(interval);
                        e.transition = Some(transition);
                    }
                }
                save_entries(&s.entries).ok();
                idx.and_then(|i| s.entries.get(i))
                    .map(|e| e.name.clone())
                    .unwrap_or_default()
            };
            let ok = {
                let s = state_set.borrow();
                match daemon_ctl::ensure_daemon_and_apply(&s.config) {
                    Ok(_) => true,
                    Err(e) => {
                        log::error!("failed to apply: {e}");
                        false
                    }
                }
            };
            if ok {
                log::info!("Wallpaper set; close this window, it keeps playing");
                show_toast(
                    &state_set,
                    &format!("“{name}” set. Close the window; it keeps playing"),
                );
                stack_set.set_visible_child_name("library");
            } else {
                show_toast(
                    &state_set,
                    "Couldn’t start the wallpaper. Run frescod --check",
                );
            }
        });
    }
    controls.append(&set_btn);

    // Bound the preview width with a Clamp: it grows with the window but never
    // past `maximum_size`. This keeps the controls column on-screen no matter how
    // big the media is, and gives the transition preview's stage a stable size so
    // its per-frame sizing can't feed back into the layout.
    let preview_clamp = adw::Clamp::new();
    preview_clamp.set_maximum_size(1600);
    preview_clamp.set_tightening_threshold(1100);
    preview_clamp.set_hexpand(true);
    preview_clamp.set_child(Some(&preview_pane));

    split.append(&preview_clamp);
    split.append(&controls);
    root.append(&split);

    // When entering the editor, load the selected entry's preview + settings.
    {
        let ce = crop_editor.clone();
        let fit_ref = fit_row.clone();
        let mute_ref = mute_sw.clone();
        let vol_ref = vol_scale.clone();
        let interval_ref = interval_row.clone();
        let transition_ref = transition_row.clone();
        let mute_row_ref = mute_row.clone();
        let vol_row_ref = vol_row.clone();
        let title_ref = title_widget.clone();
        let crop_frame_ref = crop_frame.clone();
        let tp_frame_ref = tp_frame.clone();
        let reset_crop_ref = reset_crop.clone();
        let tp = transition_preview.clone();
        let state2 = state.clone();
        stack.connect_visible_child_name_notify(move |s| {
            if s.visible_child_name().as_deref() != Some("editor") {
                tp.stop(); // free the preview timer when leaving the editor
                return;
            }
            let st = state2.borrow();
            // Show the thumbnail (videos) or the image itself as the crop preview.
            if let Some(entry) = st.editing_idx.and_then(|i| st.entries.get(i)) {
                title_ref.set_subtitle(&entry.name);
                if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
                    ce.set_media(thumb);
                } else if let Some(p) = entry
                    .path
                    .as_deref()
                    .or_else(|| entry.paths.first().map(|p| p.as_path()))
                    .filter(|p| p.exists())
                {
                    ce.set_media(p);
                }
                // Audio rows only apply to video; interval only to slideshows.
                let has_audio = matches!(entry.kind, Kind::Video | Kind::Playlist);
                mute_row_ref.set_visible(has_audio);
                vol_row_ref.set_visible(has_audio);
                let is_slideshow = entry.kind == Kind::Slideshow;
                interval_ref.set_visible(is_slideshow);
                transition_ref.set_visible(is_slideshow);
                // Slideshows preview the transition; other media show the crop tool.
                crop_frame_ref.set_visible(!is_slideshow);
                reset_crop_ref.set_visible(!is_slideshow);
                tp_frame_ref.set_visible(is_slideshow);
                if is_slideshow {
                    interval_ref.set_selected(interval_index(entry.interval_s.unwrap_or(30)));
                    let (a, b) = slideshow_preview_images(entry);
                    tp.set_images(a, b);
                    transition_ref
                        .set_selected(transition_index(entry.transition.unwrap_or_default()));
                    // Follow the combo (never the raw entry) so a removed
                    // transition like Slide can't reach the preview.
                    tp.set_transition(transition_from_index(transition_ref.selected()));
                } else {
                    tp.stop();
                }
            }
            ce.set_crop(st.config.wallpaper.crop);
            fit_ref.set_selected(match st.config.wallpaper.fit {
                Fit::Cover => 0,
                Fit::Contain => 1,
                Fit::Stretch => 2,
            });
            mute_ref.set_active(st.config.wallpaper.mute);
            vol_ref.set_value(st.config.wallpaper.volume as f64);
        });
    }

    // Live-preview the transition the moment the user picks one.
    {
        let tp = transition_preview.clone();
        transition_row.connect_selected_notify(move |row| {
            tp.set_transition(transition_from_index(row.selected()));
        });
    }

    root
}

/// The first two images of a slideshow entry, for the editor's transition
/// preview. Falls back the second to the first when only one image is present.
fn slideshow_preview_images(entry: &LibraryEntry) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut imgs: Vec<PathBuf> = if !entry.paths.is_empty() {
        entry.paths.iter().take(2).cloned().collect()
    } else if let Some(folder) = &entry.folder {
        let mut v: Vec<PathBuf> = std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| library::is_image(p))
            .collect();
        v.sort();
        v.truncate(2);
        v
    } else {
        Vec::new()
    };
    let first = (!imgs.is_empty()).then(|| imgs.remove(0));
    let second = imgs.into_iter().next().or_else(|| first.clone());
    (first, second)
}

// ─── Advanced dialog ──────────────────────────────────────────────────────────

fn show_advanced_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let dialog = adw::PreferencesWindow::new();
    dialog.set_transient_for(Some(window));
    dialog.set_modal(true);
    dialog.set_title(Some("Advanced"));

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    group.set_title("Video quality");

    let scale_row = adw::ComboRow::new();
    scale_row.set_title("Scaling quality");
    scale_row.set_subtitle("Balanced: low CPU  |  High: Lanczos resampling");
    scale_row.set_model(Some(&gtk4::StringList::new(&["Balanced", "High"])));
    let current = u32::from(matches!(state.borrow().config.scaling, Scaling::High));
    scale_row.set_selected(current);
    scale_row.connect_selected_notify(move |row| {
        let mut s = state.borrow_mut();
        s.config.scaling = if row.selected() == 1 {
            Scaling::High
        } else {
            Scaling::Balanced
        };
        s.config.save().ok();
    });

    group.add(&scale_row);
    page.add(&group);
    dialog.add(&page);
    dialog.present();
}

// ─── File picker ──────────────────────────────────────────────────────────────

fn open_file_picker(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    editing_idx: Option<usize>,
) {
    let chooser = gtk4::FileChooserNative::new(
        Some("Choose Wallpaper"),
        Some(window),
        FileChooserAction::Open,
        Some("Open"),
        Some("Cancel"),
    );
    chooser.set_select_multiple(true);

    let video_pat = ["*.mp4", "*.webm", "*.mkv", "*.avi", "*.mov", "*.gif"];
    let image_pat = ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.bmp"];

    // "All supported" first → it's the default filter, so videos AND images
    // both show without the user switching the dropdown.
    let allf = gtk4::FileFilter::new();
    allf.set_name(Some("All supported"));
    for p in video_pat.iter().chain(image_pat.iter()) {
        allf.add_pattern(p);
    }
    chooser.add_filter(&allf);

    let vf = gtk4::FileFilter::new();
    vf.set_name(Some("Video files"));
    for p in &video_pat {
        vf.add_pattern(p);
    }
    chooser.add_filter(&vf);

    let imf = gtk4::FileFilter::new();
    imf.set_name(Some("Image files"));
    for p in &image_pat {
        imf.add_pattern(p);
    }
    chooser.add_filter(&imf);

    let state_cb = state.clone();
    chooser.connect_response(move |ch, resp| {
        // Release the keep-alive ref now that the dialog has answered (also
        // breaks the chooser↔state reference cycle). GTK keeps `ch` valid for
        // the duration of this handler.
        state_cb.borrow_mut().current_picker = None;
        if resp != ResponseType::Accept {
            return;
        }

        // Collect selected files.
        let model = ch.files();
        let n = model.n_items();
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        for i in 0..n {
            if let Some(obj) = model.item(i) {
                if let Ok(file) = obj.downcast::<gio::File>() {
                    if let Some(p) = file.path() {
                        paths.push(p);
                    }
                }
            }
        }
        if paths.is_empty() {
            return;
        }

        let mut entry = if paths.len() > 1 {
            // All images → an image slideshow that loops on a timer. Mixed/videos
            // → a video playlist (images in a playlist would flash every second).
            if paths.iter().all(|p| library::is_image(p)) {
                library::LibraryEntry::new_image_set(paths)
            } else {
                library::LibraryEntry::new_playlist(paths)
            }
        } else {
            let p = paths.remove(0);
            if library::is_video(&p) {
                library::LibraryEntry::new_video(p)
            } else {
                library::LibraryEntry::new_image(p)
            }
        };
        entry.generate_thumbnail();

        let mut s = state_cb.borrow_mut();
        let idx = if let Some(ei) = editing_idx {
            s.entries[ei] = entry;
            ei
        } else {
            s.entries.push(entry);
            s.entries.len() - 1
        };
        s.config.wallpaper = s.entries[idx].to_wallpaper();
        s.editing_idx = Some(idx);
        save_entries(&s.entries).ok();
        drop(s);
        stack.set_visible_child_name("editor");
    });
    state.borrow_mut().current_picker = Some(chooser.clone());
    chooser.show();
}

/// Folder picker → create an image slideshow entry, then open the editor.
fn open_folder_picker(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
) {
    let chooser = gtk4::FileChooserNative::new(
        Some("Choose Slideshow Folder"),
        Some(window),
        FileChooserAction::SelectFolder,
        Some("Select"),
        Some("Cancel"),
    );
    let state_cb = state.clone();
    chooser.connect_response(move |ch, resp| {
        state_cb.borrow_mut().current_picker = None;
        if resp != ResponseType::Accept {
            return;
        }
        let Some(folder) = ch.file().and_then(|f| f.path()) else {
            return;
        };
        let mut entry = library::LibraryEntry::new_slideshow(folder);
        entry.generate_thumbnail();
        let mut s = state_cb.borrow_mut();
        s.entries.push(entry);
        let idx = s.entries.len() - 1;
        s.config.wallpaper = s.entries[idx].to_wallpaper();
        s.editing_idx = Some(idx);
        save_entries(&s.entries).ok();
        drop(s);
        stack.set_visible_child_name("editor");
    });
    state.borrow_mut().current_picker = Some(chooser.clone());
    chooser.show();
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Uppercase section label styled as an overline (see theme.rs `.overline`).
fn overline(text: &str) -> gtk4::Label {
    let l = gtk4::Label::new(Some(&text.to_uppercase()));
    l.add_css_class("overline");
    l.set_xalign(0.0);
    l.set_halign(gtk4::Align::Start);
    l
}

/// Icon + label content for a button (Adwaita ButtonContent).
fn button_content(icon: &str, label: &str) -> adw::ButtonContent {
    let c = adw::ButtonContent::new();
    c.set_icon_name(icon);
    c.set_label(label);
    c
}

/// A label + trailing switch row for the menu popover.
fn switch_row<F: Fn(bool) + 'static>(label: &str, active: bool, on_toggle: F) -> gtk4::Box {
    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    hbox.set_margin_start(4);
    hbox.set_margin_end(4);
    let lbl = gtk4::Label::new(Some(label));
    lbl.set_hexpand(true);
    lbl.set_xalign(0.0);
    let sw = gtk4::Switch::new();
    sw.set_active(active);
    sw.set_valign(gtk4::Align::Center);
    sw.connect_active_notify(move |sw| on_toggle(sw.is_active()));
    hbox.append(&lbl);
    hbox.append(&sw);
    hbox
}

fn show_toast(state: &Rc<RefCell<AppState>>, msg: &str) {
    let toast = adw::Toast::new(msg);
    toast.set_timeout(4);
    state.borrow().toast.add_toast(toast);
}

fn entry_is_active(entry: &LibraryEntry, cfg: &Config) -> bool {
    if !cfg.enabled {
        return false;
    }
    let w = &cfg.wallpaper;
    if entry.kind != w.kind {
        return false;
    }
    match entry.kind {
        Kind::Video | Kind::Image => entry.path.is_some() && entry.path == w.path,
        Kind::Playlist => !entry.paths.is_empty() && entry.paths == w.paths,
        Kind::Slideshow => match w.slideshow.as_ref() {
            Some(s) if !entry.paths.is_empty() => s.paths == entry.paths,
            Some(s) => entry.folder.is_some() && s.folder == entry.folder,
            None => false,
        },
    }
}

fn kind_badge(kind: Kind) -> &'static str {
    match kind {
        Kind::Video => "VIDEO",
        Kind::Image => "IMAGE",
        Kind::Playlist => "PLAYLIST",
        Kind::Slideshow => "SLIDES",
    }
}

fn accent_name(accent: Accent) -> &'static str {
    match accent {
        Accent::Blue => "Blue",
        Accent::Teal => "Teal",
        Accent::Green => "Green",
        Accent::Amber => "Amber",
        Accent::Coral => "Coral",
        Accent::Graphite => "Graphite",
    }
}

/// Slideshow interval choices (seconds), matched 1:1 with the editor combo rows
/// "5 seconds / 15 seconds / 30 seconds / 1 minute / 5 minutes / 10 minutes".
const INTERVAL_OPTIONS: [u64; 6] = [5, 15, 30, 60, 300, 600];

fn interval_secs(index: u32) -> u64 {
    INTERVAL_OPTIONS.get(index as usize).copied().unwrap_or(30)
}

fn interval_index(secs: u64) -> u32 {
    INTERVAL_OPTIONS
        .iter()
        .position(|&s| s == secs)
        .unwrap_or(2) as u32
}

fn transition_from_index(index: u32) -> Transition {
    match index {
        1 => Transition::Crossfade,
        2 => Transition::Fade,
        3 => Transition::KenBurns,
        _ => Transition::None,
    }
}

fn transition_index(t: Transition) -> u32 {
    match t {
        Transition::None => 0,
        Transition::Crossfade => 1,
        Transition::Fade => 2,
        Transition::KenBurns => 3,
        // Slide was removed from the picker; show legacy entries as Crossfade.
        Transition::Slide => 1,
    }
}

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
fn build_update_banner(
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
fn show_changelog_modal(window: &adw::ApplicationWindow, version: &str, notes: &str) {
    let dialog = adw::Window::new();
    dialog.add_css_class("glass");
    dialog.set_transient_for(Some(window));
    dialog.set_modal(true);
    dialog.set_title(Some(&format!("What's new in {version}")));
    dialog.set_default_size(660, 680);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&adw::HeaderBar::new());

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

    dialog.set_content(Some(&content));
    dialog.present();
}

// ─── Feedback prompt + admin notifications (Supabase) ──────────────────────────

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// One-time opt-in feedback prompt (after a week of use) + a poll for
/// admin-pushed notifications. Runs once at startup.
fn run_startup_checks(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let now = unix_now();
    {
        let mut s = state.borrow_mut();
        if s.config.first_run_epoch == 0 {
            s.config.first_run_epoch = now;
            s.config.save().ok();
        }
    }
    let (first_run, prompted, enabled) = {
        let s = state.borrow();
        (
            s.config.first_run_epoch,
            s.config.feedback_prompted,
            s.config.enabled,
        )
    };
    const WEEK: u64 = 7 * 24 * 60 * 60;
    if !prompted && enabled && now.saturating_sub(first_run) >= WEEK {
        show_feedback_dialog(window, state.clone());
    }
    poll_notifications(window, state);
}

fn submit_feedback_async(rating: i8, comment: &gtk4::Entry, state: &Rc<RefCell<AppState>>) {
    let text = comment.text().to_string();
    let note = if text.trim().is_empty() {
        None
    } else {
        Some(text)
    };
    std::thread::spawn(move || {
        crate::supabase::submit_feedback(rating, note).ok();
    });
    state
        .borrow()
        .toast
        .add_toast(adw::Toast::new("Thanks for the feedback!"));
}

fn show_feedback_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    {
        let mut s = state.borrow_mut();
        s.config.feedback_prompted = true; // ask at most once
        s.config.save().ok();
    }

    let dialog = adw::Window::new();
    dialog.add_css_class("glass");
    dialog.set_transient_for(Some(window));
    dialog.set_modal(true);
    dialog.set_title(Some("Feedback"));
    dialog.set_default_size(420, -1);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&adw::HeaderBar::new());

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    let heading = gtk4::Label::new(Some("Enjoying Fresco?"));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    inner.append(&heading);

    let prompt = gtk4::Label::new(Some(
        "Your rating is anonymous. An optional note helps shape what comes next.",
    ));
    prompt.add_css_class("dialog-sub");
    prompt.set_wrap(true);
    prompt.set_xalign(0.0);
    inner.append(&prompt);

    let comment = gtk4::Entry::new();
    comment.set_placeholder_text(Some("Anything we should know? (optional)"));
    comment.set_margin_top(4);
    inner.append(&comment);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_margin_top(6);
    let later = gtk4::Button::with_label("Not now");
    later.add_css_class("flat");
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let down = gtk4::Button::new();
    down.set_child(Some(&button_content("face-sad-symbolic", "Not great")));
    down.add_css_class("feedback-btn");
    let up = gtk4::Button::new();
    up.set_child(Some(&button_content("face-laugh-symbolic", "Loving it")));
    up.add_css_class("feedback-btn");
    up.add_css_class("suggested-action");
    buttons.append(&later);
    buttons.append(&spacer);
    buttons.append(&down);
    buttons.append(&up);
    inner.append(&buttons);

    content.append(&inner);
    dialog.set_content(Some(&content));

    {
        let comment = comment.clone();
        let state = state.clone();
        let d = dialog.clone();
        up.connect_clicked(move |_| {
            submit_feedback_async(1, &comment, &state);
            d.close();
        });
    }
    {
        let comment = comment.clone();
        let state = state.clone();
        let d = dialog.clone();
        down.connect_clicked(move |_| {
            submit_feedback_async(-1, &comment, &state);
            d.close();
        });
    }
    {
        let d = dialog.clone();
        later.connect_clicked(move |_| d.close());
    }

    dialog.present();
}

fn poll_notifications(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let list = crate::supabase::fetch_notifications().unwrap_or_default();
        let _ = tx.send_blocking(list);
    });
    let window = window.clone();
    glib::spawn_future_local(async move {
        let Ok(list) = rx.recv().await else {
            return;
        };
        let next = {
            let s = state.borrow();
            list.into_iter()
                .find(|n| !s.config.seen_notifications.contains(&n.id))
        };
        if let Some(n) = next {
            show_notification(&window, state, n);
        }
    });
}

fn show_notification(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    notif: crate::supabase::Notification,
) {
    {
        let mut s = state.borrow_mut();
        s.config.seen_notifications.push(notif.id.clone());
        s.config.save().ok();
    }
    // AdwToast's button triggers a GAction (not a signal in this binding), so
    // register a window action that opens the details modal.
    let action = gio::SimpleAction::new("fresco-notif-details", None);
    {
        let window = window.clone();
        let notif = notif.clone();
        action.connect_activate(move |_, _| show_notification_modal(&window, &notif));
    }
    window.add_action(&action);

    let toast = adw::Toast::new(&notif.title);
    toast.set_button_label(Some("Details"));
    toast.set_action_name(Some("win.fresco-notif-details"));
    toast.set_timeout(0);
    state.borrow().toast.add_toast(toast);
}

fn show_notification_modal(window: &adw::ApplicationWindow, notif: &crate::supabase::Notification) {
    let dialog = adw::Window::new();
    dialog.add_css_class("glass");
    dialog.set_transient_for(Some(window));
    dialog.set_modal(true);
    dialog.set_title(Some(&notif.title));
    dialog.set_default_size(440, -1);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&adw::HeaderBar::new());

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    inner.set_margin_start(20);
    inner.set_margin_end(20);
    inner.set_margin_top(8);
    inner.set_margin_bottom(20);

    let body = gtk4::Label::new(Some(&notif.body));
    body.set_wrap(true);
    body.set_xalign(0.0);
    body.set_selectable(true);
    inner.append(&body);

    if let Some(url) = notif.url.clone() {
        let open = gtk4::Button::with_label("Open link");
        open.add_css_class("suggested-action");
        open.set_halign(gtk4::Align::Start);
        let d = dialog.clone();
        open.connect_clicked(move |_| {
            let _ = gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>);
            d.close();
        });
        inner.append(&open);
    }

    content.append(&inner);
    dialog.set_content(Some(&content));
    dialog.present();
}
