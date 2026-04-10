//! `GET /device/*` endpoints — info, mode, DIP switches, connection status.
//!
//! Every endpoint that actually touches the keyboard wraps a session
//! (`open_session` / `close_session`) around the operation so the firmware
//! sees a well-formed transaction.

use axum::{extract::State, Json};
use serde::Serialize;

use hhkb_core::{DipSwitchState, KeyboardInfo, KeyboardMode};

use crate::error::ApiResult;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Serializable wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DeviceInfoDto {
    pub type_number: String,
    pub revision: [u8; 4],
    pub serial: [u8; 16],
    pub app_firmware: [u8; 8],
    pub boot_firmware: [u8; 8],
    pub running_firmware: &'static str,
}

impl From<KeyboardInfo> for DeviceInfoDto {
    fn from(info: KeyboardInfo) -> Self {
        Self {
            type_number: info.type_number,
            revision: info.revision,
            serial: info.serial,
            app_firmware: info.app_firmware,
            boot_firmware: info.boot_firmware,
            running_firmware: match info.running_firmware {
                hhkb_core::FirmwareType::Application => "application",
                hhkb_core::FirmwareType::Bootloader => "bootloader",
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ModeDto {
    pub mode: &'static str,
    pub raw: u8,
}

impl From<KeyboardMode> for ModeDto {
    fn from(mode: KeyboardMode) -> Self {
        let name = match mode {
            KeyboardMode::HHK => "hhk",
            KeyboardMode::Mac => "mac",
            KeyboardMode::Lite => "lite",
            KeyboardMode::Secret => "secret",
        };
        Self {
            mode: name,
            raw: mode as u8,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DipSwitchDto {
    pub switches: [bool; 6],
}

impl From<DipSwitchState> for DipSwitchDto {
    fn from(state: DipSwitchState) -> Self {
        Self {
            switches: state.switches,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConnectedDto {
    pub connected: bool,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn get_info(State(state): State<AppState>) -> ApiResult<Json<DeviceInfoDto>> {
    let info = state
        .with_device(|dev| {
            dev.open_session()?;
            let info = dev.get_info()?;
            dev.close_session()?;
            Ok(info)
        })
        .await?;
    Ok(Json(info.into()))
}

pub async fn get_mode(State(state): State<AppState>) -> ApiResult<Json<ModeDto>> {
    let mode = state
        .with_device(|dev| {
            dev.open_session()?;
            let mode = dev.get_mode()?;
            dev.close_session()?;
            Ok(mode)
        })
        .await?;
    Ok(Json(mode.into()))
}

pub async fn get_dipsw(State(state): State<AppState>) -> ApiResult<Json<DipSwitchDto>> {
    let state_out = state
        .with_device(|dev| {
            dev.open_session()?;
            let s = dev.get_dip_switch()?;
            dev.close_session()?;
            Ok(s)
        })
        .await?;
    Ok(Json(state_out.into()))
}

pub async fn get_connected(State(state): State<AppState>) -> Json<ConnectedDto> {
    // Opportunistically attempt a reconnect so this endpoint is also a
    // lightweight "nudge the daemon to look for the keyboard" call.
    state.try_reconnect().await;
    Json(ConnectedDto {
        connected: state.is_device_connected().await,
    })
}
