//! Kanata integration — child-process supervision + hot reload.
//!
//! Kanata is the software layer for RoninKB. The daemon spawns it as a child
//! process pointed at an on-disk `.kbd` file, and hot-reloads when the active
//! profile changes:
//!
//! - **Unix:** write the file then send `SIGUSR1` — kanata re-reads its config
//!   in place without dropping modifiers.
//! - **Windows:** kanata doesn't honour signals, so we stop and restart the
//!   process.
//!
//! This manager is intentionally minimal — it is *not* a full supervisor.
//! Callers should poll [`KanataManager::check_alive`] if they need to observe
//! crashes.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use directories::ProjectDirs;
use serde::Serialize;

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Bundled binary (feature = "bundled-kanata")
// ---------------------------------------------------------------------------

/// Raw bytes of the kanata binary downloaded at build time.
/// Only present when compiled with `--features bundled-kanata`.
#[cfg(feature = "bundled-kanata")]
static BUNDLED_KANATA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/kanata-bundle"));

/// Extract the bundled kanata binary to a discoverable location and return
/// its path. Re-extracts only when the on-disk size differs (i.e. the
/// daemon was rebuilt with a newer bundled version).
///
/// On macOS we wrap kanata in a thin `.app` bundle inside `~/Applications/`
/// so it shows up alongside other apps in the System Settings → Input
/// Monitoring picker. On other platforms it lands under the platform's
/// user-data directory (`~/.local/share/roninKB/bin/kanata` etc.).
#[cfg(feature = "bundled-kanata")]
fn extract_bundled_kanata() -> Option<PathBuf> {
    use std::io::Write as _;

    #[cfg(target_os = "macos")]
    let (path, info_plist_path) = {
        let app_root = macos_kanata_app_root()?;
        let macos_dir = app_root.join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos_dir).ok()?;
        (
            macos_dir.join("kanata"),
            Some(app_root.join("Contents").join("Info.plist")),
        )
    };

    #[cfg(not(target_os = "macos"))]
    let (path, info_plist_path): (PathBuf, Option<PathBuf>) = {
        let dirs = ProjectDirs::from("", "", "roninKB")?;
        let bin_dir = dirs.data_dir().join("bin");
        std::fs::create_dir_all(&bin_dir).ok()?;
        #[cfg(windows)]
        let bin = bin_dir.join("kanata.exe");
        #[cfg(not(windows))]
        let bin = bin_dir.join("kanata");
        (bin, None)
    };

    let expected = BUNDLED_KANATA.len() as u64;
    let needs_write = std::fs::metadata(&path)
        .map(|m| m.len() != expected)
        .unwrap_or(true);

    if needs_write {
        let mut f = std::fs::File::create(&path).ok()?;
        f.write_all(BUNDLED_KANATA).ok()?;
        drop(f);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).ok()?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).ok()?;
        }

        tracing::info!(path = %path.display(), bytes = expected, "extracted bundled kanata binary");
    }

    if let Some(plist) = info_plist_path.as_ref() {
        if !plist.exists() {
            let _ = std::fs::write(plist, KANATA_APP_INFO_PLIST);
            tracing::info!(path = %plist.display(), "wrote kanata Info.plist");
        }
    }

    #[cfg(target_os = "macos")]
    cleanup_legacy_kanata_extraction();

    Some(path)
}

/// Path to `~/Applications/RoninKB Kanata.app`. We prefer the per-user
/// Applications folder (no admin needed and macOS auto-creates it on demand
/// in Finder's sidebar) so the bundle is visible to the System Settings
/// picker without diving into Library.
#[cfg(all(target_os = "macos", feature = "bundled-kanata"))]
fn macos_kanata_app_root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let apps_dir = PathBuf::from(home).join("Applications");
    std::fs::create_dir_all(&apps_dir).ok()?;
    Some(apps_dir.join("RoninKB Kanata.app"))
}

/// Remove the legacy extraction at `~/Library/Application Support/roninKB/bin`
/// once we're sure the new `.app` location is populated. Best-effort —
/// silently ignores failures (file missing, permission denied, etc.).
#[cfg(all(target_os = "macos", feature = "bundled-kanata"))]
fn cleanup_legacy_kanata_extraction() {
    if let Some(dirs) = ProjectDirs::from("", "", "roninKB") {
        let legacy = dirs.data_dir().join("bin").join("kanata");
        if legacy.exists() {
            let _ = std::fs::remove_file(&legacy);
            tracing::info!(path = %legacy.display(), "removed legacy kanata extraction");
        }
    }
}

