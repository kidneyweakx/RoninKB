//! Shared application state (device handle + SQLite connection + event bus).
//!
//! Both are wrapped in `Arc<Mutex<_>>` because hidapi is synchronous and
//! rusqlite's `Connection` is `!Sync`. Device operations are dispatched via
//! `tokio::task::spawn_blocking` so the HID round-trips don't block the
//! tokio runtime.

use std::sync::Arc;

use directories::ProjectDirs;
use rusqlite::Connection;
use tokio::sync::{broadcast, Mutex};

use hhkb_core::device::HhkbDevice;
use hhkb_core::transport::HidApiTransport;

use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::flow::FlowManager;
use crate::kanata::KanataManager;
use crate::ws::DaemonEvent;

pub type DeviceHandle = Arc<Mutex<Option<HhkbDevice<HidApiTransport>>>>;
pub type DbHandle = Arc<Mutex<Connection>>;

#[derive(Clone)]
pub struct AppState {
    pub device: DeviceHandle,
    pub db: DbHandle,
    pub events: broadcast::Sender<DaemonEvent>,
    /// Shared handle to the kanata software-layer supervisor. Always present;
    /// if the kanata binary isn't installed the manager simply reports
    /// `NotInstalled` and refuses start/stop/reload.
    pub kanata: Arc<KanataManager>,
    /// Flow (cross-device clipboard sync) manager. Not enabled by default;
    /// the caller must POST `/flow/enable` to start it.
    pub flow: Arc<FlowManager>,
    /// Disables auto-reconnect attempts. Tests set this so they don't
    /// accidentally open a real keyboard attached to the developer's machine.
    pub auto_reconnect: bool,
}

impl AppState {
    /// Production constructor: opens the default SQLite file under the user's
    /// data directory and attempts to connect to an HHKB device.
    pub async fn new() -> anyhow::Result<Self> {
        let db_path = default_db_path()?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        tracing::info!(path = %db_path.display(), "opening SQLite profile store");

        let conn = tokio::task::spawn_blocking(move || -> anyhow::Result<Connection> {
            let conn = Connection::open(db_path)?;
            db::init_schema(&conn).map_err(|e| anyhow::anyhow!("db init: {e}"))?;
            Ok(conn)
        })
        .await??;

        // Best-effort device open; absence is fine — endpoints will retry.
        let device = tokio::task::spawn_blocking(HidApiTransport::open)
            .await
            .ok()
            .and_then(|r| r.ok())
            .map(HhkbDevice::new);

        if device.is_some() {
            tracing::info!("HHKB device connected on startup");
        } else {
            tracing::warn!("no HHKB device found on startup; endpoints will lazily retry");
        }

        let (events, _) = broadcast::channel(64);

        let kanata = KanataManager::new().map(Arc::new).unwrap_or_else(|e| {
            tracing::warn!(%e, "kanata manager init failed; using disabled stub");
            Arc::new(KanataManager::with_paths(
                None,
                std::env::temp_dir().join("roninKB-kanata-disabled.kbd"),
            ))
        });

        Ok(Self {
            device: Arc::new(Mutex::new(device)),
            db: Arc::new(Mutex::new(conn)),
            events,
            kanata,
            flow: Arc::new(FlowManager::new()),
            auto_reconnect: true,
        })
    }

    /// Test constructor: in-memory SQLite, no device, auto-reconnect disabled
    /// so tests on a developer machine with a real HHKB attached still see
    /// a "no device" state.
    pub fn for_tests() -> Self {
        let conn = db::open_in_memory().expect("in-memory sqlite");
        let (events, _) = broadcast::channel(16);
        // Deterministic "no binary, temp config path" manager so tests never
        // touch a real kanata install or the user's data directory.
        let kanata_cfg = std::env::temp_dir().join(format!(
            "roninKB-kanata-test-{}-{}.kbd",
            std::process::id(),
            uuid::Uuid::new_v4(),
        ));
        let kanata = Arc::new(KanataManager::with_paths(None, kanata_cfg));
        Self {
            device: Arc::new(Mutex::new(None)),
            db: Arc::new(Mutex::new(conn)),
            events,
            kanata,
            flow: Arc::new(FlowManager::new()),
            auto_reconnect: false,
        }
    }

    /// Report whether a device is currently held in state (no reconnect
    /// attempt). Useful for `/health` and `/device/connected`.
    pub async fn is_device_connected(&self) -> bool {
        self.device.lock().await.is_some()
    }

    /// Try to reconnect if we currently have no device. Called lazily from
    /// device endpoints. Never fails — just leaves the handle `None` on error.
    pub async fn try_reconnect(&self) {
        if !self.auto_reconnect {
            return;
        }
        let mut guard = self.device.lock().await;
        if guard.is_some() {
            return;
        }
        let result = tokio::task::spawn_blocking(HidApiTransport::open).await;
        match result {
            Ok(Ok(t)) => {
                tracing::info!("HHKB device reconnected");
                *guard = Some(HhkbDevice::new(t));
                let _ = self.events.send(DaemonEvent::DeviceConnected);
            }
            Ok(Err(e)) => {
                tracing::debug!("device reconnect failed: {e}");
            }
            Err(e) => {
                tracing::debug!("device reconnect task join failed: {e}");
            }
        }
    }

    /// Run a blocking device operation on the device, re-connecting first
    /// if needed. Returns `DeviceUnavailable` if still unreachable.
    ///
    /// The closure runs on a `spawn_blocking` thread and takes exclusive
    /// ownership of the device for the duration of the call. Because
    /// `HhkbDevice` is neither `Send` nor safely sharable across threads in
    /// this codebase, we briefly take the device out of the state, run the
    /// op, and put it back.
    pub async fn with_device<F, T>(&self, f: F) -> ApiResult<T>
    where
        F: FnOnce(&HhkbDevice<HidApiTransport>) -> hhkb_core::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        self.try_reconnect().await;

        let mut guard = self.device.lock().await;
        let device = guard.take().ok_or(ApiError::DeviceUnavailable)?;

        // Move the device onto the blocking thread, run the op, return it.
        let join = tokio::task::spawn_blocking(move || {
            let result = f(&device);
            (device, result)
        })
        .await;

        match join {
            Ok((device, result)) => {
                *guard = Some(device);
                Ok(result?)
            }
            Err(e) => {
                // The blocking task panicked — leave device slot empty so we
                // reopen on next call.
                Err(ApiError::Internal(format!("device task join: {e}")))
            }
        }
    }
}

fn default_db_path() -> anyhow::Result<std::path::PathBuf> {
    if let Some(dirs) = ProjectDirs::from("", "", "roninKB") {
        Ok(dirs.data_dir().join("profiles.db"))
    } else {
        // Fall back to the spec'd path under $HOME.
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        Ok(std::path::PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("roninKB")
            .join("profiles.db"))
    }
}
