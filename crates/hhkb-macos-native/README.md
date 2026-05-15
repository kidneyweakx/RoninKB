# hhkb-macos-native — M1 PoC for the v0.2.0 native backend

Experimental crate that scaffolds the macOS native keyboard remapper backend
described in [`docs/rfc-0001-macos-native-backend.md`](../../docs/rfc-0001-macos-native-backend.md).
This crate is the spike implementation for milestone **M1** of the
[v0.2.0 plan](../../docs/v0.2.0-plan.md). It is intentionally narrow — enough
surface to evaluate the kill-switch criteria (tap-hold latency and reliability
under fast typing) before committing to the full M2 build.

## Engine decision: kanata-keyberon

We chose [`kanata-keyberon` 0.1110](https://crates.io/crates/kanata-keyberon)
as the layer + tap-hold state machine for the PoC. It's the same fork kanata
itself uses internally, MIT-licensed (no LGPL contamination), and exposes the
exact `HoldTapAction` shape we need. The upstream `keyberon` crate (`0.1.1`)
is the original from TeXitoi but ~2 years stale on crates.io; the kanata
fork tracks current development and lines up cleanly with how kanata
configures tap-hold semantics, which means our PoC's correctness is
"whatever kanata does," modulo the OS layer.

The alternative, [`smart-keymap`](https://github.com/rgoulter/smart-keymap),
exposes a C-callable static lib and supports more advanced chord shapes than
keyberon. Deferred — we'll revisit if M1 surfaces a need keyberon can't meet.
Decision logged here per the v0.2.0 plan §3 M1 exit criteria.

## What's in here

```
src/
  engine.rs         — Caps→Esc/LCtrl tap-hold, wrapping kanata-keyberon
  keycode.rs        — HID usage / CG keycode / keyberon::KeyCode tri-mapping
  profile.rs        — minimal binding shape for the PoC
  error.rs          — error types
  event_tap.rs      — Path A: CGEventTap (macOS only)
  iohid_seize.rs    — Path B: IOHIDManager seize (macOS only)
  inject.rs         — CGEventPost re-injection (macOS only)
examples/
  spike_engine.rs        — synthetic events → engine output (CI-testable)
  spike_event_tap.rs     — 30 s session-tap log (macOS, needs Input Monitoring)
  spike_iohid_seize.rs   — 30 s IOHID seize log (macOS, external keyboards only)
  poc_full.rs            — end-to-end Caps→Esc/LCtrl integration (macOS)
```

## Running the spikes

The CI-testable engine spike runs everywhere:

```bash
cargo run -p hhkb-macos-native --example spike_engine
```

Expected output: three scenarios (quick tap, long hold, chord) printing the
HID transitions the engine emits per millisecond tick. None of this needs
macOS permissions because it's pure synthetic input.

The macOS spikes need real permissions. Grant them once via System Settings →
Privacy & Security → Input Monitoring, adding the binary path that the spike
prints on first launch (it'll be `target/debug/examples/<name>`).

```bash
# Path A — pre-Tahoe + built-in keyboards on Tahoe
cargo run -p hhkb-macos-native --example spike_event_tap

# Path B — external keyboards on Tahoe (or anytime, observation-only)
cargo run -p hhkb-macos-native --example spike_iohid_seize

# End-to-end: Caps→Esc/LCtrl, this is the M1 kill-switch demo
cargo run -p hhkb-macos-native --example poc_full
```

Each binary auto-stops after 30 s so you can't accidentally lock yourself out.

## M1 kill-switch — what counts as pass

Per [`docs/v0.2.0-plan.md`](../../docs/v0.2.0-plan.md) §3 M1, the PoC must
demonstrate Caps→Ctrl/Esc tap-hold against a live Mac with:

- Tap-hold resolution latency **≤ 300 ms** in the 99th percentile.
- Home-row mod misfire rate **< 20 %** under fast typing (≥ 80 wpm).

If either is exceeded, v0.2.0 down-scopes per the plan §2 risk paragraph:
ship with EEPROM + Hidutil as macOS defaults, defer native backend to v0.3.

## Limitations of the PoC (intentional, will revisit in M2)

- **One binding only**: Caps Lock → tap=Esc / hold=LCtrl. M2 generalises.
- **Sparse keycode table**: only ~25 keys have CG ↔ HID ↔ keyberon mappings.
  Unknown keys pass through OS untouched. M2 fills the table.
- **HoldTapConfig::Default**: time-based resolution. `HoldOnOtherKeyPress`
  may improve chord ergonomics; deferred to M2 once we have measurements.
- **No app-aware bindings**: M2 adds `NSWorkspaceDidActivateApplicationNotification`.
- **No profile loading**: the PoC layout is hard-coded. M2 wires
  `crates/hhkb-daemon/src/backend/macos_native/profile.rs`.
- **Boot-protocol HID reports only on Path B**: vendor-specific reports are
  logged and skipped. Fine for the PoC (USB/Bluetooth keyboards almost
  always emit boot-protocol on the standard input endpoint).

## License

MIT. See the workspace [`LICENSE`](../../LICENSE).
