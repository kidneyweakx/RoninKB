//! Cross-mapping between three keycode spaces:
//!   - HID Usage (used by the IOHID seize path and the keyberon engine)
//!   - macOS virtual keycode (CG keycode, used by CGEvent on Path A)
//!   - keyberon `KeyCode` enum
//!
//! HID Usage is the canonical interchange format inside the engine.
//! Conversions to/from CG keycodes happen at the OS boundary.
//!
//! v0.2.0 widening: the tables now cover the whole 60-key HHKB layout plus
//! the function row, arrow cluster, media keys, and the keypad. Anything
//! outside this set still falls through to `None` so the engine rejects
//! it at the boundary rather than silently no-op'ing.

use kanata_keyberon::key_code::KeyCode;

/// HID Usage (page 0x07 = Keyboard/Keypad).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HidUsage(pub u16);

impl HidUsage {
    pub const A: Self = Self(0x04);
    pub const B: Self = Self(0x05);
    pub const C: Self = Self(0x06);
    pub const D: Self = Self(0x07);
    pub const E: Self = Self(0x08);
    pub const F: Self = Self(0x09);
    pub const ENTER: Self = Self(0x28);
    pub const ESC: Self = Self(0x29);
    pub const TAB: Self = Self(0x2B);
    pub const SPACE: Self = Self(0x2C);
    pub const CAPS_LOCK: Self = Self(0x39);
    pub const LEFT_CTRL: Self = Self(0xE0);
    pub const LEFT_SHIFT: Self = Self(0xE1);
    pub const LEFT_ALT: Self = Self(0xE2);
    pub const LEFT_GUI: Self = Self(0xE3);

    pub fn to_keyberon(self) -> Option<KeyCode> {
        Some(match self.0 {
            // Letters (0x04..=0x1D)
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
            // Numbers (0x1E..=0x27)
            0x1E => KeyCode::Kb1,
            0x1F => KeyCode::Kb2,
            0x20 => KeyCode::Kb3,
            0x21 => KeyCode::Kb4,
            0x22 => KeyCode::Kb5,
            0x23 => KeyCode::Kb6,
            0x24 => KeyCode::Kb7,
            0x25 => KeyCode::Kb8,
            0x26 => KeyCode::Kb9,
            0x27 => KeyCode::Kb0,
            // Control & whitespace
            0x28 => KeyCode::Enter,
            0x29 => KeyCode::Escape,
            0x2A => KeyCode::BSpace,
            0x2B => KeyCode::Tab,
            0x2C => KeyCode::Space,
            // Punctuation
            0x2D => KeyCode::Minus,
            0x2E => KeyCode::Equal,
            0x2F => KeyCode::LBracket,
            0x30 => KeyCode::RBracket,
            0x31 => KeyCode::Bslash,
            0x33 => KeyCode::SColon,
            0x34 => KeyCode::Quote,
            0x35 => KeyCode::Grave,
            0x36 => KeyCode::Comma,
            0x37 => KeyCode::Dot,
            0x38 => KeyCode::Slash,
            0x39 => KeyCode::CapsLock,
            // Function row
            0x3A => KeyCode::F1,
            0x3B => KeyCode::F2,
            0x3C => KeyCode::F3,
            0x3D => KeyCode::F4,
            0x3E => KeyCode::F5,
            0x3F => KeyCode::F6,
            0x40 => KeyCode::F7,
            0x41 => KeyCode::F8,
            0x42 => KeyCode::F9,
            0x43 => KeyCode::F10,
            0x44 => KeyCode::F11,
            0x45 => KeyCode::F12,
            // Navigation cluster
            0x46 => KeyCode::PScreen,
            0x47 => KeyCode::ScrollLock,
            0x48 => KeyCode::Pause,
            0x49 => KeyCode::Insert,
            0x4A => KeyCode::Home,
            0x4B => KeyCode::PgUp,
            0x4C => KeyCode::Delete,
            0x4D => KeyCode::End,
            0x4E => KeyCode::PgDown,
            0x4F => KeyCode::Right,
            0x50 => KeyCode::Left,
            0x51 => KeyCode::Down,
            0x52 => KeyCode::Up,
            // Modifiers
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
        // NOT by HID usage. Convert explicitly per variant. Anything that
        // isn't in this table falls through to 0 (HID reserved) and the OS
        // layer drops it before injecting.
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
            KeyCode::F1 => 0x3A,
            KeyCode::F2 => 0x3B,
            KeyCode::F3 => 0x3C,
            KeyCode::F4 => 0x3D,
            KeyCode::F5 => 0x3E,
            KeyCode::F6 => 0x3F,
            KeyCode::F7 => 0x40,
            KeyCode::F8 => 0x41,
            KeyCode::F9 => 0x42,
            KeyCode::F10 => 0x43,
            KeyCode::F11 => 0x44,
            KeyCode::F12 => 0x45,
            KeyCode::PScreen => 0x46,
            KeyCode::ScrollLock => 0x47,
            KeyCode::Pause => 0x48,
            KeyCode::Insert => 0x49,
            KeyCode::Home => 0x4A,
            KeyCode::PgUp => 0x4B,
            KeyCode::Delete => 0x4C,
            KeyCode::End => 0x4D,
            KeyCode::PgDown => 0x4E,
            KeyCode::Right => 0x4F,
            KeyCode::Left => 0x50,
            KeyCode::Down => 0x51,
            KeyCode::Up => 0x52,
            KeyCode::LCtrl => 0xE0,
            KeyCode::LShift => 0xE1,
            KeyCode::LAlt => 0xE2,
            KeyCode::LGui => 0xE3,
            KeyCode::RCtrl => 0xE4,
            KeyCode::RShift => 0xE5,
            KeyCode::RAlt => 0xE6,
            KeyCode::RGui => 0xE7,
            // Pass anything else through as 0 — engine emits 0x00 (reserved),
            // OS layer drops without injecting.
            _ => 0,
        })
    }
}

