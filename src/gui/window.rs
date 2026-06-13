use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};
use gtk4::{FileChooserAction, GestureClick, PolicyType, ResponseType};
use libadwaita::{self as adw, prelude::*};

use super::{
    daemon_ctl,
    library::{self, load_entries, save_entries, LibraryEntry},
};
use crate::{
    autostart,
    config::{Config, Fit, Scaling},
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
}

// ─── Main window ─────────────────────────────────────────────────────────────

fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Fresco"));
    window.set_default_size(720, 600);
    window.set_icon_name(Some(APP_ID));

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

    let state = Rc::new(RefCell::new(AppState {
        config: Config::load().unwrap_or_default(),
        entries,
        editing_idx: None,
        current_picker: None,
    }));

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);

    let (library_view, status_label) = build_library_view(app, &window, state.clone(), &stack);
    let editor_view = build_editor_view(state.clone(), &stack);

    stack.add_named(&library_view, Some("library"));
    stack.add_named(&editor_view, Some("editor"));

    window.set_content(Some(&stack));
    window.present();

    // Poll daemon status every 3 seconds; reflect decode mode + a help hint.
    let status_label_poll = status_label.clone();
    glib::timeout_add_local(Duration::from_secs(3), move || {
        let status = daemon_ctl::get_status();
        status_label_poll.set_text(&daemon_ctl::status_line(status.as_ref()));
        status_label_poll.set_tooltip_text(daemon_ctl::hwdec_hint(status.as_ref()).as_deref());
        glib::ControlFlow::Continue
    });
}

// ─── Library view ─────────────────────────────────────────────────────────────

