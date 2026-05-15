//! Spike #3 from the M1 plan: feed synthetic events into the keyberon-backed
//! Engine and print the transitions it produces. Runs on every platform —
//! this is the CI-testable proof that the engine semantics are right.
//!
//! Run with:
//!     cargo run -p hhkb-macos-native --example spike_engine

use hhkb_macos_native::{Engine, EngineEvent, HidUsage};

fn main() {
    println!("== Caps→Ctrl/Esc engine spike ==\n");

    quick_tap();
    println!();
    long_hold();
    println!();
    chord_with_caps();
}

fn quick_tap() {
    println!("scenario: Caps tap (50ms hold, well under 200ms timeout)");
    println!("expect:   Esc press → Esc release");
    let mut e = Engine::new();
    e.input(EngineEvent::Press(HidUsage::CAPS_LOCK));
    print_transitions("  t=  0..50  :", &e_tick_n(&mut e, 50));
    e.input(EngineEvent::Release(HidUsage::CAPS_LOCK));
    print_transitions("  t= 50..100 :", &e_tick_n(&mut e, 50));
}

fn long_hold() {
    println!("scenario: Caps hold (300ms past timeout, then release)");
    println!("expect:   LCtrl press once timeout fires → LCtrl release on Caps release");
    let mut e = Engine::new();
    e.input(EngineEvent::Press(HidUsage::CAPS_LOCK));
    print_transitions("  t=  0..300:", &e_tick_n(&mut e, 300));
    e.input(EngineEvent::Release(HidUsage::CAPS_LOCK));
    print_transitions("  t=300..350:", &e_tick_n(&mut e, 50));
}

fn chord_with_caps() {
    println!("scenario: Caps held + 'a' pressed (Ctrl-A chord)");
    println!("expect:   LCtrl press, A press, A release, LCtrl release. NO Esc.");
    let mut e = Engine::new();
    e.input(EngineEvent::Press(HidUsage::CAPS_LOCK));
    print_transitions("  t=  0..250:", &e_tick_n(&mut e, 250));
    e.input(EngineEvent::Press(HidUsage::A));
    print_transitions("  t=250..260:", &e_tick_n(&mut e, 10));
    e.input(EngineEvent::Release(HidUsage::A));
    e.input(EngineEvent::Release(HidUsage::CAPS_LOCK));
    print_transitions("  t=260..280:", &e_tick_n(&mut e, 20));
}

fn e_tick_n(engine: &mut Engine, count: u32) -> Vec<hhkb_macos_native::engine::Transition> {
    engine.tick_n(count)
}

fn print_transitions(label: &str, ts: &[hhkb_macos_native::engine::Transition]) {
    if ts.is_empty() {
        println!("{label} (no transitions)");
        return;
    }
    let pretty: Vec<String> = ts
        .iter()
        .map(|t| match t {
            hhkb_macos_native::engine::Transition::Press(u) => format!("press {}", name(*u)),
            hhkb_macos_native::engine::Transition::Release(u) => format!("release {}", name(*u)),
        })
        .collect();
    println!("{label} {}", pretty.join(", "));
}

fn name(u: HidUsage) -> String {
    match u {
        HidUsage::CAPS_LOCK => "Caps".into(),
        HidUsage::ESC => "Esc".into(),
        HidUsage::LEFT_CTRL => "LCtrl".into(),
        HidUsage::A => "A".into(),
        other => format!("HID(0x{:02X})", other.0),
    }
}