#[cfg(all(target_os = "macos", feature = "bundled-kanata"))]
const KANATA_APP_INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>kanata</string>
  <key>CFBundleIdentifier</key>
  <string>gg.solidarity.roninkb.kanata</string>
  <key>CFBundleName</key>
  <string>RoninKB Kanata</string>
  <key>CFBundleDisplayName</key>
  <string>RoninKB Kanata</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.11.0</string>
  <key>CFBundleVersion</key>
  <string>1.11.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
"#;

/// Observable state of the kanata child process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum KanataStatus {
    /// No kanata binary was found on `PATH` or in `~/.cargo/bin`.
    NotInstalled,
    /// Binary is available but no child process is running.
    Stopped,
    /// Child process is alive. `pid` is the OS process id at spawn time.
    Running { pid: u32 },
}

/// Manages the kanata child process + on-disk config file.
pub struct KanataManager {
    binary_path: Option<PathBuf>,
    config_path: PathBuf,
    process: Mutex<Option<Child>>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    last_error: Arc<Mutex<Option<String>>>,
    last_device_path: Arc<Mutex<Option<String>>>,
}

impl KanataManager {
    /// Construct a new manager, auto-detecting the kanata binary and
    /// resolving the default config path (`~/.local/share/roninKB/active.kbd`
    /// on Linux, the platform equivalent elsewhere).
    ///
    /// A missing binary is **not** an error — the manager simply reports
    /// [`KanataStatus::NotInstalled`] until one becomes available.
    pub fn new() -> Result<Self, ApiError> {
        let binary_path = detect_kanata_binary();
        let config_path = default_config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if let Some(bin) = binary_path.as_ref() {
            tracing::info!(path = %bin.display(), "detected kanata binary");
        } else {
            tracing::warn!("kanata binary not found on PATH or ~/.cargo/bin");
        }

        Ok(Self {
            binary_path,
            config_path,
            process: Mutex::new(None),
            stderr_tail: Arc::new(Mutex::new(VecDeque::new())),
            last_error: Arc::new(Mutex::new(None)),
            last_device_path: Arc::new(Mutex::new(None)),
        })
    }

