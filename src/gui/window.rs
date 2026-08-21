use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

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
    config::{
        Accent, Clock, ClockThemeCfg, Config, Disc, Fit, GradientMode, Kind, LyricAnchor,
        LyricStylePreset, Lyrics, PowerSaving, Scaling, ThemeMode, Transition, Visualizer,
        VisualizerStyleCfg, WidgetTheme, Widgets,
    },
    t, tf, APP_ID,
};

pub struct FrescoApplication {
    pub app: adw::Application,
}

/// Set when this process was launched with `--feedback` (the daemon's
/// feedback-reminder notification does this) and we became the primary
/// instance — build_ui opens the feedback dialog right after presenting.
static PENDING_FEEDBACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl FrescoApplication {
    pub fn new() -> Self {
        let app = adw::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE);
        app.connect_activate(build_ui);
        FrescoApplication { app }
    }

    pub fn run(&self, args: &[String]) -> i32 {
        // `--feedback` is ours, not GLib's: strip it before GApplication sees
        // it, and route it via the exported "open-feedback" action so it works
        // whether we become the primary instance or one is already running.
        let feedback = args.iter().any(|a| a == "--feedback");
        let argv: Vec<&str> = args
            .iter()
            .filter(|a| a.as_str() != "--feedback")
            .map(String::as_str)
            .collect();
        if feedback {
            PENDING_FEEDBACK.store(true, std::sync::atomic::Ordering::Relaxed);
            if self.app.register(None::<&gio::Cancellable>).is_ok() && self.app.is_remote() {
                // A primary instance exists: raise it and forward the intent
                // over D-Bus instead of starting a second main loop.
                self.app.activate();
                self.app.activate_action("open-feedback", None);
                return 0;
            }
        }
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

/// Jump to the crop/rotate editor for entry `idx` — the same surface the
/// file-picker add flow lands on, so link-added wallpapers get preview,
/// rotate, and crop before being set.
pub(crate) fn open_editor(state: &Rc<RefCell<AppState>>, stack: &gtk4::Stack, idx: usize) {
    state.borrow_mut().editing_idx = Some(idx);
    stack.set_visible_child_name("editor");
}

pub(crate) struct AppState {
    pub(crate) config: Config,
    pub(crate) entries: Vec<LibraryEntry>,
    editing_idx: Option<usize>,
    /// Keeps the native file/folder chooser alive until it responds. Without
    /// this, the local `FileChooserNative` is dropped when the open function
    /// returns, so the portal's reply never reaches our handler.
    current_picker: Option<gtk4::FileChooserNative>,
    /// Floating toast host that wraps the whole window (set once in build_ui).
    toast: adw::ToastOverlay,
    /// Rebuilds the library grid in place; installed by build_library_view so
    /// the active-wallpaper highlight can update without a view switch.
    pub(crate) refresh: Option<Rc<dyn Fn()>>,
    /// Empty slot the async "Update available" check populates in place;
    /// installed by build_library_view (mirrors `refresh`).
    pub(crate) update_banner_slot: Option<gtk4::Box>,
    /// `Some` while the library is in multi-select mode, holding the chosen
    /// entry ids. Keyed by id, not index, so a concurrent library change (a
    /// thumbnail probe finishing, an add) can't shift a selection onto the
    /// wrong wallpaper and delete it.
    selection: Option<std::collections::HashSet<String>>,
    /// Swaps the footer between its normal actions and the selection bar.
    sync_selection_ui: Option<Rc<dyn Fn()>>,
    /// User-created folders, loaded from `collections.json`. Kept next to the
    /// entries (not in `Config`) for the reasons `library::Collection`
    /// documents: a folder list must not ride on the file the daemon rewrites.
    pub(crate) collections: Vec<library::Collection>,
    /// Sticky sort + open-folder preference (`view.json`). Normalised at load
    /// so a scope pointing at a folder that no longer exists falls back to the
    /// whole library instead of showing an empty grid with no way out.
    pub(crate) view: library::LibraryView,
}

// ─── Main window ─────────────────────────────────────────────────────────────

fn build_ui(app: &adw::Application) {
    // The app id makes us D-Bus-unique: launching fresco again re-activates
    // this process. Present the existing window instead of building a
    // duplicate (with its own status-poll timer and startup checks).
    if let Some(existing) = app.active_window() {
        existing.present();
        return;
    }

    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Fresco"));
    window.set_default_size(880, 660);
    window.set_size_request(420, 480);
    window.set_icon_name(Some(APP_ID));

    // Ctrl+Q quits. `GApplication` has no built-in "quit" action, so register
    // one explicitly rather than relying on it existing for free.
    let quit_action = gio::SimpleAction::new("quit", None);
    {
        let app = app.clone();
        quit_action.connect_activate(move |_, _| app.quit());
    }
    app.add_action(&quit_action);
    app.set_accels_for_action("app.quit", &["<primary>q"]);

    let config = Config::load().unwrap_or_default();

    // Install + apply the theme before first paint so there is no flash.
    theme::install();
    theme::set_mode(config.theme_mode);
    theme::apply(config.accent, theme::resolve_dark(config.theme_mode));

    // Session capability drives the UI: every session gets the full app; the
    // limited ones (Wayland) also get an informational banner. No hard block.
    let capability = crate::capability::detect();

    // Mark broken entries (missing source files) before showing the library.
    let mut entries = load_entries().unwrap_or_default();
    for e in &mut entries {
        e.check_health();
    }

    let collections = library::load_collections().unwrap_or_default();
    let view = normalize_view(library::load_view(), &collections);

    let toast = adw::ToastOverlay::new();

    let state = Rc::new(RefCell::new(AppState {
        config,
        entries,
        editing_idx: None,
        current_picker: None,
        toast: toast.clone(),
        refresh: None,
        update_banner_slot: None,
        selection: None,
        sync_selection_ui: None,
        collections,
        view,
    }));

    // Re-apply the palette when the system light/dark resolution flips (System
    // mode). Resolve via the chosen mode so an explicit Light/Dark isn't undone
    // by this notify firing with a stale is_dark() right after set_mode.
    {
        let state = state.clone();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            let s = state.borrow();
            theme::apply(s.config.accent, theme::resolve_dark(s.config.theme_mode));
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
    match capability_banner_text(capability) {
        Some(text) => {
            // Stack the capability banner above the toast-wrapped content.
            let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            let banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            banner.add_css_class("capability-banner");
            banner.set_margin_start(12);
            banner.set_margin_end(12);
            banner.set_margin_top(10);
            let icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
            icon.set_valign(gtk4::Align::Start);
            let label = gtk4::Label::new(Some(text));
            label.set_wrap(true);
            label.set_xalign(0.0);
            label.set_hexpand(true);
            banner.append(&icon);
            banner.append(&label);
            outer.append(&banner);
            outer.append(&toast);
            window.set_content(Some(&outer));
        }
        None => window.set_content(Some(&toast)),
    }
    window.present();

    // Headless UI-smoke hook: open the gallery immediately (tests/ci only).
    if std::env::var("FRESCO_OPEN_GALLERY").ok().as_deref() == Some("1") {
        super::gallery::show_gallery_window(&window, state.clone());
    }

    // Deep link used by the daemon's feedback-reminder notification: clicking
    // "Send feedback" runs `fresco --feedback`, which lands here (directly for
    // a fresh primary instance, via the D-Bus action when we're already open).
    {
        let win_fb = window.clone();
        let state_fb = state.clone();
        let open_feedback = gio::SimpleAction::new("open-feedback", None);
        open_feedback.connect_activate(move |_, _| {
            win_fb.present();
            show_feedback_dialog(&win_fb, state_fb.clone());
        });
        app.add_action(&open_feedback);
    }
    if !state.borrow().config.telemetry_prompted
        || state.borrow().config.telemetry_consent_version < crate::telemetry::CONSENT_VERSION
    {
        // Consent before anything else — telemetry stays fully off until the
        // user answers (telemetry::tier() returns Unanswered until then).
        //
        // The version check re-asks installs that answered under older terms.
        // It is not a nag: declining used to mean total silence and now means
        // an identifier-free country ping, so the previous answer was given to
        // a different question and cannot be carried over.
        let win_c = window.clone();
        let state_c = state.clone();
        glib::idle_add_local_once(move || show_telemetry_consent_dialog(&win_c, state_c));
    } else if PENDING_FEEDBACK.swap(false, std::sync::atomic::Ordering::Relaxed) {
        let win_fb = window.clone();
        let state_fb = state.clone();
        glib::idle_add_local_once(move || show_feedback_dialog(&win_fb, state_fb));
    } else if crate::support::has_thread() {
        // A reply that arrived while Fresco was closed. Checked before the
        // tour because someone mid-conversation has already seen the tour.
        check_support_replies(&window, state.clone());
    } else if !state.borrow().config.tour_shown {
        // First launch: show the feature tour once, so right-click menus,
        // double-click editing, and the link importer don't go undiscovered.
        let win_t = window.clone();
        let state_t = state.clone();
        glib::idle_add_local_once(move || show_tour_dialog(&win_t, state_t));
    } else if state.borrow().config.onboarding_version < ONBOARDING_VERSION {
        // Existing user on a new version: they already sat through the tour, so
        // the what's-new flow is the only thing that will ever show them what
        // changed. It runs again on every launch until it is finished — nothing
        // is recorded until its last step (see ONBOARDING_STEPS).
        let win_o = window.clone();
        let state_o = state.clone();
        glib::idle_add_local_once(move || {
            show_onboarding_dialog(&win_o, state_o);
        });
    }

    // Lazily fill missing media metadata (resolution/fps/size) in the
    // background; saves + refreshes once when the whole batch lands.
    spawn_metadata_probe(&state);

    // Ctrl+K command palette.
    {
        let win_p = window.clone();
        let state_p = state.clone();
        let stack_p = stack.clone();
        let palette = gio::SimpleAction::new("command-palette", None);
        palette.connect_activate(move |_, _| {
            show_command_palette(&win_p, state_p.clone(), stack_p.clone());
        });
        window.add_action(&palette);
        app.set_accels_for_action("win.command-palette", &["<primary>k"]);
    }

    // Drag-and-drop media files anywhere on the window → the add flow (the
    // empty state promises "Drop videos or images here").
    {
        let state_d = state.clone();
        let stack_d = stack.clone();
        let win_d = window.clone();
        let drop = gtk4::DropTarget::new(
            gtk4::gdk::FileList::static_type(),
            gtk4::gdk::DragAction::COPY,
        );
        drop.connect_drop(move |_, value, _, _| {
            let Ok(list) = value.get::<gtk4::gdk::FileList>() else {
                return false;
            };
            let paths: Vec<std::path::PathBuf> = list
                .files()
                .into_iter()
                .filter_map(|f| f.path())
                .filter(|p| library::is_video(p) || library::is_image(p))
                .collect();
            if paths.is_empty() {
                show_toast(&state_d, t!("Drop video or image files to add them"));
                return false;
            }
            // Explicit policy, same as the Add button: one file goes straight
            // in, a batch asks whether it is ten wallpapers or one playlist.
            // Dropping used to silently fuse a multi-file drop into a playlist.
            if paths.len() > 1 {
                show_import_choice_dialog(&win_d, state_d.clone(), stack_d.clone(), paths);
            } else {
                add_media_paths(&state_d, &stack_d, paths, None);
            }
            true
        });
        window.add_controller(drop);
    }

    // Anonymous opt-in feedback + admin-pushed notifications (Supabase).
    run_startup_checks(&window, state);
}

/// Informational banner text for sessions where live playback is limited.
/// `None` for X11 (full live support — no banner needed).
fn capability_banner_text(cap: crate::capability::Capability) -> Option<&'static str> {
    use crate::capability::Capability;
    match cap {
        Capability::X11 | Capability::WaylandLayerShell => None,
        Capability::WaylandGnomeStatic => Some(
            t!("On GNOME Wayland, wallpapers are shown as a static frame. For live playback, use an X11 session or a layer-shell compositor (COSMIC, Hyprland, Sway, KDE Plasma)."),
        ),
    }
}

// ─── Library view ─────────────────────────────────────────────────────────────

/// Width-threshold layout bucket, resolved from the window's `default-width`
/// (no `AdwBreakpoint` here: this build targets libadwaita 1.1, which predates
/// it). Drives FlowBox column caps and footer button density.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LayoutBucket {
    Compact,
    Regular,
    Wide,
}

impl LayoutBucket {
    fn from_width(width: i32) -> Self {
        if width < 600 {
            LayoutBucket::Compact
        } else if width < 1200 {
            LayoutBucket::Regular
        } else {
            LayoutBucket::Wide
        }
    }

    /// (min, max) children per FlowBox line for this bucket. Paired with the
    /// ~260px card minimum, Wide resolves to ~5 cards per row at 1600px.
    fn flow_caps(self) -> (u32, u32) {
        match self {
            LayoutBucket::Compact => (1, 2),
            LayoutBucket::Regular => (2, 5),
            LayoutBucket::Wide => (2, 6),
        }
    }

    fn css_class(self) -> Option<&'static str> {
        match self {
            LayoutBucket::Compact => Some("compact-layout"),
            LayoutBucket::Regular => None,
            LayoutBucket::Wide => Some("wide-layout"),
        }
    }
}

fn build_library_view(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: &gtk4::Stack,
) -> gtk4::Box {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Width-threshold layout bucket (see LayoutBucket); re-resolved on every
    // `default-width` change but only acted on when it actually changes.
    let bucket: Rc<Cell<LayoutBucket>> =
        Rc::new(Cell::new(LayoutBucket::from_width(window.default_width())));

    // ── Header bar ──
    // Deliberately no pause/stop buttons: setting a wallpaper just runs it, and
    // picking another switches it. A stray "Stop" only created a confusing
    // dead/stopped state, so the model is kept dead-simple.
    // No subtitle line: the window title already says Fresco, and every pixel
    // of chrome we drop goes to the wallpapers (content-dominant grid).
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Fresco", "")));
    header.pack_start(&super::status::build_status_pill());

    let menu_btn = gtk4::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.add_css_class("flat");
    menu_btn.set_tooltip_text(Some(t!("Menu")));
    menu_btn.set_popover(Some(&build_menu_popover(window, state.clone())));
    header.pack_end(&menu_btn);
    root.append(&header);

    // ── "What's new" banner (shown once per version after an update) ──
    if let Some(banner) = super::updates::build_update_banner(window, state.clone()) {
        root.append(&banner);
    }

    // ── "Update available" banner slot (populated asynchronously once the
    // GitHub Releases check resolves; see run_startup_checks) ──
    let update_banner_slot = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.append(&update_banner_slot);
    state.borrow_mut().update_banner_slot = Some(update_banner_slot);

    // ── Search ──
    // Side margins are set by apply_layout_bucket (tighter in compact mode).
    let search = gtk4::SearchEntry::new();
    search.add_css_class("wp-search");
    search.set_placeholder_text(Some(t!("Search wallpapers…")));
    search.set_margin_top(8);
    search.set_margin_bottom(2);
    // Cap the entry at a readable width instead of stretching edge-to-edge.
    let search_clamp = adw::Clamp::new();
    search_clamp.set_maximum_size(560);
    search_clamp.set_tightening_threshold(480);
    search_clamp.set_child(Some(&search));
    root.append(&search_clamp);

    // ── Sort + folder bar ──
    // Two top-level MenuButtons rather than dropdowns or a sidebar: libadwaita
    // is pinned to 1.1 here, so `AdwOverlaySplitView` (1.4) is not available,
    // and a `GtkDropDown` inside the header menu is the input-eating trap
    // documented on `build_language_row`.
    let (toolbar, sync_toolbar) = build_library_toolbar(window, state.clone());
    root.append(&toolbar);

    // Ctrl+F focuses search, Ctrl+, opens the header menu.
    {
        let focus_search = gio::SimpleAction::new("focus-search", None);
        let search_a = search.clone();
        focus_search.connect_activate(move |_, _| {
            search_a.grab_focus();
        });
        window.add_action(&focus_search);

        let open_menu = gio::SimpleAction::new("open-menu", None);
        let menu_btn_a = menu_btn.clone();
        open_menu.connect_activate(move |_, _| menu_btn_a.popup());
        window.add_action(&open_menu);

        if let Some(app) = window.application() {
            app.set_accels_for_action("win.focus-search", &["<primary>f"]);
            app.set_accels_for_action("win.open-menu", &["<primary>comma"]);
        }
    }

    // ── Scrollable content ──
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);

    // Side margins are set by apply_layout_bucket (tighter in compact mode).
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.set_margin_bottom(8);

    // Recent row.
    let recent_label = overline(t!("Recent"));
    recent_label.set_margin_top(10);
    recent_label.set_margin_bottom(6);
    content.append(&recent_label);

    let recent_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    // Own horizontal scroller: at narrow widths the row can outgrow the
    // window, and the page-level ScrolledWindow above is vertical-only.
    let recent_scroll = gtk4::ScrolledWindow::new();
    recent_scroll.set_policy(PolicyType::Automatic, PolicyType::Never);
    recent_scroll.set_child(Some(&recent_box));
    recent_scroll.set_margin_bottom(6);
    content.append(&recent_scroll);

    // Per-type sections (Images / Videos / GIFs); rebuilt by populate_library.
    let sections_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sections_box.set_margin_bottom(12);
    content.append(&sections_box);

    // Empty-state hero (shown when the library is empty): big dim glyph,
    // a drop invitation (the window-level DropTarget honors it), and the two
    // ways in — add files or browse the catalog.
    let welcome = adw::StatusPage::new();
    welcome.set_icon_name(Some("video-display-symbolic"));
    welcome.set_title(t!("Drop videos or images here"));
    welcome.set_description(Some(t!(
        "Drag files onto the window, or add a video, GIF, image, or folder of images"
    )));
    welcome.set_vexpand(true);
    let welcome_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    welcome_actions.set_halign(gtk4::Align::Center);
    let welcome_btn = gtk4::Button::with_label(t!("Add wallpapers"));
    welcome_btn.add_css_class("suggested-action");
    welcome_btn.add_css_class("pill");
    welcome_btn.add_css_class("welcome-cta");
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let win2 = window.clone();
        welcome_btn.connect_clicked(move |_| {
            open_file_picker(&win2, state2.clone(), stack2.clone(), None);
        });
    }
    welcome_actions.append(&welcome_btn);
    let welcome_browse = gtk4::Button::with_label(t!("Browse catalog"));
    welcome_browse.add_css_class("pill");
    welcome_browse.add_css_class("welcome-cta");
    {
        let state2 = state.clone();
        let win2 = window.clone();
        welcome_browse.connect_clicked(move |_| {
            super::gallery::show_gallery_window(&win2, state2.clone());
        });
    }
    welcome_actions.append(&welcome_browse);
    welcome.set_child(Some(&welcome_actions));
    content.append(&welcome);

    // Bound the grid width with a Clamp: it grows with the window but never
    // past `maximum_size`, so ultrawide/4K keeps a centered, readable column
    // instead of stretching edge-to-edge (mirrors the editor's preview_clamp).
    let content_clamp = adw::Clamp::new();
    content_clamp.set_maximum_size(1360);
    content_clamp.set_tightening_threshold(900);
    content_clamp.set_child(Some(&content));

    scroll.set_child(Some(&content_clamp));
    root.append(&scroll);

    // ── Footer: anchored action bar (count left, add actions right) ──
    // Horizontal inset comes from the .footer-bar CSS padding (tighter in
    // compact mode), so the hairline top border spans the full window width.
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    footer.add_css_class("footer-bar");

    let count_label = gtk4::Label::new(None);
    count_label.add_css_class("footer-count");
    count_label.set_xalign(0.0);
    count_label.set_valign(gtk4::Align::Center);
    footer.append(&count_label);

    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    footer.append(&spacer);

    let add_folder_btn = gtk4::Button::new();
    add_folder_btn.set_child(Some(&button_content(
        "folder-new-symbolic",
        t!("Add folder"),
    )));
    add_folder_btn.set_tooltip_text(Some(t!(
        "Add a folder as a timed slideshow, or as one wallpaper per file"
    )));
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let win2 = window.clone();
        add_folder_btn.connect_clicked(move |_| {
            open_folder_picker(&win2, state2.clone(), stack2.clone());
        });
    }
    footer.append(&add_folder_btn);

    // Labelled, brand-marked entry point. This was an unlabelled
    // `insert-link-symbolic` button and telemetry showed the feature at
    // literally zero uses — nobody recognised a generic chain-link glyph in a
    // footer as "paste a Pinterest link". The logo names the thing people
    // already have in their clipboard.
    let add_link_btn = gtk4::Button::new();
    add_link_btn.set_child(Some(&pinterest_button_content()));
    add_link_btn.set_tooltip_text(Some(t!(
        "Paste a Pinterest or direct media link to set as wallpaper"
    )));
    {
        let state2 = state.clone();
        let win2 = window.clone();
        let stack2 = stack.clone();
        add_link_btn.connect_clicked(move |_| {
            super::add_link::show_add_link_dialog(&win2, state2.clone(), stack2.clone());
        });
    }
    footer.append(&add_link_btn);

    let add_btn = gtk4::Button::new();
    add_btn.set_child(Some(&button_content("list-add-symbolic", t!("Add"))));
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

    // Select-mode toggle: sits with the add actions but reads as a mode switch.
    let select_btn = gtk4::Button::new();
    select_btn.set_child(Some(&button_content(
        "object-select-symbolic",
        t!("Select"),
    )));
    select_btn.set_tooltip_text(Some(t!("Select several wallpapers to remove at once")));
    {
        let state2 = state.clone();
        select_btn.connect_clicked(move |_| enter_selection(&state2, None));
    }
    footer.append(&select_btn);
    root.append(&footer);

    // ── Selection bar (replaces the footer while in select mode) ──
    let sel_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    sel_bar.add_css_class("footer-bar");
    sel_bar.set_visible(false);

    let sel_count = gtk4::Label::new(None);
    sel_count.add_css_class("footer-count");
    sel_count.set_xalign(0.0);
    sel_count.set_valign(gtk4::Align::Center);
    sel_bar.append(&sel_count);

    let sel_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    sel_spacer.set_hexpand(true);
    sel_bar.append(&sel_spacer);

    let sel_all_btn = gtk4::Button::with_label(t!("Select all"));
    sel_bar.append(&sel_all_btn);

    // Bulk filing: the fastest way to build a folder out of a library that
    // grew before folders existed.
    let sel_move = gtk4::Button::new();
    sel_move.set_child(Some(&button_content(
        "folder-symbolic",
        t!("Move to folder…"),
    )));
    {
        let state2 = state.clone();
        let win2 = window.clone();
        sel_move.connect_clicked(move |_| {
            let ids: Vec<String> = match state2.borrow().selection.clone() {
                Some(ids) if !ids.is_empty() => ids.into_iter().collect(),
                _ => return,
            };
            show_move_to_collection_dialog(&win2, state2.clone(), ids);
        });
    }
    sel_bar.append(&sel_move);

    let sel_cancel = gtk4::Button::with_label(t!("Cancel"));
    {
        let state2 = state.clone();
        sel_cancel.connect_clicked(move |_| exit_selection(&state2));
    }
    sel_bar.append(&sel_cancel);

    let sel_remove = gtk4::Button::new();
    sel_remove.add_css_class("destructive-action");
    sel_bar.append(&sel_remove);
    root.append(&sel_bar);

    // Swap footer ↔ selection bar and keep the count/labels current. Stored on
    // the state so the selection helpers can call it from anywhere.
    {
        let state2 = state.clone();
        let footer = footer.clone();
        let sel_bar = sel_bar.clone();
        let sel_count = sel_count.clone();
        let sel_remove = sel_remove.clone();
        let sel_move = sel_move.clone();
        let sync: Rc<dyn Fn()> = Rc::new(move || {
            let n = {
                let s = state2.borrow();
                s.selection.as_ref().map(|set| set.len())
            };
            match n {
                Some(n) => {
                    footer.set_visible(false);
                    sel_bar.set_visible(true);
                    sel_count.set_text(&match n {
                        0 => t!("Select wallpapers to remove").to_string(),
                        1 => t!("1 selected").to_string(),
                        n => tf!("{count} selected", "count" => n.to_string()),
                    });
                    sel_remove.set_sensitive(n > 0);
                    sel_move.set_sensitive(n > 0);
                    sel_remove.set_child(Some(&button_content(
                        "user-trash-symbolic",
                        &if n > 1 {
                            tf!("Remove {count}", "count" => n.to_string())
                        } else {
                            t!("Remove").to_string()
                        },
                    )));
                }
                None => {
                    sel_bar.set_visible(false);
                    footer.set_visible(true);
                }
            }
        });
        sync();
        state.borrow_mut().sync_selection_ui = Some(sync);
    }

    // In compact mode, condense the footer buttons to icon-only (tooltips
    // already carry the label) so they don't crowd out the search/grid at the
    // 420px minimum width.
    let condense_footer_buttons = {
        let add_folder_btn = add_folder_btn.clone();
        let add_btn = add_btn.clone();
        move |compact: bool| {
            if compact {
                add_folder_btn.set_icon_name("folder-new-symbolic");
                add_btn.set_icon_name("list-add-symbolic");
            } else {
                add_folder_btn.set_child(Some(&button_content(
                    "folder-new-symbolic",
                    t!("Add folder"),
                )));
                add_btn.set_child(Some(&button_content("list-add-symbolic", t!("Add"))));
            }
        }
    };
    add_btn.set_tooltip_text(Some(t!("Add a wallpaper")));

    // ── Live-updating sectioned library ──
    let home_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // Selection actions need the currently-listed ids, which depend on the
    // search query — so they're wired once `home_query` exists.
    {
        let visible_ids = {
            let state = state.clone();
            let home_query = home_query.clone();
            move || -> Vec<String> {
                let q = home_query.borrow().clone();
                // Scope as well as query: ticking cards from a folder the user
                // is not looking at would aim the next Remove at wallpapers
                // that are not on screen.
                let s = state.borrow();
                scoped_ids(&s.entries, &s.view, &q)
            }
        };
        {
            let state2 = state.clone();
            let visible_ids = visible_ids.clone();
            sel_all_btn.connect_clicked(move |_| select_all_visible(&state2, &visible_ids()));
        }
        let state2 = state.clone();
        let win2 = window.clone();
        sel_remove.connect_clicked(move |_| {
            let ids = match state2.borrow().selection.clone() {
                Some(ids) if !ids.is_empty() => ids,
                _ => return,
            };
            confirm_remove_selected(&win2, state2.clone(), ids);
        });
    }
    let refresh: Rc<dyn Fn()> = {
        let state = state.clone();
        let sections_box = sections_box.clone();
        let recent_box = recent_box.clone();
        let recent_label = recent_label.clone();
        let welcome = welcome.clone();
        let stack = stack.clone();
        let home_query = home_query.clone();
        let search = search.clone();
        let bucket = bucket.clone();
        let count_label = count_label.clone();
        let toolbar = toolbar.clone();
        let sync_toolbar = sync_toolbar.clone();
        Rc::new(move || {
            // Searching an empty library is pointless: hide the field until
            // there's something to search.
            let n = state.borrow().entries.len();
            search.set_visible(n > 0);
            toolbar.set_visible(n > 0);
            sync_toolbar();
            count_label.set_visible(n > 0);
            count_label.set_text(&if n == 1 {
                t!("1 wallpaper").to_string()
            } else {
                tf!("{count} wallpapers", "count" => n.to_string())
            });
            let q = home_query.borrow();
            populate_library(
                &state,
                &sections_box,
                &recent_box,
                &recent_label,
                &welcome,
                &stack,
                q.as_str(),
                bucket.get(),
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

    // Apply the bucket resolved at construction time (handles launching
    // straight into a narrow tiling-WM tile), then keep it in sync with
    // interactive resizes. `AdwBreakpoint` needs libadwaita >= 1.4; this build
    // is on 1.1, so `default-width` is the portable fallback.
    let margin_widgets = LayoutMarginWidgets {
        root: root.clone(),
        search: search.clone(),
        toolbar: toolbar.clone(),
        content: content.clone(),
    };
    apply_layout_bucket(&margin_widgets, &condense_footer_buttons, bucket.get());
    {
        let bucket = bucket.clone();
        let refresh = refresh.clone();
        window.connect_notify_local(Some("default-width"), move |win, _| {
            let resolved = LayoutBucket::from_width(win.default_width());
            if resolved == bucket.get() {
                return;
            }
            bucket.set(resolved);
            apply_layout_bucket(&margin_widgets, &condense_footer_buttons, resolved);
            refresh();
        });
    }

    root
}

/// Widgets whose margins tighten in compact mode.
struct LayoutMarginWidgets {
    root: gtk4::Box,
    search: gtk4::SearchEntry,
    /// The sort/folder bar is a direct child of `root`, so it gets no side
    /// inset from the content clamp — it has to be margined here or it sits
    /// flush against the window edge.
    toolbar: gtk4::Box,
    content: gtk4::Box,
}

/// Toggle the compact/wide CSS class, footer button density, and outer
/// margins for `bucket`. Column caps are applied separately, inside
/// `populate_library`, since they only take effect the next time a FlowBox
/// section is (re)built.
fn apply_layout_bucket(
    widgets: &LayoutMarginWidgets,
    condense_footer_buttons: &impl Fn(bool),
    bucket: LayoutBucket,
) {
    for cls in ["compact-layout", "wide-layout"] {
        widgets.root.remove_css_class(cls);
    }
    if let Some(cls) = bucket.css_class() {
        widgets.root.add_css_class(cls);
    }
    condense_footer_buttons(bucket == LayoutBucket::Compact);

    let side_margin = if bucket == LayoutBucket::Compact {
        8
    } else {
        16
    };
    widgets.search.set_margin_start(side_margin);
    widgets.search.set_margin_end(side_margin);
    widgets.toolbar.set_margin_start(side_margin);
    widgets.toolbar.set_margin_end(side_margin);
    widgets.content.set_margin_start(side_margin);
    widgets.content.set_margin_end(side_margin);
    // Footer inset comes from the .footer-bar / .compact-layout CSS padding.
}

/// Header menu: appearance (theme mode + accent) and behavior switches.
fn build_menu_popover(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) -> gtk4::Popover {
    // Built up front so every item that opens a window can pop it down first —
    // see `menu_item_opening_window` for why that is not optional on X11.
    let popover = gtk4::Popover::new();
    popover.add_css_class("fresco-menu");

    let popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    popover_box.set_margin_top(6);
    popover_box.set_margin_bottom(6);
    popover_box.set_margin_start(6);
    popover_box.set_margin_end(6);
    // Compact menu column: wide enough for the longest switch row, no wider.
    popover_box.set_width_request(300);

    // ── Appearance ──
    popover_box.append(&overline(t!("Appearance")));

    let seg = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    seg.add_css_class("linked");
    seg.add_css_class("seg");
    seg.set_homogeneous(true);
    let b_sys = gtk4::ToggleButton::with_label(t!("System"));
    let b_light = gtk4::ToggleButton::with_label(t!("Light"));
    let b_dark = gtk4::ToggleButton::with_label(t!("Dark"));
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
            // Derive dark/light from the chosen mode, not is_dark() — the latter
            // is stale right after set_mode, so the palette wouldn't switch.
            theme::apply(accent, theme::resolve_dark(mode));
        });
    }
    seg.append(&b_sys);
    seg.append(&b_light);
    seg.append(&b_dark);
    popover_box.append(&seg);

    popover_box.append(&overline(t!("Accent")));

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
                let mode = {
                    let mut s = state2.borrow_mut();
                    s.config.accent = acc;
                    s.config.save().ok();
                    s.config.theme_mode
                };
                theme::apply(acc, theme::resolve_dark(mode));
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

    popover_box.append(&overline(t!("Language")));
    popover_box.append(&build_language_row(state.clone()));

    // Separator margins come from the .fresco-menu CSS.
    popover_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    // ── Behavior ──
    popover_box.append(&overline(t!("Behavior")));
    popover_box.append(&switch_row(
        t!("Restore on login"),
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
        t!("Pause on battery"),
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
    // Quick schedule pause — only shown when a schedule exists. Turning it off
    // here keeps the configured day/night setup (unlike Advanced's "Off",
    // which deletes it); users kept hunting for this switch.
    if state.borrow().config.schedule.is_some() {
        popover_box.append(&switch_row(
            t!("Day/night schedule"),
            !state.borrow().config.schedule_paused,
            {
                let state2 = state.clone();
                move |active| {
                    {
                        let mut s = state2.borrow_mut();
                        s.config.schedule_paused = !active;
                        s.config.save().ok();
                    }
                    let s = state2.borrow();
                    daemon_ctl::ensure_daemon_and_apply(&s.config).ok();
                }
            },
        ));
    }
    // Off here means the country-only tier, not silence — the same trade the
    // consent dialog spells out. config.toml is the way to opt out entirely
    // (see telemetry::opt_out_completely), and TERMS.md documents it.
    popover_box.append(&switch_row(
        t!("Share anonymous usage statistics"),
        state.borrow().config.telemetry,
        {
            let state2 = state.clone();
            move |active| {
                let mut s = state2.borrow_mut();
                s.config.telemetry = active;
                s.config.save().ok();
            }
        },
    ));
    let bridge_row = switch_row(
        t!("Browser new-tab wallpaper (local)"),
        state.borrow().config.browser_bridge,
        {
            let state2 = state.clone();
            move |active| {
                {
                    let mut s = state2.borrow_mut();
                    s.config.browser_bridge = active;
                    s.config.save().ok();
                }
                // The daemon binds the bridge port at startup only, so make
                // sure it's running with the new setting (enable needs a live
                // daemon; disable is honored per-request either way).
                let s = state2.borrow();
                daemon_ctl::ensure_daemon_and_apply(&s.config).ok();
            }
        },
    );
    bridge_row.set_tooltip_text(Some(
        t!("Lets the Fresco browser extension show your wallpaper on new tabs. Local-only (127.0.0.1)."),
    ));
    popover_box.append(&bridge_row);

    popover_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let advanced_btn = menu_item_opening_window(t!("Advanced…"), &popover, {
        let state_adv = state.clone();
        let win_adv = window.clone();
        move || show_advanced_dialog(&win_adv, state_adv.clone())
    });
    popover_box.append(&advanced_btn);

    let browse_btn = menu_item_opening_window(t!("Browse wallpapers…"), &popover, {
        let state_b = state.clone();
        let win_b = window.clone();
        move || super::gallery::show_gallery_window(&win_b, state_b.clone())
    });
    popover_box.append(&browse_btn);

    let url_btn = menu_item_opening_window(t!("Add from URL…"), &popover, {
        let state_url = state.clone();
        let win_url = window.clone();
        move || show_add_from_url_dialog(&win_url, state_url.clone())
    });
    popover_box.append(&url_btn);

    let folders_btn = menu_item_opening_window(t!("Manage folders…"), &popover, {
        let state_f = state.clone();
        let win_f = window.clone();
        move || show_collections_dialog(&win_f, state_f.clone())
    });
    popover_box.append(&folders_btn);

    // Closes the menu like the rest: an update *is* available often enough that
    // the result dialog would otherwise open underneath it, and the "you're up
    // to date" toast is raised on the main window, which the menu overlaps.
    let update_btn = menu_item_opening_window(t!("Check for updates"), &popover, {
        let state_upd = state.clone();
        let win_upd = window.clone();
        move || super::updates::check_for_updates(&win_upd, state_upd.clone(), true)
    });
    popover_box.append(&update_btn);

    // ── Help & feedback ──
    // A user-initiated path: the feedback dialog otherwise auto-prompts only once
    // (after a week), so without this a user can neither send feedback nor reach
    // support. "Send feedback" is the anonymous one-way rating (→ dashboard);
    // "Message the maintainer" is the anonymous two-way thread; "Report a
    // problem" opens the issue tracker, which is public and needs an account.
    popover_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    popover_box.append(&overline(t!("Help & feedback")));

    let tour_btn = menu_item_opening_window(t!("What can Fresco do?"), &popover, {
        let state_t = state.clone();
        let win_t = window.clone();
        move || show_tour_dialog(&win_t, state_t.clone())
    });
    popover_box.append(&tour_btn);

    let feedback_btn = menu_item_opening_window(t!("Send feedback…"), &popover, {
        let state_fb = state.clone();
        let win_fb = window.clone();
        move || show_feedback_dialog(&win_fb, state_fb.clone())
    });
    popover_box.append(&feedback_btn);

    // Sits above the issue tracker on purpose: it is the lower-effort route
    // for someone who just wants help, and it needs no GitHub account.
    let support_btn = menu_item_opening_window(
        if crate::support::has_thread() {
            t!("Your conversation…")
        } else {
            t!("Message the maintainer…")
        },
        &popover,
        {
            let state_s = state.clone();
            let win_s = window.clone();
            move || show_support_dialog(&win_s, state_s.clone())
        },
    );
    support_btn.set_tooltip_text(Some(t!(
        "Anonymous, both ways. No account, no email address."
    )));
    popover_box.append(&support_btn);

    // The browser is someone else's toplevel, but it is a toplevel all the
    // same — it would come up behind a menu we left mapped.
    let help_btn = menu_item_opening_window(t!("Report a problem…"), &popover, || {
        let _ = std::process::Command::new("xdg-open")
            .arg(ISSUES_URL)
            .spawn();
    });
    help_btn.set_tooltip_text(Some(t!("Opens the Fresco issue tracker in your browser")));
    popover_box.append(&help_btn);

    let about_btn = menu_item_opening_window(t!("About"), &popover, {
        let win_about = window.clone();
        move || show_about_dialog(&win_about)
    });
    popover_box.append(&about_btn);

    // The menu is ~25 rows tall and grows every time a setting is added, but a
    // popover taller than the space under its button does not clip — on
    // Wayland it fails to map at all, and the button appears simply dead. That
    // is display-dependent, so it survives testing: the same menu that fits a
    // 1080p external monitor at scale 1 does not fit a fractionally scaled
    // laptop panel, whose *logical* height is several hundred units shorter.
    //
    // Scrolling the column instead of letting it size itself makes the failure
    // impossible: `propagate_natural_height` keeps the popover exactly as tall
    // as its content while that fits, and `max_content_height` caps it below
    // any plausible logical screen height, after which it scrolls.
    let scroller = gtk4::ScrolledWindow::new();
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_propagate_natural_height(true);
    scroller.set_propagate_natural_width(true);
    scroller.set_max_content_height(560);
    scroller.set_child(Some(&popover_box));

    popover.set_child(Some(&scroller));
    popover
}

// ─── Library scope, sorting and sectioning ────────────────────────────────────

/// The [`library::LibraryView::collection`] value that means "only the
/// wallpapers that are in no folder".
///
/// The persisted field is an `Option<String>` whose `None` is already spoken
/// for — it means *the whole library* — but the uncategorized pool is a third
/// scope the user has to be able to open: it is where entries land when a
/// folder is deleted, and it is the only group a folderless library has. A
/// reserved id costs nothing and cannot collide: real ids are always
/// `"{pid}-{counter}"`, so no folder can ever be called this.
const SCOPE_UNCATEGORIZED: &str = "~uncategorized";

/// Drop a saved scope that points at a folder which no longer exists.
///
/// Without this, deleting a folder in a second window (or hand-editing
/// `view.json`) leaves the library showing an empty grid filtered to an id
/// nothing matches — and the folder button would name a folder that is not in
/// its own list, so there would be no obvious way back.
fn normalize_view(
    mut view: library::LibraryView,
    collections: &[library::Collection],
) -> library::LibraryView {
    if let Some(id) = view.collection.as_deref() {
        if id != SCOPE_UNCATEGORIZED && !collections.iter().any(|c| c.id == id) {
            view.collection = None;
        }
    }
    view
}

/// Is this entry inside the folder the library is currently scoped to?
fn entry_in_scope(e: &LibraryEntry, view: &library::LibraryView) -> bool {
    match view.collection.as_deref() {
        None => true,
        Some(SCOPE_UNCATEGORIZED) => e.collection.is_none(),
        Some(id) => e.collection.as_deref() == Some(id),
    }
}

/// The open scope's name, for the folder button and the flat grid's heading.
fn scope_label(view: &library::LibraryView, collections: &[library::Collection]) -> String {
    match view.collection.as_deref() {
        None => t!("All wallpapers").to_string(),
        Some(SCOPE_UNCATEGORIZED) => t!("Uncategorized").to_string(),
        Some(id) => library::collection_name(collections, id)
            .unwrap_or(t!("All wallpapers"))
            .to_string(),
    }
}

/// One grid section: a heading, the entry ids under it in display order, and
/// whether "move earlier / move later" is meaningful there.
///
/// This is deliberately a **pure** description of the layout rather than a pile
/// of widgets: it is the one part of the folder/sort feature that can be unit
/// tested, since GTK widgets cannot be constructed without a display in this
/// crate's test harness. `populate_library` does nothing but render it.
#[derive(Debug, PartialEq)]
struct SectionPlan {
    title: String,
    ids: Vec<String>,
    /// True only when the section is *exactly one ordering group* shown in
    /// manual order. [`library::move_entry_up`] swaps an entry with its
    /// neighbour **within its own collection**, so offering the arrows on a
    /// section that mixes groups (Favorites) — or one that slices a single
    /// group by kind (the Images/Videos/GIFs layout) — would move the card
    /// past a neighbour that is not on screen and look like a no-op.
    reorderable: bool,
}

/// Decide how the grid is laid out: which sections exist, in what order, and
/// which entries are in each.
///
/// Four layouts, in priority order:
///
/// 1. **Favorites** ride on top of all of them, as they did before folders —
///    starring something is a stronger statement than filing it.
/// 2. A **chosen folder** (or a **derived sort**) collapses to one flat grid.
///    Sorting by "largest file" inside three separate sections tells you the
///    largest *image* and the largest *video* but never the largest wallpaper,
///    which is the question the sort was asked; and a folder the user opened
///    on purpose is already the grouping they wanted.
/// 3. Manual order with **folders defined** → one section per folder, in the
///    user's folder order, then everything still loose under "Uncategorized".
/// 4. Manual order with **no folders** → the pre-1.2 Images / Videos / GIFs
///    grouping, unchanged. Nobody who never makes a folder sees this feature.
fn plan_sections(
    entries: &[LibraryEntry],
    collections: &[library::Collection],
    view: &library::LibraryView,
    query: &str,
) -> Vec<SectionPlan> {
    let q = query.to_lowercase();
    let keep = |e: &LibraryEntry| entry_in_scope(e, view) && entry_matches_query(e, &q);
    let sorted = |mut v: Vec<&LibraryEntry>| -> Vec<String> {
        library::sort_entries(&mut v, view.sort);
        v.into_iter().map(|e| e.id.clone()).collect()
    };

    let mut out: Vec<SectionPlan> = Vec::new();

    let favs: Vec<&LibraryEntry> = entries.iter().filter(|e| e.favorite && keep(e)).collect();
    if !favs.is_empty() {
        out.push(SectionPlan {
            title: t!("Favorites").to_string(),
            ids: sorted(favs),
            reorderable: false,
        });
    }

    if view.collection.is_some() || !view.sort.is_manual() {
        let all: Vec<&LibraryEntry> = entries.iter().filter(|e| keep(e)).collect();
        if !all.is_empty() {
            out.push(SectionPlan {
                title: scope_label(view, collections),
                // A single scope IS a single group, so the arrows are honest
                // here — but only while nothing is re-deriving the order.
                reorderable: view.sort.is_manual() && view.collection.is_some(),
                ids: sorted(all),
            });
        }
        return out;
    }

    if collections.is_empty() {
        for cat in CATEGORY_ORDER {
            let v: Vec<&LibraryEntry> = entries
                .iter()
                .filter(|e| entry_category(e) == cat && keep(e))
                .collect();
            if v.is_empty() {
                continue;
            }
            out.push(SectionPlan {
                title: category_label(cat).to_string(),
                ids: sorted(v),
                reorderable: false,
            });
        }
        return out;
    }

    let mut ordered: Vec<&library::Collection> = collections.iter().collect();
    ordered.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));
    for c in ordered {
        let v: Vec<&LibraryEntry> = library::entries_in(entries, &c.id)
            .into_iter()
            .filter(|e| keep(e))
            .collect();
        if v.is_empty() {
            continue;
        }
        out.push(SectionPlan {
            title: c.name.clone(),
            ids: sorted(v),
            reorderable: true,
        });
    }
    let loose: Vec<&LibraryEntry> = library::uncategorized(entries)
        .into_iter()
        .filter(|e| keep(e))
        .collect();
    if !loose.is_empty() {
        out.push(SectionPlan {
            title: t!("Uncategorized").to_string(),
            ids: sorted(loose),
            reorderable: true,
        });
    }
    out
}

