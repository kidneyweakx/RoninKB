//! hhkb-daemon library.
//!
//! Exposes the HTTP+WebSocket router and application state so that both the
//! `main.rs` binary and the integration tests can reuse the same code paths.

pub mod db;
pub mod error;
pub mod kanata;
pub mod routes;
pub mod state;
pub mod ws;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// Build the full axum router for the daemon.
pub fn build_router(state: AppState) -> Router {
    // Permissive CORS so the WebHID app (running on any localhost port) can
    // talk to the daemon without preflight rejection.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(routes::health::health))
        .route("/device/info", get(routes::device::get_info))
        .route("/device/mode", get(routes::device::get_mode))
        .route("/device/dipsw", get(routes::device::get_dipsw))
        .route("/device/connected", get(routes::device::get_connected))
        .route(
            "/device/keymap",
            get(routes::keymap::get_keymap).put(routes::keymap::put_keymap),
        )
        .route(
            "/profiles",
            get(routes::profile::list).post(routes::profile::create),
        )
        .route(
            "/profiles/active",
            get(routes::profile::get_active).post(routes::profile::set_active),
        )
        .route(
            "/profiles/:id",
            get(routes::profile::get_one)
                .put(routes::profile::update)
                .delete(routes::profile::delete_one),
        )
        .route("/kanata/status", get(routes::kanata::status))
        .route("/kanata/start", post(routes::kanata::start))
        .route("/kanata/stop", post(routes::kanata::stop))
        .route("/kanata/reload", post(routes::kanata::reload))
        .route("/kanata/config", get(routes::kanata::get_config))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

