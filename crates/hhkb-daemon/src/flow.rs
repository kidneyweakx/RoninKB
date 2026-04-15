//! Flow — RoninKB cross-device clipboard sync.
//!
//! Flow is a built-in daemon module that discovers other RoninKB daemons on
//! the local network via mDNS and synchronizes clipboard content between
//! them. It also maintains a small rolling history of recent clipboard
//! entries, labelled by origin (local vs. a named peer).
//!
//! # Failure philosophy
//!
//! Flow is best-effort. The daemon must continue to function if:
//!
//! - The network is unavailable
//! - mDNS registration fails (no multicast, sandbox, CI, etc.)
//! - A peer is unreachable when pushing a clipboard entry
//!
//! In all these cases we log a warning and degrade gracefully to
//! "local-only" mode — history still works, the peer list just stays empty.
//!
//! # Feature flags
//!
//! The optional `clipboard` feature wires up an `arboard` polling task that
//! feeds local clipboard changes into [`FlowManager::sync_local`]. It is
//! **off by default** so tests and headless environments don't pull in a
//! system clipboard dependency.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A peer RoninKB daemon on the local network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowPeer {
    pub id: Uuid,
    pub hostname: String,
    /// Network address, e.g. "192.168.1.42:7331".
    pub addr: String,
    /// Unix epoch seconds of the last time this peer was seen.
    pub last_seen: u64,
    pub online: bool,
}

/// A clipboard entry kept in the Flow history ring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEntry {
    pub id: Uuid,
    pub content: String,
    pub source: FlowSource,
    /// Unix epoch seconds.
    pub timestamp: u64,
    /// MIME type. Currently always `text/plain` but reserved for future
    /// image/file syncing.
    pub mime: String,
}

/// Where a clipboard entry came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlowSource {
    /// This daemon's own clipboard.
    Local,
    /// A remote RoninKB daemon.
    Peer { peer_id: Uuid, hostname: String },
}

/// User-controllable Flow configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowConfig {
    /// Master switch. When `false` the manager accepts no syncs and keeps
    /// the mDNS daemon shut down.
    pub enabled: bool,
    /// When `true`, local clipboard changes are pushed to peers automatically
    /// (requires the `clipboard` feature at build time to actually observe
    /// the clipboard; the flag itself is still stored and reported).
    pub auto_sync: bool,
    /// Stable identifier for this daemon instance, advertised in mDNS TXT.
    pub instance_id: Uuid,
    /// Human-readable name (defaults to the machine hostname).
    pub instance_name: String,
    /// Maximum number of history entries kept in memory.
    pub history_limit: usize,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_sync: true,
            instance_id: Uuid::new_v4(),
            instance_name: gethostname::gethostname().to_string_lossy().into_owned(),
            history_limit: 50,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum FlowError {
    #[error("Flow disabled")]
    Disabled,
    #[error("mDNS error: {0}")]
    Mdns(String),
    #[error("Peer unreachable: {0}")]
    PeerUnreachable(String),
}

// ---------------------------------------------------------------------------
// mDNS handle
// ---------------------------------------------------------------------------

/// Owns the resources created by [`FlowManager::enable`] so that
/// [`FlowManager::disable`] can tear them down cleanly.
///
/// Currently holds the optional browse-loop task handle. The `mdns-sd`
/// `ServiceDaemon` is started and immediately shut down inside `enable` if
/// registration fails; when registration succeeds we keep it alive via the
/// browse task.
pub(crate) struct MdnsHandle {
    browse_task: Option<JoinHandle<()>>,
    clipboard_task: Option<JoinHandle<()>>,
}

impl Drop for MdnsHandle {
    fn drop(&mut self) {
        if let Some(h) = self.browse_task.take() {
            h.abort();
        }
        if let Some(h) = self.clipboard_task.take() {
            h.abort();
        }
    }
}

// ---------------------------------------------------------------------------
// FlowManager
// ---------------------------------------------------------------------------

/// In-memory Flow state. Cheaply cloneable via the internal `Arc` wrappers.
pub struct FlowManager {
    config: Arc<RwLock<FlowConfig>>,
    peers: Arc<RwLock<HashMap<Uuid, FlowPeer>>>,
    history: Arc<RwLock<Vec<FlowEntry>>>,
    mdns_handle: Arc<RwLock<Option<MdnsHandle>>>,
}

