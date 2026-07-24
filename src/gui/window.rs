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
    config::{Accent, Config, Fit, Kind, PowerSaving, Scaling, ThemeMode, Transition},
    APP_ID,
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
    theme::apply(config.accent, theme::is_dark());

    // Session capability drives the UI: every session gets the full app; the
    // limited ones (Wayland) also get an informational banner. No hard block.
    let capability = crate::capability::detect();

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
        update_banner_slot: None,
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
    if !state.borrow().config.telemetry_prompted {
        // Consent before anything else — telemetry stays fully off until the
        // user answers (telemetry::enabled() checks telemetry_prompted).
        let win_c = window.clone();
        let state_c = state.clone();
        glib::idle_add_local_once(move || show_telemetry_consent_dialog(&win_c, state_c));
    } else if PENDING_FEEDBACK.swap(false, std::sync::atomic::Ordering::Relaxed) {
        let win_fb = window.clone();
        let state_fb = state.clone();
        glib::idle_add_local_once(move || show_feedback_dialog(&win_fb, state_fb));
    } else if !state.borrow().config.tour_shown {
        // First launch: show the feature tour once, so right-click menus,
        // double-click editing, and the link importer don't go undiscovered.
        let win_t = window.clone();
        let state_t = state.clone();
        glib::idle_add_local_once(move || show_tour_dialog(&win_t, state_t));
    } else if state.borrow().config.onboarding_version < ONBOARDING_VERSION {
        // Existing user on a new version: they already sat through the tour,
        // so they'd never otherwise be shown the paste-a-link flow.
        let win_o = window.clone();
        let state_o = state.clone();
        let stack_o = stack.clone();
        glib::idle_add_local_once(move || {
            show_onboarding_dialog(&win_o, state_o, stack_o);
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
                show_toast(&state_d, "Drop video or image files to add them");
                return false;
            }
            add_media_paths(&state_d, &stack_d, paths, None);
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
            "On GNOME Wayland, wallpapers are shown as a static frame. For live playback, use an X11 session or a layer-shell compositor (COSMIC, Hyprland, Sway, KDE Plasma).",
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
    menu_btn.set_tooltip_text(Some("Menu"));
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
    search.set_placeholder_text(Some("Search wallpapers…"));
    search.set_margin_top(8);
    search.set_margin_bottom(2);
    // Cap the entry at a readable width instead of stretching edge-to-edge.
    let search_clamp = adw::Clamp::new();
    search_clamp.set_maximum_size(560);
    search_clamp.set_tightening_threshold(480);
    search_clamp.set_child(Some(&search));
    root.append(&search_clamp);

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
    let recent_label = overline("Recent");
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
    welcome.set_title("Drop videos or images here");
    welcome.set_description(Some(
        "Drag files onto the window, or add a video, GIF, image, or folder of images",
    ));
    welcome.set_vexpand(true);
    let welcome_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    welcome_actions.set_halign(gtk4::Align::Center);
    let welcome_btn = gtk4::Button::with_label("Add wallpapers");
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
    let welcome_browse = gtk4::Button::with_label("Browse catalog");
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

    // Labelled, brand-marked entry point. This was an unlabelled
    // `insert-link-symbolic` button and telemetry showed the feature at
    // literally zero uses — nobody recognised a generic chain-link glyph in a
    // footer as "paste a Pinterest link". The logo names the thing people
    // already have in their clipboard.
    let add_link_btn = gtk4::Button::new();
    add_link_btn.set_child(Some(&pinterest_button_content()));
    add_link_btn.set_tooltip_text(Some(
        "Paste a Pinterest or direct media link to set as wallpaper",
    ));
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
                add_folder_btn
                    .set_child(Some(&button_content("folder-new-symbolic", "Add folder")));
                add_btn.set_child(Some(&button_content("list-add-symbolic", "Add")));
            }
        }
    };
    add_btn.set_tooltip_text(Some("Add a wallpaper"));

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
        let bucket = bucket.clone();
        let count_label = count_label.clone();
        Rc::new(move || {
            // Searching an empty library is pointless: hide the field until
            // there's something to search.
            let n = state.borrow().entries.len();
            search.set_visible(n > 0);
            count_label.set_visible(n > 0);
            count_label.set_text(&if n == 1 {
                "1 wallpaper".to_string()
            } else {
                format!("{n} wallpapers")
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
    widgets.content.set_margin_start(side_margin);
    widgets.content.set_margin_end(side_margin);
    // Footer inset comes from the .footer-bar / .compact-layout CSS padding.
}

/// Header menu: appearance (theme mode + accent) and behavior switches.
fn build_menu_popover(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
) -> gtk4::Popover {
    let popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    popover_box.set_margin_top(6);
    popover_box.set_margin_bottom(6);
    popover_box.set_margin_start(6);
    popover_box.set_margin_end(6);
    // Compact menu column: wide enough for the longest switch row, no wider.
    popover_box.set_width_request(300);

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

    popover_box.append(&overline("Accent"));

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

    // Separator margins come from the .fresco-menu CSS.
    popover_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

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
    // Quick schedule pause — only shown when a schedule exists. Turning it off
    // here keeps the configured day/night setup (unlike Advanced's "Off",
    // which deletes it); users kept hunting for this switch.
    if state.borrow().config.schedule.is_some() {
        popover_box.append(&switch_row(
            "Day/night schedule",
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
    popover_box.append(&switch_row(
        "Share anonymous usage statistics",
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
        "Browser new-tab wallpaper (local)",
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
        "Lets the Fresco browser extension show your wallpaper on new tabs. Local-only (127.0.0.1).",
    ));
    popover_box.append(&bridge_row);

    popover_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let advanced_btn = menu_item("Advanced…");
    {
        let state_adv = state.clone();
        let win_adv = window.clone();
        advanced_btn.connect_clicked(move |_| {
            show_advanced_dialog(&win_adv, state_adv.clone());
        });
    }
    popover_box.append(&advanced_btn);

    let browse_btn = menu_item("Browse wallpapers…");
    {
        let state_b = state.clone();
        let win_b = window.clone();
        browse_btn.connect_clicked(move |_| {
            super::gallery::show_gallery_window(&win_b, state_b.clone());
        });
    }
    popover_box.append(&browse_btn);

    let url_btn = menu_item("Add from URL…");
    {
        let state_url = state.clone();
        let win_url = window.clone();
        url_btn.connect_clicked(move |_| {
            show_add_from_url_dialog(&win_url, state_url.clone());
        });
    }
    popover_box.append(&url_btn);

    let update_btn = menu_item("Check for updates");
    {
        let state_upd = state.clone();
        let win_upd = window.clone();
        update_btn.connect_clicked(move |_| {
            super::updates::check_for_updates(&win_upd, state_upd.clone(), true);
        });
    }
    popover_box.append(&update_btn);

    // ── Help & feedback ──
    // A user-initiated path: the feedback dialog otherwise auto-prompts only once
    // (after a week), so without this a user can neither send feedback nor reach
    // support. "Send feedback" reuses the anonymous one-way dialog (→ dashboard);
    // "Report a problem" opens the issue tracker (the two-way support channel).
    popover_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    popover_box.append(&overline("Help & feedback"));

    let tour_btn = menu_item("What can Fresco do?");
    {
        let state_t = state.clone();
        let win_t = window.clone();
        tour_btn.connect_clicked(move |_| {
            show_tour_dialog(&win_t, state_t.clone());
        });
    }
    popover_box.append(&tour_btn);

    let feedback_btn = menu_item("Send feedback…");
    {
        let state_fb = state.clone();
        let win_fb = window.clone();
        feedback_btn.connect_clicked(move |_| {
            show_feedback_dialog(&win_fb, state_fb.clone());
        });
    }
    popover_box.append(&feedback_btn);

    let help_btn = menu_item("Report a problem…");
    help_btn.set_tooltip_text(Some("Opens the Fresco issue tracker in your browser"));
    help_btn.connect_clicked(|_| {
        let _ = std::process::Command::new("xdg-open")
            .arg("https://github.com/DibbayajyotiRoy/fresco/issues")
            .spawn();
    });
    popover_box.append(&help_btn);

    let about_btn = menu_item("About");
    {
        let win_about = window.clone();
        about_btn.connect_clicked(move |_| {
            show_about_dialog(&win_about);
        });
    }
    popover_box.append(&about_btn);

    let popover = gtk4::Popover::new();
    popover.add_css_class("fresco-menu");
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

    // Favorites first — the wallpapers you starred outrank kind grouping.
    let mut first_section = true;
    {
        let favs: Vec<(usize, &LibraryEntry)> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.favorite && entry_matches_query(e, &q))
            .collect();
        if !favs.is_empty() {
            sections_box.append(&build_section(
                "Favorites",
                first_section,
                &favs,
                &cfg,
                state,
                stack,
                bucket,
            ));
            first_section = false;
        }
    }

    // One section per non-empty category: Images, Videos, GIFs.
    for cat in CATEGORY_ORDER {
        let matches: Vec<(usize, &LibraryEntry)> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| entry_category(e) == cat && entry_matches_query(e, &q))
            .collect();
        if matches.is_empty() {
            continue;
        }

        sections_box.append(&build_section(
            category_label(cat),
            first_section,
            &matches,
            &cfg,
            state,
            stack,
            bucket,
        ));
        first_section = false;
    }
}

/// One labelled FlowBox grid of library cards (used for Favorites and each
/// kind category).
fn build_section(
    label: &str,
    first_section: bool,
    matches: &[(usize, &LibraryEntry)],
    cfg: &Config,
    state: &Rc<RefCell<AppState>>,
    stack: &gtk4::Stack,
    bucket: LayoutBucket,
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
        let card = build_library_card(entry, *idx, state.clone(), stack.clone(), active);
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
            let heart = gtk4::Label::new(Some("\u{2665}"));
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
        let q = gtk4::Label::new(Some("4K"));
        q.add_css_class("wp-badge");
        q.add_css_class("quality");
        badge_row.append(&q);
    }
    overlay.add_overlay(&badge_row);

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

    // Hover-revealed action cluster (bottom-right): heart · edit · menu.
    let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    actions.add_css_class("wp-actions");
    actions.set_halign(gtk4::Align::End);
    actions.set_valign(gtk4::Align::End);
    actions.set_visible(false);

    let fav = gtk4::Button::from_icon_name("emblem-favorite-symbolic");
    fav.add_css_class("wp-edit");
    fav.add_css_class("circular");
    if entry.favorite {
        fav.add_css_class("fav-on");
        fav.set_tooltip_text(Some("Unfavorite"));
    } else {
        fav.set_tooltip_text(Some("Favorite"));
    }
    {
        let state_f = state.clone();
        fav.connect_clicked(move |_| toggle_favorite(&state_f, idx));
    }
    actions.append(&fav);

    let edit = gtk4::Button::from_icon_name("document-edit-symbolic");
    edit.add_css_class("wp-edit");
    edit.add_css_class("circular");
    edit.set_tooltip_text(Some("Edit & crop"));
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
    more.set_tooltip_text(Some("More actions"));
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
            show_card_menu(&overlay_m, state_m.clone(), stack_m.clone(), idx, x, y);
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
            show_card_menu(&overlay_c, state_c.clone(), stack_c.clone(), idx, x, y);
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
        let stop = item("Stop wallpaper");
        let s = state.clone();
        let p = pop.clone();
        stop.connect_clicked(move |_| {
            stop_wallpaper(&s);
            show_toast(
                &s,
                "Wallpaper stopped — desktop reverted to its own background",
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
            let label = format!("Set on {} ({}×{})", m.connector, m.width, m.height);
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
        let clear = item("Show default on all displays");
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
    let browser = item("Set as browser wallpaper");
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
        let clear_b = item("Clear browser wallpaper");
        let s = state.clone();
        let p = pop.clone();
        clear_b.connect_clicked(move |_| {
            {
                let mut st = s.borrow_mut();
                st.config.browser_wallpaper = None;
                st.config.save().ok();
            }
            show_toast(&s, "Browser wallpaper cleared — mirroring the desktop");
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
    let fav = item(if is_fav { "Unfavorite" } else { "Favorite" });
    {
        let s = state.clone();
        let p = pop.clone();
        fav.connect_clicked(move |_| {
            toggle_favorite(&s, idx);
            p.popdown();
        });
    }
    menu.append(&fav);

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

    if state
        .borrow()
        .entries
        .get(idx)
        .map(|e| e.broken)
        .unwrap_or(false)
    {
        let relink = item("Relink…");
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
            "Added to Favorites"
        } else {
            "Removed from Favorites"
        },
    );
    let refresh = state.borrow().refresh.clone();
    if let Some(r) = refresh {
        r();
    }
}

/// Case-insensitive substring match over both the raw and prettified names
/// (`q` must already be lowercased). Shared by home search and the palette.
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
            "Removed — desktop reverted to its own wallpaper"
        } else {
            "Removed from library"
        },
    );
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
        show_toast(&state, &format!("“{name}” set on {connector}"));
    } else {
        show_toast(&state, "Couldn’t start the wallpaper. Run frescod --check");
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
        "Browser new-tab wallpaper set".to_string()
    } else {
        "Browser new-tab wallpaper set — enable Browser new-tab in Settings".to_string()
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
        show_toast(&state, "Default wallpaper on all displays");
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
        show_toast(&state, &format!("“{name}” set as wallpaper"));
        maybe_star_nudge(&state);
    } else {
        show_toast(&state, "Couldn’t start the wallpaper. Run frescod --check");
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
        "Enjoying Fresco? A GitHub star helps other Linux users find it — and your feedback shapes what's next. Already starred? Just ignore this.",
    );
    toast.set_button_label(Some("Star on GitHub"));
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
    {
        let ce = crop_editor.clone();
        reset_crop.connect_clicked(move |_| ce.reset());
    }
    let rotate_btn = gtk4::Button::with_label("Rotate 90°");
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

    // Per-wallpaper power saving (video/playlist only). "Default" inherits the
    // global level from Settings; an explicit level overrides it for just this
    // wallpaper — e.g. keep one showpiece clip on Full.
    let power_row = adw::ComboRow::new();
    power_row.set_title("Power saving");
    power_row.set_subtitle("Default follows Settings; overrides it for this wallpaper");
    power_row.set_model(Some(&gtk4::StringList::new(&POWER_EDIT_LABELS)));
    prefs.add(&power_row);

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
        let power_row_ref = power_row.clone();
        let title_ref = title_widget.clone();
        let crop_frame_ref = crop_frame.clone();
        let tp_frame_ref = tp_frame.clone();
        let edit_actions_ref = edit_actions.clone();
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
    power_row.set_title("Power saving");
    power_row.set_subtitle("Cheaper scaling to cut GPU load; lower = softer image");
    power_row.set_model(Some(&gtk4::StringList::new(&POWER_LABELS)));
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
    add_schedule_group(&page, state);
    dialog.add(&page);
    dialog.present();
}

/// "Day & night wallpaper" preferences group (ROADMAP 3.3 GUI). v1 exposes
/// the daynight mode; times/solar stay config-file features (docs/SCRIPTING.md).
fn add_schedule_group(page: &adw::PreferencesPage, state: Rc<RefCell<AppState>>) {
    let group = adw::PreferencesGroup::new();
    group.set_title("Day &amp; night wallpaper");
    group.set_description(Some(
        "Automatically switch between two wallpapers on a schedule.",
    ));

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
    enable.set_title("Schedule");
    enable.set_model(Some(&gtk4::StringList::new(&["Off", "Day / night"])));

    let day_row = adw::ComboRow::new();
    day_row.set_title("Day wallpaper");
    day_row.set_model(Some(&gtk4::StringList::new(&names)));
    let night_row = adw::ComboRow::new();
    night_row.set_title("Night wallpaper");
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
    day_time_row.set_title("Day starts");
    day_time_row.add_suffix(&day_time);
    let night_time_row = adw::ActionRow::new();
    night_time_row.set_title("Night starts");
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

// ─── Add from URL ─────────────────────────────────────────────────────────────

/// Paste a direct media URL (…/clip.mp4) → download into the library
/// (ROADMAP 3.2). Deliberately NOT yt-dlp/YouTube: direct files only.
fn show_add_from_url_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    const MAX_BYTES: u64 = 1_000_000_000; // refuse >1 GB outright

    let (dialog, content) = glass_dialog(window, "Add from URL", 420, -1);
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_start(20);
    inner.set_margin_end(20);
    inner.set_margin_bottom(18);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("https://example.com/wallpaper.mp4"));
    inner.append(&entry);

    let hint = gtk4::Label::new(Some(
        "Direct video or image links only (.mp4, .webm, .gif, .png, …).",
    ));
    hint.add_css_class("dim");
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    inner.append(&hint);

    let progress = gtk4::ProgressBar::new();
    progress.set_visible(false);
    inner.append(&progress);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.set_halign(gtk4::Align::End);
    let cancel_btn = gtk4::Button::with_label("Cancel");
    let add_btn = gtk4::Button::with_label("Download");
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
                hint.set_text("That doesn\u{2019}t look like a direct media link.");
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
                                &format!("\u{201c}{name}\u{201d} added to the library"),
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
    let (dialog, content) = glass_dialog(window, "About Fresco", 360, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);
    inner.set_halign(gtk4::Align::Center);

    let heading = gtk4::Label::new(Some("Fresco"));
    heading.add_css_class("dialog-heading");
    inner.append(&heading);

    let version = gtk4::Label::new(Some(&format!(
        "Version {}",
        crate::update::current_version()
    )));
    version.add_css_class("dim");
    inner.append(&version);

    let desc = gtk4::Label::new(Some("Live wallpapers for Linux."));
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
        Some("Choose Wallpaper"),
        Some(window),
        FileChooserAction::Open,
        Some("Open"),
        Some("Cancel"),
    );
    chooser.set_select_multiple(true);

    // "All supported" first → it's the default filter, so videos AND images
    // both show without the user switching the dropdown.
    let all_pat: Vec<&str> = VIDEO_PATTERNS
        .iter()
        .chain(IMAGE_PATTERNS.iter())
        .copied()
        .collect();
    chooser.add_filter(&media_filter("All supported", &all_pat));
    chooser.add_filter(&media_filter("Video files", &VIDEO_PATTERNS));
    chooser.add_filter(&media_filter("Image files", &IMAGE_PATTERNS));

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
        add_media_paths(&state_cb, &stack, paths, editing_idx);
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
            Some("Relink Slideshow Folder"),
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
            finish_relink(&state_cb, idx, |e| e.folder = Some(folder));
        });
        state.borrow_mut().current_picker = Some(chooser.clone());
        chooser.show();
        return;
    }

    let chooser = gtk4::FileChooserNative::new(
        Some("Relink Source"),
        Some(window),
        FileChooserAction::Open,
        Some("Open"),
        Some("Cancel"),
    );
    chooser.set_select_multiple(matches!(kind, Kind::Playlist | Kind::Slideshow));
    // Same kind-appropriate restriction as the Add flow, so a broken Video
    // entry can't be "fixed" by pointing it at a non-media file.
    match kind {
        Kind::Video | Kind::Playlist => {
            chooser.add_filter(&media_filter("Video files", &VIDEO_PATTERNS));
        }
        Kind::Image | Kind::Slideshow => {
            chooser.add_filter(&media_filter("Image files", &IMAGE_PATTERNS));
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
    show_toast(state, "Relinked");
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
    let l = gtk4::Label::new(Some(&text.to_uppercase()));
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
        return button_content("insert-link-symbolic", "From link").upcast();
    };

    // Hand-built rather than adw::ButtonContent: that widget only takes a
    // themed icon *name*, and the brand mark must keep its own colour.
    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let image = gtk4::Image::from_paintable(Some(&texture));
    image.set_pixel_size(16);
    content.append(&image);
    content.append(&gtk4::Label::new(Some("From Pinterest")));
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
            Kind::Video => "Video",
            Kind::Image => "Image",
            Kind::Playlist => "Playlist",
            Kind::Slideshow => "Slideshow",
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
    let (dialog, content) = glass_dialog(window, "Commands", 560, 440);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_start(16);
    inner.set_margin_end(16);
    inner.set_margin_bottom(14);

    let entry = gtk4::Entry::new();
    entry.add_css_class("palette-entry");
    entry.set_placeholder_text(Some("Type a command or wallpaper name…"));
    inner.append(&entry);

    let list = gtk4::ListBox::new();
    list.add_css_class("palette-list");
    list.set_selection_mode(gtk4::SelectionMode::Browse);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    inner.append(&scroll);

    let hint = gtk4::Label::new(Some("↑↓ navigate · Enter run · Esc close"));
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
                label: format!("Set: {pretty}"),
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
            "Random wallpaper",
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
            "Browse catalog",
            Rc::new(move || super::gallery::show_gallery_window(&w, s.clone())),
        );
    }
    {
        let (w, s, st) = (window.clone(), state.clone(), stack.clone());
        add_cmd(
            "Add from link",
            Rc::new(move || super::add_link::show_add_link_dialog(&w, s.clone(), st.clone())),
        );
    }
    {
        let (w, s, st) = (window.clone(), state.clone(), stack.clone());
        add_cmd(
            "Add files",
            Rc::new(move || open_file_picker(&w, s.clone(), st.clone(), None)),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            "Advanced settings",
            Rc::new(move || show_advanced_dialog(&w, s.clone())),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            "Send feedback",
            Rc::new(move || show_feedback_dialog(&w, s.clone())),
        );
    }
    {
        let (w, s) = (window.clone(), state.clone());
        add_cmd(
            "What can Fresco do?",
            Rc::new(move || show_tour_dialog(&w, s.clone())),
        );
    }
    {
        let (w, s, st) = (window.clone(), state.clone(), stack.clone());
        add_cmd(
            "How to set a Pinterest wallpaper",
            Rc::new(move || show_onboarding_dialog(&w, s.clone(), st.clone())),
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

/// One-time telemetry consent — asked before anything is ever sent (the
/// telemetry layer is a no-op until this is answered). Both choices carry
/// equal visual weight: consent that's honest converts better than consent
/// that's tricked.
fn show_telemetry_consent_dialog(window: &adw::ApplicationWindow, state: Rc<RefCell<AppState>>) {
    let (dialog, content) = glass_dialog(window, "Help improve Fresco?", 460, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    let body = gtk4::Label::new(Some(
        "Share anonymous usage statistics to help make Fresco better?\n\n\
         What's shared: a random install id, app version, distro name, \
         desktop, feature-usage counts, and error kinds.\n\
         Never shared: personal data, file names, or your wallpapers.\n\n\
         You can change this anytime in Settings.",
    ));
    body.set_wrap(true);
    body.set_xalign(0.0);
    inner.append(&body);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    buttons.set_margin_top(6);
    let decline = gtk4::Button::with_label("No thanks");
    let accept = gtk4::Button::with_label("Share anonymously");
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
pub(crate) const ONBOARDING_VERSION: u32 = 1;

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

/// Onboarding for the paste-a-link flow. Telemetry showed `add_from_link` at
/// zero uses over 30 days while wallpapers were being set daily — the feature
/// worked, nobody found it. This walks through it once, in order, and links
/// the demo video.
///
/// Deliberately skippable. A gate that forces a rewatch punishes the users who
/// already know the flow and the ones who got interrupted, and "watched" is
/// not something an external browser can report back anyway. Discovery is the
/// problem being solved here, not compliance.
pub(crate) fn show_onboarding_dialog(
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
) {
    {
        let mut s = state.borrow_mut();
        if s.config.onboarding_version < ONBOARDING_VERSION {
            s.config.onboarding_version = ONBOARDING_VERSION;
            s.config.save().ok();
        }
    }

    let (dialog, content) = glass_dialog(window, "Set a wallpaper from Pinterest", 500, -1);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.set_margin_top(8);
    inner.set_margin_bottom(22);

    let lead = gtk4::Label::new(Some(
        "Found a live wallpaper you like on Pinterest? Copy its link and \
         Fresco will download it and set it — no files to manage.",
    ));
    lead.add_css_class("dialog-sub");
    lead.set_wrap(true);
    lead.set_xalign(0.0);
    inner.append(&lead);

    let steps: &[(&str, &str)] = &[
        (
            "1 · Copy the link",
            "On Pinterest, open the pin and hit Share → Copy link. A pin.it or \
             pinterest.com link both work, and so does any direct video or image URL.",
        ),
        (
            "2 · Click “From Pinterest”",
            "It's in the bar at the bottom of the window, next to Add folder.",
        ),
        (
            "3 · Paste and confirm",
            "Fresco downloads it, drops you into the editor to rotate or crop, \
             then you click Set as wallpaper.",
        ),
    ];
    for (title, body) in steps {
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
        inner.append(&row);
    }

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_margin_top(8);

    let watch = gtk4::Button::with_label("Watch the demo");
    watch.set_tooltip_text(Some("Opens the walkthrough video in your browser"));
    watch.connect_clicked(|_| {
        // Counts intent only: once the browser has it, Fresco can't tell
        // whether it was watched. Judge the video by whether add_from_link
        // moves, not by this number.
        crate::telemetry::event("tutorial_opened", serde_json::json!({}));
        open_in_browser(TUTORIAL_URL);
    });
    buttons.append(&watch);

    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    buttons.append(&spacer);

    let skip = gtk4::Button::with_label("Not now");
    skip.add_css_class("flat");
    {
        let dialog = dialog.clone();
        skip.connect_clicked(move |_| dialog.close());
    }
    buttons.append(&skip);

    // The point of the whole dialog: land them in the paste field while the
    // link is still in their clipboard.
    let go = gtk4::Button::with_label("Paste a link");
    go.add_css_class("suggested-action");
    {
        let dialog = dialog.clone();
        let win = window.clone();
        let state = state.clone();
        let stack = stack.clone();
        go.connect_clicked(move |_| {
            dialog.close();
            super::add_link::show_add_link_dialog(&win, state.clone(), stack.clone());
        });
    }
    buttons.append(&go);

    inner.append(&buttons);
    content.append(&inner);
    dialog.present();
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
    let (dialog, content) = glass_dialog(window, "What can Fresco do?", 520, 560);

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
            "Set anything",
            "Videos, GIFs, images, slideshows, and playlists — click a card to set it.",
        ),
        (
            "Add from a link",
            "The link button imports a Pinterest pin or any direct video/image URL — no downloads needed.",
        ),
        (
            "Preview, rotate & crop",
            "Double-click any card (or use its Edit button) to adjust before setting.",
        ),
        (
            "Per-monitor wallpapers",
            "Right-click a card → “Set on <display>” for different wallpapers per screen.",
        ),
        (
            "Day & night schedules",
            "Two wallpapers on a timer — under Advanced in the menu.",
        ),
        (
            "Wallpaper catalog",
            "Menu → “Browse wallpapers…” for curated, licensed picks in two clicks.",
        ),
        (
            "Hover to preview",
            "Hover a video card and it plays silently in place.",
        ),
        (
            "Keyboard shortcuts",
            "Ctrl+K command palette · Ctrl+F search · Ctrl+, menu · Ctrl+Q quit.",
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

    let (dialog, content) = glass_dialog(window, "Feedback", 420, -1);

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
    dialog.present();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
