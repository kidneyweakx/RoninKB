//! macOS native keyboard remapper backend for RoninKB (M1 PoC).
//!
//! This crate is the spike implementation for the v0.2.0 native backend,
//! per `docs/rfc-0001-macos-native-backend.md`. It is intentionally narrow:
//! enough surface to evaluate the kill-switch criteria (tap-hold latency,
//! reliability under fast typing) before committing to the full M2 build.
//!
//! Platform support:
//! - The engine (`engine`, `profile`, `keycode`, `error`) builds everywhere
//!   so unit tests run in CI without macOS hardware.
//! - The OS event sources (`event_tap`, `iohid_seize`, `inject`) are
//!   `cfg(target_os = "macos")` and only compile on macOS.

pub mod engine;
pub mod error;
pub mod keycode;
pub mod profile;

#[cfg(target_os = "macos")]
pub mod event_tap;

#[cfg(target_os = "macos")]
pub mod inject;

#[cfg(target_os = "macos")]
pub mod iohid_seize;

pub use engine::{Engine, EngineEvent, EngineOutput};
pub use error::{Error, Result};
pub use keycode::HidUsage;
pub use profile::{caps_ctrl_esc_layout, PocBinding};
