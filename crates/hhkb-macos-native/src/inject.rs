//! Re-injection via `CGEventCreateKeyboardEvent` + `CGEventPost`. Posts at
//! `kCGSessionEventTap` so the synthetic event flows through every later
//! tap (e.g. Spotlight, Mission Control hot-keys) the way the real key
//! would have.
//!
//! Each synthetic event carries `event_tap::SYNTHETIC_USER_DATA` in the
//! `kCGEventSourceUserData` field so our own tap callback can skip it.

use core_graphics::event::{CGEvent, CGEventTapLocation, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use crate::error::{Error, Result};
use crate::event_tap::SYNTHETIC_USER_DATA;
use crate::keycode::{hid_to_cg, HidUsage};

fn make_source() -> Result<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|()| Error::EventTapCreateFailed)
}

fn post_one(usage: HidUsage, keydown: bool) -> Result<()> {
    let Some(cg) = hid_to_cg(usage) else {
        // No CG mapping for this HID usage — drop silently. The engine
        // shouldn't be emitting unmapped usages on the PoC layout, but we
        // don't want to panic if it does.
        tracing::debug!(?usage, "drop inject: no CG mapping");
        return Ok(());
    };
    let source = make_source()?;
    let event = CGEvent::new_keyboard_event(source, cg.0, keydown)
        .map_err(|()| Error::EventTapCreateFailed)?;
    event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, SYNTHETIC_USER_DATA);
    event.post(CGEventTapLocation::Session);
    Ok(())
}

pub fn post_press(usage: HidUsage) -> Result<()> {
    post_one(usage, true)
}

pub fn post_release(usage: HidUsage) -> Result<()> {
    post_one(usage, false)
}
