//! `/backend/*` — v0.2.0 backend selection + capability surface.
//!
//! These routes expose the registry built in `state::build_backend_registry`.
//! They coexist with `/kanata/*`, which v0.1.x clients keep using; the kanata
//! routes return identical responses regardless of which backend is currently
//! active. The contract is "/backend/* is authoritative for v0.2+; /kanata/*
//! is a compat surface for old clients" — see RFC 0001 §10.

use axum::{
    extract::{Json as JsonBody, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::backend::registry::{BackendInfo, RegistryError};
use crate::backend::BackendId;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub backends: Vec<BackendInfo>,
    /// `Some(id)` of the currently active backend, or `None` if no backend
    /// passed the auto-selection permission check at startup. Frontend uses
    /// this to render the radio selector with the right preselected option.
    pub active: Option<BackendId>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub active: Option<BackendInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SelectBody {
    pub id: BackendId,
}

#[derive(Debug, Serialize)]
pub struct SelectResponse {
    pub active: BackendId,
}

/// `GET /backend/list` — every registered backend with its current
/// capability surface, permission state, and is_active flag.
pub async fn list(State(state): State<AppState>) -> Json<ListResponse> {
    let registry = state.backends.clone();
    let (backends, active) =
        tokio::task::spawn_blocking(move || (registry.list(), registry.active()))
            .await
            .unwrap_or_default();
    Json(ListResponse { backends, active })
}

/// `GET /backend/status` — diagnostics for the active backend only. Cheaper
/// to poll than `list` when the UI just needs "is the active one healthy?".
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let registry = state.backends.clone();
    let info = tokio::task::spawn_blocking(move || {
        let active_id = registry.active()?;
        registry.list().into_iter().find(|b| b.id == active_id)
    })
    .await
    .ok()
    .flatten();
    Json(StatusResponse { active: info })
}

/// `POST /backend/select` — switch the active backend. Returns 404 if the
/// requested id isn't registered. Does **not** call `apply()` on the new
/// backend; the next profile load drives that.
///
/// Persists `[backend].pin` in `config.toml` so the choice survives daemon
/// restart (RFC 0001 §4.4). A persistence failure is logged but doesn't
/// block the response — the in-memory state is the source of truth and the
/// user can always re-select.
pub async fn select(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<SelectBody>,
) -> ApiResult<Json<SelectResponse>> {
    let registry = state.backends.clone();
    let config = state.daemon_config.clone();
    let id = body.id;
    tokio::task::spawn_blocking(move || {
        registry.select(id)?;
        config.set_pinned_backend(id);
        Ok::<_, RegistryError>(())
    })
    .await
    .map_err(|e| ApiError::Internal(format!("backend select task: {e}")))?
    .map_err(|e| match e {
        RegistryError::UnknownBackend(_) => ApiError::NotFound,
    })?;
    Ok(Json(SelectResponse { active: id }))
}
