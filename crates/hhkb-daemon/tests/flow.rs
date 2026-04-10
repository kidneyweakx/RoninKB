//! End-to-end HTTP tests for the Flow (cross-device clipboard sync) module.
//!
//! These tests never touch the network. They rely on `FlowManager::enable`
//! being graceful when mDNS startup fails so we can exercise the full HTTP
//! surface in CI and on locked-down developer machines.

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

async fn get(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn post_json(app: &axum::Router, uri: &str, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn delete(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn flow_default_disabled() {
    let app = app();
    let resp = get(&app, "/flow/config").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["enabled"], false);
    assert_eq!(body["auto_sync"], true);
    assert_eq!(body["history_limit"], 50);
    assert!(body["instance_id"].is_string());
    assert!(body["instance_name"].is_string());
}

#[tokio::test]
async fn flow_enable_then_disable() {
    let app = app();

    let resp = post_json(&app, "/flow/enable", json!({})).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let cfg = body_to_json(get(&app, "/flow/config").await).await;
    assert_eq!(cfg["enabled"], true);

    let resp = post_json(&app, "/flow/disable", json!({})).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let cfg = body_to_json(get(&app, "/flow/config").await).await;
    assert_eq!(cfg["enabled"], false);
}

// ---------------------------------------------------------------------------
// Peers CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn flow_peer_crud() {
    let app = app();

    // Initially empty
    let body = body_to_json(get(&app, "/flow/peers").await).await;
    assert_eq!(body["peers"].as_array().unwrap().len(), 0);

    // Add one
    let resp = post_json(
        &app,
        "/flow/peers",
        json!({ "hostname": "iMac", "addr": "192.168.1.100:7331" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let created = body_to_json(resp).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["hostname"], "iMac");
    assert_eq!(created["addr"], "192.168.1.100:7331");

    // List: one
    let body = body_to_json(get(&app, "/flow/peers").await).await;
    assert_eq!(body["peers"].as_array().unwrap().len(), 1);

    // Delete
    let resp = delete(&app, &format!("/flow/peers/{id}")).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // List: empty again
    let body = body_to_json(get(&app, "/flow/peers").await).await;
    assert_eq!(body["peers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn flow_add_peer_rejects_empty_fields() {
    let app = app();
    let resp = post_json(
        &app,
        "/flow/peers",
        json!({ "hostname": "", "addr": "1.2.3.4:7331" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Sync / receive / history
// ---------------------------------------------------------------------------

#[tokio::test]
async fn flow_sync_requires_enable() {
    let app = app();
    let resp = post_json(&app, "/flow/sync", json!({ "content": "hi" })).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = body_to_json(resp).await;
    assert_eq!(body["error"], "flow_disabled");
}

#[tokio::test]
async fn flow_sync_local_appears_in_history() {
    let app = app();

    // Enable and turn off auto_sync so we don't try to hit any peers.
    post_json(&app, "/flow/enable", json!({})).await;
    let mut cfg = body_to_json(get(&app, "/flow/config").await).await;
    cfg["auto_sync"] = json!(false);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/flow/config")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&cfg).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Push a clip.
    let resp = post_json(&app, "/flow/sync", json!({ "content": "hello world" })).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["entry"]["content"], "hello world");
    assert_eq!(body["entry"]["source"]["type"], "local");

    // Should show in history.
    let body = body_to_json(get(&app, "/flow/history").await).await;
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["content"], "hello world");
    assert_eq!(entries[0]["source"]["type"], "local");
    assert_eq!(entries[0]["mime"], "text/plain");
}

#[tokio::test]
async fn flow_receive_from_peer_marked_as_peer_source() {
    let app = app();
    post_json(&app, "/flow/enable", json!({})).await;

    let peer_id = "00000000-0000-0000-0000-000000000abc";
    let resp = post_json(
        &app,
        "/flow/receive",
        json!({
            "peer_id": peer_id,
            "hostname": "iMac",
            "content": "from peer",
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp).await;
    assert_eq!(body["entry"]["source"]["type"], "peer");
    assert_eq!(body["entry"]["source"]["peer_id"], peer_id);
    assert_eq!(body["entry"]["source"]["hostname"], "iMac");

    let body = body_to_json(get(&app, "/flow/history").await).await;
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["source"]["type"], "peer");
}

#[tokio::test]
async fn flow_history_respects_limit() {
    let app = app();

    // Enable, set a small limit, disable auto_sync.
    post_json(&app, "/flow/enable", json!({})).await;
    let mut cfg = body_to_json(get(&app, "/flow/config").await).await;
    cfg["auto_sync"] = json!(false);
    cfg["history_limit"] = json!(5);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/flow/config")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&cfg).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    for i in 0..60 {
        let resp = post_json(&app, "/flow/sync", json!({ "content": format!("e{i}") })).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let body = body_to_json(get(&app, "/flow/history").await).await;
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 5);
    // Most recent first.
    assert_eq!(entries[0]["content"], "e59");
    assert_eq!(entries[4]["content"], "e55");
}

#[tokio::test]
async fn flow_history_clear() {
    let app = app();
    post_json(&app, "/flow/enable", json!({})).await;
    let mut cfg = body_to_json(get(&app, "/flow/config").await).await;
    cfg["auto_sync"] = json!(false);
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/flow/config")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&cfg).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    post_json(&app, "/flow/sync", json!({ "content": "a" })).await;
    post_json(&app, "/flow/sync", json!({ "content": "b" })).await;

    let resp = delete(&app, "/flow/history").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_to_json(get(&app, "/flow/history").await).await;
    assert_eq!(body["entries"].as_array().unwrap().len(), 0);
}
