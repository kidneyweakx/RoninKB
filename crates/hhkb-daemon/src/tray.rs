//! System tray icon for the RoninKB daemon.
//!
//! Gated behind the `tray` feature flag. Uses `winit` to drive the native
//! event loop on all platforms (required by AppKit on macOS so that menu
//! events are actually delivered; on Windows and Linux it is optional but
//! keeps the code uniform).
//!
//! ## Menu layout
//!
//! ```text
//! RoninKB Daemon          ← disabled header
//! ─────────────────────
//! Open Web UI             ← opens http://127.0.0.1:7331/ui/
//! ─────────────────────
//! ✓ Launch at Login       ← CheckMenuItem, toggles autostart
//! ─────────────────────
//! Reconnect Device
//! ─────────────────────
//! Quit RoninKB Daemon
//! ```

#![cfg(feature = "tray")]

use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

use crate::autostart;

/// Actions the tray menu can request of the main loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// No pending event.
    None,
    /// User asked to open the web UI.
    OpenUi,
    /// User asked to toggle the login-item / autostart entry.
    ToggleAutostart,
    /// User asked to reconnect the keyboard.
    Reconnect,
    /// User asked to quit the daemon.
    Quit,
}

/// Owns the tray icon and the menu item IDs we care about. Dropping this
/// value removes the icon from the tray.
pub struct TrayController {
    // Kept alive so the icon is not dropped.
    _tray: TrayIcon,
    open_ui_id: MenuId,
    autostart_id: MenuId,
    reconnect_id: MenuId,
    quit_id: MenuId,
    /// The CheckMenuItem handle — we need it to flip the checked state.
    autostart_item: CheckMenuItem,
}

impl TrayController {
    /// Build the tray icon + menu. Must be called on the main thread (AppKit
    /// requirement on macOS).
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let menu = Menu::new();

        let header = MenuItem::new("RoninKB Daemon", false, None);
        let sep_a = PredefinedMenuItem::separator();
        let open_ui = MenuItem::new("Open Web UI", true, None);
        let sep_b = PredefinedMenuItem::separator();
        let autostart_enabled = autostart::is_enabled();
        let autostart = CheckMenuItem::new("Launch at Login", true, autostart_enabled, None);
        let sep_c = PredefinedMenuItem::separator();
        let reconnect = MenuItem::new("Reconnect Device", true, None);
        let sep_d = PredefinedMenuItem::separator();
        let quit = MenuItem::new("Quit RoninKB Daemon", true, None);

        menu.append_items(&[
            &header, &sep_a, &open_ui, &sep_b, &autostart, &sep_c, &reconnect, &sep_d, &quit,
        ])?;

        let open_ui_id = open_ui.id().clone();
        let autostart_id = autostart.id().clone();
        let reconnect_id = reconnect.id().clone();
        let quit_id = quit.id().clone();

        let icon_bytes = include_bytes!("../assets/tray-icon.png");
        let icon = tray_icon::Icon::from_rgba(decode_rgba(icon_bytes)?, 32, 32)?;

        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("RoninKB Daemon")
            .with_icon(icon);

        // On macOS, mark the icon as a template image so AppKit auto-tints
        // it to match the menu bar text color (white on dark menu bar, black
        // on light). This is what every well-behaved tray app does — only
        // the alpha channel matters, color is taken from the system.
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_as_template(true);

        let tray = builder.build()?;

        Ok(Self {
            _tray: tray,
            open_ui_id,
            autostart_id,
            reconnect_id,
            quit_id,
            autostart_item: autostart,
        })
    }

    /// Drain one pending menu event. Returns [`TrayAction::None`] when the
    /// queue is empty.
    pub fn poll(&self) -> TrayAction {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit_id {
                return TrayAction::Quit;
            }
            if event.id == self.open_ui_id {
                return TrayAction::OpenUi;
            }
            if event.id == self.autostart_id {
                return TrayAction::ToggleAutostart;
            }
            if event.id == self.reconnect_id {
                return TrayAction::Reconnect;
            }
        }
        TrayAction::None
    }

    /// Flip the checked state of the "Launch at Login" menu item to match the
    /// actual autostart registration state.
    pub fn sync_autostart_check(&self) {
        self.autostart_item.set_checked(autostart::is_enabled());
    }
}

/// Decode an embedded PNG into an RGBA8 byte buffer.
fn decode_rgba(png_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    Ok(rgba.into_raw())
}
