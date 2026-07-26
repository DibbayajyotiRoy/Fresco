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
//! GTK4's CSS is a *subset* of web CSS: this file sticks to the supported
//! feature set (`alpha()`, `shade()`, `linear-gradient()`, `box-shadow`,
//! pseudo-classes) and never uses `transform`, `filter`, `var()`, or `calc()`.
//! It also never references an `@define-color` name — GTK ignores runtime
//! redefinitions of one, so [`build_css`] substitutes literal colors.

use libadwaita as adw;

use crate::config::{Accent, ThemeMode};

thread_local! {
    // The attached provider, so `apply()` can detach the previous stylesheet.
    static PROVIDER: std::cell::RefCell<Option<gtk4::CssProvider>> =
        const { std::cell::RefCell::new(None) };
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
    /// Linear-inspired dark scheme: a near-black canvas with a faint blue tint,
    /// a four-step surface ladder (canvas → card → hover → popover) carrying
    /// hierarchy by lift + hairline rather than shadow.
    const DARK: Pal = Pal {
        window_bg: "#090A0C", // Linear canvas — the deepest surface
        window_fg: "#F7F8F8", // ink
        view_bg: "#090A0C",
        card_bg: "#14171B",
        headerbar_bg: "#121418",
        popover_bg: "#16181D",  // surface-2/3
        card_border: "#23252A", // Linear hairline (solid, not translucent)
        card_hover: "#1D2127",
        thumb_mat: "#08090B",
        dim_fg: "#8A8F98", // ink-subtle
        destructive: "#E5484D",
        // Linear resists drop shadows on dark — kept minimal; the surface
        // ladder + hairline do the lifting.
        shadow_sm: "rgba(0,0,0,0.30)",
        shadow_md: "rgba(0,0,0,0.45)",
    };

    /// Off-white "paper" light scheme (never pure-white) with obsidian ink text.
    const LIGHT: Pal = Pal {
        window_fg: "#1C1D21",
        window_bg: "#F8F7F3",
        view_bg: "#F4F2ED",
        headerbar_bg: "#d7cfcb",
        card_bg: "#FFFFFF",
        popover_bg: "#F6F3EE",
        card_hover: "#FFFFFF",
        thumb_mat: "#0c0c0b",
        card_border: "rgba(20,20,22,0.10)",
        dim_fg: "#212126",
        destructive: "#DC2626",
        shadow_sm: "rgba(40,38,33,0.07)",
        shadow_md: "rgba(40,38,33,0.13)",
    };
}