    /// Construct a manager with explicit paths. Used by tests to guarantee
    /// a deterministic "no binary" state regardless of the developer's
    /// environment.
    #[doc(hidden)]
    pub fn with_paths(binary_path: Option<PathBuf>, config_path: PathBuf) -> Self {
        Self {
            binary_path,
            config_path,
            process: Mutex::new(None),
            stderr_tail: Arc::new(Mutex::new(VecDeque::new())),
            last_error: Arc::new(Mutex::new(None)),
            last_device_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Path to the config file kanata reads from. Useful for tests and UI.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Returns `true` if a kanata binary was detected at construction time.
    pub fn is_installed(&self) -> bool {
        self.binary_path.is_some()
    }

    /// Absolute path to the detected kanata binary, when installed.
    pub fn binary_path(&self) -> Option<PathBuf> {
        self.binary_path.clone()
    }

    /// Tail of stderr lines collected from the child process.
    pub fn stderr_tail(&self, max_lines: usize) -> Vec<String> {
        let guard = self
            .stderr_tail
            .lock()
            .expect("kanata stderr mutex poisoned");
        let take = max_lines.min(guard.len());
        guard
            .iter()
            .skip(guard.len().saturating_sub(take))
            .cloned()
            .collect()
    }

    /// Last start/runtime error captured by the manager.
    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .expect("kanata error mutex poisoned")
            .clone()
    }

    /// Linux-only `--device` path used for the current or most recent start.
    pub fn last_device_path(&self) -> Option<String> {
        self.last_device_path
            .lock()
            .expect("kanata device mutex poisoned")
            .clone()
    }

    /// macOS Input Monitoring / Accessibility preflight status.
    pub fn input_monitoring_granted(&self) -> Option<bool> {
        #[cfg(target_os = "macos")]
        {
            return Some(macos_input_monitoring_granted());
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    /// Current status. Does *not* poll the child — use [`check_alive`] for
    /// that.
    ///
    /// [`check_alive`]: Self::check_alive
    pub fn status(&self) -> KanataStatus {
        if self.binary_path.is_none() {
            return KanataStatus::NotInstalled;
        }
        let guard = self.process.lock().expect("kanata process mutex poisoned");
        match guard.as_ref() {
            Some(child) => KanataStatus::Running { pid: child.id() },
            None => KanataStatus::Stopped,
        }
    }

    /// Write `config` to the on-disk config file. Does **not** signal a
    /// reload — call [`reload`] for that.
    ///
    /// [`reload`]: Self::reload
    pub fn write_config(&self, config: &str) -> Result<(), ApiError> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.config_path, config)?;
        tracing::debug!(
            path = %self.config_path.display(),
            bytes = config.len(),
            "wrote kanata config",
        );
        Ok(())
    }

    /// Read the current on-disk config. Returns an empty string if the file
    /// does not yet exist.
    pub fn read_config(&self) -> Result<String, ApiError> {
        match std::fs::read_to_string(&self.config_path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(ApiError::KanataIo(e)),
        }
    }

    /// Spawn a new kanata child pointed at [`config_path`](Self::config_path).
    ///
    /// Returns [`ApiError::KanataNotInstalled`] if no binary was detected and
    /// [`ApiError::KanataAlreadyRunning`] if the manager is already tracking
    /// a live child.
    pub fn start(&self) -> Result<u32, ApiError> {
        const STARTUP_GRACE_MS: u64 = 120;

        let binary = self
            .binary_path
            .as_ref()
            .ok_or(ApiError::KanataNotInstalled)?;

        #[cfg(target_os = "macos")]
        if !macos_input_monitoring_granted() {
            let msg = "macOS Input Monitoring / Accessibility permission is required for kanata. \
Open System Settings -> Privacy & Security -> Input Monitoring, then allow RoninKB/kanata."
                .to_string();
            self.set_last_error(Some(msg.clone()));
            return Err(ApiError::KanataPermissionRequired(msg));
        }

        let mut guard = self.process.lock().expect("kanata process mutex poisoned");
        if guard.is_some() {
            return Err(ApiError::KanataAlreadyRunning);
        }

        // Make sure the file exists so kanata doesn't immediately exit.
        if !self.config_path.exists() {
            std::fs::write(&self.config_path, "")?;
        }

        self.clear_diagnostics();

        let mut cmd = Command::new(binary);
        cmd.arg("--cfg")
            .arg(&self.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(target_os = "linux")]
        {
            let device = detect_linux_device_path().ok_or_else(|| {
                ApiError::KanataDeviceUnavailable(
                    "failed to resolve keyboard event device. Set RONINKB_KANATA_DEVICE=/dev/input/eventX"
                        .to_string(),
                )
            })?;
            self.set_last_device_path(Some(device.display().to_string()));
            cmd.arg("--device").arg(device);
        }
        #[cfg(not(target_os = "linux"))]
        self.set_last_device_path(None);

        let mut child = cmd.spawn()?;
        if let Some(stderr) = child.stderr.take() {
            Self::spawn_stderr_pump(stderr, Arc::clone(&self.stderr_tail));
        }

        std::thread::sleep(Duration::from_millis(STARTUP_GRACE_MS));

        if let Some(status) = child.try_wait()? {
            let msg = self.compose_exit_diagnostic(status);
            self.set_last_error(Some(msg.clone()));
            tracing::warn!(status = ?status, "{msg}");
            *guard = None;
            return Err(ApiError::Internal(msg));
        }

        let pid = child.id();
        tracing::info!(pid, path = %binary.display(), "spawned kanata");
        self.set_last_error(None);
        *guard = Some(child);
        Ok(pid)
    }

    /// Terminate the running kanata child.
    ///
    /// On Unix this sends `SIGTERM` to allow a clean shutdown; on Windows
    /// we fall back to `Child::kill`. Returns [`ApiError::KanataNotRunning`]
    /// if there's nothing to stop.
    pub fn stop(&self) -> Result<(), ApiError> {
        let mut guard = self.process.lock().expect("kanata process mutex poisoned");
        let mut child = guard.take().ok_or(ApiError::KanataNotRunning)?;

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            let pid = Pid::from_raw(child.id() as i32);
            if let Err(e) = kill(pid, Signal::SIGTERM) {
                tracing::warn!(%e, "kanata SIGTERM failed; falling back to kill()");
                let _ = child.kill();
            }
        }
        #[cfg(not(unix))]
        {
            let _ = child.kill();
        }

        // Reap so we don't leave a zombie. Best-effort.
        let _ = child.wait();
        tracing::info!("kanata stopped");
        Ok(())
    }