/// The ids the grid is currently showing, honouring **both** the search box and
/// the open folder.
///
/// "Select all" is built on this. Filtering by the query alone would tick cards
/// that are not on screen, and the very next click would delete wallpapers out
/// of a folder the user was not even looking at.
fn scoped_ids(entries: &[LibraryEntry], view: &library::LibraryView, query: &str) -> Vec<String> {
    let q = query.to_lowercase();
    entries
        .iter()
        .filter(|e| entry_in_scope(e, view) && entry_matches_query(e, &q))
        .map(|e| e.id.clone())
        .collect()
}

/// A vertical list of `.menu-item` rows with a checkmark on the active one —
/// the same shape as [`build_language_row`], and for the same reason: this is
/// the popover of a top-level `GtkMenuButton`, so a `GtkDropDown` inside it
/// would open a *second* popup surface and take a second seat grab (issue #5).
/// One level of popover is safe; two is what breaks input on X11/DDE.
fn choice_menu<T: Clone + PartialEq + 'static>(
    options: Vec<(T, String)>,
    current: T,
    pop: &gtk4::Popover,
    on_pick: impl Fn(T) + 'static,
) -> gtk4::Box {
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let on_pick = Rc::new(on_pick);
    for (value, label) in options {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let lbl = gtk4::Label::new(Some(&label));
        lbl.set_xalign(0.0);
        lbl.set_halign(gtk4::Align::Start);
        lbl.set_hexpand(true);
        lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let check = gtk4::Image::from_icon_name("emblem-ok-symbolic");
        check.set_visible(value == current);
        row.append(&lbl);
        row.append(&check);

        let btn = gtk4::Button::new();
        btn.add_css_class("flat");
        btn.add_css_class("menu-item");
        btn.set_halign(gtk4::Align::Fill);
        btn.set_child(Some(&row));
        {
            let on_pick = on_pick.clone();
            let pop = pop.clone();
            btn.connect_clicked(move |_| {
                // Down before anything else: the pick rebuilds this very list,
                // and on X11 a popover left mapped over a rebuild is the same
                // stacking trap `menu_item_opening_window` documents.
                pop.popdown();
                on_pick(value.clone());
            });
        }
        vbox.append(&btn);
    }
    vbox
}

/// The sort + folder bar under the search field.
///
/// Returns the row and a closure that re-reads the state into it; the closure
/// is called from `refresh`, so creating, renaming or deleting a folder
/// anywhere in the app updates this bar without a view switch.
fn build_library_toolbar(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) -> (gtk4::Box, Rc<dyn Fn()>) {
    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    bar.set_margin_top(6);
    bar.set_margin_bottom(2);

    let scope_btn = gtk4::MenuButton::new();
    scope_btn.add_css_class("flat");
    scope_btn.set_tooltip_text(Some(t!("Show one folder, or the whole library")));
    bar.append(&scope_btn);

    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bar.append(&spacer);

    let sort_btn = gtk4::MenuButton::new();
    sort_btn.add_css_class("flat");
    sort_btn.set_tooltip_text(Some(t!("Change how the wallpapers are ordered")));
    bar.append(&sort_btn);

    let sync: Rc<dyn Fn()> = {
        let state = state.clone();
        let window = window.clone();
        let scope_btn = scope_btn.clone();
        let sort_btn = sort_btn.clone();
        Rc::new(move || {
            let (collections, view) = {
                let s = state.borrow();
                (s.collections.clone(), s.view.clone())
            };

            // ── Folder scope ──
            scope_btn.set_child(Some(&button_content(
                "folder-symbolic",
                &scope_label(&view, &collections),
            )));
            let scope_pop = gtk4::Popover::new();
            scope_pop.add_css_class("fresco-menu");
            let scope_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            scope_box.set_margin_top(6);
            scope_box.set_margin_bottom(6);
            scope_box.set_margin_start(6);
            scope_box.set_margin_end(6);
            scope_box.set_width_request(240);

            let mut options: Vec<(Option<String>, String)> =
                vec![(None, t!("All wallpapers").to_string())];
            let mut ordered: Vec<&library::Collection> = collections.iter().collect();
            ordered.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));
            for c in &ordered {
                options.push((Some(c.id.clone()), c.name.clone()));
            }
            if !collections.is_empty() {
                options.push((
                    Some(SCOPE_UNCATEGORIZED.to_string()),
                    t!("Uncategorized").to_string(),
                ));
            }
            scope_box.append(&choice_menu(
                options,
                view.collection.clone(),
                &scope_pop,
                {
                    let state = state.clone();
                    move |pick| set_scope(&state, pick)
                },
            ));

            scope_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
            scope_box.append(&menu_item_opening_window(
                t!("Manage folders…"),
                &scope_pop,
                {
                    let state = state.clone();
                    let window = window.clone();
                    move || show_collections_dialog(&window, state.clone())
                },
            ));
            scope_pop.set_child(Some(&scope_box));
            scope_btn.set_popover(Some(&scope_pop));

            // ── Sort ──
            sort_btn.set_child(Some(&button_content(
                "view-sort-descending-symbolic",
                view.sort.label(),
            )));
            let sort_pop = gtk4::Popover::new();
            sort_pop.add_css_class("fresco-menu");
            let sort_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            sort_box.set_margin_top(6);
            sort_box.set_margin_bottom(6);
            sort_box.set_margin_start(6);
            sort_box.set_margin_end(6);
            sort_box.set_width_request(200);
            sort_box.append(&choice_menu(
                sort_table()
                    .into_iter()
                    .map(|(m, l)| (m, l.to_string()))
                    .collect(),
                view.sort,
                &sort_pop,
                {
                    let state = state.clone();
                    move |mode| set_sort(&state, mode)
                },
            ));
            sort_pop.set_child(Some(&sort_box));
            sort_btn.set_popover(Some(&sort_pop));
        })
    };
    sync();
    (bar, sync)
}

/// Every sort mode paired with its translated label, in menu order.
///
/// Built at runtime rather than declared `const` like `LYRIC_STYLES` and
/// friends because the labels come from [`library::SortMode::label`], which
/// already resolves them through `t!` — duplicating the literals here would
/// mean two places to keep in step with the catalog. It still feeds
/// [`table_index`], so the same "every variant is present exactly once"
/// guarantee is testable.
fn sort_table() -> Vec<(library::SortMode, &'static str)> {
    library::SortMode::ALL
        .iter()
        .map(|m| (*m, m.label()))
        .collect()
}

