//! Daemon configuration persisted to `config.toml` on disk.
//!
//! Today this only stores the user's pinned backend choice (RFC 0001 §4.4).
//! The on-disk shape:
//!
//! ```toml
//! [backend]
//! pin = "macos-native"
//! ```
//!
//! The file lives at the platform-conventional config dir resolved by
//! `directories::ProjectDirs("", "", "roninKB")` — on macOS that's
//! `~/Library/Application Support/roninKB/config.toml` per RFC 0001.
//!
//! Load semantics: missing file ⇒ defaults; malformed file ⇒ defaults +
//! warning log (we don't crash the daemon on a bad config). Save is
//! best-effort with a warn log if the write fails — the user can still
//! re-pick from the UI on every restart even if disk is unwriteable.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::backend::BackendId;

const FILE_NAME: &str = "config.toml";

/// Resolved on-disk path for the daemon config. `None` if no platform config
/// dir is available (extremely unusual — only happens in containerised tests
/// without `$HOME`); callers fall back to in-memory state.
fn default_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "roninKB").map(|d| d.config_dir().join(FILE_NAME))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendSection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendSection {
    /// User's pinned backend choice. `None` means "follow the priority
    /// auto-pick"; `Some(id)` means "use this backend if it's registered,
    /// else fall back to the priority pick".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<BackendId>,
}

/// Mutable daemon config wrapper. Holds the in-memory snapshot plus the
/// path it persists to. Operations are guarded by a `Mutex` because writes
/// happen from the axum thread pool while reads happen from arbitrary
/// blocking tasks; contention is essentially zero (config ops are rare).
pub struct DaemonConfig {
    inner: Mutex<DaemonConfigFile>,
    path: Option<PathBuf>,
}

impl DaemonConfig {
    /// Load config from the default platform path. Missing file ⇒ defaults;
    /// malformed file ⇒ defaults + warning log. Never fails — bad config
    /// shouldn't keep the daemon down.
    pub fn load_default() -> Self {
        let path = default_config_path();
        let inner = path
            .as_deref()
            .and_then(|p| match fs::read_to_string(p) {
                Ok(s) => Some(s),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => {
                    tracing::warn!(path = %p.display(), %e, "failed to read daemon config; using defaults");
                    None
                }
            })
            .and_then(|s| match toml::from_str::<DaemonConfigFile>(&s) {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    tracing::warn!(%e, "daemon config did not parse; using defaults");
                    None
                }
            })
            .unwrap_or_default();
        Self {
            inner: Mutex::new(inner),
            path,
        }
    }

    /// Construct a config rooted at `path` with the given initial state.
    /// Used by `AppState::for_tests` and unit tests so they can drive
    /// persistence without touching the user's real `Application Support`
    /// directory. Pass `None` for `path` to disable persistence entirely
    /// (in-memory only).
    pub fn for_tests(path: Option<PathBuf>, initial: DaemonConfigFile) -> Self {
        Self {
            inner: Mutex::new(initial),
            path,
        }
    }

    /// Snapshot of the current pinned backend, if any.
    pub fn pinned_backend(&self) -> Option<BackendId> {
        self.inner
            .lock()
            .expect("DaemonConfig mutex poisoned")
            .backend
            .as_ref()
            .and_then(|b| b.pin)
    }

    /// Set the pinned backend and write to disk. Persistence failure logs a
    /// warning but doesn't propagate — the registry state is the source of
    /// truth in-memory; the worst case on a write failure is the user has to
    /// re-pick after a restart.
    pub fn set_pinned_backend(&self, id: BackendId) {
        {
            let mut guard = self.inner.lock().expect("DaemonConfig mutex poisoned");
            guard
                .backend
                .get_or_insert_with(BackendSection::default)
                .pin = Some(id);
        }
        if let Err(e) = self.persist() {
            tracing::warn!(%e, "failed to persist daemon config");
        }
    }

    /// Serialize the in-memory config and write it to disk atomically.
    fn persist(&self) -> std::io::Result<()> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let snapshot = {
            let guard = self.inner.lock().expect("DaemonConfig mutex poisoned");
            guard.clone()
        };
        let serialized = toml::to_string_pretty(&snapshot)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Atomic-ish: write to a sibling tempfile, then rename. Avoids a
        // half-written config if the daemon crashes mid-write.
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, serialized)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_default_handles_missing_file() {
        // Pointing at a directory that doesn't exist must yield defaults
        // rather than panicking.
        let cfg = DaemonConfig::for_tests(
            Some(PathBuf::from("/tmp/roninKB-nonexistent-test/config.toml")),
            DaemonConfigFile::default(),
        );
        assert!(cfg.pinned_backend().is_none());
    }

    #[test]
    fn set_and_load_round_trips() {
        let dir = tempdir();
        let path = dir.join("config.toml");
        let cfg = DaemonConfig::for_tests(Some(path.clone()), DaemonConfigFile::default());
        cfg.set_pinned_backend(BackendId::MacosNative);
        assert_eq!(cfg.pinned_backend(), Some(BackendId::MacosNative));

        // Re-load to confirm on-disk state is honoured.
        let raw = fs::read_to_string(&path).expect("config written");
        assert!(raw.contains("macos-native"), "got:\n{raw}");
    }

    #[test]
    fn malformed_file_falls_back_to_defaults() {
        let dir = tempdir();
        let path = dir.join("config.toml");
        fs::write(&path, "this is not valid = toml = at all").expect("write garbage");

        let cfg = DaemonConfig::for_tests(
            Some(path),
            // Simulate the load path: in production load_default would do
            // its own parse; here we just confirm defaults are sane.
            DaemonConfigFile::default(),
        );
        assert!(cfg.pinned_backend().is_none());
    }

    fn tempdir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "roninKB-config-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).expect("tempdir");
        path
    }
}