    /// Hot-reload: write `new_config` to disk and ask kanata to re-read it.
    ///
    /// On Unix this sends `SIGUSR1` (which kanata handles as "reload cfg");
    /// on Windows we stop and restart the process. If kanata isn't currently
    /// running, the config file is updated and the call returns `Ok`.
    pub fn reload(&self, new_config: &str) -> Result<(), ApiError> {
        self.write_config(new_config)?;

        // If nothing is running we're done — the next start() will pick up
        // the new file.
        let status = self.status();
        match status {
            KanataStatus::NotInstalled | KanataStatus::Stopped => return Ok(()),
            KanataStatus::Running { pid } => {
                #[cfg(unix)]
                {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    let target = Pid::from_raw(pid as i32);
                    kill(target, Signal::SIGUSR1).map_err(|e| {
                        ApiError::Internal(format!("SIGUSR1 to kanata {pid} failed: {e}"))
                    })?;
                    tracing::info!(pid, "sent SIGUSR1 to kanata for reload");
                }
                #[cfg(not(unix))]
                {
                    // Windows: signal-based reload unsupported. Restart the
                    // process so the new config takes effect.
                    tracing::info!(pid, "restarting kanata to apply new config (windows)");
                    let _ = self.stop();
                    self.start()?;
                }
            }
        }
        Ok(())
    }

    /// Non-blocking check: did the child exit behind our back? If so, clear
    /// the slot and return [`KanataStatus::Stopped`].
    pub fn check_alive(&self) -> KanataStatus {
        if self.binary_path.is_none() {
            return KanataStatus::NotInstalled;
        }
        let mut guard = self.process.lock().expect("kanata process mutex poisoned");
        let Some(child) = guard.as_mut() else {
            return KanataStatus::Stopped;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                let msg = self.compose_exit_diagnostic(status);
                self.set_last_error(Some(msg.clone()));
                tracing::warn!(?status, "{msg}");
                *guard = None;
                KanataStatus::Stopped
            }
            Ok(None) => KanataStatus::Running { pid: child.id() },
            Err(e) => {
                tracing::warn!(%e, "kanata try_wait failed");
                self.set_last_error(Some(format!("kanata try_wait failed: {e}")));
                KanataStatus::Running { pid: child.id() }
            }
        }
    }

    fn clear_diagnostics(&self) {
        if let Ok(mut tail) = self.stderr_tail.lock() {
            tail.clear();
        }
        if let Ok(mut err) = self.last_error.lock() {
            *err = None;
        }
    }

    fn set_last_error(&self, value: Option<String>) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = value;
        }
    }

    fn set_last_device_path(&self, value: Option<String>) {
        if let Ok(mut guard) = self.last_device_path.lock() {
            *guard = value;
        }
    }

    fn compose_exit_diagnostic(&self, status: ExitStatus) -> String {
        let mut msg = format!("kanata exited immediately with status {status}");
        let tail = self.stderr_tail(8);
        if !tail.is_empty() {
            msg.push_str(". stderr: ");
            msg.push_str(&tail.join(" | "));
        }
        #[cfg(target_os = "macos")]
        if !macos_input_monitoring_granted() {
            msg.push_str(
                ". Hint: grant Input Monitoring / Accessibility permission in System Settings.",
            );
        }
        msg
    }

    fn spawn_stderr_pump(stderr: ChildStderr, tail: Arc<Mutex<VecDeque<String>>>) {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                if let Ok(mut guard) = tail.lock() {
                    guard.push_back(line.clone());
                    while guard.len() > 120 {
                        let _ = guard.pop_front();
                    }
                }
                tracing::debug!("kanata stderr: {line}");
            }
        });
    }
}

