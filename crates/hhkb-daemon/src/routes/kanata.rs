//! `/kanata/*` — control and inspection of the Kanata software layer.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::ApiResult;
use crate::kanata::KanataStatus;
use crate::state::AppState;
use crate::ws::DaemonEvent;

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub installed: bool,
    pub config_path: String,
    #[serde(flatten)]
    pub status: KanataStatus,
}

#[derive(Debug, Serialize)]
pub struct StartResponse {
    pub pid: u32,
}

#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub status: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct ReloadBody {
    pub config: String,
}

#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub config: String,
    pub path: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    // Use check_alive so we reap a crashed child lazily on every poll.
    let kanata = state.kanata.clone();
    let (status, installed, path) = tokio::task::spawn_blocking(move || {
        let s = kanata.check_alive();
        (s, kanata.is_installed(), kanata.config_path().to_path_buf())
    })
    .await
    .unwrap_or((KanataStatus::Stopped, false, Default::default()));

    Json(StatusResponse {
        installed,
        config_path: path.display().to_string(),
        status,
    })
}

pub async fn start(State(state): State<AppState>) -> ApiResult<Json<StartResponse>> {
    let kanata = state.kanata.clone();
    let pid = tokio::task::spawn_blocking(move || kanata.start()).await??;
    let _ = state.events.send(DaemonEvent::KanataStarted { pid });
    Ok(Json(StartResponse { pid }))
}

pub async fn stop(State(state): State<AppState>) -> ApiResult<Json<OkResponse>> {
    let kanata = state.kanata.clone();
    tokio::task::spawn_blocking(move || kanata.stop()).await??;
    let _ = state.events.send(DaemonEvent::KanataStopped);
    Ok(Json(OkResponse { status: "stopped" }))
}

pub async fn reload(
    State(state): State<AppState>,
    Json(body): Json<ReloadBody>,
) -> ApiResult<Json<OkResponse>> {
    let kanata = state.kanata.clone();
    let cfg = body.config;
    tokio::task::spawn_blocking(move || kanata.reload(&cfg)).await??;
    // This endpoint doesn't know which profile triggered the reload, so
    // broadcast with an empty profile_id; the profile route emits a fuller
    // event when reload happens via profile switch.
    let _ = state.events.send(DaemonEvent::KanataReloaded {
        profile_id: String::new(),
    });
    Ok(Json(OkResponse { status: "reloaded" }))
}

pub async fn get_config(State(state): State<AppState>) -> ApiResult<Json<ConfigResponse>> {
    let kanata = state.kanata.clone();
    let (cfg, path) = tokio::task::spawn_blocking(move || {
        let path = kanata.config_path().display().to_string();
        kanata.read_config().map(|c| (c, path))
    })
    .await??;
    Ok(Json(ConfigResponse { config: cfg, path }))
}
