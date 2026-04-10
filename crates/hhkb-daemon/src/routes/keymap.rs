//! `GET/PUT /device/keymap` — read/write a 128-byte keymap.

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use hhkb_core::{Keymap, KeyboardMode};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct KeymapQuery {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub fn_layer: bool,
}

fn default_mode() -> String {
    "mac".to_string()
}

fn parse_mode(s: &str) -> ApiResult<KeyboardMode> {
    match s.to_ascii_lowercase().as_str() {
        "hhk" => Ok(KeyboardMode::HHK),
        "mac" => Ok(KeyboardMode::Mac),
        "lite" => Ok(KeyboardMode::Lite),
        "secret" => Ok(KeyboardMode::Secret),
        other => Err(ApiError::BadRequest(format!("unknown mode: {other}"))),
    }
}

#[derive(Debug, Serialize)]
pub struct KeymapDto {
    pub mode: String,
    pub fn_layer: bool,
    /// 128 raw bytes.
    pub data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct WriteKeymapBody {
    pub data: Vec<u8>,
}

pub async fn get_keymap(
    State(state): State<AppState>,
    Query(q): Query<KeymapQuery>,
) -> ApiResult<Json<KeymapDto>> {
    let mode = parse_mode(&q.mode)?;
    let fn_layer = q.fn_layer;
    let mode_label = q.mode.clone();

    let keymap = state
        .with_device(move |dev| {
            dev.open_session()?;
            let km = dev.read_keymap(mode, fn_layer)?;
            dev.close_session()?;
            Ok(km)
        })
        .await?;

    Ok(Json(KeymapDto {
        mode: mode_label,
        fn_layer,
        data: keymap.as_bytes().to_vec(),
    }))
}

pub async fn put_keymap(
    State(state): State<AppState>,
    Query(q): Query<KeymapQuery>,
    Json(body): Json<WriteKeymapBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let mode = parse_mode(&q.mode)?;
    let fn_layer = q.fn_layer;

    if body.data.len() != 128 {
        return Err(ApiError::BadRequest(format!(
            "keymap must be exactly 128 bytes, got {}",
            body.data.len()
        )));
    }

    let mut raw = [0u8; 128];
    raw.copy_from_slice(&body.data);
    let keymap = Keymap::from_bytes(raw);

    state
        .with_device(move |dev| {
            dev.open_session()?;
            dev.write_keymap(mode, fn_layer, &keymap)?;
            dev.close_session()?;
            Ok(())
        })
        .await?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}
