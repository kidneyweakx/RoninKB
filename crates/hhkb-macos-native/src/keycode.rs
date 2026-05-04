//! Cross-mapping between three keycode spaces:
//!   - HID Usage (used by the IOHID seize path and the keyberon engine)
//!   - macOS virtual keycode (CG keycode, used by CGEvent on Path A)
//!   - keyberon `KeyCode` enum
//!
//! HID Usage is the canonical interchange format inside the engine.
//! Conversions to/from CG keycodes happen at the OS boundary.

use kanata_keyberon::key_code::KeyCode;

/// HID Usage (page 0x07 = Keyboard/Keypad).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HidUsage(pub u16);

impl HidUsage {
    pub const A: Self = Self(0x04);
    pub const B: Self = Self(0x05);
    pub const ESC: Self = Self(0x29);
    pub const CAPS_LOCK: Self = Self(0x39);
    pub const LEFT_CTRL: Self = Self(0xE0);
    pub const LEFT_SHIFT: Self = Self(0xE1);
    pub const LEFT_ALT: Self = Self(0xE2);
    pub const LEFT_GUI: Self = Self(0xE3);

    pub fn to_keyberon(self) -> Option<KeyCode> {
        Some(match self.0 {
            0x04 => KeyCode::A,
            0x05 => KeyCode::B,
            0x06 => KeyCode::C,
            0x07 => KeyCode::D,
            0x08 => KeyCode::E,
            0x09 => KeyCode::F,
            0x0A => KeyCode::G,
            0x0B => KeyCode::H,
            0x0C => KeyCode::I,
            0x0D => KeyCode::J,
            0x0E => KeyCode::K,
            0x0F => KeyCode::L,
            0x10 => KeyCode::M,
            0x11 => KeyCode::N,
            0x12 => KeyCode::O,
            0x13 => KeyCode::P,
            0x14 => KeyCode::Q,
            0x15 => KeyCode::R,
            0x16 => KeyCode::S,
            0x17 => KeyCode::T,
            0x18 => KeyCode::U,
            0x19 => KeyCode::V,
            0x1A => KeyCode::W,
            0x1B => KeyCode::X,
            0x1C => KeyCode::Y,
            0x1D => KeyCode::Z,
            0x28 => KeyCode::Enter,
            0x29 => KeyCode::Escape,
            0x2A => KeyCode::BSpace,
            0x2B => KeyCode::Tab,
            0x2C => KeyCode::Space,
            0x39 => KeyCode::CapsLock,
            0xE0 => KeyCode::LCtrl,
            0xE1 => KeyCode::LShift,
            0xE2 => KeyCode::LAlt,
            0xE3 => KeyCode::LGui,
            0xE4 => KeyCode::RCtrl,
            0xE5 => KeyCode::RShift,
            0xE6 => KeyCode::RAlt,
            0xE7 => KeyCode::RGui,
            _ => return None,
        })
    }
}