/// Persist the chosen sort and redraw.
fn set_sort(state: &Rc<RefCell<AppState>>, mode: library::SortMode) {
    {
        let mut s = state.borrow_mut();
        if s.view.sort == mode {
            return;
        }
        s.view.sort = mode;
        library::save_view(&s.view).ok();
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// Persist the open folder and redraw. Leaving select mode on the way is
/// deliberate: a selection made in one folder must not survive into another,
/// where "Remove" would be aimed at cards the user can no longer see.
fn set_scope(state: &Rc<RefCell<AppState>>, collection: Option<String>) {
    {
        let mut s = state.borrow_mut();
        if s.view.collection == collection {
            return;
        }
        s.view.collection = collection;
        library::save_view(&s.view).ok();
        s.selection = None;
    }
    refresh_selection(state);
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
    bucket: LayoutBucket,
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

    // Recents (hidden while searching, to focus on the matches — and while
    // selecting, since mini cards apply a wallpaper on click and can't be ticked).
    {
        let recents = if searching || state.borrow().selection.is_some() {
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

    // Favorites, folders, kinds or one flat grid — `plan_sections` decides;
    // everything below is pure rendering of that decision.
    let (collections, view) = {
        let s = state.borrow();
        (s.collections.clone(), s.view.clone())
    };
    let plans = plan_sections(&entries, &collections, &view, query);

    // Cards still address entries by index (apply/edit/remove all take one), so
    // resolve the planned ids back to positions once instead of per card.
    let index_of: std::collections::HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();

    let mut first_section = true;
    for plan in &plans {
        let matches: Vec<(usize, &LibraryEntry)> = plan
            .ids
            .iter()
            .filter_map(|id| index_of.get(id.as_str()).copied())
            .map(|i| (i, &entries[i]))
            .collect();
        if matches.is_empty() {
            continue;
        }
        sections_box.append(&build_section(
            &plan.title,
            first_section,
            &matches,
            &cfg,
            state,
            stack,
            bucket,
            plan.reorderable,
        ));
        first_section = false;
    }

    // Scoped to a folder that the search emptied (or an empty folder): say so,
    // rather than leaving a blank page that looks like the library was lost.
    if first_section {
        let empty = gtk4::Label::new(Some(if q.is_empty() {
            t!("This folder is empty — move wallpapers into it from any card’s menu")
        } else {
            t!("No wallpapers match your search here")
        }));
        empty.add_css_class("dialog-sub");
        empty.set_wrap(true);
        empty.set_margin_top(24);
        empty.set_margin_bottom(24);
        sections_box.append(&empty);
    }
}

/// One labelled FlowBox grid of library cards — one [`SectionPlan`], rendered.
///
/// `reorderable` comes straight from the plan and is passed through to every
/// card: it is what decides whether the hover cluster grows a pair of
/// move-earlier / move-later arrows.
#[allow(clippy::too_many_arguments)]
fn build_section(
    label: &str,
    first_section: bool,
    matches: &[(usize, &LibraryEntry)],
    cfg: &Config,
    state: &Rc<RefCell<AppState>>,
    stack: &gtk4::Stack,
    bucket: LayoutBucket,
    reorderable: bool,
) -> gtk4::Box {
    let section = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let header = overline(label);
    // More air above a section than below its header (overline rhythm).
    header.set_margin_top(if first_section { 8 } else { 14 });
    header.set_margin_bottom(6);
    section.append(&header);

    let (min_children, max_children) = bucket.flow_caps();
    let flow = gtk4::FlowBox::new();
    flow.set_homogeneous(true);
    flow.set_max_children_per_line(max_children);
    flow.set_min_children_per_line(min_children);
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_valign(gtk4::Align::Start);
    flow.set_row_spacing(12);
    flow.set_column_spacing(12);
    flow.set_margin_bottom(6);
    for (idx, entry) in matches {
        let active = entry_is_active(entry, cfg);
        let card = build_library_card(
            entry,
            *idx,
            state.clone(),
            stack.clone(),
            active,
            reorderable,
        );
        flow.append(&card);
    }
    section.append(&flow);
    section
}

/// Compact recent-row card: thumbnail + title scrim, click to apply.
fn build_mini_card(
    entry: &LibraryEntry,
    idx: usize,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    active: bool,
) -> gtk4::AspectFrame {
    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("wp-mini");
    if active {
        overlay.add_css_class("active");
    }
    overlay.set_overflow(gtk4::Overflow::Hidden);

    if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
        let pic = gtk4::Picture::new();
        pic.add_css_class("wp-thumb");
        pic.set_can_shrink(true);
        pic.set_keep_aspect_ratio(true);
        pic.set_file(Some(&gio::File::for_path(thumb)));
        overlay.set_child(Some(&pic));
    } else {
        // No thumbnail (yet): show a mat + kind glyph instead of letting the
        // Picture report a 0×0 natural size and collapse the card to a sliver.
        overlay.set_child(Some(&thumb_placeholder(entry.kind)));
    }

    let scrim = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    scrim.add_css_class("wp-scrim");
    scrim.set_valign(gtk4::Align::End);
    let title = gtk4::Label::new(Some(&display_name(&entry.name, entry.kind)));
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

    if entry.name != display_name(&entry.name, entry.kind) {
        overlay.set_tooltip_text(Some(&entry.name));
    }

    // Fixed 16:9 minimum footprint (both axes): a thumb-less card must never
    // collapse to its 0-height natural size in the horizontal recent row.
    let frame = gtk4::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
    frame.set_size_request(150, 84);
    frame.set_child(Some(&overlay));
    frame
}

/// Thumbnail stand-in: `thumb_mat` background + a dim kind glyph, centered.
/// Used by the mini and library cards whenever no thumbnail file exists yet.
fn thumb_placeholder(kind: Kind) -> gtk4::Box {
    let ph = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    ph.add_css_class("wp-placeholder");
    ph.set_hexpand(true);
    ph.set_vexpand(true);
    let icon = gtk4::Image::from_icon_name(kind_icon(kind));
    icon.set_pixel_size(20);
    icon.set_halign(gtk4::Align::Center);
    icon.set_valign(gtk4::Align::Center);
    icon.set_hexpand(true);
    icon.set_vexpand(true);
    ph.append(&icon);
    ph
}

/// Cinematic 16:9 library card: poster thumbnail, gradient title scrim, kind
/// badge, active-wallpaper accent ring + pill, and a hover-revealed Edit button.
fn build_library_card(
    entry: &LibraryEntry,
    idx: usize,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    active: bool,
    reorderable: bool,
) -> gtk4::AspectFrame {
    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("wp-card");
    if active {
        overlay.add_css_class("active");
    }
    overlay.set_overflow(gtk4::Overflow::Hidden);
    overlay.set_valign(gtk4::Align::Start);

    let pic = gtk4::Picture::new();
    pic.set_can_shrink(true);
    pic.set_keep_aspect_ratio(true);
    if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
        pic.add_css_class("wp-thumb");
        // Fade the thumbnail in on first map: start invisible, drop the class
        // on idle so the .wp-thumb opacity transition plays (no transform/blur
        // — this is the GTK-honest load animation).
        pic.add_css_class("thumb-loading");
        {
            let pic2 = pic.clone();
            glib::idle_add_local_once(move || pic2.remove_css_class("thumb-loading"));
        }
        pic.set_file(Some(&gio::File::for_path(thumb)));
        overlay.set_child(Some(&pic));
    } else {
        // No thumbnail (yet): mat + kind glyph as the base layer, with the
        // (transparent, empty) Picture stacked above it so the hover preview
        // can still render into it. Keeps the card from collapsing to 0-size.
        overlay.set_child(Some(&thumb_placeholder(entry.kind)));
        pic.set_hexpand(true);
        pic.set_vexpand(true);
        overlay.add_overlay(&pic);
    }

    // Bottom gradient scrim + title. Decoration only — never a pointer target, so
    // crossing it can't emit spurious hover leave/enter (preview-flicker) and
    // clicks on it still reach the card.
    let scrim = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    scrim.add_css_class("wp-scrim");
    scrim.set_valign(gtk4::Align::End);
    scrim.set_hexpand(true);
    scrim.set_can_target(false);
    let pretty = display_name(&entry.name, entry.kind);
    if pretty != entry.name {
        overlay.set_tooltip_text(Some(&entry.name));
    }
    let title = gtk4::Label::new(Some(&pretty));
    title.add_css_class("wp-title");
    title.set_xalign(0.0);
    title.set_halign(gtk4::Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    scrim.append(&title);
    // Second scrim line: favorite heart + probed metadata ("4K · 60fps · 32 MB").
    {
        let meta_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 5);
        if entry.favorite {
            let heart = gtk4::Label::new(Some(t!("♥")));
            heart.add_css_class("wp-fav-glyph");
            meta_row.append(&heart);
        }
        if let Some(line) = entry.meta_line() {
            let meta = gtk4::Label::new(Some(&line));
            meta.add_css_class("wp-meta");
            meta.set_xalign(0.0);
            meta.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            meta_row.append(&meta);
        }
        if meta_row.first_child().is_some() {
            scrim.append(&meta_row);
        }
    }
    overlay.add_overlay(&scrim);

    // Kind badge (+ optional 4K quality badge) top-left.
    let badge_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    badge_row.add_css_class("wp-badge-row");
    badge_row.set_halign(gtk4::Align::Start);
    badge_row.set_valign(gtk4::Align::Start);
    badge_row.set_can_target(false);
    let badge = gtk4::Label::new(Some(kind_badge(entry.kind)));
    badge.add_css_class("wp-badge");
    badge_row.append(&badge);
    if entry.is_4k() {
        let q = gtk4::Label::new(Some(t!("4K")));
        q.add_css_class("wp-badge");
        q.add_css_class("quality");
        badge_row.append(&q);
    }
    overlay.add_overlay(&badge_row);

    // Active pill (top-right).
    if active {
        let pill = gtk4::Label::new(Some(t!("ACTIVE")));
        pill.add_css_class("wp-active-pill");
        pill.set_halign(gtk4::Align::End);
        pill.set_valign(gtk4::Align::Start);
        overlay.add_overlay(&pill);
    }

    // Missing-source warning.
    if entry.broken {
        let warn = gtk4::Label::new(Some(t!("MISSING")));
        warn.add_css_class("wp-badge");
        warn.add_css_class("warning");
        warn.set_halign(gtk4::Align::Start);
        warn.set_valign(gtk4::Align::End);
        warn.set_tooltip_text(entry.error.as_deref().or(Some(t!("Source file not found"))));
        overlay.add_overlay(&warn);
        overlay.set_opacity(0.65);
    }

    // ── Select mode: a tick badge replaces every other card interaction ──
    let selecting = {
        let s = state.borrow();
        s.selection.as_ref().map(|set| set.contains(&entry.id))
    };
    if let Some(ticked) = selecting {
        let check = gtk4::Label::new(Some(if ticked { t!("✓") } else { "" }));
        check.add_css_class("wp-check");
        if ticked {
            check.add_css_class("on");
            overlay.add_css_class("picked");
        }
        check.set_halign(gtk4::Align::End);
        check.set_valign(gtk4::Align::Start);
        check.set_can_target(false);
        overlay.add_overlay(&check);

        let click = GestureClick::new();
        {
            let state_c = state.clone();
            let id = entry.id.clone();
            click.connect_released(move |_, _, _, _| toggle_selected(&state_c, &id));
        }
        overlay.add_controller(click);

        let frame = gtk4::AspectFrame::new(0.5, 0.0, 16.0 / 9.0, false);
        frame.set_size_request(260, 146);
        frame.set_child(Some(&overlay));
        return frame;
    }

    // Hover-revealed action cluster (bottom-right): heart · edit · menu.
    let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    actions.add_css_class("wp-actions");
    actions.set_halign(gtk4::Align::End);
    actions.set_valign(gtk4::Align::End);
    actions.set_visible(false);

    // Manual-order arrows. Only where the section is one ordering group shown
    // in manual order (see `SectionPlan::reorderable`) — anywhere else the move
    // would be written to `order` and instantly overruled by the sort.
    if reorderable {
        for (icon, tip, delta) in [
            ("go-previous-symbolic", t!("Move earlier"), -1i32),
            ("go-next-symbolic", t!("Move later"), 1i32),
        ] {
            let b = gtk4::Button::from_icon_name(icon);
            b.add_css_class("wp-edit");
            b.add_css_class("circular");
            b.set_tooltip_text(Some(tip));
            let state_r = state.clone();
            let id = entry.id.clone();
            b.connect_clicked(move |_| move_entry(&state_r, &id, delta));
            actions.append(&b);
        }
    }

    let fav = gtk4::Button::from_icon_name("emblem-favorite-symbolic");
    fav.add_css_class("wp-edit");
    fav.add_css_class("circular");
    if entry.favorite {
        fav.add_css_class("fav-on");
        fav.set_tooltip_text(Some(t!("Unfavorite")));
    } else {
        fav.set_tooltip_text(Some(t!("Favorite")));
    }
    {
        let state_f = state.clone();
        fav.connect_clicked(move |_| toggle_favorite(&state_f, idx));
    }
    actions.append(&fav);

    let edit = gtk4::Button::from_icon_name("document-edit-symbolic");
    edit.add_css_class("wp-edit");
    edit.add_css_class("circular");
    edit.set_tooltip_text(Some(t!("Edit & crop")));
    {
        let state_e = state.clone();
        let stack_e = stack.clone();
        edit.connect_clicked(move |_| {
            state_e.borrow_mut().editing_idx = Some(idx);
            stack_e.set_visible_child_name("editor");
        });
    }
    actions.append(&edit);

    let more = gtk4::Button::from_icon_name("view-more-symbolic");
    more.add_css_class("wp-edit");
    more.add_css_class("circular");
    more.set_tooltip_text(Some(t!("More actions")));
    {
        let state_m = state.clone();
        let stack_m = stack.clone();
        let overlay_m = overlay.clone();
        more.connect_clicked(move |btn| {
            // Anchor the popover at the button (translate its origin into the
            // overlay's coordinate space; fall back to the corner).
            #[allow(deprecated)]
            let (x, y) = btn
                .translate_coordinates(&overlay_m, 0.0, 0.0)
                .unwrap_or((0.0, 0.0));
            show_card_menu(
                &overlay_m,
                state_m.clone(),
                stack_m.clone(),
                idx,
                x,
                y,
                reorderable,
            );
        });
    }
    actions.append(&more);
    overlay.add_overlay(&actions);

    let motion = gtk4::EventControllerMotion::new();
    {
        let actions = actions.clone();
        motion.connect_enter(move |_, _, _| actions.set_visible(true));
    }
    {
        let actions = actions.clone();
        motion.connect_leave(move |_| actions.set_visible(false));
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
            show_card_menu(
                &overlay_c,
                state_c.clone(),
                stack_c.clone(),
                idx,
                x,
                y,
                reorderable,
            );
        });
    }
    overlay.add_controller(rclick);

    // Video/GIF cards play a muted, looping preview while hovered. Rotated
    // entries keep their static (rotated) thumbnail instead: GTK's MediaFile
    // can't rotate, and motion in the WRONG orientation reads as a bug.
    if entry.rotation.unwrap_or(0).is_multiple_of(360) {
        if let Some(video) = preview_video_path(entry) {
            super::hover_preview::attach(&overlay, &pic, video);
        }
    }

    // Minimum 16:9 poster footprint whose height derives from the FlowBox's
    // allocated cell width, so cards grow with the window instead of being
    // pinned to a fixed pixel size; homogeneous(true) on the FlowBox gives
    // every cell in a row the same width, so they all resolve to the same
    // aspect height too — never stretched or distorted.
    let frame = gtk4::AspectFrame::new(0.5, 0.0, 16.0 / 9.0, false);
    frame.set_size_request(260, 146);
    frame.set_child(Some(&overlay));
    frame
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
        Category::Images => t!("Images"),
        Category::Videos => t!("Videos"),
        Category::Gifs => t!("GIFs"),
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
#[allow(clippy::too_many_arguments)]
fn show_card_menu(
    parent: &gtk4::Overlay,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    idx: usize,
    x: f64,
    y: f64,
    reorderable: bool,
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

    let set = item(t!("Set as wallpaper"));
    {
        let s = state.clone();
        let p = pop.clone();
        set.connect_clicked(move |_| {
            apply_entry_by_idx(s.clone(), idx);
            p.popdown();
        });
    }
    menu.append(&set);

    // When this card is the wallpaper on screen, offer to turn it off — the
    // non-destructive counterpart to "Set" (the desktop reverts to its own
    // background; the library entry is kept). This is the "unset" action.
    let is_active = {
        let s = state.borrow();
        s.entries
            .get(idx)
            .map(|e| entry_is_active(e, &s.config))
            .unwrap_or(false)
    };
    if is_active {
        let stop = item(t!("Stop wallpaper"));
        let s = state.clone();
        let p = pop.clone();
        stop.connect_clicked(move |_| {
            stop_wallpaper(&s);
            show_toast(
                &s,
                t!("Wallpaper stopped — desktop reverted to its own background"),
            );
            let refresh = s.borrow().refresh.clone();
            if let Some(r) = refresh {
                r();
            }
            p.popdown();
        });
        menu.append(&stop);
    }

    // Per-monitor assignment (ROADMAP 2.2): only offered with 2+ displays —
    // single-monitor users never see extra chrome.
    let displays = connected_monitors();
    if displays.len() >= 2 {
        for m in &displays {
            let label = tf!(
                "Set on {display} ({width}×{height})",
                "display" => m.connector,
                "width" => m.width.to_string(),
                "height" => m.height.to_string()
            );
            let btn = item(&label);
            let s = state.clone();
            let p = pop.clone();
            let connector = m.connector.clone();
            btn.connect_clicked(move |_| {
                apply_entry_on_monitor(s.clone(), idx, &connector);
                p.popdown();
            });
            menu.append(&btn);
        }
    }
    if !state.borrow().config.monitors.is_empty() {
        let clear = item(t!("Show default on all displays"));
        let s = state.clone();
        let p = pop.clone();
        clear.connect_clicked(move |_| {
            clear_overrides_and_apply(s.clone());
            p.popdown();
        });
        menu.append(&clear);
    }

    // Browser-only wallpaper (webbridge): shown in the extension's new tabs
    // instead of mirroring the desktop.
    let browser = item(t!("Set as browser wallpaper"));
    {
        let s = state.clone();
        let p = pop.clone();
        browser.connect_clicked(move |_| {
            set_browser_wallpaper(s.clone(), idx);
            p.popdown();
        });
    }
    menu.append(&browser);
    if state.borrow().config.browser_wallpaper.is_some() {
        let clear_b = item(t!("Clear browser wallpaper"));
        let s = state.clone();
        let p = pop.clone();
        clear_b.connect_clicked(move |_| {
            {
                let mut st = s.borrow_mut();
                st.config.browser_wallpaper = None;
                st.config.save().ok();
            }
            show_toast(&s, t!("Browser wallpaper cleared — mirroring the desktop"));
            p.popdown();
        });
        menu.append(&clear_b);
    }

    let is_fav = state
        .borrow()
        .entries
        .get(idx)
        .map(|e| e.favorite)
        .unwrap_or(false);
    let fav = item(if is_fav {
        t!("Unfavorite")
    } else {
        t!("Favorite")
    });
    {
        let s = state.clone();
        let p = pop.clone();
        fav.connect_clicked(move |_| {
            toggle_favorite(&s, idx);
            p.popdown();
        });
    }
    menu.append(&fav);

    let edit = item(t!("Edit / Crop…"));
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

    let rename = item(t!("Rename…"));
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

    let move_to = item(t!("Move to folder…"));
    {
        let s = state.clone();
        let p = pop.clone();
        let parent = parent.clone();
        let id = s.borrow().entries.get(idx).map(|e| e.id.clone());
        move_to.connect_clicked(move |_| {
            // Popdown BEFORE the dialog: an override-redirect popover outranks
            // any toplevel on X11 (issue #6) — see `menu_item_opening_window`.
            p.popdown();
            let Some(id) = id.clone() else { return };
            if let Some(window) = parent
                .root()
                .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
            {
                show_move_to_collection_dialog(&window, s.clone(), vec![id]);
            }
        });
    }
    menu.append(&move_to);

    // Multi-file entries own a list the user could not edit until now: adding
    // to, removing from or reordering a playlist meant rebuilding it by hand.
    let is_multi = state
        .borrow()
        .entries
        .get(idx)
        .map(|e| matches!(e.kind, Kind::Playlist | Kind::Slideshow))
        .unwrap_or(false);
    if is_multi {
        let items = item(t!("Edit items…"));
        let s = state.clone();
        let p = pop.clone();
        let parent = parent.clone();
        items.connect_clicked(move |_| {
            p.popdown();
            if let Some(window) = parent
                .root()
                .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
            {
                show_items_dialog(&window, s.clone(), idx);
            }
        });
        menu.append(&items);
    }

    // Keyboard/touch-reachable twin of the hover arrows, under the same rule.
    if reorderable {
        let id = state.borrow().entries.get(idx).map(|e| e.id.clone());
        for (label, delta) in [(t!("Move earlier"), -1i32), (t!("Move later"), 1i32)] {
            let b = item(label);
            let s = state.clone();
            let p = pop.clone();
            let id = id.clone();
            b.connect_clicked(move |_| {
                if let Some(id) = id.as_deref() {
                    move_entry(&s, id, delta);
                }
                p.popdown();
            });
            menu.append(&b);
        }
    }

    if state
        .borrow()
        .entries
        .get(idx)
        .map(|e| e.broken)
        .unwrap_or(false)
    {
        let relink = item(t!("Relink…"));
        {
            let s = state.clone();
            let p = pop.clone();
            let parent = parent.clone();
            relink.connect_clicked(move |_| {
                p.popdown();
                if let Some(window) = parent
                    .root()
                    .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
                {
                    relink_entry(&window, s.clone(), idx);
                }
            });
        }
        menu.append(&relink);
    }

    menu.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    // Gateway into multi-select, pre-ticked with the card you right-clicked.
    let select = item(t!("Select…"));
    {
        let s = state.clone();
        let p = pop.clone();
        let id = s.borrow().entries.get(idx).map(|e| e.id.clone());
        select.connect_clicked(move |_| {
            enter_selection(&s, id.clone());
            p.popdown();
        });
    }
    menu.append(&select);

    let remove = item(t!("Remove from library"));
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

/// Toggle an entry's favorite flag, persist, and refresh (the Favorites
/// section and heart glyphs re-render on refresh).
fn toggle_favorite(state: &Rc<RefCell<AppState>>, idx: usize) {
    let now_fav = {
        let mut s = state.borrow_mut();
        let Some(e) = s.entries.get_mut(idx) else {
            return;
        };
        e.favorite = !e.favorite;
        let now = e.favorite;
        save_entries(&s.entries).ok();
        now
    };
    show_toast(
        state,
        if now_fav {
            t!("Added to Favorites")
        } else {
            t!("Removed from Favorites")
        },
    );
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// Case-insensitive substring match over both the raw and prettified names
/// (`q` must already be lowercased). Shared by home search and the palette.
/// Nudge one card one place within its own folder. `delta` is -1 (earlier) or
/// +1 (later).
///
/// The model returns `false` for "already at that end" (and for an id that has
/// since been removed). That is not an error, but it must not be treated as a
/// move either: writing the library and rebuilding the whole grid for a move
/// that did not happen shows up as a visible flicker on a button that did
/// nothing.
fn move_entry(state: &Rc<RefCell<AppState>>, id: &str, delta: i32) {
    {
        let mut s = state.borrow_mut();
        let moved = if delta < 0 {
            library::move_entry_up(&mut s.entries, id)
        } else {
            library::move_entry_down(&mut s.entries, id)
        };
        if !moved {
            return;
        }
        save_entries(&s.entries).ok();
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// File `ids` into `collection` (`None` = back out of every folder), persist,
/// and redraw. Leaves select mode, since the cards it was aimed at have just
/// moved somewhere the user may not be looking.
fn move_entries_to_collection(
    state: &Rc<RefCell<AppState>>,
    ids: &[String],
    collection: Option<String>,
) {
    if ids.is_empty() {
        return;
    }
    let name = {
        let mut s = state.borrow_mut();
        library::assign(&mut s.entries, ids, collection.as_deref());
        // `assign` renumbers both the destination and every folder the entries
        // left, so this one write is the whole story.
        save_entries(&s.entries).ok();
        s.selection = None;
        collection
            .as_deref()
            .and_then(|id| library::collection_name(&s.collections, id))
            .map(str::to_string)
    };
    let msg = match (name, ids.len()) {
        (Some(name), 1) => tf!("Moved to “{folder}”", "folder" => name),
        (Some(name), n) => {
            tf!("Moved {count} wallpapers to “{folder}”", "count" => n.to_string(), "folder" => name)
        }
        (None, 1) => t!("Removed from its folder").to_string(),
        (None, n) => tf!("Removed {count} wallpapers from their folders", "count" => n.to_string()),
    };
    show_toast(state, &msg);
    refresh_selection(state);
}

fn entry_matches_query(e: &LibraryEntry, q: &str) -> bool {
    q.is_empty()
        || e.name.to_lowercase().contains(q)
        || display_name(&e.name, e.kind).to_lowercase().contains(q)
}

/// Lazily probe media metadata (resolution / fps / file size) for entries that
/// don't have it yet — one background thread for the whole batch, one save +
/// grid refresh when it completes. ffprobe missing is fine: file size still
/// fills in (which also marks the entry probed, so this never loops forever).
fn spawn_metadata_probe(state: &Rc<RefCell<AppState>>) {
    let pending: Vec<(String, PathBuf)> = state
        .borrow()
        .entries
        .iter()
        .filter(|e| e.needs_probe())
        .filter_map(|e| e.probe_source().map(|p| (e.id.clone(), p)))
        .collect();
    if pending.is_empty() {
        return;
    }

    let (tx, rx) = async_channel::bounded::<Vec<(String, library::MediaMeta)>>(1);
    std::thread::spawn(move || {
        let results: Vec<(String, library::MediaMeta)> = pending
            .into_iter()
            .map(|(id, path)| (id, library::probe_media(&path)))
            .collect();
        let _ = tx.send_blocking(results);
    });

    let state = state.clone();
    glib::spawn_future_local(async move {
        let Ok(results) = rx.recv().await else {
            return;
        };
        let mut changed = false;
        {
            let mut s = state.borrow_mut();
            for (id, meta) in results {
                if let Some(e) = s.entries.iter_mut().find(|e| e.id == id) {
                    e.width = meta.width;
                    e.height = meta.height;
                    e.fps = meta.fps;
                    e.size_bytes = meta.size_bytes;
                    changed = true;
                }
            }
            if changed {
                save_entries(&s.entries).ok();
            }
        }
        if changed {
            let refresh = state.borrow().refresh.clone();
            if let Some(r) = refresh {
                r();
            }
        }
    });
}

/// Remove a library entry (and its cached thumbnail). Does not touch the
/// original media file. Refreshes the grid afterwards.
/// Turn the wallpaper off and revert the desktop to its own background, without
/// deleting anything. Disables autostart (so login doesn't resurrect it),
/// clears the active wallpaper from config, and stops the daemon — which tears
/// down its renderers (destroying the desktop window / restoring the DDE
/// wallpaper). Setting any wallpaper again re-enables and respawns it.
fn stop_wallpaper(state: &Rc<RefCell<AppState>>) {
    {
        let mut s = state.borrow_mut();
        s.config.enabled = false;
        s.config.wallpaper.path = None;
        s.config.wallpaper.paths.clear();
        s.config.wallpaper.slideshow = None;
        s.config.monitors.clear();
        s.config.save().ok();
    }
    if crate::ipc::daemon_alive() {
        let _ = crate::ipc::request(&crate::ipc::Request::Stop);
    }
}

fn remove_entry_by_idx(state: Rc<RefCell<AppState>>, idx: usize) {
    let was_active;
    {
        let mut s = state.borrow_mut();
        if idx >= s.entries.len() {
            return;
        }
        let entry = s.entries.remove(idx);
        // Removing the wallpaper that's currently on screen must take it OFF
        // screen — otherwise the daemon keeps playing a wallpaper the user just
        // deleted, and the desktop never returns to its own background.
        was_active = entry_is_active(&entry, &s.config);
        if let Some(thumb) = &entry.thumbnail {
            std::fs::remove_file(thumb).ok();
        }
        save_entries(&s.entries).ok();
    }
    // Removing the wallpaper that's on screen must also take it off screen.
    if was_active {
        stop_wallpaper(&state);
    }
    show_toast(
        &state,
        if was_active {
            t!("Removed — desktop reverted to its own wallpaper")
        } else {
            t!("Removed from library")
        },
    );
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

// ─── Multi-select ────────────────────────────────────────────────────────────

/// Enter select mode, optionally with `first` already ticked.
fn enter_selection(state: &Rc<RefCell<AppState>>, first: Option<String>) {
    {
        let mut s = state.borrow_mut();
        let mut set = std::collections::HashSet::new();
        if let Some(id) = first {
            set.insert(id);
        }
        s.selection = Some(set);
    }
    refresh_selection(state);
}

fn exit_selection(state: &Rc<RefCell<AppState>>) {
    state.borrow_mut().selection = None;
    refresh_selection(state);
}

fn toggle_selected(state: &Rc<RefCell<AppState>>, id: &str) {
    {
        let mut s = state.borrow_mut();
        let Some(set) = s.selection.as_mut() else {
            return;
        };
        if !set.remove(id) {
            set.insert(id.to_string());
        }
    }
    refresh_selection(state);
}

/// Tick every entry currently listed (honors the active search filter, so
/// "Select all" over a search means "all matches").
fn select_all_visible(state: &Rc<RefCell<AppState>>, visible: &[String]) {
    {
        let mut s = state.borrow_mut();
        let Some(set) = s.selection.as_mut() else {
            return;
        };
        // Already all ticked → clear, so the button toggles both ways.
        if visible.iter().all(|id| set.contains(id)) {
            set.clear();
        } else {
            set.extend(visible.iter().cloned());
        }
    }
    refresh_selection(state);
}

/// Repaint the grid (checkboxes) and the footer together.
fn refresh_selection(state: &Rc<RefCell<AppState>>) {
    let (refresh, sync) = {
        let s = state.borrow();
        (s.refresh.clone(), s.sync_selection_ui.clone())
    };
    if let Some(f) = sync {
        f();
    }
    if let Some(r) = refresh {
        r();
    }
}

/// Pull every entry whose id is in `ids` out of `entries`, returning them.
fn drain_by_ids(
    entries: &mut Vec<LibraryEntry>,
    ids: &std::collections::HashSet<String>,
) -> Vec<LibraryEntry> {
    let mut gone = Vec::new();
    entries.retain(|e| {
        if ids.contains(&e.id) {
            gone.push(e.clone());
            false
        } else {
            true
        }
    });
    gone
}

/// Delete every entry in `ids` in one pass: one library write, one toast, and a
/// single stop if any of them was the wallpaper on screen.
fn remove_entries_by_ids(state: Rc<RefCell<AppState>>, ids: &std::collections::HashSet<String>) {
    if ids.is_empty() {
        return;
    }
    let (removed, had_active) = {
        let mut s = state.borrow_mut();
        let gone = drain_by_ids(&mut s.entries, ids);
        let had_active = gone.iter().any(|e| entry_is_active(e, &s.config));
        for e in &gone {
            if let Some(thumb) = &e.thumbnail {
                std::fs::remove_file(thumb).ok();
            }
        }
        save_entries(&s.entries).ok();
        s.selection = None;
        (gone.len(), had_active)
    };
    if had_active {
        stop_wallpaper(&state);
    }
    let msg = match (removed, had_active) {
        (1, true) => t!("Removed — desktop reverted to its own wallpaper").to_string(),
        (1, false) => t!("Removed from library").to_string(),
        (n, true) => tf!(
            "Removed {count} wallpapers — desktop reverted to its own wallpaper",
            "count" => n.to_string()
        ),
        (n, false) => tf!("Removed {count} wallpapers", "count" => n.to_string()),
    };
    show_toast(&state, &msg);
    refresh_selection(&state);
}

/// Confirm before a bulk delete. Removing many wallpapers at once can't be
/// undone, and the single-card path is already one click behind a menu, so the
/// batch path gets an explicit count to check against.
fn confirm_remove_selected(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    ids: std::collections::HashSet<String>,
) {
    let n = ids.len();
    let active_hit = {
        let s = state.borrow();
        s.entries
            .iter()
            .any(|e| ids.contains(&e.id) && entry_is_active(e, &s.config))
    };

    let (dialog, content) = glass_dialog(window, t!("Remove wallpapers"), 400, -1);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_top(4);
    body.set_margin_bottom(16);
    body.set_margin_start(20);
    body.set_margin_end(20);

    let heading = gtk4::Label::new(Some(&if n == 1 {
        t!("Remove 1 wallpaper?").to_string()
    } else {
        tf!("Remove {count} wallpapers?", "count" => n.to_string())
    }));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    body.append(&heading);

    let sub = gtk4::Label::new(Some(if active_hit {
        t!("They leave your Fresco library and the desktop reverts to its own wallpaper. The source files on disk are kept.")
    } else {
        t!("They leave your Fresco library. The source files on disk are kept.")
    }));
    sub.add_css_class("dialog-sub");
    sub.set_xalign(0.0);
    sub.set_wrap(true);
    body.append(&sub);

    let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    actions.set_halign(gtk4::Align::End);
    actions.set_margin_top(8);
    let cancel = gtk4::Button::with_label(t!("Cancel"));
    {
        let d = dialog.clone();
        cancel.connect_clicked(move |_| d.close());
    }
    actions.append(&cancel);
    let confirm = gtk4::Button::with_label(if n == 1 {
        t!("Remove")
    } else {
        t!("Remove all")
    });
    confirm.add_css_class("destructive-action");
    {
        let d = dialog.clone();
        let state2 = state.clone();
        confirm.connect_clicked(move |_| {
            remove_entries_by_ids(state2.clone(), &ids);
            d.close();
        });
    }
    actions.append(&confirm);
    body.append(&actions);

    content.append(&body);
    dialog.present();
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

// ─── Folders (collections) ───────────────────────────────────────────────────

/// Keep `state.collections` in the order it is displayed in, so the manage
/// dialog's up/down arrows can work on plain vector indices.
fn sort_collections_in_place(collections: &mut [library::Collection]) {
    collections.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));
}

/// Move a folder one place in the sidebar order. Returns false at the ends.
fn shift_collection(state: &Rc<RefCell<AppState>>, id: &str, delta: i32) -> bool {
    let mut s = state.borrow_mut();
    sort_collections_in_place(&mut s.collections);
    let Some(pos) = s.collections.iter().position(|c| c.id == id) else {
        return false;
    };
    let target = pos as i32 + delta;
    if target < 0 || target as usize >= s.collections.len() {
        return false;
    }
    s.collections.swap(pos, target as usize);
    library::renumber_collections(&mut s.collections);
    sort_collections_in_place(&mut s.collections);
    library::save_collections(&s.collections).ok();
    true
}

/// Create a folder and return its id, or `None` for a blank name.
fn add_collection(state: &Rc<RefCell<AppState>>, name: &str) -> Option<String> {
    if name.trim().is_empty() {
        return None;
    }
    let mut s = state.borrow_mut();
    let id = library::create_collection(&mut s.collections, name);
    sort_collections_in_place(&mut s.collections);
    library::save_collections(&s.collections).ok();
    Some(id)
}

/// Manage the folder list: create, rename, reorder, delete.
///
/// Deleting is offered without a confirmation on purpose — it is the one
/// destructive-looking action here that destroys nothing. `delete_collection`
/// hands the folder's contents back to Uncategorized in the order they were
/// in, so the worst case is one "New folder" and one bulk move to undo it.
fn show_collections_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let (dialog, content) = glass_dialog(window, t!("Folders"), 460, 520);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    body.set_margin_start(20);
    body.set_margin_end(20);
    body.set_margin_bottom(18);

    let sub = gtk4::Label::new(Some(t!(
        "Your own categories — SCI-FI, Nature, Cityscapes. Deleting a folder never deletes wallpapers: its contents go back to Uncategorized."
    )));
    sub.add_css_class("dialog-sub");
    sub.set_xalign(0.0);
    sub.set_wrap(true);
    body.append(&sub);

    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    body.append(&scroll);

    // Self-referential rebuild: every row's action has to redraw the list it
    // lives in, and the rows are built by the very closure they call.
    let rebuild: SelfRebuild = Rc::new(RefCell::new(None));
    let build: Rc<dyn Fn()> = {
        let state = state.clone();
        let list = list.clone();
        let rebuild = rebuild.clone();
        Rc::new(move || {
            while let Some(row) = list.first_child() {
                list.remove(&row);
            }
            let (collections, counts) = {
                let s = state.borrow();
                let counts: Vec<usize> = s
                    .collections
                    .iter()
                    .map(|c| library::entries_in(&s.entries, &c.id).len())
                    .collect();
                (s.collections.clone(), counts)
            };
            if collections.is_empty() {
                let empty = adw::ActionRow::new();
                empty.set_title(t!("No folders yet"));
                empty.set_subtitle(t!("Create one below, then move wallpapers into it"));
                list.append(&empty);
            }
            let again = {
                let rebuild = rebuild.clone();
                move || {
                    let f = rebuild.borrow().clone();
                    if let Some(f) = f {
                        f();
                    }
                }
            };
            for (i, c) in collections.iter().enumerate() {
                let row = adw::ActionRow::new();
                row.set_title(&glib::markup_escape_text(&c.name));
                let n = counts.get(i).copied().unwrap_or(0);
                row.set_subtitle(&if n == 1 {
                    t!("1 wallpaper").to_string()
                } else {
                    tf!("{count} wallpapers", "count" => n.to_string())
                });

                for (icon, tip, delta) in [
                    ("go-up-symbolic", t!("Move up"), -1i32),
                    ("go-down-symbolic", t!("Move down"), 1i32),
                ] {
                    let b = gtk4::Button::from_icon_name(icon);
                    b.add_css_class("flat");
                    b.set_valign(gtk4::Align::Center);
                    b.set_tooltip_text(Some(tip));
                    let state2 = state.clone();
                    let id = c.id.clone();
                    let again = again.clone();
                    b.connect_clicked(move |_| {
                        if shift_collection(&state2, &id, delta) {
                            again();
                            let r = state2.borrow().refresh.clone();
                            if let Some(r) = r {
                                r();
                            }
                        }
                    });
                    row.add_suffix(&b);
                }

                let ren = gtk4::Button::from_icon_name("document-edit-symbolic");
                ren.add_css_class("flat");
                ren.set_valign(gtk4::Align::Center);
                ren.set_tooltip_text(Some(t!("Rename…")));
                {
                    let state2 = state.clone();
                    let id = c.id.clone();
                    let current = c.name.clone();
                    let again = again.clone();
                    ren.connect_clicked(move |btn| {
                        rename_collection_popover(
                            btn,
                            state2.clone(),
                            id.clone(),
                            &current,
                            again.clone(),
                        );
                    });
                }
                row.add_suffix(&ren);

                let del = gtk4::Button::from_icon_name("user-trash-symbolic");
                del.add_css_class("flat");
                del.set_valign(gtk4::Align::Center);
                del.set_tooltip_text(Some(t!("Delete folder (wallpapers are kept)")));
                {
                    let state2 = state.clone();
                    let id = c.id.clone();
                    let again = again.clone();
                    del.connect_clicked(move |_| {
                        {
                            let mut s = state2.borrow_mut();
                            // Mutates BOTH vectors — the members are handed
                            // back to Uncategorized — so both get saved.
                            let mut cols = std::mem::take(&mut s.collections);
                            let removed =
                                library::delete_collection(&mut cols, &mut s.entries, &id);
                            s.collections = cols;
                            if !removed {
                                return;
                            }
                            library::save_collections(&s.collections).ok();
                            save_entries(&s.entries).ok();
                            s.view = normalize_view(s.view.clone(), &s.collections);
                            library::save_view(&s.view).ok();
                        }
                        show_toast(
                            &state2,
                            t!("Folder deleted — its wallpapers moved to Uncategorized"),
                        );
                        again();
                        let r = state2.borrow().refresh.clone();
                        if let Some(r) = r {
                            r();
                        }
                    });
                }
                row.add_suffix(&del);
                list.append(&row);
            }
        })
    };
    *rebuild.borrow_mut() = Some(build.clone());
    build();

    let add_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let name_entry = gtk4::Entry::new();
    name_entry.set_placeholder_text(Some(t!("New folder name")));
    name_entry.set_hexpand(true);
    let add_btn = gtk4::Button::with_label(t!("Create"));
    add_btn.add_css_class("suggested-action");
    add_row.append(&name_entry);
    add_row.append(&add_btn);
    body.append(&add_row);

    // Two commit paths, as everywhere else in this file: the button and Enter.
    let commit = {
        let state = state.clone();
        let name_entry = name_entry.clone();
        let build = build.clone();
        Rc::new(move || {
            if add_collection(&state, &name_entry.text()).is_none() {
                return;
            }
            name_entry.set_text("");
            build();
            let r = state.borrow().refresh.clone();
            if let Some(r) = r {
                r();
            }
        })
    };
    {
        let commit = commit.clone();
        add_btn.connect_clicked(move |_| commit());
    }
    {
        let commit = commit.clone();
        name_entry.connect_activate(move |_| commit());
    }

    content.append(&body);
    dialog.present();
}

/// Inline rename prompt for a folder — the same popover shape as
/// [`rename_entry`], parented to the pencil button that opened it.
fn rename_collection_popover(
    anchor: &gtk4::Button,
    state: Rc<RefCell<AppState>>,
    id: String,
    current: &str,
    after: impl Fn() + Clone + 'static,
) {
    let pop = gtk4::Popover::new();
    pop.set_parent(anchor);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row.set_margin_top(6);
    row.set_margin_bottom(6);
    row.set_margin_start(6);
    row.set_margin_end(6);
    let entry = gtk4::Entry::new();
    entry.set_text(current);
    entry.set_hexpand(true);
    let save = gtk4::Button::from_icon_name("emblem-ok-symbolic");
    save.add_css_class("suggested-action");
    row.append(&entry);
    row.append(&save);
    pop.set_child(Some(&row));

    let commit = {
        let state = state.clone();
        let id = id.clone();
        let after = after.clone();
        Rc::new(move |text: String| {
            {
                let mut s = state.borrow_mut();
                if !library::rename_collection(&mut s.collections, &id, &text) {
                    return;
                }
                library::save_collections(&s.collections).ok();
            }
            after();
            let r = state.borrow().refresh.clone();
            if let Some(r) = r {
                r();
            }
        })
    };
    {
        let commit = commit.clone();
        let entry = entry.clone();
        let pop = pop.clone();
        save.connect_clicked(move |_| {
            commit(entry.text().to_string());
            pop.popdown();
        });
    }
    {
        let commit = commit.clone();
        let pop = pop.clone();
        entry.connect_activate(move |e| {
            commit(e.text().to_string());
            pop.popdown();
        });
    }
    pop.connect_closed(|p| p.unparent());
    pop.popup();
    entry.grab_focus();
}

/// Pick a destination folder for one card or a whole selection.
///
/// A dialog rather than a submenu: a popover opened from inside another
/// popover takes a second seat grab, which is what leaves the toplevel dead to
/// input on X11/DDE (issue #5, see `build_language_row`).
fn show_move_to_collection_dialog(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    ids: Vec<String>,
) {
    if ids.is_empty() {
        return;
    }
    let (dialog, content) = glass_dialog(window, t!("Move to folder"), 420, -1);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    body.set_margin_start(20);
    body.set_margin_end(20);
    body.set_margin_bottom(18);

    let heading = gtk4::Label::new(Some(&if ids.len() == 1 {
        t!("Move 1 wallpaper").to_string()
    } else {
        tf!("Move {count} wallpapers", "count" => ids.len().to_string())
    }));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    body.append(&heading);

    let choices = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    scroll.set_max_content_height(260);
    scroll.set_propagate_natural_height(true);
    scroll.set_child(Some(&choices));
    body.append(&scroll);

    let collections = {
        let s = state.borrow();
        s.collections.clone()
    };
    let ids = Rc::new(ids);
    for c in &collections {
        let btn = menu_item(&c.name);
        let state2 = state.clone();
        let ids = ids.clone();
        let id = c.id.clone();
        let d = dialog.clone();
        btn.connect_clicked(move |_| {
            move_entries_to_collection(&state2, &ids, Some(id.clone()));
            d.close();
        });
        choices.append(&btn);
    }
    {
        let btn = menu_item(t!("Uncategorized"));
        let state2 = state.clone();
        let ids = ids.clone();
        let d = dialog.clone();
        btn.connect_clicked(move |_| {
            move_entries_to_collection(&state2, &ids, None);
            d.close();
        });
        choices.append(&btn);
    }

    body.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    let new_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let name_entry = gtk4::Entry::new();
    name_entry.set_placeholder_text(Some(t!("New folder name")));
    name_entry.set_hexpand(true);
    let create = gtk4::Button::with_label(t!("Create & move"));
    create.add_css_class("suggested-action");
    new_row.append(&name_entry);
    new_row.append(&create);
    body.append(&new_row);

    let commit = {
        let state = state.clone();
        let name_entry = name_entry.clone();
        let ids = ids.clone();
        let d = dialog.clone();
        Rc::new(move || {
            let Some(id) = add_collection(&state, &name_entry.text()) else {
                return;
            };
            move_entries_to_collection(&state, &ids, Some(id));
            d.close();
        })
    };
    {
        let commit = commit.clone();
        create.connect_clicked(move |_| commit());
    }
    {
        let commit = commit.clone();
        name_entry.connect_activate(move |_| commit());
    }

    content.append(&body);
    dialog.present();
}

// ─── Playlist / slideshow item editing ───────────────────────────────────────

/// Persist an edit to a multi-file entry's item list.
///
/// `add_paths` / `remove_path_at` / `move_path` clear `thumbnail` when the item
/// the poster was rendered from (index 0) changed. Regenerating it shells out
/// to ffmpeg, so it happens here on a clone — never while a `borrow_mut` on the
/// shared state is held.
fn commit_item_edit(state: &Rc<RefCell<AppState>>, idx: usize) {
    let stale = {
        let s = state.borrow();
        s.entries
            .get(idx)
            .filter(|e| e.thumbnail.is_none())
            .cloned()
    };
    if let Some(mut clone) = stale {
        clone.generate_thumbnail();
        let mut s = state.borrow_mut();
        if let Some(e) = s.entries.get_mut(idx) {
            if e.id == clone.id {
                e.thumbnail = clone.thumbnail;
            }
        }
    }
    {
        let s = state.borrow();
        save_entries(&s.entries).ok();
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// Edit what is inside a playlist or slideshow: add files, drop items, reorder.
///
/// Until now a multi-file entry was write-once — the only way to change it was
/// to delete it and re-pick every file, which is exactly the complaint that
/// drove this ("no way to add wallpapers to that list or take some out").
fn show_items_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>, idx: usize) {
    let Some(entry) = state.borrow().entries.get(idx).cloned() else {
        return;
    };
    let (dialog, content) = glass_dialog(window, t!("Edit items"), 560, 460);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    body.set_margin_start(20);
    body.set_margin_end(20);
    body.set_margin_bottom(18);

    let heading = gtk4::Label::new(Some(&entry.name));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    heading.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    body.append(&heading);

    let note = gtk4::Label::new(None);
    note.add_css_class("dialog-sub");
    note.set_xalign(0.0);
    note.set_wrap(true);
    body.append(&note);

    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    body.append(&scroll);

    let rebuild: SelfRebuild = Rc::new(RefCell::new(None));
    let build: Rc<dyn Fn()> = {
        let state = state.clone();
        let list = list.clone();
        let note = note.clone();
        let rebuild = rebuild.clone();
        Rc::new(move || {
            while let Some(row) = list.first_child() {
                list.remove(&row);
            }
            let Some(e) = state.borrow().entries.get(idx).cloned() else {
                return;
            };
            // A folder-backed slideshow has no `paths` to edit yet: show what
            // the folder currently holds, read-only, and say plainly what
            // adding a file does to it (`add_paths` materialises the folder).
            let folder_backed = e.paths.is_empty() && e.folder.is_some();
            if folder_backed {
                note.set_text(t!(
                    "This slideshow follows a folder. Adding a file freezes today’s contents into a hand-picked list, and it stops following the folder."
                ));
            } else {
                note.set_text(t!(
                    "Reorder with the arrows; the first item is the one the card’s thumbnail comes from."
                ));
            }
            let items: Vec<PathBuf> = if folder_backed {
                e.folder
                    .as_deref()
                    .map(|f| library::folder_media(f, false))
                    .unwrap_or_default()
            } else {
                e.paths.clone()
            };
            if items.is_empty() {
                let empty = adw::ActionRow::new();
                empty.set_title(t!("Nothing in this list yet"));
                list.append(&empty);
            }
            let again = {
                let rebuild = rebuild.clone();
                move || {
                    let f = rebuild.borrow().clone();
                    if let Some(f) = f {
                        f();
                    }
                }
            };
            let last = items.len().saturating_sub(1);
            for (i, path) in items.iter().enumerate() {
                let row = adw::ActionRow::new();
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                row.set_title(&glib::markup_escape_text(&name));
                row.set_subtitle(&glib::markup_escape_text(
                    &path.parent().unwrap_or(path).to_string_lossy(),
                ));
                if !path.exists() {
                    row.set_subtitle(t!("Missing"));
                }
                if folder_backed {
                    list.append(&row);
                    continue;
                }
                for (icon, tip, delta) in [
                    ("go-up-symbolic", t!("Move up"), -1i32),
                    ("go-down-symbolic", t!("Move down"), 1i32),
                ] {
                    let b = gtk4::Button::from_icon_name(icon);
                    b.add_css_class("flat");
                    b.set_valign(gtk4::Align::Center);
                    b.set_tooltip_text(Some(tip));
                    b.set_sensitive(if delta < 0 { i > 0 } else { i < last });
                    let state2 = state.clone();
                    let again = again.clone();
                    b.connect_clicked(move |_| {
                        let target = i as i32 + delta;
                        if target < 0 {
                            return;
                        }
                        let moved = {
                            let mut s = state2.borrow_mut();
                            s.entries
                                .get_mut(idx)
                                .map(|e| e.move_path(i, target as usize))
                                .unwrap_or(false)
                        };
                        if !moved {
                            return;
                        }
                        commit_item_edit(&state2, idx);
                        again();
                    });
                    row.add_suffix(&b);
                }
                let del = gtk4::Button::from_icon_name("list-remove-symbolic");
                del.add_css_class("flat");
                del.set_valign(gtk4::Align::Center);
                del.set_tooltip_text(Some(t!("Remove this item")));
                {
                    let state2 = state.clone();
                    let again = again.clone();
                    del.connect_clicked(move |_| {
                        let gone = {
                            let mut s = state2.borrow_mut();
                            s.entries
                                .get_mut(idx)
                                .and_then(|e| e.remove_path_at(i))
                                .is_some()
                        };
                        if !gone {
                            return;
                        }
                        commit_item_edit(&state2, idx);
                        again();
                    });
                }
                row.add_suffix(&del);
                list.append(&row);
            }
        })
    };
    *rebuild.borrow_mut() = Some(build.clone());
    build();

    let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    actions.set_halign(gtk4::Align::End);
    let add = gtk4::Button::with_label(t!("Add files…"));
    add.add_css_class("suggested-action");
    {
        let state2 = state.clone();
        let build = build.clone();
        let win = window.clone();
        add.connect_clicked(move |_| {
            pick_items_to_add(&win, state2.clone(), idx, build.clone());
        });
    }
    actions.append(&add);
    let close = gtk4::Button::with_label(t!("Done"));
    {
        let d = dialog.clone();
        close.connect_clicked(move |_| d.close());
    }
    actions.append(&close);
    body.append(&actions);

    content.append(&body);
    dialog.present();
}

/// File picker feeding [`LibraryEntry::add_paths`] for the item editor.
fn pick_items_to_add(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    idx: usize,
    after: Rc<dyn Fn()>,
) {
    let chooser = gtk4::FileChooserNative::new(
        Some(t!("Add to this list")),
        Some(window),
        FileChooserAction::Open,
        Some(t!("Open")),
        Some(t!("Cancel")),
    );
    chooser.set_select_multiple(true);
    let all_pat: Vec<&str> = VIDEO_PATTERNS
        .iter()
        .chain(IMAGE_PATTERNS.iter())
        .copied()
        .collect();
    chooser.add_filter(&media_filter(t!("All supported"), &all_pat));

    let state_cb = state.clone();
    chooser.connect_response(move |ch, resp| {
        state_cb.borrow_mut().current_picker = None;
        if resp != ResponseType::Accept {
            return;
        }
        let model = ch.files();
        let mut paths: Vec<PathBuf> = Vec::new();
        for i in 0..model.n_items() {
            if let Some(p) = model
                .item(i)
                .and_then(|o| o.downcast::<gio::File>().ok())
                .and_then(|f| f.path())
            {
                paths.push(p);
            }
        }
        if paths.is_empty() {
            return;
        }
        {
            let mut s = state_cb.borrow_mut();
            if let Some(e) = s.entries.get_mut(idx) {
                e.add_paths(paths);
            }
        }
        commit_item_edit(&state_cb, idx);
        after();
    });
    state.borrow_mut().current_picker = Some(chooser.clone());
    chooser.show();
}

// ─── Batch import ────────────────────────────────────────────────────────────

/// Import a batch as **one wallpaper per file**, skipping anything the library
/// already holds. Returns `(added, skipped)`.
fn import_individually(state: &Rc<RefCell<AppState>>, paths: Vec<PathBuf>) -> (usize, usize) {
    let (fresh, dupes) = {
        let s = state.borrow();
        library::partition_new(&s.entries, paths)
    };
    let new_entries = library::entries_for_each(fresh);
    let ids: Vec<String> = new_entries.iter().map(|e| e.id.clone()).collect();
    {
        let mut s = state.borrow_mut();
        for e in new_entries {
            // `push_entry`, not `Vec::push`: a plain push lands the entry at
            // `order == 0`, i.e. jumps it to the front of the manual order.
            library::push_entry(&mut s.entries, e);
        }
        save_entries(&s.entries).ok();
    }
    spawn_thumbnail_batch(state, ids.clone());
    spawn_metadata_probe(state);
    (ids.len(), dupes.len())
}

/// Render thumbnails for a batch off the UI thread.
///
/// One `generate_thumbnail` is an ffmpeg process; fifty of them in a row on the
/// main loop is a frozen window for the length of a folder import. Mirrors
/// `spawn_metadata_probe`: one worker thread, one save, one redraw.
fn spawn_thumbnail_batch(state: &Rc<RefCell<AppState>>, ids: Vec<String>) {
    let pending: Vec<LibraryEntry> = {
        let s = state.borrow();
        s.entries
            .iter()
            .filter(|e| ids.contains(&e.id) && e.thumbnail.is_none())
            .cloned()
            .collect()
    };
    if pending.is_empty() {
        return;
    }
    let (tx, rx) = async_channel::bounded::<Vec<(String, Option<PathBuf>)>>(1);
    std::thread::spawn(move || {
        let out: Vec<(String, Option<PathBuf>)> = pending
            .into_iter()
            .map(|mut e| {
                e.generate_thumbnail();
                (e.id, e.thumbnail)
            })
            .collect();
        let _ = tx.send_blocking(out);
    });
    let state = state.clone();
    glib::spawn_future_local(async move {
        let Ok(results) = rx.recv().await else {
            return;
        };
        {
            let mut s = state.borrow_mut();
            for (id, thumb) in results {
                if thumb.is_none() {
                    continue;
                }
                if let Some(e) = s.entries.iter_mut().find(|e| e.id == id) {
                    e.thumbnail = thumb;
                }
            }
            save_entries(&s.entries).ok();
        }
        let refresh = state.borrow().refresh.clone();
        if let Some(r) = refresh {
            r();
        }
    });
}

/// "Added 7 · skipped 3 duplicates" — one toast for a whole batch.
fn report_import(state: &Rc<RefCell<AppState>>, added: usize, skipped: usize) {
    let msg = match (added, skipped) {
        (0, 0) => t!("Nothing to add").to_string(),
        (0, n) => tf!("Already in your library — skipped {count}", "count" => n.to_string()),
        (a, 0) => tf!("Added {count} wallpapers", "count" => a.to_string()),
        (a, n) => {
            tf!("Added {count} · skipped {dupes} duplicates", "count" => a.to_string(), "dupes" => n.to_string())
        }
    };
    show_toast(state, &msg);
}

/// Ask what a multi-file pick should become.
///
/// The old behaviour — silently fusing every multi-selection into one playlist
/// — is the thing people hit when they download a batch of wallpapers and want
/// ten wallpapers, not one ten-item playlist. It is still a real intent though,
/// so this offers both rather than swapping one default for another.
fn show_import_choice_dialog(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    paths: Vec<PathBuf>,
) {
    let n = paths.len();
    let all_images = paths.iter().all(|p| library::is_image(p));
    let (dialog, content) = glass_dialog(window, t!("Add wallpapers"), 460, -1);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    body.set_margin_start(20);
    body.set_margin_end(20);
    body.set_margin_bottom(18);

    let heading = gtk4::Label::new(Some(&tf!(
        "{count} files selected",
        "count" => n.to_string()
    )));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    body.append(&heading);

    let paths = Rc::new(paths);

    let separate = gtk4::Button::new();
    separate.add_css_class("suggested-action");
    separate.set_child(Some(&two_line_choice(
        t!("Add as separate wallpapers"),
        t!("One entry per file. Keep one up for days and switch when the mood strikes."),
    )));
    {
        let state2 = state.clone();
        let paths = paths.clone();
        let d = dialog.clone();
        separate.connect_clicked(move |_| {
            let (added, skipped) = import_individually(&state2, (*paths).clone());
            report_import(&state2, added, skipped);
            let r = state2.borrow().refresh.clone();
            if let Some(r) = r {
                r();
            }
            d.close();
        });
    }
    body.append(&separate);

    let combined = gtk4::Button::new();
    combined.set_child(Some(&two_line_choice(
        if all_images {
            t!("Combine into one slideshow")
        } else {
            t!("Combine into one playlist")
        },
        t!("A single wallpaper that plays the files in sequence."),
    )));
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let paths = paths.clone();
        let d = dialog.clone();
        combined.connect_clicked(move |_| {
            add_media_paths(&state2, &stack2, (*paths).clone(), None);
            d.close();
        });
    }
    body.append(&combined);

    let cancel = gtk4::Button::with_label(t!("Cancel"));
    cancel.set_halign(gtk4::Align::End);
    {
        let d = dialog.clone();
        cancel.connect_clicked(move |_| d.close());
    }
    body.append(&cancel);

    content.append(&body);
    dialog.present();
}

/// Title + dim explanation, for the big choice buttons in the import dialogs.
fn two_line_choice(title: &str, sub: &str) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    b.set_margin_top(4);
    b.set_margin_bottom(4);
    let t = gtk4::Label::new(Some(title));
    t.set_xalign(0.0);
    t.set_halign(gtk4::Align::Start);
    b.append(&t);
    let s = gtk4::Label::new(Some(sub));
    s.add_css_class("dialog-sub");
    s.set_xalign(0.0);
    s.set_halign(gtk4::Align::Start);
    s.set_wrap(true);
    b.append(&s);
    b
}

/// Ask what a picked folder should become: the timed slideshow it has always
/// made, or one wallpaper per image in it.
fn show_folder_import_choice(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    folder: PathBuf,
) {
    let (dialog, content) = glass_dialog(window, t!("Add folder"), 460, -1);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    body.set_margin_start(20);
    body.set_margin_end(20);
    body.set_margin_bottom(18);

    let heading = gtk4::Label::new(Some(
        &folder
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| folder.to_string_lossy().into_owned()),
    ));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    heading.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    body.append(&heading);

    let recursive = gtk4::CheckButton::with_label(t!("Include subfolders"));
    body.append(&recursive);

    let slideshow = gtk4::Button::new();
    slideshow.set_child(Some(&two_line_choice(
        t!("As a timed slideshow"),
        t!("One wallpaper that cycles through the folder and follows what you put in it."),
    )));
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let folder2 = folder.clone();
        let d = dialog.clone();
        slideshow.connect_clicked(move |_| {
            create_folder_slideshow(&state2, &stack2, folder2.clone());
            d.close();
        });
    }
    body.append(&slideshow);

    let individual = gtk4::Button::new();
    individual.add_css_class("suggested-action");
    individual.set_child(Some(&two_line_choice(
        t!("As individual wallpapers"),
        t!("One entry per image or video in the folder, so you can pick between them."),
    )));
    {
        let state2 = state.clone();
        let folder2 = folder.clone();
        let recursive = recursive.clone();
        let d = dialog.clone();
        individual.connect_clicked(move |_| {
            let files = library::folder_media(&folder2, recursive.is_active());
            if files.is_empty() {
                show_toast(&state2, t!("No supported media in that folder"));
                d.close();
                return;
            }
            let (added, skipped) = import_individually(&state2, files);
            report_import(&state2, added, skipped);
            let r = state2.borrow().refresh.clone();
            if let Some(r) = r {
                r();
            }
            d.close();
        });
    }
    body.append(&individual);

    let cancel = gtk4::Button::with_label(t!("Cancel"));
    cancel.set_halign(gtk4::Align::End);
    {
        let d = dialog.clone();
        cancel.connect_clicked(move |_| d.close());
    }
    body.append(&cancel);

    content.append(&body);
    dialog.present();
}

/// The pre-1.2 "Add folder" outcome: one folder-backed slideshow, then the
/// editor. Unchanged behaviour, now reached through a choice.
fn create_folder_slideshow(state: &Rc<RefCell<AppState>>, stack: &gtk4::Stack, folder: PathBuf) {
    let mut entry = library::LibraryEntry::new_slideshow(folder);
    entry.generate_thumbnail();
    {
        let mut s = state.borrow_mut();
        library::push_entry(&mut s.entries, entry);
        let idx = s.entries.len() - 1;
        s.config.wallpaper = s.entries[idx].to_wallpaper();
        s.editing_idx = Some(idx);
        save_entries(&s.entries).ok();
    }
    stack.set_visible_child_name("editor");
}

/// Set an entry as the wallpaper of ONE display (a `config.monitors` override).
fn apply_entry_on_monitor(state: Rc<RefCell<AppState>>, idx: usize, connector: &str) {
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
        assign_entry_to_monitor(&mut s.config, wallpaper, connector);
        name
    };
    let ok = {
        let s = state.borrow();
        let r = daemon_ctl::ensure_daemon_and_apply(&s.config);
        save_entries(&s.entries).ok();
        if let Err(e) = &r {
            log::error!("failed to apply per-monitor wallpaper: {e}");
        }
        r.is_ok()
    };
    if ok {
        show_toast(
            &state,
            &tf!(
                "“{name}” set on {display}",
                "name" => name,
                "display" => connector
            ),
        );
    } else {
        show_toast(
            &state,
            t!("Couldn’t start the wallpaper. Run frescod --check"),
        );
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// Store a library entry as the browser-only wallpaper (served by the
/// daemon's local bridge instead of mirroring the desktop). No daemon apply
/// needed: the bridge re-reads config.toml on every request.
fn set_browser_wallpaper(state: Rc<RefCell<AppState>>, idx: usize) {
    crate::telemetry::event("browser_wallpaper_set", serde_json::json!({}));
    let bridge_on = {
        let mut s = state.borrow_mut();
        let Some(entry) = s.entries.get_mut(idx) else {
            return;
        };
        if entry.broken {
            return;
        }
        entry.touch();
        let wallpaper = entry.to_wallpaper();
        s.config.browser_wallpaper = Some(wallpaper);
        s.config.save().ok();
        save_entries(&s.entries).ok();
        s.config.browser_bridge
    };
    let msg = if bridge_on {
        t!("Browser new-tab wallpaper set").to_string()
    } else {
        t!("Browser new-tab wallpaper set — enable Browser new-tab in Settings").to_string()
    };
    show_toast(&state, &msg);
}

/// Clear all per-monitor overrides: the default wallpaper shows everywhere.
fn clear_overrides_and_apply(state: Rc<RefCell<AppState>>) {
    {
        let mut s = state.borrow_mut();
        clear_monitor_overrides(&mut s.config);
    }
    let ok = {
        let s = state.borrow();
        daemon_ctl::ensure_daemon_and_apply(&s.config).is_ok()
    };
    if ok {
        show_toast(&state, t!("Default wallpaper on all displays"));
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

pub(crate) fn apply_entry_by_idx(state: Rc<RefCell<AppState>>, idx: usize) {
    let (name, kind) = {
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
        let kind = wallpaper.kind;
        s.config.wallpaper = wallpaper;
        s.config.enabled = true;
        (name, kind)
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
        crate::telemetry::event(
            "wallpaper_set",
            serde_json::json!({ "kind": format!("{kind:?}").to_lowercase() }),
        );
        show_toast(&state, &tf!("“{name}” set as wallpaper", "name" => name));
        maybe_star_nudge(&state);
    } else {
        show_toast(
            &state,
            t!("Couldn’t start the wallpaper. Run frescod --check"),
        );
    }
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// Recurring ask, at a happy moment: once the user has 3+ successful applies,
/// a toast invites a GitHub star + feedback — at most once every 2 days, and
/// only right after a wallpaper visibly worked (the only honest time to ask).
fn maybe_star_nudge(state: &Rc<RefCell<AppState>>) {
    const NUDGE_INTERVAL_S: u64 = 2 * 24 * 60 * 60;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let show = {
        let mut s = state.borrow_mut();
        s.config.apply_count = s.config.apply_count.saturating_add(1);
        let show = s.config.apply_count >= 3
            && now.saturating_sub(s.config.last_star_nudge) >= NUDGE_INTERVAL_S;
        if show {
            s.config.last_star_nudge = now;
        }
        s.config.save().ok();
        show
    };
    if !show {
        return;
    }
    let toast = adw::Toast::new(
        "Enjoying Fresco? A star on GitHub helps other Linux users find it. \
         Already starred? Feel free to ignore this.",
    );
    toast.set_button_label(Some(t!("Star on GitHub")));
    toast.set_timeout(0); // sticky until acted on or dismissed
                          // This libadwaita binding predates connect_button_clicked; wire the raw
                          // "button-clicked" signal instead.
    toast.connect_local("button-clicked", false, |_| {
        let _ = gio::AppInfo::launch_default_for_uri(
            "https://github.com/DibbayajyotiRoy/fresco",
            None::<&gio::AppLaunchContext>,
        );
        None
    });
    state.borrow().toast.add_toast(toast);
}

// ─── Editor view ──────────────────────────────────────────────────────────────

fn build_editor_view(state: Rc<RefCell<AppState>>, stack: &gtk4::Stack) -> gtk4::Box {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let header = adw::HeaderBar::new();
    let title_widget = adw::WindowTitle::new(t!("Edit wallpaper"), "");
    header.set_title_widget(Some(&title_widget));
    let back = gtk4::Button::from_icon_name("go-previous-symbolic");
    back.add_css_class("flat");
    back.set_tooltip_text(Some(t!("Back to library")));
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

    let reset_crop = gtk4::Button::with_label(t!("Reset crop"));
    reset_crop.add_css_class("flat");
    {
        let ce = crop_editor.clone();
        reset_crop.connect_clicked(move |_| ce.reset());
    }
    let rotate_btn = gtk4::Button::with_label(t!("Rotate 90°"));
    rotate_btn.add_css_class("flat");
    {
        let ce = crop_editor.clone();
        rotate_btn.connect_clicked(move |_| ce.set_rotation((ce.rotation() + 90) % 360));
    }
    let edit_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    edit_actions.set_halign(gtk4::Align::End);
    edit_actions.set_margin_top(6);
    edit_actions.append(&rotate_btn);
    edit_actions.append(&reset_crop);
    preview_pane.append(&edit_actions);

    // Preferences group.
    let prefs = adw::PreferencesGroup::new();
    prefs.set_margin_top(14);

    let fit_row = adw::ComboRow::new();
    fit_row.set_title(t!("Fit"));
    fit_row.set_subtitle(t!("How the media fills the screen"));
    fit_row.set_model(Some(&gtk4::StringList::new(&[
        t!("Cover"),
        t!("Contain"),
        t!("Stretch"),
    ])));
    prefs.add(&fit_row);

    let mute_row = adw::ActionRow::new();
    mute_row.set_title(t!("Muted"));
    let mute_sw = gtk4::Switch::new();
    mute_sw.set_active(true);
    mute_sw.set_valign(gtk4::Align::Center);
    mute_row.add_suffix(&mute_sw);
    mute_row.set_activatable_widget(Some(&mute_sw));
    prefs.add(&mute_row);

    let vol_row = adw::ActionRow::new();
    vol_row.set_title(t!("Volume"));
    let vol_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 5.0);
    vol_scale.set_value(50.0);
    vol_scale.set_hexpand(true);
    vol_scale.set_size_request(180, -1);
    vol_scale.set_valign(gtk4::Align::Center);
    vol_row.add_suffix(&vol_scale);
    prefs.add(&vol_row);

    // Per-wallpaper power saving (video/playlist only). "Default" inherits the
    // global level from Settings; an explicit level overrides it for just this
    // wallpaper — e.g. keep one showpiece clip on Full.
    let power_row = adw::ComboRow::new();
    power_row.set_title(t!("Power saving"));
    power_row.set_subtitle(t!(
        "Default follows Settings; overrides it for this wallpaper"
    ));
    power_row.set_model(Some(&gtk4::StringList::new(
        &POWER_EDIT_LABELS.map(|l| t!(l)),
    )));
    prefs.add(&power_row);

    // Slideshow cadence (shown only for slideshows; see the on-enter handler).
    let interval_row = adw::ComboRow::new();
    interval_row.set_title(t!("Interval"));
    interval_row.set_subtitle(t!("How often the slideshow advances"));
    interval_row.set_model(Some(&gtk4::StringList::new(&[
        t!("5 seconds"),
        t!("15 seconds"),
        t!("30 seconds"),
        t!("1 minute"),
        t!("5 minutes"),
        t!("10 minutes"),
    ])));
    interval_row.set_selected(2);
    prefs.add(&interval_row);

    // Slideshow transition effect (shown only for slideshows).
    let transition_row = adw::ComboRow::new();
    transition_row.set_title(t!("Transition"));
    transition_row.set_subtitle(t!("Effect when the image changes"));
    transition_row.set_model(Some(&gtk4::StringList::new(&[
        t!("None"),
        t!("Crossfade"),
        t!("Fade to black"),
        t!("Ken Burns"),
    ])));
    transition_row.set_selected(1);
    prefs.add(&transition_row);

    controls.append(&prefs);

    // Playlist/slideshow item list (shown only for those kinds; see the
    // on-enter handler). The card context menu has the same entry — this is
    // the discoverable one, since the editor is where people already come to
    // change a wallpaper's settings.
    let items_btn = gtk4::Button::with_label(t!("Edit items…"));
    items_btn.add_css_class("pill");
    items_btn.set_margin_top(12);
    items_btn.set_visible(false);
    {
        let state_i = state.clone();
        items_btn.connect_clicked(move |btn| {
            let idx = state_i.borrow().editing_idx;
            let window = btn
                .root()
                .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok());
            let (Some(idx), Some(window)) = (idx, window) else {
                return;
            };
            show_items_dialog(&window, state_i.clone(), idx);
        });
    }
    controls.append(&items_btn);

    // Set Wallpaper button.
    let set_btn = gtk4::Button::with_label(t!("Set as wallpaper"));
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
        let power_ref = power_row.clone();
        set_btn.connect_clicked(move |_| {
            let crop = crop_ref.crop();
            let fit = match fit_ref.selected() {
                1 => Fit::Contain,
                2 => Fit::Stretch,
                _ => Fit::Cover,
            };
            let interval = interval_secs(interval_ref.selected());
            let transition = transition_from_index(transition_ref.selected());
            let power_saving = power_edit_from_index(power_ref.selected());
            let name = {
                let mut s = state_set.borrow_mut();
                s.config.wallpaper.crop = crop;
                s.config.wallpaper.rotation = crop_ref.rotation();
                s.config.wallpaper.fit = fit;
                s.config.wallpaper.mute = mute_ref.is_active();
                s.config.wallpaper.volume = vol_ref.value() as u8;
                s.config.wallpaper.power_saving = power_saving;
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
                    } else {
                        // Remember audio + orientation so a later gallery set (which
                        // rebuilds from the entry) keeps what was chosen here.
                        e.mute = Some(mute_ref.is_active());
                        e.volume = Some(vol_ref.value() as u8);
                        e.rotation = Some(crop_ref.rotation());
                        e.power_saving = power_saving;
                        // The card must show the new orientation immediately.
                        e.generate_thumbnail();
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
                    &tf!(
                        "“{name}” set. Close the window; it keeps playing",
                        "name" => name
                    ),
                );
                stack_set.set_visible_child_name("library");
            } else {
                show_toast(
                    &state_set,
                    t!("Couldn’t start the wallpaper. Run frescod --check"),
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
        let power_row_ref = power_row.clone();
        let title_ref = title_widget.clone();
        let crop_frame_ref = crop_frame.clone();
        let tp_frame_ref = tp_frame.clone();
        let edit_actions_ref = edit_actions.clone();
        let items_btn_ref = items_btn.clone();
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
                // Decode-skipping only matters for moving media.
                power_row_ref.set_visible(has_audio);
                items_btn_ref.set_visible(matches!(entry.kind, Kind::Playlist | Kind::Slideshow));
                let is_slideshow = entry.kind == Kind::Slideshow;
                interval_ref.set_visible(is_slideshow);
                transition_ref.set_visible(is_slideshow);
                // Slideshows preview the transition; other media show the crop tool.
                crop_frame_ref.set_visible(!is_slideshow);
                edit_actions_ref.set_visible(!is_slideshow);
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
            let ent = st.editing_idx.and_then(|i| st.entries.get(i));
            ce.set_crop(st.config.wallpaper.crop);
            ce.set_rotation(
                ent.and_then(|e| e.rotation)
                    .unwrap_or(st.config.wallpaper.rotation),
            );
            fit_ref.set_selected(match st.config.wallpaper.fit {
                Fit::Cover => 0,
                Fit::Contain => 1,
                Fit::Stretch => 2,
            });
            mute_ref.set_active(ent.and_then(|e| e.mute).unwrap_or(st.config.wallpaper.mute));
            vol_ref.set_value(
                ent.and_then(|e| e.volume)
                    .unwrap_or(st.config.wallpaper.volume) as f64,
            );
            power_row_ref.set_selected(power_edit_index(
                ent.and_then(|e| e.power_saving)
                    .or(st.config.wallpaper.power_saving),
            ));
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
    dialog.set_title(Some(t!("Advanced")));

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Video quality"));

    let scale_row = adw::ComboRow::new();
    scale_row.set_title(t!("Scaling quality"));
    scale_row.set_subtitle(t!("Balanced: low CPU  |  High: Lanczos resampling"));
    scale_row.set_model(Some(&gtk4::StringList::new(&[t!("Balanced"), t!("High")])));
    let current = u32::from(matches!(state.borrow().config.scaling, Scaling::High));
    scale_row.set_selected(current);
    {
        let state = state.clone();
        scale_row.connect_selected_notify(move |row| {
            let mut s = state.borrow_mut();
            s.config.scaling = if row.selected() == 1 {
                Scaling::High
            } else {
                Scaling::Balanced
            };
            s.config.save().ok();
        });
    }

    group.add(&scale_row);

    // Power saving: cheaper GPU scaling (see config::video_scalers) to cut
    // render load on weak hardware — a softer image for less power/heat.
    let power_row = adw::ComboRow::new();
    power_row.set_title(t!("Power saving"));
    power_row.set_subtitle(t!("Reduced saves most of the GPU cost; Full is sharpest"));
    power_row.set_model(Some(&gtk4::StringList::new(&POWER_LABELS.map(|l| t!(l)))));
    power_row.set_selected(power_index(state.borrow().config.power_saving));
    {
        let state = state.clone();
        power_row.connect_selected_notify(move |row| {
            let mut s = state.borrow_mut();
            s.config.power_saving = power_from_index(row.selected());
            s.config.save().ok();
            // Apply now (respawns renderers; the daemon stays up) so the change
            // takes effect immediately, not only on the next wallpaper set.
            daemon_ctl::ensure_daemon_and_apply(&s.config).ok();
        });
    }
    group.add(&power_row);

    page.add(&group);
    add_schedule_group(&page, state.clone());
    add_widget_appearance_group(&page, state.clone());
    add_lyrics_group(&page, state.clone());
    add_clock_group(&page, state.clone());
    add_visualizer_group(&page, state.clone());
    add_disc_group(&page, state);
    dialog.add(&page);
    dialog.present();
}

/// "Day & night wallpaper" preferences group (ROADMAP 3.3 GUI). v1 exposes
/// the daynight mode; times/solar stay config-file features (docs/SCRIPTING.md).
fn add_schedule_group(page: &adw::PreferencesPage, state: Rc<RefCell<AppState>>) {
    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Day &amp; night wallpaper"));
    group.set_description(Some(t!(
        "Automatically switch between two wallpapers on a schedule."
    )));

    // Candidate entries: playable single-media items from the library.
    let candidates: Vec<(usize, String)> = state
        .borrow()
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.broken && matches!(e.kind, Kind::Video | Kind::Image))
        .map(|(i, e)| (i, e.name.clone()))
        .collect();
    let names: Vec<&str> = candidates.iter().map(|(_, n)| n.as_str()).collect();

    let enable = adw::ComboRow::new();
    enable.set_title(t!("Schedule"));
    enable.set_model(Some(&gtk4::StringList::new(&[
        t!("Off"),
        t!("Day / night"),
    ])));

    let day_row = adw::ComboRow::new();
    day_row.set_title(t!("Day wallpaper"));
    day_row.set_model(Some(&gtk4::StringList::new(&names)));
    let night_row = adw::ComboRow::new();
    night_row.set_title(t!("Night wallpaper"));
    night_row.set_model(Some(&gtk4::StringList::new(&names)));

    let time_entry = |placeholder: &str| {
        let e = gtk4::Entry::new();
        e.set_placeholder_text(Some(placeholder));
        e.set_max_width_chars(6);
        e.set_valign(gtk4::Align::Center);
        e
    };
    let day_time = time_entry("07:00");
    let night_time = time_entry("19:00");
    let day_time_row = adw::ActionRow::new();
    day_time_row.set_title(t!("Day starts"));
    day_time_row.add_suffix(&day_time);
    let night_time_row = adw::ActionRow::new();
    night_time_row.set_title(t!("Night starts"));
    night_time_row.add_suffix(&night_time);

    // Populate from the current config.
    {
        let st = state.borrow();
        if let Some(sch) = st.config.schedule.as_ref() {
            enable.set_selected(1);
            day_time.set_text(&sch.day_start);
            night_time.set_text(&sch.night_start);
            let find = |w: Option<&crate::config::Wallpaper>| -> u32 {
                w.and_then(|w| w.path.as_ref())
                    .and_then(|p| {
                        candidates
                            .iter()
                            .position(|(i, _)| st.entries[*i].path.as_deref() == Some(p.as_path()))
                    })
                    .map(|i| i as u32)
                    .unwrap_or(0)
            };
            day_row.set_selected(find(sch.day.as_ref()));
            night_row.set_selected(find(sch.night.as_ref()));
        } else {
            day_time.set_text("07:00");
            night_time.set_text("19:00");
        }
    }

    let write = {
        let state = state.clone();
        let enable = enable.clone();
        let day_row = day_row.clone();
        let night_row = night_row.clone();
        let day_time = day_time.clone();
        let night_time = night_time.clone();
        let candidates = candidates.clone();
        move || {
            let on = enable.selected() == 1;
            let mut s = state.borrow_mut();
            if !on {
                if s.config.schedule.take().is_some() {
                    s.config.save().ok();
                    let _ = daemon_ctl::ensure_daemon_and_apply(&s.config);
                }
                return;
            }
            let (dt, nt) = (day_time.text().to_string(), night_time.text().to_string());
            if crate::schedule::parse_hhmm(&dt).is_none()
                || crate::schedule::parse_hhmm(&nt).is_none()
            {
                return; // incomplete/invalid times — wait for a valid edit
            }
            let pick = |row: &adw::ComboRow| -> Option<crate::config::Wallpaper> {
                candidates
                    .get(row.selected() as usize)
                    .and_then(|(i, _)| s.entries.get(*i))
                    .map(|e| e.to_wallpaper())
            };
            let (Some(day), Some(night)) = (pick(&day_row), pick(&night_row)) else {
                return;
            };
            s.config.schedule = Some(crate::config::Schedule {
                mode: crate::config::ScheduleMode::Daynight,
                day: Some(day),
                night: Some(night),
                day_start: dt,
                night_start: nt,
                lat: None,
                lon: None,
                at: vec![],
            });
            sync_wallpaper_to_schedule(&mut s.config);
            s.config.save().ok();
            let _ = daemon_ctl::ensure_daemon_and_apply(&s.config);
        }
    };

    let w = write.clone();
    enable.connect_selected_notify(move |_| w());
    let w = write.clone();
    day_row.connect_selected_notify(move |_| w());
    let w = write.clone();
    night_row.connect_selected_notify(move |_| w());
    let w = write.clone();
    day_time.connect_changed(move |_| w());
    let w = write;
    night_time.connect_changed(move |_| w());

    group.add(&enable);
    group.add(&day_row);
    group.add(&night_row);
    group.add(&day_time_row);
    group.add(&night_time_row);
    page.add(&group);
}

/// Point `config.wallpaper` at whatever the schedule wants RIGHT NOW, so
/// enabling/changing a schedule takes effect immediately (the daemon's
/// manual-Apply hold only protects wallpapers that differ from the schedule).
fn sync_wallpaper_to_schedule(cfg: &mut Config) {
    use chrono::Offset as _;
    let Some(sch) = cfg.schedule.as_ref() else {
        return;
    };
    let now = chrono::Local::now();
    let off = now.offset().fix().local_minus_utc() / 60;
    if let Some(w) = crate::schedule::desired(sch, now.naive_local(), off) {
        cfg.wallpaper = w.clone();
    }
}

// ─── Lyrics widget ────────────────────────────────────────────────────────────

/// Style presets in the order the "Style" combo lists them. The table is the
/// index map both ways, so the labels and the ordering live in one place.
const LYRIC_STYLES: [(LyricStylePreset, &str); 4] = [
    (LyricStylePreset::Minimal, "Minimal"),
    (LyricStylePreset::Karaoke, "Karaoke"),
    (LyricStylePreset::Subtitle, "Subtitle"),
    (LyricStylePreset::Card, "Card"),
];

/// The nine placement anchors with the names a person would use for them,
/// listed in reading order (top row first) so the combo matches the screen.
const LYRIC_ANCHORS: [(LyricAnchor, &str); 9] = [
    (LyricAnchor::TopLeft, "Top left"),
    (LyricAnchor::TopCenter, "Top center"),
    (LyricAnchor::TopRight, "Top right"),
    (LyricAnchor::MidLeft, "Middle left"),
    (LyricAnchor::MidCenter, "Middle center"),
    (LyricAnchor::MidRight, "Middle right"),
    (LyricAnchor::BottomLeft, "Bottom left"),
    (LyricAnchor::BottomCenter, "Bottom center"),
    (LyricAnchor::BottomRight, "Bottom right"),
];

/// Shown as the folder row's subtitle while no folder is set.
const LYRIC_FOLDER_HINT: &str = "Searched when no .lrc file sits beside the audio file.";

/// Subtitle of the lyric colour row while no colour has been chosen, i.e.
/// while the style preset is still picking one.
const LYRIC_COLOUR_HINT: &str =
    "Currently the colour the style picks. Used while “Follow accent colour” is off.";

/// What the lyric colour button shows before a colour has been chosen. The
/// fill three of the four presets use, and near enough for the fourth to be a
/// sensible place to start dragging from.
const LYRIC_PRESET_COLOUR: &str = "#FFFFFF";

/// Redraws the lyrics-folder row for a given folder, or for none. Shared by the
/// folder picker and the Clear button so the two cannot disagree about it.
type FolderDisplay = Rc<dyn Fn(Option<&std::path::Path>)>;

/// A list-rebuilding closure that the rows it builds call to redraw the list.
///
/// The cell is what breaks the cycle: the closure is constructed capturing an
/// empty slot, then stored into it, so a row's handler can reach the rebuild
/// that created the row.
type SelfRebuild = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// Position of `value` in a label table. The tables above cover every variant,
/// so the fallback is unreachable; it exists so adding a variant degrades to
/// "first entry selected" instead of a panic in a settings dialog.
fn table_index<T: Copy + PartialEq>(table: &[(T, &str)], value: T) -> u32 {
    table.iter().position(|(v, _)| *v == value).unwrap_or(0) as u32
}

/// The label column of a table, translated and ready for [`gtk4::StringList`].
///
/// Translation happens here rather than in each table's definition because the
/// tables are `const` data and the catalog is resolved at runtime — a `t!()`
/// inside a `const` would not compile. Every caller feeds the result straight
/// into a dropdown, so this is the single choke point for all of them.
fn table_labels<T>(table: &[(T, &'static str)]) -> Vec<&'static str> {
    table.iter().map(|(_, label)| t!(*label)).collect()
}

/// The lyric settings currently in force — the defaults when `config.widgets`
/// is absent, which is the normal state for anyone who has never opened this
/// group. Read-only: it takes a shared borrow and returns a copy, so callers
/// can populate widgets without holding a borrow across GTK calls.
fn lyrics_settings(state: &Rc<RefCell<AppState>>) -> Lyrics {
    state
        .borrow()
        .config
        .widgets
        .as_ref()
        .map(|w| w.lyrics.clone())
        .unwrap_or_default()
}

/// Apply one edit to the widget block, save, and push the result to the daemon
/// — the single mutation path behind every row in every widget group.
///
/// `Config::widgets` is `Option<Widgets>` so that a config written by someone
/// who never asked for a widget carries no `[widgets]` key at all. The first
/// edit here is what materialises the block; doing it in one helper keeps
/// `get_or_insert_with` out of twenty call sites.
///
/// The scoping is load-bearing, not style: `ensure_daemon_and_apply` re-reads
/// the config through a shared borrow, so the mutable borrow must be dropped
/// before it runs or the dialog panics on the first toggle. Every widget group
/// funnels through this one function so that discipline exists in one place
/// and cannot be got wrong twice.
fn edit_widgets(state: &Rc<RefCell<AppState>>, edit: impl FnOnce(&mut Widgets)) {
    {
        let mut s = state.borrow_mut();
        edit(s.config.widgets.get_or_insert_with(Widgets::default));
        s.config.save().ok();
    }
    let s = state.borrow();
    daemon_ctl::ensure_daemon_and_apply(&s.config).ok();
}

/// Apply one edit to the lyric settings — see [`edit_widgets`].
fn edit_lyrics(state: &Rc<RefCell<AppState>>, edit: impl FnOnce(&mut Lyrics)) {
    edit_widgets(state, |w| edit(&mut w.lyrics));
}

/// Apply one edit to the clock settings — see [`edit_widgets`].
fn edit_clock(state: &Rc<RefCell<AppState>>, edit: impl FnOnce(&mut Clock)) {
    edit_widgets(state, |w| edit(&mut w.clock));
}

/// An [`adw::ActionRow`] carrying a trailing switch. `apply` takes the new
/// state and routes it to whichever widget block owns the setting. Activating
/// the row anywhere flips the switch.
fn widget_switch_row(
    title: &str,
    subtitle: &str,
    active: bool,
    apply: impl Fn(bool) + 'static,
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    // Set before connecting: populating the dialog must not look like a user
    // edit and must not wake the daemon.
    sw.set_active(active);
    sw.connect_active_notify(move |sw| apply(sw.is_active()));
    row.add_suffix(&sw);
    row.set_activatable_widget(Some(&sw));
    row
}

/// An [`adw::ActionRow`] carrying a trailing spin button. `libadwaita` is
/// pinned to 1.1 for Debian (see `Cargo.toml`), which predates `AdwSpinRow`,
/// so numeric rows are assembled the same way the schedule group builds its
/// time rows.
fn widget_spin_row(
    title: &str,
    subtitle: &str,
    range: (f64, f64, f64),
    value: f64,
    apply: impl Fn(i32) + 'static,
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    let (min, max, step) = range;
    let spin = gtk4::SpinButton::with_range(min, max, step);
    spin.set_valign(gtk4::Align::Center);
    spin.set_numeric(true);
    spin.set_value(value);
    spin.connect_value_changed(move |spin| apply(spin.value_as_int()));
    // Deliberately not the row's activatable widget: activating a GtkSpinButton
    // re-parses and snaps its entry, so a stray click on the row would write a
    // value and wake the daemon.
    row.add_suffix(&spin);
    row
}

/// An [`adw::ActionRow`] carrying a trailing colour button.
///
/// `gtk4::ColorDialogButton` is the modern spelling and needs GTK 4.10;
/// `adw::ActionRow` + a control is what every other setting in this file uses
/// and works on the pinned GTK 4.6, so this is assembled the same way
/// [`widget_spin_row`] is. `GtkColorButton` is deprecated *in GTK 4.10*, which
/// is precisely why it is still the right choice here: nothing in the pinned
/// version is deprecated, and bumping the pin to avoid a widget would cost
/// Debian users the package.
///
/// `apply` receives a normalised `#RRGGBB`, never a `rgb()` string or an alpha
/// channel — the button is told not to offer one, because every colour this
/// dialog sets ends up in an ASS payload that has a separate opacity slider.
fn widget_colour_row(
    title: &str,
    subtitle: &str,
    hex: &str,
    apply: impl Fn(String) + 'static,
) -> (adw::ActionRow, gtk4::ColorButton) {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    let button = gtk4::ColorButton::new();
    button.set_valign(gtk4::Align::Center);
    button.set_use_alpha(false);
    button.set_title(title);
    // Set before connecting: populating the dialog must not look like a user
    // edit and must not wake the daemon.
    button.set_rgba(&hex_to_rgba(hex));
    button.connect_rgba_notify(move |b| apply(rgba_to_hex(&b.rgba())));
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    (row, button)
}

/// `#RRGGBB` → a GDK colour, falling back to opaque white on anything the
/// parser cannot read — the same fallback the renderers use, so a broken value
/// looks the same in the dialog as it does on the wallpaper.
fn hex_to_rgba(hex: &str) -> gtk4::gdk::RGBA {
    gtk4::gdk::RGBA::parse(hex).unwrap_or_else(|_| gtk4::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0))
}

/// A GDK colour as `#RRGGBB`.
///
/// Rounded to bytes here rather than anywhere downstream: the config file, the
/// ASS payload and the colour button must all agree on the same eight bits per
/// channel, or a colour picked once starts drifting every time the dialog is
/// reopened.
fn rgba_to_hex(c: &gtk4::gdk::RGBA) -> String {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", q(c.red()), q(c.green()), q(c.blue()))
}

/// [`widget_switch_row`] wired to the lyric block.
fn lyric_switch_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    active: bool,
    edit: impl Fn(&mut Lyrics, bool) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_switch_row(title, subtitle, active, move |on| {
        edit_lyrics(&state, |l| edit(l, on));
    })
}

/// [`widget_spin_row`] wired to the lyric block.
fn lyric_spin_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    range: (f64, f64, f64),
    value: f64,
    edit: impl Fn(&mut Lyrics, i32) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_spin_row(title, subtitle, range, value, move |v| {
        edit_lyrics(&state, |l| edit(l, v));
    })
}

// ─── Widget appearance ────────────────────────────────────────────────────────

/// The three palette choices in the order the combo lists them.
///
/// `Auto` leads because it is the default and the right answer for almost
/// everyone; `Light` and `Dark` follow as the manual override. See
/// [`config::WidgetTheme`] for why `Auto` resolves to dark rather than
/// following the app's own light/dark setting: a widget is drawn over the
/// user's wallpaper, not over the app, so the desktop theme says nothing
/// useful about what is behind it.
const WIDGET_THEMES: [(WidgetTheme, &str); 3] = [
    (WidgetTheme::Auto, "Automatic"),
    (WidgetTheme::Light, "Light"),
    (WidgetTheme::Dark, "Dark"),
];

/// The widget palette currently in force — the default when `config.widgets`
/// is absent, which is the normal state for anyone who has never opened these
/// groups. Read-only, like [`lyrics_settings`]: it takes a shared borrow and
/// returns a copy, so the caller is not holding one across a GTK call.
fn widget_theme(state: &Rc<RefCell<AppState>>) -> WidgetTheme {
    state
        .borrow()
        .config
        .widgets
        .as_ref()
        .map_or_else(WidgetTheme::default, |w| w.theme)
}

/// "Widget appearance" preferences group.
///
/// Sits above the four per-widget groups because it governs all of them: the
/// clock, lyrics, visualiser and disc are drawn from one palette, so offering
/// the choice four times would imply they can disagree. Unlike those groups
/// there is no master switch — a palette is not a feature to turn on, and the
/// row stays usable when every widget is off so the choice can be made before
/// the first one is enabled.
fn add_widget_appearance_group(page: &adw::PreferencesPage, state: Rc<RefCell<AppState>>) {
    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Widget appearance"));
    group.set_description(Some(t!(
        "How the clock, lyrics, visualiser and disc are drawn. The setting applies to all of them."
    )));

    let theme_row = adw::ComboRow::new();
    theme_row.set_title(t!("Palette"));
    theme_row.set_subtitle(t!(
        "Dark cards suit most wallpapers. Choose Light if yours are consistently bright."
    ));
    theme_row.set_model(Some(&gtk4::StringList::new(&table_labels(&WIDGET_THEMES))));
    theme_row.set_selected(table_index(&WIDGET_THEMES, widget_theme(&state)));
    {
        let state = state.clone();
        theme_row.connect_selected_notify(move |row| {
            let pick = WIDGET_THEMES
                .get(row.selected() as usize)
                .map_or_else(WidgetTheme::default, |(t, _)| *t);
            edit_widgets(&state, |w| w.theme = pick);
        });
    }
    group.add(&theme_row);

    page.add(&group);
}

/// "Lyrics" preferences group (WIDGETS_ROADMAP W1 GUI). Off by default, and
/// every row below the master switch is insensitive until it is on, so the
/// group reads as one feature rather than nine unrelated settings.
///
/// v1 is local `.lrc` files only — there is no network source and no key for
/// one, so nothing here promises lyrics the user has not supplied.
fn add_lyrics_group(page: &adw::PreferencesPage, state: Rc<RefCell<AppState>>) {
    let cur = lyrics_settings(&state);

    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Lyrics"));
    group.set_description(Some(t!(
        "Show the current line of the song that is playing on top of your wallpaper."
    )));

    let enable = adw::ActionRow::new();
    enable.set_title(t!("Show song lyrics"));
    enable.set_subtitle(
        "Reads .lrc files saved beside your music. Needs a media player that reports \
         what it is playing over MPRIS.",
    );
    let enable_switch = gtk4::Switch::new();
    enable_switch.set_valign(gtk4::Align::Center);
    enable_switch.set_active(cur.enabled);
    enable.add_suffix(&enable_switch);
    enable.set_activatable_widget(Some(&enable_switch));

    let style_row = adw::ComboRow::new();
    style_row.set_title(t!("Style"));
    style_row.set_subtitle(t!(
        "Minimal is one quiet line; Subtitle outlines the text for busy video"
    ));
    style_row.set_model(Some(&gtk4::StringList::new(&table_labels(&LYRIC_STYLES))));
    style_row.set_selected(table_index(&LYRIC_STYLES, cur.style));

    let anchor_row = adw::ComboRow::new();
    anchor_row.set_title(t!("Position"));
    anchor_row.set_subtitle(t!("Where the line sits on the screen"));
    anchor_row.set_model(Some(&gtk4::StringList::new(&table_labels(&LYRIC_ANCHORS))));
    anchor_row.set_selected(table_index(&LYRIC_ANCHORS, cur.anchor));

    let size_row = lyric_spin_row(
        &state,
        t!("Text size"),
        t!("In points. Larger text reads from further away and wraps sooner."),
        (12.0, 96.0, 1.0),
        f64::from(cur.font_size_pt),
        |l, v| l.font_size_pt = v.max(0) as u32,
    );

    let margin_row = lyric_spin_row(
        &state,
        t!("Margin"),
        t!("Pixels from the screen edge. Raise it to clear a panel, dock or rounded corner."),
        (0.0, 300.0, 1.0),
        f64::from(cur.margin_px),
        |l, v| l.margin_px = v.max(0) as u32,
    );

    let offset_row = lyric_spin_row(
        &state,
        t!("Sync offset"),
        "Milliseconds added to every timestamp. .lrc files are hand-timed, so lines can \
         run early or late; a positive value shows each line later.",
        (-5000.0, 5000.0, 50.0),
        f64::from(cur.offset_ms),
        |l, v| l.offset_ms = v,
    );

    // ── Colour ──
    // The switch and the picker below it are one decision in two rows, so the
    // picker goes insensitive while the switch is on rather than sitting there
    // looking editable and doing nothing.
    let (colour_row, colour_btn) = {
        let state = state.clone();
        widget_colour_row(
            t!("Lyric colour"),
            t!(LYRIC_COLOUR_HINT),
            cur.colour.as_deref().unwrap_or(LYRIC_PRESET_COLOUR),
            move |hex| edit_lyrics(&state, |l| l.colour = Some(hex.clone())),
        )
    };
    // Clearing hands the colour back to the style preset — Karaoke's amber,
    // Card's ink — which is a different thing from any colour the picker can
    // produce, so it needs its own control.
    let colour_clear = gtk4::Button::from_icon_name("edit-clear-symbolic");
    colour_clear.add_css_class("flat");
    colour_clear.set_valign(gtk4::Align::Center);
    colour_clear.set_tooltip_text(Some(t!("Use the colour the style picks")));
    colour_clear.set_visible(cur.colour.is_some());
    colour_row.add_prefix(&colour_clear);
    {
        let state = state.clone();
        let colour_btn = colour_btn.clone();
        let colour_row = colour_row.clone();
        colour_clear.connect_clicked(move |btn| {
            edit_lyrics(&state, |l| l.colour = None);
            btn.set_visible(false);
            colour_row.set_subtitle(t!(LYRIC_COLOUR_HINT));
            colour_btn.set_rgba(&hex_to_rgba(LYRIC_PRESET_COLOUR));
        });
    }
    {
        let colour_clear = colour_clear.clone();
        let colour_row = colour_row.clone();
        colour_btn.connect_rgba_notify(move |_| {
            colour_clear.set_visible(true);
            colour_row.set_subtitle(t!("Used while “Follow accent colour” is off."));
        });
    }

    let accent_row = {
        let state = state.clone();
        let colour_row = colour_row.clone();
        widget_switch_row(
            t!("Follow accent colour"),
            t!("Tint the lyric with the app accent instead of the colour below."),
            cur.accent_follow,
            move |on| {
                edit_lyrics(&state, |l| l.accent_follow = on);
                // Only meaningful while the group itself is on; the master
                // switch's own handler re-applies this rule.
                colour_row.set_sensitive(!on && lyrics_settings(&state).enabled);
            },
        )
    };

    let next_row = lyric_switch_row(
        &state,
        t!("Show next line"),
        t!("Also show the upcoming line, dimmed. Covers twice as much of the desktop."),
        cur.show_next_line,
        |l, on| l.show_next_line = on,
    );

    // Beside "Show next line" because it is the same kind of choice: how much
    // else to put on the wallpaper alongside the lyric itself.
    let track_info_row = lyric_switch_row(
        &state,
        t!("Show track title and artist"),
        "Adds the song title and artist above the lyric line. Stays on screen between \
         songs, where the lyric line does not.",
        cur.show_track_info,
        |l, on| l.show_track_info = on,
    );

    // ── Lyrics folder ──
    let folder_row = adw::ActionRow::new();
    folder_row.set_title(t!("Lyrics folder"));
    let choose_btn = gtk4::Button::with_label(t!("Choose…"));
    choose_btn.set_valign(gtk4::Align::Center);
    let clear_btn = gtk4::Button::from_icon_name("edit-clear-symbolic");
    clear_btn.add_css_class("flat");
    clear_btn.set_valign(gtk4::Align::Center);
    clear_btn.set_tooltip_text(Some(t!("Use only .lrc files beside the music")));
    folder_row.add_suffix(&clear_btn);
    folder_row.add_suffix(&choose_btn);

    // One place decides what the row says and whether Clear is offered.
    let show_folder: FolderDisplay = {
        let folder_row = folder_row.clone();
        let clear_btn = clear_btn.clone();
        Rc::new(move |folder: Option<&std::path::Path>| match folder {
            // Escaped: Adwaita row subtitles are Pango markup, and a path may
            // legitimately contain '&' or '<'.
            Some(p) => {
                folder_row.set_subtitle(&glib::markup_escape_text(&p.display().to_string()));
                clear_btn.set_visible(true);
            }
            None => {
                folder_row.set_subtitle(t!(LYRIC_FOLDER_HINT));
                clear_btn.set_visible(false);
            }
        })
    };
    show_folder(cur.folder.as_deref());

    {
        let state = state.clone();
        let show_folder = show_folder.clone();
        choose_btn.connect_clicked(move |btn| {
            // The chooser needs the dialog it was opened from as its parent;
            // take it from the widget tree rather than threading a window
            // argument through every preferences helper.
            let parent = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
            let chooser = gtk4::FileChooserNative::new(
                Some(t!("Choose Lyrics Folder")),
                parent.as_ref(),
                FileChooserAction::SelectFolder,
                Some(t!("Select")),
                Some(t!("Cancel")),
            );
            let state_cb = state.clone();
            let show_folder = show_folder.clone();
            chooser.connect_response(move |ch, resp| {
                state_cb.borrow_mut().current_picker = None;
                if resp != ResponseType::Accept {
                    return;
                }
                let Some(folder) = ch.file().and_then(|f| f.path()) else {
                    return;
                };
                edit_lyrics(&state_cb, |l| l.folder = Some(folder.clone()));
                show_folder(Some(folder.as_path()));
            });
            // Same lifetime trap as the other pickers in this file: a local
            // FileChooserNative is dropped before the portal replies.
            state.borrow_mut().current_picker = Some(chooser.clone());
            chooser.show();
        });
    }

    {
        let state = state.clone();
        let show_folder = show_folder.clone();
        clear_btn.connect_clicked(move |_| {
            edit_lyrics(&state, |l| l.folder = None);
            show_folder(None);
        });
    }

    // Combos last: set_selected above would otherwise fire these while the
    // dialog is still being built.
    {
        let state = state.clone();
        style_row.connect_selected_notify(move |row| {
            let Some((style, _)) = LYRIC_STYLES.get(row.selected() as usize).copied() else {
                return;
            };
            edit_lyrics(&state, |l| l.style = style);
        });
    }
    {
        let state = state.clone();
        anchor_row.connect_selected_notify(move |row| {
            let Some((anchor, _)) = LYRIC_ANCHORS.get(row.selected() as usize).copied() else {
                return;
            };
            edit_lyrics(&state, |l| l.anchor = anchor);
        });
    }

    let dependents: Vec<gtk4::Widget> = vec![
        style_row.clone().upcast(),
        anchor_row.clone().upcast(),
        size_row.clone().upcast(),
        margin_row.clone().upcast(),
        offset_row.clone().upcast(),
        accent_row.clone().upcast(),
        next_row.clone().upcast(),
        track_info_row.clone().upcast(),
        folder_row.clone().upcast(),
    ];
    let sync_sensitive = {
        let colour_row = colour_row.clone();
        let state = state.clone();
        move |on: bool| {
            for row in &dependents {
                row.set_sensitive(on);
            }
            // The colour is only in force when the accent is not, so it carries
            // the group's switch *and* the accent switch.
            colour_row.set_sensitive(on && !lyrics_settings(&state).accent_follow);
        }
    };
    sync_sensitive(cur.enabled);
    {
        let state = state.clone();
        enable_switch.connect_active_notify(move |sw| {
            let on = sw.is_active();
            sync_sensitive(on);
            edit_lyrics(&state, |l| l.enabled = on);
        });
    }

    group.add(&enable);
    group.add(&style_row);
    group.add(&anchor_row);
    group.add(&size_row);
    group.add(&margin_row);
    group.add(&offset_row);
    group.add(&accent_row);
    group.add(&colour_row);
    group.add(&next_row);
    group.add(&track_info_row);
    group.add(&folder_row);
    page.add(&group);
}

// ─── Clock widget ─────────────────────────────────────────────────────────────

/// Clock themes in the order the "Theme" combo lists them. The labels are the
/// ones `clock::ClockTheme::label` returns and the order is `ClockTheme::ALL`'s
/// — `clock_theme_labels_match_the_renderer` holds the two together, so the
/// picker cannot start naming a look the renderer spells differently.
/// The themes the picker actually offers.
///
/// A subset of [`CLOCK_THEMES`], not a replacement for it: that table stays
/// complete so `clock_theme_labels_match_the_renderer` can keep checking the
/// GUI's words against the renderer's, which is the guard that catches a theme
/// being renamed in one place and not the other. This list is only about what
/// is *shown*, and re-adding a look is a line here rather than a re-wiring.
const CLOCK_THEMES_SHOWN: [(ClockThemeCfg, &str); 1] = [(ClockThemeCfg::Nos, "NOS")];

/// Only the tests read this now that the picker is driven by
/// [`CLOCK_THEMES_SHOWN`]; it is kept because
/// `clock_theme_labels_match_the_renderer` is the guard that catches a theme
/// renamed in `clock.rs` and not here, and that guard has to see all seven.
#[cfg(test)]
const CLOCK_THEMES: [(ClockThemeCfg, &str); 7] = [
    (ClockThemeCfg::Digital, "Digital"),
    (ClockThemeCfg::Minimal, "Minimal"),
    (ClockThemeCfg::Segment, "Segment"),
    (ClockThemeCfg::Stacked, "Stacked"),
    (ClockThemeCfg::Wordy, "Wordy"),
    (ClockThemeCfg::Card, "Card"),
    (ClockThemeCfg::Nos, "NOS"),
];

/// The clock settings currently in force — the defaults when `config.widgets`
/// is absent, which is the normal state for anyone who has never opened this
/// group. Read-only: it takes a shared borrow and returns a copy, so callers
/// can populate widgets without holding a borrow across GTK calls.
fn clock_settings(state: &Rc<RefCell<AppState>>) -> Clock {
    state
        .borrow()
        .config
        .widgets
        .as_ref()
        .map(|w| w.clock.clone())
        .unwrap_or_default()
}

/// [`widget_switch_row`] wired to the clock block.
fn clock_switch_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    active: bool,
    edit: impl Fn(&mut Clock, bool) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_switch_row(title, subtitle, active, move |on| {
        edit_clock(&state, |c| edit(c, on));
    })
}

/// [`widget_spin_row`] wired to the clock block.
fn clock_spin_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    range: (f64, f64, f64),
    value: f64,
    edit: impl Fn(&mut Clock, i32) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_spin_row(title, subtitle, range, value, move |v| {
        edit_clock(&state, |c| edit(c, v));
    })
}

/// "Clock" preferences group (WIDGETS_ROADMAP W1 GUI). Built exactly like
/// [`add_lyrics_group`]: off by default, and every row below the master switch
/// is insensitive until it is on, so the group reads as one feature rather than
/// nine unrelated settings.
///
/// Two subtitles here state a cost rather than describing a control, and both
/// are deliberate. "Show seconds" is the only switch in the dialog that changes
/// how often Fresco wakes the machine, so it says so in the row instead of
/// hiding it in the docs. "Show date" is overruled by two themes, so it says
/// that too — a switch that silently does nothing reads as a bug.
fn add_clock_group(page: &adw::PreferencesPage, state: Rc<RefCell<AppState>>) {
    let cur = clock_settings(&state);

    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Clock"));
    group.set_description(Some(t!(
        "Show the time on top of your wallpaper, behind your windows."
    )));

    let enable = adw::ActionRow::new();
    enable.set_title(t!("Show a clock"));
    enable.set_subtitle(
        "Draws the current time on top of the wallpaper. Needs nothing playing and no \
         internet connection.",
    );
    let enable_switch = gtk4::Switch::new();
    enable_switch.set_valign(gtk4::Align::Center);
    enable_switch.set_active(cur.enabled);
    enable.add_suffix(&enable_switch);
    enable.set_activatable_widget(Some(&enable_switch));

    let theme_row = adw::ComboRow::new();
    theme_row.set_title(t!("Theme"));
    theme_row.set_subtitle(t!(
        "Digital reads at a glance; Wordy spells the time out, like \"half past ten\""
    ));
    theme_row.set_model(Some(&gtk4::StringList::new(&table_labels(
        &CLOCK_THEMES_SHOWN,
    ))));
    theme_row.set_selected(table_index(&CLOCK_THEMES_SHOWN, cur.theme));

    // The same nine anchors, with the same names, as the lyric overlay: one
    // placement vocabulary for every widget.
    let anchor_row = adw::ComboRow::new();
    anchor_row.set_title(t!("Position"));
    anchor_row.set_subtitle(t!("Where the clock sits on the screen"));
    anchor_row.set_model(Some(&gtk4::StringList::new(&table_labels(&LYRIC_ANCHORS))));
    anchor_row.set_selected(table_index(&LYRIC_ANCHORS, cur.anchor));

    let size_row = clock_spin_row(
        &state,
        t!("Text size"),
        t!("In points at 1080p, and scaled with the screen. Each theme sizes itself around it."),
        (12.0, 200.0, 1.0),
        f64::from(cur.font_size_pt),
        |c, v| c.font_size_pt = v.max(0) as u32,
    );

    let margin_row = clock_spin_row(
        &state,
        t!("Margin"),
        t!("Pixels from the screen edge. Raise it to clear a panel, dock or rounded corner."),
        (0.0, 300.0, 1.0),
        f64::from(cur.margin_px),
        |c, v| c.margin_px = v.max(0) as u32,
    );

    let hour_row = clock_switch_row(
        &state,
        t!("24-hour time"),
        "Show 13:00 rather than 1:00 PM. Fixed width, so a clock in a corner does not \
         shift as the hour changes.",
        cur.use_24h,
        |c, on| c.use_24h = on,
    );

    let date_row = clock_switch_row(
        &state,
        t!("Show date"),
        "Adds the date under the time. Minimal never shows one and Stacked always does, \
         whatever this says.",
        cur.show_date,
        |c, on| c.show_date = on,
    );

    let seconds_row = clock_switch_row(
        &state,
        t!("Show seconds"),
        "Uses more power: the clock is redrawn every second instead of once a minute. \
         Wordy ignores it.",
        cur.show_seconds,
        |c, on| c.show_seconds = on,
    );

    let accent_row = clock_switch_row(
        &state,
        t!("Follow accent colour"),
        t!("Draw the clock in the app accent colour instead of plain white."),
        cur.accent_follow,
        |c, on| c.accent_follow = on,
    );

    // Combos last: set_selected above would otherwise fire these while the
    // dialog is still being built.
    {
        let state = state.clone();
        theme_row.connect_selected_notify(move |row| {
            let Some((theme, _)) = CLOCK_THEMES_SHOWN.get(row.selected() as usize).copied() else {
                return;
            };
            edit_clock(&state, |c| c.theme = theme);
        });
    }
    {
        let state = state.clone();
        anchor_row.connect_selected_notify(move |row| {
            let Some((anchor, _)) = LYRIC_ANCHORS.get(row.selected() as usize).copied() else {
                return;
            };
            edit_clock(&state, |c| c.anchor = anchor);
        });
    }

    let dependents: Vec<gtk4::Widget> = vec![
        theme_row.clone().upcast(),
        anchor_row.clone().upcast(),
        size_row.clone().upcast(),
        margin_row.clone().upcast(),
        hour_row.clone().upcast(),
        date_row.clone().upcast(),
        seconds_row.clone().upcast(),
        accent_row.clone().upcast(),
    ];
    let sync_sensitive = move |on: bool| {
        for row in &dependents {
            row.set_sensitive(on);
        }
    };
    sync_sensitive(cur.enabled);
    {
        let state = state.clone();
        enable_switch.connect_active_notify(move |sw| {
            let on = sw.is_active();
            sync_sensitive(on);
            edit_clock(&state, |c| c.enabled = on);
        });
    }

    group.add(&enable);
    group.add(&theme_row);
    group.add(&anchor_row);
    group.add(&size_row);
    group.add(&margin_row);
    group.add(&hour_row);
    group.add(&date_row);
    group.add(&seconds_row);
    group.add(&accent_row);
    page.add(&group);
}

// ─── Visualiser widget ────────────────────────────────────────────────────────

/// Visualiser styles in the order the "Style" combo lists them, matching
/// `visualizer::VisualStyle::ALL`. The table is the index map both ways, so the
/// labels and the ordering live in one place.
const VISUAL_STYLES: [(VisualizerStyleCfg, &str); 5] = [
    (VisualizerStyleCfg::Bars, "Bars"),
    (VisualizerStyleCfg::Mirror, "Mirror"),
    (VisualizerStyleCfg::Wave, "Wave"),
    (VisualizerStyleCfg::Dots, "Dots"),
    (VisualizerStyleCfg::Ring, "Ring"),
];

/// Gradient modes in the order the "Colour blend" combo lists them, matching
/// `config::GradientMode`. Named for what they do rather than for how they are
/// implemented: "Linear" is a graphics term, and the choice being made here is
/// between one colour, two, and a rainbow.
const VISUAL_GRADIENTS: [(GradientMode, &str); 3] = [
    (GradientMode::None, "Single colour"),
    (GradientMode::Linear, "Blend two colours"),
    (GradientMode::Spectrum, "Rainbow"),
];

/// The visualiser settings currently in force — the defaults when
/// `config.widgets` is absent, which is the normal state for anyone who has
/// never opened this group. Read-only: it takes a shared borrow and returns a
/// copy, so callers can populate widgets without holding a borrow across GTK
/// calls.
fn visualizer_settings(state: &Rc<RefCell<AppState>>) -> Visualizer {
    state
        .borrow()
        .config
        .widgets
        .as_ref()
        .map(|w| w.visualizer.clone())
        .unwrap_or_default()
}

/// Apply one edit to the visualiser settings — see [`edit_widgets`].
fn edit_visualizer(state: &Rc<RefCell<AppState>>, edit: impl FnOnce(&mut Visualizer)) {
    edit_widgets(state, |w| edit(&mut w.visualizer));
}

/// [`widget_switch_row`] wired to the visualiser block.
fn visualizer_switch_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    active: bool,
    edit: impl Fn(&mut Visualizer, bool) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_switch_row(title, subtitle, active, move |on| {
        edit_visualizer(&state, |v| edit(v, on));
    })
}

/// [`widget_spin_row`] wired to the visualiser block.
fn visualizer_spin_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    range: (f64, f64, f64),
    value: f64,
    edit: impl Fn(&mut Visualizer, i32) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_spin_row(title, subtitle, range, value, move |v| {
        edit_visualizer(&state, |vis| edit(vis, v));
    })
}

/// "Audio visualiser" preferences group (WIDGETS_ROADMAP W2 GUI). Built exactly
/// like [`add_lyrics_group`] and [`add_clock_group`]: off by default, and every
/// row below the master switch is insensitive until it is on.
///
/// The master switch's subtitle says, in the row and not in the docs, that this
/// widget listens to the computer's audio output. That is the one thing a
/// person must know *before* flipping it: no other widget in this dialog opens
/// a capture device, and finding out afterwards — from a desktop recording
/// indicator, say — would be a nasty surprise. It also names the two sound
/// servers that can provide it, because on a machine with neither the switch
/// would otherwise just appear not to work.
fn add_visualizer_group(page: &adw::PreferencesPage, state: Rc<RefCell<AppState>>) {
    let cur = visualizer_settings(&state);

    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Audio visualiser"));
    group.set_description(Some(t!(
        "Show a moving spectrum of whatever is playing on top of your wallpaper."
    )));

    let enable = adw::ActionRow::new();
    enable.set_title(t!("Show audio visualiser"));
    enable.set_subtitle(
        "Listens to your computer's audio output so the bars can react to the music. \
         Needs PipeWire or PulseAudio. Uses more power than the other widgets, because \
         it is redrawn continuously while sound is playing.",
    );
    let enable_switch = gtk4::Switch::new();
    enable_switch.set_valign(gtk4::Align::Center);
    enable_switch.set_active(cur.enabled);
    enable.add_suffix(&enable_switch);
    enable.set_activatable_widget(Some(&enable_switch));

    let style_row = adw::ComboRow::new();
    style_row.set_title(t!("Style"));
    style_row.set_subtitle(t!(
        "Bars is the classic spectrum; Ring lays the same bars out in a circle"
    ));
    style_row.set_model(Some(&gtk4::StringList::new(&table_labels(&VISUAL_STYLES))));
    style_row.set_selected(table_index(&VISUAL_STYLES, cur.style));

    // The same nine anchors, with the same names, as every other widget.
    let anchor_row = adw::ComboRow::new();
    anchor_row.set_title(t!("Position"));
    anchor_row.set_subtitle(t!("Where the visualiser sits on the screen"));
    anchor_row.set_model(Some(&gtk4::StringList::new(&table_labels(&LYRIC_ANCHORS))));
    anchor_row.set_selected(table_index(&LYRIC_ANCHORS, cur.anchor));

    let width_row = visualizer_spin_row(
        &state,
        t!("Width"),
        "Percentage of the screen width the visualiser spans. Stays right when the \
         resolution changes.",
        (5.0, 100.0, 5.0),
        f64::from(cur.width_pct),
        |v, n| v.width_pct = n.clamp(0, 100) as u32,
    );

    let height_row = visualizer_spin_row(
        &state,
        t!("Height"),
        t!("How tall the bars can grow, in pixels at 1080p and scaled with the screen."),
        (16.0, 600.0, 8.0),
        f64::from(cur.height_px),
        |v, n| v.height_px = n.max(0) as u32,
    );

    let margin_row = visualizer_spin_row(
        &state,
        t!("Margin"),
        t!("Pixels from the screen edge. Raise it to clear a panel, dock or rounded corner."),
        (0.0, 300.0, 1.0),
        f64::from(cur.margin_px),
        |v, n| v.margin_px = n.max(0) as u32,
    );

    let bands_row = visualizer_spin_row(
        &state,
        t!("Bands"),
        "How many bars the sound is split into. Past a couple of hundred they are \
         thinner than the gaps between them and read as noise.",
        (4.0, 192.0, 1.0),
        f64::from(cur.bands),
        |v, n| v.bands = n.max(1) as u32,
    );

    let rounded_row = visualizer_switch_row(
        &state,
        t!("Rounded shapes"),
        t!("Round the bar caps, dots and wave. Off gives the same layout with hard edges."),
        cur.rounded,
        |v, on| v.rounded = on,
    );

    // ── Colour ──
    // Four rows that are really one decision, so they explain each other by
    // going insensitive rather than by being documented somewhere else: the
    // accent switch overrides the first colour, and the blend mode decides
    // whether the second colour means anything at all.
    let gradient_row = adw::ComboRow::new();
    gradient_row.set_title(t!("Colour blend"));
    gradient_row.set_subtitle(
        "Rainbow sweeps the whole spectrum and needs no colours of its own. \
         Blending steps the colour along the bars — the wave style is a single \
         shape and always draws in one colour.",
    );
    gradient_row.set_model(Some(&gtk4::StringList::new(&table_labels(
        &VISUAL_GRADIENTS,
    ))));
    gradient_row.set_selected(table_index(&VISUAL_GRADIENTS, cur.gradient));

    let (colour_row, _) = {
        let state = state.clone();
        widget_colour_row(
            t!("Colour"),
            t!("The colour of the bars, or the near end of a blend."),
            &cur.colour,
            move |hex| edit_visualizer(&state, |v| v.colour = hex.clone()),
        )
    };
    let (colour_end_row, _) = {
        let state = state.clone();
        widget_colour_row(
            t!("Blend to"),
            t!("The far end of the blend — the colour the last bar is drawn in."),
            &cur.colour_end,
            move |hex| edit_visualizer(&state, |v| v.colour_end = hex.clone()),
        )
    };

    // The three switches that decide which colour rows mean anything, tracked
    // here because a GTK widget cannot be asked "would you be sensitive if your
    // group were on".
    let group_on = Rc::new(Cell::new(cur.enabled));
    let accent_on = Rc::new(Cell::new(cur.accent_follow));
    let blend = Rc::new(Cell::new(cur.gradient));
    let sync_colours: Rc<dyn Fn()> = {
        let (colour_row, colour_end_row) = (colour_row.clone(), colour_end_row.clone());
        let (group_on, accent_on, blend) = (group_on.clone(), accent_on.clone(), blend.clone());
        Rc::new(move || {
            let on = group_on.get();
            // Ignored while the accent is in force, and by Rainbow, which picks
            // its own colours from end to end.
            colour_row
                .set_sensitive(on && !accent_on.get() && blend.get() != GradientMode::Spectrum);
            colour_end_row.set_sensitive(on && blend.get() == GradientMode::Linear);
        })
    };

    let accent_row = {
        let state = state.clone();
        let sync_colours = sync_colours.clone();
        let accent_on = accent_on.clone();
        widget_switch_row(
            t!("Follow accent colour"),
            t!("Draw the spectrum in the app accent colour instead of the colour below."),
            cur.accent_follow,
            move |on| {
                accent_on.set(on);
                edit_visualizer(&state, |v| v.accent_follow = on);
                sync_colours();
            },
        )
    };

    // Combos last: set_selected above would otherwise fire these while the
    // dialog is still being built.
    {
        let state = state.clone();
        style_row.connect_selected_notify(move |row| {
            let Some((style, _)) = VISUAL_STYLES.get(row.selected() as usize).copied() else {
                return;
            };
            edit_visualizer(&state, |v| v.style = style);
        });
    }
    {
        let state = state.clone();
        anchor_row.connect_selected_notify(move |row| {
            let Some((anchor, _)) = LYRIC_ANCHORS.get(row.selected() as usize).copied() else {
                return;
            };
            edit_visualizer(&state, |v| v.anchor = anchor);
        });
    }
    {
        let state = state.clone();
        let sync_colours = sync_colours.clone();
        let blend = blend.clone();
        gradient_row.connect_selected_notify(move |row| {
            let Some((mode, _)) = VISUAL_GRADIENTS.get(row.selected() as usize).copied() else {
                return;
            };
            blend.set(mode);
            edit_visualizer(&state, |v| v.gradient = mode);
            sync_colours();
        });
    }

    let dependents: Vec<gtk4::Widget> = vec![
        style_row.clone().upcast(),
        anchor_row.clone().upcast(),
        width_row.clone().upcast(),
        height_row.clone().upcast(),
        margin_row.clone().upcast(),
        bands_row.clone().upcast(),
        rounded_row.clone().upcast(),
        accent_row.clone().upcast(),
        gradient_row.clone().upcast(),
    ];
    let sync_sensitive = {
        let sync_colours = sync_colours.clone();
        let group_on = group_on.clone();
        move |on: bool| {
            group_on.set(on);
            for row in &dependents {
                row.set_sensitive(on);
            }
            sync_colours();
        }
    };
    sync_sensitive(cur.enabled);
    {
        let state = state.clone();
        enable_switch.connect_active_notify(move |sw| {
            let on = sw.is_active();
            // The one switch in this dialog that opens a capture device. It may
            // not do so before the question has been asked and answered — see
            // `show_audio_consent_dialog`, and `Config::audio_capture_consented`
            // for why the check is duplicated where the config is loaded.
            if on && !state.borrow().config.audio_capture_consented {
                // Back off until the question is answered: a switch that reads
                // "on" while nothing is capturing is a lie either way round.
                sw.set_active(false);
                let switch = sw.clone();
                show_audio_consent_dialog(sw, state.clone(), move |agreed| {
                    if agreed {
                        // Re-enters this handler, now with consent on file.
                        switch.set_active(true);
                    }
                });
                return;
            }
            sync_sensitive(on);
            edit_visualizer(&state, |v| v.enabled = on);
        });
    }

    group.add(&enable);
    group.add(&style_row);
    group.add(&anchor_row);
    group.add(&width_row);
    group.add(&height_row);
    group.add(&margin_row);
    group.add(&bands_row);
    group.add(&rounded_row);
    group.add(&accent_row);
    group.add(&gradient_row);
    group.add(&colour_row);
    group.add(&colour_end_row);
    page.add(&group);
}

/// One-time audio-capture consent, asked the first time the visualiser is
/// switched on and never again.
///
/// Modelled on [`show_telemetry_consent_dialog`] deliberately, down to both
/// buttons carrying equal weight: this is the same kind of promise, and the
/// visualiser's is the larger one. It is asked *here*, at the switch, rather
/// than at startup — consent for a feature nobody has asked for yet is noise,
/// and noise is what teaches people to click through consent dialogs.
///
/// `answer(false)` for Cancel and for closing the window, so the only way to
/// start a capture is to have read the dialog and pressed the button on it.
/// The flag is written before the callback runs, so the caller may simply flip
/// the switch back on and let the normal path take over.
fn show_audio_consent_dialog(
    anchor: &gtk4::Switch,
    state: Rc<RefCell<AppState>>,
    answer: impl Fn(bool) + 'static,
) {
    let dialog = adw::Window::new();
    dialog.add_css_class("glass");
    dialog.set_transient_for(
        anchor
            .root()
            .and_then(|r| r.downcast::<gtk4::Window>().ok())
            .as_ref(),
    );
    dialog.set_modal(true);
    dialog.set_title(Some(t!("Let the visualiser listen?")));
    dialog.set_default_size(460, -1);
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&adw::HeaderBar::new());
    dialog.set_content(Some(&content));

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    let body = gtk4::Label::new(Some(
        "The audio visualiser reads your computer's sound output so the bars can \
         react to what is playing.\n\n\
         The sound is analysed on this computer and never leaves it. Nothing is \
         recorded, saved or sent anywhere — the samples become bar heights and are \
         thrown away.\n\n\
         It captures everything your speakers play, not only your music player, and \
         some desktops show a recording indicator while it runs. You can turn it off \
         again at any time from this switch.",
    ));
    body.set_wrap(true);
    body.set_xalign(0.0);
    inner.append(&body);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    buttons.set_margin_top(6);
    let cancel = gtk4::Button::with_label(t!("Cancel"));
    let accept = gtk4::Button::with_label(t!("Enable visualiser"));
    accept.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&accept);
    inner.append(&buttons);

    // Shared so that closing the window counts as Cancel exactly once: the
    // close-request fires on the button path too, and answering twice would
    // flip the switch back and forth.
    let answered = Rc::new(Cell::new(false));
    let answer = Rc::new(answer);
    let reply = {
        let dialog = dialog.clone();
        let answered = answered.clone();
        let answer = answer.clone();
        move |agreed: bool| {
            if answered.replace(true) {
                return;
            }
            if agreed {
                let mut s = state.borrow_mut();
                s.config.audio_capture_consented = true;
                s.config.save().ok();
            }
            dialog.close();
            answer(agreed);
        }
    };
    {
        let reply = reply.clone();
        cancel.connect_clicked(move |_| reply(false));
    }
    {
        let reply = reply.clone();
        accept.connect_clicked(move |_| reply(true));
    }
    dialog.connect_close_request(move |_| {
        reply(false);
        glib::Propagation::Proceed
    });

    content.append(&inner);
    dialog.present();
}

