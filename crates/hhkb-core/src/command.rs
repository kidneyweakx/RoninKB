//! HHKB command builders and response parsers.
//!
//! Each request function returns a 65-byte buffer ready to be written to the
//! HID device. Each response parser takes a [`Response`] (already validated
//! by [`protocol::parse_response`]) and extracts the structured payload.

use crate::error::Result;
use crate::protocol::{self, Response};
use crate::types::{DipSwitchState, KeyboardInfo, KeyboardMode};

// ---------------------------------------------------------------------------
// Command IDs
// ---------------------------------------------------------------------------

pub const CMD_NOTIFY_APP_STATE: u8 = 0x01;
pub const CMD_GET_KEYBOARD_INFO: u8 = 0x02;
pub const CMD_RESET_FACTORY: u8 = 0x03;
pub const CMD_CONFIRM_KEYMAP: u8 = 0x04;
pub const CMD_GET_DIP_SWITCH: u8 = 0x05;
pub const CMD_GET_KEYBOARD_MODE: u8 = 0x06;
pub const CMD_RESET_DIPSW: u8 = 0x07;
pub const CMD_SET_KEYMAP: u8 = 0x86;
pub const CMD_GET_KEYMAP: u8 = 0x87;

// ---------------------------------------------------------------------------
// Request builders
// ---------------------------------------------------------------------------

/// Notify the keyboard that the configuration application has opened.
pub fn notify_app_open() -> [u8; 65] {
    // params occupy buf[4..] = [0x00, 0x01, 0x00]
    protocol::build_request(CMD_NOTIFY_APP_STATE, &[0x00, 0x01, 0x00])
}

/// Notify the keyboard that the configuration application has closed.
pub fn notify_app_close() -> [u8; 65] {
    protocol::build_request(CMD_NOTIFY_APP_STATE, &[0x00, 0x01, 0x01])
}

/// Request keyboard identity, firmware, and serial number.
pub fn get_keyboard_info() -> [u8; 65] {
    protocol::build_request(CMD_GET_KEYBOARD_INFO, &[])
}

/// Request the current DIP switch states.
pub fn get_dip_switch() -> [u8; 65] {
    protocol::build_request(CMD_GET_DIP_SWITCH, &[])
}

/// Request the current keyboard operating mode.
pub fn get_keyboard_mode() -> [u8; 65] {
    protocol::build_request(CMD_GET_KEYBOARD_MODE, &[])
}

/// Request the keymap for a given `mode` and layer.
///
/// `fn_layer = false` reads the base layer, `true` reads the Fn layer.
pub fn get_keymap(mode: KeyboardMode, fn_layer: bool) -> [u8; 65] {
    protocol::build_request(
        CMD_GET_KEYMAP,
        &[0x00, 0x02, u8::from(mode), fn_layer as u8],
    )
}

/// First of three writes that upload a full 128-byte keymap payload.
///
/// Packs 57 bytes of keymap data into buf[8..65].
pub fn set_keymap_write1(
    mode: KeyboardMode,
    fn_layer: bool,
    data: &[u8; 57],
) -> [u8; 65] {
    let mut params = [0u8; 61];
    params[0] = 65; // total offset written so far (including header)
    params[1] = 59; // number of keymap bytes in this packet (header + data)
    params[2] = u8::from(mode);
    params[3] = fn_layer as u8;
    params[4..61].copy_from_slice(data);
    protocol::build_request(CMD_SET_KEYMAP, &params)
}

/// Second of three keymap writes — 59 bytes of data occupying buf[6..65].
pub fn set_keymap_write2(data: &[u8; 59]) -> [u8; 65] {
    let mut params = [0u8; 61];
    params[0] = 130;
    params[1] = 59;
    params[2..61].copy_from_slice(data);
    protocol::build_request(CMD_SET_KEYMAP, &params)
}

