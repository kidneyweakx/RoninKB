# RFC 0002 — Native App Shell (single binary, no localhost port)

| Status     | Draft                                                  |
| ---------- | ------------------------------------------------------ |
| Owner      | @kidneyweakx                                           |
| Created    | 2026-05-15                                             |
| Target     | v0.3.0                                                 |
| Supersedes | n/a                                                    |
| Related    | [`docs/rfc-0001-macos-native-backend.md`](rfc-0001-macos-native-backend.md), [`docs/v0.3.0-plan.md`](v0.3.0-plan.md), [`spec/protocol/native-bridge.md`](../spec/protocol/native-bridge.md) |

## 1. Motivation

v0.2.0 lands the macOS native backend, but the user experience is still:

1. Install `hhkb-daemon` (Homebrew or curl).
2. `brew services start roninkb` to keep the daemon process alive.
3. `open http://127.0.0.1:7331/` to use the UI inside the user's default
   browser tab.

That's a **two-process, one-localhost-port, one-browser-tab** shape. It works,
but it's a "web app with a backing service" — not an app. Concretely:

- The UI lives in a Chrome/Safari tab the user has to find. It looks like a
  webpage; it has the browser's chrome, the browser's context menu, the
  browser's keybindings. No global hotkey, no menubar entry point, no
  platform materials (NSVisualEffectView / Liquid Glass on macOS 26).
- The daemon binds `127.0.0.1:7331`. If a user already has a service on that
  port, install fails. If a user runs two profiles of RoninKB (real + dev),
  they fight. The CORS is `Any` because any localhost origin can hit the
  daemon.
- Two processes, two crash reporting surfaces, two upgrade paths. The app
  has no idea the daemon is running; the daemon has no idea anyone is
  watching.
- Autostart means "run the daemon as a headless background service". A
  fresh-boot user sees nothing — the app is invisible until they remember
  to open the browser tab.

Raycast's 2.0 rewrite (referenced by the user) puts the React UI, the
Node/Rust backend, and the native shell into **one process, one binary, one
icon in the menubar**. They share a `Coordinator` exposed from a Rust
dylib loaded by the native shell, and a WKWebView that renders the React
tree directly off the app bundle — no localhost server, no separate process.

RoninKB has already built the hardest piece of that architecture: the Rust
core (`hhkb-core`, `hhkb-macos-native`, `hhkb-daemon`'s `Backend` registry).
What it's missing is the **shell that adopts it as a library instead of a
subprocess** and the **WebView that renders the React UI without going
through HTTP**.

This RFC defines that shell and the bridge it speaks.

## 2. Goals

- **Single process on macOS.** The app launches; one `RoninKB.app` binary
  contains the native shell, the WKWebView, and the daemon work as a
  linked library. No `hhkb-daemon` subprocess by default. No localhost
  bind.
- **No localhost port** on the default install path. The React UI talks
  to Rust through a custom URL scheme (`roninkb://api/...`) handled
  in-process. Port 7331 is freed.
- **The React codebase stays.** `apps/hhkb-app` (Chakra + Zustand + ~9k
  LoC TS) is the UI on every platform. The only thing that changes is
  the transport layer in `daemonClient.ts`.
- **Native feel on macOS.** Menubar entry point, global hotkey (default
  `⌘⇧K`) summons a `NSPanel` with `NSVisualEffectView` backing, ESC
  dismisses, `LSUIElement` so no Dock icon by default. Liquid Glass on
  macOS 26, Aqua vibrancy on Sequoia.
- **Headless daemon mode survives.** Power users, CI hosts, Linux, and
  Windows users still get the `hhkb-daemon` binary that opens
  `127.0.0.1:7331`. They're the same Rust code; only the host process
  differs.
- **CLI keeps working.** `hhkb-cli` already talks to the device directly
  through `hhkb-core` (not through the daemon HTTP API). No change.
- **Cold start under 500 ms warm.** Press `⌘⇧K` from menubar idle to
  panel visible: budget ≤ 100 ms warm, ≤ 500 ms cold. Prewarm the
  WKWebView at app launch.
- **One schema.** The daemon's existing REST shape (`/backend/*`,
  `/profiles/*`, `/flow/*`, `/device/*`) maps 1:1 onto the
  `roninkb://api/...` scheme. Same DTOs, same error shape. No new
  serialization formats.

## 3. Non-goals

- **Eliminating the HTTP API entirely.** It stays as the daemon-mode
  transport for headless / Linux / Windows. The native app *does not*
  serve it.
- **Replacing the React UI with SwiftUI / AppKit.** Tenet 5 of the
  native-feel skill: the iteration loop is the product. We keep React.
