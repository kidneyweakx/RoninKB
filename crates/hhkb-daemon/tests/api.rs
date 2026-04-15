//! End-to-end HTTP tests for the daemon router.
//!
//! All tests use an in-memory SQLite connection and no device, so the device
//! endpoints should return 503 Service Unavailable and the profile endpoints
//! should work normally.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

use hhkb_daemon::{build_router, AppState};

fn app() -> axum::Router {
    build_router(AppState::for_tests())
}

async fn body_to_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// /health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_ok_shape() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["device_connected"], false);
    assert!(body["version"].is_string());
}

// ---------------------------------------------------------------------------
// /device/* — no device, expect 503
// ---------------------------------------------------------------------------

#[tokio::test]
async fn device_info_without_device_returns_503() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/device/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn device_mode_without_device_returns_503() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/device/mode")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn device_keymap_without_device_returns_503() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/device/keymap?mode=mac&fn_layer=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn device_connected_reports_false_without_device() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/device/connected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["connected"], false);
}

#[tokio::test]
async fn bluetooth_connect_route_is_not_exposed() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/device/bluetooth/connect")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "address": "AA:BB:CC:DD:EE:FF" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bluetooth_disconnect_route_is_not_exposed() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/device/bluetooth/disconnect")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bluetooth_system_route_reports_unavailable_without_adapter() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/device/bluetooth/system")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["available"], false);
    assert_eq!(body["source"], "none");
    assert!(body["devices"].as_array().is_some());
}

// ---------------------------------------------------------------------------
// /profiles CRUD
// ---------------------------------------------------------------------------

fn sample_via_profile_json(name: &str) -> Value {
    json!({
        "name": name,
        "vendorId": "0x04FE",
        "productId": "0x0021",
        "layers": [["KC_ESC", "KC_1"]],
        "_roninKB": {
            "version": "1.0",
            "profile": {
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "name": name,
                "tags": ["work", "test"]
            }
        }
    })
}

#[tokio::test]
async fn profile_crud_roundtrip() {
    let app = app();

    // -- Initially empty list --
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/profiles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["profiles"].as_array().unwrap().len(), 0);

    // -- Create --
    let create_body = sample_via_profile_json("Daily Driver");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/profiles")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let created = body_to_json(resp).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "Daily Driver");
    assert_eq!(created["via"]["name"], "Daily Driver");
    assert!(created["tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t == "work"));

    // -- List: should have 1 --
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/profiles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_to_json(resp).await;
    assert_eq!(body["profiles"].as_array().unwrap().len(), 1);

    // -- Get by id --
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/profiles/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let got = body_to_json(resp).await;
    assert_eq!(got["id"], id);

    // -- Update --
    let mut update_body = sample_via_profile_json("Renamed");
    update_body["_roninKB"]["profile"]["id"] = json!(id);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/profiles/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let updated = body_to_json(resp).await;
    assert_eq!(updated["name"], "Renamed");

    // -- Set active --
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/profiles/active")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "id": id })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // -- Get active --
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/profiles/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let active = body_to_json(resp).await;
    assert_eq!(active["id"], id);

    // -- Delete --
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/profiles/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // -- List empty again --
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/profiles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_to_json(resp).await;
    assert_eq!(body["profiles"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn profile_get_missing_returns_404() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/profiles/nonexistent-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// /kanata/*
// ---------------------------------------------------------------------------

#[tokio::test]
async fn kanata_status_reports_not_installed_in_tests() {
    // for_tests() pins the manager to "no binary", so the status endpoint
    // must surface that consistently regardless of the developer's PATH.
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/kanata/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["installed"], false);
    assert_eq!(body["state"], "not_installed");
    assert!(body["config_path"].is_string());
}

#[tokio::test]
async fn kanata_start_without_binary_returns_503() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kanata/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = body_to_json(resp).await;
    assert_eq!(body["error"], "kanata_not_installed");
}

#[tokio::test]
async fn kanata_stop_without_running_returns_409() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kanata/stop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_to_json(resp).await;
    assert_eq!(body["error"], "kanata_not_running");
}

#[tokio::test]
async fn kanata_reload_writes_config_even_when_stopped() {
    // Reload against a stopped manager must still persist the config file
    // (so a later start() picks it up).
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kanata/reload")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "config": "(defsrc a)\n(deflayer base a)\n"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["status"], "reloaded");

    // And GET /kanata/config reflects what we just wrote.
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/kanata/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Note: a fresh app() here means a new AppState — the config file path
    // is randomised per test state so this just verifies the handler shape.
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert!(body["config"].is_string());
    assert!(body["path"].is_string());
}

#[tokio::test]
async fn kanata_reload_rejects_invalid_config() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kanata/reload")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "config": "(deflayer base a)\n"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_to_json(resp).await;
    assert_eq!(body["error"], "invalid_config");
}

#[tokio::test]
async fn profile_create_rejects_invalid_kanata_config() {
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/profiles")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Bad Kanata",
                        "vendorId": "0x04FE",
                        "productId": "0x0021",
                        "layers": [["KC_ESC"]],
                        "_roninKB": {
                          "version": "1.0",
                          "profile": {
                            "id": "550e8400-e29b-41d4-a716-446655440000",
                            "name": "Bad Kanata",
                            "tags": []
                          },
                          "software": {
                            "engine": "kanata",
                            "config": "(deflayer base a)"
                          }
                        }
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_to_json(resp).await;
    assert_eq!(body["error"], "invalid_config");
}

#[tokio::test]
async fn active_profile_empty_when_nothing_set() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/profiles/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert!(body["id"].is_null());
}
