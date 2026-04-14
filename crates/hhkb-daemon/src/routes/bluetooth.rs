//! Bluetooth endpoints — backed by `btleplug` via [`crate::ble::BleManager`].
//!
//! All endpoints return HTTP 200 even when BLE is unavailable; the `available`
//! field in the response tells the client whether the adapter is present.
//!
//! ## Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET  | /device/bluetooth          | Status of the currently OS-connected HHKB BLE device |
//! | POST | /device/bluetooth/scan     | Start a background 4-second scan |
//! | GET  | /device/bluetooth/devices  | Nearby HHKB-ish devices + scan flag |

use axum::{extract::State, Json};
use serde::Serialize;

use crate::ble::BleDeviceInfo;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct BluetoothStatusDto {
    pub available: bool,
    pub connected: bool,
    pub name: Option<String>,
    pub address: Option<String>,
    pub battery: Option<u8>,
    pub rssi: Option<i16>,
}

#[derive(Serialize)]
pub struct BluetoothDevicesDto {
    pub available: bool,
    pub scanning: bool,
    pub devices: Vec<BleDeviceInfo>,
}

#[derive(Serialize)]
pub struct BluetoothScanStartedDto {
    pub scanning: bool,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /device/bluetooth`
///
/// Returns the currently OS-connected HHKB BLE device, or `connected: false`
/// if none (or if BLE is unavailable on this machine).
pub async fn get_bluetooth(State(state): State<AppState>) -> Json<BluetoothStatusDto> {
    if !state.ble.is_available() {
        return Json(BluetoothStatusDto {
            available: false,
            connected: false,
            name: None,
            address: None,
            battery: None,
            rssi: None,
        });
    }

    match state.ble.status().await {
        Some(d) => Json(BluetoothStatusDto {
            available: true,
            connected: true,
            name: d.name,
            address: Some(d.address),
            battery: d.battery,
            rssi: d.rssi,
        }),
        None => Json(BluetoothStatusDto {
            available: true,
            connected: false,
            name: None,
            address: None,
            battery: None,
            rssi: None,
        }),
    }
}

/// `POST /device/bluetooth/scan`
///
/// Starts a background BLE scan.  Returns immediately with `scanning: true`.
/// When the scan finishes, the daemon pushes a `bluetooth_scan_complete` WS
/// event.  Poll `GET /device/bluetooth/devices` for interim results.
pub async fn post_scan(State(state): State<AppState>) -> Json<BluetoothScanStartedDto> {
    if !state.ble.is_available() {
        return Json(BluetoothScanStartedDto {
            scanning: false,
            message: "BLE adapter unavailable on this machine".to_string(),
        });
    }

    let started = state.ble.start_scan(state.events.clone());
    Json(BluetoothScanStartedDto {
        scanning: started,
        message: if started {
            "scan started — takes ~4 seconds".to_string()
        } else {
            "scan already in progress".to_string()
        },
    })
}

/// `GET /device/bluetooth/devices`
///
/// Returns the device list from the most recent scan plus the current
/// `scanning` flag.
pub async fn get_devices(State(state): State<AppState>) -> Json<BluetoothDevicesDto> {
    if !state.ble.is_available() {
        return Json(BluetoothDevicesDto {
            available: false,
            scanning: false,
            devices: vec![],
        });
    }

    let (scanning, devices) = state.ble.devices().await;
    Json(BluetoothDevicesDto {
        available: true,
        scanning,
        devices,
    })
}
