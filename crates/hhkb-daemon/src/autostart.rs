//! Cross-platform login-item / autostart helper for hhkb-daemon.
//!
//! * **macOS** — writes / removes a LaunchAgent plist at
//!   `~/Library/LaunchAgents/com.roninKB.daemon.plist`.
//! * **Windows** — writes / removes a value in
//!   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//! * **Linux** — writes / removes a `.desktop` file at
//!   `~/.config/autostart/roninKB.desktop`.
//!
//! All functions are no-ops (returning `Ok(())` / `false`) when the platform
//! cannot be determined.

use anyhow::Result;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Register the daemon binary as a login item / startup entry.
pub fn enable() -> Result<()> {
    platform::enable()
}

/// Remove the login item / startup entry if it exists.
pub fn disable() -> Result<()> {
    platform::disable()
}

/// Return `true` if the daemon is currently registered to start at login.
pub fn is_enabled() -> bool {
    platform::is_enabled()
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod platform {
    use anyhow::{Context, Result};
    use directories::BaseDirs;
    use std::path::PathBuf;

    const LABEL: &str = "com.roninKB.daemon";

    fn plist_path() -> Option<PathBuf> {
        BaseDirs::new().map(|d| {
            d.home_dir()
                .join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist"))
        })
    }

    pub fn enable() -> Result<()> {
        let exe = std::env::current_exe().context("cannot determine daemon path")?;
        let exe_str = exe.to_string_lossy();

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_str}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>StandardOutPath</key>
    <string>/tmp/roninKB-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/roninKB-daemon.log</string>
</dict>
</plist>
"#
        );

        let path = plist_path().context("cannot determine LaunchAgents path")?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, plist)
            .with_context(|| format!("write plist {}", path.display()))?;
        tracing::info!("autostart: enabled via {}", path.display());
        Ok(())
    }

    pub fn disable() -> Result<()> {
        if let Some(path) = plist_path() {
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("remove plist {}", path.display()))?;
                tracing::info!("autostart: disabled (removed {})", path.display());
            }
        }
        Ok(())
    }

    pub fn is_enabled() -> bool {
        plist_path().map(|p| p.exists()).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod platform {
    use anyhow::{Context, Result};
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    const APP_NAME: &str = "RoninKB";
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn enable() -> Result<()> {
        let exe = std::env::current_exe().context("cannot determine daemon path")?;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey(RUN_KEY)
            .context("open Run registry key")?;
        key.set_value(APP_NAME, &exe.to_string_lossy().as_ref())
            .context("write registry value")?;
        tracing::info!("autostart: enabled via registry Run key");
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_WRITE) {
            let _ = key.delete_value(APP_NAME);
        }
        tracing::info!("autostart: disabled (removed registry value)");
        Ok(())
    }

    pub fn is_enabled() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey(RUN_KEY)
            .and_then(|k| k.get_value::<String, _>(APP_NAME))
            .is_ok()
    }
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod platform {
    use anyhow::{Context, Result};
    use directories::BaseDirs;
    use std::path::PathBuf;

    const FILE: &str = "roninKB.desktop";

    fn desktop_path() -> Option<PathBuf> {
        BaseDirs::new().map(|d| d.config_dir().join("autostart").join(FILE))
    }

    pub fn enable() -> Result<()> {
        let exe = std::env::current_exe().context("cannot determine daemon path")?;
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=RoninKB\nExec={}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        let path = desktop_path().context("cannot determine autostart dir")?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, content)
            .with_context(|| format!("write desktop file {}", path.display()))?;
        tracing::info!("autostart: enabled via {}", path.display());
        Ok(())
    }

    pub fn disable() -> Result<()> {
        if let Some(path) = desktop_path() {
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("remove desktop file {}", path.display()))?;
                tracing::info!("autostart: disabled (removed {})", path.display());
            }
        }
        Ok(())
    }

    pub fn is_enabled() -> bool {
        desktop_path().map(|p| p.exists()).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Fallback (other platforms)
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    use anyhow::Result;

    pub fn enable() -> Result<()> { Ok(()) }
    pub fn disable() -> Result<()> { Ok(()) }
    pub fn is_enabled() -> bool { false }
}
