# Native Bridge Protocol (`roninkb://`)

The IPC contract between the Swift native shell, the WKWebView (React UI),
and the Rust runtime when RoninKB runs as a single-process macOS app.

This spec is paired with [`rfc-0002-native-app-shell.md`](../../docs/rfc-0002-native-app-shell.md)
and [`v0.3.0-plan.md`](../../docs/v0.3.0-plan.md). Daemon-mode HTTP at
`127.0.0.1:7331` is unchanged and remains authoritative in
[`daemon-http.md`](daemon-http.md).

## 1. Overview

- **Transport (UI ↔ Swift):** custom URL scheme `roninkb://` handled by a
  `WKURLSchemeHandler` registered on the WebView's `WKWebViewConfiguration`.
- **Transport (Swift ↔ Rust):** UniFFI-generated bindings around a single
  Rust trait, `Coordinator`. No JSON crosses this boundary — UniFFI
  marshals typed values directly.
- **Push events (Rust → UI):** Rust holds a Swift-implemented
  `EventHandler` callback (UniFFI callback interface). Swift translates
  each event to JSON and pushes it into the WebView via
  `evaluateJavaScript("window.__roninkb_dispatch(...)")`.
- **Trust model:** in-process. There is no auth between layers; the OS
  enforces process isolation. The scheme is registered with
  `nonPersistentDataStore` so other Web content cannot reach it.

## 2. The `Coordinator` interface

Defined in `crates/hhkb-runtime/src/coordinator.rs`. UniFFI exposes the
Rust trait to Swift as a protocol of the same name.

```rust
#[uniffi::export]
pub trait Coordinator: Send + Sync {
    /// Construct a new coordinator. Idempotent; safe to call again
    /// after `stop()`.
    #[uniffi::constructor]
    fn new(config: CoordinatorConfig) -> Arc<Self>;

    /// Start the runtime. Opens the device, opens SQLite, registers the
    /// backend registry, probes BLE, starts the kanata supervisor in
    /// stopped state. Acquires the runtime lock file. Returns
    /// `RuntimeError::AlreadyRunning` if another instance holds the
    /// lock.
    fn start(&self, events: Arc<dyn EventHandler>) -> Result<(), RuntimeError>;

    /// Stop the runtime. Drains events, tears down backends, closes
    /// SQLite, releases the runtime lock file. Safe to call multiple
    /// times.
    fn stop(&self) -> Result<(), RuntimeError>;

    /// Send a single typed request, get a single typed response. This
    /// is the only request/response surface; every UI action maps to
    /// one `send` call.
    fn send(&self, request: Request) -> Result<Response, RequestError>;

    /// Snapshot of runtime state for diagnostics. Cheap (no I/O).
    fn get_state(&self) -> CoordinatorState;
}

#[uniffi::export(callback_interface)]
pub trait EventHandler: Send + Sync {
    /// A runtime event was emitted. Implementations must not block;
    /// they should enqueue and return. Swift's impl marshals to JSON
    /// and posts to the WebView.
    fn on_event(&self, event: Event);

    /// A fatal error occurred (panic, lock contention, BLE crash).
    /// Swift's impl shows an alert and offers Quit.
    fn on_failure(&self, error: RuntimeError);

    /// Tracing log line (DEBUG and above). Swift's impl forwards to
    /// `os_log`.
    fn on_log(&self, level: LogLevel, target: String, message: String);
}
```

`CoordinatorConfig`:

```rust
#[derive(uniffi::Record)]
pub struct CoordinatorConfig {
    /// Application Support directory root. macOS:
    /// `~/Library/Application Support/RoninKB`. Daemon-mode:
    /// `directories::ProjectDirs::from("", "", "roninKB").data_dir()`.
    pub data_dir: String,
    /// Path to the bundled kanata binary, if any.
    pub kanata_binary: Option<String>,
    /// Whether to auto-reconnect to a missing device. App: true.
    /// Tests: false.
    pub auto_reconnect: bool,
    /// Whether to attempt the runtime lock acquisition. Daemon mode in
    /// scripted tests can pass false; production paths must pass true.
    pub acquire_lock: bool,
}
```

