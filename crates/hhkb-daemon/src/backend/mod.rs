//! v0.2.0 Backend abstraction (RFC 0001 §4).
//!
//! A `Backend` is one strategy for turning a [`ViaProfile`] into actual key
//! remapping behaviour on the host. Today (v0.1.x) the daemon hardcodes
//! "kanata + hhkb-core EEPROM"; in v0.2.0 the daemon will own
//! `Vec<Box<dyn Backend>>` and select the highest-priority backend whose
//! permissions are satisfied. M0 introduces the trait without changing
//! behaviour: the existing kanata supervisor + EEPROM device path stay
//! authoritative for routing, and the trait wrappers exist for forward
//! compatibility and trait-conformance tests.
//!
//! See `docs/rfc-0001-macos-native-backend.md` for the full design and
//! `docs/v0.2.0-plan.md` for the milestone breakdown.

pub mod eeprom;
pub mod kanata;
#[cfg(target_os = "macos")]
pub mod macos_native;

use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

use hhkb_core::ViaProfile;

// ---------------------------------------------------------------------------
// Identity + capability surface
// ---------------------------------------------------------------------------

/// Stable, machine-readable backend identifier. Lowercase ASCII; matches the
/// `id` field returned by `/backend/list` once that endpoint lands in M4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendId {
    /// HHKB hardware EEPROM (per-keyboard, durable across reboots).
    Eeprom,
    /// macOS native CGEventTap + IOHIDManager backend (v0.2.0 default on macOS).
    MacosNative,
    /// `hidutil`-driven user key mapping (macOS, persisted via LaunchAgent).
    Hidutil,
    /// Kanata + (Karabiner-DriverKit on macOS / uinput on Linux / Interception
    /// on Windows). Power-user backend — opt-in on macOS in v0.2.0.
    Kanata,
}

impl BackendId {
    pub const fn as_str(self) -> &'static str {
        match self {
            BackendId::Eeprom => "eeprom",
            BackendId::MacosNative => "macos-native",
            BackendId::Hidutil => "hidutil",
            BackendId::Kanata => "kanata",
        }
    }
}

/// What a backend can express. Mirrors the table in RFC 0001 §4.2 — the UI
/// uses this to gray out features the active backend can't honour rather
/// than letting the user think tap-hold works on EEPROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Capabilities {
    /// 1-to-1 keycode override at the per-physical-key level.
    pub per_key_remap: bool,
    /// Number of independent layers the backend can hold simultaneously.
    /// `0` = none, `1` = base only, `>1` = layers.
    pub layers: u8,
    /// Tap-hold quality tier; see [`TapHoldQuality`].
    pub tap_hold: TapHoldQuality,
    /// Leader / sequence keys (vim-style "a, b, c -> something").
    pub leader_keys: bool,
    /// Multi-key chord triggers (combos).
    pub combos: bool,
    /// Per-frontmost-app rules — different layout per running app.
    pub app_aware: bool,
    /// Per-keyboard rules — only one keyboard sees the binding.
    pub per_device: bool,
    /// Survives reboot without the daemon running.
    pub persistent: bool,
    /// Profile change without process restart / dropped modifiers.
    pub hot_reload: bool,
    /// Macros / multi-key strings on a single press.
    pub macros: bool,
    /// Maximum macro length, in keys, when `macros = true`.
    pub max_macro_length: u32,
}

