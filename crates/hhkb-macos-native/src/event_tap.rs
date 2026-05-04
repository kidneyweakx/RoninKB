//! Path A: `CGEventTap` at `kCGSessionEventTap` with `headInsertEventTap`
//! placement. Used for built-in keyboards on every macOS, and for all
//! keyboards on pre-Tahoe systems.
//!
//! Synthetic events we post via `inject` are tagged with
//! `SYNTHETIC_USER_DATA` in the `kCGEventSourceUserData` field; the tap
//! callback ignores them so we don't recurse into our own injection.

use std::sync::{Arc, Mutex};

use core_foundation::runloop::CFRunLoop;
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField,
};

use crate::error::{Error, Result};
use crate::keycode::{cg_to_hid, CgKeyCode, HidUsage};

/// Sentinel value posted in `kCGEventSourceUserData` on synthetic events
/// emitted by this crate. The tap callback skips events carrying this
/// sentinel so we don't recurse on our own injections.
pub const SYNTHETIC_USER_DATA: i64 = 0x736F_6C69; // "soli"

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedEvent {
    Pressed(HidUsage),
    Released(HidUsage),
    Unknown { cg_keycode: u16, kind: ObservedKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedKind {
    KeyDown,
    KeyUp,
    FlagsChanged,
}

/// What the user callback wants the tap to do with the original event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Let the original event continue down the responder chain.
    PassThrough,
    /// Suppress the original event. Use this for keys we own; the engine
    /// re-injects synthetic events via `inject::post_press / post_release`.
    Suppress,
}

pub type EventCallback = dyn FnMut(ObservedEvent) -> Verdict + Send + 'static;

/// Install a session-level event tap and run the CFRunLoop on the calling
/// thread until the loop is stopped (or the process exits).
///
/// The tap callback runs on this same thread. Make sure the callback returns
/// quickly — the system gives us a budget of a few milliseconds before it
/// disables the tap.
pub fn install_and_run(callback: Box<EventCallback>) -> Result<()> {
    let cb = Arc::new(Mutex::new(callback));

    let cb_for_tap = Arc::clone(&cb);
    let result = CGEventTap::with_enabled(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
        ],
        move |_proxy, ev_type, event| {
            if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA)
                == SYNTHETIC_USER_DATA
            {
                return CallbackResult::Keep;
            }

            let cg_keycode =
                event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            let observed = decode(ev_type, cg_keycode);

            let verdict = {
                let mut guard = cb_for_tap.lock().expect("event-tap callback poisoned");
                (guard)(observed)
            };

            match verdict {
                Verdict::PassThrough => CallbackResult::Keep,
                Verdict::Suppress => CallbackResult::Drop,
            }
        },
        || {
            CFRunLoop::run_current();
        },
    );

    match result {
        Ok(()) => Ok(()),
        Err(()) => Err(Error::EventTapCreateFailed),
    }
}

fn decode(ev_type: CGEventType, cg_keycode: u16) -> ObservedEvent {
    let kind = match ev_type {
        CGEventType::KeyDown => ObservedKind::KeyDown,
        CGEventType::KeyUp => ObservedKind::KeyUp,
        CGEventType::FlagsChanged => ObservedKind::FlagsChanged,
        _ => {
            return ObservedEvent::Unknown {
                cg_keycode,
                kind: ObservedKind::KeyDown,
            }
        }
    };

    let Some(hid) = cg_to_hid(CgKeyCode(cg_keycode)) else {
        return ObservedEvent::Unknown { cg_keycode, kind };
    };

    match (kind, hid) {
        (ObservedKind::KeyDown, h) => ObservedEvent::Pressed(h),
        (ObservedKind::KeyUp, h) => ObservedEvent::Released(h),
        // For modifier keys (FlagsChanged), the tap doesn't tell us press vs
        // release directly. We treat every FlagsChanged as a toggle on the
        // observed key — the engine reconciles the actual state via its
        // internal pressed-set. The OS emits one FlagsChanged per modifier
        // transition, so this round-trips.
        (ObservedKind::FlagsChanged, h) => ObservedEvent::Pressed(h),
    }
}
