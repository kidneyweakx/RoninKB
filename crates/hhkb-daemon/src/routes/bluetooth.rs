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
//! | GET  | /device/bluetooth/system   | Host OS connected-device list (macOS) |

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
pub struct SystemBluetoothDeviceDto {
    pub name: String,
    pub address: Option<String>,
    pub kind: Option<String>,
    pub battery: Option<u8>,
    pub services: Option<String>,
}

#[derive(Serialize)]
pub struct SystemBluetoothDevicesDto {
    pub available: bool,
    pub source: String,
    pub devices: Vec<SystemBluetoothDeviceDto>,
    pub message: Option<String>,
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
    // Try btleplug first (works when our app made the connection).
    if state.ble.is_available() {
        if let Some(d) = state.ble.status().await {
            return Json(BluetoothStatusDto {
                available: true,
                connected: true,
                name: d.name,
                address: Some(d.address),
                battery: d.battery,
                rssi: d.rssi,
            });
        }
    }

    // Fallback: system_profiler on macOS surfaces OS-managed connections
    // (btleplug can't see peripherals paired outside our CBCentralManager).
    #[cfg(target_os = "macos")]
    if let Ok(sys) = read_macos_connected_devices().await {
        if let Some(hhkb) = sys.iter().find(|d| d.name.starts_with("HHKB")) {
            return Json(BluetoothStatusDto {
                available: true,
                connected: true,
                name: Some(hhkb.name.clone()),
                address: hhkb.address.clone(),
                battery: hhkb.battery,
                rssi: None,
            });
        }
    }

    Json(BluetoothStatusDto {
        available: state.ble.is_available(),
        connected: false,
        name: None,
        address: None,
        battery: None,
        rssi: None,
    })
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

/// `GET /device/bluetooth/system`
///
/// Returns the host OS Bluetooth connected-device list.
/// On macOS this is sourced from `system_profiler SPBluetoothDataType -json`.
/// On other platforms this endpoint returns `source="unsupported"` with an
/// empty device list.
pub async fn get_system_devices(State(state): State<AppState>) -> Json<SystemBluetoothDevicesDto> {
    if !state.ble.is_available() {
        return Json(SystemBluetoothDevicesDto {
            available: false,
            source: "none".to_string(),
            devices: vec![],
            message: Some("BLE adapter unavailable on this machine".to_string()),
        });
    }

    #[cfg(target_os = "macos")]
    {
        match read_macos_connected_devices().await {
            Ok(devices) => Json(SystemBluetoothDevicesDto {
                available: true,
                source: "system_profiler".to_string(),
                devices,
                message: None,
            }),
            Err(e) => Json(SystemBluetoothDevicesDto {
                available: true,
                source: "system_profiler".to_string(),
                devices: vec![],
                message: Some(e),
            }),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Json(SystemBluetoothDevicesDto {
            available: true,
            source: "unsupported".to_string(),
            devices: vec![],
            message: Some("connected-device listing is currently macOS-only".to_string()),
        })
    }
}

#[cfg(target_os = "macos")]
async fn read_macos_connected_devices() -> Result<Vec<SystemBluetoothDeviceDto>, String> {
    let output = tokio::task::spawn_blocking(|| {
        std::process::Command::new("system_profiler")
            .args(["SPBluetoothDataType", "-json"])
            .output()
    })
    .await
    .map_err(|e| format!("system_profiler task failed: {e}"))?
    .map_err(|e| format!("system_profiler exec failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "system_profiler exited with status {}",
            output.status
        ));
    }

    parse_macos_connected_devices(&output.stdout)
}

#[cfg(target_os = "macos")]
fn parse_macos_connected_devices(stdout: &[u8]) -> Result<Vec<SystemBluetoothDeviceDto>, String> {
    let root: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|e| format!("invalid system_profiler JSON: {e}"))?;
    let entries = root
        .get("SPBluetoothDataType")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing SPBluetoothDataType array".to_string())?;

    let mut devices = Vec::new();
    for entry in entries {
        let Some(connected) = entry.get("device_connected").and_then(|v| v.as_array()) else {
            continue;
        };

        for item in connected {
            let Some(map) = item.as_object() else {
                continue;
            };

            for (name, details) in map {
                let address = details
                    .get("device_address")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let kind = details
                    .get("device_minorType")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let services = details
                    .get("device_services")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let battery = parse_battery_percent(
                    details
                        .get("device_batteryLevelMain")
                        .and_then(|v| v.as_str()),
                );

                devices.push(SystemBluetoothDeviceDto {
                    name: name.clone(),
                    address,
                    kind,
                    battery,
                    services,
                });
            }
        }
    }

    devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(devices)
}

#[cfg(target_os = "macos")]
fn parse_battery_percent(raw: Option<&str>) -> Option<u8> {
    let s = raw?.trim();
    let digits = s.strip_suffix('%').unwrap_or(s).trim();
    let n = digits.parse::<u8>().ok()?;
    (n <= 100).then_some(n)
}
