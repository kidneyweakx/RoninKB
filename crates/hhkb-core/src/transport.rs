//! Abstract HID transport layer for HHKB device communication.
//!
//! Real-world implementations (hidapi for native, WebHID for wasm) live in
//! their respective backend crates. This module defines the trait they must
//! implement, plus a [`MockTransport`] used by unit tests.

use std::cell::RefCell;
use std::collections::VecDeque;

use crate::error::{Error, Result};

/// Abstract HID transport for device communication.
///
/// Implementations:
/// - `hidapi` (native, behind the `hidapi-backend` feature)
/// - WebHID (wasm)
/// - [`MockTransport`] (tests)
pub trait HidTransport {
    /// Write a 65-byte HID output report (including report ID byte 0).
    fn write(&self, data: &[u8]) -> Result<usize>;

    /// Read a 64-byte HID input report. Timeout in milliseconds (-1 = blocking).
    fn read(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize>;
}

// ---------------------------------------------------------------------------
// MockTransport
// ---------------------------------------------------------------------------

/// Mock transport that records writes and plays back pre-loaded responses.
///
/// Used in unit tests to drive [`crate::device::HhkbDevice`] without any real
/// HID hardware. Uses interior mutability so tests can borrow the device
/// immutably while still queueing responses / asserting on writes.
pub struct MockTransport {
    /// Pre-loaded responses that will be returned by `read()` in order.
    responses: RefCell<VecDeque<Vec<u8>>>,
    /// All data written via `write()`, recorded for assertions.
    writes: RefCell<Vec<Vec<u8>>>,
}

impl MockTransport {
    /// Create an empty mock transport with no queued responses.
    pub fn new() -> Self {
        Self {
            responses: RefCell::new(VecDeque::new()),
            writes: RefCell::new(Vec::new()),
        }
    }

    /// Queue a response that will be returned by the next `read()` call.
    ///
    /// Responses are dequeued in FIFO order.
    pub fn queue_response(&self, data: Vec<u8>) {
        self.responses.borrow_mut().push_back(data);
    }

    /// Get all recorded writes (in the order they occurred).
    pub fn get_writes(&self) -> Vec<Vec<u8>> {
        self.writes.borrow().clone()
    }

    /// Get the Nth write (0-indexed), or `None` if fewer than `index + 1`
    /// writes have occurred.
    pub fn get_write(&self, index: usize) -> Option<Vec<u8>> {
        self.writes.borrow().get(index).cloned()
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl HidTransport for MockTransport {
    fn write(&self, data: &[u8]) -> Result<usize> {
        self.writes.borrow_mut().push(data.to_vec());
        Ok(data.len())
    }

    fn read(&self, buf: &mut [u8], _timeout_ms: i32) -> Result<usize> {
        let mut queue = self.responses.borrow_mut();
        let resp = queue.pop_front().ok_or_else(|| {
            Error::Transport("MockTransport: no queued response for read()".into())
        })?;

        let n = resp.len().min(buf.len());
        buf[..n].copy_from_slice(&resp[..n]);
        Ok(n)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_write_records() {
        let mock = MockTransport::new();
        let payload = vec![0x00, 0xAA, 0xAA, 0x01, 0x02, 0x03];

        let n = mock.write(&payload).unwrap();
        assert_eq!(n, payload.len());

        let writes = mock.get_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0], payload);
        assert_eq!(mock.get_write(0), Some(payload));
        assert_eq!(mock.get_write(1), None);
    }

    #[test]
    fn test_mock_read_returns_queued() {
        let mock = MockTransport::new();

        let first = {
            let mut r = vec![0u8; 64];
            r[0] = 0x55;
            r[1] = 0x55;
            r[2] = 0xAA;
            r
        };
        let second = {
            let mut r = vec![0u8; 64];
            r[0] = 0x55;
            r[1] = 0x55;
            r[2] = 0xBB;
            r
        };

        mock.queue_response(first.clone());
        mock.queue_response(second.clone());

        let mut buf = [0u8; 64];
        let n = mock.read(&mut buf, 1000).unwrap();
        assert_eq!(n, 64);
        assert_eq!(&buf[..], &first[..]);

        let mut buf = [0u8; 64];
        let n = mock.read(&mut buf, 1000).unwrap();
        assert_eq!(n, 64);
        assert_eq!(&buf[..], &second[..]);
    }

    #[test]
    fn test_mock_read_empty_errors() {
        let mock = MockTransport::new();
        let mut buf = [0u8; 64];

        let err = mock.read(&mut buf, 1000).unwrap_err();
        assert!(matches!(err, Error::Transport(_)));
        assert!(
            err.to_string().contains("no queued response"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_mock_multiple_writes_preserve_order() {
        let mock = MockTransport::new();
        mock.write(&[1, 2, 3]).unwrap();
        mock.write(&[4, 5, 6]).unwrap();
        mock.write(&[7, 8, 9]).unwrap();

        let writes = mock.get_writes();
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0], vec![1, 2, 3]);
        assert_eq!(writes[1], vec![4, 5, 6]);
        assert_eq!(writes[2], vec![7, 8, 9]);
    }

    #[test]
    fn test_mock_read_into_smaller_buffer_truncates() {
        let mock = MockTransport::new();
        mock.queue_response(vec![1, 2, 3, 4, 5, 6, 7, 8]);

        let mut buf = [0u8; 4];
        let n = mock.read(&mut buf, -1).unwrap();
        assert_eq!(n, 4);
        assert_eq!(buf, [1, 2, 3, 4]);
    }
}