## 3. The `Request` / `Response` types

Every endpoint exposed today via HTTP becomes one variant of `Request`
and one variant of `Response`. The mapping is mechanical and 1:1 with
[`daemon-http.md`](daemon-http.md). The runtime carries the same DTOs;
only the envelope changes.

```rust
#[derive(uniffi::Enum)]
pub enum Request {
    Health,

    DeviceInfo,
    DeviceMode,
    DeviceDipsw,
    DeviceConnected,
    DeviceKeymap,
    DeviceKeymapPut { keymap: ViaProfile },

    DeviceBluetooth,
    DeviceBluetoothScan,
    DeviceBluetoothDevices,
    DeviceBluetoothSystem,

    ProfilesList,
    ProfilesCreate { profile: ProfileCreate },
    ProfilesGet { id: String },
    ProfilesUpdate { id: String, profile: ProfileUpdate },
    ProfilesDelete { id: String },
    ProfilesActive,
    ProfilesSetActive { id: String },

    BackendList,
    BackendStatus,
    BackendSelect { id: BackendId },

    KanataStatus,
    KanataStart,
    KanataStop,
    KanataReload,
    KanataConfig,
    KanataReveal,
    KanataDriverActivate,
    KanataDriverOpenSettings,

    FlowConfig,
    FlowConfigPut { config: FlowConfig },
    FlowEnable,
    FlowDisable,
    FlowPeersList,
    FlowPeersAdd { peer: FlowPeer },
    FlowPeersRemove { id: String },
    FlowHistory,
    FlowHistoryClear,
    FlowSync { entry: FlowEntry },
    FlowReceive { envelope: FlowEnvelope },

    // Native-app only — has no daemon-mode HTTP equivalent.
    Permissions,
    OpenSettingsUrl { url: String },
}

#[derive(uniffi::Enum)]
pub enum Response {
    Empty,
    Json { value: String }, // serde_json::Value as string for UniFFI
    DeviceInfo { info: DeviceInfo },
    Keymap { keymap: ViaProfile },
    ProfileList { items: Vec<ProfileMeta> },
    Profile { profile: Profile },
    BackendList { items: Vec<BackendInfo> },
    BackendStatus { status: BackendStatusSnapshot },
    Permissions { snapshot: PermissionsSnapshot },
    KanataStatus { status: KanataStatus },
    FlowConfig { config: FlowConfig },
    FlowPeers { items: Vec<FlowPeer> },
    FlowHistory { items: Vec<FlowEntry> },
    // …one variant per response shape used by the React UI.
}
```

### 3.1 Why one `Request` enum, not many methods

UniFFI methods cost ~1 line of generated Swift per method per language.
With ~40 endpoints, that's 40 methods on a trait. A single `send`
method with a typed enum is:

- Easier to evolve — adding an endpoint touches one Rust file, one
  `Request` variant, one `Response` variant, and the URL-scheme router.
- Easier to log — every IPC call hits one `tracing::span!` in `send`.
- Closer to today's HTTP shape — the React UI just translates
  `(method, path, body)` into a `Request` variant; no per-endpoint
  binding code in TS.

## 4. Error model

`RuntimeError` covers process-level failures (lock contention, fatal
init, panic relays). `RequestError` covers per-call failures.

```rust
#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum RuntimeError {
    #[error("another RoninKB instance is running (pid {pid})")]
    AlreadyRunning { pid: u32, since_unix_seconds: u64 },
    #[error("failed to open data dir: {message}")]
    DataDir { message: String },
    #[error("failed to open SQLite: {message}")]
    Db { message: String },
    #[error("fatal panic: {message}")]
    Panic { message: String },
    #[error("not started")]
    NotStarted,
}

#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum RequestError {
    #[error("device unavailable")]
    DeviceUnavailable,
    #[error("device error: {message}")]
    Device { message: String },
    #[error("not found")]
    NotFound,
    #[error("bad request: {message}")]
    BadRequest { message: String },
    #[error("backend inactive: {message}")]
    BackendInactive { message: String },
    #[error("internal error: {message}")]
    Internal { message: String },
}
```