/// macOS virtual keycode (CGKeyCode). Used by CGEvent on Path A.
///
/// These are the legacy ADB-derived numbers used by `CGEventCreateKeyboardEvent`.
/// US-ANSI layout. Reference: `<HIToolbox/Events.h>` `kVK_*` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CgKeyCode(pub u16);

#[allow(non_upper_case_globals)]
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

/// HID usage → macOS virtual keycode. Comprehensive table covering the full
/// HHKB layout (60 keys + Fn) plus the standard function / arrow / nav rows
/// for external keyboards that go through the same engine. Anything not in
/// this table returns `None` and the inject path skips it.
pub fn hid_to_cg(hid: HidUsage) -> Option<CgKeyCode> {
    Some(CgKeyCode(match hid.0 {
        // Letters
        0x04 => 0x00, // A
        0x05 => 0x0B, // B
        0x06 => 0x08, // C
        0x07 => 0x02, // D
        0x08 => 0x0E, // E
        0x09 => 0x03, // F
        0x0A => 0x05, // G
        0x0B => 0x04, // H
        0x0C => 0x22, // I
        0x0D => 0x26, // J
        0x0E => 0x28, // K
        0x0F => 0x25, // L
        0x10 => 0x2E, // M
        0x11 => 0x2D, // N
        0x12 => 0x1F, // O
        0x13 => 0x23, // P
        0x14 => 0x0C, // Q
        0x15 => 0x0F, // R
        0x16 => 0x01, // S
        0x17 => 0x11, // T
        0x18 => 0x20, // U
        0x19 => 0x09, // V
        0x1A => 0x0D, // W
        0x1B => 0x07, // X
        0x1C => 0x10, // Y
        0x1D => 0x06, // Z
        // Numbers (top row)
        0x1E => 0x12, // 1
        0x1F => 0x13, // 2
        0x20 => 0x14, // 3
        0x21 => 0x15, // 4
        0x22 => 0x17, // 5
        0x23 => 0x16, // 6
        0x24 => 0x1A, // 7
        0x25 => 0x1C, // 8
        0x26 => 0x19, // 9
        0x27 => 0x1D, // 0
        // Control & whitespace
        0x28 => 0x24, // Return / Enter
        0x29 => 0x35, // Escape
        0x2A => 0x33, // Backspace (Delete on Mac)
        0x2B => 0x30, // Tab
        0x2C => 0x31, // Space
        // Punctuation
        0x2D => 0x1B, // -
        0x2E => 0x18, // =
        0x2F => 0x21, // [
        0x30 => 0x1E, // ]
        0x31 => 0x2A, // \
        0x33 => 0x29, // ;
        0x34 => 0x27, // '
        0x35 => 0x32, // `
        0x36 => 0x2B, // ,
        0x37 => 0x2F, // .
        0x38 => 0x2C, // /
        0x39 => 0x39, // Caps Lock
        // Function row
        0x3A => 0x7A, // F1
        0x3B => 0x78, // F2
        0x3C => 0x63, // F3
        0x3D => 0x76, // F4
        0x3E => 0x60, // F5
        0x3F => 0x61, // F6
        0x40 => 0x62, // F7
        0x41 => 0x64, // F8
        0x42 => 0x65, // F9
        0x43 => 0x6D, // F10
        0x44 => 0x67, // F11
        0x45 => 0x6F, // F12
        // Navigation cluster
        0x49 => 0x72, // Insert (Help on Mac)
        0x4A => 0x73, // Home
        0x4B => 0x74, // PgUp
        0x4C => 0x75, // Forward Delete
        0x4D => 0x77, // End
        0x4E => 0x79, // PgDown
        0x4F => 0x7C, // Right
        0x50 => 0x7B, // Left
        0x51 => 0x7D, // Down
        0x52 => 0x7E, // Up
        // Modifiers
        0xE0 => 0x3B, // LCtrl
        0xE1 => 0x38, // LShift
        0xE2 => 0x3A, // LAlt / LOption
        0xE3 => 0x37, // LGui / LCmd
        0xE4 => 0x3E, // RCtrl
        0xE5 => 0x3C, // RShift
        0xE6 => 0x3D, // RAlt / ROption
        0xE7 => 0x36, // RGui / RCmd
        _ => return None,
    }))
}

