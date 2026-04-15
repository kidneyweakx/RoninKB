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

    // -- Flow (cross-device clipboard sync) --------------------------------
    #[error("Flow error: {0}")]
    Flow(#[from] crate::flow::FlowError),
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

        let body = Json(json!({
            "error": code,
            "message": self.to_string(),
        }));

        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
