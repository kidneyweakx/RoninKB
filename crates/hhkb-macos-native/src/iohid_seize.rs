//! Path B: `IOHIDManager` with `kIOHIDOptionsTypeSeizeDevice` for external
//! keyboards on macOS 26 Tahoe (where `kCGSessionEventTap` no longer sees
//! external HID input).
//!
//! Decodes the standard 8-byte HID boot keyboard report (modifier byte +
//! reserved + up to 6 pressed usage codes) and diffs it against the previous
//! report to derive press/release events. Vendor-specific or report-protocol
//! reports of other lengths are logged and skipped — fine for the M1 PoC.
//!
//! The user callback is invoked on the CFRunLoop thread that processes the
//! seized device's input reports. The caller is responsible for keeping a
//! CFRunLoop alive on that thread (e.g. `CFRunLoop::run_current()`).

use std::ffi::{c_char, c_void, CStr};
use std::ptr;
use std::sync::Mutex;

use core_foundation::base::{CFIndex, CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoopGetCurrent};
use core_foundation::set::{CFSetGetCount, CFSetGetValues};
use core_foundation::string::CFString;

use io_kit_sys::hid::base::IOHIDDeviceRef;
use io_kit_sys::hid::device::{
    IOHIDDeviceGetProperty, IOHIDDeviceOpen, IOHIDDeviceRegisterInputReportCallback,
    IOHIDDeviceScheduleWithRunLoop,
};
use io_kit_sys::hid::keys::{kIOHIDOptionsTypeSeizeDevice, IOHIDReportType};
use io_kit_sys::hid::manager::{
    kIOHIDManagerOptionNone, IOHIDManagerCopyDevices, IOHIDManagerCreate, IOHIDManagerOpen,
    IOHIDManagerRef, IOHIDManagerScheduleWithRunLoop, IOHIDManagerSetDeviceMatching,
};
use io_kit_sys::ret::{kIOReturnNotPermitted, kIOReturnSuccess};

use crate::error::{Error, Result};
use crate::keycode::HidUsage;

/// `kHIDPage_GenericDesktop` from `<IOKit/hid/IOHIDUsageTables.h>`.
const HID_PAGE_GENERIC_DESKTOP: i32 = 0x01;
/// `kHIDUsage_GD_Keyboard` from `<IOKit/hid/IOHIDUsageTables.h>`.
const HID_USAGE_GD_KEYBOARD: i32 = 0x06;

/// Standard HID boot-protocol keyboard report length: 1 modifier byte +
/// 1 reserved + 6 keycode slots.
const BOOT_REPORT_LEN: usize = 8;

/// Per-device input buffer size. Real keyboards produce 8-byte boot reports;
/// allocate more to absorb the occasional vendor report without trashing the
/// CFRunLoop on overflow.
const REPORT_BUF_LEN: usize = 64;

/// Press/release event derived from a decoded HID input report.
#[derive(Debug, Clone, Copy)]
pub enum HidEvent {
    Pressed(HidUsage),
    Released(HidUsage),
}

/// Heap state owned by the manager and pointed to from the C callback context.
///
/// The callback re-enters us via raw pointer, so the layout must stay stable
/// for the lifetime of the manager. We use `Mutex` because input report
/// callbacks fire on the CFRunLoop thread but `Send` user callbacks may
/// internally synchronize themselves; the mutex narrows the race to a single
/// `&mut` borrow per report and protects the previous-report cache.
struct CallbackState {
    user_cb: Mutex<Box<dyn FnMut(HidEvent) + Send + 'static>>,
    /// Previous decoded keyboard state per device. Keyed by `IOHIDDeviceRef`
    /// pointer value (devices live for the manager's lifetime, no churn).
    prev_state: Mutex<Vec<(usize, KeyboardState)>>,
}

/// Decoded boot-protocol keyboard state — modifiers as a bitmap + the array
/// of currently-pressed usage codes (0x00 = empty slot).
#[derive(Default, Clone, Copy)]
struct KeyboardState {
    modifiers: u8,
    keys: [u8; 6],
}

/// Owns all CF/IOHID resources for one matched IOHIDManager.
pub struct SeizeManager {
    manager: IOHIDManagerRef,
    /// Retained refs to every device we opened-with-seize, so we hold a strong
    /// ref until `Drop`. The CFSet returned by `IOHIDManagerCopyDevices` is
    /// release()d after we copy refs out.
    seized_devices: Vec<IOHIDDeviceRef>,
    /// Boxed report buffers, one per device. The pointer is registered with
    /// `IOHIDDeviceRegisterInputReportCallback` and must outlive the device
    /// being scheduled. Stored as `Box<[u8]>` (unsized slice) so each
    /// allocation has a stable heap address regardless of `Vec` resizing.
    _report_buffers: Vec<Box<[u8]>>,
    /// Boxed callback state; raw pointer was passed to IOKit as the C context.
    callback_state: *mut CallbackState,
}

