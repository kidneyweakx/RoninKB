//! Tap-hold + layer state machine, wrapping `kanata-keyberon`.
//!
//! The PoC engine is intentionally narrow: a single Caps→tap=Esc/hold=LCtrl
//! HoldTap binding routed through keyberon, plus a passthrough set for every
//! other key. That's enough to evaluate the M1 kill-switch criteria
//! (tap-hold latency, reliability under fast typing) without forcing a
//! full HID matrix into a const generic at this stage.
//!
//! Two-stage IO:
//!   1. Caller feeds key events via `input()` whenever the OS layer
//!      observes a press/release.
//!   2. Caller calls `tick()` once per millisecond. `tick` returns the
//!      transitions (press/release of HID usages) the OS layer should
//!      re-inject this tick.
//!
//! Transitions, not active-set, because the OS layer thinks in terms of
//! discrete `CGEventCreateKeyboardEvent` press/release calls.

use std::collections::BTreeSet;
use std::convert::Infallible;

use kanata_keyberon::action::{Action, HoldTapAction, HoldTapConfig};
use kanata_keyberon::key_code::KeyCode;
use kanata_keyberon::layout::{Event as KbEvent, Layout};

use crate::keycode::HidUsage;

/// Default tap-hold timeout for Caps→Ctrl/Esc, in milliseconds.
///
/// 200ms is the conservative starting point — long enough that home-row mods
/// won't misfire on average typing, short enough that Esc tap feels
/// responsive. The M1 kill-switch criterion bounds this at 300ms ceiling.
pub const CAPS_HOLD_TAP_TIMEOUT_MS: u16 = 200;

/// HoldTap configuration for the Caps-Lock cell of the layout.
///
/// `Default` config means: hold action resolves only after timeout elapses
/// or another key is also released before timeout. `HoldOnOtherKeyPress`
/// would resolve hold the instant any other key presses — better for chords,
/// worse for tapping. We start with `Default` and revisit in M1 results.
static HOLD_TAP_CAPS: HoldTapAction<'static, Infallible> = HoldTapAction {
    timeout: CAPS_HOLD_TAP_TIMEOUT_MS,
    hold: Action::KeyCode(KeyCode::LCtrl),
    tap: Action::KeyCode(KeyCode::Escape),
    timeout_action: Action::KeyCode(KeyCode::LCtrl),
    config: HoldTapConfig::Default,
    tap_hold_interval: 0,
    on_press_reset_timeout_to: None,
};

