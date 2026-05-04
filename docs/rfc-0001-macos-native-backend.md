# RFC 0001 — macOS Native Backend (no third-party driver by default)

| Status     | Draft                                         |
| ---------- | --------------------------------------------- |
| Owner      | @kidneyweakx                                  |
| Created    | 2026-04-28                                    |
| Target     | v0.2.0                                        |
| Supersedes | n/a                                           |
| Related    | [`docs/v0.2.0-plan.md`](v0.2.0-plan.md)       |

## 1. Motivation

In v0.1.x, RoninKB's macOS software-binding story relies on **kanata + Karabiner-DriverKit-VirtualHIDDevice**. That works, but it forces every macOS user to:

1. Install a third-party signed system extension (Karabiner-Elements).
2. Approve a `[activated waiting for user]` sysext prompt.
3. Grant Input Monitoring to a separate `kanata` binary.

That's three Apple-gated prompts, one third-party driver, and a UX that screams "this is a Karabiner mod" rather than "this is RoninKB". On Linux/Windows kanata grabs keys via `uinput`/Interception with no equivalent ceremony — macOS is therefore a second-class platform in our current shape.

Apple does not, and is not going to, ship a public API that matches DriverKit-quality keyboard interception. **But ~95% of the use cases we care about (tap-hold, layers, leader keys, hyper, app-aware bindings) can be reached cleanly via `CGEventTap` + `IOHIDManager` seizure** — the same architecture Hyperkey, BetterTouchTool, and Karabiner-Elements' fallback path use. This RFC defines a tiered backend system so RoninKB can lead with that elegant path and keep kanata + Karabiner as an opt-in for power users who want sub-100ms tap-hold latency.

## 2. Goals

- **Default macOS install needs zero third-party drivers**. No Karabiner, no DriverKit sysext, no kernel extension. One Apple-gated permission (Input Monitoring) and we run.
- **Feature parity for the ~95% case**: tap-hold, layers, leader keys, hyper, modifier-on-modifier, app-aware bindings, hot reload — all available on macOS with the native backend.
- **Honest capability surface**: every backend reports machine-readable capabilities; the React UI grays out features the active backend can't honor instead of pretending.
- **Kanata stays opt-in, never required**. Power users who want absolute best-in-class tap-hold can still install Karabiner separately and switch to the kanata backend.
- **macOS 26 Tahoe-ready**: external keyboards must work even though `CGEventTap` no longer receives them at the session-tap level on Tahoe.
- **MIT-clean**: no LGPL pull-in for the default install path. `keyberon` (MIT/Apache) provides the layer/tap-hold engine.

## 3. Non-goals

- **Sub-100ms tap-hold for fast home-row mods on macOS without DriverKit.** Not physically possible in pure userspace today; we surface this honestly and offer the kanata path for users who need it.
- **Per-keyboard rules on the native backend**. `CGEventPost` re-injection loses source-device identity; per-device rules require the kanata path.
- **Replacing kanata's macros / sequences engine wholesale**. v0.2.0 ships the high-leverage subset (layers, tap-hold, leader, hyper, app-aware). Macros and complex sequences land in v0.3.x or remain on the kanata path.
- **Replacing the HHKB hardware EEPROM path**. Hardware remains the most durable, permission-free surface and is preferred for what it can do.

## 4. Architecture

### 4.1 The `Backend` trait

Each binding strategy implements a single trait. Daemon owns a `Box<dyn Backend>` chosen at startup.

```rust
// crates/hhkb-daemon/src/backend/mod.rs
pub trait Backend: Send + Sync {
    /// Stable identifier ("eeprom", "hidutil", "macos-native", "kanata").
    fn id(&self) -> &'static str;
    fn human_name(&self) -> &'static str;

    /// What this backend can express.
    fn capabilities(&self) -> Capabilities;

    /// What permissions are still missing before start() can succeed.
    /// Returns `Granted` when ready.
    fn permission_status(&self) -> PermissionStatus;

    /// Apply `profile` to the OS / hardware. Idempotent.
    fn apply(&self, profile: &Profile) -> Result<(), BackendError>;

    /// Tear down whatever apply() created. No-op if not running.
    fn teardown(&self) -> Result<(), BackendError>;

    /// Hot-swap to a new profile without dropping modifiers.
    /// Default impl = teardown + apply; backends override when they can do
    /// better (kanata SIGUSR1, native backend in-process state swap).
    fn reload(&self, profile: &Profile) -> Result<(), BackendError> { ... }

    /// Is the backend currently driving the keyboard?
    fn is_running(&self) -> bool;

    /// Diagnostic snapshot for /backend/status.
    fn diagnostics(&self) -> BackendDiagnostics;
}
```

