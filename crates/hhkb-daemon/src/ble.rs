//! Cross-platform BLE manager for HHKB Hybrid keyboards using `btleplug`.
//!
//! The manager is initialised once at daemon startup.  If no Bluetooth adapter
//! is present (or the user has not granted permission) all public methods
//! return graceful "unavailable" responses — no panics.
//!
//! ## Scan flow
//! 1. Caller calls `BleManager::start_scan()` — returns immediately.
//! 2. A background tokio task scans for `SCAN_DURATION_SECS` seconds.
//! 3. Task stores found devices in `BleState::devices`.
//! 4. Task sets `scanning = false` and sends a `DaemonEvent::BluetoothScanComplete`.
//!
//! ## Battery reading
//! For devices the OS already has connected, the manager tries to read the
//! standard BLE Battery Service (UUID 0x180F) / Battery Level characteristic
//! (0x2A19) when accessible.

use std::sync::Arc;
use std::time::Duration;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::ws::DaemonEvent;

pub const HHKB_NAME_PREFIX: &str = "HHKB-Hybrid";
/// User-facing scan: long enough to catch advertising-only devices.
const SCAN_DURATION_SECS: u64 = 4;
/// Quick startup probe: just long enough to populate Core Bluetooth's cache.
const STARTUP_SCAN_SECS: u64 = 2;

fn battery_char_uuid() -> Uuid {
    Uuid::parse_str("00002a19-0000-1000-8000-00805f9b34fb").unwrap()
}

