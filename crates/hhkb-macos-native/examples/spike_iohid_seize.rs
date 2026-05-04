//! Spike #2 from the M1 plan: open external keyboards via `IOHIDManager` with
//! `kIOHIDOptionsTypeSeizeDevice`, decode boot-protocol reports, and print
//! the press/release events the OS layer would feed into the engine.
//!
//! On Tahoe, `kCGSessionEventTap` no longer receives external HID input —
//! this is the fallback path. Built-in keyboards are intentionally NOT
//! seized so the user can keep typing during the spike.
//!
//! The runloop auto-stops after 30s so this is harmless to run during testing.
//!
//! Run with:
//!     cargo run -p hhkb-macos-native --example spike_iohid_seize
//!
//! Requires Input Monitoring permission. If the seize call returns
//! `kIOReturnNotPermitted`, grant the running terminal Input Monitoring in
//! System Settings → Privacy & Security and try again.

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Duration;

    use core_foundation::runloop::CFRunLoop;
    use hhkb_macos_native::iohid_seize::{HidEvent, SeizeManager};

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("== IOHIDManager seize spike ==");
    println!("Open external keyboards, run for 30s, then exit.\n");

    let manager = match SeizeManager::open_external_keyboards(|ev| match ev {
        HidEvent::Pressed(usage) => {
            println!("pressed  HID 0x{:02X} ({})", usage.0, name(usage));
        }
        HidEvent::Released(usage) => {
            println!("released HID 0x{:02X} ({})", usage.0, name(usage));
        }
    }) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("failed to open external keyboards: {e}");
            eprintln!("hint: grant Input Monitoring permission in System Settings.");
            std::process::exit(1);
        }
    };

    println!(
        "seized {} external keyboard(s).",
        manager.seized_device_count()
    );
    println!("running CFRunLoop for 30s — type on an external keyboard to test.\n");

    // Schedule a 30s auto-stop. Capture the runloop on the main thread so the
    // helper thread can stop it without re-querying.
    let main_loop = CFRunLoop::get_current();
    let stop_handle = main_loop.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        println!("\n30s elapsed — stopping CFRunLoop.");
        stop_handle.stop();
    });

    CFRunLoop::run_current();

    println!("CFRunLoop stopped. Closing manager.");
    manager.close();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("spike_iohid_seize requires macOS");
}

#[cfg(target_os = "macos")]
fn name(u: hhkb_macos_native::HidUsage) -> &'static str {
    use hhkb_macos_native::HidUsage;
    match u {
        HidUsage::A => "A",
        HidUsage::B => "B",
        HidUsage::ESC => "Esc",
        HidUsage::CAPS_LOCK => "Caps",
        HidUsage::LEFT_CTRL => "LCtrl",
        HidUsage::LEFT_SHIFT => "LShift",
        HidUsage::LEFT_ALT => "LAlt",
        HidUsage::LEFT_GUI => "LGui",
        u if u.0 == 0xE4 => "RCtrl",
        u if u.0 == 0xE5 => "RShift",
        u if u.0 == 0xE6 => "RAlt",
        u if u.0 == 0xE7 => "RGui",
        _ => "?",
    }
}
