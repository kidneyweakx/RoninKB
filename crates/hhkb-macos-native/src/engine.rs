//! Tap-hold + layer state machine, wrapping `kanata-keyberon`.
//!
//! v0.2.0 widening: the engine is no longer hardcoded to a single Caps cell.
//! It runs a `Layout<COLS, ROWS, _>` keyed by HID usage — every HID usage
//! 0x01–0xFF maps to a `(row, col)` cell via [`hid_to_pos`], so we can
//! parameterise per-key bindings (remap, tap-hold) at any position the OS
//! event source observes.
//!
//! ## Why 16×16 instead of the HHKB 16×6 matrix?
//!
//! The macOS event source talks **HID usage**, not USB matrix coordinates.
//! By laying the keyberon `Layout` over the HID address space (`usage / 16`,
//! `usage % 16`) we don't need a per-keyboard matrix table to route events,
//! and the same engine can be driven by any keyboard the OS surfaces. The
//! 16×16 grid covers the full keyboard/keypad HID page (0x07) including
//! modifier usages 0xE0–0xE7.
//!
//! ## Engine ownership model
//!
//! Each cell either:
//! - is `Passthrough` — the OS layer should leave the key alone, the engine
//!   pretends it doesn't exist;
//! - is `Remap(kc)` — engine owns the key, suppresses the original, emits
//!   `kc` instead;
//! - is `HoldTap { tap, hold, timeout_ms }` — engine owns the key, runs
//!   tap-hold resolution, emits `tap` or `hold` accordingly.
//!
//! The OS event tap consults [`Engine::is_owned`] per observed event; owned
//! keys are routed into `input()` and the original event is suppressed.
//! Unowned keys flow through unchanged.
//!
//! ## Hot-swap
//!
//! [`Engine::carry_over_to`] preserves the engine's currently-active HID set
//! across a layout swap. After replacing the engine in place, the next
//! `tick()` emits release transitions for any keys the old layout was
//! still emitting that the new layout doesn't — modifier draining without
//! requiring the OS layer to track per-key state itself.
//!
//! Two-stage IO unchanged from M1:
//!   1. Caller feeds key events via `input()` whenever the OS layer
//!      observes a press/release on an owned key.
//!   2. Caller calls `tick()` once per millisecond. `tick` returns the
//!      transitions (press/release of HID usages) the OS layer should
//!      re-inject this tick.

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

/// Layout grid dimensions. 16×16 covers the entire HID Keyboard/Keypad page
/// (0x00–0xFF) with the simple `(usage / 16, usage % 16)` mapping. Bumping
/// either constant requires re-checking [`hid_to_pos`].
pub const COLS: usize = 16;
pub const ROWS: usize = 16;

/// Layer storage shape used internally by the engine. Public so the daemon's
/// per-key profile builder can construct one before handing it to
/// [`Engine::from_layer`].
pub type LayerArr = [[Action<'static, Infallible>; COLS]; ROWS];

/// One PoC-ish binding cell from the user's profile. The daemon's profile
/// parser converts JSON into `[[KeyAction; COLS]; ROWS]` and feeds that to
/// the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Leave the original key alone — OS handles it natively.
    Passthrough,
    /// Replace the source key with `to` whenever pressed.
    Remap(KeyCode),
    /// Tap → `tap`; hold past `timeout_ms` → `hold`.
    HoldTap {
        timeout_ms: u16,
        tap: KeyCode,
        hold: KeyCode,
    },
}

/// Spec for the Caps cell of the layout, used by [`Engine::from_spec`].
/// Allows the daemon to translate a `ViaProfile` into a parameterised engine
/// without the engine knowing about VIA / JSON.
#[derive(Debug, Clone)]
pub enum CapsBindingSpec {
    /// HoldTap: tap emits `tap`, hold (after `timeout_ms`) emits `hold`.
    HoldTap {
        timeout_ms: u16,
        hold: KeyCode,
        tap: KeyCode,
    },
    /// No binding — Caps Lock passes through unchanged.
    Passthrough,
}

impl CapsBindingSpec {
    /// Default M1 PoC binding: tap=Esc, hold=LCtrl, 200ms.
    pub const fn caps_ctrl_esc() -> Self {
        Self::HoldTap {
            timeout_ms: CAPS_HOLD_TAP_TIMEOUT_MS,
            hold: KeyCode::LCtrl,
            tap: KeyCode::Escape,
        }
    }
}

/// Map a HID usage to a `(row, col)` cell. `None` for `0x00` (reserved) and
/// any usage outside the byte range — the keyboard/keypad page tops out at
/// `0xE7`, but the table is sized 16×16 so 0x00–0xFF are all addressable.
pub fn hid_to_pos(usage: HidUsage) -> Option<(usize, usize)> {
    let v = usage.0;
    if v == 0 || v > 0xFF {
        return None;
    }
    Some(((v as usize) / COLS, (v as usize) % COLS))
}

/// Inverse of [`hid_to_pos`]. `None` for `(0, 0)` since usage 0 is reserved.
pub fn pos_to_hid(row: usize, col: usize) -> Option<HidUsage> {
    let v = row * COLS + col;
    if v == 0 || v > 0xFF {
        return None;
    }
    Some(HidUsage(v as u16))
}

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
    layout: Layout<'static, COLS, ROWS, Infallible>,
    /// Last tick's active HID usages, for transition computation.
    last_active: BTreeSet<HidUsage>,
    /// HID usages whose cells aren't `Passthrough`. The OS layer reads this
    /// via [`Engine::is_owned`] to decide between Suppress + route-to-engine
    /// and PassThrough.
    owned: BTreeSet<HidUsage>,
}

