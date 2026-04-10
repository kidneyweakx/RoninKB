//! # hhkb-core
//!
//! Core library for HHKB keyboard HID communication, keymap manipulation,
//! and VIA-compatible profile management.
//!
//! ## Modules
//!
//! - [`protocol`] — request/response framing (magic bytes `0xAA 0xAA` / `0x55 0x55`)
//! - [`command`] — HID command builders and response parsers (CMD `0x01`–`0xE3`)
//! - [`keymap`] — 128-byte keymap parse/serialize (3-chunk HID transfer)
//! - [`transport`] — `HidTransport` trait + `MockTransport` for testing
//! - [`device`] — high-level [`device::HhkbDevice`] API
//! - [`via`] — VIA JSON profile format with `_roninKB` extension
//! - [`types`] — `KeyboardMode`, `KeyboardInfo`, `DipSwitchState`, constants
//! - [`error`] — `Error` enum and `Result` alias
//!
//! ## Quick start
//!
//! ```ignore
//! use hhkb_core::device::HhkbDevice;
//! use hhkb_core::transport::HidTransport;
//! use hhkb_core::types::KeyboardMode;
//!
//! let transport: Box<dyn HidTransport> = /* hidapi or WebHID impl */;
//! let dev = HhkbDevice::new(transport);
//! dev.open_session()?;
//! let keymap = dev.read_keymap(KeyboardMode::Mac, false)?;
//! dev.close_session()?;
//! ```

pub mod command;
pub mod device;
pub mod error;
pub mod keymap;
pub mod protocol;
pub mod transport;
pub mod types;
pub mod via;

// Re-export the most common types for convenience
pub use error::{Error, Result};
pub use keymap::Keymap;
pub use types::{
    DipSwitchState, FirmwareType, KeyboardInfo, KeyboardMode, HHKB_PRODUCT_IDS, HHKB_VENDOR_ID,
    VENDOR_INTERFACE,
};
pub use via::ViaProfile;
