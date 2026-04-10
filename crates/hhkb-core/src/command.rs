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
pub const CMD_DUMP_FIRMWARE: u8 = 0xD0;
pub const CMD_FIRMUP_MODE_CHANGE: u8 = 0xE0;
pub const CMD_FIRMUP_START: u8 = 0xE1;
pub const CMD_FIRMUP_SEND: u8 = 0xE2;
pub const CMD_FIRMUP_END: u8 = 0xE3;

/// Maximum firmware payload bytes that fit in a single `FirmupSend` packet.
///
/// Matches the happy-hacking-gnu reference implementation: the writer packs
/// 57 data bytes starting at `buf[8]`, leaving `buf[5]` for the length marker
/// and `buf[6..8]` for the 16-bit packet number.
pub const FIRMUP_SEND_CHUNK_MAX: usize = 57;

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
pub fn set_keymap_write1(mode: KeyboardMode, fn_layer: bool, data: &[u8; 57]) -> [u8; 65] {
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
// Firmware commands
// ---------------------------------------------------------------------------

/// Build a `DumpFirmware` (0xD0) request.
///
/// The request body is empty; the keyboard replies with a stream of 64-byte
/// response packets where `resp[5]` encodes `payload_len + 2` and the actual
/// firmware bytes live at `resp[8..8 + (resp[5] - 2)]`. Reads terminate once
/// a short packet (`resp[5] < 58`) is observed.
///
/// Safe read-only operation.
pub fn dump_firmware() -> [u8; 65] {
    protocol::build_request(CMD_DUMP_FIRMWARE, &[])
}

/// Build a `FirmupModeChange` (0xE0) request.
///
/// **DANGER**: sending this puts the keyboard into firmware update mode and
/// causes the device to disconnect. Only use as part of a full firmware
/// update sequence.
pub fn firmup_mode_change() -> [u8; 65] {
    protocol::build_request(CMD_FIRMUP_MODE_CHANGE, &[])
}

/// Build a `FirmupStart` (0xE1) request.
///
/// Byte layout (matches happy-hacking-gnu `hhkb_firmup_start`):
/// - `[3]`     = `0xE1`
/// - `[4]`     = `0x00`
/// - `[5]`     = `0x08`  (length marker: 4 bytes size + 2 bytes CRC + padding)
/// - `[6..10]` = firmware size, little-endian `u32`
/// - `[10..12]`= CRC-16 of firmware file, little-endian
///
/// **DANGER**: only call after `FirmupModeChange` has succeeded and the
/// device has re-enumerated.
pub fn firmup_start(file_size: u32, crc16: u16) -> [u8; 65] {
    // params start at buf[4] — we need [4]=0, [5]=8, [6..10]=size, [10..12]=crc.
    let mut params = [0u8; 8];
    params[0] = 0x00; // buf[4] — unknown, C ref leaves it zero
    params[1] = 0x08; // buf[5] — fixed length marker
    params[2..6].copy_from_slice(&file_size.to_le_bytes()); // buf[6..10]
    params[6..8].copy_from_slice(&crc16.to_le_bytes()); // buf[10..12]
    protocol::build_request(CMD_FIRMUP_START, &params)
}

/// Build a `FirmupSend` (0xE2) request for one firmware chunk.
///
/// Byte layout (matches happy-hacking-gnu `hhkb_firmup_send`):
/// - `[3]`         = `0xE2`
/// - `[4]`         = `0x00`
/// - `[5]`         = `chunk.len() + 2`          (length marker)
/// - `[6..8]`      = `packet_num` little-endian `u16`
/// - `[8..8+len]`  = firmware data bytes
///
/// # Panics
///
/// Panics if `chunk.len() > FIRMUP_SEND_CHUNK_MAX` (57 bytes).
pub fn firmup_send(packet_num: u16, chunk: &[u8]) -> [u8; 65] {
    assert!(
        chunk.len() <= FIRMUP_SEND_CHUNK_MAX,
        "firmup_send chunk too large: max {} bytes, got {}",
        FIRMUP_SEND_CHUNK_MAX,
        chunk.len()
    );

    // params cover buf[4..4+params.len()] — we need:
    //   buf[4] = 0  (offset 0 in params)
    //   buf[5] = len+2 (offset 1)
    //   buf[6..8] = packet_num LE (offsets 2..4)
    //   buf[8..8+len] = chunk (offsets 4..4+len)
    let mut params = [0u8; 4 + FIRMUP_SEND_CHUNK_MAX];
    params[0] = 0x00;
    params[1] = (chunk.len() as u8).wrapping_add(2);
    params[2..4].copy_from_slice(&packet_num.to_le_bytes());
    params[4..4 + chunk.len()].copy_from_slice(chunk);
    protocol::build_request(CMD_FIRMUP_SEND, &params[..4 + chunk.len()])
}

/// Build a `FirmupEnd` (0xE3) request. Body is empty.
///
/// **DANGER**: marks the firmware stream complete and commits the update.
pub fn firmup_end() -> [u8; 65] {
    protocol::build_request(CMD_FIRMUP_END, &[])
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
        assert_eq!(state.switches, [false, true, false, true, false, false]);
    }

    // -- Firmware command builders ------------------------------------------

    #[test]
    fn test_dump_firmware_request_bytes() {
        let req = dump_firmware();
        assert_eq!(req[0], 0x00);
        assert_eq!(req[1], 0xAA);
        assert_eq!(req[2], 0xAA);
        assert_eq!(req[3], CMD_DUMP_FIRMWARE);
        assert_eq!(req[3], 0xD0);
        // Body is empty — everything after the cmd byte must be zero.
        for &b in &req[4..] {
            assert_eq!(b, 0x00);
        }
    }

    #[test]
    fn test_firmup_mode_change_request_bytes() {
        let req = firmup_mode_change();
        assert_eq!(req[1], 0xAA);
        assert_eq!(req[2], 0xAA);
        assert_eq!(req[3], CMD_FIRMUP_MODE_CHANGE);
        assert_eq!(req[3], 0xE0);
        for &b in &req[4..] {
            assert_eq!(b, 0x00);
        }
    }

    #[test]
    fn test_firmup_start_request_bytes() {
        // file_size = 0x0001_2345, crc16 = 0xBEEF
        let file_size: u32 = 0x0001_2345;
        let crc16: u16 = 0xBEEF;
        let req = firmup_start(file_size, crc16);

        assert_eq!(req[3], CMD_FIRMUP_START);
        assert_eq!(req[3], 0xE1);
        assert_eq!(req[4], 0x00);
        assert_eq!(req[5], 0x08);
        // file_size LE at [6..10]
        assert_eq!(&req[6..10], &file_size.to_le_bytes());
        // crc16 LE at [10..12]
        assert_eq!(&req[10..12], &crc16.to_le_bytes());
        // Remaining bytes zero.
        for &b in &req[12..] {
            assert_eq!(b, 0x00);
        }
    }

    #[test]
    fn test_firmup_send_with_data() {
        // Pack 32 bytes with a recognizable pattern.
        let mut chunk = [0u8; 32];
        for (i, b) in chunk.iter_mut().enumerate() {
            *b = (i as u8) ^ 0x5A;
        }
        let packet_num: u16 = 0x1234;
        let req = firmup_send(packet_num, &chunk);

        assert_eq!(req[3], CMD_FIRMUP_SEND);
        assert_eq!(req[3], 0xE2);
        assert_eq!(req[4], 0x00);
        // length marker = chunk_len + 2
        assert_eq!(req[5], 32 + 2);
        // packet number LE at [6..8]
        assert_eq!(&req[6..8], &packet_num.to_le_bytes());
        // data at [8..8+32]
        assert_eq!(&req[8..8 + 32], &chunk[..]);
        // Nothing leaking beyond the chunk.
        for &b in &req[8 + 32..] {
            assert_eq!(b, 0x00);
        }
    }

    #[test]
    fn test_firmup_send_max_chunk() {
        // Largest valid chunk — 57 bytes.
        let chunk = [0xABu8; FIRMUP_SEND_CHUNK_MAX];
        let req = firmup_send(0, &chunk);
        assert_eq!(req[5], FIRMUP_SEND_CHUNK_MAX as u8 + 2);
        assert_eq!(&req[8..8 + FIRMUP_SEND_CHUNK_MAX], &chunk[..]);
    }

    #[test]
    #[should_panic(expected = "firmup_send chunk too large")]
    fn test_firmup_send_oversized_panics() {
        let chunk = [0u8; FIRMUP_SEND_CHUNK_MAX + 1];
        let _ = firmup_send(0, &chunk);
    }

    #[test]
    fn test_firmup_end_request_bytes() {
        let req = firmup_end();
        assert_eq!(req[1], 0xAA);
        assert_eq!(req[2], 0xAA);
        assert_eq!(req[3], CMD_FIRMUP_END);
        assert_eq!(req[3], 0xE3);
        for &b in &req[4..] {
            assert_eq!(b, 0x00);
        }
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
            dump_firmware(),
            firmup_mode_change(),
            firmup_start(0x1000, 0xABCD),
            firmup_send(0, &[0u8; 16]),
            firmup_end(),
        ];

        for (i, req) in reqs.iter().enumerate() {
            assert_eq!(req[0], 0x00, "request {i} missing report ID prefix");
            assert_eq!(req[1], 0xAA, "request {i} missing magic byte 1");
            assert_eq!(req[2], 0xAA, "request {i} missing magic byte 2");
        }
    }
}