impl Engine {
    /// Construct a fresh engine wired with the default Caps→Ctrl/Esc binding.
    /// Equivalent to `from_spec(&CapsBindingSpec::caps_ctrl_esc())`.
    pub fn new() -> Self {
        Self::from_spec(&CapsBindingSpec::caps_ctrl_esc())
    }

    /// Construct an engine where every cell is passthrough except the Caps
    /// cell, which carries `spec` (HoldTap or Passthrough).
    ///
    /// Note: keyberon requires `'static` references for layouts. We
    /// `Box::leak` the layer storage. The leak is bounded — one allocation
    /// per engine swap — and profile changes are rare events in practice.
    pub fn from_spec(spec: &CapsBindingSpec) -> Self {
        let mut grid = passthrough_grid();
        match spec {
            CapsBindingSpec::Passthrough => {}
            CapsBindingSpec::HoldTap {
                timeout_ms,
                hold,
                tap,
            } => {
                set_cell(
                    &mut grid,
                    HidUsage::CAPS_LOCK,
                    KeyAction::HoldTap {
                        timeout_ms: *timeout_ms,
                        tap: *tap,
                        hold: *hold,
                    },
                );
            }
        }
        Self::from_grid(grid)
    }

    /// Construct an engine from a full per-position binding grid. The grid
    /// is `[[KeyAction; COLS]; ROWS]`, indexed via [`hid_to_pos`]. Cells the
    /// caller doesn't care about should be `KeyAction::Passthrough`.
    pub fn from_grid(grid: [[KeyAction; COLS]; ROWS]) -> Self {
        let mut owned = BTreeSet::new();
        let mut layer: LayerArr = std::array::from_fn(|_| std::array::from_fn(|_| Action::NoOp));

        for (r, row) in grid.iter().enumerate() {
            for (c, action) in row.iter().enumerate() {
                let Some(src_hid) = pos_to_hid(r, c) else {
                    continue;
                };
                match *action {
                    KeyAction::Passthrough => {
                        // Mirror the source key so the layout produces the
                        // identity binding when no remap is configured. The
                        // OS layer will skip these (not owned) and let the
                        // OS handle them natively, but having an explicit
                        // identity cell means a `Remap` swap doesn't fall
                        // off the end of the layout.
                        if let Some(kc) = src_hid.to_keyberon() {
                            layer[r][c] = Action::KeyCode(kc);
                        }
                    }
                    KeyAction::Remap(target) => {
                        layer[r][c] = Action::KeyCode(target);
                        // Identity remap is functionally passthrough, so
                        // don't claim ownership — saves a synthetic
                        // re-injection that the OS would have done anyway.
                        if HidUsage::from(target) != src_hid {
                            owned.insert(src_hid);
                        }
                    }
                    KeyAction::HoldTap {
                        timeout_ms,
                        tap,
                        hold,
                    } => {
                        let holdtap: &'static HoldTapAction<'static, Infallible> =
                            Box::leak(Box::new(HoldTapAction {
                                timeout: timeout_ms,
                                hold: Action::KeyCode(hold),
                                tap: Action::KeyCode(tap),
                                timeout_action: Action::KeyCode(hold),
                                config: HoldTapConfig::Default,
                                tap_hold_interval: 0,
                                on_press_reset_timeout_to: None,
                            }));
                        layer[r][c] = Action::HoldTap(holdtap);
                        owned.insert(src_hid);
                    }
                }
            }
        }

        let layers: &'static [LayerArr; 1] = Box::leak(Box::new([layer]));
        let src_keys: &'static [Action<'static, Infallible>; COLS] =
            Box::leak(Box::new(std::array::from_fn(|_| Action::NoOp)));

        let layout = Layout::new_with_trans_action_settings(
            src_keys, layers, /* trans_v2 */ true, false,
        );

