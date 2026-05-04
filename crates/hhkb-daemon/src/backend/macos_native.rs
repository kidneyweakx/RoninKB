//! `MacosNativeBackend` — wraps `hhkb-macos-native` (the M1 PoC) as a
//! `Backend`. macOS-only.
//!
//! Lifecycle:
//! - `apply()` spawns a worker thread that owns a `CFRunLoop`. The thread
//!   installs a `CGEventTap`, runs the engine, and re-injects via
//!   `CGEventPost`. It blocks on `CFRunLoop::run_current()` until told to
//!   stop.
//! - `teardown()` calls `CFRunLoop::stop()` on the worker's run loop ref,
//!   which unblocks `install_and_run` and lets the thread exit cleanly.
//!   We then `join()` the thread.
//!
//! M0 / partial M2 caveat: the daemon's REST routing still goes through
//! kanata. This trait impl exists so that M4 can `Box<dyn Backend>` it
//! alongside KanataBackend without a second rewrite, and so that integration
//! testing (manual, on a Mac with permissions granted) can drive it via the
//! trait surface. The PoC layout (Caps Lock → tap=Esc / hold=LCtrl) is
//! hardcoded — full ViaProfile translation is M2 work.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use core_foundation::runloop::CFRunLoop;
use hhkb_core::ViaProfile;
use hhkb_macos_native::engine::Transition;
use hhkb_macos_native::event_tap::{install_and_run, ObservedEvent, Verdict};
use hhkb_macos_native::{
    inject, passthrough_grid, set_cell, CapsBindingSpec, Engine, EngineEvent, HidUsage, KeyAction,
    KeyCode, COLS, ROWS,
};

use super::{
    Backend, BackendDiagnostics, BackendError, BackendId, Capabilities, PermissionStatus,
    RequiredPermission, TapHoldQuality,
};

/// Owned by the running runtime so `teardown` has a stable handle to stop the
/// CFRunLoop. The `runloop` field is `Send`-incompatible to use directly, so
/// we wrap the `CFRunLoop::stop` invocation in a closure stored on a thread
/// that shares its run loop ref via a `Mutex<Option<CFRunLoop>>`.
struct Runtime {
    /// Set by `teardown` to signal the tick thread to exit.
    stop: Arc<AtomicBool>,
    /// Worker thread driving the CGEventTap CFRunLoop.
    tap_thread: Option<JoinHandle<()>>,
    /// Worker thread draining engine transitions every 1ms.
    tick_thread: Option<JoinHandle<()>>,
    /// CFRunLoop handle for the tap thread, populated once the thread starts.
    /// `Mutex<Option<...>>` because we need to set it from the worker thread
    /// and read it from `teardown`.
    tap_runloop: Arc<Mutex<Option<CFRunLoop>>>,
    /// Shared engine handle so `apply()` can hot-swap the layout in place
    /// without restarting the tap/tick threads. The tap callback and tick
    /// thread both lock this; in normal operation contention is per-event
    /// and trivial.
    engine: Arc<Mutex<Engine>>,
}

pub struct MacosNativeBackend {
    runtime: Mutex<Option<Runtime>>,
}

impl MacosNativeBackend {
    pub fn new() -> Self {
        Self {
            runtime: Mutex::new(None),
        }
    }
}

impl Default for MacosNativeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for MacosNativeBackend {
    fn id(&self) -> BackendId {
        BackendId::MacosNative
    }