impl Drop for KanataManager {
    fn drop(&mut self) {
        // Best-effort cleanup so tests and daemon shutdown don't leak kanata.
        if let Ok(mut guard) = self.process.lock() {
            if let Some(mut child) = guard.take() {
                #[cfg(unix)]
                {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM);
                }
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn detect_kanata_binary() -> Option<PathBuf> {
    // 1. System PATH (e.g. /usr/local/bin/kanata)
    if let Ok(p) = which::which("kanata") {
        return Some(p);
    }
    // 2. Cargo install fallback (~/.cargo/bin/kanata)
    if let Some(home) = std::env::var_os("HOME") {
        let cargo_bin = PathBuf::from(home)
            .join(".cargo")
            .join("bin")
            .join("kanata");
        if cargo_bin.is_file() {
            return Some(cargo_bin);
        }
    }
    // 3. Bundled binary extracted from the daemon itself.
    #[cfg(feature = "bundled-kanata")]
    if let Some(p) = extract_bundled_kanata() {
        return Some(p);
    }
    None
}

fn default_config_path() -> Result<PathBuf, ApiError> {
    if let Some(dirs) = ProjectDirs::from("", "", "roninKB") {
        return Ok(dirs.data_dir().join("active.kbd"));
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("roninKB")
        .join("active.kbd"))
}

#[cfg(target_os = "linux")]
fn detect_linux_device_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("RONINKB_KANATA_DEVICE") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    let by_id = Path::new("/dev/input/by-id");
    if let Ok(entries) = std::fs::read_dir(by_id) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.ends_with("-event-kbd") && path.is_file() {
                return Some(path);
            }
        }
    }

    let input = Path::new("/dev/input");
    if let Ok(entries) = std::fs::read_dir(input) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.starts_with("event") && path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn macos_input_monitoring_granted() -> bool {
    // SAFETY: pure preflight API, no ownership crossing.
    unsafe { cg_preflight_listen_event_access() }
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CGPreflightListenEventAccess"]
    fn cg_preflight_listen_event_access() -> bool;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_config_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "roninKB-kanata-{}-{}.kbd",
            std::process::id(),
            name
        ));
        p
    }

    #[test]
    fn new_does_not_panic_without_binary() {
        // Just make sure construction succeeds regardless of PATH state.
        let mgr = KanataManager::new().expect("construct manager");
        // status() must not panic
        let _ = mgr.status();
    }

    #[test]
    fn status_reports_not_installed_when_binary_missing() {
        let mgr = KanataManager::with_paths(None, tmp_config_path("status"));
        assert_eq!(mgr.status(), KanataStatus::NotInstalled);
        assert_eq!(mgr.check_alive(), KanataStatus::NotInstalled);
        assert!(!mgr.is_installed());
    }

    #[test]
    fn status_reports_stopped_when_binary_present_no_child() {
        // A file path that is_file() returns true for works as a sentinel.
        let fake = tmp_config_path("sentinel-bin");
        std::fs::write(&fake, b"not a real binary").unwrap();
        let mgr = KanataManager::with_paths(Some(fake.clone()), tmp_config_path("stopped"));
        assert!(matches!(mgr.status(), KanataStatus::Stopped));
        let _ = std::fs::remove_file(&fake);
    }

    #[test]
    fn write_config_creates_file_with_content() {
        let path = tmp_config_path("write");
        let _ = std::fs::remove_file(&path);
        let mgr = KanataManager::with_paths(None, path.clone());
        let cfg = "(defsrc a)\n(deflayer base a)\n";
        mgr.write_config(cfg).expect("write ok");
        let read = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read, cfg);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_config_returns_empty_when_missing() {
        let path = tmp_config_path("read-missing");
        let _ = std::fs::remove_file(&path);
        let mgr = KanataManager::with_paths(None, path);
        assert_eq!(mgr.read_config().unwrap(), "");
    }

    #[test]
    fn start_without_binary_returns_not_installed() {
        let mgr = KanataManager::with_paths(None, tmp_config_path("start-nobin"));
        let err = mgr.start().unwrap_err();
        assert!(matches!(err, ApiError::KanataNotInstalled));
    }

    #[test]
    fn stop_without_running_returns_not_running() {
        let mgr = KanataManager::with_paths(None, tmp_config_path("stop-nobin"));
        let err = mgr.stop().unwrap_err();
        assert!(matches!(err, ApiError::KanataNotRunning));
    }

    #[test]
    fn reload_without_running_still_writes_config() {
        let path = tmp_config_path("reload-nobin");
        let _ = std::fs::remove_file(&path);
        let mgr = KanataManager::with_paths(None, path.clone());
        mgr.reload("(defsrc b)\n(deflayer base b)\n").unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("defsrc b"));
        let _ = std::fs::remove_file(&path);
    }
}