fn build_library_view(
    _app: &adw::Application,
    window: &adw::ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    stack: &gtk4::Stack,
) -> (gtk4::Box, gtk4::Label) {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // ── Header bar ──
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Fresco", "")));

    // Play/Pause toggle. Tracks the last action so one button does both.
    let paused = Rc::new(std::cell::Cell::new(false));
    let pause_btn = gtk4::Button::from_icon_name("media-playback-pause-symbolic");
    pause_btn.set_tooltip_text(Some("Pause / resume wallpaper"));
    {
        let paused = paused.clone();
        pause_btn.connect_clicked(move |btn| {
            if paused.get() {
                daemon_ctl::resume_daemon().ok();
                paused.set(false);
                btn.set_icon_name("media-playback-pause-symbolic");
            } else {
                daemon_ctl::pause_daemon().ok();
                paused.set(true);
                btn.set_icon_name("media-playback-start-symbolic");
            }
        });
    }
    header.pack_start(&pause_btn);

    let stop_btn = gtk4::Button::from_icon_name("media-playback-stop-symbolic");
    stop_btn.add_css_class("destructive-action");
    stop_btn.set_tooltip_text(Some("Stop wallpaper"));
    {
        let state2 = state.clone();
        stop_btn.connect_clicked(move |_| {
            daemon_ctl::stop_daemon().ok();
            let mut s = state2.borrow_mut();
            s.config.enabled = false;
            s.config.save().ok();
        });
    }
    header.pack_start(&stop_btn);

    let menu_btn = gtk4::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    let popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    popover_box.set_margin_top(8);
    popover_box.set_margin_bottom(8);
    popover_box.set_margin_start(4);
    popover_box.set_margin_end(4);

    // Autostart switch row.
    let autostart_row = menu_switch_row("Restore on login", state.borrow().config.autostart);
    {
        let state2 = state.clone();
        autostart_row.connect_active_notify(move |sw| {
            let mut s = state2.borrow_mut();
            s.config.autostart = sw.is_active();
            s.config.save().ok();
            if sw.is_active() {
                autostart::enable().ok();
            } else {
                autostart::disable().ok();
            }
        });
    }
    let autostart_label = gtk4::Label::new(Some("Restore on login"));
    autostart_label.set_hexpand(true);
    autostart_label.set_xalign(0.0);
    let autostart_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    autostart_hbox.set_margin_start(8);
    autostart_hbox.set_margin_end(8);
    autostart_hbox.append(&autostart_label);
    autostart_hbox.append(&autostart_row);
    popover_box.append(&autostart_hbox);

    // Battery pause switch row.
    let battery_row = menu_switch_row("Pause on battery", state.borrow().config.pause_on_battery);
    {
        let state2 = state.clone();
        battery_row.connect_active_notify(move |sw| {
            let mut s = state2.borrow_mut();
            s.config.pause_on_battery = sw.is_active();
            s.config.save().ok();
        });
    }
    let battery_label = gtk4::Label::new(Some("Pause on battery"));
    battery_label.set_hexpand(true);
    battery_label.set_xalign(0.0);
    let battery_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    battery_hbox.set_margin_start(8);
    battery_hbox.set_margin_end(8);
    battery_hbox.append(&battery_label);
    battery_hbox.append(&battery_row);
    popover_box.append(&battery_hbox);

    // Separator + Advanced.
    popover_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    let advanced_btn = gtk4::Button::with_label("Advanced…");
    advanced_btn.add_css_class("flat");
    advanced_btn.set_margin_start(4);
    advanced_btn.set_margin_end(4);
    let state_adv = state.clone();
    let win_adv = window.clone();
    advanced_btn.connect_clicked(move |_| {
        show_advanced_dialog(&win_adv, state_adv.clone());
    });
    popover_box.append(&advanced_btn);

    let popover = gtk4::Popover::new();
    popover.set_child(Some(&popover_box));
    menu_btn.set_popover(Some(&popover));
    header.pack_end(&menu_btn);
    root.append(&header);

    // ── Status bar ──
    let status_label = gtk4::Label::new(Some("Checking…"));
    status_label.add_css_class("caption");
    status_label.set_xalign(0.0);
    status_label.set_margin_start(12);
    status_label.set_margin_end(12);
    status_label.set_margin_top(4);
    status_label.set_margin_bottom(2);
    root.append(&status_label);

    // ── Search ──
    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some("Search wallpapers…"));
    search.set_margin_start(12);
    search.set_margin_end(12);
    search.set_margin_top(4);
    search.set_margin_bottom(4);
    root.append(&search);

    // ── Scrollable content ──
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Recent row.
    let recent_label = gtk4::Label::new(Some("Recent"));
    recent_label.add_css_class("heading");
    recent_label.set_xalign(0.0);
    recent_label.set_margin_start(12);
    recent_label.set_margin_top(12);
    recent_label.set_margin_bottom(4);
    content.append(&recent_label);

    let recent_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    recent_box.set_margin_start(12);
    recent_box.set_margin_end(12);
    recent_box.set_margin_bottom(8);
    content.append(&recent_box);

    // All wallpapers heading.
    let all_label = gtk4::Label::new(Some("All Wallpapers"));
    all_label.add_css_class("heading");
    all_label.set_xalign(0.0);
    all_label.set_margin_start(12);
    all_label.set_margin_top(8);
    all_label.set_margin_bottom(4);
    content.append(&all_label);

    // FlowBox grid.
    let flow = gtk4::FlowBox::new();
    flow.set_homogeneous(true);
    flow.set_max_children_per_line(6);
    flow.set_min_children_per_line(2);
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_margin_start(12);
    flow.set_margin_end(12);
    flow.set_margin_bottom(12);
    flow.set_row_spacing(8);
    flow.set_column_spacing(8);
    content.append(&flow);

    // Welcome page (shown when library is empty).
    let welcome = adw::StatusPage::new();
    welcome.set_icon_name(Some("video-display-symbolic"));
    welcome.set_title("No wallpapers yet");
    welcome.set_description(Some("Click Add to pick a video, image, or folder"));
    content.append(&welcome);

    scroll.set_child(Some(&content));
    root.append(&scroll);

    // ── Add buttons ──
    let add_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    add_row.set_halign(gtk4::Align::End);
    add_row.set_margin_start(12);
    add_row.set_margin_end(12);
    add_row.set_margin_top(8);
    add_row.set_margin_bottom(12);

    let add_folder_btn = gtk4::Button::with_label("Add Folder…");
    add_folder_btn.set_tooltip_text(Some("Create an image slideshow from a folder"));
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let win2 = window.clone();
        add_folder_btn.connect_clicked(move |_| {
            open_folder_picker(&win2, state2.clone(), stack2.clone());
        });
    }
    add_row.append(&add_folder_btn);

    let add_btn = gtk4::Button::with_label("+ Add");
    add_btn.add_css_class("suggested-action");
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        let win2 = window.clone();
        add_btn.connect_clicked(move |_| {
            open_file_picker(&win2, state2.clone(), stack2.clone(), None);
        });
    }
    add_row.append(&add_btn);
    root.append(&add_row);

    // Search filter.
    {
        let flow2 = flow.clone();
        search.connect_search_changed(move |entry| {
            let query = entry.text().to_lowercase();
            flow2.invalidate_filter();
            // Simple label-based filter using set_filter_func.
            let q = query.clone();
            flow2.set_filter_func(move |child| {
                if q.is_empty() {
                    return true;
                }
                // Get the name label from our card widget structure.
                let name = get_card_name(child);
                name.to_lowercase().contains(&q)
            });
        });
    }

    // Initial population.
    populate_library(&state, &flow, &recent_box, &recent_label, &welcome, stack);

    // Repopulate whenever we return to the library view (e.g. after adding).
    {
        let state2 = state.clone();
        let stack2 = stack.clone();
        stack.connect_visible_child_name_notify(move |s| {
            if s.visible_child_name().as_deref() == Some("library") {
                populate_library(
                    &state2,
                    &flow,
                    &recent_box,
                    &recent_label,
                    &welcome,
                    &stack2,
                );
            }
        });
    }

    (root, status_label)
}