    fn human_name(&self) -> &'static str {
        "macOS Native"
    }

    fn capabilities(&self) -> Capabilities {
        // RFC 0001 §5.3: tap-hold is BestEffort (CGEventTap latency is in the
        // 150–250ms band, which is fine for default conservative tap-holds
        // but worse than DriverKit-grade for home-row mods). Macros are
        // explicitly out of scope for v0.2.0 — kanata path stays the answer
        // for those.
        Capabilities {
            per_key_remap: true,
            layers: 16,
            tap_hold: TapHoldQuality::BestEffort,
            leader_keys: true,
            combos: true,
            // App-aware via NSWorkspace; M2 work.
            app_aware: true,
            // CGEventPost re-injection loses source-device identity, so
            // per-keyboard rules are out — that's a kanata-only feature.
            per_device: false,
            // Daemon-driven; no LaunchAgent piece on this backend.
            persistent: false,
            // In-process state swap is the whole point of the native backend.
            hot_reload: true,
            macros: false,
            max_macro_length: 0,
        }
    }

    fn permission_status(&self) -> PermissionStatus {
        let mut required = Vec::new();

        if !cg_preflight_listen_event_access() {
            required.push(RequiredPermission::InputMonitoring {
                tcc_path: std::path::PathBuf::from(
                    "/Library/Application Support/com.apple.TCC/TCC.db",
                ),
                deep_link:
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
                        .to_string(),
            });
        }

        if !ax_is_process_trusted() {
            required.push(RequiredPermission::Accessibility {
                tcc_path: std::path::PathBuf::from(
                    "/Library/Application Support/com.apple.TCC/TCC.db",
                ),
                deep_link:
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
                        .to_string(),
            });
        }

        if required.is_empty() {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Required(required)
        }
    }

    fn apply(&self, profile: &ViaProfile) -> Result<(), BackendError> {
        // Bail loudly if permissions aren't there — the CGEventTap will
        // simply not receive events otherwise, and the user gets a "nothing
        // happens" experience.
        if let PermissionStatus::Required(missing) = self.permission_status() {
            return Err(BackendError::NotReady(missing));
        }

        let new_engine = build_engine_for_profile(profile)?;

        let mut guard = self
            .runtime
            .lock()
            .expect("MacosNativeBackend runtime mutex poisoned");

        if let Some(runtime) = guard.as_ref() {
            // Hot-swap: the threads keep running, we just replace the engine
            // contents in place. The tap callback's next event will see the
            // new ownership map; the tick thread's next iteration will diff
            // the carried-over `last_active` against the new layout's active
            // set, emitting releases for stale modifiers (RFC 0001 §5.3.2 —
            // hot-reload is what the native backend is for).
            let mut engine_guard = runtime
                .engine
                .lock()
                .expect("MacosNativeBackend engine mutex poisoned");
            let mut next = new_engine;
            engine_guard.carry_over_to(&mut next);
            *engine_guard = next;
            return Ok(());
        }

        let stop = Arc::new(AtomicBool::new(false));
        let engine = Arc::new(Mutex::new(new_engine));
        let tap_runloop: Arc<Mutex<Option<CFRunLoop>>> = Arc::new(Mutex::new(None));

        // Tick thread: drains engine output every 1ms, re-injects synthetic
        // press/release events.
        let tick_engine = Arc::clone(&engine);
        let tick_stop = Arc::clone(&stop);
        let tick_thread = thread::Builder::new()
            .name("hhkb-native-tick".to_string())
            .spawn(move || {
                while !tick_stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(1));
                    let transitions = {
                        let mut e = tick_engine.lock().expect("engine poisoned");
                        e.tick().transitions
                    };
                    for t in transitions {
                        let res = match t {
                            Transition::Press(u) => inject::post_press(u),
                            Transition::Release(u) => inject::post_release(u),
                        };
                        if let Err(err) = res {
                            tracing::debug!(?err, "macos-native inject error");
                        }
                    }
                }
            })
            .map_err(|e| BackendError::Internal(format!("spawn macos-native tick thread: {e}")))?;

        // Tap thread: installs the CGEventTap and blocks on CFRunLoop::run.
        let tap_engine = Arc::clone(&engine);
        let tap_runloop_set = Arc::clone(&tap_runloop);
        let tap_thread = thread::Builder::new()
            .name("hhkb-native-tap".to_string())
            .spawn(move || {
                // Capture the runloop ref before install_and_run blocks; this
                // is the handle teardown() needs to stop us.
                {
                    let mut slot = tap_runloop_set.lock().expect("tap_runloop_set poisoned");
                    *slot = Some(CFRunLoop::get_current());
                }
                let cb_engine = Arc::clone(&tap_engine);
                let result = install_and_run(Box::new(move |ev| match ev {
                    ObservedEvent::Pressed(usage) => {
                        let mut e = cb_engine.lock().expect("engine poisoned");
                        if e.is_owned(usage) {
                            e.input(EngineEvent::Press(usage));
                            Verdict::Suppress
                        } else {
                            Verdict::PassThrough
                        }
                    }
                    ObservedEvent::Released(usage) => {
                        let mut e = cb_engine.lock().expect("engine poisoned");
                        if e.is_owned(usage) {
                            e.input(EngineEvent::Release(usage));
                            Verdict::Suppress
                        } else {
                            Verdict::PassThrough
                        }
                    }
                    _ => Verdict::PassThrough,
                }));
                if let Err(e) = result {
                    tracing::error!(?e, "macos-native event tap install failed");
                }
            })
            .map_err(|e| {
                stop.store(true, Ordering::Relaxed);
                BackendError::Internal(format!("spawn macos-native tap thread: {e}"))
            })?;

        *guard = Some(Runtime {
            stop,
            tap_thread: Some(tap_thread),
            tick_thread: Some(tick_thread),
            tap_runloop,
            engine,
        });
        Ok(())
    }

    fn teardown(&self) -> Result<(), BackendError> {
        let mut guard = self
            .runtime
            .lock()
            .expect("MacosNativeBackend runtime mutex poisoned");
        let Some(mut runtime) = guard.take() else {
            return Ok(());
        };

        runtime.stop.store(true, Ordering::Relaxed);

        // Stop the tap thread's CFRunLoop. The runloop ref is set by the tap
        // thread at startup; if it's still None we missed the window — wait
        // briefly and retry, then give up and let the thread join naturally
        // when the AtomicBool flips (it won't, since CFRunLoop blocks; this
        // edge case is logged so future M2 work can address it).
        let mut tries = 0;
        let runloop = loop {
            if let Some(rl) = runtime
                .tap_runloop
                .lock()
                .expect("tap_runloop poisoned")
                .clone()
            {
                break Some(rl);
            }
            if tries >= 50 {
                break None;
            }
            tries += 1;
            thread::sleep(Duration::from_millis(10));
        };
        if let Some(rl) = runloop {
            rl.stop();
        } else {
            tracing::warn!(
                "macos-native: tap runloop never registered; tap thread may not exit cleanly"
            );
        }

        if let Some(handle) = runtime.tap_thread.take() {
            if let Err(e) = handle.join() {
                tracing::warn!(?e, "macos-native tap thread join failed");
            }
        }
        if let Some(handle) = runtime.tick_thread.take() {
            if let Err(e) = handle.join() {
                tracing::warn!(?e, "macos-native tick thread join failed");
            }
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.runtime
            .lock()
            .expect("MacosNativeBackend runtime mutex poisoned")
            .is_some()
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        let state = if self.is_running() {
            "running"
        } else {
            "stopped"
        };
        BackendDiagnostics { state, note: None }
    }
}

