use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("input monitoring permission not granted")]
    InputMonitoringDenied,

    #[error("accessibility permission not granted")]
    AccessibilityDenied,

    #[error("could not open CGEventTap (tap creation returned NULL)")]
    EventTapCreateFailed,

    #[error("IOHIDManager open failed: {0:#x}")]
    IoHidOpenFailed(i32),

    #[error("IOHIDDevice seize failed for device {device}: {code:#x}")]
    IoHidSeizeFailed { device: String, code: i32 },

    #[error("unknown HID usage: page={page:#x} usage={usage:#x}")]
    UnknownHidUsage { page: u32, usage: u32 },

    #[error("CFRunLoop missing on the calling thread")]
    NoRunLoop,
}