        Self {
            layout,
            last_active: BTreeSet::new(),
            owned,
        }
    }

    /// `true` if this engine owns `usage` — meaning the OS event tap should
    /// suppress the original event and route press/release to `input()`.
    /// Unowned keys flow through the OS unchanged.
    pub fn is_owned(&self, usage: HidUsage) -> bool {
        self.owned.contains(&usage)
    }

    /// Carry over the running engine's active HID set into `next` so that
    /// `next`'s first `tick()` after a hot-swap emits releases for any keys
    /// the previous layout was still driving but the new layout no longer
    /// owns. Modifier draining without requiring the OS layer to track
    /// engine-side state.
    pub fn carry_over_to(&self, next: &mut Engine) {
        next.last_active = self.last_active.clone();
    }

    /// Feed a key event in. No output is produced here — call `tick()` for
    /// the engine's verdict. Events for cells that don't map to an owned
    /// position are silently dropped (the OS layer shouldn't be calling us
    /// for them anyway).
    pub fn input(&mut self, ev: EngineEvent) {
        let (usage, kind) = match ev {
            EngineEvent::Press(u) => (u, true),
            EngineEvent::Release(u) => (u, false),
        };
        let Some((r, c)) = hid_to_pos(usage) else {
            return;
        };
        let r = r as u8;
        let c = c as u16;
        if kind {
            self.layout.event(KbEvent::Press(r, c));
        } else {
            self.layout.event(KbEvent::Release(r, c));
        }
    }

    /// Advance one millisecond of engine time and return transitions to apply.
    pub fn tick(&mut self) -> EngineOutput {
        let _ = self.layout.tick();

        let active: BTreeSet<HidUsage> = self
            .layout
            .keycodes()
            .map(HidUsage::from)
            .filter(|u| u.0 != 0)
            .collect();

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

/// All-passthrough grid — the starting point that every engine builder
/// starts from before applying user bindings.
pub fn passthrough_grid() -> [[KeyAction; COLS]; ROWS] {
    [[KeyAction::Passthrough; COLS]; ROWS]
}

/// Set the cell at `usage`'s position to `action`. Silently no-op for HID
/// usages outside the addressable range.
pub fn set_cell(grid: &mut [[KeyAction; COLS]; ROWS], usage: HidUsage, action: KeyAction) {
    if let Some((r, c)) = hid_to_pos(usage) {
        grid[r][c] = action;
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
    fn passthrough_keys_dont_route_through_engine_owned_set() {
        // Default engine owns Caps only — every other key should be
        // unowned, telling the OS layer to PassThrough.
        let e = Engine::new();
        assert!(e.is_owned(HidUsage::CAPS_LOCK));
        assert!(!e.is_owned(HidUsage::A));
        assert!(!e.is_owned(HidUsage::ESC));
    }

    #[test]
    fn idle_ticks_emit_nothing() {
        let mut e = Engine::new();
        for _ in 0..50 {
            assert_eq!(e.tick().transitions, vec![], "idle engine should be silent");
        }
    }

    #[test]
    fn remap_translates_one_key_to_another() {
        // Per-position layout: 'A' is remapped to 'B'. The engine should
        // claim ownership of 'A' and emit B-press/B-release in its place.
        let mut grid = passthrough_grid();
        set_cell(&mut grid, HidUsage::A, KeyAction::Remap(KeyCode::B));
        let mut e = Engine::from_grid(grid);

        assert!(e.is_owned(HidUsage::A));
        assert!(!e.is_owned(HidUsage::B));

        e.input(EngineEvent::Press(HidUsage::A));
        let press_trans = e.tick_n(5);
        assert!(
            presses(&press_trans).contains(&HidUsage::B),
            "expected B press from A→B remap, got {press_trans:?}"
        );

        e.input(EngineEvent::Release(HidUsage::A));
        let release_trans = e.tick_n(5);
        assert!(
            releases(&release_trans).contains(&HidUsage::B),
            "expected B release from A→B remap, got {release_trans:?}"
        );
    }

    #[test]
    fn identity_remap_is_not_claimed_as_owned() {
        // A→A should be treated as passthrough so the OS handles it
        // natively — no point re-injecting an identical key.
        let mut grid = passthrough_grid();
        set_cell(&mut grid, HidUsage::A, KeyAction::Remap(KeyCode::A));
        let e = Engine::from_grid(grid);
        assert!(!e.is_owned(HidUsage::A));
    }

    #[test]
    fn hid_to_pos_round_trips() {
        for usage in [
            HidUsage::A,
            HidUsage::CAPS_LOCK,
            HidUsage::ESC,
            HidUsage::LEFT_CTRL,
            HidUsage::LEFT_SHIFT,
        ] {
            let (r, c) = hid_to_pos(usage).expect("addressable");
            assert_eq!(pos_to_hid(r, c), Some(usage));
        }
    }

    #[test]
    fn carry_over_emits_release_for_stale_modifier() {
        // Old engine: caps → hold=LCtrl. Hold past timeout so LCtrl is in
        // last_active. Hot-swap to a passthrough engine — the new engine,
        // seeded with the old last_active, must emit a LCtrl release on
        // its first tick so we don't leave a stuck modifier.
        let mut old = Engine::new();
        old.input(EngineEvent::Press(HidUsage::CAPS_LOCK));
        let _ = old.tick_n(250);
        // Sanity: LCtrl is now part of last_active.
        assert!(old.last_active.contains(&HidUsage::LEFT_CTRL));

        let mut new = Engine::from_spec(&CapsBindingSpec::Passthrough);
        old.carry_over_to(&mut new);

        let trans = new.tick();
        assert!(
            releases(&trans.transitions).contains(&HidUsage::LEFT_CTRL),
            "expected stale LCtrl release after hot-swap, got {trans:?}"
        );
    }
}
