pub mod autostart;
pub mod config;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(feature = "gui")]
pub mod gui;
pub mod ipc;

/// Application ID used for the desktop file, autostart entry, and GTK app.
pub const APP_ID: &str = "io.github.dibbayajyotiroy.Fresco";
pub const APP_NAME: &str = "Fresco";
/// WM_CLASS of wallpaper windows, so users/extensions can target them.
pub const WALLPAPER_WM_CLASS: &str = "fresco-wallpaper";
