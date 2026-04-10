//! High-level HHKB device API.
//!
//! Wraps an [`HidTransport`] and exposes the full HHKB command set in terms
//! of typed inputs and outputs.

use crate::command;
use crate::error::{Error, Result};
use crate::keymap::Keymap;
use crate::protocol::{self, Response};
use crate::transport::HidTransport;
use crate::types::{DipSwitchState, KeyboardInfo, KeyboardMode};

/// Default read timeout (milliseconds) for HID responses.
const DEFAULT_READ_TIMEOUT_MS: i32 = 1000;

/// High-level HHKB device handle.
pub struct HhkbDevice<T: HidTransport> {
    transport: T,
}

impl<T: HidTransport> HhkbDevice<T> {
    /// Wrap an existing transport.
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    // ---- Internal helpers -------------------------------------------------

    /// Send a request, read one response, parse it, and verify the status byte.
    fn send_and_receive(&self, request: &[u8]) -> Result<Response> {
        self.transport.write(request)?;
        let mut buf = [0u8; 64];
        self.transport.read(&mut buf, DEFAULT_READ_TIMEOUT_MS)?;
        let resp = protocol::parse_response(&buf)?;
        if resp.status != 0x00 {
            return Err(Error::CommandFailed {
                cmd: resp.command,
                status: resp.status,
            });
        }
        Ok(resp)
    }

    /// Read a single 64-byte chunk (used when a single command yields
    /// multiple response packets, e.g. `GetKeymap`).
    fn read_chunk(&self) -> Result<[u8; 64]> {
        let mut buf = [0u8; 64];
        self.transport.read(&mut buf, DEFAULT_READ_TIMEOUT_MS)?;
        Ok(buf)
    }

    // ---- Session management ----------------------------------------------

    /// Send `NotifyApplicationState(open)`. Must be called before other
    /// commands.
    pub fn open_session(&self) -> Result<()> {
        let req = command::notify_app_open();
        self.send_and_receive(&req)?;
        Ok(())
    }

    /// Send `NotifyApplicationState(close)`. Call when done.
    pub fn close_session(&self) -> Result<()> {
        let req = command::notify_app_close();
        self.send_and_receive(&req)?;
        Ok(())
    }

    // ---- Info queries -----------------------------------------------------

    /// Get keyboard info (type number, firmware version, serial).
    pub fn get_info(&self) -> Result<KeyboardInfo> {
        let req = command::get_keyboard_info();
        let resp = self.send_and_receive(&req)?;
        command::parse_keyboard_info(&resp)
    }

    /// Get the current keyboard mode (HHK / Mac / Lite / Secret).
    pub fn get_mode(&self) -> Result<KeyboardMode> {
        let req = command::get_keyboard_mode();
        let resp = self.send_and_receive(&req)?;
        command::parse_keyboard_mode(&resp)
    }

    /// Get DIP switch states.
    pub fn get_dip_switch(&self) -> Result<DipSwitchState> {
        let req = command::get_dip_switch();
        let resp = self.send_and_receive(&req)?;
        command::parse_dip_switch(&resp)
    }

    // ---- Keymap I/O -------------------------------------------------------

    /// Read a full 128-byte keymap from the device.
    ///
    /// Sends a `GetKeymap` command and reads 3 response chunks, then
    /// assembles them into a [`Keymap`].
    pub fn read_keymap(&self, mode: KeyboardMode, fn_layer: bool) -> Result<Keymap> {
        let req = command::get_keymap(mode, fn_layer);
        self.transport.write(&req)?;

        let chunk1 = self.read_chunk()?;
        let chunk2 = self.read_chunk()?;
        let chunk3 = self.read_chunk()?;

        Keymap::from_chunks(&chunk1, &chunk2, &chunk3)
    }

