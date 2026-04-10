use crate::error::Error;

pub const HHKB_VENDOR_ID: u16 = 0x04FE;
pub const HHKB_PRODUCT_IDS: [u16; 3] = [0x0020, 0x0021, 0x0022];
pub const VENDOR_INTERFACE: i32 = 2;

// ---------------------------------------------------------------------------
// KeyboardMode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyboardMode {
    HHK = 0,
    Mac = 1,
    Lite = 2,
    Secret = 3,
}

impl TryFrom<u8> for KeyboardMode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::HHK),
            1 => Ok(Self::Mac),
            2 => Ok(Self::Lite),
            3 => Ok(Self::Secret),
            _ => Err(Error::Transport(format!(
                "invalid keyboard mode: {}",
                value
            ))),
        }
    }
}

impl From<KeyboardMode> for u8 {
    fn from(mode: KeyboardMode) -> u8 {
        mode as u8
    }
}

// ---------------------------------------------------------------------------
// FirmwareType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FirmwareType {
    Application = 0,
    Bootloader = 1,
}

impl TryFrom<u8> for FirmwareType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Application),
            1 => Ok(Self::Bootloader),
            _ => Err(Error::Transport(format!(
                "invalid firmware type: {}",
                value
            ))),
        }
    }
}

impl From<FirmwareType> for u8 {
    fn from(ft: FirmwareType) -> u8 {
        ft as u8
    }
}

// ---------------------------------------------------------------------------
// DipSwitchState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DipSwitchState {
    pub switches: [bool; 6],
}

impl From<&[u8]> for DipSwitchState {
    fn from(bytes: &[u8]) -> Self {
        let mut switches = [false; 6];
        for (i, s) in switches.iter_mut().enumerate() {
            *s = bytes.get(i).copied().unwrap_or(0) != 0;
        }
        Self { switches }
    }
}

// ---------------------------------------------------------------------------
// KeyboardInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardInfo {
    pub type_number: String,
    pub revision: [u8; 4],
    pub serial: [u8; 16],
    pub app_firmware: [u8; 8],
    pub boot_firmware: [u8; 8],
    pub running_firmware: FirmwareType,
}

