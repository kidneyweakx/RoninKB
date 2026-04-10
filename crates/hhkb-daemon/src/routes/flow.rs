//! `/flow/*` — cross-device clipboard sync (Flow) HTTP surface.
//!
//! All endpoints operate on [`crate::flow::FlowManager`] held inside
//! [`AppState`]. The manager keeps its own internal state; these handlers
//! are thin JSON adaptors that also broadcast relevant events over the
//! WebSocket bus.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::flow::{FlowConfig, FlowEntry, FlowPeer};
use crate::state::AppState;
use crate::ws::DaemonEvent;

// ---------------------------------------------------------------------------
// Request / response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct PeersResponse {
    pub peers: Vec<FlowPeer>,
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub entries: Vec<FlowEntry>,
}

#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub status: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct AddPeerBody {
    pub hostname: String,
    pub addr: String,
}

#[derive(Debug, Deserialize)]
pub struct SyncBody {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveBody {
    pub peer_id: Uuid,
    pub hostname: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub entry: FlowEntry,
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

pub async fn get_config(State(state): State<AppState>) -> Json<FlowConfig> {
    Json(state.flow.config().await)
}

pub async fn put_config(
    State(state): State<AppState>,
    Json(body): Json<FlowConfig>,
) -> Json<FlowConfig> {
    state.flow.set_config(body).await;
    Json(state.flow.config().await)
}

// ---------------------------------------------------------------------------
// Enable / disable
// ---------------------------------------------------------------------------

pub async fn enable(State(state): State<AppState>) -> ApiResult<Json<OkResponse>> {
    state.flow.enable().await?;
    let _ = state.events.send(DaemonEvent::FlowEnabled);
    Ok(Json(OkResponse { status: "enabled" }))
}

pub async fn disable(State(state): State<AppState>) -> ApiResult<Json<OkResponse>> {
    state.flow.disable().await?;
    let _ = state.events.send(DaemonEvent::FlowDisabled);
    Ok(Json(OkResponse { status: "disabled" }))
}

// ---------------------------------------------------------------------------
// Peers
// ---------------------------------------------------------------------------

pub async fn list_peers(State(state): State<AppState>) -> Json<PeersResponse> {
    Json(PeersResponse {
        peers: state.flow.peers().await,
    })
}

pub async fn add_peer(
    State(state): State<AppState>,
    Json(body): Json<AddPeerBody>,
) -> ApiResult<Json<FlowPeer>> {
    if body.hostname.is_empty() || body.addr.is_empty() {
        return Err(ApiError::BadRequest(
            "hostname and addr are required".into(),
        ));
    }
    let peer = FlowPeer {
        id: Uuid::new_v4(),
        hostname: body.hostname,
        addr: body.addr,
        last_seen: now_secs(),
        online: true,
    };
    state.flow.add_peer(peer.clone()).await;
    let _ = state
        .events
        .send(DaemonEvent::FlowPeerDiscovered { peer: peer.clone() });
    Ok(Json(peer))
}

pub async fn remove_peer(State(state): State<AppState>, Path(id): Path<Uuid>) -> Json<OkResponse> {
    state.flow.remove_peer(id).await;
    let _ = state.events.send(DaemonEvent::FlowPeerLost { peer_id: id });
    Json(OkResponse { status: "removed" })
}

// ---------------------------------------------------------------------------
// History / sync / receive
// ---------------------------------------------------------------------------

pub async fn get_history(State(state): State<AppState>) -> Json<HistoryResponse> {
    Json(HistoryResponse {
        entries: state.flow.history().await,
    })
}

pub async fn clear_history(State(state): State<AppState>) -> Json<OkResponse> {
    state.flow.clear_history().await;
    Json(OkResponse { status: "cleared" })
}

pub async fn sync(
    State(state): State<AppState>,
    Json(body): Json<SyncBody>,
) -> ApiResult<Json<SyncResponse>> {
    let entry = state.flow.sync_local(body.content).await?;
    let _ = state.events.send(DaemonEvent::FlowSynced {
        entry_id: entry.id,
        source: entry.source.clone(),
    });
    Ok(Json(SyncResponse { entry }))
}

pub async fn receive(
    State(state): State<AppState>,
    Json(body): Json<ReceiveBody>,
) -> ApiResult<Json<SyncResponse>> {
    let entry = state
        .flow
        .receive_from_peer(body.peer_id, body.hostname, body.content)
        .await?;
    let _ = state.events.send(DaemonEvent::FlowSynced {
        entry_id: entry.id,
        source: entry.source.clone(),
    });
    Ok(Json(SyncResponse { entry }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
