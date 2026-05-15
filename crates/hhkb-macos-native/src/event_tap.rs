//! Path A: `CGEventTap` at `kCGSessionEventTap` with `headInsertEventTap`
//! placement. Used for built-in keyboards on every macOS, and for all
//! keyboards on pre-Tahoe systems.
//!
//! Synthetic events we post via `inject` are tagged with
//! `SYNTHETIC_USER_DATA` in the `kCGEventSourceUserData` field; the tap
//! callback ignores them so we don't recurse into our own injection.

use std::collections::BTreeSet;
use std::panic::AssertUnwindSafe;
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
///
/// Resilience guarantees:
/// - Callback panics are caught (`catch_unwind`) so a buggy user callback
///   doesn't tear down the tap thread mid-event.
/// - Mutex poisoning is recovered with `into_inner()` instead of `expect()`
///   so a one-time panic doesn't permanently freeze the tap.
/// - `FlagsChanged` events are disambiguated into press/release by tracking
///   the set of currently-pressed modifier usages locally. The OS only
///   tells us "this modifier transitioned"; without this state we'd emit
///   two presses in a row for tap-then-release.
pub fn install_and_run(callback: Box<EventCallback>) -> Result<()> {
    let cb = Arc::new(Mutex::new(callback));
    let mod_state = Arc::new(Mutex::new(BTreeSet::<HidUsage>::new()));

    let cb_for_tap = Arc::clone(&cb);
    let mod_state_for_tap = Arc::clone(&mod_state);
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
            let observed = decode_with_modifier_state(ev_type, cg_keycode, &mod_state_for_tap);

            // Run the user callback under catch_unwind so a panic doesn't
            // propagate into the tap thread (which would then unwind through
            // the C boundary — undefined behaviour). On panic we keep the
            // event and log; the daemon's tap thread keeps running.
            let cb_arc = Arc::clone(&cb_for_tap);
            let verdict = std::panic::catch_unwind(AssertUnwindSafe(move || {
                let mut guard = cb_arc.lock().unwrap_or_else(|p| {
                    tracing::warn!("event-tap callback mutex was poisoned; recovering");
                    p.into_inner()
                });
                (guard)(observed)
            }));

            match verdict {
                Ok(Verdict::PassThrough) => CallbackResult::Keep,
                Ok(Verdict::Suppress) => CallbackResult::Drop,
                Err(_) => {
                    tracing::error!(
                        "event-tap user callback panicked; passing event through to avoid drop"
                    );
                    CallbackResult::Keep
                }
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

/// Decode a CGEvent into an ObservedEvent.
///
/// For `KeyDown` / `KeyUp` the press/release direction is unambiguous. For
/// `FlagsChanged` we consult the locally-tracked modifier set: if the
/// usage is already in the set this is a release; otherwise a press. The
/// OS emits one FlagsChanged per modifier transition, so this round-trips
/// without us reading the CGEvent flags field.
fn decode_with_modifier_state(
    ev_type: CGEventType,
    cg_keycode: u16,
    mod_state: &Arc<Mutex<BTreeSet<HidUsage>>>,
) -> ObservedEvent {
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
        (ObservedKind::FlagsChanged, h) => {
            let mut state = mod_state.lock().unwrap_or_else(|p| {
                tracing::warn!("event-tap modifier-state mutex was poisoned; recovering");
                p.into_inner()
            });
            if state.contains(&h) {
                state.remove(&h);
                ObservedEvent::Released(h)
            } else {
                state.insert(h);
                ObservedEvent::Pressed(h)
            }
        }
    }
}
