//! `/profiles` CRUD and active-profile endpoints.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use hhkb_core::ViaProfile;

use crate::db;
use crate::db::ProfileRecord;
use crate::error::{ApiError, ApiResult};
use crate::kanata::KanataStatus;
use crate::kanata_config;
use crate::state::AppState;
use crate::ws::DaemonEvent;

#[derive(Debug, Serialize)]
pub struct ProfileListResponse {
    pub profiles: Vec<ProfileRecord>,
}

#[derive(Debug, Serialize)]
pub struct ActiveResponse {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetActiveBody {
    pub id: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<ProfileListResponse>> {
    let conn = state.db.lock().await;
    let profiles = db::list_profiles(&conn)?;
    Ok(Json(ProfileListResponse { profiles }))
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ProfileRecord>> {
    let conn = state.db.lock().await;
    Ok(Json(db::get_profile(&conn, &id)?))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<ViaProfile>,
) -> ApiResult<Json<ProfileRecord>> {
    validate_profile_kanata_config(&body)?;
    let conn = state.db.lock().await;
    let rec = db::create_profile(&conn, body)?;
    Ok(Json(rec))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ViaProfile>,
) -> ApiResult<Json<ProfileRecord>> {
    validate_profile_kanata_config(&body)?;
    let conn = state.db.lock().await;
    let rec = db::update_profile(&conn, &id, body)?;
    drop(conn);
    let _ = state
        .events
        .send(DaemonEvent::ProfileChanged { id: rec.id.clone() });
    Ok(Json(rec))
}

pub async fn delete_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let conn = state.db.lock().await;
    db::delete_profile(&conn, &id)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn get_active(State(state): State<AppState>) -> ApiResult<Json<ActiveResponse>> {
    let conn = state.db.lock().await;
    let id = db::get_active(&conn)?;
    Ok(Json(ActiveResponse { id }))
}

pub async fn set_active(
    State(state): State<AppState>,
    Json(body): Json<SetActiveBody>,
) -> ApiResult<Json<ActiveResponse>> {
    // Resolve + persist the active profile, then capture any kanata config
    // before dropping the db lock.
    let kanata_config = {
        let conn = state.db.lock().await;
        db::set_active(&conn, &body.id)?;
        let rec = db::get_profile(&conn, &body.id)?;
        kanata_config::derive_profile_kanata_config(&rec.via).map_err(ApiError::InvalidConfig)?
    };

    let _ = state.events.send(DaemonEvent::ProfileChanged {
        id: body.id.clone(),
    });

    // If the profile carries a kanata config, persist it on disk. Only
    // hot-reload if kanata is actually running — otherwise the file is
    // updated and the next start() picks it up.
    if let Some(cfg) = kanata_config {
        let kanata = state.kanata.clone();
        let was_running = matches!(kanata.check_alive(), KanataStatus::Running { .. });
        let reload_result = tokio::task::spawn_blocking(move || {
            if was_running {
                kanata.reload(&cfg)
            } else {
                kanata.write_config(&cfg)
            }
        })
        .await?;

        match reload_result {
            Ok(()) if was_running => {
                let _ = state.events.send(DaemonEvent::KanataReloaded {
                    profile_id: body.id.clone(),
                });
            }
            Ok(()) => { /* config written, nothing running to reload */ }
            Err(e) => {
                tracing::warn!(%e, profile = %body.id, "kanata reload on profile switch failed");
            }
        }
    }

    Ok(Json(ActiveResponse { id: Some(body.id) }))
}

fn validate_profile_kanata_config(via: &ViaProfile) -> ApiResult<()> {
    if let Some(cfg) =
        kanata_config::derive_profile_kanata_config(via).map_err(ApiError::InvalidConfig)?
    {
        kanata_config::validate_kanata_config(&cfg).map_err(ApiError::InvalidConfig)?;
    }
    Ok(())
}
