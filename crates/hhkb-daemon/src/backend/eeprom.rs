//! `EepromBackend` — wraps the HHKB hardware EEPROM as a `Backend`.
//!
//! EEPROM is the most durable surface RoninKB has: a 128-byte keymap (× 2
//! layers, base + Fn) per keyboard mode, written over USB-HID and surviving
//! both reboot and re-attach to a different host. It cannot tap-hold, layer
//! beyond `Base + Fn`, or do anything app-aware — but everything it *does*
//! do needs zero permissions and zero running daemon.
//!
//! This trait impl exists so M2/M4 can route hardware-targeted bindings
//! through the same `Backend` plumbing as software backends. M0 ships the
//! shape; the daemon's existing `routes/keymap.rs` keeps writing keymaps
//! directly until the migration completes.

use std::sync::Arc;

use hhkb_core::keymap::{Keymap, KEYMAP_SIZE};
use hhkb_core::types::KeyboardMode;
use hhkb_core::ViaProfile;
use tokio::sync::Mutex;

use crate::state::DeviceHandle;

use super::{
    Backend, BackendDiagnostics, BackendError, BackendId, Capabilities, PermissionStatus,
    TapHoldQuality,
};

pub struct EepromBackend {
    device: Arc<Mutex<<DeviceHandle as DeviceHandleAlias>::Inner>>,
}

// Helper trait so the impl can name `DeviceHandle`'s inner type without
// pulling its concrete generics into this file. `DeviceHandle` is
// `Arc<tokio::sync::Mutex<Option<HhkbDevice<HidApiTransport>>>>`; we just
// want the inner `Option<...>` part.
trait DeviceHandleAlias {
    type Inner;
}
impl<T> DeviceHandleAlias for Arc<Mutex<T>> {
    type Inner = T;
}

impl EepromBackend {
    pub fn new(device: DeviceHandle) -> Self {
        Self { device }
    }
}

impl Backend for EepromBackend {
    fn id(&self) -> BackendId {
        BackendId::Eeprom
    }

    fn human_name(&self) -> &'static str {
        "HHKB Hardware (EEPROM)"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            per_key_remap: true,
            // HHKB Pro Hybrid stores Base + Fn per keyboard mode (HHK / Mac /
            // Lite / Secret). The Backend abstraction surfaces the layer count
            // the user can target with one profile load — that's 2.
            layers: 2,
            tap_hold: TapHoldQuality::None,
            leader_keys: false,
            combos: false,
            app_aware: false,
            // EEPROM is per-keyboard by definition: each unit stores its own
            // keymap. UI uses this to advertise "this binding moves with the
            // keyboard, even on a different host".
            per_device: true,
            persistent: true,
            // Hardware writes are immediate — no process restart, no flash
            // dance. The keyboard simply reads the new mapping from EEPROM.
            hot_reload: true,
            macros: false,
            max_macro_length: 0,
        }
    }

    fn permission_status(&self) -> PermissionStatus {
        // EEPROM needs zero OS permissions. Linux's udev rule is a one-time
        // install handled outside the daemon (and is already in v0.1.x), so
        // the trait surface treats it as Granted unconditionally.
        PermissionStatus::Granted
    }

    fn apply(&self, profile: &ViaProfile) -> Result<(), BackendError> {
        let raw = profile
            .ronin
            .as_ref()
            .and_then(|r| r.hardware.as_ref())
            .ok_or_else(|| {
                BackendError::ProfileRejected(
                    "profile has no _roninKB.hardware section to apply to EEPROM".to_string(),
                )
            })?;

        let mode = KeyboardMode::try_from(raw.keyboard_mode).map_err(|e| {
            BackendError::ProfileRejected(format!(
                "invalid keyboard_mode {}: {e}",
                raw.keyboard_mode
            ))
        })?;

        let base = bytes_to_keymap(&raw.raw_layers.base, "base")?;
        let fn_layer = bytes_to_keymap(&raw.raw_layers.r#fn, "fn")?;

        // tokio::sync::Mutex#blocking_lock is safe from a sync context as long
        // as we're not on a tokio runtime thread; Backend::apply is invoked
        // from spawn_blocking by the eventual M4 wiring, so this is fine.
        let mut guard = self.device.blocking_lock();
        let device = guard.as_mut().ok_or_else(|| {
            BackendError::Internal("HHKB device not connected (USB unplugged?)".to_string())
        })?;

        device.open_session().map_err(io_to_internal)?;
        let write_result = (|| {
            device.write_keymap(mode, false, &base)?;
            device.write_keymap(mode, true, &fn_layer)
        })();
        // close_session even when write failed — we don't want the keyboard
        // stuck mid-session if a single packet errored.
        let close_result = device.close_session();
        write_result.map_err(io_to_internal)?;
        close_result.map_err(io_to_internal)?;
        Ok(())
    }

    fn teardown(&self) -> Result<(), BackendError> {
        // EEPROM has no "running" concept to tear down — once you write a
        // keymap, that's the new state. To "undo", apply a different profile.
        Ok(())
    }

    fn is_running(&self) -> bool {
        // EEPROM-stored bindings are always live; the keyboard's firmware is
        // doing the lookup, with or without the daemon. From the user's POV
        // EEPROM is "running" any time the keyboard is plugged in.
        true
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        let connected = self
            .device
            .try_lock()
            .ok()
            .map(|g| g.is_some())
            .unwrap_or(false);
        BackendDiagnostics {
            state: if connected {
                "connected"
            } else {
                "disconnected"
            },
            note: None,
        }
    }
}