/// Translate the profile's macOS-native section (if any) into an engine
/// instance, ready to drop into the runtime. Returns the M1 PoC default
/// (Caps tap=Esc / hold=LCtrl, 200ms) when the profile is silent — so the
/// native backend always has something to apply.
///
/// Two recognised JSON shapes (under `_roninKB.software.config` when
/// `engine == "macos-native"`):
///
/// 1. **Legacy single-binding** (M1 PoC, kept for back-compat):
///    ```json
///    { "caps": { "type": "holdtap", "tap": "esc", "hold": "lctl" } }
///    ```
///    or `{ "caps": { "type": "passthrough" } }` to leave Caps alone.
///
/// 2. **Per-position bindings** (M2): a free-form map keyed by source key
///    name, value is a per-cell action.
///    ```json
///    {
///      "bindings": {
///        "caps": { "type": "holdtap", "tap": "esc", "hold": "lctl" },
///        "a":    { "type": "remap",   "to": "b" }
///      }
///    }
///    ```
///    Unspecified keys default to passthrough. The `caps` shortcut is
///    treated as `bindings.caps` if both are present (last-write-wins).
fn build_engine_for_profile(profile: &ViaProfile) -> Result<Engine, BackendError> {
    let Some(software) = profile.ronin.as_ref().and_then(|r| r.software.as_ref()) else {
        return Ok(Engine::from_spec(&CapsBindingSpec::caps_ctrl_esc()));
    };
    if software.engine != "macos-native" {
        return Ok(Engine::from_spec(&CapsBindingSpec::caps_ctrl_esc()));
    }

    let parsed = parse_native_config(&software.config)?;
    if parsed.is_empty() {
        return Ok(Engine::from_spec(&CapsBindingSpec::caps_ctrl_esc()));
    }
    Ok(Engine::from_grid(parsed.into_grid()?))
}

