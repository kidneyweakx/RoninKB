# Daemon HTTP REST Surface

The RoninKB daemon exposes a single REST API over HTTP plus a WebSocket
upgrade at `/ws`. This document is the authoritative endpoint inventory.

## 1. Overview

- **Trust model:** LAN-trust, localhost bind. No auth, no TLS.
- **Audience:** the React app (`apps/hhkb-app`), the CLI (`hhkb-cli`), and
  peer daemons on the same host for Flow peer delivery.
- **Stability:** `v0.1.0`, unversioned URLs.

Source: `main.rs:52`, `Cargo.toml:3`, `lib.rs:27-105`.

## 2. Base URL

```
http://127.0.0.1:7331
```

The bind is hard-coded in `main.rs`:

```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:7331").await?;
```

Source: `main.rs:52`, `main.rs:75`.

## 3. Global conventions

| Concern | Value |
|---------|-------|
| Request `Content-Type` | `application/json` for `POST`/`PUT` with body |
| Response `Content-Type` | `application/json` (except embedded UI `/ui/*`) |
| Query string | `application/x-www-form-urlencoded`, parsed by axum `Query` |
| Character set | UTF-8 |
| Trailing slashes | Not normalised (`/ui` and `/ui/` are distinct handlers) |
| Logging | `tower_http::trace::TraceLayer` on every route |

### 3.1 Error response shape

All `ApiError` variants serialise via a single `IntoResponse` impl:

```json
{
  "error": "<short_code>",
  "message": "<human readable>"
}
```

Status mapping from `error.rs:66-103`:

| Variant | Status | `error` |
|---------|--------|---------|
| `DeviceUnavailable` | 503 | `device_unavailable` |
| `Device(_)` | 500 | `device_error` |
| `NotFound` | 404 | `not_found` |
| `Db(_)` | 500 | `db_error` |
| `Json(_)` | 400 | `bad_json` |
| `BadRequest(_)` | 400 | `bad_request` |
| `InvalidConfig(_)` | 400 | `invalid_config` |
| `Internal(_)` | 500 | `internal_error` |
| `KanataNotInstalled` | 503 | `kanata_not_installed` |
| `KanataAlreadyRunning` | 409 | `kanata_already_running` |
| `KanataNotRunning` | 409 | `kanata_not_running` |
| `KanataIo(_)` | 500 | `kanata_io_error` |
| `Flow(Disabled)` | 503 | `flow_disabled` |
| `Flow(Mdns)` | 500 | `flow_mdns_error` |
| `Flow(PeerUnreachable)` | 500 | `flow_peer_unreachable` |

## 4. Endpoint inventory

Derived from `crates/hhkb-daemon/src/lib.rs:35-90`.

### 4.1 Health

| Method | Path | Purpose | Request | Response | Errors |
|--------|------|---------|---------|----------|--------|
| GET | `/health` | Liveness + version + device flag | — | `{ status, version, device_connected }` | — |

### 4.2 Device

| Method | Path | Purpose | Request | Response | Errors |
|--------|------|---------|---------|----------|--------|
| GET | `/device/info` | Firmware, type number, serial | — | `DeviceInfoDto` | `device_unavailable`, `device_error` |
| GET | `/device/mode` | Current keyboard mode | — | `{ mode, raw }` | `device_unavailable`, `device_error` |
| ~~PUT~~ | ~~`/device/mode`~~ | **Not implemented** — HHKB keyboard mode is set exclusively by physical DIP switches (SW1/SW2); there is no firmware command to change it from software. This endpoint is intentionally absent. | — | — | — |
| GET | `/device/dipsw` | DIP switch state | — | `{ switches: [6]bool }` | `device_unavailable` |
| GET | `/device/connected` | Reconnect hint + bool | — | `{ connected: bool }` | — |
| GET | `/device/bluetooth` | OS-managed BLE connection state + metadata | — | `BluetoothStatusDto` | — |
| POST | `/device/bluetooth/scan` | Start 4-second BLE refresh in background | — | `{ scanning: bool, message: string }` | — |
| GET | `/device/bluetooth/devices` | List scanned HHKB candidates + scan status | — | `{ available: bool, scanning: bool, devices: BleDevice[] }` | — |
| GET | `/device/bluetooth/system` | List host OS connected Bluetooth devices (macOS) | — | `{ available, source, devices, message? }` | — |
| GET | `/device/keymap` | Read 128-byte keymap | `?mode=mac&fn_layer=false` | `{ mode, fn_layer, data: [128]u8 }` | `bad_request`, `device_error` |
| PUT | `/device/keymap` | Write 128-byte keymap | `?mode=mac&fn_layer=false`, body `{ data: u8[128] }` | `{ status: "ok" }` | `bad_request` (len != 128) |