The mapping `RuntimeError ↔ ApiError` and `RequestError ↔ ApiError` is
1:1 with the HTTP status codes already documented in
[`daemon-http.md`](daemon-http.md) §3.1. Daemon mode keeps that mapping;
the native app surfaces the same codes through the URL scheme handler.

## 5. Event push contract

The event payload is the same `DaemonEvent` enum the WebSocket emits in
daemon mode. See [`websocket-events.md`](websocket-events.md) for the
canonical list and shape. This spec just covers the transport.

### 5.1 Rust side

```rust
#[derive(uniffi::Enum, Serialize, Deserialize, Clone)]
pub enum Event {
    DeviceConnected,
    DeviceDisconnected,
    KeymapChanged { source: String },
    BackendStatusChanged { id: BackendId, state: String },
    KanataStateChanged { state: String },
    FlowEntryReceived { entry: FlowEntry },
    BluetoothDevicesChanged,
    HealthTick,
    // …mirrors the existing DaemonEvent variants 1:1
}
```

Runtime calls `event_handler.on_event(event)` from a Tokio task. The
callback must return quickly; Swift's implementation enqueues a JSON
serialization onto the main queue.

### 5.2 Swift side

```swift
final class EventHandlerImpl: EventHandler {
    weak var webView: WKWebView?

    func onEvent(event: Event) {
        let json = encodeAsJsonString(event)
        DispatchQueue.main.async { [weak self] in
            self?.webView?.evaluateJavaScript(
                "window.__roninkb_dispatch(\(json))",
                completionHandler: nil
            )
        }
    }

    func onFailure(error: RuntimeError) { /* show NSAlert */ }
    func onLog(level: LogLevel, target: String, message: String) {
        os_log("%{public}@: %{public}@", log: appLog, type: level.osLogType, target, message)
    }
}
```

### 5.3 JS side

The Swift `WKUserContentController` injects, at document start, this
bootstrap script:

```js
(function () {
  window.__roninkb_native = true;
  window.__roninkb_dispatch_table = new Set();
  window.__roninkb_dispatch = function (event) {
    window.__roninkb_dispatch_table.forEach(function (fn) {
      try { fn(event); } catch (e) { console.error(e); }
    });
  };
  window.__roninkb_events = {
    subscribe: function (fn) {
      window.__roninkb_dispatch_table.add(fn);
      return function () { window.__roninkb_dispatch_table.delete(fn); };
    },
  };
})();
```

`apps/hhkb-app/src/hhkb/daemonClient.ts` uses
`window.__roninkb_events.subscribe(...)` when `window.__roninkb_native`
is true; otherwise it opens a WebSocket as today.

## 6. URL scheme: `roninkb://`

Registered exclusively on the app's `WKWebViewConfiguration` via
`setURLSchemeHandler(_:forURLScheme:)`. Not registered as a system URL
handler — other apps cannot invoke it.

### 6.1 Authority `app` — static assets

`roninkb://app/<path>` serves the React bundle from
`Bundle.main.url(forResource: "Resources/web/<path>")`.

Resolution rules (in order):

1. If `path` exists as a file under `Resources/web/`, return its bytes.
2. If `path` is empty or ends with `/`, return `Resources/web/index.html`.
3. If `path` has no extension and does not contain `.`, return
   `Resources/web/index.html` (SPA client-side route fallback).
4. Otherwise, return 404 (with a JSON error body, for symmetry with §6.2).

MIME type is guessed from extension via `UTType(filenameExtension:)`. The
React build is configured (M3) to produce relative asset URLs so that
chunk splits resolve correctly under this scheme.

