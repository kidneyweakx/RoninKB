use crate::error::{Error, Result};

pub const MAGIC_REQUEST: [u8; 2] = [0xAA, 0xAA];
pub const MAGIC_RESPONSE: [u8; 2] = [0x55, 0x55];
pub const REQUEST_SIZE: usize = 65;
pub const RESPONSE_SIZE: usize = 64;

/// Parsed HID response from the HHKB device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub command: u8,
    pub status: u8,
    pub param: u8,
    pub length: u8,
    pub data: Vec<u8>, // bytes [6..64], always 58 elements
}

/// Build a 65-byte HID request buffer.
///
/// Layout:
/// - `[0]`    = `0x00` — hidapi report ID prefix
/// - `[1..3]` = `0xAA 0xAA` — magic bytes
/// - `[3]`    = command ID
/// - `[4..]`  = params, zero-padded to fill the remaining 61 bytes
///
/// # Panics
///
/// Panics if `params.len() > 61`.
pub fn build_request(command: u8, params: &[u8]) -> [u8; REQUEST_SIZE] {
    assert!(
        params.len() <= 61,
        "params too long: max 61 bytes, got {}",
        params.len()
    );

    let mut buf = [0u8; REQUEST_SIZE];
    // [0] = 0x00 (report ID, already zero)
    buf[1] = MAGIC_REQUEST[0];
    buf[2] = MAGIC_REQUEST[1];
    buf[3] = command;
    buf[4..4 + params.len()].copy_from_slice(params);
    buf
}

/// Parse a 64-byte HID response from the device.
///
/// Returns `Error::InvalidMagic` if the first two bytes are not `0x55 0x55`.
/// Returns `Error::Transport` if the slice is shorter than 64 bytes.
pub fn parse_response(data: &[u8]) -> Result<Response> {
    if data.len() < RESPONSE_SIZE {
        return Err(Error::Transport(format!(
            "response too short: expected {} bytes, got {}",
            RESPONSE_SIZE,
            data.len()
        )));
    }

    if data[0] != MAGIC_RESPONSE[0] || data[1] != MAGIC_RESPONSE[1] {
        return Err(Error::InvalidMagic(data[0], data[1]));
    }

    Ok(Response {
        command: data[2],
        status: data[3],
        param: data[4],
        length: data[5],
        data: data[6..64].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_request_basic() {
        let req = build_request(0x02, &[]);
        assert_eq!(req.len(), 65);
        assert_eq!(req[0], 0x00);
        assert_eq!(req[1], 0xAA);
        assert_eq!(req[2], 0xAA);
        assert_eq!(req[3], 0x02);
        // All remaining bytes are zero-padded.
        for &b in &req[4..] {
            assert_eq!(b, 0x00);
        }
    }

    #[test]
    fn test_build_request_with_params() {
        let params = [0x00, 0x02, 0x01, 0x00];
        let req = build_request(0x87, &params);
        assert_eq!(req[3], 0x87);
        assert_eq!(req[4], 0x00);
        assert_eq!(req[5], 0x02);
        assert_eq!(req[6], 0x01);
        assert_eq!(req[7], 0x00);
    }

    #[test]
    fn test_build_request_padding() {
        let params = [0xFF, 0xFE];
        let req = build_request(0x10, &params);
        // Params occupy [4] and [5].
        assert_eq!(req[4], 0xFF);
        assert_eq!(req[5], 0xFE);
        // Everything after the params must be zero.
        for &b in &req[6..] {
            assert_eq!(b, 0x00);
        }
    }

    #[test]
    fn test_build_request_magic_always_present() {
        // Regardless of command or params, magic bytes are always at [1] and [2].
        for cmd in [0x00, 0x02, 0x87, 0xFF] {
            let req = build_request(cmd, &[]);
            assert_eq!(req[1], 0xAA, "magic byte 1 mismatch for cmd 0x{cmd:02X}");
            assert_eq!(req[2], 0xAA, "magic byte 2 mismatch for cmd 0x{cmd:02X}");
        }
    }

    #[test]
    fn test_parse_response_valid() {
        let mut buf = [0u8; 64];
        buf[0] = 0x55;
        buf[1] = 0x55;
        buf[2] = 0x87; // command
        buf[3] = 0x00; // status = success
        buf[4] = 0x01; // param
        buf[5] = 0x04; // length

        let resp = parse_response(&buf).unwrap();
        assert_eq!(resp.command, 0x87);
        assert_eq!(resp.status, 0x00);
        assert_eq!(resp.param, 0x01);
        assert_eq!(resp.length, 0x04);
    }

    #[test]
    fn test_parse_response_invalid_magic() {
        let mut buf = [0u8; 64];
        buf[0] = 0x00;
        buf[1] = 0x00;

        let err = parse_response(&buf).unwrap_err();
        assert!(matches!(err, Error::InvalidMagic(0x00, 0x00)));
    }

    #[test]
    fn test_parse_response_extracts_data() {
        let mut buf = [0u8; 64];
        buf[0] = 0x55;
        buf[1] = 0x55;
        // Fill data region [6..64] with recognizable values.
        for (i, byte) in buf.iter_mut().enumerate().skip(6) {
            *byte = i as u8;
        }

        let resp = parse_response(&buf).unwrap();
        assert_eq!(resp.data.len(), 58);
        for (idx, &val) in resp.data.iter().enumerate() {
            assert_eq!(val, (idx + 6) as u8);
        }
    }

    #[test]
    fn test_parse_response_too_short() {
        // Empty slice.
        let err = parse_response(&[]).unwrap_err();
        assert!(matches!(err, Error::Transport(_)));

        // Too short (only 10 bytes).
        let err = parse_response(&[0x55; 10]).unwrap_err();
        assert!(matches!(err, Error::Transport(_)));
    }

    #[test]
    fn test_roundtrip_constants() {
        assert_eq!(REQUEST_SIZE, 65);
        assert_eq!(RESPONSE_SIZE, 64);
    }
}
