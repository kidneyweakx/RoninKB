//! `GET /health` — daemon liveness check.

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub device_connected: bool,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let device_connected = state.is_device_connected().await;
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        device_connected,
    })
}