#[derive(serde::Deserialize, Default)]
struct NativeConfig {
    /// Legacy shortcut for the Caps cell.
    #[serde(default)]
    caps: Option<CellConfig>,
    /// Free-form per-key bindings; key is one of [`parse_keycode`]'s names.
    #[serde(default)]
    bindings: std::collections::HashMap<String, CellConfig>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum CellConfig {
    HoldTap {
        #[serde(default = "default_timeout_ms")]
        timeout_ms: u16,
        tap: String,
        hold: String,
    },
    Remap {
        to: String,
    },
    Passthrough,
}

fn default_timeout_ms() -> u16 {
    200
}

impl NativeConfig {
    fn is_empty(&self) -> bool {
        self.caps.is_none() && self.bindings.is_empty()
    }

    /// Convert into a per-position `KeyAction` grid suitable for
    /// [`Engine::from_grid`]. Returns `ProfileRejected` if any key name or
    /// keycode in the spec doesn't resolve.
    fn into_grid(self) -> Result<[[KeyAction; COLS]; ROWS], BackendError> {
        let mut grid = passthrough_grid();

        if let Some(cell) = self.caps {
            apply_cell(&mut grid, "caps", cell)?;
        }
        for (name, cell) in self.bindings {
            apply_cell(&mut grid, &name, cell)?;
        }
        Ok(grid)
    }
}

fn apply_cell(
    grid: &mut [[KeyAction; COLS]; ROWS],
    src_name: &str,
    cell: CellConfig,
) -> Result<(), BackendError> {
    let src_kc = parse_keycode(src_name)
        .ok_or_else(|| BackendError::ProfileRejected(format!("unknown source key {src_name:?}")))?;
    let src_hid = HidUsage::from(src_kc);

    let action = match cell {
        CellConfig::Passthrough => KeyAction::Passthrough,
        CellConfig::Remap { to } => {
            let target = parse_keycode(&to).ok_or_else(|| {
                BackendError::ProfileRejected(format!("unknown remap target {to:?}"))
            })?;
            KeyAction::Remap(target)
        }
        CellConfig::HoldTap {
            timeout_ms,
            tap,
            hold,
        } => {
            let tap = parse_keycode(&tap).ok_or_else(|| {
                BackendError::ProfileRejected(format!("unknown tap keycode {tap:?}"))
            })?;
            let hold = parse_keycode(&hold).ok_or_else(|| {
                BackendError::ProfileRejected(format!("unknown hold keycode {hold:?}"))
            })?;
            KeyAction::HoldTap {
                timeout_ms,
                tap,
                hold,
            }
        }
    };

    set_cell(grid, src_hid, action);
    Ok(())
}

fn parse_native_config(s: &str) -> Result<NativeConfig, BackendError> {
    serde_json::from_str(s).map_err(|e| {
        BackendError::ProfileRejected(format!(
            "macos-native profile JSON did not match schema: {e}"
        ))
    })
}

/// Map the names the profile schema accepts to keyberon's `KeyCode`.
/// Covers letters a–z, digits 0–9, modifiers, common edit keys, and `caps`.
/// The set is deliberately bounded — extending it is a one-line change
/// when a new key is needed, and the parser surfaces unknown names as
/// `ProfileRejected` so misspellings don't silently no-op.
fn parse_keycode(s: &str) -> Option<KeyCode> {
    let n = s.trim().to_ascii_lowercase();
    if n.len() == 1 {
        let c = n.as_bytes()[0];
        if c.is_ascii_lowercase() {
            // Map 'a'..'z' to KeyCode::A..KeyCode::Z. The kanata-keyberon
            // KeyCode discriminants are not portable across versions, so
            // build via match rather than `transmute`.
            return Some(match c {
                b'a' => KeyCode::A,
                b'b' => KeyCode::B,
                b'c' => KeyCode::C,
                b'd' => KeyCode::D,
                b'e' => KeyCode::E,
                b'f' => KeyCode::F,
                b'g' => KeyCode::G,
                b'h' => KeyCode::H,
                b'i' => KeyCode::I,
                b'j' => KeyCode::J,
                b'k' => KeyCode::K,
                b'l' => KeyCode::L,
                b'm' => KeyCode::M,
                b'n' => KeyCode::N,
                b'o' => KeyCode::O,
                b'p' => KeyCode::P,
                b'q' => KeyCode::Q,
                b'r' => KeyCode::R,
                b's' => KeyCode::S,
                b't' => KeyCode::T,
                b'u' => KeyCode::U,
                b'v' => KeyCode::V,
                b'w' => KeyCode::W,
                b'x' => KeyCode::X,
                b'y' => KeyCode::Y,
                b'z' => KeyCode::Z,
                _ => unreachable!(),
            });
        }
        if c.is_ascii_digit() {
            return Some(match c {
                b'1' => KeyCode::Kb1,
                b'2' => KeyCode::Kb2,
                b'3' => KeyCode::Kb3,
                b'4' => KeyCode::Kb4,
                b'5' => KeyCode::Kb5,
                b'6' => KeyCode::Kb6,
                b'7' => KeyCode::Kb7,
                b'8' => KeyCode::Kb8,
                b'9' => KeyCode::Kb9,
                b'0' => KeyCode::Kb0,
                _ => unreachable!(),
            });
        }
    }
    Some(match n.as_str() {
        "esc" | "escape" => KeyCode::Escape,
        "tab" => KeyCode::Tab,
        "space" | "spc" => KeyCode::Space,
        "enter" | "ret" => KeyCode::Enter,
        "bspc" | "backspace" => KeyCode::BSpace,
        "del" | "delete" => KeyCode::Delete,
        "caps" | "capslock" => KeyCode::CapsLock,
        "lctl" | "lctrl" => KeyCode::LCtrl,
        "rctl" | "rctrl" => KeyCode::RCtrl,
        "lsft" | "lshift" => KeyCode::LShift,
        "rsft" | "rshift" => KeyCode::RShift,
        "lalt" => KeyCode::LAlt,
        "ralt" => KeyCode::RAlt,
        "lgui" | "lcmd" | "lwin" => KeyCode::LGui,
        "rgui" | "rcmd" | "rwin" => KeyCode::RGui,
        _ => return None,
    })
}

/// Wrapper around `CGPreflightListenEventAccess` for "Input Monitoring
/// granted?" without spawning kanata. Same call kanata.rs makes; duplicated
/// here to keep this module standalone (it's macOS-only by definition).
fn cg_preflight_listen_event_access() -> bool {
    // SAFETY: pure preflight API, no ownership crossing.
    unsafe { ffi::CGPreflightListenEventAccess() }
}

fn ax_is_process_trusted() -> bool {
    // Pass `false` for prompt — we don't want a permission prompt to fire
    // during status polling; we just want to know the current state.
    unsafe { ffi::AXIsProcessTrustedWithOptions(std::ptr::null()) }
}

mod ffi {
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn CGPreflightListenEventAccess() -> bool;
        pub fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_and_name_are_stable() {
        let b = MacosNativeBackend::new();
        assert_eq!(b.id(), BackendId::MacosNative);
        assert_eq!(b.id().as_str(), "macos-native");
        assert_eq!(b.human_name(), "macOS Native");
    }

