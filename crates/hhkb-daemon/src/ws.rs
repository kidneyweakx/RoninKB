//! `GET /ws` — WebSocket event stream.
//!
//! Clients subscribe to a broadcast channel of [`DaemonEvent`]s. Events are
//! emitted when:
//!
//! - A device connects or disconnects
//! - A profile is created / updated / activated
//!
//! Clients may also send a `ping` text frame; the daemon replies `pong`.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonEvent {
    DeviceConnected,
    DeviceDisconnected,
    ProfileChanged {
        id: String,
    },
    KanataStarted {
        pid: u32,
    },
    KanataStopped,
    KanataReloaded {
        profile_id: String,
    },
    // -- Flow (cross-device clipboard sync) --------------------------------
    #[serde(rename = "flow_peer_discovered")]
    FlowPeerDiscovered {
        peer: crate::flow::FlowPeer,
    },
    #[serde(rename = "flow_peer_lost")]
    FlowPeerLost {
        peer_id: uuid::Uuid,
    },
    #[serde(rename = "flow_synced")]
    FlowSynced {
        entry_id: uuid::Uuid,
        source: crate::flow::FlowSource,
    },
    #[serde(rename = "flow_enabled")]
    FlowEnabled,
    #[serde(rename = "flow_disabled")]
    FlowDisabled,
    // -- Bluetooth (BLE via btleplug) ----------------------------------------
    BluetoothScanComplete {
        devices: Vec<crate::ble::BleDeviceInfo>,
    },
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.events.subscribe();

    loop {
        tokio::select! {
            maybe_msg = socket.recv() => {
                match maybe_msg {
                    Some(Ok(Message::Text(t)))
                        if t.trim() == "ping"
                            && socket.send(Message::Text("pong".to_string())).await.is_err() =>
                    {
                        break;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            evt = rx.recv() => {
                match evt {
                    Ok(event) => {
                        let json = match serde_json::to_string(&event) {
                            Ok(j) => j,
                            Err(_) => continue,
                        };
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
}