**`BleDevice` shape** (used by `/device/bluetooth/devices`):

```json
{
  "id": "string",
  "name": "HHKB-Hybrid_1",
  "address": "E0:71:2C:05:3A:BB",
  "battery": 100,
  "connected": false,
  "rssi": -72
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Opaque device identifier |
| `name` | `string?` | Human-readable BLE device name |
| `address` | `string` | Bluetooth MAC address |
| `battery` | `number?` | Battery percentage 0–100 |
| `connected` | `boolean` | Whether this device is currently connected |
| `rssi` | `number?` | Signal strength in dBm |

### 4.3 Profiles

| Method | Path | Purpose | Request | Response | Errors |
|--------|------|---------|---------|----------|--------|
| GET | `/profiles` | List all profiles | — | `{ profiles: ProfileRecord[] }` | `db_error` |
| POST | `/profiles` | Create profile | `ViaProfile` | `ProfileRecord` | `invalid_config`, `db_error` |
| GET | `/profiles/:id` | Fetch one | — | `ProfileRecord` | `not_found`, `db_error` |
| PUT | `/profiles/:id` | Update | `ViaProfile` | `ProfileRecord` | `invalid_config`, `not_found` |
| DELETE | `/profiles/:id` | Delete | — | `{ status: "ok" }` | `db_error` |
| GET | `/profiles/active` | Active profile id | — | `{ id: string? }` | `db_error` |
| POST | `/profiles/active` | Switch active | `{ id }` | `{ id }` | `not_found`, `kanata_*`, `internal_error` |

### 4.4 Kanata

| Method | Path | Purpose | Request | Response | Errors |
|--------|------|---------|---------|----------|--------|
| GET | `/kanata/status` | Binary + live state | — | `{ installed, config_path, state, pid? }` | — |
| POST | `/kanata/start` | Spawn child | — | `{ pid }` | `kanata_not_installed`, `kanata_already_running` |
| POST | `/kanata/stop` | Terminate child | — | `{ status: "stopped" }` | `kanata_not_running` |
| POST | `/kanata/reload` | Write cfg + SIGUSR1 | `{ config }` | `{ status: "reloaded" }` | `kanata_io_error`, `internal_error` |
| GET | `/kanata/config` | Read current on-disk cfg | — | `{ config, path }` | `kanata_io_error` |

### 4.5 Flow

See `flow.md` for wire formats.

| Method | Path | Purpose | Request | Response | Errors |
|--------|------|---------|---------|----------|--------|
| GET | `/flow/config` | Current Flow config | — | `FlowConfig` | — |
| PUT | `/flow/config` | Replace config | `FlowConfig` | `FlowConfig` | — |
| POST | `/flow/enable` | Turn Flow on | — | `{ status: "enabled" }` | `flow_mdns_error` |
| POST | `/flow/disable` | Turn Flow off | — | `{ status: "disabled" }` | — |
| GET | `/flow/peers` | List known peers | — | `{ peers }` | — |
| POST | `/flow/peers` | Add manual peer | `{ hostname, addr }` | `FlowPeer` | `bad_request` |
| DELETE | `/flow/peers/:id` | Remove peer | — | `{ status: "removed" }` | — |
| GET | `/flow/history` | Clipboard history | — | `{ entries }` | — |
| DELETE | `/flow/history` | Clear history | — | `{ status: "cleared" }` | — |
| POST | `/flow/sync` | Push local clipboard | `{ content }` | `{ entry }` | `flow_disabled`, `flow_peer_unreachable` |
| POST | `/flow/receive` | Inbound from peer | `{ peer_id, hostname, content }` | `{ entry }` | `flow_disabled` |

### 4.6 WebSocket + embedded UI

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/ws` | WebSocket upgrade (see `websocket-events.md`) |
| GET | `/` | 307 redirect to `/ui/` (only when built with `embedded-ui`) |
| GET | `/ui`, `/ui/`, `/ui/*path` | Serve baked-in React bundle (only with `embedded-ui`) |

