//! Embedded hhkb-app static UI (built with `npm run build` from apps/hhkb-app).
//!
//! Available behind the `embedded-ui` feature flag. Build prerequisite:
//! the `apps/hhkb-app/dist` directory must exist and contain a built bundle.
//! The build script `build.rs` enforces this.

#![cfg(feature = "embedded-ui")]

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../apps/hhkb-app/dist"]
struct EmbeddedUi;

/// Axum handler for `/ui`, `/ui/`, and `/ui/*path`. Serves the embedded
/// SPA, falling back to `index.html` for client-side routes (HTML5 history
/// API).
pub async fn ui_handler(uri: Uri) -> Response {
    let path = uri
        .path()
        .trim_start_matches("/ui")
        .trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match EmbeddedUi::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            // Hashed Vite assets under /assets/* are safe to cache forever;
            // index.html lands in the miss branch below with a no-cache header.
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(
                    header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable",
                )
                .body(Body::from(file.data.into_owned()))
                .unwrap()
        }
        None => {
            // SPA fallback: serve index.html for unknown paths so React Router
            // (or any client-side router) can handle the route.
            match EmbeddedUi::get("index.html") {
                Some(file) => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .body(Body::from(file.data.into_owned()))
                    .unwrap(),
                None => (StatusCode::NOT_FOUND, "ui not embedded").into_response(),
            }
        }
    }
}
