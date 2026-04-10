//! System tray icon for the RoninKB daemon.
//!
//! Gated behind the `tray` feature flag because `tray-icon` pulls in system
//! dependencies (AppKit on macOS, GTK on Linux, Win32 on Windows) that we
//! don't want in headless / CI builds of the daemon.
//!
//! ## Platform caveats
//!
//! * **Linux (X11/GTK)** — works out of the box with a simple polling loop;
//!   `tray-icon` dispatches menu events through a global channel.
//! * **Windows** — works, but the tray requires a message pump on the thread
//!   that created the icon. We drive it from the main thread via
//!   [`TrayController::poll`] on a timer. That's good enough for a menu with
//!   a single "Quit" item.
//! * **macOS** — the icon *appears* but menu events are only delivered when a
//!   Cocoa `NSApplication` run loop is pumping them. We don't start one (that
//!   would require `tao`/`winit` and significantly more code), so on macOS the
//!   tray is effectively cosmetic in v1: the icon shows up, but clicking
//!   "Quit" won't trigger a shutdown. Users can still `Ctrl+C` or `launchctl
//!   unload` the LaunchAgent. The code path *compiles* and *runs* without
//!   panicking so we can ship a single binary.
//!
//! None of this matters for tests — the whole module is `#![cfg(feature =
//! "tray")]`, and tests run without the feature.

#![cfg(feature = "tray")]

use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

/// Actions the tray menu can request of the main loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// No pending event.
    None,
    /// User asked to reconnect the keyboard.
    Reconnect,
    /// User asked to quit the daemon.
    Quit,
}

/// Owns the tray icon and the menu item IDs we care about. Dropping this
/// value removes the icon from the tray.
pub struct TrayController {
    // Kept alive so the icon isn't dropped. We don't read the field.
    _tray: TrayIcon,
    quit_id: MenuId,
    reconnect_id: MenuId,
}

impl TrayController {
    /// Build the tray icon, menu, and return a handle. The caller is
    /// responsible for driving [`poll`](Self::poll) on the main thread.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let menu = Menu::new();
        let header = MenuItem::new("RoninKB Daemon", false, None);
        let reconnect = MenuItem::new("Reconnect Device", true, None);
        let quit = MenuItem::new("Quit RoninKB Daemon", true, None);
        let sep_a = PredefinedMenuItem::separator();
        let sep_b = PredefinedMenuItem::separator();
        menu.append_items(&[&header, &sep_a, &reconnect, &sep_b, &quit])?;

        let reconnect_id = reconnect.id().clone();
        let quit_id = quit.id().clone();

        let icon_bytes = include_bytes!("../assets/tray-icon.png");
        let icon = tray_icon::Icon::from_rgba(decode_rgba(icon_bytes)?, 32, 32)?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("RoninKB Daemon")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            _tray: tray,
            quit_id,
            reconnect_id,
        })
    }

    /// Drain one pending menu event, if any. Returns
    /// [`TrayAction::None`] when the queue is empty.
    pub fn poll(&self) -> TrayAction {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit_id {
                return TrayAction::Quit;
            }
            if event.id == self.reconnect_id {
                return TrayAction::Reconnect;
            }
        }
        TrayAction::None
    }
}

/// Decode an embedded PNG into an RGBA8 byte buffer suitable for
/// [`tray_icon::Icon::from_rgba`].
fn decode_rgba(png_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    Ok(rgba.into_raw())
}