impl From<KeyCode> for HidUsage {
    fn from(kc: KeyCode) -> Self {
        // kanata-keyberon's KeyCode enum is ordered by Linux input-event code,
        // NOT by HID usage. Convert explicitly per variant.
        Self(match kc {
            KeyCode::A => 0x04,
            KeyCode::B => 0x05,
            KeyCode::C => 0x06,
            KeyCode::D => 0x07,
            KeyCode::E => 0x08,
            KeyCode::F => 0x09,
            KeyCode::G => 0x0A,
            KeyCode::H => 0x0B,
            KeyCode::I => 0x0C,
            KeyCode::J => 0x0D,
            KeyCode::K => 0x0E,
            KeyCode::L => 0x0F,
            KeyCode::M => 0x10,
            KeyCode::N => 0x11,
            KeyCode::O => 0x12,
            KeyCode::P => 0x13,
            KeyCode::Q => 0x14,
            KeyCode::R => 0x15,
            KeyCode::S => 0x16,
            KeyCode::T => 0x17,
            KeyCode::U => 0x18,
            KeyCode::V => 0x19,
            KeyCode::W => 0x1A,
            KeyCode::X => 0x1B,
            KeyCode::Y => 0x1C,
            KeyCode::Z => 0x1D,
            KeyCode::Kb1 => 0x1E,
            KeyCode::Kb2 => 0x1F,
            KeyCode::Kb3 => 0x20,
            KeyCode::Kb4 => 0x21,
            KeyCode::Kb5 => 0x22,
            KeyCode::Kb6 => 0x23,
            KeyCode::Kb7 => 0x24,
            KeyCode::Kb8 => 0x25,
            KeyCode::Kb9 => 0x26,
            KeyCode::Kb0 => 0x27,
            KeyCode::Enter => 0x28,
            KeyCode::Escape => 0x29,
            KeyCode::BSpace => 0x2A,
            KeyCode::Tab => 0x2B,
            KeyCode::Space => 0x2C,
            KeyCode::Minus => 0x2D,
            KeyCode::Equal => 0x2E,
            KeyCode::LBracket => 0x2F,
            KeyCode::RBracket => 0x30,
            KeyCode::Bslash => 0x31,
            KeyCode::SColon => 0x33,
            KeyCode::Quote => 0x34,
            KeyCode::Grave => 0x35,
            KeyCode::Comma => 0x36,
            KeyCode::Dot => 0x37,
            KeyCode::Slash => 0x38,
            KeyCode::CapsLock => 0x39,
            KeyCode::LCtrl => 0xE0,
            KeyCode::LShift => 0xE1,
            KeyCode::LAlt => 0xE2,
            KeyCode::LGui => 0xE3,
            KeyCode::RCtrl => 0xE4,
            KeyCode::RShift => 0xE5,
            KeyCode::RAlt => 0xE6,
            KeyCode::RGui => 0xE7,
            // Pass anything else through as 0 — unknown to the PoC. The OS
            // layer drops zeros without injecting.
            _ => 0,
        })
    }
}

/// macOS virtual keycode (CGKeyCode). Used by CGEvent on Path A.
///
/// These are the legacy ADB-derived numbers used by `CGEventCreateKeyboardEvent`.
/// Mapping is sparse on purpose — only the keys we actually use in the PoC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CgKeyCode(pub u16);

impl CgKeyCode {
    pub const A: Self = Self(0x00);
    pub const B: Self = Self(0x0B);
    pub const C: Self = Self(0x08);
    pub const ESC: Self = Self(0x35);
    pub const CAPS_LOCK: Self = Self(0x39);
    pub const LEFT_CTRL: Self = Self(0x3B);
    pub const LEFT_SHIFT: Self = Self(0x38);
    pub const LEFT_ALT: Self = Self(0x3A);
    pub const LEFT_GUI: Self = Self(0x37);
}

/// HID usage → macOS virtual keycode for the keys the PoC uses.
pub fn hid_to_cg(hid: HidUsage) -> Option<CgKeyCode> {
    Some(match hid.0 {
        0x04 => CgKeyCode::A,
        0x05 => CgKeyCode::B,
        0x06 => CgKeyCode::C,
        0x29 => CgKeyCode::ESC,
        0x39 => CgKeyCode::CAPS_LOCK,
        0xE0 => CgKeyCode::LEFT_CTRL,
        0xE1 => CgKeyCode::LEFT_SHIFT,
        0xE2 => CgKeyCode::LEFT_ALT,
        0xE3 => CgKeyCode::LEFT_GUI,
        _ => return None,
    })
}

/// macOS virtual keycode → HID usage. Reverse of `hid_to_cg`.
pub fn cg_to_hid(cg: CgKeyCode) -> Option<HidUsage> {
    Some(match cg.0 {
        0x00 => HidUsage::A,
        0x0B => HidUsage::B,
        0x08 => HidUsage(0x06), // C
        0x35 => HidUsage::ESC,
        0x39 => HidUsage::CAPS_LOCK,
        0x3B => HidUsage::LEFT_CTRL,
        0x38 => HidUsage::LEFT_SHIFT,
        0x3A => HidUsage::LEFT_ALT,
        0x37 => HidUsage::LEFT_GUI,
        _ => return None,
    })
}