// ─── Album art widget ─────────────────────────────────────────────────────────

/// The disc settings currently in force — the defaults when `config.widgets` is
/// absent, which is the normal state for anyone who has never opened this
/// group. Read-only: it takes a shared borrow and returns a copy, so callers can
/// populate widgets without holding a borrow across GTK calls.
fn disc_settings(state: &Rc<RefCell<AppState>>) -> Disc {
    state
        .borrow()
        .config
        .widgets
        .as_ref()
        .map(|w| w.disc.clone())
        .unwrap_or_default()
}

/// Apply one edit to the disc settings — see [`edit_widgets`].
fn edit_disc(state: &Rc<RefCell<AppState>>, edit: impl FnOnce(&mut Disc)) {
    edit_widgets(state, |w| edit(&mut w.disc));
}

/// [`widget_switch_row`] wired to the disc block.
fn disc_switch_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    active: bool,
    edit: impl Fn(&mut Disc, bool) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_switch_row(title, subtitle, active, move |on| {
        edit_disc(&state, |d| edit(d, on));
    })
}

/// [`widget_spin_row`] wired to the disc block.
fn disc_spin_row(
    state: &Rc<RefCell<AppState>>,
    title: &str,
    subtitle: &str,
    range: (f64, f64, f64),
    value: f64,
    edit: impl Fn(&mut Disc, i32) + 'static,
) -> adw::ActionRow {
    let state = state.clone();
    widget_spin_row(title, subtitle, range, value, move |v| {
        edit_disc(&state, |d| edit(d, v));
    })
}

