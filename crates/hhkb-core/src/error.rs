use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("HID transport error: {0}")]
    Transport(String),

    #[error("invalid magic bytes: expected 0x55 0x55, got 0x{0:02X} 0x{1:02X}")]
    InvalidMagic(u8, u8),

    #[error("command 0x{cmd:02X} failed with status 0x{status:02X}")]
    CommandFailed { cmd: u8, status: u8 },

    #[error("invalid keymap size: expected 128, got {0}")]
    InvalidKeymapSize(usize),

    #[error("device not found: VID=0x{vid:04X} PID=0x{pid:04X}")]
    DeviceNotFound { vid: u16, pid: u16 },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_error_displays_message() {
        let err = Error::Transport("connection lost".into());
        assert_eq!(err.to_string(), "HID transport error: connection lost");
    }

    #[test]
    fn invalid_magic_displays_hex() {
        let err = Error::InvalidMagic(0xAA, 0xBB);
        assert_eq!(
            err.to_string(),
            "invalid magic bytes: expected 0x55 0x55, got 0xAA 0xBB"
        );
    }

    #[test]
    fn command_failed_displays_hex() {
        let err = Error::CommandFailed {
            cmd: 0x10,
            status: 0xFF,
        };
        assert_eq!(err.to_string(), "command 0x10 failed with status 0xFF");
    }

    #[test]
    fn invalid_keymap_size_displays_value() {
        let err = Error::InvalidKeymapSize(64);
        assert_eq!(err.to_string(), "invalid keymap size: expected 128, got 64");
    }

    #[test]
    fn device_not_found_displays_vid_pid() {
        let err = Error::DeviceNotFound {
            vid: 0x04FE,
            pid: 0x0020,
        };
        assert_eq!(err.to_string(), "device not found: VID=0x04FE PID=0x0020");
    }

    #[test]
    fn json_error_converts_from_serde() {
        let json_err: serde_json::Error =
            serde_json::from_str::<String>("not valid json").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Json(_)));
        assert!(err.to_string().starts_with("JSON error:"));
    }

    #[test]
    fn io_error_converts_from_std() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn result_alias_works() {
        fn make_ok(value: u8) -> Result<u8> {
            Ok(value)
        }
        let ok = make_ok(42);
        assert!(matches!(ok, Ok(42)));

        let fail: Result<u8> = Err(Error::InvalidKeymapSize(0));
        assert!(fail.is_err());
    }
}