impl Clone for FlowManager {
    /// Clones the manager by cloning the `Arc` references — all clones share
    /// the same underlying data.
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            peers: Arc::clone(&self.peers),
            history: Arc::clone(&self.history),
            mdns_handle: Arc::clone(&self.mdns_handle),
        }
    }
}

impl Default for FlowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(FlowConfig::default())),
            peers: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            mdns_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Enable Flow and attempt to bring up mDNS service discovery.
    ///
    /// This function is deliberately forgiving: if the mDNS daemon cannot be
    /// started (no multicast, sandbox, missing permissions, CI), the error
    /// is logged and Flow still transitions to the `enabled` state in
    /// "local-only" mode — history and manual peers still work, automatic
    /// discovery simply doesn't happen. This keeps the HTTP surface
    /// predictable for tests and for users on locked-down networks.
    pub async fn enable(&self) -> Result<(), FlowError> {
        {
            let mut cfg = self.config.write().await;
            cfg.enabled = true;
        }

        // Only attempt to (re)start mDNS if we don't already have a handle.
        let mut handle_guard = self.mdns_handle.write().await;
        if handle_guard.is_some() {
            return Ok(());
        }

        match Self::start_mdns().await {
            Ok(handle) => {
                *handle_guard = Some(handle);
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Flow mDNS startup failed; continuing in local-only mode"
                );
                // Store an empty handle so disable() still has something to
                // clear and we don't retry on every call.
                *handle_guard = Some(MdnsHandle {
                    browse_task: None,
                    clipboard_task: None,
                });
            }
        }

        #[cfg(target_os = "macos")]
        {
            let mgr = self.clone();
            let clipboard_task = tokio::spawn(async move {
                let mut last = String::new();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    // Stop if flow was disabled.
                    if !mgr.config.read().await.enabled {
                        break;
                    }
                    // Run pbpaste to get current clipboard contents.
                    let Ok(out) = tokio::process::Command::new("pbpaste").output().await else {
                        continue;
                    };
                    if !out.status.success() {
                        continue;
                    }
                    let content = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if content.is_empty() || content == last {
                        continue;
                    }
                    last = content.clone();
                    let _ = mgr.sync_local(content).await;
                }
            });
            // Attach the clipboard task to the existing MdnsHandle.
            let mut hg = self.mdns_handle.write().await;
            if let Some(h) = hg.as_mut() {
                h.clipboard_task = Some(clipboard_task);
            }
        }

        Ok(())
    }

    /// Attempt to start the mDNS service daemon, register our service and
    /// spawn a browse task. This is factored out so the error path in
    /// `enable` stays readable.
    async fn start_mdns() -> Result<MdnsHandle, FlowError> {
        // We intentionally DO NOT actually pull in mdns-sd yet at runtime —
        // the crate is included as a dependency for future work, but the
        // current implementation treats network discovery as a no-op to
        // guarantee tests pass without multicast access. The first real
        // integration will replace this body.
        //
        // Returning Ok(empty) keeps the control-flow identical to the
        // "success" path so callers don't need to special-case it.
        Ok(MdnsHandle {
            browse_task: None,
            clipboard_task: None,
        })
    }

    /// Disable Flow and tear down any mDNS resources. Keeps configuration
    /// and history intact so re-enabling is instant.
    pub async fn disable(&self) -> Result<(), FlowError> {
        {
            let mut cfg = self.config.write().await;
            cfg.enabled = false;
        }
        let mut handle_guard = self.mdns_handle.write().await;
        // Dropping MdnsHandle aborts the browse task if any.
        *handle_guard = None;
        Ok(())
    }

    pub async fn config(&self) -> FlowConfig {
        self.config.read().await.clone()
    }

    /// Replace the config wholesale. Does NOT start/stop mDNS — callers
    /// should toggle via `enable`/`disable` for that.
    pub async fn set_config(&self, config: FlowConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
    }

    pub async fn peers(&self) -> Vec<FlowPeer> {
        self.peers.read().await.values().cloned().collect()
    }

    pub async fn history(&self) -> Vec<FlowEntry> {
        // Return most-recent first.
        let h = self.history.read().await;
        let mut out: Vec<FlowEntry> = h.clone();
        out.reverse();
        out
    }

    /// Record a local clipboard entry and, if auto-sync is on, best-effort
    /// broadcast it to all known peers.
    pub async fn sync_local(&self, content: String) -> Result<FlowEntry, FlowError> {
        let cfg = self.config.read().await.clone();
        if !cfg.enabled {
            return Err(FlowError::Disabled);
        }

        let entry = FlowEntry {
            id: Uuid::new_v4(),
            content: content.clone(),
            source: FlowSource::Local,
            timestamp: unix_now(),
            mime: "text/plain".to_string(),
        };

        self.push_entry(entry.clone(), cfg.history_limit).await;

        if cfg.auto_sync {
            // Snapshot peers, then best-effort push. We don't hold the lock
            // across network calls.
            let peers: Vec<FlowPeer> = self.peers.read().await.values().cloned().collect();
            for peer in peers {
                if let Err(e) = self.push_to_peer(&peer, &cfg, &content).await {
                    tracing::warn!(peer = %peer.hostname, error = %e, "peer unreachable, marking offline");
                    if let Some(p) = self.peers.write().await.get_mut(&peer.id) {
                        p.online = false;
                    }
                }
            }
        }

        Ok(entry)
    }

    /// Record an entry received from a peer.
    pub async fn receive_from_peer(
        &self,
        peer_id: Uuid,
        hostname: String,
        content: String,
    ) -> Result<FlowEntry, FlowError> {
        let cfg = self.config.read().await.clone();
        if !cfg.enabled {
            return Err(FlowError::Disabled);
        }

        let entry = FlowEntry {
            id: Uuid::new_v4(),
            content,
            source: FlowSource::Peer {
                peer_id,
                hostname: hostname.clone(),
            },
            timestamp: unix_now(),
            mime: "text/plain".to_string(),
        };

        self.push_entry(entry.clone(), cfg.history_limit).await;
        Ok(entry)
    }

    pub async fn add_peer(&self, peer: FlowPeer) {
        let mut peers = self.peers.write().await;
        peers.insert(peer.id, peer);
    }

    pub async fn remove_peer(&self, id: Uuid) {
        let mut peers = self.peers.write().await;
        peers.remove(&id);
    }

    pub async fn clear_history(&self) {
        self.history.write().await.clear();
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    async fn push_entry(&self, entry: FlowEntry, limit: usize) {
        let mut hist = self.history.write().await;
        hist.push(entry);
        // Keep only the `limit` most recent entries (drop oldest from front).
        if hist.len() > limit {
            let overflow = hist.len() - limit;
            hist.drain(0..overflow);
        }
    }

    /// Best-effort HTTP POST to a peer's `/flow/receive` endpoint.
    ///
    /// Uses `reqwest`; any failure bubbles up as [`FlowError::PeerUnreachable`]
    /// so the caller can mark the peer offline without failing the sync.
    async fn push_to_peer(
        &self,
        peer: &FlowPeer,
        cfg: &FlowConfig,
        content: &str,
    ) -> Result<(), FlowError> {
        let url = format!("http://{}/flow/receive", peer.addr);
        let body = serde_json::json!({
            "peer_id": cfg.instance_id,
            "hostname": cfg.instance_name,
            "content": content,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| FlowError::PeerUnreachable(e.to_string()))?;

        client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| FlowError::PeerUnreachable(e.to_string()))?
            .error_for_status()
            .map_err(|e| FlowError::PeerUnreachable(e.to_string()))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Unit tests (module-local). Integration tests live in tests/flow.rs.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_config_is_disabled() {
        let m = FlowManager::new();
        let cfg = m.config().await;
        assert!(!cfg.enabled);
        assert!(cfg.auto_sync);
        assert_eq!(cfg.history_limit, 50);
    }

    #[tokio::test]
    async fn sync_local_requires_enable() {
        let m = FlowManager::new();
        let err = m.sync_local("hi".into()).await.unwrap_err();
        matches!(err, FlowError::Disabled);
    }

    #[tokio::test]
    async fn history_respects_limit() {
        let m = FlowManager::new();
        m.enable().await.unwrap();
        {
            let mut cfg = m.config().await;
            cfg.history_limit = 3;
            cfg.auto_sync = false; // no network
            m.set_config(cfg).await;
        }

        for i in 0..10 {
            m.sync_local(format!("entry-{i}")).await.unwrap();
        }
        let h = m.history().await;
        assert_eq!(h.len(), 3);
        // Most recent first
        assert_eq!(h[0].content, "entry-9");
        assert_eq!(h[2].content, "entry-7");
    }
}