### 4.2 Capability bits

```rust
pub struct Capabilities {
    /// Per-key 1-to-1 keycode override.
    pub per_key_remap: bool,
    /// Number of independent layers (0 = none, 1 = base only, ...).
    pub layers: u8,
    /// Tap-hold quality tier, see `TapHoldQuality`.
    pub tap_hold: TapHoldQuality,
    /// Leader / sequence keys.
    pub leader_keys: bool,
    /// Multi-key chord triggers.
    pub combos: bool,
    /// Per-app rules (different remap per frontmost app).
    pub app_aware: bool,
    /// Per-keyboard rules (only one keyboard sees the rule).
    pub per_device: bool,
    /// Survives reboot without the daemon running.
    pub persistent: bool,
    /// Profile change without process restart.
    pub hot_reload: bool,
    /// Macros (multi-key string on a single press).
    pub macros: bool,
    /// Maximum macro length, when `macros = true`.
    pub max_macro_length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapHoldQuality {
    /// Backend cannot tap-hold (e.g. EEPROM, hidutil).
    None,
    /// CGEventTap-style — 150–250ms latency, best-effort under fast typing.
    BestEffort,
    /// DriverKit-grade — sub-100ms, deterministic.
    DriverGrade,
}
```

### 4.3 Permission model

```rust
pub enum PermissionStatus {
    /// All permissions granted; ready to apply().
    Granted,
    /// One or more pre-conditions still missing.
    Required(Vec<RequiredPermission>),
}

pub enum RequiredPermission {
    InputMonitoring { tcc_path: PathBuf, deep_link: String },
    Accessibility { tcc_path: PathBuf, deep_link: String },
    SystemExtension { bundle_id: String, install_command: Option<String> },
    /// Not a system permission — a user choice (e.g. "install Karabiner first").
    UserAction { description: String, deep_link: Option<String> },
}
```

`deep_link` is the `x-apple.systempreferences:` URL the UI uses to jump the user to the right pane in one click.

### 4.4 Backend selection policy

Daemon startup:

1. Read `~/Library/Application Support/roninKB/config.toml` for `[backend].pin = "..."` (user override).
2. Otherwise: enumerate backends in **priority order** (table below), pick the first whose `permission_status() == Granted`.
3. If none granted, daemon stays in `Backend::None` state — UI shows the setup wizard for the highest-tier backend the user can reach.

| Priority | Backend       | macOS | Linux | Windows |
| -------- | ------------- | ----- | ----- | ------- |
| 1        | `kanata`      | opt-in (`backend.pin = "kanata"`) | default | default |
| 2        | `macos-native`| **default** | n/a   | n/a     |
| 3        | `hidutil`     | fallback (always available) | n/a   | n/a     |
| 4        | `eeprom`      | always co-exists (hardware) | always | always |

EEPROM lives at priority 4 because it always co-exists — it doesn't compete with the others, it's a hardware layer that the active software backend stacks on top of. Profiles can declare which bindings target hardware vs. software; the daemon dispatches accordingly.

## 5. Backends in detail

### 5.1 `EEPROMBackend` — already implemented

