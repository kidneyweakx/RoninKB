//! M1 PoC integration: end-to-end Caps Lock → tap=Esc / hold=LCtrl on a
//! live Mac. Reads through Path A (CGEventTap), routes Caps through the
//! keyberon engine, re-injects via CGEventPost.
//!
//! Runs for 30 seconds then exits. This is the binary the M1 kill-switch
//! evaluation runs against:
//!   - tap-hold latency (target ≤ 300 ms)
//!   - home-row-mod misfire rate under fast typing (target < 20 %)
//!
//! Run with:
//!     cargo run -p hhkb-macos-native --example poc_full
//!
//! Requires Input Monitoring + Accessibility for the binary. macOS will
//! prompt on first run; grant both, then re-run.

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use core_foundation::runloop::CFRunLoop;
    use hhkb_macos_native::engine::Transition;
    use hhkb_macos_native::event_tap::{install_and_run, ObservedEvent, Verdict};
    use hhkb_macos_native::{inject, Engine, EngineEvent, HidUsage};

    tracing_subscriber::fmt::try_init().ok();

    println!("== Caps → Esc/LCtrl PoC (30 s) ==");
    println!("- Tap Caps quickly: emits Esc.");
    println!("- Hold Caps > 200 ms: holds LCtrl.");
    println!("- Hold Caps + press a letter: should chord as Ctrl-letter.\n");

    let engine = Arc::new(Mutex::new(Engine::new()));
    let stop = Arc::new(AtomicBool::new(false));

    // 30 s safety timeout — stop the run loop, signal the tick thread.
    let main_loop = CFRunLoop::get_current();
    let stop_for_safety = Arc::clone(&stop);
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(30));
        println!("\n[poc] 30 s elapsed — stopping");
        stop_for_safety.store(true, Ordering::Relaxed);
        main_loop.stop();
    });

    // Tick thread: 1 ms cadence, drains engine transitions, re-injects.
    let engine_tick = Arc::clone(&engine);
    let stop_for_tick = Arc::clone(&stop);
    thread::spawn(move || {
        while !stop_for_tick.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(1));
            let transitions = {
                let mut e = engine_tick.lock().expect("engine poisoned");
                e.tick().transitions
            };
            for t in transitions {
                let res = match t {
                    Transition::Press(u) => inject::post_press(u),
                    Transition::Release(u) => inject::post_release(u),
                };
                if let Err(err) = res {
                    eprintln!("[poc] inject error: {err}");
                }
            }
        }
    });

    let engine_cb = Arc::clone(&engine);
    install_and_run(Box::new(move |ev| match ev {
        ObservedEvent::Pressed(HidUsage::CAPS_LOCK) => {
            engine_cb
                .lock()
                .expect("engine poisoned")
                .input(EngineEvent::Press(HidUsage::CAPS_LOCK));
            Verdict::Suppress
        }
        ObservedEvent::Released(HidUsage::CAPS_LOCK) => {
            engine_cb
                .lock()
                .expect("engine poisoned")
                .input(EngineEvent::Release(HidUsage::CAPS_LOCK));
            Verdict::Suppress
        }
        _ => Verdict::PassThrough,
    }))?;

    stop.store(true, Ordering::Relaxed);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("poc_full requires macOS");
}
