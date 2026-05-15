//! `/profiles` CRUD and active-profile endpoints.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use hhkb_core::ViaProfile;

use crate::backend::BackendId;
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
    // Resolve + persist the active profile, then capture the parsed VIA
    // payload and any kanata config before dropping the db lock.
    let (profile_via, kanata_config) = {
        let conn = state.db.lock().await;
        db::set_active(&conn, &body.id)?;
        let rec = db::get_profile(&conn, &body.id)?;
        let kcfg = kanata_config::derive_profile_kanata_config(&rec.via)
            .map_err(ApiError::InvalidConfig)?;
        (rec.via, kcfg)
    };

    let _ = state.events.send(DaemonEvent::ProfileChanged {
        id: body.id.clone(),
    });

    // 1) Kanata compat path: even when kanata isn't the active backend right
    //    now, we keep its config file in sync so a later /backend/select to
    //    kanata picks up the latest profile without an extra round-trip.
    if let Some(cfg) = kanata_config {
        let kanata = state.kanata.clone();
        let was_running = matches!(kanata.check_alive(), KanataStatus::Running { .. });
        let kanata_is_active = state.backends.active() == Some(BackendId::Kanata);
        let should_reload_kanata = was_running && kanata_is_active;
        let reload_result = tokio::task::spawn_blocking(move || {
            if should_reload_kanata {
                kanata.reload(&cfg)
            } else {
                kanata.write_config(&cfg)
            }
        })
        .await?;

        match reload_result {
            Ok(()) if should_reload_kanata => {
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

    // 2) Active backend dispatch: ask whichever backend the user picked to
    //    apply the new profile. For non-kanata backends this is the only
    //    path that drives the OS — without it, /backend/select macos-native
    //    + active profile load would silently leave the keyboard idle.
    //    Kanata is handled by the compat path above, so skip the registry
    //    call for it to preserve the "don't auto-start kanata on profile
    //    switch" semantics v0.1.x clients rely on.
    if let Some(backend) = state.backends.active_backend() {
        let backend_id = backend.id();
        // Skip the dispatch entirely when the profile clearly isn't aimed
        // at this backend — saves a guaranteed ProfileRejected round-trip
        // and surfaces as a debug log instead of a 400. This is the common
        // case when the user has a kanata profile loaded but EEPROM/hidutil
        // is the active backend.
        if backend_id != BackendId::Kanata && profile_targets_backend(&profile_via, backend_id) {
            let profile_for_backend = profile_via.clone();
            let backend_for_task = backend.clone();
            let profile_id = body.id.clone();
            let reload =
                tokio::task::spawn_blocking(move || backend_for_task.reload(&profile_for_backend))
                    .await?;
            match reload {
                Ok(()) => {
                    tracing::info!(
                        backend = %backend_id,
                        profile = %profile_id,
                        "active backend reloaded with new profile"
                    );
                }
                Err(e) => {
                    // EEPROM legitimately rejects software-only profiles
                    // (no `_roninKB.hardware` section). The user is loading
                    // a kanata/macos-native profile while EEPROM is the
                    // current backend — that's not a hard failure, the
                    // profile is still persisted and the hardware EEPROM
                    // just doesn't change. Same treatment we use in
                    // /backend/select.
                    if matches!(backend_id, BackendId::Eeprom)
                        && matches!(e, crate::backend::BackendError::ProfileRejected(_))
                    {
                        tracing::debug!(
                            backend = %backend_id,
                            %e,
                            "eeprom reload skipped — profile has no hardware section"
                        );
                    } else {
                        // Surface to the client — profile is persisted in
                        // the DB either way, but the user needs to know
                        // their bindings didn't activate. The 4xx/5xx
                        // mapping in error.rs picks the right status
                        // (NotReady → 503, ProfileRejected → 400, Internal
                        // → 500).
                        return Err(backend_error_to_api(e, backend_id));
                    }
                }
            }
        }
    }

    Ok(Json(ActiveResponse { id: Some(body.id) }))
}

/// Should the active backend even try to apply this profile? Pre-flight
/// check so we don't fire a guaranteed-to-fail apply() and surface
/// `ProfileRejected` to the user when the right outcome is "no-op, this
/// profile isn't for me".
///
/// Rules (matches each backend's apply() expectations):
/// - **EEPROM**: needs `_roninKB.hardware` populated.
/// - **Software backends** (kanata / hidutil / macos-native): the profile's
///   `_roninKB.software.engine` either names this backend, or is missing
///   entirely (macos-native treats absence as "use PoC default"; kanata /
///   hidutil treat absence as "nothing to apply, skip cleanly").
pub(crate) fn profile_targets_backend(profile: &hhkb_core::ViaProfile, backend: BackendId) -> bool {
    match backend {
        BackendId::Eeprom => profile
            .ronin
            .as_ref()
            .and_then(|r| r.hardware.as_ref())
            .is_some(),
        BackendId::Kanata | BackendId::Hidutil | BackendId::MacosNative => {
            let engine = profile
                .ronin
                .as_ref()
                .and_then(|r| r.software.as_ref())
                .map(|s| s.engine.as_str())
                .unwrap_or("");
            // macos-native is forgiving — it falls back to PoC defaults
            // when the engine doesn't match. Kanata + hidutil reject, so
            // they only run when the engine name matches theirs.
            if backend == BackendId::MacosNative {
                engine.is_empty() || engine == backend.as_str()
            } else {
                engine == backend.as_str()
            }
        }
    }
}

fn backend_error_to_api(err: crate::backend::BackendError, backend: BackendId) -> ApiError {
    use crate::backend::BackendError;
    match err {
        BackendError::NotReady(missing) => ApiError::BackendNotReady {
            backend: backend.as_str().to_string(),
            missing: serde_json::to_value(missing).unwrap_or(serde_json::Value::Null),
        },
        BackendError::ProfileRejected(msg) => ApiError::InvalidConfig(format!("{backend}: {msg}")),
        BackendError::Internal(msg) => ApiError::Internal(format!("{backend} backend: {msg}")),
    }
}

fn validate_profile_kanata_config(via: &ViaProfile) -> ApiResult<()> {
    if let Some(cfg) =
        kanata_config::derive_profile_kanata_config(via).map_err(ApiError::InvalidConfig)?
    {
        kanata_config::validate_kanata_config(&cfg).map_err(ApiError::InvalidConfig)?;
    }
    Ok(())
}
