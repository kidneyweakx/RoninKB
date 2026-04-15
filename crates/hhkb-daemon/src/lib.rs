//! hhkb-daemon library.
//!
//! Exposes the HTTP+WebSocket router and application state so that both the
//! `main.rs` binary and the integration tests can reuse the same code paths.

pub mod autostart;
pub mod ble;
pub mod db;
pub mod error;
pub mod flow;
pub mod kanata;
pub mod kanata_config;
pub mod routes;
pub mod state;
pub mod tray;
#[cfg(feature = "embedded-ui")]
pub mod ui;
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

    let router = Router::new()
        .route("/health", get(routes::health::health))
        .route("/device/info", get(routes::device::get_info))
        .route("/device/mode", get(routes::device::get_mode))
        .route("/device/dipsw", get(routes::device::get_dipsw))
        .route("/device/connected", get(routes::device::get_connected))
        .route("/device/bluetooth", get(routes::bluetooth::get_bluetooth))
        .route("/device/bluetooth/scan", post(routes::bluetooth::post_scan))
        .route(
            "/device/bluetooth/devices",
            get(routes::bluetooth::get_devices),
        )
        .route(
            "/device/bluetooth/system",
            get(routes::bluetooth::get_system_devices),
        )
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
        // -- Flow (cross-device clipboard sync) ---------------------------
        .route(
            "/flow/config",
            get(routes::flow::get_config).put(routes::flow::put_config),
        )
        .route("/flow/enable", post(routes::flow::enable))
        .route("/flow/disable", post(routes::flow::disable))
        .route(
            "/flow/peers",
            get(routes::flow::list_peers).post(routes::flow::add_peer),
        )
        .route(
            "/flow/peers/:id",
            axum::routing::delete(routes::flow::remove_peer),
        )
        .route(
            "/flow/history",
            get(routes::flow::get_history).delete(routes::flow::clear_history),
        )
        .route("/flow/sync", post(routes::flow::sync))
        .route("/flow/receive", post(routes::flow::receive))
        // -----------------------------------------------------------------
        .route("/ws", get(ws::ws_handler))
        .with_state(state);

    // Embedded UI: when the feature is enabled, also serve the React bundle
    // at `/ui/*` and redirect `/` to `/ui/` so users can open the bare
    // `http://127.0.0.1:7331/` and land on the app.
    #[cfg(feature = "embedded-ui")]
    let router = router
        .route(
            "/",
            get(|| async { axum::response::Redirect::temporary("/ui/") }),
        )
        .route("/ui", get(ui::ui_handler))
        .route("/ui/", get(ui::ui_handler))
        .route("/ui/*path", get(ui::ui_handler));

    router.layer(cors).layer(TraceLayer::new_for_http())
}
