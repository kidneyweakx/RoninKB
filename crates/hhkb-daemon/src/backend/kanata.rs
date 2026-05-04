//! `KanataBackend` — wraps the existing `KanataManager` as a `Backend`.
//!
//! M0 doesn't change behaviour; this trait impl exists so future code paths
//! (the M4 `/backend/select` endpoint, the M2 native backend swap) can
//! treat all backends uniformly. The supervisor itself is unchanged — see
//! `crate::kanata` for the actual process-management code.

use std::path::PathBuf;
use std::sync::Arc;

use hhkb_core::ViaProfile;

use crate::kanata::{DriverState, KanataManager, KanataStatus};
use crate::kanata_config;

use super::{
    Backend, BackendDiagnostics, BackendError, BackendId, Capabilities, PermissionStatus,
    RequiredPermission, TapHoldQuality,
};

/// Karabiner DriverKit sysext bundle id, used in the `SystemExtension`
/// permission entry. Mirrors the constant used inside `crate::kanata` so the
/// UI sees the same identifier in both places.
const KARABINER_BUNDLE_ID: &str = "org.pqrs.Karabiner-DriverKit-VirtualHIDDevice";

/// Karabiner's bundled CLI to register / re-arm the sysext. `None` when the
/// bundle isn't installed; the UI uses this to decide between an "Activate"
/// button and "Get Karabiner-Elements" link.
const KARABINER_ACTIVATE_CMD: &str =
    "/Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/\
     Karabiner-VirtualHIDDevice-Manager activate";

/// Deep-link to the Driver Extensions pane in macOS System Settings.
const DRIVER_EXT_DEEP_LINK: &str =
    "x-apple.systempreferences:com.apple.LoginItems-Settings.extension?\
     extensionPointIdentifier=com.apple.system_extension.driver_extension";

pub struct KanataBackend {
    manager: Arc<KanataManager>,
}

impl KanataBackend {
    pub fn new(manager: Arc<KanataManager>) -> Self {
        Self { manager }
    }
}

impl Backend for KanataBackend {
    fn id(&self) -> BackendId {
        BackendId::Kanata
    }