/// "Album art" preferences group (WIDGETS_ROADMAP W2 GUI). Built exactly like
/// the three groups above it: off by default, and every row below the master
/// switch is insensitive until it is on.
///
/// "Spin while playing" carries its cost in the row for the same reason the
/// clock's "Show seconds" does — it is the difference between a widget drawn
/// once per track and one drawn continuously — and says that a paused track
/// stops, so nobody reads the switch as "spin forever".
fn add_disc_group(page: &adw::PreferencesPage, state: Rc<RefCell<AppState>>) {
    let cur = disc_settings(&state);

    let group = adw::PreferencesGroup::new();
    group.set_title(t!("Album art"));
    group.set_description(Some(t!(
        "Show the cover of the song that is playing as a record on top of your wallpaper."
    )));

    let enable = adw::ActionRow::new();
    enable.set_title(t!("Show album art"));
    enable.set_subtitle(
        "Shows the current track's cover art. Needs a media player that reports what it \
         is playing over MPRIS; nothing is drawn while nothing is playing.",
    );
    let enable_switch = gtk4::Switch::new();
    enable_switch.set_valign(gtk4::Align::Center);
    enable_switch.set_active(cur.enabled);
    enable.add_suffix(&enable_switch);
    enable.set_activatable_widget(Some(&enable_switch));

    // The same nine anchors, with the same names, as every other widget.
    let anchor_row = adw::ComboRow::new();
    anchor_row.set_title(t!("Position"));
    anchor_row.set_subtitle(t!("Where the record sits on the screen"));
    anchor_row.set_model(Some(&gtk4::StringList::new(&table_labels(&LYRIC_ANCHORS))));
    anchor_row.set_selected(table_index(&LYRIC_ANCHORS, cur.anchor));

    let size_row = disc_spin_row(
        &state,
        t!("Size"),
        t!("Diameter in pixels at 1080p, and scaled with the screen."),
        (48.0, 800.0, 8.0),
        f64::from(cur.size_px),
        |d, v| d.size_px = v.max(1) as u32,
    );

    let margin_row = disc_spin_row(
        &state,
        t!("Margin"),
        t!("Pixels from the screen edge. Raise it to clear a panel, dock or rounded corner."),
        (0.0, 300.0, 1.0),
        f64::from(cur.margin_px),
        |d, v| d.margin_px = v.max(0) as u32,
    );

    let spin_row = disc_switch_row(
        &state,
        t!("Spin while playing"),
        "Turns the record at 33\u{2153} rpm, and stops when the track is paused. Uses more \
         power: a still cover is drawn once per song, a turning one continuously.",
        cur.spin,
        |d, on| d.spin = on,
    );

    let opacity_row = disc_spin_row(
        &state,
        t!("Opacity"),
        t!("0 is invisible, 255 is solid. Lower it to let the wallpaper through."),
        (0.0, 255.0, 5.0),
        f64::from(cur.opacity),
        |d, v| d.opacity = v.clamp(0, 255) as u8,
    );

    // Combo last: set_selected above would otherwise fire this while the dialog
    // is still being built.
    {
        let state = state.clone();
        anchor_row.connect_selected_notify(move |row| {
            let Some((anchor, _)) = LYRIC_ANCHORS.get(row.selected() as usize).copied() else {
                return;
            };
            edit_disc(&state, |d| d.anchor = anchor);
        });
    }

    let dependents: Vec<gtk4::Widget> = vec![
        anchor_row.clone().upcast(),
        size_row.clone().upcast(),
        margin_row.clone().upcast(),
        spin_row.clone().upcast(),
        opacity_row.clone().upcast(),
    ];
    let sync_sensitive = move |on: bool| {
        for row in &dependents {
            row.set_sensitive(on);
        }
    };
    sync_sensitive(cur.enabled);
    {
        let state = state.clone();
        enable_switch.connect_active_notify(move |sw| {
            let on = sw.is_active();
            sync_sensitive(on);
            edit_disc(&state, |d| d.enabled = on);
        });
    }

    group.add(&enable);
    group.add(&anchor_row);
    group.add(&size_row);
    group.add(&margin_row);
    group.add(&spin_row);
    group.add(&opacity_row);
    page.add(&group);
}