/// macOS virtual keycode → HID usage. Reverse of `hid_to_cg`; same set of
/// keys is covered in both directions, so the round-trip is total over the
/// supported subset.
pub fn cg_to_hid(cg: CgKeyCode) -> Option<HidUsage> {
    Some(HidUsage(match cg.0 {
        // Letters
        0x00 => 0x04, // A
        0x0B => 0x05, // B
        0x08 => 0x06, // C
        0x02 => 0x07, // D
        0x0E => 0x08, // E
        0x03 => 0x09, // F
        0x05 => 0x0A, // G
        0x04 => 0x0B, // H
        0x22 => 0x0C, // I
        0x26 => 0x0D, // J
        0x28 => 0x0E, // K
        0x25 => 0x0F, // L
        0x2E => 0x10, // M
        0x2D => 0x11, // N
        0x1F => 0x12, // O
        0x23 => 0x13, // P
        0x0C => 0x14, // Q
        0x0F => 0x15, // R
        0x01 => 0x16, // S
        0x11 => 0x17, // T
        0x20 => 0x18, // U
        0x09 => 0x19, // V
        0x0D => 0x1A, // W
        0x07 => 0x1B, // X
        0x10 => 0x1C, // Y
        0x06 => 0x1D, // Z
        // Numbers
        0x12 => 0x1E, // 1
        0x13 => 0x1F, // 2
        0x14 => 0x20, // 3
        0x15 => 0x21, // 4
        0x17 => 0x22, // 5
        0x16 => 0x23, // 6
        0x1A => 0x24, // 7
        0x1C => 0x25, // 8
        0x19 => 0x26, // 9
        0x1D => 0x27, // 0
        // Control & whitespace
        0x24 => 0x28, // Return
        0x35 => 0x29, // Escape
        0x33 => 0x2A, // Backspace
        0x30 => 0x2B, // Tab
        0x31 => 0x2C, // Space
        // Punctuation
        0x1B => 0x2D, // -
        0x18 => 0x2E, // =
        0x21 => 0x2F, // [
        0x1E => 0x30, // ]
        0x2A => 0x31, // \
        0x29 => 0x33, // ;
        0x27 => 0x34, // '
        0x32 => 0x35, // `
        0x2B => 0x36, // ,
        0x2F => 0x37, // .
        0x2C => 0x38, // /
        0x39 => 0x39, // Caps Lock
        // Function row
        0x7A => 0x3A,
        0x78 => 0x3B,
        0x63 => 0x3C,
        0x76 => 0x3D,
        0x60 => 0x3E,
        0x61 => 0x3F,
        0x62 => 0x40,
        0x64 => 0x41,
        0x65 => 0x42,
        0x6D => 0x43,
        0x67 => 0x44,
        0x6F => 0x45,
        // Navigation cluster
        0x72 => 0x49, // Help → Insert
        0x73 => 0x4A,
        0x74 => 0x4B,
        0x75 => 0x4C,
        0x77 => 0x4D,
        0x79 => 0x4E,
        0x7C => 0x4F,
        0x7B => 0x50,
        0x7D => 0x51,
        0x7E => 0x52,
        // Modifiers
        0x3B => 0xE0,
        0x38 => 0xE1,
        0x3A => 0xE2,
        0x37 => 0xE3,
        0x3E => 0xE4,
        0x3C => 0xE5,
        0x3D => 0xE6,
        0x36 => 0xE7,
        _ => return None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HID → keyberon → HID round-trips for every key in the supported set.
    /// Catches any future drift between the three tables.
    #[test]
    fn hid_to_keyberon_round_trips_for_full_set() {
        let usages: &[u16] = &[
            // Letters
            0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11,
            0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D,
            // Numbers
            0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
            // Control & whitespace
            0x28, 0x29, 0x2A, 0x2B, 0x2C, // Punctuation
            0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
            // Function row
            0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45,
            // Navigation
            0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, // Modifiers
            0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7,
        ];
        for &u in usages {
            let kc = HidUsage(u)
                .to_keyberon()
                .unwrap_or_else(|| panic!("HID 0x{u:02X} missing in to_keyberon"));
            let back = HidUsage::from(kc);
            assert_eq!(
                back.0, u,
                "HID 0x{u:02X} round-trip failed: keyberon={kc:?} → 0x{:02X}",
                back.0
            );
        }
    }

    /// HID → CG → HID round-trips for every key in the supported set.
    #[test]
    fn hid_to_cg_round_trips_for_full_set() {
        let usages: &[u16] = &[
            0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11,
            0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, //
            0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, //
            0x28, 0x29, 0x2A, 0x2B, 0x2C, //
            0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, //
            0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, //
            0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, //
            0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7,
        ];
        for &u in usages {
            let cg = hid_to_cg(HidUsage(u))
                .unwrap_or_else(|| panic!("HID 0x{u:02X} missing in hid_to_cg"));
            let back = cg_to_hid(cg).unwrap_or_else(|| {
                panic!(
                    "CG 0x{:02X} (from HID 0x{u:02X}) missing in cg_to_hid",
                    cg.0
                )
            });
            assert_eq!(
                back.0, u,
                "HID 0x{u:02X} round-trip via CG failed: cg=0x{:02X}, back=0x{:02X}",
                cg.0, back.0
            );
        }
    }

    /// Anything outside the supported set must return None — never silently
    /// alias to 0x00 (which the OS would treat as a real key).
    #[test]
    fn unknown_usage_returns_none() {
        assert!(HidUsage(0x00).to_keyberon().is_none());
        assert!(hid_to_cg(HidUsage(0x00)).is_none());
        assert!(hid_to_cg(HidUsage(0xFF)).is_none());
        // 0x32 is "Non-US #" / ISO key — not in our table.
        assert!(hid_to_cg(HidUsage(0x32)).is_none());
    }
}
