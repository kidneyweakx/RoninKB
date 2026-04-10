//! hhkb-daemon binary entrypoint.
//!
//! Starts the HTTP+WebSocket server on `127.0.0.1:7331` and wires up the
//! shared [`AppState`] (device handle + SQLite profile store).
//!
//! When built with `--features tray`, the axum server runs on a worker
//! thread so that the main thread is free to drive the `tray-icon` event
//! loop (required by AppKit and Win32). See `src/tray.rs` for runtime
//! caveats.

use hhkb_daemon::{build_router, AppState};

/// Startup banner so users instantly see which endpoints are available.
/// With `embedded-ui`, the bare host URL redirects to `/ui/`, so we advertise
/// both the REST API and the Web UI in one line.
fn startup_banner() -> &'static str {
    #[cfg(feature = "embedded-ui")]
    {
        "hhkb-daemon listening on http://127.0.0.1:7331 (REST API + Web UI at /ui/)"
    }
    #[cfg(not(feature = "embedded-ui"))]
    {
        "hhkb-daemon listening on http://127.0.0.1:7331 (REST API)"
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,hhkb_daemon=debug")),
        )
        .init();

    #[cfg(feature = "tray")]
    {
        tray_main()
    }

    #[cfg(not(feature = "tray"))]
    {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async_main())
    }
}

/// Headless mode: axum runs on the main tokio runtime.
#[cfg(not(feature = "tray"))]
async fn async_main() -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7331").await?;
    tracing::info!("{}", startup_banner());
    axum::serve(listener, app).await?;
    Ok(())
}

/// Tray mode: spawn axum on a worker thread, keep the main thread for the
/// tray event loop. Menu "Quit" triggers a graceful shutdown of the server.
#[cfg(feature = "tray")]
fn tray_main() -> anyhow::Result<()> {
    use std::time::Duration;

    let tray = hhkb_daemon::tray::TrayController::new()
        .map_err(|e| anyhow::anyhow!("tray init: {}", e))?;

    // One-shot channel used to tell axum to exit cleanly.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_thread = std::thread::spawn(move || -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async move {
            let state = AppState::new().await?;
            let app = build_router(state);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:7331").await?;
            tracing::info!("{}", startup_banner());
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                    tracing::info!("tray: shutdown requested, draining axum");
                })
                .await?;
            Ok::<_, anyhow::Error>(())
        })
    });

    // Main-thread tray poll loop. 100 ms is fast enough that "Quit" feels
    // instant without burning CPU. On macOS this loop still runs but the
    // AppKit event pump isn't spinning, so menu events never arrive — the
    // icon is effectively cosmetic there in v1 (see src/tray.rs).
    let mut shutdown_tx = Some(shutdown_tx);
    loop {
        match tray.poll() {
            hhkb_daemon::tray::TrayAction::Quit => {
                tracing::info!("tray: quit clicked");
                if let Some(tx) = shutdown_tx.take() {
                    let _ = tx.send(());
                }
                break;
            }
            hhkb_daemon::tray::TrayAction::Reconnect => {
                // TODO: plumb a broadcast channel into AppState so this can
                // force a device reopen. For v1 the /device/* endpoints
                // already lazily reconnect on demand.
                tracing::info!("tray: reconnect requested (no-op in v1)");
            }
            hhkb_daemon::tray::TrayAction::None => {}
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    match server_thread.join() {
        Ok(inner) => inner?,
        Err(_) => anyhow::bail!("axum server thread panicked"),
    }
    Ok(())
}