    fn human_name(&self) -> &'static str {
        "Kanata"
    }

    fn capabilities(&self) -> Capabilities {
        // Kanata is the most expressive backend — full feature set, with
        // DriverKit-grade tap-hold on macOS (when Karabiner is present) and
        // uinput-grade on Linux. We report the optimistic capability set;
        // platform-specific qualifiers (per-device on macOS only) are
        // documented in `permission_status` and `diagnostics`.
        Capabilities {
            per_key_remap: true,
            layers: u8::MAX,
            tap_hold: TapHoldQuality::DriverGrade,
            leader_keys: true,
            combos: true,
            app_aware: false, // kanata's main weakness vs the native backend
            per_device: cfg!(target_os = "macos"),
            persistent: false,
            hot_reload: true,
            macros: true,
            max_macro_length: 1024,
        }
    }

    fn permission_status(&self) -> PermissionStatus {
        let mut required = Vec::new();

        if !self.manager.is_installed() {
            required.push(RequiredPermission::UserAction {
                description: "Kanata binary is not installed. Use the bundled-kanata \
                              feature, install via Cargo, or place the binary on PATH."
                    .to_string(),
                deep_link: None,
            });
        }

        // macOS — Karabiner DriverKit sysext + Input Monitoring.
        #[cfg(target_os = "macos")]
        {
            match self.manager.driver_state() {
                DriverState::Activated | DriverState::Unknown => {}
                DriverState::WaitingForUser | DriverState::NotRegistered => {
                    required.push(RequiredPermission::SystemExtension {
                        bundle_id: KARABINER_BUNDLE_ID.to_string(),
                        install_command: Some(KARABINER_ACTIVATE_CMD.to_string()),
                    });
                }
                DriverState::KarabinerNotInstalled => {
                    required.push(RequiredPermission::UserAction {
                        description:
                            "Karabiner-Elements is required for the kanata backend on macOS \
                             (it ships the DriverKit sysext kanata grabs the keyboard with)."
                                .to_string(),
                        deep_link: Some("https://karabiner-elements.pqrs.org/".to_string()),
                    });
                }
            }

            if self.manager.input_monitoring_granted().is_some_and(|g| !g) {
                required.push(RequiredPermission::InputMonitoring {
                    tcc_path: PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db"),
                    deep_link: DRIVER_EXT_DEEP_LINK.to_string(),
                });
            }
        }

        if required.is_empty() {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Required(required)
        }
    }

    fn apply(&self, profile: &ViaProfile) -> Result<(), BackendError> {
        let cfg = kanata_config::derive_profile_kanata_config(profile)
            .map_err(|e| BackendError::ProfileRejected(format!("derive kanata config: {e}")))?
            .unwrap_or_else(|| kanata_config::default_minimal_config(60));

        kanata_config::validate_kanata_config(&cfg)
            .map_err(|e| BackendError::ProfileRejected(format!("invalid kanata config: {e}")))?;

        self.manager
            .write_config(&cfg)
            .map_err(|e| BackendError::Internal(format!("write kanata config: {e}")))?;

        // Apply means "make the profile take effect"; if kanata is already
        // running, a SIGUSR1-style hot reload is sufficient. If it's not
        // running, start it. Either way the daemon is now driving keys with
        // the new config.
        match self.manager.status() {
            KanataStatus::Running { .. } => self
                .manager
                .reload(&cfg)
                .map_err(|e| BackendError::Internal(format!("reload kanata: {e}"))),
            _ => self
                .manager
                .start()
                .map(|_pid| ())
                .map_err(|e| BackendError::Internal(format!("start kanata: {e}"))),
        }
    }

    fn teardown(&self) -> Result<(), BackendError> {
        if matches!(self.manager.status(), KanataStatus::Running { .. }) {
            self.manager
                .stop()
                .map_err(|e| BackendError::Internal(format!("stop kanata: {e}")))?;
        }
        Ok(())
    }

    fn reload(&self, profile: &ViaProfile) -> Result<(), BackendError> {
        // Override the default teardown+apply: kanata supports SIGUSR1
        // in-place reload that keeps modifier state intact.
        let cfg = kanata_config::derive_profile_kanata_config(profile)
            .map_err(|e| BackendError::ProfileRejected(format!("derive kanata config: {e}")))?
            .unwrap_or_else(|| kanata_config::default_minimal_config(60));
        kanata_config::validate_kanata_config(&cfg)
            .map_err(|e| BackendError::ProfileRejected(format!("invalid kanata config: {e}")))?;
        self.manager
            .reload(&cfg)
            .map_err(|e| BackendError::Internal(format!("reload kanata: {e}")))
    }

    fn is_running(&self) -> bool {
        matches!(self.manager.status(), KanataStatus::Running { .. })
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        let state = match self.manager.status() {
            KanataStatus::NotInstalled => "not_installed",
            KanataStatus::Stopped => "stopped",
            KanataStatus::Running { .. } => "running",
        };
        BackendDiagnostics {
            state,
            note: self.manager.last_error(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn fixture() -> KanataBackend {
        // No-binary, temp-config manager. Deterministic across machines so
        // CI always sees `not_installed`.
        let cfg = std::env::temp_dir().join(format!(
            "roninKB-kanata-trait-test-{}.kbd",
            std::process::id(),
        ));
        let mgr = Arc::new(KanataManager::with_paths(None, cfg));
        KanataBackend::new(mgr)
    }

    #[test]
    fn id_and_name_are_stable() {
        let b = fixture();
        assert_eq!(b.id(), BackendId::Kanata);
        assert_eq!(b.id().as_str(), "kanata");
        assert_eq!(b.human_name(), "Kanata");
    }

    #[test]
    fn missing_binary_is_reported_as_required_user_action() {
        let b = fixture();
        let status = b.permission_status();
        let PermissionStatus::Required(items) = status else {
            panic!("expected Required, got Granted");
        };
        assert!(
            items.iter().any(|p| matches!(
                p,
                RequiredPermission::UserAction { description, .. }
                    if description.contains("Kanata binary is not installed")
            )),
            "missing binary should appear in permission_status: {items:?}"
        );
    }

    #[test]
    fn diagnostics_state_matches_status() {
        let b = fixture();
        // No binary => not_installed
        let d = b.diagnostics();
        assert_eq!(d.state, "not_installed");
    }

    #[test]
    fn capabilities_describe_full_kanata_surface() {
        let caps = fixture().capabilities();
        // The headline features that distinguish kanata from EEPROM/hidutil.
        assert!(caps.per_key_remap);
        assert!(caps.tap_hold == TapHoldQuality::DriverGrade);
        assert!(caps.macros);
        assert!(caps.hot_reload);
    }
}