// ---------------------------------------------------------------------------
// Public DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleDeviceInfo {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
    pub battery: Option<u8>,
    pub connected: bool,
    pub rssi: Option<i16>,
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct BleState {
    /// Devices discovered during the most recent scan.
    devices: Vec<BleDeviceInfo>,
    /// True while a background scan task is running.
    scanning: bool,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct BleManager {
    adapter: Option<Adapter>,
    state: Arc<Mutex<BleState>>,
}

impl BleManager {
    /// Initialise; silently degrades if no adapter or no permission.
    pub async fn new() -> Self {
        let adapter = Self::try_get_adapter().await;
        if adapter.is_none() {
            tracing::warn!("ble: no adapter or permission — BLE unavailable");
        } else {
            tracing::info!("ble: adapter ready");
        }
        Self {
            adapter,
            state: Arc::new(Mutex::new(BleState::default())),
        }
    }

    /// Return a permanently-unavailable manager (used in tests).
    pub fn unavailable() -> Self {
        Self {
            adapter: None,
            state: Arc::new(Mutex::new(BleState::default())),
        }
    }

    async fn try_get_adapter() -> Option<Adapter> {
        let manager = Manager::new().await.ok()?;
        manager.adapters().await.ok()?.into_iter().next()
    }

    pub fn is_available(&self) -> bool {
        self.adapter.is_some()
    }

    /// Kick off a short startup probe in the background to populate the adapter's
    /// peripheral cache before the first user-triggered scan.  Fire-and-forget.
    pub fn probe_connected(&self, events: broadcast::Sender<DaemonEvent>) {
        let Some(adapter) = self.adapter.clone() else {
            return;
        };
        let state = self.state.clone();
        tokio::spawn(async move {
            let found = do_scan(&adapter, STARTUP_SCAN_SECS).await;
            let mut guard = state.lock().await;
            // Only update state if we found something; don't clobber a
            // user-triggered scan that may be in progress.
            if !found.is_empty() {
                guard.devices = found.clone();
                let _ = events.send(DaemonEvent::BluetoothScanComplete { devices: found });
            }
        });
    }

    /// Kick off a background scan.  Returns `false` if BLE is unavailable or
    /// a scan is already in progress.
    pub fn start_scan(&self, events: broadcast::Sender<DaemonEvent>) -> bool {
        let Some(adapter) = self.adapter.clone() else {
            return false;
        };
        let state = self.state.clone();

        {
            // Check / set scanning flag synchronously before spawning.
            let Ok(mut guard) = state.try_lock() else {
                return false;
            };
            if guard.scanning {
                return false;
            }
            guard.scanning = true;
            guard.devices.clear();
        }

        tokio::spawn(async move {
            let found = do_scan(&adapter, SCAN_DURATION_SECS).await;
            let mut guard = state.lock().await;
            guard.devices = found.clone();
            guard.scanning = false;
            let _ = events.send(DaemonEvent::BluetoothScanComplete { devices: found });
        });

        true
    }

    /// Current scan state + device list (snapshot).
    pub async fn devices(&self) -> (bool, Vec<BleDeviceInfo>) {
        let guard = self.state.lock().await;
        (guard.scanning, guard.devices.clone())
    }

    /// Status of the currently connected HHKB BLE device, if any.
    pub async fn status(&self) -> Option<BleDeviceInfo> {
        let known_devices = {
            let guard = self.state.lock().await;
            guard.devices.clone()
        };

        // Check whether the OS already has an HHKB-like device connected. We
        // intentionally do not call `connect()` here; the system Bluetooth
        // stack owns pairing and transport selection.
        let Some(adapter) = &self.adapter else {
            return None;
        };

        let peripherals = adapter.peripherals().await.ok()?;
        for p in peripherals {
            let connected = p.is_connected().await.unwrap_or(false);
            if !connected {
                continue;
            }
            let props = p.properties().await.ok().flatten();
            let name = props.as_ref().and_then(|pr| pr.local_name.clone());
            let address = props.as_ref().map(|pr| pr.address.to_string()).unwrap_or_default();
            if !matches_hhkb_candidate(&p, &address, name.as_deref(), &known_devices) {
                continue;
            }
            let battery = read_battery(&p).await;
            return Some(BleDeviceInfo {
                id: p.id().to_string(),
                name,
                address,
                battery,
                connected: true,
                rssi: props.as_ref().and_then(|pr| pr.rssi),
            });
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Scan helper
// ---------------------------------------------------------------------------

async fn do_scan(adapter: &Adapter, duration_secs: u64) -> Vec<BleDeviceInfo> {
    if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
        tracing::warn!("ble: start_scan failed: {e}");
        return vec![];
    }

    tokio::time::sleep(Duration::from_secs(duration_secs)).await;
    let _ = adapter.stop_scan().await;

    let peripherals = match adapter.peripherals().await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("ble: peripherals() failed: {e}");
            return vec![];
        }
    };

    tracing::info!("ble: {} peripheral(s) visible after scan", peripherals.len());
    let mut devices = Vec::new();
    for p in peripherals {
        let props = match p.properties().await {
            Ok(Some(props)) => props,
            _ => continue,
        };
        let is_connected = p.is_connected().await.unwrap_or(false);
        let name = props.local_name.clone();

        tracing::debug!(
            "ble: peripheral {:?} addr={} rssi={:?} connected={}",
            name,
            props.address,
            props.rssi,
            is_connected,
        );

        // Keep HHKB-named devices plus already-connected nameless devices.
        // This keeps the UI focused on plausible keyboard entries while still
        // surfacing privacy-redacted connected peripherals on macOS.
        if !is_hhkb_name(name.as_deref()) && !is_connected {
            continue;
        }
        devices.push(BleDeviceInfo {
            id: p.id().to_string(),
            name,
            address: props.address.to_string(),
            battery: None,
            connected: is_connected,
            rssi: props.rssi,
        });
    }

    // Sort: connected first, then by RSSI (stronger signal first).
    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| b.rssi.unwrap_or(i16::MIN).cmp(&a.rssi.unwrap_or(i16::MIN)))
    });

    tracing::info!("ble: scan complete, {} peripheral(s) total", devices.len());
    devices
}

// ---------------------------------------------------------------------------
// GATT battery helper
// ---------------------------------------------------------------------------

async fn read_battery(peripheral: &Peripheral) -> Option<u8> {
    let chars = peripheral.characteristics();
    let bat = chars
        .iter()
        .find(|c| c.uuid == battery_char_uuid())?;
    peripheral.read(bat).await.ok()?.first().copied()
}

fn is_hhkb_name(name: Option<&str>) -> bool {
    matches!(name, Some(name) if name.starts_with(HHKB_NAME_PREFIX))
}

fn matches_hhkb_candidate(
    peripheral: &Peripheral,
    address: &str,
    name: Option<&str>,
    known_devices: &[BleDeviceInfo],
) -> bool {
    if is_hhkb_name(name) {
        return true;
    }

    known_devices.iter().any(|device| {
        let same_id = device.id == peripheral.id().to_string();
        let same_address =
            !address.is_empty() && device.address.eq_ignore_ascii_case(address);
        (same_id || same_address) && is_hhkb_name(device.name.as_deref())
    })
}