Cache headers: `Cache-Control: no-store` for `index.html`,
`Cache-Control: public, max-age=31536000, immutable` for hashed assets
under `assets/`.

### 6.2 Authority `api` — runtime request/response

`roninkb://api/<endpoint>` translates to a `Request` enum variant and
calls `Coordinator::send`.

Mapping is built mechanically from `daemon-http.md`. Examples:

| `roninkb://api/...`       | HTTP method | `Request` variant            |
| ------------------------- | ----------- | ---------------------------- |
| `health`                  | GET         | `Health`                     |
| `device/info`             | GET         | `DeviceInfo`                 |
| `device/keymap`           | GET         | `DeviceKeymap`               |
| `device/keymap`           | PUT         | `DeviceKeymapPut { ... }`    |
| `profiles`                | GET         | `ProfilesList`               |
| `profiles`                | POST        | `ProfilesCreate { ... }`     |
| `profiles/{id}`           | GET         | `ProfilesGet { id }`         |
| `profiles/{id}`           | PUT         | `ProfilesUpdate { id, ... }` |
| `profiles/{id}`           | DELETE      | `ProfilesDelete { id }`      |
| `profiles/active`         | GET         | `ProfilesActive`             |
| `profiles/active`         | POST        | `ProfilesSetActive { ... }`  |
| `backend/list`            | GET         | `BackendList`                |
| `backend/status`          | GET         | `BackendStatus`              |
| `backend/select`          | POST        | `BackendSelect { ... }`      |
| `kanata/status`           | GET         | `KanataStatus`               |
| `kanata/start`            | POST        | `KanataStart`                |
| `kanata/stop`             | POST        | `KanataStop`                 |
| `kanata/reload`           | POST        | `KanataReload`               |
| `flow/config`             | GET / PUT   | `FlowConfig` / `FlowConfigPut` |
| `flow/enable`             | POST        | `FlowEnable`                 |
| `flow/disable`            | POST        | `FlowDisable`                |
| `flow/peers`              | GET / POST  | `FlowPeersList` / `FlowPeersAdd` |
| `flow/peers/{id}`         | DELETE      | `FlowPeersRemove { id }`     |
| `flow/history`            | GET / DELETE| `FlowHistory` / `FlowHistoryClear` |
| `permissions`             | GET         | `Permissions`                |
| `permissions/open`        | POST        | `OpenSettingsUrl { ... }`    |

A response is `200 OK` with body `application/json` carrying the
serialized `Response::Json { value }` for trivial cases, or the
serialized `Response::*` variant payload as JSON for typed cases. The
React UI never sees the `Response` enum directly — it receives the JSON
shape the daemon would have returned at the equivalent HTTP endpoint,
because that's what `daemonClient.ts` already parses today.

### 6.3 Why JSON in-process when UniFFI could do it typed

Two reasons:

1. **`daemonClient.ts` already parses these shapes.** Going through JSON
   means the React side does not need a separate generated typed client
   per endpoint, and we keep one source of truth for response shapes
   (the `daemon-http.md` documents and the existing Zustand stores).
2. **Tenet 6 (cross boundaries intentionally).** A typed UniFFI call
   per endpoint would tempt callers to make IPC look like a function
   call. The JSON shape preserves the "you are crossing a boundary"
   ergonomics. Cost is small (~100 µs per call to serialize/deserialize
   a typical 1–10 KB payload).

The cost is paid once at the URL-scheme handler in Swift, never in
hot loops.

## 7. Permissions endpoint

`roninkb://api/permissions` is unique to the native app — there is no
equivalent HTTP route in daemon mode.

Response shape:

```json
{
  "accessibility": {
    "granted": true,
    "deepLink": "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
  },
  "inputMonitoring": {
    "granted": false,
    "deepLink": "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
  },
  "bluetooth": {
    "granted": true,
    "deepLink": "x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth"
  },
  "notifications": {
    "granted": null,
    "deepLink": null
  }
}
```