impl KeyboardInfo {
    /// Parse from a 58-byte (or longer) data slice.
    ///
    /// Layout:
    ///   [ 0..20)  type_number   — 20 bytes ASCII, NUL-padded
    ///   [20..24)  revision      — 4 bytes
    ///   [24..40)  serial        — 16 bytes
    ///   [40..48)  app_firmware  — 8 bytes
    ///   [48..56)  boot_firmware — 8 bytes
    ///   [56]      running_firmware — 1 byte (0 = Application, 1 = Bootloader)
    ///
    /// The minimum required length is 57 bytes (indices 0..=56).
    pub fn parse(data: &[u8]) -> crate::error::Result<Self> {
        if data.len() < 57 {
            return Err(Error::Transport(format!(
                "keyboard info too short: {} bytes (need at least 57)",
                data.len()
            )));
        }

        let type_number = String::from_utf8_lossy(&data[0..20])
            .trim_end_matches('\0')
            .to_string();

        let mut revision = [0u8; 4];
        revision.copy_from_slice(&data[20..24]);

        let mut serial = [0u8; 16];
        serial.copy_from_slice(&data[24..40]);

        let mut app_firmware = [0u8; 8];
        app_firmware.copy_from_slice(&data[40..48]);

        let mut boot_firmware = [0u8; 8];
        boot_firmware.copy_from_slice(&data[48..56]);

        let running_firmware = FirmwareType::try_from(data[56])?;

        Ok(Self {
            type_number,
            revision,
            serial,
            app_firmware,
            boot_firmware,
            running_firmware,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- KeyboardMode -------------------------------------------------------

    #[test]
    fn keyboard_mode_roundtrip() {
        let modes = [
            (KeyboardMode::HHK, 0u8),
            (KeyboardMode::Mac, 1),
            (KeyboardMode::Lite, 2),
            (KeyboardMode::Secret, 3),
        ];
        for (mode, byte) in modes {
            // Into<u8>
            assert_eq!(u8::from(mode), byte);
            // TryFrom<u8>
            assert_eq!(KeyboardMode::try_from(byte).unwrap(), mode);
        }
    }

    #[test]
    fn keyboard_mode_invalid_value_returns_error() {
        let result = KeyboardMode::try_from(4u8);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("invalid keyboard mode: 4"),
            "unexpected error message: {}",
            err
        );
    }

    // -- FirmwareType -------------------------------------------------------

    #[test]
    fn firmware_type_roundtrip() {
        let types = [
            (FirmwareType::Application, 0u8),
            (FirmwareType::Bootloader, 1),
        ];
        for (ft, byte) in types {
            assert_eq!(u8::from(ft), byte);
            assert_eq!(FirmwareType::try_from(byte).unwrap(), ft);
        }
    }

    #[test]
    fn firmware_type_invalid_value_returns_error() {
        let result = FirmwareType::try_from(99u8);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid firmware type: 99"));
    }

    // -- DipSwitchState -----------------------------------------------------

    #[test]
    fn dip_switch_from_byte_slice() {
        let bytes: &[u8] = &[0, 1, 0, 1, 0, 0];
        let state = DipSwitchState::from(bytes);
        assert_eq!(
            state.switches,
            [false, true, false, true, false, false]
        );
    }

    #[test]
    fn dip_switch_from_short_slice_pads_false() {
        let bytes: &[u8] = &[1, 0];
        let state = DipSwitchState::from(bytes);
        assert_eq!(
            state.switches,
            [true, false, false, false, false, false]
        );
    }

    #[test]
    fn dip_switch_nonzero_is_true() {
        let bytes: &[u8] = &[0xFF, 2, 0, 0, 0, 1];
        let state = DipSwitchState::from(bytes);
        assert_eq!(
            state.switches,
            [true, true, false, false, false, true]
        );
    }

    // -- KeyboardInfo -------------------------------------------------------

    #[test]
    fn keyboard_info_parse_from_simulated_58_bytes() {
        let mut data = [0u8; 58];

        // [0..20) type_number: "PD-KB800BNS" padded with zeros
        let name = b"PD-KB800BNS";
        data[..name.len()].copy_from_slice(name);

        // [20..24) revision
        data[20] = 1;
        data[21] = 0;
        data[22] = 3;
        data[23] = 7;

        // [24..40) serial — fill with 0xAA pattern
        for b in &mut data[24..40] {
            *b = 0xAA;
        }

        // [40..48) app_firmware
        data[40] = 2;
        data[41] = 0;
        data[42] = 1;
        data[43] = 0;

        // [48..56) boot_firmware
        data[48] = 1;
        data[49] = 0;
        data[50] = 0;
        data[51] = 5;

        // [56] running_firmware = Application (0)
        data[56] = 0;

        let info = KeyboardInfo::parse(&data).expect("parse should succeed");
        assert_eq!(info.type_number, "PD-KB800BNS");
        assert_eq!(info.revision, [1, 0, 3, 7]);
        assert_eq!(info.serial, [0xAA; 16]);
        assert_eq!(info.app_firmware[0..4], [2, 0, 1, 0]);
        assert_eq!(info.boot_firmware[0..4], [1, 0, 0, 5]);
        assert_eq!(info.running_firmware, FirmwareType::Application);
    }

    #[test]
    fn keyboard_info_parse_bootloader_firmware() {
        let mut data = [0u8; 57];
        data[..3].copy_from_slice(b"ABC");
        data[56] = 1; // Bootloader
        let info = KeyboardInfo::parse(&data).unwrap();
        assert_eq!(info.type_number, "ABC");
        assert_eq!(info.running_firmware, FirmwareType::Bootloader);
    }

    #[test]
    fn keyboard_info_parse_too_short_returns_error() {
        let data = [0u8; 50];
        let result = KeyboardInfo::parse(&data);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("too short"));
    }

    #[test]
    fn keyboard_info_parse_invalid_firmware_type_returns_error() {
        let mut data = [0u8; 57];
        data[56] = 99; // invalid
        let result = KeyboardInfo::parse(&data);
        assert!(result.is_err());
    }

    // -- Constants ----------------------------------------------------------

    #[test]
    fn constants_have_correct_values() {
        assert_eq!(HHKB_VENDOR_ID, 0x04FE);
        assert_eq!(HHKB_PRODUCT_IDS, [0x0020, 0x0021, 0x0022]);
        assert_eq!(VENDOR_INTERFACE, 2);
    }
}