    /// Write a full 128-byte keymap to the device.
    ///
    /// Sends 3 `SetKeymap` writes (each followed by an ACK read), then
    /// `ConfirmKeymap` and `ResetDIPSW`.
    pub fn write_keymap(
        &self,
        mode: KeyboardMode,
        fn_layer: bool,
        keymap: &Keymap,
    ) -> Result<()> {
        let (a, b, c) = keymap.to_write_chunks();

        let req = command::set_keymap_write1(mode, fn_layer, &a);
        self.send_and_receive(&req)?;

        let req = command::set_keymap_write2(&b);
        self.send_and_receive(&req)?;

        let req = command::set_keymap_write3(&c);
        self.send_and_receive(&req)?;

        let req = command::confirm_keymap();
        self.send_and_receive(&req)?;

        let req = command::reset_dipsw();
        self.send_and_receive(&req)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KEYMAP_SIZE;
    use crate::transport::MockTransport;
    use crate::types::FirmwareType;

    /// Build a minimal 64-byte success response for `cmd`.
    fn mock_success_response(cmd: u8) -> Vec<u8> {
        let mut resp = vec![0u8; 64];
        resp[0] = 0x55;
        resp[1] = 0x55;
        resp[2] = cmd;
        resp[3] = 0x00; // success
        resp
    }

    // -- Session ------------------------------------------------------------

    #[test]
    fn test_open_session() {
        let mock = MockTransport::new();
        mock.queue_response(mock_success_response(0x01));

        let device = HhkbDevice::new(mock);
        device.open_session().expect("open_session should succeed");

        let writes = device.transport.get_writes();
        assert_eq!(writes.len(), 1);
        let req = &writes[0];

        assert_eq!(req[0], 0x00, "report ID prefix");
        assert_eq!(req[1], 0xAA, "magic 1");
        assert_eq!(req[2], 0xAA, "magic 2");
        assert_eq!(req[3], 0x01, "NotifyApplicationState cmd");
        // state=open → byte 6 should be 0x00
        assert_eq!(req[6], 0x00, "state byte = open(0)");
    }

    #[test]
    fn test_close_session() {
        let mock = MockTransport::new();
        mock.queue_response(mock_success_response(0x01));

        let device = HhkbDevice::new(mock);
        device.close_session().expect("close_session should succeed");

        let req = device.transport.get_write(0).unwrap();
        assert_eq!(req[3], 0x01, "NotifyApplicationState cmd");
        assert_eq!(req[6], 0x01, "state byte = close(1)");
    }

    // -- Queries ------------------------------------------------------------

    #[test]
    fn test_get_mode() {
        let mock = MockTransport::new();
        let mut resp = mock_success_response(0x06);
        resp[6] = 1; // mode = Mac
        mock.queue_response(resp);

        let device = HhkbDevice::new(mock);
        let mode = device.get_mode().expect("get_mode should succeed");
        assert_eq!(mode, KeyboardMode::Mac);

        let req = device.transport.get_write(0).unwrap();
        assert_eq!(req[3], 0x06);
    }

    #[test]
    fn test_get_info() {
        let mock = MockTransport::new();
        let mut resp = mock_success_response(0x02);

        // Response data begins at offset 6.
        let name = b"PD-KB800BNS";
        resp[6..6 + name.len()].copy_from_slice(name);
        // Revision
        resp[6 + 20] = 1;
        resp[6 + 21] = 0;
        resp[6 + 22] = 0;
        resp[6 + 23] = 2;
        // Serial (0xAA pattern)
        for b in &mut resp[6 + 24..6 + 40] {
            *b = 0xAA;
        }
        // App firmware
        resp[6 + 40] = 2;
        resp[6 + 41] = 0;
        // Boot firmware
        resp[6 + 48] = 1;
        // Running firmware = Application
        resp[6 + 56] = 0;

        mock.queue_response(resp);

        let device = HhkbDevice::new(mock);
        let info = device.get_info().expect("get_info should succeed");

        assert_eq!(info.type_number, "PD-KB800BNS");
        assert_eq!(info.revision, [1, 0, 0, 2]);
        assert_eq!(info.serial, [0xAA; 16]);
        assert_eq!(info.app_firmware[0], 2);
        assert_eq!(info.boot_firmware[0], 1);
        assert_eq!(info.running_firmware, FirmwareType::Application);

        let req = device.transport.get_write(0).unwrap();
        assert_eq!(req[3], 0x02, "GetKeyboardInformation cmd");
    }

    #[test]
    fn test_get_dip_switch() {
        let mock = MockTransport::new();
        let mut resp = mock_success_response(0x05);
        resp[6] = 1;
        resp[7] = 0;
        resp[8] = 1;
        resp[9] = 0;
        resp[10] = 1;
        resp[11] = 0;
        mock.queue_response(resp);

        let device = HhkbDevice::new(mock);
        let state = device.get_dip_switch().unwrap();
        assert_eq!(state.switches, [true, false, true, false, true, false]);
    }

    // -- Read keymap --------------------------------------------------------

    #[test]
    fn test_read_keymap() {
        let mock = MockTransport::new();

        // Build 3 chunks with recognizable data.
        let mut chunk1 = vec![0u8; 64];
        let mut chunk2 = vec![0u8; 64];
        let mut chunk3 = vec![0u8; 64];

        // Header bytes are ignored by from_chunks (it reads starting at [6]).
        // Give the chunks valid-looking framing anyway.
        for c in [&mut chunk1, &mut chunk2, &mut chunk3] {
            c[0] = 0x55;
            c[1] = 0x55;
            c[2] = 0x87;
        }

        // Fill keymap data: chunk1[6..64] = layout[0..58], chunk2 similar,
        // chunk3[6..18] = layout[116..128].
        for i in 0..58 {
            chunk1[6 + i] = i as u8; // layout[0..58]  = 0..58
        }
        for i in 0..58 {
            chunk2[6 + i] = (58 + i) as u8; // layout[58..116] = 58..116
        }
        for i in 0..12 {
            chunk3[6 + i] = (116 + i) as u8; // layout[116..128] = 116..128
        }

        mock.queue_response(chunk1);
        mock.queue_response(chunk2);
        mock.queue_response(chunk3);

        let device = HhkbDevice::new(mock);
        let keymap = device
            .read_keymap(KeyboardMode::Mac, false)
            .expect("read_keymap should succeed");

        // Verify the request.
        let req = device.transport.get_write(0).unwrap();
        assert_eq!(req[3], 0x87, "GetKeymap cmd");
        assert_eq!(req[5], 0x02, "param2 must be 2");
        assert_eq!(req[6], u8::from(KeyboardMode::Mac));
        assert_eq!(req[7], 0, "base layer");

        // Verify every byte of the assembled keymap.
        for i in 0..KEYMAP_SIZE {
            assert_eq!(
                keymap.get(i),
                Some(i as u8),
                "keymap byte {} mismatch",
                i
            );
        }
    }

    // -- Write keymap -------------------------------------------------------

    #[test]
    fn test_write_keymap() {
        let mock = MockTransport::new();

        // Queue 5 ACKs: 3 chunk writes + confirm + reset_dipsw.
        mock.queue_response(mock_success_response(0x86));
        mock.queue_response(mock_success_response(0x86));
        mock.queue_response(mock_success_response(0x86));
        mock.queue_response(mock_success_response(0x04));
        mock.queue_response(mock_success_response(0x07));

        // Build a keymap with distinct byte values.
        let mut raw = [0u8; KEYMAP_SIZE];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(2).wrapping_add(1);
        }
        let keymap = Keymap::from_bytes(raw);

        let device = HhkbDevice::new(mock);
        device
            .write_keymap(KeyboardMode::HHK, true, &keymap)
            .expect("write_keymap should succeed");

        let writes = device.transport.get_writes();
        assert_eq!(writes.len(), 5, "expected 5 writes (3 chunks + confirm + reset)");

        // -- Write 1: set_keymap_write1 --
        let w1 = &writes[0];
        assert_eq!(w1[3], 0x86, "SetKeymap cmd");
        assert_eq!(w1[4], 65);
        assert_eq!(w1[5], 59);
        assert_eq!(w1[6], u8::from(KeyboardMode::HHK));
        assert_eq!(w1[7], 1, "fn layer = true");
        assert_eq!(&w1[8..65], &raw[0..57]);

        // -- Write 2: set_keymap_write2 --
        let w2 = &writes[1];
        assert_eq!(w2[3], 0x86);
        assert_eq!(w2[4], 130);
        assert_eq!(w2[5], 59);
        assert_eq!(&w2[6..65], &raw[57..116]);

        // -- Write 3: set_keymap_write3 --
        let w3 = &writes[2];
        assert_eq!(w3[3], 0x86);
        assert_eq!(w3[4], 195);
        assert_eq!(w3[5], 12);
        assert_eq!(&w3[6..18], &raw[116..128]);

        // -- Write 4: confirm_keymap --
        let w4 = &writes[3];
        assert_eq!(w4[3], 0x04, "ConfirmKeymap cmd");

        // -- Write 5: reset_dipsw --
        let w5 = &writes[4];
        assert_eq!(w5[3], 0x07, "ResetDIPSW cmd");
    }

    // -- Error propagation --------------------------------------------------

    #[test]
    fn test_command_failed_status_returns_error() {
        let mock = MockTransport::new();
        let mut resp = mock_success_response(0x06);
        resp[3] = 0xFF; // non-zero status = failure
        mock.queue_response(resp);

        let device = HhkbDevice::new(mock);
        let err = device.get_mode().unwrap_err();
        assert!(matches!(
            err,
            Error::CommandFailed {
                cmd: 0x06,
                status: 0xFF
            }
        ));
    }
}