Sources used inside the runtime / shell:

- `accessibility`: `AXIsProcessTrustedWithOptions(nil)`
- `inputMonitoring`: `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)`
- `bluetooth`: `CBManager.authorization` (read on first init)
- `notifications`: `UNUserNotificationCenter.notificationSettings`
  (async; cached)

`roninkb://api/permissions/open` accepts `{ "kind": "accessibility" |
"inputMonitoring" | "bluetooth" }` and opens the matching System
Settings deep link via `NSWorkspace.shared.open(_:)`.

## 8. Mutual exclusion lock

Location:
- macOS: `~/Library/Application Support/RoninKB/runtime.lock`
- Linux: `$XDG_RUNTIME_DIR/roninkb/runtime.lock` or `/tmp/roninkb-<uid>.lock`
- Windows: `%LOCALAPPDATA%/RoninKB/runtime.lock`

Format: a plaintext file containing JSON:

```json
{ "pid": 12345, "since_unix_seconds": 1714329600, "mode": "app" | "daemon" }
```

Acquisition uses `flock(LOCK_EX | LOCK_NB)` on Unix and
`LockFileEx(LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)` on
Windows. The file is rewritten on every acquisition (the previous
holder's PID is overwritten only after the lock is granted).

Surfacing in the UI:
- If the app starts and finds the lock held by `mode: "daemon"`, it
  shows a one-time alert: "RoninKB is running in headless daemon mode.
  Quit the daemon to use the app?" with "Quit Daemon", "Use Daemon
  Mode", "Cancel".
- If the daemon binary starts and finds the lock held by `mode: "app"`,
  it logs `RoninKB.app is running; daemon mode is disabled while the app
  is open` and exits 0.

The lock is released on clean shutdown (`Coordinator::stop`) and via a
panic hook that drops the lock before re-raising.

## 9. Versioning and compatibility

This spec versions independently from the HTTP API. A new field on
`Request` or `Response` is a major-version-tolerable change if it has a
default. Adding a new enum variant is a breaking change for older
clients; if such a variant lands, the React side gates on
`window.__roninkb_native_version` (also injected at bootstrap time)
before sending it.

`window.__roninkb_native_version = "0.3.0"` is set at WKWebView
injection time. The React client should `parseInt(major)` and refuse to
send variants introduced after that version.

## 10. Testing

- **Rust runtime**: `crates/hhkb-runtime/tests/coordinator.rs`. Constructs
  a `Coordinator` directly (no UniFFI, no Swift), exercises every
  `Request` variant against an in-memory SQLite + fake device transport,
  asserts `Response` shape parity with the HTTP route output recorded
  in fixtures from `docs/fixtures/`.
- **Swift bridge**: `apps/hhkb-macos/RoninKBTests/CoordinatorBridgeTests.swift`.
  Constructs a `Coordinator` via UniFFI, sends a handful of representative
  requests, asserts the typed Swift side decodes them. Runs on every
  Xcode build.
- **End-to-end (XCUITest)**:
  `apps/hhkb-macos/RoninKBUITests/PanelFlowTests.swift`. Launches the
  app, simulates a global hotkey via the helper script
  `scripts/macos/press-hotkey.applescript`, asserts the panel renders
  within 500 ms, asserts a known piece of React content is present in
  the WKWebView DOM via `XCUIElement` snapshotting.

## 11. Out of scope for this spec

- The exact UniFFI version and whether UDL or proc-macro mode is used —
  decided in M1 of [`v0.3.0-plan.md`](../../docs/v0.3.0-plan.md).
- The exact dylib build orchestration between Cargo and Xcode — decided
  in M2.
- The Sparkle appcast hosting URL — pinned in M5.
- The cask name (`roninkb-app` vs `roninkb`) and how it relates to the
  daemon-mode formula — decided in M6.
