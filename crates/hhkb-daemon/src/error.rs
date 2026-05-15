//! Error type that implements `IntoResponse` for uniform HTTP error returns.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("device not connected")]
    DeviceUnavailable,

    #[error("device error: {0}")]
    Device(#[from] hhkb_core::Error),

    #[error("profile not found")]
    NotFound,

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid request: {0}")]
    BadRequest(String),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("kanata binary not installed")]
    KanataNotInstalled,

    #[error("kanata is already running")]
    KanataAlreadyRunning,

    #[error("kanata is not running")]
    KanataNotRunning,

    #[error("kanata io error: {0}")]
    KanataIo(#[from] std::io::Error),

    #[error("kanata permission required: {0}")]
    KanataPermissionRequired(String),

    #[error("kanata device unavailable: {0}")]
    KanataDeviceUnavailable(String),

    #[error("kanata driver not activated: {0}")]
    KanataDriverMissing(String),

    // -- Flow (cross-device clipboard sync) --------------------------------
    #[error("Flow error: {0}")]
    Flow(#[from] crate::flow::FlowError),

    /// A `/kanata/*` mutating route was hit while the active backend isn't
    /// kanata. v0.1.x clients still see compat aliases for `/kanata/status`
    /// and `/kanata/config` (those stay 200 OK) — but driver / start / stop
    /// / reload only make sense when kanata is the actual active backend.
    /// Per M4 §82, returning 409 with `backend_inactive` lets old clients
    /// detect that they need to switch via `/backend/select` first.
    #[error("backend kanata is not active (current: {active})")]
    BackendInactive { active: String },

    /// The active backend can't apply the profile because OS-level
    /// permissions (Input Monitoring, Accessibility, sysext) are missing.
    /// Returned as 503 with the missing-permission list so the UI can
    /// render the right deep-links — same shape as `/backend/list`'s
    /// `permission_status.required.permissions`.
    #[error("backend {backend} is not ready (missing permissions)")]
    BackendNotReady {
        backend: String,
        missing: serde_json::Value,
    },
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(err: tokio::task::JoinError) -> Self {
        ApiError::Internal(format!("task join: {err}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Pull the BackendInactive `active` field out before the match so we
        // can emit the optional `next_action` body field below without
        // re-matching. None for every other variant.
        let backend_inactive_active = match &self {
            ApiError::BackendInactive { active } => Some(active.clone()),
            _ => None,
        };
        let backend_not_ready = match &self {
            ApiError::BackendNotReady { backend, missing } => {
                Some((backend.clone(), missing.clone()))
            }
            _ => None,
        };

        let (status, code) = match &self {
            ApiError::DeviceUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "device_unavailable"),
            ApiError::Device(_) => (StatusCode::INTERNAL_SERVER_ERROR, "device_error"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "db_error"),
            ApiError::Json(_) => (StatusCode::BAD_REQUEST, "bad_json"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::InvalidConfig(_) => (StatusCode::BAD_REQUEST, "invalid_config"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            ApiError::KanataNotInstalled => {
                (StatusCode::SERVICE_UNAVAILABLE, "kanata_not_installed")
            }
            ApiError::KanataAlreadyRunning => (StatusCode::CONFLICT, "kanata_already_running"),
            ApiError::KanataNotRunning => (StatusCode::CONFLICT, "kanata_not_running"),
            ApiError::KanataIo(_) => (StatusCode::INTERNAL_SERVER_ERROR, "kanata_io_error"),
            ApiError::KanataPermissionRequired(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "kanata_permission_required",
            ),
            ApiError::KanataDeviceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "kanata_device_unavailable")
            }
            ApiError::KanataDriverMissing(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "kanata_driver_missing")
            }
            ApiError::BackendInactive { .. } => (StatusCode::CONFLICT, "backend_inactive"),
            ApiError::BackendNotReady { .. } => {
                (StatusCode::SERVICE_UNAVAILABLE, "backend_not_ready")
            }
            ApiError::Flow(e) => match e {
                crate::flow::FlowError::Disabled => {
                    (StatusCode::SERVICE_UNAVAILABLE, "flow_disabled")
                }
                crate::flow::FlowError::Mdns(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "flow_mdns_error")
                }
                crate::flow::FlowError::PeerUnreachable(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "flow_peer_unreachable")
                }
            },
        };

        let body = if let Some(active) = backend_inactive_active {
            // RFC 0001 §10 contract: the 409 carries enough information for a
            // v0.1.x client to recover automatically — `active` names the
            // backend that owns the keyboard right now, and `next_action`
            // points at the canonical fix (`POST /backend/select`).
            Json(json!({
                "error": code,
                "message": self.to_string(),
                "active": active,
                "next_action": "POST /backend/select with {\"id\":\"kanata\"}",
            }))
        } else if let Some((backend, missing)) = backend_not_ready {
            // 503 carries the missing permission list verbatim so the UI can
            // render Open-System-Settings deep links without an extra round
            // trip to /backend/list.
            Json(json!({
                "error": code,
                "message": self.to_string(),
                "backend": backend,
                "missing": missing,
            }))
        } else {
            Json(json!({
                "error": code,
                "message": self.to_string(),
            }))
        };

        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