// SeizeManager isn't Send/Sync — the IOHIDManagerRef is bound to the
// CFRunLoop it was scheduled on. That's fine; the caller drives that loop.

impl SeizeManager {
    /// Open IOHIDManager, match keyboards, seize the external ones, register
    /// the input report callback. Caller drives the CFRunLoop afterward.
    ///
    /// On Apple Silicon / Tahoe, the built-in keyboard appears with
    /// `Transport == "SPI"` (or "FIFO" pre-T2). External devices use "USB" or
    /// "Bluetooth". We skip built-ins entirely so the user can keep typing
    /// in Terminal during the spike.
    pub fn open_external_keyboards<F>(callback: F) -> Result<Self>
    where
        F: FnMut(HidEvent) + Send + 'static,
    {
        unsafe {
            let manager = IOHIDManagerCreate(ptr::null_mut(), kIOHIDManagerOptionNone);
            if manager.is_null() {
                return Err(Error::IoHidOpenFailed(0));
            }

            // Build {DeviceUsagePage: 0x01, DeviceUsage: 0x06} matching dict.
            let usage_page_key = CFString::from_static_string("DeviceUsagePage");
            let usage_key = CFString::from_static_string("DeviceUsage");
            let usage_page = CFNumber::from(HID_PAGE_GENERIC_DESKTOP);
            let usage = CFNumber::from(HID_USAGE_GD_KEYBOARD);
            let pairs: [(CFType, CFType); 2] = [
                (usage_page_key.as_CFType(), usage_page.as_CFType()),
                (usage_key.as_CFType(), usage.as_CFType()),
            ];
            let matching = CFDictionary::from_CFType_pairs(&pairs);
            IOHIDManagerSetDeviceMatching(manager, matching.as_concrete_TypeRef());

            let open_ret = IOHIDManagerOpen(manager, kIOHIDManagerOptionNone);
            if open_ret != kIOReturnSuccess {
                CFRelease(manager as CFTypeRef);
                return Err(Error::IoHidOpenFailed(open_ret));
            }

            IOHIDManagerScheduleWithRunLoop(manager, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode);

            // Allocate the callback context up front; we'll register it per
            // seized device below.
            let state = Box::into_raw(Box::new(CallbackState {
                user_cb: Mutex::new(Box::new(callback)),
                prev_state: Mutex::new(Vec::new()),
            }));

            let device_set = IOHIDManagerCopyDevices(manager);
            if device_set.is_null() {
                tracing::warn!("IOHIDManagerCopyDevices returned null — no keyboards matched yet");
                return Ok(SeizeManager {
                    manager,
                    seized_devices: Vec::new(),
                    _report_buffers: Vec::new(),
                    callback_state: state,
                });
            }

            let count = CFSetGetCount(device_set) as usize;
            let mut device_ptrs: Vec<*const c_void> = vec![ptr::null(); count];
            CFSetGetValues(device_set, device_ptrs.as_mut_ptr());

            let mut seized_devices: Vec<IOHIDDeviceRef> = Vec::new();
            let mut report_buffers: Vec<Box<[u8]>> = Vec::new();

            for raw in &device_ptrs {
                let device = *raw as IOHIDDeviceRef;
                if device.is_null() {
                    continue;
                }

                let kind = describe_device(device);
                if kind.is_builtin {
                    tracing::info!(
                        product = %kind.product,
                        transport = %kind.transport,
                        "skipping built-in keyboard (not seized)"
                    );
                    continue;
                }

                tracing::info!(
                    product = %kind.product,
                    transport = %kind.transport,
                    location_id = ?kind.location_id,
                    "opening external keyboard with seize"
                );

                let ret = IOHIDDeviceOpen(device, kIOHIDOptionsTypeSeizeDevice);
                if ret != kIOReturnSuccess {
                    if ret == kIOReturnNotPermitted {
                        // Drop the manager; surface the permission error.
                        let _ = Box::from_raw(state);
                        CFRelease(device_set as CFTypeRef);
                        CFRelease(manager as CFTypeRef);
                        return Err(Error::IoHidSeizeFailed {
                            device: kind.product.clone(),
                            code: ret,
                        });
                    }
                    tracing::warn!(
                        product = %kind.product,
                        code = format!("{:#x}", ret),
                        "IOHIDDeviceOpen seize failed; skipping device"
                    );
                    continue;
                }

                // Retain so the device outlives `device_set`.
                CFRetain(device as CFTypeRef);

                let mut buf: Box<[u8]> = vec![0u8; REPORT_BUF_LEN].into_boxed_slice();
                IOHIDDeviceRegisterInputReportCallback(
                    device,
                    buf.as_mut_ptr(),
                    REPORT_BUF_LEN as CFIndex,
                    Some(input_report_trampoline),
                    state as *mut c_void,
                );
                IOHIDDeviceScheduleWithRunLoop(
                    device,
                    CFRunLoopGetCurrent(),
                    kCFRunLoopDefaultMode,
                );

                seized_devices.push(device);
                report_buffers.push(buf);
            }

            CFRelease(device_set as CFTypeRef);

            tracing::info!(seized = seized_devices.len(), "SeizeManager ready");

            Ok(SeizeManager {
                manager,
                seized_devices,
                _report_buffers: report_buffers,
                callback_state: state,
            })
        }
    }

