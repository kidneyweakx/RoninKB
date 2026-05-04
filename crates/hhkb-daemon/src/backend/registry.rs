//! `BackendRegistry` — the M4 backend-selection layer.
//!
//! Holds `Vec<Arc<dyn Backend>>` in priority order (RFC 0001 §4.4) plus the
//! currently active backend id. The daemon constructs one of these at
//! startup; the `/backend/*` routes read and mutate it.
//!
//! M4 caveat: the registry coexists with the existing `state.kanata` field.
//! Routes still go through the concrete kanata path for now; the registry
//! exposes the new uniform surface so v0.2.0 clients can switch backends
//! without the daemon having to fully re-route everything in one PR.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::{Backend, BackendDiagnostics, BackendId, Capabilities, PermissionStatus};

#[derive(Debug, Serialize)]
pub struct BackendInfo {
    pub id: BackendId,
    pub human_name: &'static str,
    pub capabilities: Capabilities,
    pub permission_status: PermissionStatus,
    pub diagnostics: BackendDiagnostics,
    /// `true` when this backend is the one the daemon currently dispatches
    /// to. Exactly one backend in `/backend/list` has `is_active = true` at
    /// any time (or none, if the registry is empty / all backends failed
    /// permissions on startup).
    pub is_active: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("backend {0} is not registered")]
    UnknownBackend(BackendId),
}

pub struct BackendRegistry {
    backends: Vec<Arc<dyn Backend>>,
    active: Mutex<Option<BackendId>>,
}

impl BackendRegistry {
    /// Construct a registry from a priority-ordered list of backends. The
    /// first backend with `permission_status() == Granted` is auto-selected
    /// as the initial active. If none qualify, `active` is `None` until the
    /// user explicitly picks one (or grants permissions and the next
    /// `/backend/list` poll re-picks).
    pub fn new(backends: Vec<Arc<dyn Backend>>) -> Self {
        let active = backends.iter().find_map(|b| {
            matches!(b.permission_status(), PermissionStatus::Granted).then(|| b.id())
        });
        Self {
            backends,
            active: Mutex::new(active),
        }
    }

    pub fn list(&self) -> Vec<BackendInfo> {
        let active = self.active();
        self.backends
            .iter()
            .map(|b| BackendInfo {
                id: b.id(),
                human_name: b.human_name(),
                capabilities: b.capabilities(),
                permission_status: b.permission_status(),
                diagnostics: b.diagnostics(),
                is_active: Some(b.id()) == active,
            })
            .collect()
    }

    pub fn active(&self) -> Option<BackendId> {
        *self
            .active
            .lock()
            .expect("BackendRegistry active mutex poisoned")
    }

    /// Look up the active backend handle. `None` when no backend is active
    /// (fresh install with no permissions granted).
    pub fn active_backend(&self) -> Option<Arc<dyn Backend>> {
        let id = self.active()?;
        self.backends.iter().find(|b| b.id() == id).cloned()
    }

    /// Switch the active backend. Does **not** call `apply()` on the new
    /// backend — that's the caller's job, since "select" is independent of
    /// "load this profile". Returns `UnknownBackend` if `id` isn't in the
    /// registry; the caller maps that to a 404.
    pub fn select(&self, id: BackendId) -> Result<(), RegistryError> {
        if !self.backends.iter().any(|b| b.id() == id) {
            return Err(RegistryError::UnknownBackend(id));
        }
        *self
            .active
            .lock()
            .expect("BackendRegistry active mutex poisoned") = Some(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{BackendError, RequiredPermission, TapHoldQuality};
    use hhkb_core::ViaProfile;

    /// Test double: a backend with configurable id + permission state.
    struct StubBackend {
        id: BackendId,
        granted: bool,
    }

    impl Backend for StubBackend {
        fn id(&self) -> BackendId {
            self.id
        }
        fn human_name(&self) -> &'static str {
            "Stub"
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities::none()
        }
        fn permission_status(&self) -> PermissionStatus {
            if self.granted {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Required(vec![RequiredPermission::UserAction {
                    description: "stub".into(),
                    deep_link: None,
                }])
            }
        }
        fn apply(&self, _: &ViaProfile) -> Result<(), BackendError> {
            Ok(())
        }
        fn teardown(&self) -> Result<(), BackendError> {
            Ok(())
        }
        fn is_running(&self) -> bool {
            false
        }
        fn diagnostics(&self) -> BackendDiagnostics {
            BackendDiagnostics {
                state: "stub",
                note: None,
            }
        }
    }

    fn _assert_caps_unused() {
        // Touch TapHoldQuality so the import isn't dead-code in cfg variants.
        let _ = TapHoldQuality::None;
    }

    #[test]
    fn priority_pick_selects_first_granted() {
        let r = BackendRegistry::new(vec![
            Arc::new(StubBackend {
                id: BackendId::Kanata,
                granted: false,
            }),
            Arc::new(StubBackend {
                id: BackendId::MacosNative,
                granted: true,
            }),
            Arc::new(StubBackend {
                id: BackendId::Eeprom,
                granted: true,
            }),
        ]);
        assert_eq!(r.active(), Some(BackendId::MacosNative));
    }

    #[test]
    fn empty_registry_has_no_active() {
        let r = BackendRegistry::new(vec![]);
        assert_eq!(r.active(), None);
        assert!(r.list().is_empty());
    }

    #[test]
    fn select_switches_active_backend() {
        let r = BackendRegistry::new(vec![
            Arc::new(StubBackend {
                id: BackendId::Kanata,
                granted: true,
            }),
            Arc::new(StubBackend {
                id: BackendId::Eeprom,
                granted: true,
            }),
        ]);
        assert_eq!(r.active(), Some(BackendId::Kanata));
        r.select(BackendId::Eeprom).unwrap();
        assert_eq!(r.active(), Some(BackendId::Eeprom));
    }

    #[test]
    fn select_unknown_backend_errors() {
        let r = BackendRegistry::new(vec![Arc::new(StubBackend {
            id: BackendId::Kanata,
            granted: true,
        })]);
        let err = r.select(BackendId::MacosNative).unwrap_err();
        assert!(matches!(err, RegistryError::UnknownBackend(_)));
        // Active stays unchanged on failed select.
        assert_eq!(r.active(), Some(BackendId::Kanata));
    }

    #[test]
    fn list_marks_active_correctly() {
        let r = BackendRegistry::new(vec![
            Arc::new(StubBackend {
                id: BackendId::Kanata,
                granted: true,
            }),
            Arc::new(StubBackend {
                id: BackendId::Eeprom,
                granted: true,
            }),
        ]);
        let list = r.list();
        let active_count = list.iter().filter(|b| b.is_active).count();
        assert_eq!(active_count, 1, "exactly one backend should be active");
        let active = list.iter().find(|b| b.is_active).unwrap();
        assert_eq!(active.id, BackendId::Kanata);
    }
}