fn get_card_name(child: &gtk4::FlowBoxChild) -> String {
    child
        .child()
        .and_then(|w| w.downcast::<gtk4::Box>().ok())
        .and_then(|b| {
            // Last child of the vbox is our label row (a Box with the Label inside).
            let mut last: Option<gtk4::Widget> = None;
            let mut cur = b.first_child();
            while let Some(w) = cur {
                last = Some(w.clone());
                cur = w.next_sibling();
            }
            last
        })
        .and_then(|row| row.downcast::<gtk4::Box>().ok())
        .and_then(|row_box| row_box.first_child())
        .and_then(|w| w.downcast::<gtk4::Label>().ok())
        .map(|l| l.text().to_string())
        .unwrap_or_default()
}

fn populate_library(
    state: &Rc<RefCell<AppState>>,
    flow: &gtk4::FlowBox,
    recent_box: &gtk4::Box,
    recent_label: &gtk4::Label,
    welcome: &adw::StatusPage,
    stack: &gtk4::Stack,
) {
    // Clear.
    while let Some(c) = flow.first_child() {
        flow.remove(&c);
    }
    while let Some(c) = recent_box.first_child() {
        recent_box.remove(&c);
    }

    let entries = state.borrow().entries.clone();

    if entries.is_empty() {
        welcome.set_visible(true);
        recent_label.set_visible(false);
        return;
    }
    welcome.set_visible(false);

    // Recents.
    {
        let s = state.borrow();
        let recents = library::recent_entries(&s.entries, 5);
        recent_label.set_visible(!recents.is_empty());
        for e in recents {
            recent_box.append(&build_mini_card(e));
        }
    }

    // All entries.
    for (idx, entry) in entries.iter().enumerate() {
        let card = build_library_card(entry, idx, state.clone(), stack.clone());
        flow.append(&card);
    }
}

fn build_mini_card(entry: &LibraryEntry) -> gtk4::Box {
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    vbox.set_size_request(88, 88);

    let pic = gtk4::Picture::new();
    pic.set_size_request(88, 64);
    pic.set_hexpand(false);
    if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
        pic.set_file(Some(&gio::File::for_path(thumb)));
    }
    vbox.append(&pic);

    let label = gtk4::Label::new(Some(&truncate(&entry.name, 11)));
    label.add_css_class("caption");
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    vbox.append(&label);
    vbox
}

fn build_library_card(
    entry: &LibraryEntry,
    idx: usize,
    state: Rc<RefCell<AppState>>,
    stack: gtk4::Stack,
) -> gtk4::Box {
    let frame = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    frame.set_size_request(148, 128);
    frame.add_css_class("card");

    let pic = gtk4::Picture::new();
    pic.set_size_request(148, 96);
    pic.set_hexpand(true);
    if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
        pic.set_file(Some(&gio::File::for_path(thumb)));
    }
    frame.append(&pic);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    row.set_margin_start(4);
    row.set_margin_end(4);
    row.set_margin_bottom(4);

    let name = gtk4::Label::new(Some(&truncate(&entry.name, 18)));
    name.set_hexpand(true);
    name.set_xalign(0.0);
    name.add_css_class("caption");
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    row.append(&name);

    if entry.broken {
        let badge = gtk4::Label::new(Some("⚠"));
        badge.add_css_class("warning");
        badge.set_tooltip_text(entry.error.as_deref().or(Some("File not found")));
        row.append(&badge);
    }
    frame.append(&row);

    // Single click = apply; double click = open editor.
    let click = GestureClick::new();
    let state_click = state.clone();
    let stack_click = stack.clone();
    click.connect_released(move |_, n_press, _, _| {
        if n_press == 1 {
            apply_entry_by_idx(state_click.clone(), idx);
        } else if n_press == 2 {
            state_click.borrow_mut().editing_idx = Some(idx);
            stack_click.set_visible_child_name("editor");
        }
    });
    frame.add_controller(click);

    frame
}

