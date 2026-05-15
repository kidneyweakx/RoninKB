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
use crate::backend::{BackendError, BackendId};
use crate::db;
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

/// `POST /backend/select` — switch the active backend.
///
/// Switching follows the order:
///   1. Tear down the previously active backend (if running) so its OS hooks
///      release the keyboard before we hand control to the new one. Without
///      this the old CGEventTap / kanata process would keep intercepting and
///      we'd get ghost events.
///   2. Swap the registry's active id + persist the pin to `config.toml`.
///   3. Apply the currently-active profile (from `db::get_active`) to the
///      new backend. This is the single point that turns "user selected
///      macos-native" into "the engine is actually driving keys" — without
///      step 3 the daemon would sit idle until the next profile switch.
///
/// Returns 404 if the requested id isn't registered. A teardown failure on
/// the *old* backend is logged but doesn't block the switch — we'd rather
/// have a noisy log than refuse to migrate. An apply failure on the new
/// backend surfaces to the client (BackendNotReady → 503, etc.) so the UI
/// can show the missing-permission deep link.
pub async fn select(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<SelectBody>,
) -> ApiResult<Json<SelectResponse>> {
    let registry = state.backends.clone();
    let config = state.daemon_config.clone();
    let id = body.id;

    // 1) Teardown previous backend (best-effort; never blocks the switch).
    let old_active = registry.active_backend();
    if let Some(old) = old_active {
        if old.id() != id {
            let old_for_task = old.clone();
            let old_id = old.id();
            let join = tokio::task::spawn_blocking(move || old_for_task.teardown()).await;
            match join {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::warn!(backend = %old_id, %e, "teardown on backend switch failed");
                }
                Err(e) => {
                    tracing::warn!(backend = %old_id, %e, "teardown task join failed");
                }
            }
        }
    }

    // 2) Swap registry + persist pin. UnknownBackend is the only failure
    //    mode and we surface it as 404.
    let registry_swap = registry.clone();
    let config_swap = config.clone();
    tokio::task::spawn_blocking(move || {
        registry_swap.select(id)?;
        config_swap.set_pinned_backend(id);
        Ok::<_, RegistryError>(())
    })
    .await
    .map_err(|e| ApiError::Internal(format!("backend select task: {e}")))?
    .map_err(|e| match e {
        RegistryError::UnknownBackend(_) => ApiError::NotFound,
    })?;

    // 3) Auto-apply current active profile to the new backend.
    //
    //    Kanata is the only backend we deliberately skip here: its
    //    process lifecycle is still user-driven (start/stop buttons) in
    //    v0.2.0 so /backend/select kanata shouldn't auto-launch the
    //    process. macos-native / hidutil / eeprom on the other hand only
    //    "run" when we tell them to.
    if id != BackendId::Kanata {
        let active_profile_via = {
            let conn = state.db.lock().await;
            match db::get_active(&conn)? {
                Some(profile_id) => Some(db::get_profile(&conn, &profile_id)?.via),
                None => None,
            }
        };
        if let Some(via) = active_profile_via {
            if let Some(backend) = registry.active_backend() {
                let backend_id = backend.id();
                // Pre-flight: skip if the profile clearly isn't aimed at
                // this backend. Avoids surfacing a guaranteed
                // ProfileRejected to the user when the right outcome is
                // "no-op, you just switched backends to one this profile
                // doesn't target".
                if crate::routes::profile::profile_targets_backend(&via, backend_id) {
                    let backend_for_task = backend.clone();
                    let join =
                        tokio::task::spawn_blocking(move || backend_for_task.apply(&via)).await;
                    match join {
                        Ok(Ok(())) => {
                            tracing::info!(
                                backend = %backend_id,
                                "applied active profile on switch"
                            );
                        }
                        Ok(Err(e)) => {
                            return Err(backend_error_to_api(e, backend_id));
                        }
                        Err(e) => {
                            return Err(ApiError::Internal(format!("backend apply task: {e}")));
                        }
                    }
                } else {
                    tracing::debug!(
                        backend = %backend_id,
                        "profile doesn't target this backend; skipping auto-apply on switch"
                    );
                }
            }
        }
    }

    Ok(Json(SelectResponse { active: id }))
}

fn backend_error_to_api(err: BackendError, backend: BackendId) -> ApiError {
    match err {
        BackendError::NotReady(missing) => ApiError::BackendNotReady {
            backend: backend.as_str().to_string(),
            missing: serde_json::to_value(missing).unwrap_or(serde_json::Value::Null),
        },
        BackendError::ProfileRejected(msg) => ApiError::InvalidConfig(format!("{backend}: {msg}")),
        BackendError::Internal(msg) => ApiError::Internal(format!("{backend} backend: {msg}")),
    }
}