- **A Windows app in v0.3.0.** Windows users continue with the daemon +
  browser-tab UX. Windows native shell ships in v0.4.0 as a parallel
  effort (`apps/hhkb-windows/`, C# + WPF + WebView2).
- **A Linux app.** Linux stays on the daemon path indefinitely. WebKitGTK
  is not part of v0.3.0.
- **Bundling Node, Electron, Tauri, or any cross-platform UI framework.**
  We use the system WKWebView directly.
- **Sandboxing the app via the App Store sandbox.** The `CGEventTap`
  permission requires distribution outside the sandbox; we sign and
  notarize, but do not enter the MAS sandbox.
- **Replacing the SQLite profile store.** It moves into the app's
  Application Support directory; schema unchanged.

## 4. Architecture

### 4.1 Layers

```
┌──────────────────────────────────────────────────────────────────────┐
│  RoninKB.app — single process, single binary                         │
│                                                                      │
│  ┌────────────────────────────┐    ┌─────────────────────────────┐   │
│  │ Swift / AppKit shell       │    │  WKWebView                  │   │
│  │  - MenubarController       │    │   - loads `roninkb://app/`  │   │
│  │  - GlobalHotkey (CGTap)    │◀──▶│   - hosts apps/hhkb-app     │   │
│  │  - PanelController (NSPanel│ JS │     React bundle from        │   │
│  │      + NSVisualEffectView) │ msg│     `Resources/web/`         │   │
│  │  - SettingsWindowController│    │   - calls roninkb://api/...  │   │
│  │  - PermissionsCoordinator  │    │     via fetch()              │   │
│  │  - URLSchemeHandler        │    │   - receives push events     │   │
│  └────────────┬───────────────┘    │     via injected JS bridge   │   │
│               │ UniFFI             └─────────────────────────────┘   │
│               ▼                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  hhkb-runtime — Rust dylib (libhhkb_runtime.dylib)           │    │
│  │   - Coordinator { start, stop, send(Request) → Response }    │    │
│  │   - EventHandler callback (Rust → Swift → WebView)           │    │
│  │   - Owns: AppState (device, db, backends, flow, ble)         │    │
│  │   - Wraps the existing hhkb-daemon route handlers as in-     │    │
│  │     process function calls (no axum, no tokio runtime per    │    │
│  │     request).                                                │    │
│  └──────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────┘
```

The same `hhkb-runtime` crate is also linked into the **standalone
`hhkb-daemon` binary** (for headless / Linux / Windows), where axum wraps
its `Coordinator::send` calls behind HTTP routes — exactly today's
behaviour, just refactored to share a single core.

### 4.2 Three call paths

Every interaction the React UI initiates falls into one of three call
paths. The RFC pins each one to a specific transport so there's no
ambiguity in implementation.

| Path                                     | Today (v0.2.0)                                 | Native app (v0.3.0)                               |
| ---------------------------------------- | ---------------------------------------------- | ------------------------------------------------- |
| **Request/response** (`GET /profiles`)   | `fetch('http://127.0.0.1:7331/profiles')`      | `fetch('roninkb://api/profiles')` → URLSchemeHandler → UniFFI |
| **Server push** (`/ws` `DaemonEvent`)    | `new WebSocket('ws://127.0.0.1:7331/ws')`      | `window.roninkb.events.subscribe(...)` (injected) — events flow Rust → Swift (callback) → `webView.evaluateJavaScript('window.__roninkb_dispatch(...)')` |
| **Static assets** (`/ui/index.html`)     | `rust-embed` from daemon binary                | `Bundle.main` resources under `Resources/web/` → URLSchemeHandler `roninkb://app/...` |

In daemon mode (the legacy headless path), the same three call paths use
axum HTTP + axum WebSocket + axum static-file serving exactly as today.

### 4.3 Why a custom URL scheme, not `loadFileURL`

`WKWebView.loadFileURL(_:allowingReadAccessTo:)` works for static assets
but cannot intercept `fetch()` calls — the browser would either hit the
real network or fail. We need *both* "load the React bundle off disk"
*and* "intercept `fetch('/profiles')` calls and route them into Rust", so
we register a `WKURLSchemeHandler` for a scheme reserved exclusively for
RoninKB.

Scheme: `roninkb://`. Two authorities:

- `roninkb://app/<path>` → maps to `Bundle.main.path("Resources/web/<path>")`.
  Serves `index.html`, `assets/*.js`, `assets/*.css`. MIME guessed by
  extension. `index.html` returned for any unknown path inside the SPA
  (client-side router fallback).
- `roninkb://api/<endpoint>` → maps to `Coordinator::send(Request {
  endpoint, method, body })` and serializes the response back as
  `application/json`.

The scheme is registered with `nonPersistentDataStore` so it doesn't
collide with any system handler.

### 4.4 Why UniFFI

Tenet 2 (one schema, many languages). The native shell speaks Swift; the
runtime speaks Rust. Hand-written FFI between them drifts within a
sprint. UniFFI generates:

- Swift `Coordinator` protocol matching Rust's `pub trait Coordinator`
- Swift `EventHandler` callback protocol that Swift implements and Rust
  invokes
- Swift error enums matching Rust's `RuntimeError`, `RequestError`, etc.

Single source of truth: the `.udl` file (or, on UniFFI 0.28+, the
proc-macro annotations on the Rust trait). Adding a new endpoint means
editing one Rust file; Swift gets the new method on the next build.

Raycast Beta ships exactly this pattern: `libraycast_host.dylib` exports
`Coordinator`, `EventHandler`, `LogHandler`, `NativeSentryClient` — all
UniFFI-generated. See `references/02-architecture.md` § Layer 4 in the
native-feel skill.

### 4.5 Process model and lifecycle

- **App launch** (user clicks Dock icon, opens menubar item, or LaunchAgent
  fires at login): native shell starts, calls
  `Coordinator::new()` → `Coordinator::start()`. Runtime spins up
  AppState (device probe, SQLite open, backends register, BLE probe, Flow
  manager init). Same as today's `AppState::new()`.
- **First panel summon** (`⌘⇧K`): native shell ensures WKWebView is
  loaded (prewarmed at start), positions the `NSPanel` near the menubar
  item, presents it. React UI is already mounted — no cold-start cost.
- **App idle**: WKWebView is hidden but not torn down. Runtime keeps
  AppState alive; backends keep their permissions; Flow keeps its mDNS
  socket open.
- **App quit**: native shell calls `Coordinator::stop()`. Runtime drains
  events, shuts down backends, closes SQLite. Process exits cleanly.

Backend permission prompts (Input Monitoring, Accessibility) are now
attributed to **RoninKB.app**, not to a separate `hhkb-daemon` binary
buried in `/usr/local/bin`. This is a UX win (the prompt names match what
the user installed) and a permission-stability win (one prompt, one app
identity).

### 4.6 Daemon binary in v0.3.0

`hhkb-daemon` does not go away. It is rewritten as a **thin axum host**
that links the same `hhkb-runtime` and wraps `Coordinator::send` in HTTP
route handlers. Concretely:

- The `hhkb-daemon` crate keeps `main.rs` + `routes/*` + `ws.rs` + `ui.rs`,
  but `state.rs` / `backend/*` / `flow.rs` / `kanata.rs` / `ble.rs` move
  into `hhkb-runtime`.
- The HTTP API at `127.0.0.1:7331` is unchanged.
- macOS users get the daemon binary as part of the same release artefact
  but do not run it by default. It's there for `hhkb-cli` and headless
  use.
- Linux + Windows users continue running the daemon binary as today.
  When the v0.4.0 Windows native app lands, Windows will get its own
  in-process shell; until then, daemon-mode is the supported path.

### 4.7 Mutual exclusion

The native app and the daemon both want exclusive access to the HHKB
device (hidapi is not safe to share across processes) and exclusive
ownership of the macOS native backend (only one `CGEventTap` should be
active per user session). We must prevent the user from running both.

Mechanism: a **single advisory lock file** at
`~/Library/Application Support/RoninKB/runtime.lock` (macOS) /
`$XDG_RUNTIME_DIR/roninkb/runtime.lock` (Linux). The runtime acquires it
on `start`; if it's already held by a different PID, `start` fails with
`RuntimeError::AlreadyRunning { pid, since }`. The app surfaces this as
"Another RoninKB instance is running" with a "Quit the other" button
that sends SIGTERM to the held PID. The daemon binary fails fast and
exits.

The 7331 port bind is also a de facto lock (only one binder per port),
so daemon-vs-daemon is naturally guarded. App-vs-app is guarded by the
NSWorkspace single-instance behaviour. The lock file covers
app-vs-daemon.

## 5. Trade-offs surfaced

This RFC follows the native-feel skill's discipline: every win names what
is given up.

| Win                                            | Cost                                                                                 |
| ---------------------------------------------- | ------------------------------------------------------------------------------------ |
| Single binary, no localhost port               | macOS users can no longer hit `127.0.0.1:7331` from arbitrary scripts on the same machine unless they explicitly start daemon-mode (`hhkb-daemon --headless`). Tooling that assumed the port has to be updated. |
| Native materials, Liquid Glass, global hotkey  | ~150 MB resident baseline (WKWebView + WebKit shared frameworks). Today the daemon resides at ~30 MB; we cross the "Activity Monitor will notice" threshold. Tenet 8: baseline cost, communicate honestly. |
| One process to crash, one to update            | Crashes in the React tree now blank the app instead of just the browser tab. WKWebView crash recovery (auto-reload after `webContentProcessDidTerminate`) becomes mandatory. |
| Permission prompt attributed to RoninKB.app    | Users upgrading from v0.2.0 will be re-prompted for Input Monitoring / Accessibility because the bundle identifier changes. One-time UX bump; the upgrade notes have to call it out. |
| React iteration loop stays at ~200 ms HMR      | Cargo build of `hhkb-runtime` now blocks the Xcode build of the macOS app. We add a `cargo build --release-dylib` Xcode build phase and depend on incremental builds being fast enough. Measure during M1. |
| WebSocket replaced by in-process event push    | The push surface is now Swift-mediated, which means events must be serialized to JSON before crossing into the WebView. Same payload, one extra hop. Trace every event's frequency in M3. |
| LaunchAgent runs the app instead of the daemon | `LSUIElement = YES` so no Dock icon, but the app does briefly flash in the App Switcher when first launched until we add the no-activation startup path. M2 detail. |

## 6. Decision points deferred to milestones

These choices need a prototype, not a desk decision.

- **UniFFI version**: 0.28+ proc-macro mode vs 0.27 UDL file. Decision in
  M0 based on Xcode integration friction.
- **Whether `hhkb-app` ships two builds** (a daemon-HTTP build and a
  native-bridge build) or one build with runtime transport detection.
  Decision in M3 once we have measured bundle-size impact.
- **Whether the `NSPanel` is one window per "page"** (main, settings,
  flow log) or a single resizable panel with internal routing. Decision
  in M4 after the React side is split into entry points.
- **Whether Sparkle ships with EdDSA-signed updates from GitHub releases**
  or we wait for v0.3.1. Default: yes, ship Sparkle in v0.3.0 to avoid a
  manual re-install for users upgrading to v0.3.1.

## 7. What stays out of scope (and why)

- **A plugin/extension API.** Raycast's Layer 3 (Node backend) exists to
  host JS extensions. RoninKB has no plugin ecosystem and no plan to
  build one in v0.3. Skip Layer 3 entirely — keep it three-layer (Swift
  shell + WKWebView + Rust runtime).
- **An AI surface.** No.
- **Multi-window beyond menubar panel + settings + Flow log.** Three
  windows max in v0.3.0. Any further window types deferred.
- **Auto-updater for Linux / Windows daemon binary.** Their install paths
  (apt, scoop, raw curl) handle their own updates.
- **In-process kanata.** kanata stays a subprocess for LGPL compliance
  reasons (see [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md) and
  `CLAUDE.md` §"Third-party licenses"). The kanata supervisor in
  `hhkb-runtime` still launches it as a child process exactly as today.

## 8. Open questions

1. Does WebKit on macOS Tahoe (26) require any additional configuration
   for Liquid Glass to apply through a custom URL scheme? Verify in M2.
2. How fast can we get the `cargo build` → Xcode rebuild loop? Target
   < 5 s incremental. If we cannot, we add a "Swift-only" dev mode that
   uses the previously-built dylib without re-running Cargo.
3. Should daemon mode and app mode share the same `Application Support`
   directory or use separate ones to make A/B testing easier during
   transition? Default: same directory, with the lock file enforcing
   mutual exclusion (see §4.7). Decide in M0.
4. WebSocket subprotocol — today's `DaemonEvent` enum is serialized as
   JSON. We preserve that exact shape in the in-process bridge so the
   frontend's event dispatch code does not change. Confirm this stays
   true after M3.

## 9. Reference list

- Native-feel skill: `~/.claude/skills/native-feel-cross-platform-desktop/`
- RFC 0001 (macOS native backend): [`rfc-0001-macos-native-backend.md`](rfc-0001-macos-native-backend.md)
- Existing daemon HTTP surface: [`spec/protocol/daemon-http.md`](../spec/protocol/daemon-http.md)
- Existing WebSocket events: [`spec/protocol/websocket-events.md`](../spec/protocol/websocket-events.md)
- Kanata supervisor protocol: [`spec/protocol/kanata-supervisor.md`](../spec/protocol/kanata-supervisor.md)
- Flow sync protocol: [`spec/protocol/flow.md`](../spec/protocol/flow.md)
- UniFFI: https://mozilla.github.io/uniffi-rs/
- Raycast 2.0 deep dive: https://www.raycast.com/blog/a-technical-deep-dive-into-the-new-raycast
- `WKURLSchemeHandler`: https://developer.apple.com/documentation/webkit/wkurlschemehandler
- `NSPanel` non-activating style: https://developer.apple.com/documentation/appkit/nspanel