/// One layer × one row × one column — the Caps cell only.
static LAYERS: [[[Action<'static, Infallible>; 1]; 1]; 1] = [[[Action::HoldTap(&HOLD_TAP_CAPS)]]];

/// `Layout::new_with_trans_action_settings` requires a `src_keys` row used to
/// resolve `Action::Trans` against. Our PoC has no Trans actions; passing
/// `NoOp` is the documented neutral.
static SRC_KEYS: [Action<'static, Infallible>; 1] = [Action::NoOp];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineEvent {
    Press(HidUsage),
    Release(HidUsage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Press(HidUsage),
    Release(HidUsage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOutput {
    pub transitions: Vec<Transition>,
}

pub struct Engine {
    layout: Layout<'static, 1, 1, Infallible>,
    /// HID usages currently held that the engine doesn't intercept (everything
    /// except Caps Lock for the PoC).
    passthrough: BTreeSet<HidUsage>,
    /// Last tick's active HID usages, for transition computation.
    last_active: BTreeSet<HidUsage>,
}

impl Engine {
    /// Construct a fresh engine wired with the default Caps→Ctrl/Esc binding.
    pub fn new() -> Self {
        let layout = Layout::new_with_trans_action_settings(
            &SRC_KEYS, &LAYERS, true,  // trans_resolution_behavior_v2 — newer kanata default
            false, // delegate_to_first_layer — N/A for one-layer layout
        );
        Self {
            layout,
            passthrough: BTreeSet::new(),
            last_active: BTreeSet::new(),
        }
    }

    /// Feed a key event in. No output is produced here — call `tick()` for
    /// the engine's verdict.
    pub fn input(&mut self, ev: EngineEvent) {
        match ev {
            EngineEvent::Press(HidUsage::CAPS_LOCK) => {
                self.layout.event(KbEvent::Press(0, 0));
            }
            EngineEvent::Release(HidUsage::CAPS_LOCK) => {
                self.layout.event(KbEvent::Release(0, 0));
            }
            EngineEvent::Press(usage) => {
                self.passthrough.insert(usage);
            }
            EngineEvent::Release(usage) => {
                self.passthrough.remove(&usage);
            }
        }
    }

    /// Advance one millisecond of engine time and return transitions to apply.
    pub fn tick(&mut self) -> EngineOutput {
        let _ = self.layout.tick();

        let mut active: BTreeSet<HidUsage> = self.layout.keycodes().map(HidUsage::from).collect();
        active.extend(self.passthrough.iter().copied());

        let mut transitions = Vec::new();
        for usage in active.difference(&self.last_active) {
            transitions.push(Transition::Press(*usage));
        }
        for usage in self.last_active.difference(&active) {
            transitions.push(Transition::Release(*usage));
        }
        self.last_active = active;

        EngineOutput { transitions }
    }

    /// Tick `count` times in a row, returning every transition produced.
    /// Convenience for tests + the spike binary.
    pub fn tick_n(&mut self, count: u32) -> Vec<Transition> {
        let mut out = Vec::new();
        for _ in 0..count {
            out.extend(self.tick().transitions);
        }
        out
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presses(t: &[Transition]) -> Vec<HidUsage> {
        t.iter()
            .filter_map(|t| match t {
                Transition::Press(u) => Some(*u),
                _ => None,
            })
            .collect()
    }

    fn releases(t: &[Transition]) -> Vec<HidUsage> {
        t.iter()
            .filter_map(|t| match t {
                Transition::Release(u) => Some(*u),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn caps_quick_tap_emits_escape() {
        let mut e = Engine::new();

        e.input(EngineEvent::Press(HidUsage::CAPS_LOCK));
        // Tap window: release before timeout (well under 200ms).
        let _ = e.tick_n(20);
        e.input(EngineEvent::Release(HidUsage::CAPS_LOCK));

        let trans = e.tick_n(20);
        let pressed: Vec<_> = presses(&trans);
        let released: Vec<_> = releases(&trans);

        assert!(
            pressed.contains(&HidUsage::ESC),
            "expected Esc press, got {trans:?}"
        );
        assert!(
            released.contains(&HidUsage::ESC),
            "expected Esc release in tap-then-release window, got {trans:?}"
        );
        assert!(
            !pressed.contains(&HidUsage::LEFT_CTRL),
            "tap shouldn't emit LCtrl, got {trans:?}"
        );
    }

    #[test]
    fn caps_held_long_emits_lctrl() {
        let mut e = Engine::new();

        e.input(EngineEvent::Press(HidUsage::CAPS_LOCK));
        // Hold past the 200ms timeout.
        let trans = e.tick_n(250);

        assert!(
            presses(&trans).contains(&HidUsage::LEFT_CTRL),
            "expected LCtrl press after hold timeout, got {trans:?}"
        );
        assert!(
            !presses(&trans).contains(&HidUsage::ESC),
            "hold shouldn't tap Esc, got {trans:?}"
        );

        // Now release — LCtrl should release.
        e.input(EngineEvent::Release(HidUsage::CAPS_LOCK));
        let trans = e.tick_n(20);
        assert!(
            releases(&trans).contains(&HidUsage::LEFT_CTRL),
            "expected LCtrl release after Caps release, got {trans:?}"
        );
    }

    #[test]
    fn caps_then_other_key_does_not_tap_escape() {
        // Holding Caps and pressing another key should resolve the HoldTap as
        // hold (LCtrl), not tap (Esc), because there's no way the user
        // intended a tap if a chord followed.
        //
        // With HoldTapConfig::Default, this resolves on Caps release if Caps
        // is released after the other key. We advance the clock past timeout
        // first so the resolution is unambiguous.
        let mut e = Engine::new();

        e.input(EngineEvent::Press(HidUsage::CAPS_LOCK));
        let _ = e.tick_n(250); // past timeout — engine commits to hold
        e.input(EngineEvent::Press(HidUsage::A));
        let trans_during = e.tick_n(20);

        let pressed = presses(&trans_during);
        assert!(
            pressed.contains(&HidUsage::A),
            "expected 'a' press during chord, got {trans_during:?}"
        );

        e.input(EngineEvent::Release(HidUsage::A));
        e.input(EngineEvent::Release(HidUsage::CAPS_LOCK));
        let trans_after = e.tick_n(20);

        // Across the entire interaction, Esc must never fire.
        let any_esc_press = presses(&trans_during)
            .iter()
            .chain(presses(&trans_after).iter())
            .any(|u| *u == HidUsage::ESC);
        assert!(!any_esc_press, "Esc should never fire on a chord");
    }

    #[test]
    fn passthrough_keys_propagate_unchanged() {
        let mut e = Engine::new();

        e.input(EngineEvent::Press(HidUsage::A));
        let trans = e.tick();
        assert_eq!(
            trans.transitions,
            vec![Transition::Press(HidUsage::A)],
            "passthrough press should appear on next tick"
        );

        e.input(EngineEvent::Release(HidUsage::A));
        let trans = e.tick();
        assert_eq!(
            trans.transitions,
            vec![Transition::Release(HidUsage::A)],
            "passthrough release should appear on next tick"
        );
    }

    #[test]
    fn idle_ticks_emit_nothing() {
        let mut e = Engine::new();
        for _ in 0..50 {
            assert_eq!(e.tick().transitions, vec![], "idle engine should be silent");
        }
    }
}
