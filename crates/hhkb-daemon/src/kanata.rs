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

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use directories::ProjectDirs;
use serde::Serialize;

use crate::error::ApiError;

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
        let binary = self
            .binary_path
            .as_ref()
            .ok_or(ApiError::KanataNotInstalled)?;

        let mut guard = self.process.lock().expect("kanata process mutex poisoned");
        if guard.is_some() {
            return Err(ApiError::KanataAlreadyRunning);
        }

        // Make sure the file exists so kanata doesn't immediately exit.
        if !self.config_path.exists() {
            std::fs::write(&self.config_path, "")?;
        }

        let child = Command::new(binary)
            .arg("--cfg")
            .arg(&self.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let pid = child.id();
        tracing::info!(pid, path = %binary.display(), "spawned kanata");
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
                tracing::warn!(?status, "kanata exited");
                *guard = None;
                KanataStatus::Stopped
            }
            Ok(None) => KanataStatus::Running { pid: child.id() },
            Err(e) => {
                tracing::warn!(%e, "kanata try_wait failed");
                KanataStatus::Running { pid: child.id() }
            }
        }
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
    if let Ok(p) = which::which("kanata") {
        return Some(p);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let cargo_bin = PathBuf::from(home).join(".cargo").join("bin").join("kanata");
        if cargo_bin.is_file() {
            return Some(cargo_bin);
        }
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
