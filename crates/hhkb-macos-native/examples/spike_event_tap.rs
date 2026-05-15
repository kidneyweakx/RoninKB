//! Spike #1 from the M1 plan: open a `CGEventTap` at the session-level,
//! observe key events for 30 seconds, exit. Pure observation — does NOT
//! suppress or re-inject anything.
//!
//! Run with:
//!     cargo run -p hhkb-macos-native --example spike_event_tap
//!
//! Requires Input Monitoring (System Settings → Privacy & Security → Input
//! Monitoring → enable for the binary). On first run macOS will prompt;
//! the spike binary path is `target/debug/examples/spike_event_tap`.

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use std::thread;
    use std::time::{Duration, Instant};

    use core_foundation::runloop::CFRunLoop;
    use hhkb_macos_native::event_tap::{install_and_run, ObservedEvent, Verdict};

    tracing_subscriber::fmt::try_init().ok();

    println!("== CGEventTap spike (30 s) ==");
    println!("Press a few keys, including Caps Lock and a letter. Events are");
    println!("logged but NOT suppressed — your typing reaches its app normally.\n");

    let started = Instant::now();
    let main_loop = CFRunLoop::get_current();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(30));
        println!("\n[spike] 30 s elapsed — stopping run loop");
        main_loop.stop();
    });

    install_and_run(Box::new(move |ev| {
        let elapsed_ms = started.elapsed().as_millis();
        match ev {
            ObservedEvent::Pressed(u) => println!("{elapsed_ms:>5}ms  press   HID 0x{:02X}", u.0),
            ObservedEvent::Released(u) => println!("{elapsed_ms:>5}ms  release HID 0x{:02X}", u.0),
            ObservedEvent::Unknown { cg_keycode, kind } => {
                println!(
                    "{elapsed_ms:>5}ms  unknown CG 0x{:02X} ({kind:?})",
                    cg_keycode
                );
            }
        }
        Verdict::PassThrough
    }))?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("spike_event_tap requires macOS");
}