Source: `lib.rs:88-100`, `ui.rs:23-59`.

## 5. Per-endpoint detail

### 5.1 `GET /device/keymap`

```bash
curl "http://127.0.0.1:7331/device/keymap?mode=mac&fn_layer=false"
```

```json
{
  "mode": "mac",
  "fn_layer": false,
  "data": [ /* 128 bytes */ ]
}
```

- Query defaults: `mode=mac`, `fn_layer=false`.
- `mode` values: `hhk`, `mac`, `lite`, `secret` (case-insensitive).
- Opens a HID session, reads, closes session.

Source: `routes/keymap.rs:49-71`.

### 5.2 `PUT /device/keymap`

```bash
curl -X PUT \
  "http://127.0.0.1:7331/device/keymap?mode=mac&fn_layer=false" \
  -H 'Content-Type: application/json' \
  -d '{ "data": [0,0,0, /* … 128 total … */] }'
```

The body must contain exactly **128 bytes**. Any other length returns
`400 bad_request` with message `keymap must be exactly 128 bytes, got N`.

Source: `routes/keymap.rs:73-102`.

### 5.3 `POST /profiles`

```bash
curl -X POST http://127.0.0.1:7331/profiles \
  -H 'Content-Type: application/json' \
  -d @profile.json
```

Body is a full `ViaProfile` JSON document. If the profile contains a
software config (`via.ronin.software.config`), the daemon runs a minimal
kanata sanity check:

- No NUL bytes.
- Balanced parentheses (strings and `;` line comments respected).
- Contains at least one `(defsrc …)` or `(defcfg …)` top-level form.

Validation failure returns `400 invalid_config`.

Source: `routes/profile.rs:41-116,172-180`.

### 5.4 `POST /profiles/active`

```bash
curl -X POST http://127.0.0.1:7331/profiles/active \
  -H 'Content-Type: application/json' \
  -d '{ "id": "9f8a…" }'
```

Response:

```json
{ "id": "9f8a…" }
```

Side effects:

1. Persist active id in SQLite.
2. Fetch profile; extract `via.ronin.software.config` when `engine == "kanata"`.
3. If kanata is running → `reload(cfg)` (SIGUSR1 on Unix, restart on Windows).
4. Else → `write_config(cfg)` (on-disk only; next `start()` picks it up).
5. Broadcast `profile_changed { id }`; broadcast `kanata_reloaded { profile_id: id }` when a live reload succeeded.

Source: `routes/profile.rs:260-311`.

### 5.5 `POST /kanata/reload`

```bash
curl -X POST http://127.0.0.1:7331/kanata/reload \
  -H 'Content-Type: application/json' \
  -d '{ "config": "(defsrc a)\n(deflayer base a)\n" }'
```

Writes the payload to the active `.kbd` path, then signals the running
child. Broadcasts `kanata_reloaded { profile_id: "" }` — profile-triggered
reloads use a filled `profile_id` instead.

Source: `routes/kanata.rs:75-89`.

### 5.6 `POST /flow/sync`

```bash
curl -X POST http://127.0.0.1:7331/flow/sync \
  -H 'Content-Type: application/json' \
  -d '{ "content": "hello from my laptop" }'
```

- Records a `FlowEntry` with `source=local`.
- If `auto_sync` is on, POSTs `/flow/receive` to every known peer with a 3-second timeout.
- Failed peers are marked `online=false` but the local entry is still
  returned successfully.
- Returns `503 flow_disabled` when Flow is off.

Source: `routes/flow.rs:148-158`, `flow.rs:261-292`.

### 5.7 `GET /device/bluetooth`

