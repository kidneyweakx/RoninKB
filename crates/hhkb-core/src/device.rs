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

    // ---- Firmware (read-only, safe) --------------------------------------

    /// Dump the firmware from the device. Returns the raw firmware bytes.
    ///
    /// Sends a `DumpFirmware` (0xD0) request and then reads response packets
    /// until a short packet is seen (happy-hacking-gnu uses `read >= 56` as
    /// the continuation condition; `read = buf[5] - 2`).
    ///
    /// Each response packet carries its payload length at `buf[5]` and the
    /// actual firmware bytes at `buf[8..8 + (buf[5] - 2)]`. The final packet
    /// has `buf[5] < 58` (so payload < 56) and is still included in the
    /// output — this matches the reference behaviour exactly.
    ///
    /// This is **read-only** and cannot brick the device.
    pub fn dump_firmware(&self) -> Result<Vec<u8>> {
        // Hard cap so a malfunctioning / mock-driven read loop can't spin
        // forever. Real HHKB firmware is well under 300 KiB.
        const MAX_FIRMWARE_BYTES: usize = 512 * 1024;

        let req = command::dump_firmware();
        self.transport.write(&req)?;

        let mut out: Vec<u8> = Vec::with_capacity(128 * 1024);
        loop {
            let chunk = self.read_chunk()?;

            // buf[5] encodes payload_len + 2. If it's <2 we can't trust it —
            // treat as end of stream.
            let marker = chunk[5] as usize;
            if marker < 2 {
                break;
            }
            let payload_len = marker - 2;

            // Bounds: data lives at [8..8 + payload_len], and the chunk is
            // 64 bytes total, so clamp defensively.
            let end = (8 + payload_len).min(chunk.len());
            if end > 8 {
                out.extend_from_slice(&chunk[8..end]);
            }

            if out.len() > MAX_FIRMWARE_BYTES {
                return Err(Error::Transport(format!(
                    "dump_firmware exceeded {MAX_FIRMWARE_BYTES} bytes — runaway read"
                )));
            }

            // Termination: happy-hacking-gnu stops when the decoded payload
            // length drops below 56 bytes. We include the final short packet.
            if payload_len < 56 {
                break;
            }
        }

        Ok(out)
    }

    /// Write a full 128-byte keymap to the device.
    ///
    /// Sends 3 `SetKeymap` writes (each followed by an ACK read), then
    /// `ConfirmKeymap` and `ResetDIPSW`.
    pub fn write_keymap(&self, mode: KeyboardMode, fn_layer: bool, keymap: &Keymap) -> Result<()> {
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
// Feature-gated firmware write API (DANGEROUS — can brick the device)
// ---------------------------------------------------------------------------

#[cfg(feature = "firmware-write")]
impl<T: HidTransport> HhkbDevice<T> {
    /// DANGEROUS: perform a full firmware update.
    ///
    /// Sequence (matches happy-hacking-gnu `hhkb_firmup`):
    ///   1. `FirmupModeChange` (0xE0) — device disconnects/reconnects.
    ///      NOTE: this method does **not** re-open the underlying transport
    ///      for you; the caller is responsible for managing reconnection if
    ///      their transport requires it. The C reference closes and reopens
    ///      the handle around this step.
    ///   2. `FirmupStart` (0xE1) with file size + CRC-16.
    ///   3. `FirmupSend` (0xE2) in a loop, up to 57 bytes per packet,
    ///      incrementing `packet_num` from 0.
    ///   4. `FirmupEnd` (0xE3).
    ///
    /// `firmware` must be the raw firmware file bytes **including** the
    /// leading 2-byte CRC header; the implementation skips those 2 bytes
    /// when chunking (matching the reference's `fw->raw_data + 2` /
    /// `fw->file_size - 2` convention) and uses them as the CRC.
    ///
    /// # Safety
    ///
    /// This is not memory-unsafe, but misuse can **permanently brick** the
    /// keyboard. The `unsafe` keyword is here purely as a speed bump. You
    /// must:
    /// - verify you have the correct firmware file for your hardware,
    /// - call [`Self::open_session`] first,
    /// - be prepared for the device to disconnect after step 1.
    pub unsafe fn firmware_update(&self, firmware: &[u8]) -> Result<()> {
        if firmware.len() < 2 {
            return Err(Error::Transport(
                "firmware_update: firmware too short (need >= 2 bytes for CRC header)".into(),
            ));
        }

        // Step 1: enter firmware update mode.
        let req = command::firmup_mode_change();
        self.send_and_receive(&req)?;

        // Extract CRC header and payload per the reference layout.
        let crc16 = u16::from_le_bytes([firmware[0], firmware[1]]);
        let payload = &firmware[2..];
        let file_size = (firmware.len()) as u32;

        // Step 2: firmup_start with file_size + crc.
        let req = command::firmup_start(file_size, crc16);
        self.send_and_receive(&req)?;

        // Step 3: chunked send.
        let mut packet_num: u16 = 0;
        for chunk in payload.chunks(command::FIRMUP_SEND_CHUNK_MAX) {
            let req = command::firmup_send(packet_num, chunk);
            let resp = self.send_and_receive(&req)?;
            // Reference verifies the echoed packet number at resp.data[0..2]
            // (which corresponds to raw buf[6..8] — i.e. the start of the
            // parsed `data` region).
            if resp.data.len() >= 2 {
                let echoed = u16::from_le_bytes([resp.data[0], resp.data[1]]);
                if echoed != packet_num {
                    return Err(Error::Transport(format!(
                        "firmware_update: packet number mismatch (expected {packet_num}, got {echoed})"
                    )));
                }
            }
            packet_num = packet_num.wrapping_add(1);
        }

        // Step 4: firmup_end.
        let req = command::firmup_end();
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
        device
            .close_session()
            .expect("close_session should succeed");

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
            assert_eq!(keymap.get(i), Some(i as u8), "keymap byte {} mismatch", i);
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
        assert_eq!(
            writes.len(),
            5,
            "expected 5 writes (3 chunks + confirm + reset)"
        );

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

    // -- Firmware dump (read-only) -----------------------------------------

    /// Build a firmware dump packet with `payload_len` bytes of data
    /// starting at `start_val` (wrapping pattern). Encodes `buf[5] = len+2`.
    fn mock_firmware_packet(payload_len: usize, start_val: u8) -> Vec<u8> {
        assert!(payload_len <= 56, "firmware payload per packet is <= 56");
        let mut resp = vec![0u8; 64];
        resp[0] = 0x55;
        resp[1] = 0x55;
        resp[2] = 0xD0;
        resp[3] = 0x00; // status
        resp[4] = 0x00;
        resp[5] = (payload_len + 2) as u8;
        for i in 0..payload_len {
            resp[8 + i] = start_val.wrapping_add(i as u8);
        }
        resp
    }

    #[test]
    fn test_dump_firmware_via_device() {
        let mock = MockTransport::new();

        // Expected: happy-hacking-gnu reads packets where buf[5] - 2 is the
        // data length and stops once length drops below 56. The packet size
        // that the keyboard actually uses is 56 data bytes (buf[5] = 58),
        // so we stream 3 full-size packets and then a zero-length terminator.
        //
        // NOTE: the task specified "3 chunks of 58 bytes = 174 bytes", but
        // the wire format can only carry 56 data bytes per packet (the
        // chunk is `buf[8..64]`, 56 bytes). We honour the spirit of the
        // request: 3 full-size data chunks followed by a terminator.
        mock.queue_response(mock_firmware_packet(56, 0x00));
        mock.queue_response(mock_firmware_packet(56, 0x38));
        mock.queue_response(mock_firmware_packet(56, 0x70));
        // Terminator: payload_len < 56 (empty packet).
        mock.queue_response(mock_firmware_packet(0, 0x00));

        let device = HhkbDevice::new(mock);
        let fw = device
            .dump_firmware()
            .expect("dump_firmware should succeed");

        // 3 full packets × 56 bytes = 168 bytes.
        assert_eq!(fw.len(), 56 * 3);
        // Verify the content pattern matches the sequential fill.
        for (i, &b) in fw.iter().enumerate() {
            assert_eq!(b, i as u8, "firmware byte {i} mismatch");
        }

        // Verify we sent exactly one write (the DUMP_FIRMWARE request).
        let writes = device.transport.get_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0][3], 0xD0);
    }

    #[test]
    fn test_dump_firmware_stops_on_short_packet() {
        let mock = MockTransport::new();
        // One full packet, then a partial (30 bytes) which should terminate.
        mock.queue_response(mock_firmware_packet(56, 0x00));
        mock.queue_response(mock_firmware_packet(30, 0x38));

        let device = HhkbDevice::new(mock);
        let fw = device.dump_firmware().unwrap();
        assert_eq!(fw.len(), 56 + 30);
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

    // -- Feature-gated firmware-write tests --------------------------------

    #[cfg(feature = "firmware-write")]
    #[test]
    fn test_firmware_update_calls_correct_sequence() {
        // Build a tiny firmware blob: 2-byte CRC header + 120 bytes payload.
        // With FIRMUP_SEND_CHUNK_MAX=57, 120 bytes = 3 chunks (57 + 57 + 6).
        let mut firmware = vec![0u8; 2 + 120];
        firmware[0] = 0xEF; // crc low
        firmware[1] = 0xBE; // crc high  -> crc16 = 0xBEEF
        for i in 0..120 {
            firmware[2 + i] = i as u8;
        }

        let mock = MockTransport::new();

        // 1) mode change ACK
        mock.queue_response(mock_success_response(0xE0));
        // 2) firmup start ACK
        mock.queue_response(mock_success_response(0xE1));
        // 3) Three firmup_send ACKs. Each must echo the packet number at
        //    resp[6..8] (which lands in resp.data[0..2]).
        for n in 0u16..3 {
            let mut resp = mock_success_response(0xE2);
            let bytes = n.to_le_bytes();
            resp[6] = bytes[0];
            resp[7] = bytes[1];
            mock.queue_response(resp);
        }
        // 4) firmup end ACK
        mock.queue_response(mock_success_response(0xE3));

        let device = HhkbDevice::new(mock);
        unsafe {
            device
                .firmware_update(&firmware)
                .expect("firmware_update should succeed");
        }

        let writes = device.transport.get_writes();
        // mode_change + start + 3×send + end = 6 writes
        assert_eq!(writes.len(), 6, "expected 6 total writes");

        assert_eq!(writes[0][3], 0xE0, "write 0: FirmupModeChange");

        // Write 1: FirmupStart
        assert_eq!(writes[1][3], 0xE1);
        assert_eq!(writes[1][4], 0x00);
        assert_eq!(writes[1][5], 0x08);
        assert_eq!(&writes[1][6..10], &(firmware.len() as u32).to_le_bytes());
        assert_eq!(&writes[1][10..12], &0xBEEFu16.to_le_bytes());

        // Writes 2..5: FirmupSend, packet_num 0..3, chunks of 57/57/6.
        for (i, &(pn, expected_len)) in [(0u16, 57usize), (1, 57), (2, 6)].iter().enumerate() {
            let w = &writes[2 + i];
            assert_eq!(w[3], 0xE2, "write {} cmd", 2 + i);
            assert_eq!(w[4], 0x00);
            assert_eq!(w[5], (expected_len as u8) + 2);
            assert_eq!(&w[6..8], &pn.to_le_bytes());
            let data_start = 8;
            let src_start = pn as usize * 57;
            assert_eq!(
                &w[data_start..data_start + expected_len],
                &firmware[2 + src_start..2 + src_start + expected_len],
            );
        }

        // Write 5: FirmupEnd
        assert_eq!(writes[5][3], 0xE3);
    }

    #[cfg(feature = "firmware-write")]
    #[test]
    fn test_firmware_update_rejects_tiny_firmware() {
        let mock = MockTransport::new();
        let device = HhkbDevice::new(mock);
        let err = unsafe { device.firmware_update(&[0x01]) }.unwrap_err();
        assert!(matches!(err, Error::Transport(_)));
    }
}