    #[test]
    fn capabilities_describe_native_surface() {
        let caps = MacosNativeBackend::new().capabilities();
        assert!(caps.per_key_remap);
        assert_eq!(caps.tap_hold, TapHoldQuality::BestEffort);
        assert!(caps.app_aware);
        assert!(!caps.per_device, "CGEventPost loses device identity");
        assert!(!caps.macros);
        assert!(caps.hot_reload);
    }

    #[test]
    fn idle_backend_reports_stopped() {
        let b = MacosNativeBackend::new();
        assert!(!b.is_running());
        assert_eq!(b.diagnostics().state, "stopped");
    }

    #[test]
    fn teardown_without_apply_is_noop() {
        let b = MacosNativeBackend::new();
        // Should be Ok(()) — teardown of a never-started backend is harmless.
        b.teardown().expect("teardown noop");
        assert!(!b.is_running());
    }

    fn with_software(engine: &str, config: &str) -> ViaProfile {
        use hhkb_core::via::{ProfileMeta, RoninExtension, SoftwareConfig};
        ViaProfile {
            name: "p".into(),
            vendor_id: "0x0".into(),
            product_id: "0x0".into(),
            matrix: None,
            layouts: None,
            layers: vec![],
            lighting: None,
            keycodes: vec![],
            ronin: Some(RoninExtension {
                version: "1".into(),
                profile: ProfileMeta {
                    id: uuid::Uuid::new_v4(),
                    name: "p".into(),
                    icon: None,
                    tags: vec![],
                },
                hardware: None,
                software: Some(SoftwareConfig {
                    engine: engine.to_string(),
                    engine_version: None,
                    config: config.to_string(),
                }),
            }),
        }
    }