fn bytes_to_keymap(bytes: &[u8], which: &str) -> Result<Keymap, BackendError> {
    if bytes.len() != KEYMAP_SIZE {
        return Err(BackendError::ProfileRejected(format!(
            "{which} layer has {} bytes, expected {}",
            bytes.len(),
            KEYMAP_SIZE
        )));
    }
    let mut arr = [0u8; KEYMAP_SIZE];
    arr.copy_from_slice(bytes);
    Ok(Keymap::from_bytes(arr))
}

fn io_to_internal(e: hhkb_core::Error) -> BackendError {
    BackendError::Internal(format!("hhkb-core: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> EepromBackend {
        // Empty device handle — `apply` will return DeviceUnavailable, which
        // is what we want to assert in the trait-conformance suite.
        EepromBackend::new(Arc::new(Mutex::new(None)))
    }

    #[test]
    fn id_and_capabilities_are_stable() {
        let b = fixture();
        assert_eq!(b.id(), BackendId::Eeprom);
        assert_eq!(b.id().as_str(), "eeprom");

        let caps = b.capabilities();
        assert!(caps.per_key_remap);
        assert_eq!(caps.layers, 2);
        assert!(caps.persistent);
        assert!(caps.per_device);
        assert!(!caps.macros);
    }

    #[test]
    fn permissions_are_always_granted() {
        let b = fixture();
        assert_eq!(b.permission_status(), PermissionStatus::Granted);
    }

    #[test]
    fn diagnostics_reports_disconnected_when_no_device() {
        assert_eq!(fixture().diagnostics().state, "disconnected");
    }

    #[test]
    fn apply_without_ronin_hardware_section_is_rejected() {
        let b = fixture();
        let pure_via = hhkb_core::ViaProfile {
            name: "pure VIA".into(),
            vendor_id: "0x04FE".into(),
            product_id: "0x0011".into(),
            matrix: None,
            layouts: None,
            layers: vec![],
            lighting: None,
            keycodes: vec![],
            ronin: None,
        };
        let err = b.apply(&pure_via).unwrap_err();
        assert!(
            matches!(err, BackendError::ProfileRejected(_)),
            "expected ProfileRejected, got {err:?}"
        );
    }
}