/// Third of three keymap writes — the final 12 bytes (buf[6..18]).
pub fn set_keymap_write3(data: &[u8; 12]) -> [u8; 65] {
    let mut params = [0u8; 14];
    params[0] = 195;
    params[1] = 12;
    params[2..14].copy_from_slice(data);
    protocol::build_request(CMD_SET_KEYMAP, &params)
}

/// Commit / confirm a previously written keymap.
pub fn confirm_keymap() -> [u8; 65] {
    protocol::build_request(CMD_CONFIRM_KEYMAP, &[])
}

/// Reset all DIP switches to their default state.
pub fn reset_dipsw() -> [u8; 65] {
    protocol::build_request(CMD_RESET_DIPSW, &[])
}

// ---------------------------------------------------------------------------
// Response parsers
// ---------------------------------------------------------------------------

/// Parse the response payload of [`get_keyboard_info`].
pub fn parse_keyboard_info(resp: &Response) -> Result<KeyboardInfo> {
    KeyboardInfo::parse(&resp.data)
}

/// Parse the response payload of [`get_dip_switch`] into a [`DipSwitchState`].
pub fn parse_dip_switch(resp: &Response) -> Result<DipSwitchState> {
    Ok(DipSwitchState::from(&resp.data[0..6]))
}

/// Parse the response payload of [`get_keyboard_mode`] into a [`KeyboardMode`].
pub fn parse_keyboard_mode(resp: &Response) -> Result<KeyboardMode> {
    KeyboardMode::try_from(resp.data[0])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_response(data: Vec<u8>) -> Response {
        // Pad/truncate to exactly 58 bytes (the size emitted by parse_response).
        let mut padded = data;
        padded.resize(58, 0);
        Response {
            command: 0x00,
            status: 0x00,
            param: 0x00,
            length: 0x00,
            data: padded,
        }
    }

    // -- Request builders ---------------------------------------------------

    #[test]
    fn test_notify_app_open() {
        let req = notify_app_open();
        assert_eq!(req[1], 0xAA);
        assert_eq!(req[2], 0xAA);
        assert_eq!(req[3], CMD_NOTIFY_APP_STATE);
        assert_eq!(req[3], 0x01);
        assert_eq!(req[4], 0x00);
        assert_eq!(req[5], 0x01);
        assert_eq!(req[6], 0x00);
    }

    #[test]
    fn test_notify_app_close() {
        let req = notify_app_close();
        assert_eq!(req[3], 0x01);
        assert_eq!(req[4], 0x00);
        assert_eq!(req[5], 0x01);
        assert_eq!(req[6], 0x01);
    }

    #[test]
    fn test_get_keyboard_info_request() {
        let req = get_keyboard_info();
        assert_eq!(req[0], 0x00);
        assert_eq!(req[1], 0xAA);
        assert_eq!(req[2], 0xAA);
        assert_eq!(req[3], 0x02);
        // No params — everything after the command byte must be zero.
        for &b in &req[4..] {
            assert_eq!(b, 0x00);
        }
    }

    #[test]
    fn test_get_keymap_mac_base() {
        let req = get_keymap(KeyboardMode::Mac, false);
        assert_eq!(req[3], 0x87);
        assert_eq!(req[4], 0x00);
        assert_eq!(req[5], 0x02);
        assert_eq!(req[6], 0x01); // Mac
        assert_eq!(req[7], 0x00); // base layer
    }

    #[test]
    fn test_get_keymap_hhk_fn() {
        let req = get_keymap(KeyboardMode::HHK, true);
        assert_eq!(req[3], 0x87);
        assert_eq!(req[4], 0x00);
        assert_eq!(req[5], 0x02);
        assert_eq!(req[6], 0x00); // HHK
        assert_eq!(req[7], 0x01); // Fn layer
    }

    #[test]
    fn test_set_keymap_write1() {
        let mut data = [0u8; 57];
        for (i, b) in data.iter_mut().enumerate() {
            *b = i as u8;
        }
        let req = set_keymap_write1(KeyboardMode::Mac, true, &data);

        assert_eq!(req[3], 0x86);
        assert_eq!(req[4], 65);
        assert_eq!(req[5], 59);
        assert_eq!(req[6], u8::from(KeyboardMode::Mac));
        assert_eq!(req[7], 1);
        assert_eq!(&req[8..65], &data[..]);
    }

    #[test]
    fn test_set_keymap_write2() {
        let mut data = [0u8; 59];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i as u8).wrapping_add(100);
        }
        let req = set_keymap_write2(&data);

        assert_eq!(req[3], 0x86);
        assert_eq!(req[4], 130);
        assert_eq!(req[5], 59);
        assert_eq!(&req[6..65], &data[..]);
    }

    #[test]
    fn test_set_keymap_write3() {
        let data: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let req = set_keymap_write3(&data);

        assert_eq!(req[3], 0x86);
        assert_eq!(req[4], 195);
        assert_eq!(req[5], 12);
        assert_eq!(&req[6..18], &data[..]);
        // Remaining bytes must be zero-padded.
        for &b in &req[18..] {
            assert_eq!(b, 0x00);
        }
    }

    // -- Response parsers ---------------------------------------------------

    #[test]
    fn test_parse_keyboard_info() {
        let mut data = vec![0u8; 58];
        // Type number "PD-KB800BNS" padded with zeros to 20 bytes.
        let name = b"PD-KB800BNS";
        data[..name.len()].copy_from_slice(name);
        // Revision
        data[20] = 1;
        data[21] = 2;
        data[22] = 3;
        data[23] = 4;
        // Serial — filled with 0x5A
        for b in &mut data[24..40] {
            *b = 0x5A;
        }
        // App firmware
        for (i, b) in data[40..48].iter_mut().enumerate() {
            *b = i as u8;
        }
        // Boot firmware
        for (i, b) in data[48..56].iter_mut().enumerate() {
            *b = (i as u8) + 10;
        }
        // Running firmware = Application
        data[56] = 0;

        let resp = make_response(data);
        let info = parse_keyboard_info(&resp).unwrap();
        assert_eq!(info.type_number, "PD-KB800BNS");
        assert_eq!(info.revision, [1, 2, 3, 4]);
        assert_eq!(info.serial, [0x5A; 16]);
        assert_eq!(info.app_firmware, [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(info.boot_firmware, [10, 11, 12, 13, 14, 15, 16, 17]);
        assert_eq!(
            info.running_firmware,
            crate::types::FirmwareType::Application
        );
    }

    #[test]
    fn test_parse_keyboard_mode_mac() {
        let resp = make_response(vec![1]);
        let mode = parse_keyboard_mode(&resp).unwrap();
        assert_eq!(mode, KeyboardMode::Mac);
    }

    #[test]
    fn test_parse_dip_switch() {
        let resp = make_response(vec![0, 1, 0, 1, 0, 0]);
        let state = parse_dip_switch(&resp).unwrap();
        assert_eq!(
            state.switches,
            [false, true, false, true, false, false]
        );
    }

    // -- Invariants ---------------------------------------------------------

    #[test]
    fn test_all_commands_have_magic() {
        let reqs: Vec<[u8; 65]> = vec![
            notify_app_open(),
            notify_app_close(),
            get_keyboard_info(),
            get_dip_switch(),
            get_keyboard_mode(),
            get_keymap(KeyboardMode::HHK, false),
            get_keymap(KeyboardMode::Mac, true),
            set_keymap_write1(KeyboardMode::HHK, false, &[0u8; 57]),
            set_keymap_write2(&[0u8; 59]),
            set_keymap_write3(&[0u8; 12]),
            confirm_keymap(),
            reset_dipsw(),
        ];

        for (i, req) in reqs.iter().enumerate() {
            assert_eq!(req[0], 0x00, "request {i} missing report ID prefix");
            assert_eq!(req[1], 0xAA, "request {i} missing magic byte 1");
            assert_eq!(req[2], 0xAA, "request {i} missing magic byte 2");
        }
    }
}
