# WebSocket Events

The daemon exposes a single WebSocket endpoint that pushes state changes to
subscribed clients. It is a one-shot broadcast bus: clients attach after a
successful `/health` call, listen, and react.

## 1. Overview

| Property | Value |
|----------|-------|
| Transport | WebSocket over HTTP |
| Endpoint | `ws://127.0.0.1:7331/ws` |
| Bus | `tokio::sync::broadcast::Sender<DaemonEvent>` with capacity 64 |
| Direction | Daemon → client (with a `ping`/`pong` liveness exchange) |
| Encoding | Text frames, JSON |

The bus is created in `AppState::new` and cloned into every route that needs
to emit. Handlers call `state.events.send(DaemonEvent::…)` and ignore the
`Result` (no subscribers is not an error).

Source: `state.rs:73,86`, `ws.rs:57-96`.

## 2. Connection

### 2.1 URL and handshake

The app client (`apps/hhkb-app/src/hhkb/daemonClient.ts`) connects to:

```
ws://localhost:7331/ws
```

Axum's `WebSocketUpgrade` handles the HTTP 101 handshake. There is no
subprotocol, no auth header, no cookie.

### 2.2 Liveness

- Client may send a `ping` text frame.
- Daemon replies with a `pong` text frame.
- Any send failure (dead socket) terminates the handler loop.

Source: `ws.rs:61-95`.

### 2.3 Reconnection (client side)

`DaemonWebSocket` in `daemonClient.ts` implements a **fixed 5000 ms
retry**, not exponential backoff:

| Event | Action |
|-------|--------|
| `connect()` | open socket, set `shouldReconnect=true` |
| `onclose` | if `shouldReconnect`, schedule `setTimeout(…, 5000)` |
| `onerror` | no-op; `onclose` drives retry |
| `disconnect()` | clear timer, close socket, set `shouldReconnect=false` |

Malformed JSON frames are silently dropped.

Source: `daemonClient.ts:282-349`.

## 3. Event envelope

Every message is a JSON object using serde's internally-tagged form:

```json
{ "type": "<variant>", ...payload }
```

The `DaemonEvent` enum uses `#[serde(tag = "type", rename_all =
"snake_case")]`, with some variants overriding the tag via
`#[serde(rename = "…")]`. Field names use snake_case.

Source: `ws.rs:22-55`.

## 4. Event catalog

| Type | Payload | Emitted when | Typical consumer action |
|------|---------|--------------|-------------------------|
| `device_connected` | — | `AppState::try_reconnect` successfully opens the HID device | Refresh `/device/info`, flip connected badge |
| `device_disconnected` | — | (emit path reserved; currently no caller sends this) | Flip connected badge |
| `profile_changed` | `{ "id": string }` | `PUT /profiles/:id` and `POST /profiles/active` | Refresh profile list / active highlight |
| `kanata_started` | `{ "pid": u32 }` | `POST /kanata/start` succeeds | Show running indicator |
| `kanata_stopped` | — | `POST /kanata/stop` succeeds | Show stopped indicator |
| `kanata_reloaded` | `{ "profile_id": string }` | `POST /kanata/reload` (empty `profile_id`) or `POST /profiles/active` when running | Toast "config reloaded" |
| `flow_peer_discovered` | `{ "peer": FlowPeer }` | `POST /flow/peers` | Add to peer list |
| `flow_peer_lost` | `{ "peer_id": UUID }` | `DELETE /flow/peers/:id` | Remove from peer list |
| `flow_synced` | `{ "entry_id": UUID, "source": FlowSource }` | `POST /flow/sync` or `POST /flow/receive` | Refresh clipboard history |
| `flow_enabled` | — | `POST /flow/enable` | Enable Flow panel |
| `flow_disabled` | — | `POST /flow/disable` | Disable Flow panel |
| `bluetooth_scan_complete` | `{ "devices": BleDevice[] }` | 4-second BLE refresh triggered by `POST /device/bluetooth/scan` finishes | Refresh scanned device list, then re-query `/device/bluetooth` |

`device_disconnected` is defined on the enum but no code path currently
sends it; the daemon detects "device gone" lazily on the next `with_device`
call and surfaces it as `DeviceUnavailable` on the REST side. Consumers
should still handle the variant in case a future change emits it.

Source: `ws.rs:22-55`, `state.rs:125-147`, `routes/profile.rs:191-194,278-300`,
`routes/kanata.rs:63-88`, `routes/flow.rs:82-172`.

### 4.1 Example frames

```json
{ "type": "device_connected" }
```

```json
{ "type": "profile_changed", "id": "a3f2…" }
```

```json
{ "type": "kanata_started", "pid": 54231 }
```

```json
{
  "type": "flow_peer_discovered",
  "peer": {
    "id": "9b1a…",
    "hostname": "desk-mac",
    "addr": "192.168.1.42:7331",
    "last_seen": 1728566400,
    "online": true
  }
}
```

```json
{
  "type": "flow_synced",
  "entry_id": "c4d5…",
  "source": { "type": "local" }
}
```

```json
{
  "type": "bluetooth_scan_complete",
  "devices": [
    { "id": "E0:71:2C:05:3A:BB", "name": "HHKB-Hybrid_1", "address": "E0:71:2C:05:3A:BB", "battery": 100, "connected": false, "rssi": -65 }
  ]
}
```

## 5. Ordering and delivery

- **At-most-once.** The bus uses `tokio::sync::broadcast` with capacity 64.
  If a client is slower than the producer the channel lags and the receive
  loop observes `Err(Lagged)` → the handler breaks out of its `select!` and
  the socket is closed. The client must reconnect and re-query state.
- **No replay on reconnect.** There is no event log, no sequence number, no
  `Last-Event-ID` equivalent. Re-opening a WS gives only future events.
- **No sequencing.** Events are not timestamped; clients that need order
  must rely on the `profile_changed.id`, `kanata_started.pid`, etc. payloads.
- **No fan-out fairness.** Each subscriber has its own cursor; a slow
  subscriber only hurts itself.

Source: `state.rs:73`, `ws.rs:80-93`.

## Source footnotes

- `crates/hhkb-daemon/src/ws.rs`
- `crates/hhkb-daemon/src/state.rs`
- `crates/hhkb-daemon/src/routes/profile.rs`
- `crates/hhkb-daemon/src/routes/kanata.rs`
- `crates/hhkb-daemon/src/routes/flow.rs`
- `crates/hhkb-daemon/src/routes/bluetooth.rs`
- `apps/hhkb-app/src/hhkb/daemonClient.ts`
