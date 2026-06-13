//! Runtime CSS theming engine for Fresco.
//!
//! Holds a single app-level [`gtk4::CssProvider`] (kept in a `thread_local!`)
//! attached to the default display, and rebuilds the stylesheet on demand so
//! the accent color and light/dark scheme can be live-swapped without
//! re-creating any widgets.
//!
//! Taste rules baked in here: Inter type, one neutral base (obsidian / paper)
//! with a single restrained accent (saturation kept modest), tinted shadows
//! instead of pure-black, and crisp accent borders instead of colored glow.
//!
//! GTK4's CSS is a *subset* of web CSS: this file deliberately sticks to the
//! supported feature set (`@define-color`, `alpha()`, `shade()`,
//! `linear-gradient()`, `box-shadow`, pseudo-classes, etc.) and never uses
//! `transform`, `filter`, `var()`, or `calc()`.

use libadwaita as adw;

use crate::config::{Accent, ThemeMode};

thread_local! {
    static PROVIDER: gtk4::CssProvider = gtk4::CssProvider::new();
}

/// A single resolved color palette (dark or light). Every field is a CSS
/// color literal usable verbatim inside a declaration.
struct Pal {
    window_bg: &'static str,
    window_fg: &'static str,
    view_bg: &'static str,
    card_bg: &'static str,
    headerbar_bg: &'static str,
    popover_bg: &'static str,
    card_border: &'static str,
    card_hover: &'static str,
    thumb_mat: &'static str,
    dim_fg: &'static str,
    destructive: &'static str,
    /// Resting elevation shadow, tinted to the surface (never pure black on light).
    shadow_sm: &'static str,
    /// Hover / lifted elevation shadow.
    shadow_md: &'static str,
}

impl Pal {
    /// Obsidian dark scheme (never pitch-black).
    const DARK: Pal = Pal {
        window_bg: "#15171C",
        window_fg: "#ECEDF1",
        view_bg: "#131519",
        card_bg: "#1C1F27",
        headerbar_bg: "#16181E",
        popover_bg: "#1E212A",
        card_border: "rgba(255,255,255,0.09)",
        card_hover: "#232733",
        thumb_mat: "#0E1014",
        dim_fg: "#9AA0AC",
        destructive: "#E5484D",
        shadow_sm: "rgba(0,0,0,0.32)",
        shadow_md: "rgba(0,0,0,0.45)",
    };

    /// Off-white "paper" light scheme (never pure-white) with obsidian ink text.
    const LIGHT: Pal = Pal {
        window_bg: "#F6F5F1",
        window_fg: "#1C1D21",
        view_bg: "#F4F2EE",
        card_bg: "#FCFBF8",
        headerbar_bg: "#F1EFEA",
        popover_bg: "#FCFBF8",
        card_border: "rgba(20,20,22,0.10)",
        card_hover: "#FFFDFA",
        thumb_mat: "#ECEAE3",
        dim_fg: "#6B6E77",
        destructive: "#DC2626",
        shadow_sm: "rgba(40,38,33,0.07)",
        shadow_md: "rgba(40,38,33,0.13)",
    };
}

/// Resolve the `(accent_bg_color, accent_fg_color)` pair for an accent in the
/// given scheme. Colors are kept tasteful (modest saturation) and never purple.
fn accent_pair(accent: Accent, dark: bool) -> (&'static str, &'static str) {
    match (accent, dark) {
        (Accent::Blue, true) => ("#4F8FF7", "#F7FAFF"),
        (Accent::Blue, false) => ("#2A6BE0", "#F7FAFF"),
        (Accent::Teal, true) => ("#2BB6A2", "#04221E"),
        (Accent::Teal, false) => ("#0E8C7E", "#EFFBF8"),
        (Accent::Green, true) => ("#46B96B", "#06210F"),
        (Accent::Green, false) => ("#2C9A4C", "#F1FBF3"),
        (Accent::Amber, true) => ("#DBA13C", "#2A1E03"),
        (Accent::Amber, false) => ("#AE7820", "#FFFBF2"),
        (Accent::Coral, true) => ("#F0708A", "#2A0A12"),
        (Accent::Coral, false) => ("#DE4567", "#FFF4F6"),
        (Accent::Graphite, true) => ("#98A1B0", "#10131A"),
        (Accent::Graphite, false) => ("#5B626F", "#F7F8FA"),
    }
}

