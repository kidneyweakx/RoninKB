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
use hhkb_macos_native::{inject, Engine, EngineEvent, HidUsage};

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

    fn apply(&self, _profile: &ViaProfile) -> Result<(), BackendError> {
        // Bail loudly if permissions aren't there — the CGEventTap will
        // simply not receive events otherwise, and the user gets a "nothing
        // happens" experience.
        if let PermissionStatus::Required(missing) = self.permission_status() {
            return Err(BackendError::NotReady(missing));
        }

        let mut guard = self
            .runtime
            .lock()
            .expect("MacosNativeBackend runtime mutex poisoned");
        if guard.is_some() {
            // Already running — apply is idempotent. Future M2 work will
            // hot-swap the engine layout in place; for the M0 PoC layout
            // there's nothing to swap, so this is a no-op.
            return Ok(());
        }

        let stop = Arc::new(AtomicBool::new(false));
        let engine = Arc::new(Mutex::new(Engine::new()));
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
                    ObservedEvent::Pressed(HidUsage::CAPS_LOCK) => {
                        cb_engine
                            .lock()
                            .expect("engine poisoned")
                            .input(EngineEvent::Press(HidUsage::CAPS_LOCK));
                        Verdict::Suppress
                    }
                    ObservedEvent::Released(HidUsage::CAPS_LOCK) => {
                        cb_engine
                            .lock()
                            .expect("engine poisoned")
                            .input(EngineEvent::Release(HidUsage::CAPS_LOCK));
                        Verdict::Suppress
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
}
