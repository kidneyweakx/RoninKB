//! hhkb-daemon binary entrypoint.
//!
//! Starts the HTTP+WebSocket server on `127.0.0.1:7331` and wires up the
//! shared [`AppState`] (device handle + SQLite profile store).

use hhkb_daemon::{build_router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,hhkb_daemon=debug")),
        )
        .init();

    let state = AppState::new().await?;
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7331").await?;
    tracing::info!("hhkb-daemon listening on http://127.0.0.1:7331");
    axum::serve(listener, app).await?;
    Ok(())
}
