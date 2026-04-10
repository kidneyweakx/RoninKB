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
/// winit event loop (required by Cocoa on macOS so that menu events are
/// actually delivered). Menu actions trigger graceful shutdown and other
/// behaviour via the `ApplicationHandler` impl below.
#[cfg(feature = "tray")]
fn tray_main() -> anyhow::Result<()> {
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::WindowId,
    };
    #[cfg(target_os = "macos")]
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

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

    // winit drives the native event loop (required for Cocoa on macOS).
    struct TrayApp {
        tray: hhkb_daemon::tray::TrayController,
        shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    }

    impl ApplicationHandler for TrayApp {
        fn resumed(&mut self, _el: &ActiveEventLoop) {}

        fn window_event(
            &mut self,
            _el: &ActiveEventLoop,
            _wid: WindowId,
            _event: WindowEvent,
        ) {
        }

        fn about_to_wait(&mut self, el: &ActiveEventLoop) {
            el.set_control_flow(ControlFlow::Poll);
            std::thread::sleep(std::time::Duration::from_millis(80));

            match self.tray.poll() {
                hhkb_daemon::tray::TrayAction::Quit => {
                    tracing::info!("tray: quit clicked");
                    if let Some(tx) = self.shutdown_tx.take() {
                        let _ = tx.send(());
                    }
                    el.exit();
                }
                hhkb_daemon::tray::TrayAction::OpenUi => {
                    tracing::info!("tray: opening web UI");
                    let _ = open::that("http://127.0.0.1:7331/ui/");
                }
                hhkb_daemon::tray::TrayAction::ToggleAutostart => {
                    let enabled = hhkb_daemon::autostart::is_enabled();
                    if enabled {
                        if let Err(e) = hhkb_daemon::autostart::disable() {
                            tracing::warn!("autostart disable failed: {e}");
                        }
                    } else if let Err(e) = hhkb_daemon::autostart::enable() {
                        tracing::warn!("autostart enable failed: {e}");
                    }
                    self.tray.sync_autostart_check();
                }
                hhkb_daemon::tray::TrayAction::Reconnect => {
                    tracing::info!("tray: reconnect requested (no-op in v1)");
                }
                hhkb_daemon::tray::TrayAction::None => {}
            }
        }
    }

    let tray = hhkb_daemon::tray::TrayController::new()
        .map_err(|e| anyhow::anyhow!("tray init: {e}"))?;

    // Build event loop. On macOS, Accessory policy hides the Dock icon and
    // Cmd+Tab entry — this process should live only in the menu bar.
    let event_loop = {
        let mut builder = EventLoop::builder();
        #[cfg(target_os = "macos")]
        builder.with_activation_policy(ActivationPolicy::Accessory);
        builder.build()?
    };
    let mut app = TrayApp {
        tray,
        shutdown_tx: Some(shutdown_tx),
    };
    event_loop.run_app(&mut app)?;

    match server_thread.join() {
        Ok(inner) => inner?,
        Err(_) => anyhow::bail!("axum server thread panicked"),
    }
    Ok(())
}