fn apply_entry_by_idx(state: Rc<RefCell<AppState>>, idx: usize) {
    let mut s = state.borrow_mut();
    let Some(entry) = s.entries.get_mut(idx) else {
        return;
    };
    if entry.broken {
        return;
    }
    entry.touch();
    let wallpaper = entry.to_wallpaper();
    s.config.wallpaper = wallpaper;
    s.config.enabled = true;
    drop(s);
    let s2 = state.borrow();
    daemon_ctl::ensure_daemon_and_apply(&s2.config).ok();
    save_entries(&s2.entries).ok();
}

// ─── Editor view ──────────────────────────────────────────────────────────────

fn build_editor_view(state: Rc<RefCell<AppState>>, stack: &gtk4::Stack) -> gtk4::Box {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Edit Wallpaper", "")));
    let back = gtk4::Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some("Back to library"));
    {
        let stack2 = stack.clone();
        back.connect_clicked(move |_| {
            stack2.set_visible_child_name("library");
        });
    }
    header.pack_start(&back);
    root.append(&header);

    // Crop editor.
    let crop_editor = super::preview::CropEditor::new(None);
    crop_editor.overlay.set_size_request(-1, 250);
    crop_editor.overlay.set_margin_start(12);
    crop_editor.overlay.set_margin_end(12);
    crop_editor.overlay.set_margin_top(12);
    root.append(&crop_editor.overlay);

    let reset_crop = gtk4::Button::with_label("Reset crop");
    reset_crop.add_css_class("flat");
    reset_crop.set_halign(gtk4::Align::End);
    reset_crop.set_margin_end(12);
    reset_crop.set_margin_top(4);
    {
        let ce = crop_editor.clone();
        reset_crop.connect_clicked(move |_| ce.reset());
    }
    root.append(&reset_crop);

    // Preferences group.
    let prefs = adw::PreferencesGroup::new();
    prefs.set_margin_start(12);
    prefs.set_margin_end(12);
    prefs.set_margin_top(12);

    let fit_row = adw::ComboRow::new();
    fit_row.set_title("Fit");
    fit_row.set_subtitle("How to fill the screen");
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
    vol_scale.set_size_request(160, -1);
    vol_row.add_suffix(&vol_scale);
    prefs.add(&vol_row);

    root.append(&prefs);

    // Set Wallpaper button.
    let set_btn = gtk4::Button::with_label("Set as Wallpaper");
    set_btn.add_css_class("suggested-action");
    set_btn.set_margin_start(12);
    set_btn.set_margin_end(12);
    set_btn.set_margin_top(12);
    set_btn.set_margin_bottom(12);

    let state_set = state.clone();
    let stack_set = stack.clone();
    let crop_ref = crop_editor.clone();
    let fit_ref = fit_row.clone();
    let mute_ref = mute_sw.clone();
    let vol_ref = vol_scale.clone();
    set_btn.connect_clicked(move |_| {
        let crop = crop_ref.crop();
        let fit = match fit_ref.selected() {
            1 => Fit::Contain,
            2 => Fit::Stretch,
            _ => Fit::Cover,
        };
        let mut s = state_set.borrow_mut();
        s.config.wallpaper.crop = crop;
        s.config.wallpaper.fit = fit;
        s.config.wallpaper.mute = mute_ref.is_active();
        s.config.wallpaper.volume = vol_ref.value() as u8;
        s.config.enabled = true;
        drop(s);
        let s2 = state_set.borrow();
        match daemon_ctl::ensure_daemon_and_apply(&s2.config) {
            Ok(_) => {
                log::info!("Wallpaper set — close this window, it keeps playing");
                drop(s2);
                stack_set.set_visible_child_name("library");
            }
            Err(e) => log::error!("Failed to apply: {e}"),
        }
    });
    root.append(&set_btn);

    // When entering the editor, load the selected entry's preview + settings.
    {
        let ce = crop_editor.clone();
        let fit_ref = fit_row.clone();
        let mute_ref = mute_sw.clone();
        let vol_ref = vol_scale.clone();
        let state2 = state.clone();
        stack.connect_visible_child_name_notify(move |s| {
            if s.visible_child_name().as_deref() != Some("editor") {
                return;
            }
            let st = state2.borrow();
            // Show the thumbnail (videos) or the image itself as the crop preview.
            if let Some(entry) = st.editing_idx.and_then(|i| st.entries.get(i)) {
                if let Some(thumb) = entry.thumbnail.as_deref().filter(|p| p.exists()) {
                    ce.set_media(thumb);
                } else if let Some(p) = entry.path.as_deref().filter(|p| p.exists()) {
                    ce.set_media(p);
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

    root
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
            library::LibraryEntry::new_playlist(paths)
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

fn menu_switch_row(_label: &str, active: bool) -> gtk4::Switch {
    let sw = gtk4::Switch::new();
    sw.set_active(active);
    sw
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        format!("{}…", chars[..max].iter().collect::<String>())
    }
}