// ─── Add from URL ─────────────────────────────────────────────────────────────

/// Paste a direct media URL (…/clip.mp4) → download into the library
/// (ROADMAP 3.2). Deliberately NOT yt-dlp/YouTube: direct files only.
fn show_add_from_url_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    const MAX_BYTES: u64 = 1_000_000_000; // refuse >1 GB outright

    let (dialog, content) = glass_dialog(window, t!("Add from URL"), 420, -1);
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_start(20);
    inner.set_margin_end(20);
    inner.set_margin_bottom(18);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("https://example.com/wallpaper.mp4"));
    inner.append(&entry);

    let hint = gtk4::Label::new(Some(t!(
        "Direct video or image links only (.mp4, .webm, .gif, .png, …)."
    )));
    hint.add_css_class("dim");
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    inner.append(&hint);

    let progress = gtk4::ProgressBar::new();
    progress.set_visible(false);
    inner.append(&progress);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.set_halign(gtk4::Align::End);
    let cancel_btn = gtk4::Button::with_label(t!("Cancel"));
    let add_btn = gtk4::Button::with_label(t!("Download"));
    add_btn.add_css_class("suggested-action");
    row.append(&cancel_btn);
    row.append(&add_btn);
    inner.append(&row);
    content.append(&inner);

    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
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
        let progress = progress.clone();
        let hint = hint.clone();
        let flag = cancel_flag;
        add_btn.connect_clicked(move |btn| {
            let url = entry_w.text().trim().to_string();
            if crate::download::media_filename(&url).is_none() {
                hint.set_text(t!("That doesn’t look like a direct media link."));
                return;
            }
            btn.set_sensitive(false);
            entry_w.set_sensitive(false);
            progress.set_visible(true);

            enum Msg {
                Progress(f64),
                Done(Result<std::path::PathBuf, String>),
            }
            let (tx, rx) = async_channel::bounded::<Msg>(16);
            let flag_worker = flag.clone();
            let dest = library::library_dir().join("downloads");
            std::thread::spawn(move || {
                let tx_p = tx.clone();
                let result = crate::download::download(
                    &url,
                    &dest,
                    MAX_BYTES,
                    &flag_worker,
                    move |got, total| {
                        if let Some(t) = total {
                            let _ = tx_p.try_send(Msg::Progress(got as f64 / t as f64));
                        }
                    },
                );
                let _ = tx.send_blocking(Msg::Done(result.map_err(|e| e.to_string())));
            });

            let state = state.clone();
            let dialog = dialog.clone();
            let progress = progress.clone();
            let hint = hint.clone();
            let btn = btn.clone();
            let entry_w = entry_w.clone();
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
                            e.generate_thumbnail();
                            let name = e.name.clone();
                            {
                                let mut s = state.borrow_mut();
                                s.entries.push(e);
                                save_entries(&s.entries).ok();
                            }
                            show_toast(
                                &state,
                                &tf!(
                                    "“{name}” added to the library",
                                    "name" => name
                                ),
                            );
                            let refresh = state.borrow().refresh.clone();
                            if let Some(r) = refresh {
                                r();
                            }
                            dialog.close();
                            break;
                        }
                        Msg::Done(Err(msg)) => {
                            hint.set_text(&msg);
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

// ─── About dialog ─────────────────────────────────────────────────────────────

fn show_about_dialog(window: &adw::ApplicationWindow) {
    let (dialog, content) = glass_dialog(window, t!("About Fresco"), 360, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);
    inner.set_halign(gtk4::Align::Center);

    let heading = gtk4::Label::new(Some("Fresco"));
    heading.add_css_class("dialog-heading");
    inner.append(&heading);

    let version = gtk4::Label::new(Some(&tf!(
        "Version {version}",
        "version" => crate::update::current_version()
    )));
    version.add_css_class("dim");
    inner.append(&version);

    let desc = gtk4::Label::new(Some(t!("Live wallpapers for Linux.")));
    desc.add_css_class("dialog-sub");
    desc.set_wrap(true);
    desc.set_justify(gtk4::Justification::Center);
    inner.append(&desc);

    let link = gtk4::LinkButton::with_label(
        "https://github.com/DibbayajyotiRoy/fresco",
        "github.com/DibbayajyotiRoy/fresco",
    );
    link.set_margin_top(4);
    inner.append(&link);

    content.append(&inner);
    dialog.present();
}

// ─── File picker ──────────────────────────────────────────────────────────────

const VIDEO_PATTERNS: [&str; 6] = ["*.mp4", "*.webm", "*.mkv", "*.avi", "*.mov", "*.gif"];
const IMAGE_PATTERNS: [&str; 5] = ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.bmp"];

fn media_filter(name: &str, patterns: &[&str]) -> gtk4::FileFilter {
    let f = gtk4::FileFilter::new();
    f.set_name(Some(name));
    for p in patterns {
        f.add_pattern(p);
    }
    f
}

fn open_file_picker(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
    editing_idx: Option<usize>,
) {
    let chooser = gtk4::FileChooserNative::new(
        Some(t!("Choose Wallpaper")),
        Some(window),
        FileChooserAction::Open,
        Some(t!("Open")),
        Some(t!("Cancel")),
    );
    chooser.set_select_multiple(true);

    // "All supported" first → it's the default filter, so videos AND images
    // both show without the user switching the dropdown.
    let all_pat: Vec<&str> = VIDEO_PATTERNS
        .iter()
        .chain(IMAGE_PATTERNS.iter())
        .copied()
        .collect();
    chooser.add_filter(&media_filter(t!("All supported"), &all_pat));
    chooser.add_filter(&media_filter(t!("Video files"), &VIDEO_PATTERNS));
    chooser.add_filter(&media_filter(t!("Image files"), &IMAGE_PATTERNS));

    let state_cb = state.clone();
    let win_cb = window.clone();
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
        // Replacing an existing entry's source keeps the old semantics: the
        // choice only makes sense when something new is being created.
        if editing_idx.is_none() && paths.len() > 1 {
            show_import_choice_dialog(&win_cb, state_cb.clone(), stack.clone(), paths);
        } else {
            add_media_paths(&state_cb, &stack, paths, editing_idx);
        }
    });
    state.borrow_mut().current_picker = Some(chooser.clone());
    chooser.show();
}

/// Shared tail of the add flows (file picker + window drag-and-drop): build the
/// right entry kind from the picked/dropped files, thumbnail it, store it, and
/// land in the editor.
fn add_media_paths(
    state: &Rc<RefCell<AppState>>,
    stack: &gtk4::Stack,
    mut paths: Vec<std::path::PathBuf>,
    editing_idx: Option<usize>,
) {
    if paths.is_empty() {
        return;
    }
    // Adding a file the library already holds used to make a second, identical
    // card — one of the two things the folders request was actually about.
    // Open the entry that already has it instead.
    if editing_idx.is_none() && paths.len() == 1 {
        let existing = {
            let s = state.borrow();
            library::duplicate_of(&s.entries, &paths[0]).map(|e| (e.id.clone(), e.name.clone()))
        };
        if let Some((id, name)) = existing {
            let idx = state.borrow().entries.iter().position(|e| e.id == id);
            if let Some(idx) = idx {
                show_toast(
                    state,
                    &tf!("“{name}” is already in your library", "name" => name),
                );
                state.borrow_mut().editing_idx = Some(idx);
                stack.set_visible_child_name("editor");
                return;
            }
        }
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

    {
        let mut s = state.borrow_mut();
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
    }
    spawn_metadata_probe(state);
    stack.set_visible_child_name("editor");
}

/// Folder picker → ask whether the folder becomes one timed slideshow or one
/// wallpaper per file, then do that.
fn open_folder_picker(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
) {
    let chooser = gtk4::FileChooserNative::new(
        Some(t!("Choose Slideshow Folder")),
        Some(window),
        FileChooserAction::SelectFolder,
        Some(t!("Select")),
        Some(t!("Cancel")),
    );
    let state_cb = state.clone();
    let win_cb = window.clone();
    chooser.connect_response(move |ch, resp| {
        state_cb.borrow_mut().current_picker = None;
        if resp != ResponseType::Accept {
            return;
        }
        let Some(folder) = ch.file().and_then(|f| f.path()) else {
            return;
        };
        show_folder_import_choice(&win_cb, state_cb.clone(), stack.clone(), folder);
    });
    state.borrow_mut().current_picker = Some(chooser.clone());
    chooser.show();
}

/// Re-pick the source for a broken library entry (its file/folder was moved
/// or deleted). Kind-aware: a single file for Video/Image, multiple files for
/// Playlist/paths-based Slideshow, a folder for folder-based Slideshow.
/// Clears `broken` on success via `check_health`.
fn relink_entry(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>, idx: usize) {
    let (kind, use_folder) = {
        let s = state.borrow();
        let Some(e) = s.entries.get(idx) else {
            return;
        };
        (e.kind, e.kind == Kind::Slideshow && e.paths.is_empty())
    };

    if use_folder {
        let chooser = gtk4::FileChooserNative::new(
            Some(t!("Relink Slideshow Folder")),
            Some(window),
            FileChooserAction::SelectFolder,
            Some(t!("Select")),
            Some(t!("Cancel")),
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
            finish_relink(&state_cb, idx, |e| e.folder = Some(folder));
        });
        state.borrow_mut().current_picker = Some(chooser.clone());
        chooser.show();
        return;
    }

    let chooser = gtk4::FileChooserNative::new(
        Some(t!("Relink Source")),
        Some(window),
        FileChooserAction::Open,
        Some(t!("Open")),
        Some(t!("Cancel")),
    );
    chooser.set_select_multiple(matches!(kind, Kind::Playlist | Kind::Slideshow));
    // Same kind-appropriate restriction as the Add flow, so a broken Video
    // entry can't be "fixed" by pointing it at a non-media file.
    match kind {
        Kind::Video | Kind::Playlist => {
            chooser.add_filter(&media_filter(t!("Video files"), &VIDEO_PATTERNS));
        }
        Kind::Image | Kind::Slideshow => {
            chooser.add_filter(&media_filter(t!("Image files"), &IMAGE_PATTERNS));
        }
    }
    let state_cb = state.clone();
    chooser.connect_response(move |ch, resp| {
        state_cb.borrow_mut().current_picker = None;
        if resp != ResponseType::Accept {
            return;
        }
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
        match kind {
            Kind::Playlist | Kind::Slideshow => {
                finish_relink(&state_cb, idx, |e| e.paths = paths);
            }
            _ => {
                finish_relink(&state_cb, idx, |e| e.path = Some(paths.remove(0)));
            }
        }
    });
    state.borrow_mut().current_picker = Some(chooser.clone());
    chooser.show();
}

/// Apply a relinked source to entry `idx`, re-check health, persist, toast,
/// and refresh the grid. Mirrors `commit_rename`'s save→toast→refresh tail.
fn finish_relink(state: &Rc<RefCell<AppState>>, idx: usize, apply: impl FnOnce(&mut LibraryEntry)) {
    {
        let mut s = state.borrow_mut();
        let Some(e) = s.entries.get_mut(idx) else {
            return;
        };
        apply(e);
        e.check_health();
        save_entries(&s.entries).ok();
    }
    show_toast(state, t!("Relinked"));
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Shared shell for the glass-styled modals: a transient `adw::Window` whose
/// content box already holds the header bar. Callers append their body to the
/// returned box and call `dialog.present()`.
pub(crate) fn glass_dialog(
    window: &adw::ApplicationWindow,
    title: &str,
    width: i32,
    height: i32,
) -> (adw::Window, gtk4::Box) {
    let dialog = adw::Window::new();
    dialog.add_css_class("glass");
    dialog.set_transient_for(Some(window));
    dialog.set_modal(true);
    dialog.set_title(Some(title));
    dialog.set_default_size(width, height);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&adw::HeaderBar::new());
    dialog.set_content(Some(&content));
    (dialog, content)
}

/// Uppercase section label styled as an overline (see theme.rs `.overline`).
fn overline(text: &str) -> gtk4::Label {
    // Upper-casing is the overline's whole visual signature in English, and a
    // no-op in scripts without case — but `to_uppercase` on mixed text would
    // shout the Latin fragments inside an otherwise Chinese heading. Apply it
    // only when the label is entirely caseless-or-uppercasable Latin, and leave
    // everything else to the `.overline` CSS (size, tracking, colour), which
    // carries the style on its own.
    let display = if text.chars().any(|c| c.is_alphabetic() && !c.is_ascii()) {
        text.to_string()
    } else {
        text.to_uppercase()
    };
    let l = gtk4::Label::new(Some(&display));
    l.add_css_class("overline");
    l.set_xalign(0.0);
    l.set_halign(gtk4::Align::Start);
    l
}

/// Icon + label content for a button (Adwaita ButtonContent).
/// The Pinterest brand glyph, embedded so it works regardless of the user's
/// icon theme (no theme ships a Pinterest icon). Decoding is fallible only if
/// the platform lacks an SVG loader; in that case we fall back to the generic
/// link icon rather than shipping a blank button.
///
/// The mark is Pinterest's trademark — see `data/icons/pinterest.svg` for the
/// usage terms this follows. It is rendered unmodified, at its own colour, and
/// paired with a neutral label that describes the action rather than claiming
/// any affiliation.
fn pinterest_button_content() -> gtk4::Widget {
    use gtk4::prelude::*;

    const LOGO: &[u8] = include_bytes!("../../data/icons/pinterest.svg");

    let texture = gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from_static(LOGO)).ok();

    let Some(texture) = texture else {
        log::debug!("no SVG loader for the Pinterest glyph; using a generic link icon");
        return button_content("insert-link-symbolic", t!("From link")).upcast();
    };

    // Hand-built rather than adw::ButtonContent: that widget only takes a
    // themed icon *name*, and the brand mark must keep its own colour.
    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let image = gtk4::Image::from_paintable(Some(&texture));
    image.set_pixel_size(16);
    content.append(&image);
    content.append(&gtk4::Label::new(Some(t!("From Pinterest"))));
    content.upcast()
}

fn button_content(icon: &str, label: &str) -> adw::ButtonContent {
    let c = adw::ButtonContent::new();
    c.set_icon_name(icon);
    c.set_label(label);
    c
}

/// A flat, full-width, left-aligned action row for the menu popover (GTK-menu
/// style: no button chrome at rest, subtle background on hover — see
/// `.menu-item` in theme.rs).
fn menu_item(label: &str) -> gtk4::Button {
    let btn = gtk4::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("menu-item");
    btn.set_halign(gtk4::Align::Fill);
    let lbl = gtk4::Label::new(Some(label));
    lbl.set_xalign(0.0);
    lbl.set_halign(gtk4::Align::Start);
    lbl.set_hexpand(true);
    btn.set_child(Some(&lbl));
    btn
}

/// A menu item that opens a window, closing the menu on the way.
///
/// The popdown is the load-bearing part. A `GtkPopover` is a native surface of
/// its own, and on X11 that is an override-redirect window — one the window
/// manager will never stack below an ordinary toplevel, no matter which window
/// has focus. So a dialog opened while the menu is still up is drawn *behind*
/// the menu that opened it, and on a compositor that does not dim or clip
/// popups (Deepin's DDE, for one) it simply looks like the menu ate the page
/// (issue #6). Wayland has the same stacking rule; there the menu is merely
/// smaller than the dialog more often, so it hides less of it.
///
/// GTK does not do this for us: a `GtkPopover` filled with widgets only
/// auto-dismisses on an outside click, unlike the menu-model popover a
/// `GtkMenuButton` builds from a `GMenu`.
///
/// `open` runs after the popdown, which in GTK4 hides the surface and drops its
/// seat grab synchronously — so the new toplevel maps into a clear stack and
/// can take input immediately.
fn menu_item_opening_window<F: Fn() + 'static>(
    label: &str,
    popover: &gtk4::Popover,
    open: F,
) -> gtk4::Button {
    let btn = menu_item(label);
    let popover = popover.clone();
    btn.connect_clicked(move |_| {
        popover.popdown();
        open();
    });
    btn
}

/// A label + trailing switch row for the menu popover.
fn switch_row<F: Fn(bool) + 'static>(label: &str, active: bool, on_toggle: F) -> gtk4::Box {
    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    hbox.add_css_class("menu-row");
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

/// Language picker for the menu popover.
///
/// One flat row per language with a checkmark on the active one — deliberately
/// *not* a `GtkDropDown`. A dropdown reads better as the list grows, but its
/// list is a popover, and a popover nested inside the menu popover hands GTK a
/// second popup surface to grab. On X11 that grab is a real seat grab, and
/// dismissing the inner list by clicking the dropdown again (rather than
/// clicking outside it) leaves the toplevel with no input: the window paints
/// and the daemon keeps rendering, but nothing in the main window responds
/// (issue #5, Deepin 25 / DDE / X11). Rows in the parent popover open no second
/// surface, so there is nothing to leak. If the list ever outgrows this menu,
/// move it into its own dialog window — not back into a nested popup.
///
/// The change takes effect on restart, and the toast says so. Fresco resolves
/// every translated string to a `&'static str` that lives for the process
/// lifetime (see [`crate::i18n`]), so a live swap would have to rebuild every
/// widget in the window; a toast that names the requirement is more honest than
/// a partial retranslation, and matches how GNOME's own language switch
/// behaves.
fn build_language_row(state: Rc<RefCell<AppState>>) -> gtk4::Box {
    use crate::i18n::Language;

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let current = state.borrow().config.language;

    // Every row's checkmark, so picking one can clear the others.
    let checks: Rc<RefCell<Vec<(Language, gtk4::Image)>>> = Rc::new(RefCell::new(Vec::new()));

    for lang in Language::ALL {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let lbl = gtk4::Label::new(Some(lang.display_name()));
        lbl.set_xalign(0.0);
        lbl.set_halign(gtk4::Align::Start);
        lbl.set_hexpand(true);
        let check = gtk4::Image::from_icon_name("emblem-ok-symbolic");
        check.set_visible(lang == current);
        row.append(&lbl);
        row.append(&check);

        let btn = gtk4::Button::new();
        btn.add_css_class("flat");
        btn.add_css_class("menu-item");
        btn.set_halign(gtk4::Align::Fill);
        btn.set_child(Some(&row));
        {
            let state2 = state.clone();
            let checks = checks.clone();
            btn.connect_clicked(move |_| {
                {
                    let mut s = state2.borrow_mut();
                    if s.config.language == lang {
                        return;
                    }
                    s.config.language = lang;
                    s.config.save().ok();
                }
                for (l, img) in checks.borrow().iter() {
                    img.set_visible(*l == lang);
                }
                show_toast(&state2, t!("Restart Fresco to apply the new language"));
            });
        }
        vbox.append(&btn);
        checks.borrow_mut().push((lang, check));
    }

    vbox
}

pub(crate) fn show_toast(state: &Rc<RefCell<AppState>>, msg: &str) {
    let toast = adw::Toast::new(msg);
    toast.set_timeout(4);
    state.borrow().toast.add_toast(toast);
}

fn entry_is_active(entry: &LibraryEntry, cfg: &Config) -> bool {
    if !cfg.enabled {
        return false;
    }
    // Active = the default wallpaper OR any per-monitor override.
    entry_matches_wallpaper(entry, &cfg.wallpaper)
        || cfg
            .monitors
            .values()
            .any(|w| entry_matches_wallpaper(entry, w))
}

fn entry_matches_wallpaper(entry: &LibraryEntry, w: &crate::config::Wallpaper) -> bool {
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

/// Write a per-monitor assignment: only `[monitors."<connector>"]` changes;
/// the default wallpaper is untouched. (ROADMAP 2.2)
fn assign_entry_to_monitor(cfg: &mut Config, wallpaper: crate::config::Wallpaper, connector: &str) {
    cfg.monitors.insert(connector.to_string(), wallpaper);
    cfg.enabled = true;
}

/// Remove every per-monitor override so the default wallpaper shows everywhere.
fn clear_monitor_overrides(cfg: &mut Config) {
    cfg.monitors.clear();
}

/// Connected displays as the daemon reports them (empty when it isn't running).
fn connected_monitors() -> Vec<crate::ipc::MonitorInfo> {
    match crate::ipc::request(&crate::ipc::Request::Status) {
        Ok(crate::ipc::Response::Status(s)) => s.monitors_info,
        _ => Vec::new(),
    }
}

/// Human-friendly card title for an entry, without renaming anything: hex/uuid
/// auto-names become "Video · aeb8", marketing prefixes like "From <site>- "
/// are trimmed, and long titles are middle-truncated. The untouched name stays
/// available as the card tooltip.
fn display_name(name: &str, kind: Kind) -> String {
    let mut n = name.trim().to_string();

    // Hex/uuid-ish stems (downloader auto-names) → "<Kind> · <last 4>".
    let ident: String = n.chars().filter(|c| *c != '-' && *c != '_').collect();
    if ident.chars().count() >= 16 && ident.chars().all(|c| c.is_ascii_hexdigit()) {
        let label = match kind {
            Kind::Video => t!("Video"),
            Kind::Image => t!("Image"),
            Kind::Playlist => t!("Playlist"),
            Kind::Slideshow => t!("Slideshow"),
        };
        let suffix: String = ident
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return format!("{label} · {suffix}");
    }

    // Trim marketing prefixes ("From Klickpin.com- 14 Beautiful…").
    if let Some(rest) = n.strip_prefix("From ").or_else(|| n.strip_prefix("from ")) {
        if let Some(pos) = rest.find("- ") {
            let site = &rest[..pos];
            if site.contains('.') && !site.contains(' ') {
                let trimmed = rest[pos + 2..].trim();
                if !trimmed.is_empty() {
                    n = trimmed.to_string();
                }
            }
        }
    }

    // Middle-truncate long titles, keeping start + end.
    let chars: Vec<char> = n.chars().collect();
    if chars.len() > 48 {
        let head: String = chars[..30].iter().collect();
        let tail: String = chars[chars.len() - 15..].iter().collect();
        n = format!("{}…{}", head.trim_end(), tail.trim_start());
    }
    n
}

/// Symbolic icon shown on a card that has no thumbnail (yet).
fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Video => "video-x-generic-symbolic",
        Kind::Image => "image-x-generic-symbolic",
        Kind::Playlist => "view-list-symbolic",
        Kind::Slideshow => "folder-pictures-symbolic",
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
        Accent::Blue => t!("Blue"),
        Accent::Teal => t!("Teal"),
        Accent::Green => t!("Green"),
        Accent::Amber => t!("Amber"),
        Accent::Coral => t!("Coral"),
        Accent::Graphite => t!("Graphite"),
    }
}

/// Slideshow interval choices (seconds), matched 1:1 with the editor combo rows
/// "5 seconds / 15 seconds / 30 seconds / 1 minute / 5 minutes / 10 minutes".
const INTERVAL_OPTIONS: [u64; 6] = [5, 15, 30, 60, 300, 600];

/// Global power-saving choices (Settings). Order matches [`POWER_VALUES`].
const POWER_LABELS: [&str; 3] = ["Full quality", "Reduced", "Minimum"];
const POWER_VALUES: [PowerSaving; 3] = [
    PowerSaving::Full,
    PowerSaving::Reduced,
    PowerSaving::Minimum,
];

fn power_from_index(index: u32) -> PowerSaving {
    POWER_VALUES
        .get(index as usize)
        .copied()
        .unwrap_or(PowerSaving::Full)
}

fn power_index(p: PowerSaving) -> u32 {
    POWER_VALUES.iter().position(|&v| v == p).unwrap_or(0) as u32
}

/// Per-wallpaper editor choices. Index 0 = "Default" (inherit the global
/// level → `None`); the rest override it.
const POWER_EDIT_LABELS: [&str; 4] = ["Default", "Full quality", "Reduced", "Minimum"];

/// Dropdown index → per-wallpaper override (`None` = inherit the global level).
fn power_edit_from_index(index: u32) -> Option<PowerSaving> {
    (index > 0).then(|| power_from_index(index - 1))
}

/// Per-wallpaper override → dropdown index; `None` shows as "Default".
fn power_edit_index(p: Option<PowerSaving>) -> u32 {
    p.map(|v| power_index(v) + 1).unwrap_or(0)
}

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
    poll_notifications(window, state.clone());
    super::updates::check_for_updates(window, state, false);
}

/// Submit a rating, and — when `allow_reply` — open a support thread so the
/// maintainer can actually answer.
///
/// The thread is what turns "does not work, wallpaper is just black" from an
/// unactionable row into a conversation. Both calls run on one background
/// thread and in this order: the feedback row is the thing that must not be
/// lost, so a failure to open the thread never costs the rating.
fn submit_feedback_async(
    rating: i8,
    comment: &gtk4::Entry,
    allow_reply: bool,
    state: &Rc<RefCell<AppState>>,
) {
    let text = comment.text().to_string();
    let note = if text.trim().is_empty() {
        None
    } else {
        Some(text.clone())
    };
    std::thread::spawn(move || {
        let ticket = allow_reply.then(crate::support::ticket_for_reply).flatten();
        crate::supabase::submit_feedback(rating, note, ticket).ok();
        if allow_reply {
            let env = crate::telemetry::env_summary();
            crate::support::open_from_feedback(rating, &text, Some(&env)).ok();
        }
    });
    // A 👎 deserves an apology, not a cheer. The same toast for both reads as
    // tone-deaf to someone whose wallpaper just isn't working.
    let message = if rating < 0 {
        if allow_reply {
            t!("Thanks for telling us. We'll look into it and reply in the app.")
        } else {
            t!("Thanks for telling us. We'll look into it.")
        }
    } else {
        t!("Thanks, that means a lot!")
    };
    state.borrow().toast.add_toast(adw::Toast::new(message));
}

// ─── Command palette (Ctrl+K) ─────────────────────────────────────────────────

/// One palette entry: display label, lowercase haystack for filtering, action.
struct PaletteCmd {
    label: String,
    hay: String,
    run: Rc<dyn Fn()>,
}