- Crate: `hhkb-core` (existing).
- Capabilities: `per_key_remap = true`, `layers = 2` (Base + Fn) × 4 modes (HHK / Mac / Lite / Secret), `persistent = true`, everything else `false`.
- Permissions: none.
- Mechanism: HHKB Pro Hybrid HID protocol (PFU's Keymap Tool wire format). 128-byte payload per layer.
- Use it for: durable per-key remaps, modifier swaps that should survive a different host (e.g. Caps→Ctrl), Bluetooth-attached usage where the daemon isn't always running.

No code change required — wrap the existing `hhkb-core` device API in a `Backend` impl and surface its capabilities in the new shape.

### 5.2 `HidutilBackend` — new, small

- Crate: `hhkb-daemon` internal module `backend/hidutil.rs`.
- Capabilities: `per_key_remap = true`, `layers = 1` (no layering), `persistent = false` (re-applied at login by LaunchAgent), `app_aware = false`.
- Permissions: macOS Sonoma+ requires Input Monitoring for non-modifier remaps; modifier-only remaps need none.
- Mechanism: shells out to `/usr/bin/hidutil property --set '{"UserKeyMapping": [...]}'`. Each entry is `{HIDKeyboardModifierMappingSrc: 0x700000000 | usage, HIDKeyboardModifierMappingDst: ...}`. Persistence via a generated `~/Library/LaunchAgents/gg.solidarity.roninkb.hidutil.plist`.
- Use it for: system-wide simple key swaps that should affect every keyboard the user plugs in (Caps↔Esc on built-in + external simultaneously) — something EEPROM can't do because EEPROM is per-keyboard.
- Failure modes: macOS 13.6 / 14.2 / Tahoe 26 betas have all temporarily broken `hidutil` at one point. Backend reports "degraded" if the verb is missing, falls through to native backend.

~200 LoC including LaunchAgent management.

### 5.3 `MacosNativeBackend` — new, the centerpiece

- Crate: new workspace member `crates/hhkb-macos-native` (cfg-gated to `target_os = "macos"`).
- Capabilities: `per_key_remap = true`, `layers = 16+`, `tap_hold = BestEffort`, `leader_keys = true`, `combos = true`, `app_aware = true`, `per_device = false`, `persistent = false` (daemon-driven), `hot_reload = true`, `macros = false` for v0.2.0.
- Permissions: Input Monitoring (mandatory). Accessibility only if we post synthetic key events (we do — required for re-injection).

#### 5.3.1 Event ingestion

Two source paths, chosen at runtime:

**Path A (default, pre-Tahoe and built-in keyboards on Tahoe)**: `CGEventTap` at `kCGSessionEventTap` with `headInsertEventTap` placement. Standard public API. Tap callback runs on a dedicated CFRunLoop thread inside the daemon process.

**Path B (external keyboards on macOS 26 Tahoe and later)**: `IOHIDManager` opens the matching keyboard with `kIOHIDOptionsTypeSeizeDevice`, intercepts raw HID reports, decodes, feeds into the same engine, re-injects via `CGEventPost`. Detection: on daemon start we enumerate keyboards via `IOHIDManagerCreate` + `IOHIDManagerSetDeviceMatching`; for each external keyboard on Tahoe-or-later we open a seize handle.

Path A and Path B feed into the same `keyberon`-based state machine — they only differ in how raw events arrive.

#### 5.3.2 Engine

[`keyberon`](https://github.com/TeXitoi/keyberon) (MIT/Apache) provides the layer + tap-hold + chord state machine. We use the `keyberon::layout::Layout` type directly. Profile JSON → `Vec<Vec<Vec<Action<()>>>>` translation in `crates/hhkb-daemon/src/backend/macos_native/profile.rs`.

If `keyberon` proves too constrained, the alternate is [`smart-keymap`](https://github.com/rgoulter/smart-keymap), which exposes a C-callable static lib and supports more advanced chord/tap-hold variants. Decision deferred to M1 (PoC).

#### 5.3.3 Re-injection

Engine outputs `[Action]` per tick (typically 0 or 1 actions, occasionally a deferred tap-hold flush). Each action becomes:

- Press / release events via `CGEventCreateKeyboardEvent` + `CGEventPost(kCGSessionEventTap, …)`.
- Modifier-flag events via `CGEventSetFlags` on the synthesised event.

Side note: re-injection inserts events *after* our tap, so we must not also pass the original event through — engine semantics are "consume input, emit output". Implemented by returning `NULL` from the tap callback for any key we own and posting our synthetic events instead.

#### 5.3.4 App-aware bindings

`NSWorkspace.shared.frontmostApplication` polled every 200ms via a dispatch source; engine layer-switch on bundle-id transitions. Profile schema gets a `per_app: { "com.apple.Safari": "browse-layer" }` map.

### 5.4 `KanataBackend` — existing, becomes opt-in on macOS

- Crate: existing `hhkb-daemon::kanata` (no rename, just refactor to impl `Backend`).
- Capabilities: full kanata feature set — `tap_hold = DriverGrade`, `per_device = true`, all the macros and sequences kanata supports.
- Permissions: macOS — Karabiner DriverKit sysext + Input Monitoring. Linux — none. Windows — Interception driver bundled.
- macOS default behaviour: stays installed, but daemon does **not** auto-select it. User opts in via UI ("Switch to Kanata backend" → daemon checks Karabiner DriverKit, prompts install if missing).
- The driver_activated preflight added in v0.1.1 stays in place — it's now the kanata-backend-specific permission check.

## 6. Permissions story

| Backend       | macOS                                  | Linux              | Windows           |
| ------------- | -------------------------------------- | ------------------ | ----------------- |
| EEPROM        | none                                   | udev rule (one-time, already shipped) | none |
| hidutil       | Input Monitoring (alphanumeric remaps only) | n/a            | n/a               |
| macos-native  | Input Monitoring + Accessibility       | n/a                | n/a               |
| kanata        | Karabiner DriverKit sysext + Input Monitoring | none        | none (Interception is bundled) |

Default macOS install: **EEPROM + Input Monitoring** (one prompt, deep-linked from the UI). That's it.

## 7. Frontend changes

### 7.1 Capability-aware UI

`/backend/list` returns the available backends with their capabilities. The React UI:

- Shows a backend selector in `SettingsPanel.tsx` (radio: Native / Hidutil / Kanata / EEPROM-only).
- On the active backend, grays out features it can't express. E.g. when Hidutil is active, the "Add tap-hold binding" button is disabled with tooltip "tap-hold requires Native or Kanata backend".
- Drops the existing kanata-specific section in favour of a unified "Software bindings" pane that doesn't leak the backend identity unless the user opens "Advanced".

### 7.2 First-run wizard

On first launch, the wizard:

1. Detects platform.
2. macOS: explains the Native backend, walks through Input Monitoring + Accessibility grants (deep-link buttons), tests that the engine receives a known keypress.
3. macOS power-user path: a small "Want sub-100ms tap-hold? Install Karabiner-Elements and switch to Kanata backend." card under Advanced.
4. Linux/Windows: the existing Kanata flow (unchanged).

### 7.3 New API endpoints

- `GET /backend/list` → `[{id, name, capabilities, permission_status}]`
- `POST /backend/select` body `{id}` → switches active backend, returns new state
- `GET /backend/status` → diagnostics for the active backend (replaces `/kanata/status` semantically; old endpoint stays as a compat alias)

## 8. macOS 26 Tahoe specifics

- Apple-shipped change: `kCGSessionEventTap` no longer receives events from external HID keyboards (built-in keyboards still work). Confirmed by Karabiner-Elements, Hyperkey, BetterTouchTool community threads.
- Detection: `MacosNativeBackend` runs an `IOHIDManager` enumeration at startup; for each keyboard whose `kIOHIDLocationIDKey` indicates external transport we attach a Path B (seize) handler. Built-in keyboards stay on Path A.
- Reboot survival: seize handles drop on logout. LaunchAgent restarts the daemon at login; daemon re-seizes on startup. No user-visible difference.
- Mid-session hotplug: `IOHIDManager` registration callback re-runs enumeration on USB attach/detach.

## 9. Tap-hold quality matrix

The UI surfaces this honestly:

| Backend       | Tap-hold quality | Latency        | Reliability under burst typing |
| ------------- | ---------------- | -------------- | ------------------------------ |
| macos-native  | BestEffort       | 150–250ms      | Good for normal typing, occasional misfires above ~120 wpm with home-row mods |
| kanata + Karabiner | DriverGrade | sub-100ms      | Best-in-class                  |
| eeprom        | None             | n/a            | n/a                            |
| hidutil       | None             | n/a            | n/a                            |

The wizard tells home-row-mod power users to install Karabiner. The default user never sees this trade-off because tap-hold defaults are conservative (Esc/Ctrl on Caps, etc., where 200ms is invisible).

## 10. Migration & breaking changes

- `/kanata/*` REST endpoints stay as deprecated aliases for `/backend/*` so v0.1.x clients keep working through the v0.2.0 cycle. Removed in v0.3.0.
- Existing user profiles parse identically — the binding schema doesn't change. The daemon translates them to whichever engine is active.
- macOS users who previously installed Karabiner: daemon detects the existing install, keeps the kanata backend selectable, but does **not** auto-select it. Active backend persists across upgrades — if a v0.1.x user had kanata running, they stay on kanata after upgrading to v0.2.0; if they're a fresh install, they default to native.
- New macOS users: never see Karabiner unless they open Advanced.
- `bundled-kanata` feature on macOS becomes default-off but still buildable. The kanata binary is no longer extracted unless the kanata backend is selected.

## 11. Open questions

1. **`keyberon` vs `smart-keymap` engine choice** — decided in M1 PoC.
2. **App-aware polling rate** — 200ms is a guess; might need event-driven via `NSWorkspaceDidActivateApplicationNotification`.
3. **Profile schema additions** — how do we express tap-hold parameters (tap term, hold timeout) in JSON without forking the existing schema? Probably a `params` sub-object per binding.
4. **Capability negotiation when a profile uses features the active backend can't express** — fail closed (refuse to load) or downgrade (load with warnings)? Lean toward fail-closed with clear error.
5. **Should EEPROM remain priority 4 or move to priority 1 on macOS?** EEPROM-only would mean no software backend at all — viable for users who only want hardware remap. Defer to a config flag `[backend].software = "auto" | "off"`.
6. **License-clean re-injection** — `core-graphics-sys` is MIT, `io-kit-sys` is MIT. No LGPL pull-in expected. Confirm during M1.

## 12. Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| `IOHIDManager` seize on Tahoe behaves differently than expected | Spike in M1 PoC against a real Tahoe machine before committing the architecture |
| Tap-hold under fast typing feels worse than promised | Surface the quality tier honestly; provide Kanata escape hatch from day one |
| Apple changes CGEventTap semantics again in macOS 27 | Backend trait isolates the OS-specific bits — only the event-source layer needs to track Apple changes |
| `keyberon` lacks a feature we need (e.g. specific chord shape) | Switch to `smart-keymap` or contribute upstream; both are MIT/Apache |
| User confusion about backend selection | Default selection is automatic and silent; the selector lives in Advanced and shows clear capability comparison |

## 13. References

- [Hyperkey reference architecture (MIT)](https://github.com/feedthejim/hyperkey)
- [Hammerspoon — Lua-driven CGEventTap remapper (MIT)](https://github.com/Hammerspoon/hammerspoon)
- [keyberon Rust crate (MIT/Apache)](https://github.com/TeXitoi/keyberon)
- [smart-keymap library (Apache, active 2025)](https://github.com/rgoulter/smart-keymap)
- [kanata — LGPL engine, opt-in backend in v0.2.0](https://github.com/jtroo/kanata)
- [Karabiner-Elements DEVELOPMENT.md (CGEventTap fallback path)](https://github.com/pqrs-org/Karabiner-Elements/blob/main/DEVELOPMENT.md)
- [Karabiner-DriverKit-VirtualHIDDevice (LGPL sysext, the kanata-path dependency)](https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice)
- [Apple TN2450: Remapping Keys in macOS](https://developer.apple.com/library/archive/technotes/tn2450/_index.html)
- [Apple CGEvent.tapCreate](https://developer.apple.com/documentation/coregraphics/cgevent/tapcreate(tap:place:options:eventsofinterest:callback:userinfo:))
- [Apple IOHIDManager (Path B seize on Tahoe)](https://developer.apple.com/documentation/iokit/iohidmanager)
- [Apple NSWorkspace (frontmost-app + activation notifications)](https://developer.apple.com/documentation/appkit/nsworkspace)
- [macOS 26 Tahoe Release Notes](https://developer.apple.com/documentation/macos-release-notes/macos-26-release-notes)
- [Year of the homerow mods (Callista, 2025)](https://callistaenterprise.se/blogg/teknik/2025/01/10/homerow-mods/)
- [CGEvent Taps and Code Signing race (Daniel Raffel, Feb 2026)](https://danielraffel.me/til/2026/02/19/cgevent-taps-and-code-signing-the-silent-disable-race/)
- [PFU HHKB Pro Hybrid manual](https://origin.pfultd.com/downloads/hhkb/manual/P3PC-6641-01EN.pdf)
- Local: [`crates/hhkb-core/src/keymap.rs`](../crates/hhkb-core/src/keymap.rs)
- Local: [`crates/hhkb-daemon/src/kanata.rs`](../crates/hhkb-daemon/src/kanata.rs) (current implementation)
