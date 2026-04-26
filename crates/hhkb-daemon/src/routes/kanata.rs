//! `/kanata/*` — control and inspection of the Kanata software layer.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::kanata::KanataStatus;
use crate::kanata_config;
use crate::state::AppState;
use crate::ws::DaemonEvent;

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub installed: bool,
    pub binary_path: Option<String>,
    pub config_path: String,
    pub input_monitoring_granted: Option<bool>,
    pub last_error: Option<String>,
    pub stderr_tail: Vec<String>,
    pub device_path: Option<String>,
    #[serde(flatten)]
    pub status: KanataStatus,
}

#[derive(Debug, Serialize)]
pub struct StartResponse {
    pub pid: u32,
}

#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub status: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct ReloadBody {
    pub config: String,
}

#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub config: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct RevealResponse {
    pub path: String,
    pub bundle: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    // Use check_alive so we reap a crashed child lazily on every poll.
    let kanata = state.kanata.clone();
    let (
        status,
        installed,
        config_path,
        binary_path,
        input_monitoring_granted,
        last_error,
        stderr_tail,
        device_path,
    ) = tokio::task::spawn_blocking(move || {
        let s = kanata.check_alive();
        (
            s,
            kanata.is_installed(),
            kanata.config_path().to_path_buf(),
            kanata.binary_path().map(|p| p.display().to_string()),
            kanata.input_monitoring_granted(),
            kanata.last_error(),
            kanata.stderr_tail(20),
            kanata.last_device_path(),
        )
    })
    .await
    .unwrap_or((
        KanataStatus::Stopped,
        false,
        Default::default(),
        None,
        None,
        Some("failed to read kanata status".to_string()),
        vec![],
        None,
    ));

    Json(StatusResponse {
        installed,
        binary_path,
        config_path: config_path.display().to_string(),
        input_monitoring_granted,
        last_error,
        stderr_tail,
        device_path,
        status,
    })
}

pub async fn start(State(state): State<AppState>) -> ApiResult<Json<StartResponse>> {
    ensure_startup_config(&state).await?;
    let kanata = state.kanata.clone();
    let pid = tokio::task::spawn_blocking(move || kanata.start()).await??;
    let _ = state.events.send(DaemonEvent::KanataStarted { pid });
    Ok(Json(StartResponse { pid }))
}

pub async fn stop(State(state): State<AppState>) -> ApiResult<Json<OkResponse>> {
    let kanata = state.kanata.clone();
    tokio::task::spawn_blocking(move || kanata.stop()).await??;
    let _ = state.events.send(DaemonEvent::KanataStopped);
    Ok(Json(OkResponse { status: "stopped" }))
}

pub async fn reload(
    State(state): State<AppState>,
    Json(body): Json<ReloadBody>,
) -> ApiResult<Json<OkResponse>> {
    kanata_config::validate_kanata_config(&body.config)
        .map_err(crate::error::ApiError::InvalidConfig)?;
    let kanata = state.kanata.clone();
    let cfg = body.config;
    tokio::task::spawn_blocking(move || kanata.reload(&cfg)).await??;
    // This endpoint doesn't know which profile triggered the reload, so
    // broadcast with an empty profile_id; the profile route emits a fuller
    // event when reload happens via profile switch.
    let _ = state.events.send(DaemonEvent::KanataReloaded {
        profile_id: String::new(),
    });
    Ok(Json(OkResponse { status: "reloaded" }))
}

/// `POST /kanata/reveal` — open a Finder/Explorer window with the kanata
/// binary (or its enclosing `.app` bundle on macOS) selected, so the user
/// can drag it into the Input Monitoring picker.
pub async fn reveal(State(state): State<AppState>) -> ApiResult<Json<RevealResponse>> {
    let path = state
        .kanata
        .binary_path()
        .ok_or(ApiError::KanataNotInstalled)?;

    // On macOS the binary lives at `<bundle>.app/Contents/MacOS/kanata`. We
    // reveal the enclosing `.app` so Finder shows it as a single icon.
    let reveal_target = bundle_root_for(&path).unwrap_or_else(|| path.clone());

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(&reveal_target)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let parent = path.parent().unwrap_or(std::path::Path::new("."));
        let _ = std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer.exe")
            .arg("/select,")
            .arg(&reveal_target)
            .spawn();
    }

    Ok(Json(RevealResponse {
        path: path.display().to_string(),
        bundle: bundle_root_for(&path).map(|p| p.display().to_string()),
    }))
}

/// Walk up from a binary inside `<X>.app/Contents/MacOS/<bin>` and return
/// the `.app` root. Returns `None` if the path doesn't match that layout.
fn bundle_root_for(binary: &std::path::Path) -> Option<std::path::PathBuf> {
    let macos_dir = binary.parent()?;
    if macos_dir.file_name()?.to_str()? != "MacOS" {
        return None;
    }
    let contents = macos_dir.parent()?;
    if contents.file_name()?.to_str()? != "Contents" {
        return None;
    }
    let app_root = contents.parent()?;
    if app_root.extension()?.to_str()? != "app" {
        return None;
    }
    Some(app_root.to_path_buf())
}

pub async fn get_config(State(state): State<AppState>) -> ApiResult<Json<ConfigResponse>> {
    let kanata = state.kanata.clone();
    let (cfg, path) = tokio::task::spawn_blocking(move || {
        let path = kanata.config_path().display().to_string();
        kanata.read_config().map(|c| (c, path))
    })
    .await??;
    Ok(Json(ConfigResponse { config: cfg, path }))
}

async fn ensure_startup_config(state: &AppState) -> ApiResult<()> {
    let active_profile_cfg = {
        let conn = state.db.lock().await;
        match db::get_active(&conn)? {
            Some(id) => {
                let rec = db::get_profile(&conn, &id)?;
                match kanata_config::derive_profile_kanata_config(&rec.via) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        tracing::warn!(profile = %id, %e, "invalid profile kanata config; falling back to minimal config");
                        None
                    }
                }
            }
            None => None,
        }
    };

    let kanata = state.kanata.clone();
    tokio::task::spawn_blocking(move || {
        let current = kanata.read_config()?;
        if current.trim().is_empty() {
            let next =
                active_profile_cfg.unwrap_or_else(|| kanata_config::default_minimal_config(60));
            kanata_config::validate_kanata_config(&next)
                .map_err(crate::error::ApiError::InvalidConfig)?;
            kanata.write_config(&next)?;
            return Ok(()) as ApiResult<()>;
        }

        kanata_config::validate_kanata_config(&current)
            .map_err(crate::error::ApiError::InvalidConfig)?;
        Ok(())
    })
    .await??;

    Ok(())
}