    #[test]
    fn build_engine_returns_default_when_profile_is_silent() {
        let p = ViaProfile {
            name: "p".into(),
            vendor_id: "0x0".into(),
            product_id: "0x0".into(),
            matrix: None,
            layouts: None,
            layers: vec![],
            lighting: None,
            keycodes: vec![],
            ronin: None,
        };
        let engine = build_engine_for_profile(&p).expect("default engine");
        // Default = Caps→Ctrl/Esc, so Caps is owned and nothing else.
        assert!(engine.is_owned(HidUsage::CAPS_LOCK));
        assert!(!engine.is_owned(HidUsage::A));
    }

    #[test]
    fn build_engine_accepts_legacy_caps_holdtap_overrides() {
        let p = with_software(
            "macos-native",
            r#"{"caps":{"type":"holdtap","timeout_ms":250,"tap":"esc","hold":"rctl"}}"#,
        );
        let engine = build_engine_for_profile(&p).expect("legacy caps override");
        assert!(engine.is_owned(HidUsage::CAPS_LOCK));
    }

    #[test]
    fn build_engine_accepts_caps_passthrough() {
        let p = with_software("macos-native", r#"{"caps":{"type":"passthrough"}}"#);
        let engine = build_engine_for_profile(&p).expect("caps passthrough");
        // Passthrough means Caps is no longer owned — OS handles it natively.
        assert!(!engine.is_owned(HidUsage::CAPS_LOCK));
    }

    #[test]
    fn build_engine_rejects_unknown_keycode() {
        let p = with_software(
            "macos-native",
            r#"{"caps":{"type":"holdtap","tap":"esc","hold":"meta"}}"#,
        );
        let err = build_engine_for_profile(&p)
            .err()
            .expect("expected ProfileRejected");
        assert!(matches!(err, BackendError::ProfileRejected(_)));
    }

    #[test]
    fn build_engine_ignores_other_engines() {
        // A kanata profile or hidutil profile shouldn't make the native
        // backend complain — the M4 selection layer routes profiles to the
        // matching backend, but the native backend should fall back to its
        // PoC default rather than reject every non-native profile.
        let p = with_software("kanata", "(defcfg)");
        let engine = build_engine_for_profile(&p).expect("ignore non-native engine");
        assert!(engine.is_owned(HidUsage::CAPS_LOCK));
    }

    #[test]
    fn build_engine_supports_per_key_remap_via_bindings_block() {
        // Per-position layout: 'A' remapped to 'B', plus the caps shortcut.
        let p = with_software(
            "macos-native",
            r#"{
                "caps": {"type":"holdtap","tap":"esc","hold":"lctl"},
                "bindings": {
                    "a": {"type":"remap","to":"b"}
                }
            }"#,
        );
        let engine = build_engine_for_profile(&p).expect("per-key bindings");
        assert!(engine.is_owned(HidUsage::CAPS_LOCK), "caps remains owned");
        assert!(engine.is_owned(HidUsage::A), "remap claims source key");
        assert!(
            !engine.is_owned(HidUsage::B),
            "remap target stays passthrough"
        );
    }

    #[test]
    fn build_engine_rejects_unknown_remap_source() {
        let p = with_software(
            "macos-native",
            r#"{"bindings":{"meta":{"type":"remap","to":"a"}}}"#,
        );
        let err = build_engine_for_profile(&p)
            .err()
            .expect("expected ProfileRejected");
        assert!(matches!(err, BackendError::ProfileRejected(_)));
    }
}