/// Ctrl+K command palette: a glass modal with a big entry + result list.
/// Everything is built from in-memory state once at open — filtering on each
/// keystroke does no I/O.
fn show_command_palette(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
) {
    let (dialog, content) = glass_dialog(window, t!("Commands"), 560, 440);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_start(16);
    inner.set_margin_end(16);
    inner.set_margin_bottom(14);

    let entry = gtk4::Entry::new();
    entry.add_css_class("palette-entry");
    entry.set_placeholder_text(Some(t!("Type a command or wallpaper name…")));
    inner.append(&entry);

    let list = gtk4::ListBox::new();
    list.add_css_class("palette-list");
    list.set_selection_mode(gtk4::SelectionMode::Browse);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    inner.append(&scroll);

    let hint = gtk4::Label::new(Some(t!("↑↓ navigate · Enter run · Esc close")));
    hint.add_css_class("palette-hint");
    hint.set_xalign(0.0);
    inner.append(&hint);
    content.append(&inner);

    // ── Build the command set once (wallpapers + static commands) ──
    let mut cmds: Vec<PaletteCmd> = Vec::new();
    {
        let entries = &state.borrow().entries;
        for (idx, e) in entries.iter().enumerate() {
            if e.broken {
                continue;
            }
            let pretty = display_name(&e.name, e.kind);
            let s = state.clone();
            cmds.push(PaletteCmd {
                label: tf!("Set: {name}", "name" => pretty),
                hay: format!("{} {}", pretty.to_lowercase(), e.name.to_lowercase()),
                run: Rc::new(move || apply_entry_by_idx(s.clone(), idx)),
            });
        }
    }
    let mut add_cmd = |label: &str, run: Rc<dyn Fn()>| {
        cmds.push(PaletteCmd {
            label: label.to_string(),
            hay: label.to_lowercase(),
            run,
        });
    };
    {
        let s = state.clone();
        add_cmd(
            t!("Random wallpaper"),
            Rc::new(move || {
                let candidates: Vec<usize> = s
                    .borrow()
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| !e.broken)
                    .map(|(i, _)| i)
                    .collect();
                if candidates.is_empty() {
                    return;
                }
                // Cheap non-crypto pick; not worth a rand dependency.
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0) as usize;
                apply_entry_by_idx(s.clone(), candidates[nanos % candidates.len()]);
            }),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            t!("Browse catalog"),
            Rc::new(move || super::gallery::show_gallery_window(&w, s.clone())),
        );
    }
    {
        let (w, s, st) = (window.clone(), state.clone(), stack.clone());
        add_cmd(
            t!("Add from link"),
            Rc::new(move || super::add_link::show_add_link_dialog(&w, s.clone(), st.clone())),
        );
    }
    {
        let (w, s, st) = (window.clone(), state.clone(), stack.clone());
        add_cmd(
            t!("Add files"),
            Rc::new(move || open_file_picker(&w, s.clone(), st.clone(), None)),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            t!("Advanced settings"),
            Rc::new(move || show_advanced_dialog(&w, s.clone())),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            t!("Send feedback"),
            Rc::new(move || show_feedback_dialog(&w, s.clone())),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            t!("What can Fresco do?"),
            Rc::new(move || show_tour_dialog(&w, s.clone())),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            t!("What’s new in Fresco"),
            Rc::new(move || show_onboarding_dialog(&w, s.clone())),
        );
    }
    let cmds = Rc::new(cmds);

    // Actions of the currently listed rows, parallel to the ListBox rows.
    type RowActions = Rc<RefCell<Vec<Rc<dyn Fn()>>>>;
    let visible: RowActions = Rc::new(RefCell::new(Vec::new()));

    let rebuild = {
        let list = list.clone();
        let cmds = cmds.clone();
        let visible = visible.clone();
        Rc::new(move |query: &str| {
            while let Some(row) = list.first_child() {
                list.remove(&row);
            }
            let q = query.trim().to_lowercase();
            let mut shown = Vec::new();
            for cmd in cmds.iter() {
                if !q.is_empty() && !cmd.hay.contains(&q) {
                    continue;
                }
                let lbl = gtk4::Label::new(Some(&cmd.label));
                lbl.set_xalign(0.0);
                lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                let row = gtk4::ListBoxRow::new();
                row.set_child(Some(&lbl));
                list.append(&row);
                shown.push(cmd.run.clone());
                if shown.len() >= 40 {
                    break;
                }
            }
            *visible.borrow_mut() = shown;
            if let Some(first) = list.row_at_index(0) {
                list.select_row(Some(&first));
            }
        })
    };
    rebuild("");
    {
        let rebuild = rebuild.clone();
        entry.connect_changed(move |e| rebuild(&e.text()));
    }

    // Run the selected (or top) result and close.
    let activate = {
        let list = list.clone();
        let visible = visible.clone();
        let dialog = dialog.clone();
        Rc::new(move || {
            let idx = list.selected_row().map(|r| r.index()).unwrap_or(0).max(0) as usize;
            let run = visible.borrow().get(idx).cloned();
            if let Some(run) = run {
                dialog.close();
                run();
            }
        })
    };
    {
        let activate = activate.clone();
        entry.connect_activate(move |_| activate());
    }
    {
        let activate = activate.clone();
        list.connect_row_activated(move |_, _| activate());
    }

    // Up/Down move the selection while the entry keeps focus; Esc closes.
    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let list = list.clone();
        let dialog = dialog.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            use gtk4::gdk::Key;
            match key {
                Key::Escape => {
                    dialog.close();
                    glib::Propagation::Stop
                }
                Key::Down | Key::Up => {
                    let cur = list.selected_row().map(|r| r.index()).unwrap_or(0);
                    let next = if key == Key::Down { cur + 1 } else { cur - 1 };
                    if let Some(row) = list.row_at_index(next.max(0)) {
                        list.select_row(Some(&row));
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
    }
    dialog.add_controller(keys);

    dialog.present();
    entry.grab_focus();
}

/// Telemetry consent — asked before anything is ever sent (the telemetry layer
/// is a no-op until this is answered). Both choices carry equal visual weight:
/// consent that's honest converts better than consent that's tricked.
///
/// Declining is not total silence, and this dialog says so in as many words
/// rather than burying it. That sentence is the whole reason the two-button
/// shape is defensible: a user who declines and is still counted has been told
/// exactly what is still counted (their country, and nothing that identifies
/// them), before they choose. Removing or softening it would turn an honest
/// trade-off into a dark pattern — if this text changes, bump
/// [`crate::telemetry::CONSENT_VERSION`] and update TERMS.md to match.
fn show_telemetry_consent_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let (dialog, content) = glass_dialog(window, t!("Help improve Fresco?"), 500, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    // One paragraph per `t!` on purpose: a single wall of text would be one
    // enormous translation unit that goes stale the moment a comma moves, and
    // the i18n msgid extractor cannot reconstruct a backslash-continued
    // literal anyway (see `source_msgids` in i18n.rs).
    let paragraphs = [
        t!("Fresco is free: no ads, no accounts, nothing to sell. These numbers exist for one reason, which is to know which distros and desktops people actually run, so that the one you use keeps working."),
        t!("Either way, Fresco counts you once a day: a random install id (not tied to you, your hardware, or your name), your country, and the app version. That is how many people use Fresco and where, which decides what gets tested."),
        t!("Accept all adds the detail: your distro, desktop, session type, video backend, monitor count, which features you use, error kinds, and the time of each check-in."),
        t!("Decline optional and none of that detail is sent, and your check-in is recorded as a date rather than a time."),
        t!("Never collected, either way: personal data, file names, your wallpapers, or your IP address."),
        t!("Accepting all also accepts the terms of use. You can change your answer anytime in Settings."),
    ];
    let body = gtk4::Label::new(Some(&paragraphs.join("\n\n")));
    body.set_wrap(true);
    body.set_xalign(0.0);
    inner.append(&body);

    // Accepting all is also acceptance of the terms, so the terms have to be
    // reachable from the dialog that says so, not just from the README.
    let terms = gtk4::LinkButton::with_label(
        "https://github.com/DibbayajyotiRoy/fresco/blob/main/TERMS.md",
        t!("Read the full terms of use"),
    );
    terms.set_halign(gtk4::Align::Start);
    terms.set_margin_start(0);
    inner.append(&terms);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    buttons.set_margin_top(6);
    let decline = gtk4::Button::with_label(t!("Decline optional"));
    let accept = gtk4::Button::with_label(t!("Accept all"));
    accept.add_css_class("suggested-action");
    buttons.append(&decline);
    buttons.append(&accept);
    inner.append(&buttons);

    let answer = {
        let state = state.clone();
        let dialog = dialog.clone();
        move |yes: bool| {
            let mut s = state.borrow_mut();
            s.config.telemetry = yes;
            s.config.telemetry_prompted = true;
            // Stamp which terms this answer was given under, so a future
            // change to what is collected re-asks instead of assuming.
            s.config.telemetry_consent_version = crate::telemetry::CONSENT_VERSION;
            s.config.save().ok();
            drop(s);
            dialog.close();
        }
    };
    {
        let answer = answer.clone();
        decline.connect_clicked(move |_| answer(false));
    }
    accept.connect_clicked(move |_| answer(true));

    content.append(&inner);
    dialog.present();
}

/// Current onboarding revision. Bump this when the flow it teaches changes
/// materially — every install with a lower `config.onboarding_version` is
/// walked through once on next launch, including users upgrading from a
/// version that predates it.
///
/// Revision 2 replaces the single paste-a-link screen with a two-step
/// what's-new flow: setting a wallpaper from a Pinterest or direct link (with
/// a working link already in the box, so it can be finished in one click), and
/// where the lyrics and clock widget settings live.
pub(crate) const ONBOARDING_VERSION: u32 = 2;

/// The tutorial video. Opened in the user's browser rather than embedded:
/// Fresco ships no browser engine, and pulling in WebKitGTK to play one clip
/// would dwarf the rest of the app.
const TUTORIAL_URL: &str = "https://youtu.be/YWzD3-xkCEc";

/// Open a URL in the user's browser. Best-effort — a missing `xdg-open` must
/// never take the app down.
fn open_in_browser(url: &str) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(url).spawn() {
        log::debug!("couldn't open {url}: {e}");
    }
}

/// A Pinterest pin that is a live wallpaper, pre-filled into step 1 so someone
/// with nothing to paste can still finish the step in one click. Typing over it
/// imports whatever they pasted instead — the entry is not special-cased.
const ONBOARDING_SAMPLE_LINK: &str = "https://pin.it/2q9awnLre";

/// Same 1 GB ceiling the "Add from link" and "Add from URL" flows use.
const ONBOARDING_MAX_BYTES: u64 = 1_000_000_000;

/// What finishing a step does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StepEnd {
    /// Show the next step. Nothing is persisted.
    Advance,
    /// The last step: finishing it is what marks onboarding done.
    Complete,
}

/// One screen of the what's-new flow.
struct OnboardingStep {
    /// `gtk4::Stack` child name for this step's page.
    id: &'static str,
    heading: &'static str,
    /// Label on the primary (right-hand) button.
    primary: &'static str,
    /// Label the primary button takes once this step's own action has
    /// succeeded, and the marker for "this step has an action at all".
    /// `None` for steps that only read.
    primary_done: Option<&'static str>,
    end: StepEnd,
}

/// The flow, in order.
///
/// **Completion is all-or-nothing.** Exactly one step ends in
/// [`StepEnd::Complete`] — the last one — and [`complete_onboarding`] is called
/// from nowhere else. Nothing about the flow's progress is written while it
/// runs, so quitting Fresco on step 2 shows step 1 again on the next launch.
/// That is deliberate: a half-remembered position turns "the user has seen
/// what's new" into a per-step ledger that can disagree with itself, and
/// repeating two screens is the cheaper failure. `ONBOARDING_VERSION` stays a
/// single honest fact — either the whole flow was seen, or it wasn't.
const ONBOARDING_STEPS: [OnboardingStep; 2] = [
    OnboardingStep {
        id: "link",
        heading: "Set a wallpaper from a link",
        primary: "Set wallpaper",
        primary_done: Some("Next"),
        end: StepEnd::Advance,
    },
    OnboardingStep {
        id: "widgets",
        heading: "Put widgets on your wallpaper",
        primary: "Done",
        primary_done: None,
        end: StepEnd::Complete,
    },
];

/// Record that the whole flow was seen. The only writer of
/// `config.onboarding_version` in the GUI — see the note on
/// [`ONBOARDING_STEPS`] for why it is called once, at the end, and never
/// mid-flow. Re-running the flow from the command palette after it has already
/// been completed writes nothing.
fn complete_onboarding(state: &Rc<RefCell<AppState>>) {
    let mut s = state.borrow_mut();
    if s.config.onboarding_version < ONBOARDING_VERSION {
        s.config.onboarding_version = ONBOARDING_VERSION;
        s.config.save().ok();
    }
}

/// A bold title over a dim body — the row shape used inside the step pages.
fn onboarding_row(title: &str, body: &str) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let t = gtk4::Label::new(Some(title));
    t.add_css_class("heading");
    t.set_xalign(0.0);
    let b = gtk4::Label::new(Some(body));
    b.add_css_class("dim");
    b.set_wrap(true);
    b.set_xalign(0.0);
    row.append(&t);
    row.append(&b);
    row
}

/// Heading + lead paragraph + labelled rows: the shape both steps share.
fn onboarding_page(step: &OnboardingStep, lead: &str, rows: &[(&str, &str)]) -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);

    let heading = gtk4::Label::new(Some(t!(step.heading)));
    heading.add_css_class("dialog-heading");
    heading.set_wrap(true);
    heading.set_xalign(0.0);
    page.append(&heading);

    let lead_label = gtk4::Label::new(Some(lead));
    lead_label.add_css_class("dialog-sub");
    lead_label.set_wrap(true);
    lead_label.set_xalign(0.0);
    page.append(&lead_label);

    for (title, body) in rows {
        page.append(&onboarding_row(title, body));
    }
    page
}

/// Step 1's widgets, grouped so its background job can be handed all of them.
#[derive(Clone)]
struct LinkStep {
    entry: gtk4::Entry,
    error: gtk4::Label,
    status: gtk4::Label,
    progress: gtk4::ProgressBar,
}

/// Every library entry paired with the file names backing it.
///
/// Step 1 dedupes against these the way "Add from link" does — by looking for
/// the Pinterest pin id stamped into a downloaded file's name. Replaying the
/// flow is normal here, not an edge case: it restarts from step 1 whenever it
/// is quit part-way, so pressing the same pre-filled button a second time must
/// recognise the wallpaper instead of fetching another copy of it.
fn library_file_names(state: &Rc<RefCell<AppState>>) -> Vec<(usize, String)> {
    state
        .borrow()
        .entries
        .iter()
        .enumerate()
        .flat_map(|(i, e)| {
            e.path
                .iter()
                .chain(e.paths.iter())
                .chain(e.folder.iter())
                .filter_map(|p| p.file_name().map(|n| (i, n.to_string_lossy().into_owned())))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Rename a freshly downloaded file so its name carries the Pinterest pin id.
/// That id is the dedupe key both this flow and "Add from link" search for, and
/// the CDN file name does not contain it. Best-effort: a failed rename keeps
/// the URL-derived name, which is still a perfectly good library entry.
fn stamp_pin_id(path: PathBuf, pin_id: &str) -> PathBuf {
    let already = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().contains(pin_id));
    if already {
        return path;
    }
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return path;
    };
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    let target = (0..)
        .map(|i| {
            if i == 0 {
                dir.join(format!("{stem}-{pin_id}{ext}"))
            } else {
                dir.join(format!("{stem}-{pin_id}-{i}{ext}"))
            }
        })
        .find(|p| !p.exists())
        .expect("some suffix is free");
    match std::fs::rename(&path, &target) {
        Ok(()) => target,
        Err(_) => path,
    }
}

/// Step 1's action: resolve the pasted link, download it, add it to the library
/// and set it as the wallpaper.
///
/// This drives [`crate::linkresolve::resolve`] and [`crate::download::download`]
/// directly — the same functions behind the "Add from link" dialog — rather
/// than opening that dialog on top of this one. Two reasons: it takes no
/// pre-fill (it reads the clipboard, which this step must not clobber), and it
/// ends in the crop editor, whereas this step has to end with the wallpaper
/// actually on screen. No URL handling is repeated here; every bit of
/// resolution stays in `linkresolve`.
fn run_link_step(
    state: &Rc<RefCell<AppState>>,
    ui: &LinkStep,
    primary: &gtk4::Button,
    done: &Rc<Cell<bool>>,
    render: &Rc<dyn Fn()>,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let url = ui.entry.text().trim().to_string();
    if !url.starts_with("http") {
        ui.error.set_text(t!("That doesn’t look like a link."));
        ui.error.set_visible(true);
        return;
    }
    // Telemetry label only — the resolver decides how the URL is handled.
    let source = if url.contains("pin.it") || url.contains("pinterest.com") {
        "pinterest"
    } else {
        "direct"
    };

    ui.entry.set_sensitive(false);
    primary.set_sensitive(false);
    ui.error.set_visible(false);
    ui.status.set_text(t!("Resolving link…"));
    ui.status.add_css_class("shimmer");
    ui.status.set_visible(true);
    ui.progress.set_visible(true);

    enum Msg {
        Downloading,
        Progress(f64),
        /// Already downloaded by an earlier run through this flow.
        Duplicate(String),
        /// Downloaded file, plus the resolved title to name the entry after.
        Done(Result<(PathBuf, Option<String>), String>),
    }
    let (tx, rx) = async_channel::bounded::<Msg>(16);
    let existing: Vec<String> = library_file_names(state)
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    let dest = library::library_dir().join("downloads");
    let cancel_worker = cancel.clone();
    std::thread::spawn(move || {
        let resolved = match crate::linkresolve::resolve(&url) {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send_blocking(Msg::Done(Err(e.to_string())));
                return;
            }
        };
        if let Some(pin) = &resolved.pin_id {
            if existing.iter().any(|n| n.contains(pin.as_str())) {
                let _ = tx.send_blocking(Msg::Duplicate(pin.clone()));
                return;
            }
        }
        let _ = tx.send_blocking(Msg::Downloading);
        let tx_p = tx.clone();
        let got = crate::download::download(
            &resolved.media_url,
            &dest,
            ONBOARDING_MAX_BYTES,
            &cancel_worker,
            move |got, total| {
                if let Some(t) = total {
                    let _ = tx_p.try_send(Msg::Progress(got as f64 / t as f64));
                }
            },
        );
        let _ = tx.send_blocking(Msg::Done(
            got.map(|p| {
                let p = match &resolved.pin_id {
                    Some(pin) => stamp_pin_id(p, pin),
                    None => p,
                };
                (p, resolved.title.clone())
            })
            .map_err(|e| e.to_string()),
        ));
    });

    // Pulse while no byte-level progress is known (the resolve phase, or a
    // server that omits Content-Length); the first Progress switches the bar to
    // determinate.
    let pulsing = Rc::new(Cell::new(true));
    {
        let progress = ui.progress.clone();
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
    let ui = ui.clone();
    let primary = primary.clone();
    let done = done.clone();
    let render = render.clone();
    glib::spawn_future_local(async move {
        // The step counts as done only once the wallpaper is on screen.
        let finish = |idx: usize, downloaded: bool| {
            // Mirrors the "Add from link" event so the two are comparable, but
            // tagged: onboarding-driven adds must be excludable from the
            // discovery metric this flow exists to move.
            crate::telemetry::event(
                "add_from_link",
                serde_json::json!({
                    "ok": true,
                    "source": source,
                    "onboarding": true,
                    "reused": !downloaded,
                }),
            );
            apply_entry_by_idx(state.clone(), idx);
            pulsing.set(false);
            done.set(true);
            ui.status.remove_css_class("shimmer");
            ui.status.set_text(t!("Set as your wallpaper."));
            ui.progress.set_visible(false);
            primary.set_sensitive(true);
            render();
        };
        // Never wedge the flow: the entry and the primary button come back, so
        // the same link can be retried, and Skip was never disabled.
        let fail = |msg: &str| {
            pulsing.set(false);
            ui.error.set_text(msg);
            ui.error.set_visible(true);
            ui.status.set_visible(false);
            ui.progress.set_visible(false);
            ui.entry.set_sensitive(true);
            primary.set_sensitive(true);
        };

        while let Ok(msg) = rx.recv().await {
            match msg {
                Msg::Downloading => ui.status.set_text(t!("Downloading…")),
                Msg::Progress(f) => {
                    pulsing.set(false);
                    ui.progress.set_fraction(f.clamp(0.0, 1.0));
                }
                Msg::Duplicate(pin) => {
                    // Re-scan rather than trust an index captured before the
                    // download: cheap, and immune to the library moving.
                    match library_file_names(&state)
                        .into_iter()
                        .find(|(_, n)| n.contains(&pin))
                    {
                        Some((idx, _)) => finish(idx, false),
                        None => fail(
                            "That wallpaper is in your library but its file has moved. \
                             Set it from the grid instead.",
                        ),
                    }
                    break;
                }
                Msg::Done(Ok((path, title))) => {
                    let mut e = if library::is_video(&path) {
                        LibraryEntry::new_video(path)
                    } else {
                        LibraryEntry::new_image(path)
                    };
                    // The pin's own title reads better than the CDN file name.
                    if let Some(t) = title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                        e.name = t.to_string();
                    }
                    e.generate_thumbnail();
                    let idx = {
                        let mut s = state.borrow_mut();
                        s.entries.push(e);
                        save_entries(&s.entries).ok();
                        s.entries.len() - 1
                    };
                    finish(idx, true);
                    break;
                }
                Msg::Done(Err(msg)) => {
                    crate::telemetry::event(
                        "add_from_link",
                        serde_json::json!({ "ok": false, "source": source, "onboarding": true }),
                    );
                    // `linkresolve` and `download` both return user-readable
                    // messages ("no network", "that pin.it link didn't lead to
                    // a pin"), so they go straight on screen.
                    fail(&msg);
                    break;
                }
            }
        }
    });
}

/// The what's-new flow: a stepper through [`ONBOARDING_STEPS`], shown once per
/// [`ONBOARDING_VERSION`] and reachable afterwards from the command palette.
///
/// Every step has to be *seen*. An individual step can be skipped — Skip moves
/// to the next one without acting — but the flow itself cannot be dismissed
/// early: there is no close button and no "skip all". The cost of that is one
/// extra click for a user who wants none of it; the benefit is that "this
/// install has been shown what changed" stays true rather than nearly true.
///
/// Nothing is written until the last step is finished. See [`ONBOARDING_STEPS`].
pub(crate) fn show_onboarding_dialog(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) {
    let (dialog, content) = glass_dialog(window, t!("What’s new in Fresco"), 520, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(4);
    inner.set_margin_bottom(22);

    let indicator = overline(&tf!(
        "Step {current} of {total}",
        "current" => "1",
        "total" => ONBOARDING_STEPS.len().to_string()
    ));
    inner.append(&indicator);

    let pages = gtk4::Stack::new();
    pages.set_transition_type(gtk4::StackTransitionType::SlideLeft);
    pages.set_transition_duration(180);
    inner.append(&pages);

    // ── Step 1: add from a link ──────────────────────────────────────────────
    let link_page = onboarding_page(
        &ONBOARDING_STEPS[0],
        "Paste a Pinterest pin — a pin.it or pinterest.com link — or a direct \
         link to a video, GIF or image. Fresco downloads it, adds it to your \
         library and sets it.",
        &[],
    );
    let link_ui = LinkStep {
        entry: gtk4::Entry::new(),
        error: gtk4::Label::new(None),
        status: gtk4::Label::new(None),
        progress: gtk4::ProgressBar::new(),
    };
    link_ui
        .entry
        .set_placeholder_text(Some(t!("Paste a Pinterest or direct video/image link")));
    // Pre-filled, not merely suggested: pressing the button uses this pin,
    // typing replaces it. The clipboard is left alone — unlike "Add from link",
    // this dialog opens on its own, before the user has copied anything.
    link_ui.entry.set_text(ONBOARDING_SAMPLE_LINK);
    link_page.append(&link_ui.entry);

    link_ui.error.add_css_class("error");
    link_ui.error.add_css_class("dim");
    link_ui.error.set_wrap(true);
    link_ui.error.set_xalign(0.0);
    link_ui.error.set_visible(false);
    link_page.append(&link_ui.error);

    link_ui.status.set_xalign(0.0);
    link_ui.status.set_visible(false);
    link_page.append(&link_ui.status);

    link_ui.progress.add_css_class("update-progress");
    link_ui.progress.set_hexpand(true);
    link_ui.progress.set_visible(false);
    link_page.append(&link_ui.progress);

    link_page.append(&onboarding_row(
        t!("Where this lives afterwards"),
        "The From Pinterest button in the bar at the bottom of the window, or \
         Ctrl+K \u{2192} \u{201c}Add from link\u{201d}.",
    ));

    let demo = gtk4::Button::with_label(t!("Watch the demo"));
    demo.add_css_class("flat");
    demo.set_halign(gtk4::Align::Start);
    demo.set_tooltip_text(Some(t!("Opens the walkthrough video in your browser")));
    demo.connect_clicked(|_| {
        // Counts intent only: once the browser has it, Fresco can't tell
        // whether it was watched. Judge the video by whether add_from_link
        // moves, not by this number.
        crate::telemetry::event("tutorial_opened", serde_json::json!({}));
        open_in_browser(TUTORIAL_URL);
    });
    link_page.append(&demo);

    pages.add_named(&link_page, Some(ONBOARDING_STEPS[0].id));

    // ── Step 2: widgets ──────────────────────────────────────────────────────
    // This screen may now say what gets drawn, not just where the settings are:
    // the daemon renders all four overlays. It deliberately does not show
    // pictures of them — they are drawn over the user's own wallpaper, so any
    // mockup here would be of a desktop that is not theirs.
    let widgets_page = onboarding_page(
        &ONBOARDING_STEPS[1],
        "Widgets draw on top of the wallpaper, underneath your windows. There \
         are four, all off until you turn them on.",
        &[
            (
                t!("Lyrics"),
                "The current line of the song that is playing. It reads .lrc \
                 files saved beside your music, and needs a media player that \
                 reports what it is playing over MPRIS. Style, position, text \
                 size and a sync offset are all adjustable.",
            ),
            (
                t!("Clock"),
                "The time, in one of five themes — Digital, Minimal, Segment, \
                 Stacked and Wordy — with position, text size, 24-hour time, \
                 date and seconds.",
            ),
            (
                t!("Visualiser and disc"),
                "A spectrum that moves with whatever is playing, and the album \
                 art as a spinning record. Both follow the same media player \
                 the lyrics do.",
            ),
            (
                t!("Where the settings are"),
                "Open the app menu (Ctrl+,), choose Advanced…, and scroll to \
                 Widget appearance and the four groups below it. Widget \
                 appearance sets the palette for all of them at once.",
            ),
        ],
    );
    pages.add_named(&widgets_page, Some(ONBOARDING_STEPS[1].id));

    // ── Footer ───────────────────────────────────────────────────────────────
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    footer.set_margin_top(4);
    let skip = gtk4::Button::with_label(t!("Skip"));
    skip.add_css_class("flat");
    skip.set_tooltip_text(Some(t!("Go to the next step without doing anything")));
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let primary = gtk4::Button::with_label(ONBOARDING_STEPS[0].primary);
    primary.add_css_class("suggested-action");
    footer.append(&skip);
    footer.append(&spacer);
    footer.append(&primary);
    inner.append(&footer);

    content.append(&inner);

    // Which step we're on, whether step 1's wallpaper landed, and whether the
    // flow is over. All of it lives and dies with this dialog — none of it is
    // persisted, by design (see ONBOARDING_STEPS).
    let step = Rc::new(Cell::new(0usize));
    let link_done = Rc::new(Cell::new(false));
    let finished = Rc::new(Cell::new(false));
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let render: Rc<dyn Fn()> = {
        let pages = pages.clone();
        let indicator = indicator.clone();
        let skip = skip.clone();
        let primary = primary.clone();
        let step = step.clone();
        let link_done = link_done.clone();
        Rc::new(move || {
            let i = step.get();
            let current = &ONBOARDING_STEPS[i];
            pages.set_visible_child_name(current.id);
            indicator.set_label(
                &tf!(
                    "Step {current} of {total}",
                    "current" => (i + 1).to_string(),
                    "total" => ONBOARDING_STEPS.len().to_string()
                )
                .to_uppercase(),
            );
            // Skip means "advance without acting". The last step has nothing to
            // advance to and its primary button is already the no-op finish, so
            // showing both would be two buttons doing the same thing.
            skip.set_visible(i + 1 < ONBOARDING_STEPS.len());
            // Once a step's action has succeeded its button stops being the
            // action and becomes the way forward — nobody should be left
            // staring at a button that repeats work they already did.
            primary.set_label(match current.primary_done {
                Some(label) if link_done.get() => t!(label),
                _ => t!(current.primary),
            });
        })
    };

    let advance: Rc<dyn Fn()> = {
        let step = step.clone();
        let render = render.clone();
        let finished = finished.clone();
        let dialog = dialog.clone();
        let state = state.clone();
        let cancel = cancel.clone();
        Rc::new(move || {
            let i = step.get();
            if ONBOARDING_STEPS[i].end == StepEnd::Advance {
                step.set(i + 1);
                render();
                return;
            }
            // Last step finished — the one and only place the flow is recorded.
            complete_onboarding(&state);
            // Same convention as "Add from link": a transfer does not outlive
            // the window that asked for it.
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            finished.set(true);
            dialog.set_deletable(true);
            dialog.close();
        })
    };

    {
        let advance = advance.clone();
        skip.connect_clicked(move |_| advance());
    }
    {
        let step = step.clone();
        let link_done = link_done.clone();
        let render = render.clone();
        let state = state.clone();
        let ui = link_ui.clone();
        let cancel = cancel.clone();
        primary.connect_clicked(move |btn| {
            let current = &ONBOARDING_STEPS[step.get()];
            // A step with an action runs it; once it has succeeded (or on any
            // step without one) the same button moves the flow along.
            if current.primary_done.is_some() && !link_done.get() {
                run_link_step(&state, &ui, btn, &link_done, &render, &cancel);
                return;
            }
            advance();
        });
    }
    {
        // Enter in the link box does what the primary button does — routed
        // through the button so it goes past the same "already set?" check.
        let primary = primary.clone();
        link_ui
            .entry
            .connect_activate(move |_| primary.emit_clicked());
    }

    // Non-closable until the flow is over. `set_deletable(false)` takes the
    // close button out of the header bar; `close-request` is what actually
    // holds the line, because the button is far from the only way to ask a
    // window to close (Escape, the window manager, an alt-F4). Skip is always
    // available, so this can never trap anyone — it costs one click per step,
    // not an escape hatch.
    dialog.set_deletable(false);
    {
        let finished = finished.clone();
        dialog.connect_close_request(move |_| {
            if finished.get() {
                glib::Propagation::Proceed
            } else {
                glib::Propagation::Stop
            }
        });
    }

    render();
    dialog.present();
    // Focus last: the pre-filled link is selected, so the first keystroke
    // replaces it and Enter alone accepts it.
    link_ui.entry.grab_focus();
    link_ui.entry.select_region(0, -1);
}

/// "What can Fresco do?" — a compact feature tour. Users kept missing features
/// (right-click menus, double-click editing, the link importer), so every
/// capability gets one line + where to find it. Opens from the menu, and once
/// automatically on a fresh install.
pub(crate) fn show_tour_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    {
        let mut s = state.borrow_mut();
        if !s.config.tour_shown {
            s.config.tour_shown = true;
            s.config.save().ok();
        }
    }
    let (dialog, content) = glass_dialog(window, t!("What can Fresco do?"), 520, 560);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    list.set_margin_start(24);
    list.set_margin_end(24);
    list.set_margin_top(10);
    list.set_margin_bottom(24);

    let rows: &[(&str, &str)] = &[
        (
            t!("Set anything"),
            t!("Videos, GIFs, images, slideshows, and playlists — click a card to set it."),
        ),
        (
            t!("Add from a link"),
            t!("The link button imports a Pinterest pin or any direct video/image URL — no downloads needed."),
        ),
        (
            t!("Preview, rotate & crop"),
            t!("Double-click any card (or use its Edit button) to adjust before setting."),
        ),
        (
            t!("Per-monitor wallpapers"),
            t!("Right-click a card → “Set on <display>” for different wallpapers per screen."),
        ),
        (
            t!("Day & night schedules"),
            t!("Two wallpapers on a timer — under Advanced in the menu."),
        ),
        (
            t!("Wallpaper catalog"),
            t!("Menu → “Browse wallpapers…” for curated, licensed picks in two clicks."),
        ),
        (
            t!("Hover to preview"),
            t!("Hover a video card and it plays silently in place."),
        ),
        (
            t!("Keyboard shortcuts"),
            t!("Ctrl+K command palette · Ctrl+F search · Ctrl+, menu · Ctrl+Q quit."),
        ),
    ];
    for (title, body) in rows {
        let row = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let t = gtk4::Label::new(Some(title));
        t.add_css_class("dialog-heading");
        t.set_xalign(0.0);
        let b = gtk4::Label::new(Some(body));
        b.add_css_class("dim");
        b.set_wrap(true);
        b.set_xalign(0.0);
        row.append(&t);
        row.append(&b);
        list.append(&row);
    }

    scroll.set_child(Some(&list));
    content.append(&scroll);
    dialog.present();
}

fn show_feedback_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    {
        let mut s = state.borrow_mut();
        s.config.feedback_prompted = true; // ask at most once
        s.config.save().ok();
    }

    let (dialog, content) = glass_dialog(window, t!("Feedback"), 420, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    let heading = gtk4::Label::new(Some(t!("Enjoying Fresco?")));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    inner.append(&heading);

    let prompt = gtk4::Label::new(Some(t!(
        "Your rating is anonymous. A quick note helps us know what to work on next."
    )));
    prompt.add_css_class("dialog-sub");
    prompt.set_wrap(true);
    prompt.set_xalign(0.0);
    inner.append(&prompt);

    let comment = gtk4::Entry::new();
    comment.set_placeholder_text(Some(t!("Anything we should know? (optional)")));
    comment.set_margin_top(4);
    inner.append(&comment);

    // The reason a 👎 used to be unactionable: there was nobody to ask what
    // broke. Ticking this attaches the anonymous support ticket to the row and
    // opens a thread, so the maintainer can answer inside the app. Default on,
    // because someone reporting a problem usually wants it solved — and off is
    // one click away, with the consequence spelled out rather than implied.
    let allow_reply =
        gtk4::CheckButton::with_label(t!("Let the maintainer reply (stays anonymous)"));
    allow_reply.set_active(true);
    allow_reply.set_margin_top(2);
    inner.append(&allow_reply);

    let reply_note = gtk4::Label::new(Some(t!(
        "They see your message and your setup, never who you are. Untick to send the rating with no way to reach you."
    )));
    reply_note.add_css_class("dialog-sub");
    reply_note.set_wrap(true);
    reply_note.set_xalign(0.0);
    reply_note.set_margin_start(26);
    inner.append(&reply_note);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_margin_top(6);
    let later = gtk4::Button::with_label(t!("Not now"));
    later.add_css_class("flat");
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let down = gtk4::Button::new();
    down.set_child(Some(&button_content("face-sad-symbolic", t!("Not great"))));
    down.add_css_class("feedback-btn");
    let up = gtk4::Button::new();
    up.set_child(Some(&button_content(
        "face-laugh-symbolic",
        t!("Loving it"),
    )));
    up.add_css_class("feedback-btn");
    up.add_css_class("suggested-action");
    buttons.append(&later);
    buttons.append(&spacer);
    buttons.append(&down);
    buttons.append(&up);
    inner.append(&buttons);

    content.append(&inner);

    {
        let comment = comment.clone();
        let state = state.clone();
        let d = dialog.clone();
        let allow = allow_reply.clone();
        up.connect_clicked(move |_| {
            submit_feedback_async(1, &comment, allow.is_active(), &state);
            d.close();
        });
    }
    {
        let comment = comment.clone();
        let state = state.clone();
        let d = dialog.clone();
        let win = window.clone();
        let allow = allow_reply.clone();
        down.connect_clicked(move |_| {
            let note = comment.text().to_string();
            let replyable = allow.is_active();
            submit_feedback_async(-1, &comment, replyable, &state);
            d.close();
            // With a reply channel open, the in-app thread IS the two-way
            // channel and needs no GitHub account, so pushing them to the
            // issue tracker on top of it would be asking twice for the same
            // thing. Only offer the issue when they declined the reply, which
            // is the case this prompt was written for.
            if !replyable {
                show_issue_prompt(&win, &note);
            }
        });
    }
    {
        let d = dialog.clone();
        later.connect_clicked(move |_| d.close());
    }

    dialog.present();
}