Returns the connection state and available metadata for the HHKB keyboard when
connected over Bluetooth (BLE). Pairing, connecting, and profile switching are
owned by the operating system; the RoninKB daemon only reports the currently
connected HHKB it can observe via its BLE backend and always returns HTTP 200.
Callers must inspect the `connected` field to determine whether a keyboard is
present.

Source: `routes/bluetooth.rs`.

```bash
curl http://127.0.0.1:7331/device/bluetooth
```

#### Response

```json
{
  "available": true,
  "connected": true,
  "name": "HHKB-Hybrid_1",
  "address": "E0:71:2C:05:3A:BB",
  "battery": 100,
  "rssi": -65
}
```

| Field | Type | Description |
|-------|------|-------------|
| `available` | `bool` | Whether a BLE adapter is present on this host |
| `connected` | `bool` | Whether an HHKB keyboard is currently connected via Bluetooth at the OS level |
| `name` | `string \| null` | Bluetooth device name (e.g. `"HHKB-Hybrid_1"`) |
| `address` | `string \| null` | Bluetooth address when available; may be privacy-redacted on macOS |
| `battery` | `number \| null` | Battery percentage 0–100, `null` when unavailable |
| `rssi` | `number \| null` | Signal strength in dBm (negative value), `null` when unavailable |

When `connected` is `false`, `name`, `address`, `battery`, and `rssi` are `null`.

#### Errors

This endpoint does not return errors. Failures in the underlying
BLE adapter query are handled gracefully by returning `{ "connected": false }`
or `{ "available": false }`.

### 5.8 `GET /device/bluetooth/system`

Returns the host OS Bluetooth connected-device list for UI visibility.

- **macOS**: sourced from `system_profiler SPBluetoothDataType -json`.
- **other OSes**: returns `source: "unsupported"` and `devices: []`.
- **BLE unavailable**: returns `available: false`, `source: "none"`, and `devices: []`.

#### Response

```json
{
  "available": true,
  "source": "system_profiler",
  "devices": [
    {
      "name": "HHKB-Hybrid_1",
      "address": "E0:71:2C:05:3A:BB",
      "kind": "Keyboard",
      "battery": 100,
      "services": "0x400000 < BLE >"
    }
  ],
  "message": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `available` | `bool` | Whether BLE is available on this host |
| `source` | `string` | Device-list backend (`system_profiler`, `unsupported`, `none`) |
| `devices` | `array` | Currently connected host Bluetooth devices |
| `message` | `string \| null` | Optional diagnostic text when list is empty/fallback |

## 6. Authentication

**None.** The daemon binds only to `127.0.0.1` and relies on OS-level
loopback isolation. There is no token, cookie, basic auth, or mTLS path.
Any local process running as the user can call every endpoint.

Source: `main.rs:52,75`.

## 7. CORS

`lib.rs:30-33` installs `tower-http`'s `CorsLayer` with maximally
permissive settings:

```rust
let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);
```

This is intentional — the WebHID frontend runs on arbitrary dev-server
ports (Vite default `5173`) and credentials-less browser requests would
otherwise be rejected. Combined with the localhost bind, the policy
does not expand the trust surface beyond "processes on this machine".

Source: `lib.rs:30-33`.

## 8. Versioning

- Current version: `v0.1.0` (from `Cargo.toml:3` and reported by `/health`).
- **Paths are unversioned.** There is no `/v1/` prefix; breaking changes
  will require a coordinated bump of daemon + app + CLI.
- The `/health` response includes `version` so clients can feature-gate.

Source: `Cargo.toml:3`, `routes/health.rs:8-22`.

## Source footnotes

- `crates/hhkb-daemon/src/lib.rs`
- `crates/hhkb-daemon/src/main.rs`
- `crates/hhkb-daemon/src/error.rs`
- `crates/hhkb-daemon/src/routes/health.rs`
- `crates/hhkb-daemon/src/routes/device.rs`
- `crates/hhkb-daemon/src/routes/keymap.rs`
- `crates/hhkb-daemon/src/routes/profile.rs`
- `crates/hhkb-daemon/src/routes/kanata.rs`
- `crates/hhkb-daemon/src/routes/flow.rs`
- `crates/hhkb-daemon/src/routes/bluetooth.rs`
- `crates/hhkb-daemon/src/ui.rs`