impl Capabilities {
    /// Conservative neutral that says "I can't do anything specialised". Used
    /// as a starting point; concrete backends override fields they support.
    pub const fn none() -> Self {
        Self {
            per_key_remap: false,
            layers: 0,
            tap_hold: TapHoldQuality::None,
            leader_keys: false,
            combos: false,
            app_aware: false,
            per_device: false,
            persistent: false,
            hot_reload: false,
            macros: false,
            max_macro_length: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapHoldQuality {
    /// Backend cannot tap-hold at all (e.g. EEPROM, hidutil).
    None,
    /// CGEventTap-style — 150–250ms latency, best-effort under fast typing.
    BestEffort,
    /// DriverKit-grade — sub-100ms, deterministic.
    DriverGrade,
}

// ---------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------

/// What the backend is currently missing before it can `apply()` anything.
/// `Granted` means it's ready to go; `Required(_)` lists the missing pieces
/// so the UI can render deep-links instead of generic "permission denied".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PermissionStatus {
    Granted,
    Required(Vec<RequiredPermission>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequiredPermission {
    /// macOS `CGPreflightListenEventAccess` returned false.
    InputMonitoring {
        tcc_path: PathBuf,
        deep_link: String,
    },
    /// macOS `AXIsProcessTrustedWithOptions` returned false.
    Accessibility {
        tcc_path: PathBuf,
        deep_link: String,
    },
    /// A driver / system extension is needed but not active. Used by
    /// the kanata backend on macOS for Karabiner-DriverKit.
    SystemExtension {
        bundle_id: String,
        /// Optional CLI invocation that registers the sysext (e.g. the
        /// Karabiner-VirtualHIDDevice-Manager activate command).
        install_command: Option<String>,
    },
    /// User has to take an action that isn't a system permission per se
    /// (e.g. install Karabiner-Elements first).
    UserAction {
        description: String,
        deep_link: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Errors + diagnostics
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("backend not ready: {0:?}")]
    NotReady(Vec<RequiredPermission>),

    #[error("profile rejected by backend: {0}")]
    ProfileRejected(String),

    #[error("backend internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct BackendDiagnostics {
    /// Current backend state ("running" / "stopped" / "not_installed" / etc).
    /// Free-form so each backend can use whatever vocabulary fits.
    pub state: &'static str,
    /// Human-readable note (last error, version string, etc).
    pub note: Option<String>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// One strategy for applying a [`ViaProfile`] to the host. M0 only requires
/// that the trait exists and the existing backends conform to it; later
/// milestones thread it through the AppState + REST routes.
pub trait Backend: Send + Sync {
    /// Stable identifier ("eeprom", "kanata", ...).
    fn id(&self) -> BackendId;

    /// Human-readable display name for the UI.
    fn human_name(&self) -> &'static str;

    /// Capability surface this backend exposes to the UI.
    fn capabilities(&self) -> Capabilities;

    /// What the backend is missing before `apply()` would succeed. Cheap to
    /// call — the daemon polls it on every backend-list request.
    fn permission_status(&self) -> PermissionStatus;

    /// Apply `profile` to the OS / hardware. Idempotent — re-applying the
    /// same profile is a no-op (or as close as the backend can manage).
    fn apply(&self, profile: &ViaProfile) -> Result<(), BackendError>;

    /// Tear down whatever `apply()` created. No-op if not running.
    fn teardown(&self) -> Result<(), BackendError>;

    /// Hot-swap to a new profile without dropping modifiers. The default
    /// implementation is `teardown()` then `apply()`; backends override when
    /// they can do better (kanata SIGUSR1, native backend in-process state
    /// swap).
    fn reload(&self, profile: &ViaProfile) -> Result<(), BackendError> {
        self.teardown()?;
        self.apply(profile)
    }

    /// `true` when the backend is currently driving the keyboard (i.e. has
    /// applied a profile that hasn't been torn down).
    fn is_running(&self) -> bool;

    /// Diagnostic snapshot for `/backend/status`.
    fn diagnostics(&self) -> BackendDiagnostics;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_id_strings_are_stable() {
        // The /backend/list contract depends on these strings; if you rename
        // one, you've broken every shipped frontend.
        assert_eq!(BackendId::Eeprom.as_str(), "eeprom");
        assert_eq!(BackendId::MacosNative.as_str(), "macos-native");
        assert_eq!(BackendId::Hidutil.as_str(), "hidutil");
        assert_eq!(BackendId::Kanata.as_str(), "kanata");
    }

    #[test]
    fn capabilities_none_is_all_false() {
        let c = Capabilities::none();
        assert!(!c.per_key_remap);
        assert_eq!(c.layers, 0);
        assert_eq!(c.tap_hold, TapHoldQuality::None);
        assert!(!c.leader_keys);
        assert!(!c.combos);
        assert!(!c.app_aware);
        assert!(!c.per_device);
        assert!(!c.persistent);
        assert!(!c.hot_reload);
        assert!(!c.macros);
        assert_eq!(c.max_macro_length, 0);
    }
}