    /// How many devices were matched and successfully opened with seize.
    pub fn seized_device_count(&self) -> usize {
        self.seized_devices.len()
    }

    /// Close the manager and release all seized devices. Equivalent to
    /// dropping; provided for callers who want to be explicit.
    pub fn close(self) {
        drop(self);
    }
}

impl Drop for SeizeManager {
    fn drop(&mut self) {
        unsafe {
            for device in self.seized_devices.drain(..) {
                let _ =
                    io_kit_sys::hid::device::IOHIDDeviceClose(device, kIOHIDOptionsTypeSeizeDevice);
                CFRelease(device as CFTypeRef);
            }
            if !self.manager.is_null() {
                let _ = io_kit_sys::hid::manager::IOHIDManagerClose(
                    self.manager,
                    kIOHIDManagerOptionNone,
                );
                CFRelease(self.manager as CFTypeRef);
            }
            if !self.callback_state.is_null() {
                let _ = Box::from_raw(self.callback_state);
            }
        }
    }
}

/// Static `extern "C"` trampoline registered with IOKit; dispatches to the
/// boxed user callback after decoding the report.
unsafe extern "C" fn input_report_trampoline(
    context: *mut c_void,
    _result: io_kit_sys::ret::IOReturn,
    sender: *mut c_void,
    _report_type: IOHIDReportType,
    _report_id: u32,
    report: *mut u8,
    report_length: CFIndex,
) {
    if context.is_null() || report.is_null() {
        return;
    }
    let len = report_length as usize;
    if len != BOOT_REPORT_LEN {
        // Vendor / report-protocol report — beyond the PoC's scope.
        tracing::debug!(len, "skipping non-boot HID keyboard report");
        return;
    }

    // SAFETY: pointer was registered as the boxed CallbackState; macOS calls
    // us on the CFRunLoop thread, single-threaded relative to ourselves.
    let state = unsafe { &*(context as *const CallbackState) };
    let bytes = unsafe { std::slice::from_raw_parts(report, len) };

    let device_key = sender as usize;
    let mut prev_guard = match state.prev_state.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let prev = match prev_guard.iter().position(|(k, _)| *k == device_key) {
        Some(idx) => prev_guard[idx].1,
        None => {
            prev_guard.push((device_key, KeyboardState::default()));
            KeyboardState::default()
        }
    };

    let curr = KeyboardState {
        modifiers: bytes[0],
        keys: [bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
    };

    let events = diff_states(&prev, &curr);

    if let Some(slot) = prev_guard.iter_mut().find(|(k, _)| *k == device_key) {
        slot.1 = curr;
    }
    drop(prev_guard);

    if !events.is_empty() {
        if let Ok(mut cb) = state.user_cb.lock() {
            for ev in events {
                cb(ev);
            }
        }
    }
}

/// Diff a previous keyboard state against the current one and produce
/// press/release events for both modifier bits and the keycode array.
fn diff_states(prev: &KeyboardState, curr: &KeyboardState) -> Vec<HidEvent> {
    let mut out = Vec::new();

    // Modifiers: bit 0 = LCtrl (0xE0), bit 1 = LShift (0xE1), bit 2 = LAlt
    // (0xE2), bit 3 = LGui (0xE3), bit 4..7 = right-side equivalents.
    for bit in 0..8u8 {
        let mask = 1u8 << bit;
        let was = (prev.modifiers & mask) != 0;
        let now = (curr.modifiers & mask) != 0;
        if was != now {
            let usage = HidUsage(0xE0 + u16::from(bit));
            out.push(if now {
                HidEvent::Pressed(usage)
            } else {
                HidEvent::Released(usage)
            });
        }
    }

    // Keys: any code in `curr` that wasn't in `prev` is a press; any in
    // `prev` not in `curr` is a release. 0x00 entries are empty slots.
    for &code in &curr.keys {
        if code != 0 && !prev.keys.contains(&code) {
            out.push(HidEvent::Pressed(HidUsage(u16::from(code))));
        }
    }
    for &code in &prev.keys {
        if code != 0 && !curr.keys.contains(&code) {
            out.push(HidEvent::Released(HidUsage(u16::from(code))));
        }
    }

    out
}

/// Snapshot of an enumerated device's identifying properties.
pub struct DeviceKind {
    pub product: String,
    pub transport: String,
    pub location_id: Option<i64>,
    pub is_builtin: bool,
}

/// Read product / transport / location id from a device and decide whether
/// it should be considered built-in for the purposes of skipping seize.
pub fn describe_device(device: IOHIDDeviceRef) -> DeviceKind {
    let product = string_property(device, "Product").unwrap_or_else(|| "<unknown>".to_string());
    let transport = string_property(device, "Transport").unwrap_or_default();
    let location_id = number_property(device, "LocationID");

    // Apple Silicon built-in keyboards expose Transport=SPI, older T2 macs
    // expose FIFO; both lack a LocationID. External USB / Bluetooth keyboards
    // expose Transport=USB or Bluetooth and have a LocationID. Treat the
    // transport as authoritative — LocationID is the secondary check.
    let is_builtin = matches!(transport.as_str(), "SPI" | "FIFO" | "AIDB" | "I2C")
        || (location_id.is_none()
            && !matches!(
                transport.as_str(),
                "USB" | "Bluetooth" | "BluetoothLowEnergy"
            ));

    DeviceKind {
        product,
        transport,
        location_id,
        is_builtin,
    }
}

fn string_property(device: IOHIDDeviceRef, key: &str) -> Option<String> {
    unsafe {
        let cf_key = cstr_to_cfstring(key);
        let value = IOHIDDeviceGetProperty(device, cf_key.as_concrete_TypeRef());
        if value.is_null() {
            return None;
        }
        let cf: CFString = CFString::wrap_under_get_rule(value as _);
        Some(cf.to_string())
    }
}

fn number_property(device: IOHIDDeviceRef, key: &str) -> Option<i64> {
    unsafe {
        let cf_key = cstr_to_cfstring(key);
        let value = IOHIDDeviceGetProperty(device, cf_key.as_concrete_TypeRef());
        if value.is_null() {
            return None;
        }
        let cf: CFNumber = CFNumber::wrap_under_get_rule(value as _);
        cf.to_i64()
    }
}

/// IOKit's HID property keys are NUL-terminated `*const c_char`. The crate
/// exposes them as static byte literals; build a `CFString` from a Rust `&str`
/// (we only ever query well-known ASCII keys).
fn cstr_to_cfstring(s: &str) -> CFString {
    CFString::new(s)
}

#[allow(dead_code)]
fn _unused_cstr_helper(p: *const c_char) -> String {
    // Kept for future use if we want to compare CFString to the IOKit-provided
    // raw-bytes constants (e.g. `kIOHIDProductKey`). The current path uses the
    // Rust `&str` keys directly — same wire bytes.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_emits_press_for_new_keycode() {
        let prev = KeyboardState::default();
        let curr = KeyboardState {
            modifiers: 0,
            keys: [0x04, 0, 0, 0, 0, 0],
        };
        let events = diff_states(&prev, &curr);
        assert_eq!(events.len(), 1);
        match events[0] {
            HidEvent::Pressed(u) => assert_eq!(u, HidUsage::A),
            _ => panic!("expected pressed"),
        }
    }

    #[test]
    fn diff_emits_release_when_key_disappears() {
        let prev = KeyboardState {
            modifiers: 0,
            keys: [0x04, 0, 0, 0, 0, 0],
        };
        let curr = KeyboardState::default();
        let events = diff_states(&prev, &curr);
        assert_eq!(events.len(), 1);
        match events[0] {
            HidEvent::Released(u) => assert_eq!(u, HidUsage::A),
            _ => panic!("expected released"),
        }
    }

    #[test]
    fn diff_modifier_bit_to_usage() {
        let prev = KeyboardState::default();
        let curr = KeyboardState {
            modifiers: 0b0000_0001, // LCtrl
            keys: [0; 6],
        };
        let events = diff_states(&prev, &curr);
        assert!(matches!(events.as_slice(), [HidEvent::Pressed(u)] if *u == HidUsage::LEFT_CTRL));
    }

    #[test]
    fn diff_no_change_emits_nothing() {
        let s = KeyboardState {
            modifiers: 0b1010_0101,
            keys: [0x04, 0x05, 0, 0, 0, 0],
        };
        assert!(diff_states(&s, &s).is_empty());
    }
}