/// Resolve the `(accent_bg_color, accent_fg_color)` pair for an accent in the
/// given scheme. The default Blue is Linear's lavender-blue signature; the rest
/// stay tasteful, modestly-saturated hues.
fn accent_pair(accent: Accent, dark: bool) -> (&'static str, &'static str) {
    match (accent, dark) {
        // Linear lavender-blue (#5e6ad2) — brand accent / primary CTA / focus.
        (Accent::Blue, true) => ("#5E6AD2", "#F7F8FF"),
        (Accent::Blue, false) => ("#5058C4", "#F7F8FF"),
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

/// Retained for call-site ordering; the provider is now created and attached by
/// [`apply`], which the caller invokes right after. No-op on its own.
pub fn install() {}

/// Swap in a freshly-built stylesheet for `accent` + dark/light. Live-safe: a
/// running window re-resolves and repaints, so the toggle and dots work.
#[allow(deprecated)]
pub fn apply(accent: Accent, dark: bool) {
    let Some(display) = gtk4::gdk::Display::default() else {
        return;
    };
    let css = build_css(accent, dark);
    let new = gtk4::CssProvider::new();
    new.load_from_data(&css);
    gtk4::style_context_add_provider_for_display(
        &display,
        &new,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    PROVIDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(old) = slot.take() {
            gtk4::style_context_remove_provider_for_display(&display, &old);
        }
        *slot = Some(new);
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

/// Whether `mode` should paint dark, right now. For explicit Light/Dark this is
/// the mode itself — critically NOT `is_dark()`, which updates asynchronously
/// after `set_mode` and so is stale in the same call (the "clicking Light did
/// nothing" bug). Only `System` defers to the resolved scheme.
pub fn resolve_dark(mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => is_dark(),
    }
}

/// Build the full stylesheet for the given accent + scheme.
fn build_css(accent: Accent, dark: bool) -> String {
    let p = if dark { &Pal::DARK } else { &Pal::LIGHT };
    let (accent_bg, accent_fg) = accent_pair(accent, dark);
    // Smoked-glass opacity is scheme-dependent. On dark, content behind a
    // translucent dark panel stays hidden (dark on dark). On light, the dark
    // TEXT behind bleeds straight through a light panel and collides with the
    // modal's own text, so light needs a much higher opacity to stay readable.
    let glass_alpha = if dark { "0.86" } else { "0.97" };

    let css = format!(
        "/* ===== Surfaces + type =====
   Deliberately NO @define-color block: GTK keeps an already-defined named
   color at its startup value, so redefining the palette here would freeze
   every libadwaita-drawn widget (window title, window controls, popover
   labels) in the scheme the app launched in. Colors are literal, and anything
   we don't style follows libadwaita's own scheme via set_mode(). */
window, .background {{ background-color: {window_bg}; color: {window_fg}; font-family: \"Inter\", \"Adwaita Sans\", \"Cantarell\", sans-serif; }}
headerbar, headerbar.flat {{ background-color: {headerbar_bg}; color: {window_fg}; box-shadow: none; border-bottom: 1px solid {card_border}; }}
headerbar label, headerbar .title {{ color: {window_fg}; }}
/* Window controls + header buttons: libadwaita would otherwise draw these in
   the frozen startup foreground, which is invisible after a live switch. */
windowcontrols button image, headerbar button image {{ color: {window_fg}; }}
popover label, popover > contents label {{ color: {window_fg}; }}

/* ===== Glass modals ===== translucent surface (readable), glass edge + sheen.
   GTK4 CSS has no backdrop blur, so this is a high-opacity smoked-glass look. */
window.glass {{ background-color: alpha(@window_bg_color, {glass_alpha}); border: 1px solid alpha(@window_fg_color, 0.14); border-radius: 22px; box-shadow: inset 0 1px 0 alpha(@window_fg_color, 0.08), 0 32px 80px {shadow_md}, 0 4px 14px {shadow_sm}; }}
window.glass headerbar, window.glass headerbar.flat {{ background: transparent; box-shadow: none; border-bottom: 1px solid alpha(@window_fg_color, 0.07); }}
window.glass .background {{ background: transparent; }}

.overline {{ font-size: 11px; font-weight: 600; letter-spacing: 0.05em; color: @dim_fg; }}
.dim {{ color: @dim_fg; }}
.dialog-heading {{ font-size: 19px; font-weight: 700; letter-spacing: -0.01em; }}
.dialog-sub {{ color: @dim_fg; font-size: 13px; }}

/* ===== Changelog modal (bigger, readable) ===== */
.changelog-title {{ font-size: 23px; font-weight: 700; letter-spacing: -0.01em; }}
.changelog-section {{ font-size: 13px; font-weight: 700; letter-spacing: 0.07em; color: @accent_color; }}
.changelog-body {{ font-size: 15px; }}

/* ===== Wallpaper card ===== */
.wp-card {{ background-color: @card_bg_color; border: 1px solid @card_border; border-radius: 12px; box-shadow: 0 1px 2px {shadow_sm}; transition: box-shadow 160ms cubic-bezier(0.4, 0, 0.2, 1), border-color 160ms cubic-bezier(0.4, 0, 0.2, 1), background-color 160ms cubic-bezier(0.4, 0, 0.2, 1); }}
.wp-card:hover {{ background-color: @card_hover; border-color: alpha(@accent_bg_color,0.55); box-shadow: 0 8px 24px {shadow_md}, 0 2px 6px {shadow_sm}; }}
.wp-card.active {{ border: 2px solid @accent_bg_color; box-shadow: 0 1px 3px {shadow_sm}; }}

.wp-thumb {{ background-color: @thumb_mat; transition: opacity 260ms cubic-bezier(0.4, 0, 0.2, 1); }}
/* Thumbnail fade-in: cards start with .thumb-loading (opacity 0); an idle
   callback removes it right after map, so the opacity transition plays. */
.wp-thumb.thumb-loading {{ opacity: 0; }}
.wp-scrim {{ background: linear-gradient(to top, alpha(black,0.84) 0%, alpha(black,0.40) 52%, alpha(black,0) 100%); padding: 24px 12px 9px 12px; }}
.wp-title {{ color: #FFFFFF; font-weight: 600; font-size: 13px; letter-spacing: -0.01em; text-shadow: 0 1px 3px rgba(0,0,0,0.6); }}
.wp-meta {{ color: rgba(255,255,255,0.72); font-size: 11px; text-shadow: 0 1px 2px rgba(0,0,0,0.6); }}
.wp-fav-glyph {{ color: @accent_bg_color; font-size: 11px; text-shadow: 0 1px 2px rgba(0,0,0,0.6); }}
.wp-badge {{ background-color: alpha(black,0.55); color: #FFFFFF; font-size: 9px; font-weight: 700; letter-spacing: 0.05em; padding: 2px 7px; border-radius: 7px; margin: 8px; }}
/* 4K quality badge sits in the same top-left row as the kind badge; the row
   carries the outer margin, so badges inside it only keep a small gap. */
.wp-badge-row {{ margin: 8px; }}
.wp-badge-row .wp-badge {{ margin: 0 6px 0 0; }}
.wp-badge.quality {{ background-color: alpha(@accent_bg_color,0.85); color: @accent_fg_color; }}
/* ===== Select mode ===== */
.wp-card.picked {{ border: 2px solid @accent_bg_color; }}
.wp-check {{ background-color: alpha(black,0.55); color: @accent_fg_color; border: 2px solid #FFFFFF; border-radius: 999px; min-width: 22px; min-height: 22px; margin: 8px; font-size: 13px; font-weight: 700; }}
.wp-check.on {{ background-color: @accent_bg_color; border-color: @accent_fg_color; }}

.wp-active-pill {{ background-color: @accent_bg_color; color: @accent_fg_color; font-size: 9px; font-weight: 700; letter-spacing: 0.04em; padding: 2px 8px; border-radius: 7px; margin: 8px; }}
.wp-edit {{ background-color: alpha(black,0.55); color: #FFFFFF; border-radius: 999px; min-height: 26px; min-width: 26px; padding: 3px; transition: background-color 150ms cubic-bezier(0.4, 0, 0.2, 1), color 150ms cubic-bezier(0.4, 0, 0.2, 1); }}
.wp-edit:hover {{ background-color: alpha(black,0.78); }}
.wp-edit.fav-on {{ color: @accent_bg_color; }}
/* Hover action cluster (heart / edit / menu), bottom-right. */
.wp-actions {{ margin: 8px; }}

/* ===== Compact layout (narrow window; see LayoutBucket in window.rs) ===== */
.compact-layout .wp-scrim {{ padding: 16px 8px 6px 8px; }}
.compact-layout .wp-badge, .compact-layout .wp-active-pill {{ margin: 6px; }}

/* ===== Mini card ===== */
.wp-mini {{ background-color: @card_bg_color; border: 1px solid @card_border; border-radius: 10px; box-shadow: 0 1px 2px {shadow_sm}; transition: box-shadow 150ms cubic-bezier(0.4, 0, 0.2, 1), border-color 150ms cubic-bezier(0.4, 0, 0.2, 1); }}
.wp-mini:hover {{ border-color: alpha(@accent_bg_color,0.45); box-shadow: 0 5px 14px {shadow_md}; }}
.wp-mini.active {{ border: 2px solid @accent_bg_color; }}

/* Placeholder shown inside a card while (or if) no thumbnail exists. */
.wp-placeholder {{ background-color: @thumb_mat; }}
.wp-placeholder image {{ color: @dim_fg; }}

/* ===== Status ===== */
.status-pill {{ background-color: @card_bg_color; border: 1px solid @card_border; border-radius: 999px; padding: 2px 12px; color: @dim_fg; font-size: 12px; }}
.pill-overline {{ font-size: 9px; font-weight: 700; letter-spacing: 0.09em; color: @dim_fg; }}
.pill-name {{ font-size: 12px; font-weight: 600; color: @window_fg_color; }}
.dot-ok {{ color: #3FB950; }}
.dot-warn {{ color: #D29922; }}
.dot-off {{ color: @dim_fg; }}

/* ===== What's-new banner ===== */
.banner {{ background-color: alpha(@accent_bg_color,0.10); border: 1px solid alpha(@accent_bg_color,0.28); border-radius: 12px; padding: 5px 6px 5px 14px; }}

/* ===== Buttons ===== */
.set-btn {{ min-height: 42px; font-size: 14px; font-weight: 600; border-radius: 12px; }}
.feedback-btn {{ min-height: 38px; padding-left: 14px; padding-right: 14px; border-radius: 11px; }}
button {{ border-radius: 9px; transition: background-color 130ms cubic-bezier(0.4, 0, 0.2, 1), border-color 130ms cubic-bezier(0.4, 0, 0.2, 1), box-shadow 130ms cubic-bezier(0.4, 0, 0.2, 1); }}
button.suggested-action {{ font-weight: 600; background-color: @accent_bg_color; color: @accent_fg_color; }}
button.suggested-action:hover {{ background-color: alpha(@accent_bg_color, 0.85); }}
.seg button:checked {{ background-color: @accent_bg_color; color: @accent_fg_color; }}

/* ===== Accent picker dots ===== */
.accent-dot {{ min-width: 22px; min-height: 22px; border-radius: 999px; padding: 0; border: 2px solid transparent; transition: border-color 130ms cubic-bezier(0.4, 0, 0.2, 1); }}
.accent-dot.selected {{ border-color: @window_fg_color; }}
.accent-blue {{ background-image: none; background-color: #5E6AD2; }}
.accent-teal {{ background-image: none; background-color: #1FAE9A; }}
.accent-green {{ background-image: none; background-color: #3AAE5C; }}
.accent-amber {{ background-image: none; background-color: #CC9233; }}
.accent-coral {{ background-image: none; background-color: #E85C7A; }}
.accent-graphite {{ background-image: none; background-color: #7A8392; }}

/* ===== Update progress ===== shimmering status text (GTK CSS can't clip a
   gradient to text, so the wave is a smooth animated color cycle through the
   accent) over a slim rounded bar. */
@keyframes fresco-shimmer {{
  0%   {{ color: @dim_fg; }}
  35%  {{ color: @accent_bg_color; }}
  65%  {{ color: shade(@accent_bg_color, 1.35); }}
  100% {{ color: @dim_fg; }}
}}
.shimmer {{ font-size: 15px; font-weight: 600; letter-spacing: -0.01em; animation: fresco-shimmer 2.2s ease-in-out infinite; }}
.update-progress {{ min-height: 8px; }}
.update-progress trough {{ min-height: 8px; border-radius: 999px; background-color: alpha(@window_fg_color, 0.08); }}
.update-progress progress {{ min-height: 8px; border-radius: 999px; background-image: linear-gradient(to right, @accent_bg_color, shade(@accent_bg_color, 1.25)); }}

/* ===== Search ===== */
entry.wp-search {{ min-height: 36px; border-radius: 10px; background-color: @card_bg_color; border: 1px solid @card_border; box-shadow: none; padding-left: 8px; padding-right: 8px; transition: border-color 130ms cubic-bezier(0.4, 0, 0.2, 1), box-shadow 130ms cubic-bezier(0.4, 0, 0.2, 1); }}
entry.wp-search image {{ color: @dim_fg; margin-right: 4px; }}
entry.wp-search:focus-within {{ border-color: @accent_bg_color; box-shadow: 0 0 0 2px alpha(@accent_bg_color, 0.22); }}

/* ===== Header menu popover ===== compact grouped menu, GTK-menu-like rows.
   Clean lifted panel: drop the default popover frame's extra outer border and
   define the panel by surface lift + one soft shadow instead. */
popover.background {{ background: none; box-shadow: none; border: none; }}
popover > contents {{ background-color: @popover_bg_color; border: none; border-radius: 14px; padding: 4px; box-shadow: 0 14px 40px alpha(black, 0.55), 0 3px 10px alpha(black, 0.4); }}
.fresco-menu .overline {{ margin-top: 8px; margin-bottom: 2px; }}
.fresco-menu separator {{ min-height: 1px; background-color: @card_border; margin-top: 4px; margin-bottom: 4px; }}
.menu-row {{ min-height: 30px; }}
.menu-item {{ min-height: 22px; padding: 3px 8px; border-radius: 7px; background-image: none; background-color: transparent; border: none; box-shadow: none; font-weight: 400; transition: background-color 130ms cubic-bezier(0.4, 0, 0.2, 1); }}
.menu-item:hover {{ background-color: alpha(@window_fg_color, 0.07); }}
.menu-item:active {{ background-color: alpha(@window_fg_color, 0.11); }}
.fresco-menu .seg button {{ min-height: 24px; font-size: 13px; }}

/* ===== Footer action bar ===== */
.footer-bar {{ border-top: 1px solid @card_border; background-color: @headerbar_bg_color; padding: 8px 16px; }}
.compact-layout .footer-bar {{ padding: 8px 8px; }}
.footer-bar button {{ min-height: 26px; }}
.footer-count {{ color: @dim_fg; font-size: 12px; }}

/* ===== Command palette (Ctrl+K) ===== */
.palette-entry {{ min-height: 44px; font-size: 16px; border-radius: 12px; background-color: @card_bg_color; border: 1px solid @card_border; box-shadow: none; padding-left: 12px; padding-right: 12px; transition: border-color 130ms cubic-bezier(0.4, 0, 0.2, 1), box-shadow 130ms cubic-bezier(0.4, 0, 0.2, 1); }}
.palette-entry:focus-within {{ border-color: @accent_bg_color; box-shadow: 0 0 0 2px alpha(@accent_bg_color, 0.22); }}
.palette-list {{ background: transparent; }}
.palette-list row {{ border-radius: 10px; padding: 8px 12px; transition: background-color 130ms cubic-bezier(0.4, 0, 0.2, 1); }}
.palette-list row:hover {{ background-color: alpha(@window_fg_color, 0.06); }}
.palette-list row:selected {{ background-color: alpha(@accent_bg_color, 0.18); color: @window_fg_color; }}
.palette-hint {{ color: @dim_fg; font-size: 11px; }}

/* ===== Accent audit: switches follow the accent (Adwaita default is blue) ===== */
switch:checked {{ background-color: @accent_bg_color; }}

/* ===== Semantic colors =====
   Define these ourselves; anything left undefined falls back to Adwaita's
   defaults, which are tuned for Adwaita's palette and not ours. */
@define-color destructive_fg_color #FFFFFF;
@define-color error_color {destructive};
@define-color warning_color #D29922;

/* A FLAT destructive row (the Remove-from-library item in the card right-click
   menu) must take the destructive COLOR for its text. Adwaita's
   .destructive-action paints a filled red button and sets the label to
   destructive_fg_color (white) — once .flat strips the red fill, white-on-paper
   is invisible in the light scheme. Restate both so the classes compose. */
button.flat.destructive-action {{ background-image: none; background-color: transparent; color: @destructive_color; }}
button.flat.destructive-action:hover {{ background-color: alpha(@destructive_color, 0.12); color: @destructive_color; }}
button.flat.destructive-action:active {{ background-color: alpha(@destructive_color, 0.18); color: @destructive_color; }}

/* An error label also carries .dim (for wrapping/type); .dim would otherwise
   win and render the message as ordinary grey text. */
label.error, label.error.dim {{ color: @destructive_color; }}

/* Capability notice (e.g. GNOME Wayland static fallback) — was styled by no
   rule at all, so it rendered as bare unframed text. */
.capability-banner {{ background-color: alpha(@window_fg_color, 0.05); border: 1px solid @card_border; border-radius: 12px; padding: 8px 12px; }}

/* Crop editor + transition preview stage: give the media a defined edge
   instead of floating on the window background. */
.crop-frame {{ background-color: @thumb_mat; border: 1px solid @card_border; border-radius: 12px; }}

/* ===== Misc ===== */
.welcome-cta {{ min-height: 40px; border-radius: 11px; font-weight: 600; }}
",
        window_bg = p.window_bg,
        window_fg = p.window_fg,
        headerbar_bg = p.headerbar_bg,
        card_border = p.card_border,
        destructive = p.destructive,
        shadow_sm = p.shadow_sm,
        shadow_md = p.shadow_md,
        glass_alpha = glass_alpha,
    );
    // Substitute literals: a redefined @define-color name keeps its startup
    // value at runtime, which froze the light/dark toggle and accent picker.
    css.replace("@window_bg_color", p.window_bg)
        .replace("@window_fg_color", p.window_fg)
        .replace("@view_bg_color", p.view_bg)
        .replace("@view_fg_color", p.window_fg)
        .replace("@card_bg_color", p.card_bg)
        .replace("@card_fg_color", p.window_fg)
        .replace("@headerbar_bg_color", p.headerbar_bg)
        .replace("@headerbar_fg_color", p.window_fg)
        .replace("@popover_bg_color", p.popover_bg)
        .replace("@popover_fg_color", p.window_fg)
        .replace("@card_border", p.card_border)
        .replace("@card_hover", p.card_hover)
        .replace("@thumb_mat", p.thumb_mat)
        .replace("@dim_fg", p.dim_fg)
        .replace("@destructive_bg_color", p.destructive)
        .replace("@destructive_color", p.destructive)
        .replace("@accent_bg_color", accent_bg)
        .replace("@accent_fg_color", accent_fg)
        .replace("@accent_color", accent_bg)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `@name` reference in a declaration keeps its startup value at runtime,
    /// which is what killed live theme switching. Literals only.
    #[test]
    fn build_css_has_no_named_color_references() {
        // A *reference* (`@name` in a declaration) is what freezes; the
        // `@define-color` / `@keyframes` at-rules themselves are fine.
        let is_reference = |line: &str| {
            let line = line.trim();
            line.match_indices('@').any(|(i, _)| {
                let rest = &line[i + 1..];
                !rest.starts_with("define-color") && !rest.starts_with("keyframes")
            }) && !line.starts_with("/*")
                && !line.starts_with("Deliberately")
        };
        for accent in [
            Accent::Blue,
            Accent::Teal,
            Accent::Green,
            Accent::Amber,
            Accent::Coral,
            Accent::Graphite,
        ] {
            for dark in [true, false] {
                let css = build_css(accent, dark);
                let bad: Vec<&str> = css.lines().filter(|l| is_reference(l)).collect();
                assert!(bad.is_empty(), "frozen color reference: {bad:?}");
            }
        }
    }

    /// Light and dark must differ, so switching modes repaints.
    #[test]
    fn light_and_dark_palettes_differ() {
        let dark = build_css(Accent::Blue, true);
        let light = build_css(Accent::Blue, false);
        assert_ne!(dark, light);
        assert!(dark.contains(Pal::DARK.window_bg));
        assert!(light.contains(Pal::LIGHT.window_bg));
    }

    /// Each accent must reach the stylesheet, so the dots repaint.
    #[test]
    fn accent_reaches_the_stylesheet() {
        for accent in [Accent::Blue, Accent::Teal, Accent::Amber, Accent::Coral] {
            let (bg, _) = accent_pair(accent, true);
            assert!(build_css(accent, true).contains(bg), "{accent:?} missing");
        }
    }
}