const ISSUES_URL: &str = "https://github.com/DibbayajyotiRoy/fresco/issues";
const NEW_ISSUE_URL: &str = "https://github.com/DibbayajyotiRoy/fresco/issues/new";

/// Follow-up shown right after a 👎, offering to open a pre-filled GitHub issue.
///
/// Feedback is submitted anonymously (no install id, no address), so the row we
/// receive is a dead end: we cannot ask which compositor, which wallpaper, or
/// what the log said. This dialog is the hand-off from "we heard you" to "we can
/// actually fix it" — declining costs the user nothing, the 👎 is already sent.
fn show_issue_prompt(window: &adw::ApplicationWindow, note: &str) {
    let (dialog, content) = glass_dialog(window, t!("Report a problem"), 420, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    let heading = gtk4::Label::new(Some(t!("Sorry about that")));
    heading.add_css_class("dialog-heading");
    heading.set_xalign(0.0);
    inner.append(&heading);

    let body = gtk4::Label::new(Some(
        "Thanks for telling us. Your rating is anonymous, though, so we can't \
         write back or ask what went wrong. If you have a minute, opening an \
         issue gives us enough to actually dig in. Your note and system info \
         are already filled in.",
    ));
    body.add_css_class("dialog-sub");
    body.set_wrap(true);
    body.set_xalign(0.0);
    inner.append(&body);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_margin_top(6);
    let later = gtk4::Button::with_label(t!("Not now"));
    later.add_css_class("flat");
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let open = gtk4::Button::with_label(t!("Tell us more"));
    open.set_tooltip_text(Some(t!(
        "Opens a pre-filled report on GitHub in your browser"
    )));
    open.add_css_class("suggested-action");
    buttons.append(&later);
    buttons.append(&spacer);
    buttons.append(&open);
    inner.append(&buttons);

    content.append(&inner);

    {
        let d = dialog.clone();
        let note = note.to_string();
        open.connect_clicked(move |_| {
            let _ = gio::AppInfo::launch_default_for_uri(
                &issue_url(&note),
                None::<&gio::AppLaunchContext>,
            );
            d.close();
        });
    }
    {
        let d = dialog.clone();
        later.connect_clicked(move |_| d.close());
    }

    dialog.present();
}

/// GitHub "new issue" URL pre-filled with the user's note and their environment.
///
/// The environment block is the whole point: without it a report says only "it
/// failed", since the feedback table's `os` column is a compile-time constant.
fn issue_url(note: &str) -> String {
    let note = note.trim();
    // The title is what we triage from, so lead with the user's own words;
    // `chars` (not bytes) so a truncation can't split a multi-byte character.
    let title = if note.is_empty() {
        "Bug report".to_string()
    } else {
        let first = note.lines().next().unwrap_or(note).trim();
        let short: String = first.chars().take(70).collect();
        if short.chars().count() < first.chars().count() {
            format!("{short}…")
        } else {
            short
        }
    };

    let mut body = String::from("**What happened**\n\n");
    if note.is_empty() {
        body.push_str("<!-- Describe the problem here. -->\n");
    } else {
        body.push_str(note);
        body.push('\n');
    }
    body.push_str("\n**Steps to reproduce**\n\n1. \n2. \n\n**Environment**\n\n");
    body.push_str(&crate::telemetry::env_summary());
    body.push_str(
        "\n\n**Diagnostics**\n\n\
         <!-- Thanks for taking the time to report this. If you can, paste the \
         output of these two commands below. It usually tells us the cause \
         right away:\n\
         \n\
         fresco doctor\n\
         fresco logs 200\n\
         -->\n",
    );

    // Escape every reserved character: the note is arbitrary user text and
    // rides in a query value.
    let esc = |s: &str| glib::Uri::escape_string(s, None, false).to_string();
    format!(
        "{NEW_ISSUE_URL}?labels=bug&title={}&body={}",
        esc(&title),
        esc(&body)
    )
}

/// A release-availability announcement ("Fresco vX.Y.Z is available") duplicates
/// the in-app update banner (see `updates.rs`, driven by GitHub releases), so we
/// don't ALSO surface it as a bottom toast. Genuine announcements (other titles)
/// still toast normally.
/// Anonymous conversation with the maintainer.
///
/// Deliberately not a support portal: no account, no email, no ticket number to
/// quote. The user writes, the maintainer answers, and neither learns who the
/// other is — the thread is addressed by a random ticket held only by this
/// install (see [`crate::support`]).
///
/// Messages load in the background; the dialog opens immediately with a
/// spinner rather than blocking the UI thread on a network round trip.
fn show_support_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let (dialog, content) = glass_dialog(window, t!("Message the maintainer"), 520, 560);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    inner.set_margin_start(20);
    inner.set_margin_end(20);
    inner.set_margin_top(8);
    inner.set_margin_bottom(18);
    inner.set_vexpand(true);

    let prompt = gtk4::Label::new(Some(t!(
        "Write to the person who makes Fresco. This is anonymous in both directions: they never learn who you are, only what you tell them."
    )));
    prompt.add_css_class("dialog-sub");
    prompt.set_wrap(true);
    prompt.set_xalign(0.0);
    inner.append(&prompt);

    // Transcript, in a scroller that grows with the dialog.
    let thread_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    thread_box.set_margin_top(4);
    thread_box.set_margin_bottom(4);
    let scroller = gtk4::ScrolledWindow::new();
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_vexpand(true);
    scroller.set_child(Some(&thread_box));
    inner.append(&scroller);

    let status = gtk4::Label::new(Some(t!("Loading…")));
    status.add_css_class("dialog-sub");
    status.set_xalign(0.0);
    thread_box.append(&status);

    // Composer.
    let entry = gtk4::TextView::new();
    entry.set_wrap_mode(gtk4::WrapMode::WordChar);
    entry.set_top_margin(8);
    entry.set_bottom_margin(8);
    entry.set_left_margin(8);
    entry.set_right_margin(8);
    let entry_frame = gtk4::ScrolledWindow::new();
    entry_frame.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    entry_frame.set_min_content_height(72);
    entry_frame.set_max_content_height(140);
    entry_frame.add_css_class("frame");
    entry_frame.set_child(Some(&entry));
    inner.append(&entry_frame);

    // Attaching the environment is a choice, and the exact text is shown
    // beneath the checkbox so "what am I sending" is never a guess.
    let env_text = crate::telemetry::env_summary();
    let attach = gtk4::CheckButton::with_label(t!("Attach my setup (helps diagnose)"));
    attach.set_active(true);
    inner.append(&attach);

    let env_preview = gtk4::Label::new(Some(&env_text));
    env_preview.add_css_class("dialog-sub");
    env_preview.set_xalign(0.0);
    env_preview.set_wrap(true);
    env_preview.set_margin_start(26);
    inner.append(&env_preview);
    {
        let env_preview = env_preview.clone();
        attach.connect_toggled(move |c| env_preview.set_visible(c.is_active()));
    }

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_margin_top(4);
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let close = gtk4::Button::with_label(t!("Close"));
    close.add_css_class("flat");
    let send = gtk4::Button::with_label(t!("Send"));
    send.add_css_class("suggested-action");
    buttons.append(&close);
    buttons.append(&spacer);
    buttons.append(&send);
    inner.append(&buttons);

    /// One message bubble, aligned by who sent it.
    fn bubble(message: &crate::support::Message) -> gtk4::Box {
        let row = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let who = gtk4::Label::new(Some(if message.is_maintainer() {
            t!("Fresco maintainer")
        } else {
            t!("You")
        }));
        who.add_css_class("overline");
        let body = gtk4::Label::new(Some(&message.body));
        body.set_wrap(true);
        body.set_xalign(0.0);
        body.set_selectable(true);
        if message.is_maintainer() {
            who.set_xalign(0.0);
            body.add_css_class("dialog-heading");
        } else {
            who.set_xalign(0.0);
            body.add_css_class("dialog-sub");
        }
        row.append(&who);
        row.append(&body);
        row
    }

    // Load the transcript off the UI thread.
    let refresh = {
        let thread_box = thread_box.clone();
        let scroller = scroller.clone();
        move || {
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let _ = tx.send_blocking(crate::support::poll().unwrap_or_default());
            });
            let thread_box = thread_box.clone();
            let scroller = scroller.clone();
            glib::spawn_future_local(async move {
                let Ok(messages) = rx.recv().await else {
                    return;
                };
                while let Some(child) = thread_box.first_child() {
                    thread_box.remove(&child);
                }
                if messages.is_empty() {
                    let empty = gtk4::Label::new(Some(t!(
                        "No messages yet. Whatever you send starts a thread only you and the maintainer can see."
                    )));
                    empty.add_css_class("dialog-sub");
                    empty.set_wrap(true);
                    empty.set_xalign(0.0);
                    thread_box.append(&empty);
                } else {
                    for m in &messages {
                        thread_box.append(&bubble(m));
                    }
                    // Everything on screen counts as seen, so the launch check
                    // stops announcing a reply the user has now read.
                    crate::support::set_seen_count(messages.len());
                    crate::support::mark_read();
                }
                // Pin to the newest message.
                glib::idle_add_local_once(move || {
                    let adj = scroller.vadjustment();
                    adj.set_value(adj.upper() - adj.page_size());
                });
            });
        }
    };
    refresh();

    {
        let entry = entry.clone();
        let attach = attach.clone();
        let send_btn = send.clone();
        let refresh = refresh.clone();
        send.connect_clicked(move |_| {
            let buffer = entry.buffer();
            let (start, end) = buffer.bounds();
            let body = buffer.text(&start, &end, false).trim().to_string();
            if body.is_empty() {
                return;
            }
            // Clear and disable immediately: a second click before the POST
            // returns would send the message twice.
            buffer.set_text("");
            send_btn.set_sensitive(false);
            let env = attach.is_active().then(crate::telemetry::env_summary);

            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let _ = tx.send_blocking(crate::support::send(&body, env.as_deref()).is_ok());
            });
            let send_btn = send_btn.clone();
            let refresh = refresh.clone();
            glib::spawn_future_local(async move {
                let ok = rx.recv().await.unwrap_or(false);
                send_btn.set_sensitive(true);
                if !ok {
                    log::debug!("support send failed");
                }
                refresh();
            });
        });
    }

    {
        let dialog = dialog.clone();
        close.connect_clicked(move |_| dialog.close());
    }

    let _ = &state;
    content.append(&inner);
    dialog.present();
    entry.grab_focus();
}

/// On launch, tell the user if the maintainer wrote back while Fresco was
/// closed. Silent when there is no thread or no new message — this must never
/// become a reason to open a dialog at people.
fn check_support_replies(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    if !crate::support::has_thread() {
        return;
    }
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(crate::support::unread_replies());
    });
    let window = window.clone();
    glib::spawn_future_local(async move {
        if rx.recv().await.unwrap_or(0) == 0 {
            return;
        }
        show_support_dialog(&window, state);
    });
}

fn is_release_announcement(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    t.contains("fresco") && t.contains("is available")
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
            list.into_iter().find(|n| {
                !s.config.seen_notifications.contains(&n.id) && !is_release_announcement(&n.title)
            })
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
    toast.set_button_label(Some(t!("Details")));
    toast.set_action_name(Some("win.fresco-notif-details"));
    toast.set_timeout(0);
    state.borrow().toast.add_toast(toast);
}

fn show_notification_modal(window: &adw::ApplicationWindow, notif: &crate::supabase::Notification) {
    let (dialog, content) = glass_dialog(window, &notif.title, 440, -1);

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
        let open = gtk4::Button::with_label(t!("Open link"));
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
    dialog.present();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn release_announcements_are_filtered_but_others_toast() {
        // These duplicate the update banner → no bottom toast.
        assert!(is_release_announcement("Fresco v1.1.34 is available"));
        assert!(is_release_announcement("fresco 2.0 is available now"));
        // Genuine announcements still toast.
        assert!(!is_release_announcement(
            "New wallpapers added to the catalog"
        ));
        assert!(!is_release_announcement("Scheduled maintenance tonight"));
    }

    /// The 👎 note is arbitrary user text pasted into a URL query value. A raw
    /// `&` or `#` would truncate the body at GitHub's end, silently dropping
    /// exactly the environment block this feature exists to deliver.
    #[test]
    fn issue_url_escapes_the_note_and_carries_the_environment() {
        let url = issue_url("black & broken #1 on 100% zoom");
        assert!(url.starts_with(NEW_ISSUE_URL));
        // Everything after the first two separators must be escaped: the only
        // bare `&` is the one joining `title=` to `body=`.
        assert_eq!(url.matches('&').count(), 2, "note's `&` must be escaped");
        assert!(!url.contains('#'), "a bare `#` would cut the body off");
        assert!(
            url.contains("Fresco%3A"),
            "environment block must be present"
        );
    }

    /// An empty note must still produce a usable report — the environment block
    /// alone is worth more than the anonymous feedback row it follows.
    #[test]
    fn issue_url_handles_an_empty_note() {
        let url = issue_url("   ");
        assert!(url.contains("title=Bug%20report"));
        assert!(url.contains("Fresco%3A"));
    }

    /// Titles are truncated to 70 chars. Doing that by byte would panic on a
    /// multi-byte boundary — feedback arrives in every language.
    #[test]
    fn issue_title_truncates_on_character_boundaries() {
        let note = "。".repeat(200); // 3 bytes per char
        let url = issue_url(&note);
        assert!(url.contains("title="));
        // Only the first line is used, and it is capped well under the input.
        let title_len = issue_url("x".repeat(200).as_str())
            .split("title=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .map(|s| s.chars().filter(|c| *c == 'x').count())
            .unwrap();
        assert_eq!(title_len, 70);
    }

    /// Batch remove is keyed by id precisely so it stays correct when indices
    /// move — reordering the library between selecting and removing must still
    /// delete the chosen wallpapers and nothing else.
    #[test]
    fn batch_remove_follows_ids_not_positions() {
        let mut entries = vec![entry("/a.mp4"), entry("/b.mp4"), entry("/c.mp4")];
        let ids: std::collections::HashSet<String> =
            [entries[0].id.clone(), entries[2].id.clone()].into();

        entries.reverse(); // library changed underneath the selection
        let gone = drain_by_ids(&mut entries, &ids);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, Some(PathBuf::from("/b.mp4")));
        assert_eq!(gone.len(), 2);
    }

    #[test]
    fn batch_remove_ignores_unknown_ids_and_empty_sets() {
        let mut entries = vec![entry("/a.mp4")];
        assert!(drain_by_ids(&mut entries, &Default::default()).is_empty());
        assert!(drain_by_ids(&mut entries, &["nope".to_string()].into()).is_empty());
        assert_eq!(entries.len(), 1);
    }

    fn entry(path: &str) -> LibraryEntry {
        LibraryEntry {
            path: Some(PathBuf::from(path)),
            ..LibraryEntry::new_video(PathBuf::from(path))
        }
    }

    #[test]
    fn per_monitor_assign_touches_only_the_override() {
        let mut cfg = Config::default();
        cfg.wallpaper.path = Some(PathBuf::from("/default.mp4"));
        let before_default = cfg.wallpaper.clone();

        let e = entry("/side.mp4");
        assign_entry_to_monitor(&mut cfg, e.to_wallpaper(), "HDMI-1");

        assert_eq!(cfg.wallpaper, before_default, "default wallpaper untouched");
        assert_eq!(cfg.monitors.len(), 1);
        assert_eq!(
            cfg.monitors["HDMI-1"].path,
            Some(PathBuf::from("/side.mp4"))
        );
        assert!(cfg.enabled);

        clear_monitor_overrides(&mut cfg);
        assert!(cfg.monitors.is_empty());
        assert_eq!(cfg.wallpaper, before_default);
    }

    #[test]
    fn sync_wallpaper_follows_the_schedule() {
        use crate::config::{Schedule, ScheduleMode};
        let mk = |p: &str| entry(p).to_wallpaper();
        let mut cfg = Config {
            schedule: Some(Schedule {
                mode: ScheduleMode::Daynight,
                day: Some(mk("/day.mp4")),
                night: Some(mk("/night.mp4")),
                day_start: "07:00".into(),
                night_start: "19:00".into(),
                lat: None,
                lon: None,
                at: vec![],
            }),
            ..Default::default()
        };
        sync_wallpaper_to_schedule(&mut cfg);
        let got = cfg.wallpaper.path.clone().unwrap();
        assert!(got.as_os_str() == "/day.mp4" || got.as_os_str() == "/night.mp4");
        // Self-consistency with the engine for the same instant.
        use chrono::Offset as _;
        let now = chrono::Local::now();
        let off = now.offset().fix().local_minus_utc() / 60;
        let want = crate::schedule::desired(cfg.schedule.as_ref().unwrap(), now.naive_local(), off)
            .unwrap()
            .path
            .clone()
            .unwrap();
        assert_eq!(got, want);
    }

    /// The lyric combos map selection index → enum through these tables, so a
    /// variant missing from one of them would silently become unreachable in
    /// the GUI (and `table_index` would park the combo on the wrong entry).
    #[test]
    fn lyric_label_tables_cover_every_variant() {
        for (i, (style, label)) in LYRIC_STYLES.iter().enumerate() {
            assert_eq!(table_index(&LYRIC_STYLES, *style), i as u32, "{label}");
        }
        for (i, (anchor, label)) in LYRIC_ANCHORS.iter().enumerate() {
            assert_eq!(table_index(&LYRIC_ANCHORS, *anchor), i as u32, "{label}");
        }
        // Both tables are exhaustive over their enums: every default must be
        // found, not fall through to index 0.
        assert_eq!(
            table_index(&LYRIC_ANCHORS, LyricAnchor::default()),
            7,
            "BottomCenter is the default and must select its own row"
        );
        assert_eq!(table_labels(&LYRIC_STYLES).len(), LYRIC_STYLES.len());
        assert_eq!(table_labels(&LYRIC_ANCHORS).len(), 9);
    }

    /// Same trap as the lyric tables, for the clock's theme combo.
    #[test]
    fn clock_label_table_covers_every_variant() {
        for (i, (theme, label)) in CLOCK_THEMES.iter().enumerate() {
            assert_eq!(table_index(&CLOCK_THEMES, *theme), i as u32, "{label}");
        }
        // The invariant that actually matters is about the *shown* table, since
        // that is what drives the combo: the default has to be offerable, or the
        // picker opens on some other row and the first edit writes that other
        // theme back over a config the user never touched.
        assert!(
            CLOCK_THEMES_SHOWN
                .iter()
                .any(|(t, _)| *t == ClockThemeCfg::default()),
            "the default theme must be one the picker offers"
        );
        assert_eq!(
            table_index(&CLOCK_THEMES_SHOWN, ClockThemeCfg::default()),
            0,
            "the default must select its own row in the shown table"
        );
        // The clock reuses the lyric anchor table, so a variant missing there
        // would silently be unreachable from *both* groups.
        assert_eq!(
            table_index(&LYRIC_ANCHORS, Clock::default().anchor),
            2,
            "the default clock anchor (top right) must select its own row"
        );
    }

    /// Same trap as the lyric and clock tables, for the widget palette combo,
    /// plus one guard the older tables cannot make.
    ///
    /// Iterating a table proves only that it holds no duplicates. A variant
    /// *missing* from it is the dangerous case and is invisible to that check:
    /// [`table_index`] answers 0 for anything it cannot find, so the combo
    /// opens on "Automatic" however the config actually reads, and the next
    /// selection writes that wrong value back. The exhaustive `match` below
    /// turns "someone added a `WidgetTheme` and forgot this table" from a
    /// silent mis-selection into a compile error.
    #[test]
    fn widget_theme_label_table_covers_every_variant() {
        for (i, (theme, label)) in WIDGET_THEMES.iter().enumerate() {
            assert_eq!(table_index(&WIDGET_THEMES, *theme), i as u32, "{label}");
        }
        assert_eq!(
            table_index(&WIDGET_THEMES, WidgetTheme::default()),
            0,
            "Automatic is the default and must select its own row"
        );

        for theme in [WidgetTheme::Auto, WidgetTheme::Light, WidgetTheme::Dark] {
            // Adding a variant stops this compiling, which is the whole point.
            match theme {
                WidgetTheme::Auto | WidgetTheme::Light | WidgetTheme::Dark => {}
            }
            assert!(
                WIDGET_THEMES.iter().any(|(t, _)| *t == theme),
                "{theme:?} is missing from WIDGET_THEMES and would select row 0"
            );
        }

        // The write-back path the combo actually uses, mirrored: row index in,
        // palette out. A table reordered without touching the handler would
        // otherwise store whatever sat at the old index.
        for (i, (theme, label)) in WIDGET_THEMES.iter().enumerate() {
            let back = WIDGET_THEMES
                .get(i)
                .map_or_else(WidgetTheme::default, |(t, _)| *t);
            assert_eq!(back, *theme, "{label}");
        }

        assert_eq!(table_labels(&WIDGET_THEMES).len(), WIDGET_THEMES.len());
    }

    /// Same trap as the lyric and clock tables, for the visualiser's style
    /// combo — and for the anchor table, which all four widget groups now share.
    #[test]
    fn visualizer_and_disc_label_tables_cover_every_variant() {
        for (i, (style, label)) in VISUAL_STYLES.iter().enumerate() {
            assert_eq!(table_index(&VISUAL_STYLES, *style), i as u32, "{label}");
        }
        assert_eq!(
            table_index(&VISUAL_STYLES, VisualizerStyleCfg::default()),
            0,
            "Bars is the default and must select its own row"
        );
        // The visualiser style list must name the same looks, in the same
        // order, as the renderer's own `VisualStyle::ALL` — otherwise the
        // picker offers a shape the renderer does not draw, or offers them in
        // an order that makes the labels point at the wrong style.
        assert_eq!(
            VISUAL_STYLES.len(),
            crate::visualizer::VisualStyle::ALL.len(),
            "GUI style list must cover every renderer style"
        );
        // Both new groups reuse the lyric anchor table, so a variant missing
        // there would be unreachable from four groups rather than two.
        assert_eq!(
            table_index(&LYRIC_ANCHORS, Visualizer::default().anchor),
            7,
            "the default visualiser anchor (bottom center) must select its own row"
        );
        assert_eq!(
            table_index(&LYRIC_ANCHORS, Disc::default().anchor),
            8,
            "the default disc anchor (bottom right) must select its own row"
        );
    }

    /// The combo names the user picks from and the names the renderer knows are
    /// two hand-written lists in two modules. Pin them together — otherwise
    /// renaming a theme in `clock.rs` leaves the picker offering the old word,
    /// or reordering `ClockTheme::ALL` leaves the two lists disagreeing about
    /// which look is which.
    #[test]
    fn clock_theme_labels_match_the_renderer() {
        let ours = table_labels(&CLOCK_THEMES);
        let theirs: Vec<&str> = crate::clock::ClockTheme::ALL
            .iter()
            .map(|t| t.label())
            .collect();
        assert_eq!(ours, theirs, "GUI theme list must match clock::ClockTheme");
    }

    /// Completion is all-or-nothing, and the whole guarantee rests on the shape
    /// of this table: exactly one step ends the flow, it is the last one, and
    /// nothing before it can. Get that wrong in either direction and the flow
    /// either never records itself (shown again forever) or records itself
    /// before the user has seen every step.
    #[test]
    fn onboarding_finishes_only_on_the_last_step() {
        // A const array's emptiness is known at compile time, so assert it there —
        // a runtime `assert!` on it is a no-op clippy rightly rejects.
        const {
            assert!(
                !ONBOARDING_STEPS.is_empty(),
                "a flow with no steps would never complete, and would reopen every launch"
            )
        };
        let (last, rest) = ONBOARDING_STEPS.split_last().expect("non-empty above");
        assert_eq!(
            last.end,
            StepEnd::Complete,
            "the last step must be the one that completes onboarding"
        );
        assert!(
            rest.iter().all(|s| s.end == StepEnd::Advance),
            "no step before the last may complete onboarding"
        );

        // The ids double as gtk4::Stack child names; a duplicate would silently
        // show the wrong page.
        let mut ids: Vec<&str> = ONBOARDING_STEPS.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ONBOARDING_STEPS.len(), "step ids must be unique");

        // `primary_done` is what the primary button consults to decide between
        // running a step's action and advancing, so only steps that actually
        // have one may set it — and the finishing step never does.
        assert_eq!(
            ONBOARDING_STEPS
                .iter()
                .filter(|s| s.primary_done.is_some())
                .count(),
            1,
            "the link step is the only one with an action of its own"
        );
        assert!(
            ONBOARDING_STEPS[0].primary_done.is_some(),
            "step 1 is the link step"
        );
        assert!(
            last.primary_done.is_none(),
            "the finishing step takes no action, so its button never changes label"
        );

        // 0 is config.rs's "never shown" sentinel; a flow pinned there would be
        // completed and immediately un-completed.
        const { assert!(ONBOARDING_VERSION > 0) };
    }

    #[test]
    fn the_colour_button_and_the_config_agree_on_a_colour() {
        // The picker is the only writer of these keys, so a rounding
        // disagreement between it and the config would show up as a colour that
        // drifts every time the dialog is reopened.
        for hex in ["#000000", "#FFFFFF", "#3584E4", "#FF3B6B", "#22D3EE"] {
            assert_eq!(rgba_to_hex(&hex_to_rgba(hex)), hex, "round trip {hex}");
        }
        // Anything the GDK parser cannot read lands on the same white the
        // renderers fall back to, rather than on transparent black.
        assert_eq!(rgba_to_hex(&hex_to_rgba("not a colour")), "#FFFFFF");
        assert_eq!(rgba_to_hex(&hex_to_rgba("")), "#FFFFFF");
        // GDK understands more spellings than the config file does; whatever it
        // reads must still leave here as a plain six-digit hex.
        let named = rgba_to_hex(&hex_to_rgba("red"));
        assert_eq!(named, "#FF0000");
    }

    #[test]
    fn the_gradient_combo_lists_every_mode_exactly_once() {
        // A combo that silently omits a variant is a setting nobody can reach,
        // and `table_index` would quietly select the first entry for it.
        for mode in [
            GradientMode::None,
            GradientMode::Linear,
            GradientMode::Spectrum,
        ] {
            let at = table_index(&VISUAL_GRADIENTS, mode) as usize;
            assert_eq!(VISUAL_GRADIENTS[at].0, mode, "{mode:?} is not in the combo");
        }
        assert_eq!(VISUAL_GRADIENTS.len(), 3);
        assert_eq!(table_labels(&VISUAL_GRADIENTS).len(), 3);
    }

    #[test]
    fn active_marker_covers_monitor_overrides() {
        let mut cfg = Config {
            enabled: true,
            ..Default::default()
        };
        cfg.wallpaper.path = Some(PathBuf::from("/default.mp4"));
        let side = entry("/side.mp4");
        assert!(!entry_is_active(&side, &cfg));
        assign_entry_to_monitor(&mut cfg, side.to_wallpaper(), "DP-2");
        assert!(entry_is_active(&side, &cfg), "override counts as active");
    }

    // ─── Folders, sorting and batch import ───────────────────────────────

    /// A video entry named after `name`, filed into `collection`.
    fn filed(name: &str, collection: Option<&str>, order: i64) -> LibraryEntry {
        LibraryEntry {
            collection: collection.map(str::to_string),
            order,
            ..LibraryEntry::new_video(PathBuf::from(format!("/{name}.mp4")))
        }
    }

    /// Same trap the lyric/clock tables have: a variant added to `SortMode`
    /// that never reaches the menu would silently select "My order" instead.
    #[test]
    fn sort_label_table_covers_every_variant() {
        let table = sort_table();
        assert_eq!(table.len(), library::SortMode::ALL.len());
        for (i, (mode, label)) in table.iter().enumerate() {
            assert_eq!(table_index(&table, *mode), i as u32, "{label}");
            assert!(!label.is_empty(), "every mode needs a menu label");
        }
        // The default must select its own row, not fall through to index 0.
        assert_eq!(table[0].0, library::SortMode::Manual);
        assert_eq!(table_index(&table, library::SortMode::default()), 0);
        // Two rows reading the same is a menu you cannot use.
        let mut labels: Vec<&str> = table.iter().map(|(_, l)| *l).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), library::SortMode::ALL.len());
    }

    /// Folders render in the user's folder order, with everything still loose
    /// collected at the bottom rather than dropped.
    #[test]
    fn sections_follow_the_folder_order_then_uncategorized() {
        let space = library::Collection::new("Space", 1);
        let nature = library::Collection::new("Nature", 0);
        let entries = vec![
            filed("a", Some(&space.id), 0),
            filed("b", Some(&nature.id), 0),
            filed("c", None, 0),
        ];
        let plans = plan_sections(
            &entries,
            &[space, nature],
            &library::LibraryView::default(),
            "",
        );
        let titles: Vec<&str> = plans.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, vec!["Nature", "Space", t!("Uncategorized")]);
        assert!(
            plans.iter().all(|p| p.reorderable),
            "each of these sections is exactly one ordering group"
        );
    }

    /// Nobody who never makes a folder may notice this feature exists.
    #[test]
    fn no_folders_keeps_the_pre_folder_kind_grouping() {
        let entries = vec![
            filed("clip", None, 0),
            LibraryEntry::new_image(PathBuf::from("/pic.png")),
        ];
        let plans = plan_sections(&entries, &[], &library::LibraryView::default(), "");
        let titles: Vec<&str> = plans.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, vec![t!("Images"), t!("Videos")]);
        assert!(
            plans.iter().all(|p| !p.reorderable),
            "a kind section slices one group in two: a move would jump a card \
             past a neighbour that is not on screen"
        );
    }

    /// "Largest file" split across three sections answers a question nobody
    /// asked, so a derived sort collapses the grid to one list.
    #[test]
    fn a_derived_sort_flattens_the_grid_and_hides_the_arrows() {
        let nature = library::Collection::new("Nature", 0);
        let inside = filed("b", Some(&nature.id), 0);
        let loose = filed("a", None, 0);
        let view = library::LibraryView {
            sort: library::SortMode::NameAsc,
            collection: None,
        };
        let plans = plan_sections(&[inside.clone(), loose.clone()], &[nature], &view, "");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].title, t!("All wallpapers"));
        assert!(!plans[0].reorderable, "a move would be instantly overruled");
        assert_eq!(plans[0].ids, vec![loose.id, inside.id]);
    }

    /// Opening one folder gives a flat, reorderable grid of just that folder —
    /// the "reorder them in each folder" half of the request.
    #[test]
    fn an_open_folder_is_flat_and_reorderable() {
        let nature = library::Collection::new("Nature", 0);
        let entries = vec![filed("b", Some(&nature.id), 1), filed("a", None, 0)];
        let view = library::LibraryView {
            sort: library::SortMode::Manual,
            collection: Some(nature.id.clone()),
        };
        let plans = plan_sections(&entries, std::slice::from_ref(&nature), &view, "");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].title, "Nature");
        assert_eq!(plans[0].ids.len(), 1, "the loose entry is out of scope");
        assert!(plans[0].reorderable);
    }

    /// Favorites keep their place at the top of every layout, and never grow
    /// arrows: the section mixes folders, so "one place earlier" is undefined.
    #[test]
    fn favorites_stay_on_top_and_are_never_reorderable() {
        let nature = library::Collection::new("Nature", 0);
        let fav = LibraryEntry {
            favorite: true,
            ..filed("star", Some(&nature.id), 0)
        };
        let plans = plan_sections(
            &[filed("plain", None, 0), fav],
            &[nature],
            &library::LibraryView::default(),
            "",
        );
        assert_eq!(plans[0].title, t!("Favorites"));
        assert!(!plans[0].reorderable);
    }

    /// The invariant behind "Select all": it must never tick a card that the
    /// open folder is hiding, or the next Remove deletes off-screen wallpapers.
    #[test]
    fn scoped_ids_respect_the_open_folder_and_the_search_box() {
        let space = library::Collection::new("Space", 0);
        let inside = filed("nebula", Some(&space.id), 0);
        let outside = filed("desk", None, 0);
        let entries = vec![inside.clone(), outside.clone()];

        let all = library::LibraryView::default();
        assert_eq!(scoped_ids(&entries, &all, "").len(), 2);

        let in_folder = library::LibraryView {
            sort: library::SortMode::Manual,
            collection: Some(space.id.clone()),
        };
        assert_eq!(scoped_ids(&entries, &in_folder, ""), vec![inside.id]);
        assert!(
            scoped_ids(&entries, &in_folder, "desk").is_empty(),
            "the search must narrow within the folder, not escape it"
        );

        let loose = library::LibraryView {
            sort: library::SortMode::Manual,
            collection: Some(SCOPE_UNCATEGORIZED.to_string()),
        };
        assert_eq!(scoped_ids(&entries, &loose, ""), vec![outside.id]);
    }

    /// A folder deleted underneath a saved scope must not leave the library
    /// filtered to an id nothing matches, with no visible way back.
    #[test]
    fn a_scope_pointing_at_a_deleted_folder_falls_back_to_everything() {
        let gone = library::Collection::new("Gone", 0);
        let view = library::LibraryView {
            sort: library::SortMode::Manual,
            collection: Some(gone.id.clone()),
        };
        assert_eq!(
            normalize_view(view.clone(), std::slice::from_ref(&gone)).collection,
            Some(gone.id.clone())
        );
        assert_eq!(normalize_view(view, &[]).collection, None);

        // The uncategorized pool matches no folder by design, and must survive.
        let loose = library::LibraryView {
            sort: library::SortMode::Manual,
            collection: Some(SCOPE_UNCATEGORIZED.to_string()),
        };
        assert_eq!(
            normalize_view(loose, &[]).collection,
            Some(SCOPE_UNCATEGORIZED.to_string())
        );
    }

    /// The number the import toast reports. Both kinds of duplicate count: one
    /// the library already holds, and one the same batch names twice.
    #[test]
    fn a_batch_import_counts_the_duplicates_it_skips() {
        let existing = vec![entry("/wall/a.mp4")];
        let picked = vec![
            PathBuf::from("/wall/a.mp4"),
            PathBuf::from("/wall/b.mp4"),
            PathBuf::from("/wall/b.mp4"),
        ];
        let (fresh, dupes) = library::partition_new(&existing, picked);
        assert_eq!(fresh, vec![PathBuf::from("/wall/b.mp4")]);
        assert_eq!(dupes.len(), 2);
        // …and the fresh ones become one wallpaper each, not one playlist.
        assert_eq!(library::entries_for_each(fresh).len(), 1);
    }
}
