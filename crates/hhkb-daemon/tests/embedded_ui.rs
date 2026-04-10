//! Integration tests for the `embedded-ui` feature.
//!
//! PREREQUISITE: `apps/hhkb-app/dist/index.html` must exist. The daemon's
//! `build.rs` enforces this at compile time, so if this test file compiles
//! at all under `--features embedded-ui` the dist is present.
//!
//! Build & run:
//!   cd apps/hhkb-app && npm run build
//!   cargo test -p hhkb-daemon --features embedded-ui

#![cfg(feature = "embedded-ui")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use hhkb_daemon::{build_router, AppState};

fn app() -> axum::Router {
    build_router(AppState::for_tests())
}

async fn body_to_bytes(resp: axum::response::Response) -> Vec<u8> {
    resp.into_body().collect().await.unwrap().to_bytes().to_vec()
}

#[tokio::test]
async fn ui_serves_index_html() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/ui/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.starts_with("text/html"), "unexpected content-type: {ct}");
    let body = body_to_bytes(resp).await;
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("<html"), "expected html markup in index, got: {html:.200}");
}

#[tokio::test]
async fn ui_bare_prefix_serves_index_html() {
    // `/ui` (no trailing slash) should also land on index.html so sharable
    // links work regardless of whether the user typed the slash.
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/ui")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_bytes(resp).await;
    assert!(std::str::from_utf8(&body).unwrap().contains("<html"));
}

#[tokio::test]
async fn ui_unknown_path_falls_back_to_index() {
    // SPA fallback: React Router should receive the path client-side, so the
    // daemon must return index.html with 200 for any unknown /ui/* route.
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/ui/this/does/not/exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_bytes(resp).await;
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("<html"));
}

#[tokio::test]
async fn root_redirects_to_ui() {
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        resp.headers().get("location").unwrap().to_str().unwrap(),
        "/ui/"
    );
}

#[tokio::test]
async fn api_routes_still_work_alongside_ui() {
    // Regression guard: adding the UI routes must not break existing REST
    // endpoints.
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
}