/// Create the app-level [`gtk4::CssProvider`] and attach it to the default
/// display. Call once after the GTK app has activated.
pub fn install() {
    let display = gtk4::gdk::Display::default()
        .expect("Fresco theme: no default GDK display (call install() after activation)");
    PROVIDER.with(|provider| {
        gtk4::style_context_add_provider_for_display(
            &display,
            provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
}

/// Rebuild the stylesheet for `accent` + dark/light and load it into the
/// installed provider (live-swappable; call again to re-theme).
#[allow(deprecated)]
pub fn apply(accent: Accent, dark: bool) {
    let css = build_css(accent, dark);
    PROVIDER.with(|provider| {
        provider.load_from_data(&css);
    });
}

/// Map the user's [`ThemeMode`] preference onto libadwaita's color scheme.
pub fn set_mode(mode: ThemeMode) {
    adw::StyleManager::default().set_color_scheme(match mode {
        ThemeMode::System => adw::ColorScheme::Default,
        ThemeMode::Light => adw::ColorScheme::ForceLight,
        ThemeMode::Dark => adw::ColorScheme::ForceDark,
    });
}

/// Whether libadwaita currently resolves to a dark scheme.
pub fn is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

/// Build the full stylesheet for the given accent + scheme.
fn build_css(accent: Accent, dark: bool) -> String {
    let p = if dark { &Pal::DARK } else { &Pal::LIGHT };
    let (accent_bg, accent_fg) = accent_pair(accent, dark);

    format!(
        "/* ===== Fresco palette ===== */
@define-color window_bg_color {window_bg};
@define-color window_fg_color {window_fg};
@define-color view_bg_color {view_bg};
@define-color view_fg_color {window_fg};
@define-color card_bg_color {card_bg};
@define-color card_fg_color {window_fg};
@define-color headerbar_bg_color {headerbar_bg};
@define-color headerbar_fg_color {window_fg};
@define-color popover_bg_color {popover_bg};
@define-color popover_fg_color {window_fg};
@define-color card_border {card_border};
@define-color card_hover {card_hover};
@define-color thumb_mat {thumb_mat};
@define-color dim_fg {dim_fg};
@define-color destructive_bg_color {destructive};
@define-color destructive_color {destructive};

/* ===== Accent (single, locked) ===== */
@define-color accent_color {accent_bg};
@define-color accent_bg_color {accent_bg};
@define-color accent_fg_color {accent_fg};

/* ===== Surfaces + type ===== */
window, .background {{ background-color: @window_bg_color; color: @window_fg_color; font-family: \"Inter\", \"Adwaita Sans\", \"Cantarell\", sans-serif; }}
headerbar, headerbar.flat {{ background-color: @headerbar_bg_color; box-shadow: none; border-bottom: 1px solid @card_border; }}

.overline {{ font-size: 11px; font-weight: 600; letter-spacing: 0.05em; color: @dim_fg; }}
.dim {{ color: @dim_fg; }}
.dialog-heading {{ font-size: 19px; font-weight: 700; letter-spacing: -0.01em; }}
.dialog-sub {{ color: @dim_fg; font-size: 13px; }}

/* ===== Wallpaper card ===== */
.wp-card {{ background-color: @card_bg_color; border: 1px solid @card_border; border-radius: 14px; box-shadow: 0 1px 2px {shadow_sm}; transition: box-shadow 180ms ease, border-color 180ms ease, background-color 180ms ease; }}
.wp-card:hover {{ background-color: @card_hover; border-color: alpha(@accent_bg_color,0.45); box-shadow: 0 6px 18px {shadow_md}; }}
.wp-card.active {{ border: 2px solid @accent_bg_color; box-shadow: 0 1px 3px {shadow_sm}; }}

.wp-thumb {{ background-color: @thumb_mat; }}
.wp-scrim {{ background: linear-gradient(to top, alpha(black,0.82) 0%, alpha(black,0.38) 52%, alpha(black,0) 100%); padding: 22px 12px 9px 12px; }}
.wp-title {{ color: #FFFFFF; font-weight: 600; font-size: 13px; letter-spacing: -0.01em; text-shadow: 0 1px 3px rgba(0,0,0,0.6); }}
.wp-badge {{ background-color: alpha(black,0.55); color: #FFFFFF; font-size: 9px; font-weight: 700; letter-spacing: 0.05em; padding: 2px 7px; border-radius: 7px; margin: 8px; }}
.wp-active-pill {{ background-color: @accent_bg_color; color: @accent_fg_color; font-size: 9px; font-weight: 700; letter-spacing: 0.04em; padding: 2px 8px; border-radius: 7px; margin: 8px; }}
.wp-edit {{ background-color: alpha(black,0.55); color: #FFFFFF; border-radius: 999px; min-height: 26px; min-width: 26px; padding: 3px; margin: 8px; }}
.wp-edit:hover {{ background-color: alpha(black,0.78); }}

/* ===== Mini card ===== */
.wp-mini {{ background-color: @card_bg_color; border: 1px solid @card_border; border-radius: 10px; box-shadow: 0 1px 2px {shadow_sm}; transition: box-shadow 160ms ease, border-color 160ms ease; }}
.wp-mini:hover {{ border-color: alpha(@accent_bg_color,0.45); box-shadow: 0 5px 14px {shadow_md}; }}
.wp-mini.active {{ border: 2px solid @accent_bg_color; }}

/* ===== Status ===== */
.status-pill {{ background-color: @card_bg_color; border: 1px solid @card_border; border-radius: 999px; padding: 3px 12px; color: @dim_fg; font-size: 12px; }}
.dot-ok {{ color: #3FB950; }}
.dot-warn {{ color: #D29922; }}
.dot-off {{ color: @dim_fg; }}

/* ===== What's-new banner ===== */
.banner {{ background-color: alpha(@accent_bg_color,0.10); border: 1px solid alpha(@accent_bg_color,0.28); border-radius: 12px; padding: 5px 6px 5px 14px; }}

/* ===== Buttons ===== */
.set-btn {{ min-height: 42px; font-size: 14px; font-weight: 600; border-radius: 12px; }}
.feedback-btn {{ min-height: 38px; padding-left: 14px; padding-right: 14px; border-radius: 11px; }}
button {{ border-radius: 9px; }}
button.suggested-action {{ font-weight: 600; }}
.seg button:checked {{ background-color: @accent_bg_color; color: @accent_fg_color; }}

/* ===== Accent picker dots ===== */
.accent-dot {{ min-width: 22px; min-height: 22px; border-radius: 999px; padding: 0; border: 2px solid transparent; }}
.accent-dot.selected {{ border-color: @window_fg_color; }}
.accent-blue {{ background-image: none; background-color: #3D7BEF; }}
.accent-teal {{ background-image: none; background-color: #1FAE9A; }}
.accent-green {{ background-image: none; background-color: #3AAE5C; }}
.accent-amber {{ background-image: none; background-color: #CC9233; }}
.accent-coral {{ background-image: none; background-color: #E85C7A; }}
.accent-graphite {{ background-image: none; background-color: #7A8392; }}

/* ===== Misc ===== */
entry.wp-search {{ border-radius: 10px; }}
.welcome-cta {{ min-height: 40px; border-radius: 11px; font-weight: 600; }}
",
        window_bg = p.window_bg,
        window_fg = p.window_fg,
        view_bg = p.view_bg,
        card_bg = p.card_bg,
        headerbar_bg = p.headerbar_bg,
        popover_bg = p.popover_bg,
        card_border = p.card_border,
        card_hover = p.card_hover,
        thumb_mat = p.thumb_mat,
        dim_fg = p.dim_fg,
        destructive = p.destructive,
        shadow_sm = p.shadow_sm,
        shadow_md = p.shadow_md,
        accent_bg = accent_bg,
        accent_fg = accent_fg,
    )
}
