# M1 Kill-Switch Validation — Hardware-in-Loop Test Plan

| Status   | Deferred (hardware required)                                  |
| -------- | ------------------------------------------------------------- |
| Owner    | Whoever has a Mac + an external HHKB at the time              |
| Related  | [`docs/v0.2.0-plan.md` §M1](v0.2.0-plan.md), [`docs/rfc-0001-macos-native-backend.md` §9](rfc-0001-macos-native-backend.md), [`docs/rfc-0001-macos-native-backend.md` §12 R1](rfc-0001-macos-native-backend.md) |

The plan's M1 milestone has a kill-switch criterion (plan §3.M1):

> If tap-hold latency on the spike exceeds 300ms or fast typing makes
> home-row mods misfire >20%, abort the native backend for v0.2.0 and
> re-scope.

This file describes how to run that validation. It can't be automated
in CI: we need a real keyboard, a real Mac with TCC permissions
granted, and a real human typing fast enough to stress home-row mods.
Until someone with that setup runs it and records results, the M1
kill-switch decision stays "tentatively passing" — the engine's unit
tests cover the state-machine correctness, but not the
sub-300ms-under-load part.

## What we're measuring

1. **Tap-hold latency.** Time between a Caps Lock tap and the
   re-injected Esc keypress visible to the OS. We need a tight
   sub-300ms upper bound, and we want to know the median for the
   shipped default (200ms timeout) so we can compare to the kanata +
   Karabiner path.
2. **Misfire rate under fast typing.** Percentage of home-row-mod taps
   that get incorrectly resolved as holds when typing at 100+ WPM.
   Threshold: <20% misfires.

## Setup

1. **Hardware.** A Mac (Sequoia or Tahoe) with an external HHKB Pro
   Hybrid plugged in via USB. Built-in keyboard works for the
   session-tap path; external is needed to also exercise the IOHID
   seize path on Tahoe.
2. **Build.** `cargo build -p hhkb-daemon --release` from a clean
   checkout of the v0.2.0 branch.
3. **Permissions.** Grant the daemon Input Monitoring **and**
   Accessibility (System Settings → Privacy & Security). Confirm with
   `GET /backend/list`: `macos-native` should report
   `permission_status.kind = "granted"`.
4. **Pin the backend.**
   `curl -X POST localhost:7331/backend/select -d '{"id":"macos-native"}'`.
5. **Apply a known profile.** Default Caps→Ctrl/Esc HoldTap with
   `timeout_ms: 200`. Use the wizard or the test profile committed at
   `docs/fixtures/m1-default.json` (TODO when this is added).

## Test 1 — Tap-hold latency

The cheapest measurement that doesn't require a logic analyzer is the
event-tap timestamp delta. We record:

- `t0` = the original Caps Lock press, captured by the daemon's tap
  callback (already logged at `tracing::debug` in `event_tap.rs`).
- `t1` = the re-injected Esc press, posted by the engine tick thread.

Both timestamps are `mach_absolute_time` based, monotonic, and read
inside the daemon process — so the delta is the engine's contribution
to latency, not OS HID-stack noise.

Procedure:

1. Run the daemon with `RUST_LOG=hhkb_daemon=trace,hhkb_macos_native=trace`.
2. Tap Caps Lock 100 times with a steady ~1 Hz rhythm.
3. Pipe the log into `scripts/measure-tap-latency.sh` (TODO: write
   this) which extracts `(t0, t1)` pairs and reports min / median /
   p95 / p99 / max.

**Pass criterion:** p99 < 250ms; max < 300ms (matches plan §3.M1
ceiling).

**Fail action:** record results, file as a v0.2.0 blocker, decide
between (a) raising the default `timeout_ms` floor, (b) shipping with
tap-hold disabled and the backend documented as "remap only", or (c)
descoping the native backend per plan §2 risk.

## Test 2 — Misfire rate under fast typing

Procedure:

1. Configure a home-row-mod profile (Caps→Ctrl/Esc + `a`→hold-LSft,
   `s`→hold-LCtl, etc.) — we don't ship one by default, but
   `bindings:` JSON now supports it. Sample profile to be added at
   `docs/fixtures/m1-homerow.json`.
2. Open a typing test page (monkeytype.com works well). Pick a
   word-list that produces ~120 WPM for the tester.
3. Type for 5 minutes. Count misfires manually — a misfire is any
   character that came out as a modifier flag instead of the literal.
   `monkeytype` highlights wrong characters; misfires show up as
   missing characters with a stray modifier.

**Pass criterion:** misfire rate ≤ 20% of opportunity (plan §3.M1).

**Fail action:** same as Test 1.

## What unit tests already cover

The engine's state-machine correctness is fully covered by
`crates/hhkb-macos-native/src/engine.rs` tests:

- `caps_quick_tap_emits_escape` — tap window resolution.
- `caps_held_long_emits_lctrl` — hold timeout resolution.
- `caps_then_other_key_does_not_tap_escape` — chord resolution doesn't
  spuriously emit the tap action.
- `carry_over_emits_release_for_stale_modifier` — hot-swap drains
  modifiers cleanly.

What these *don't* cover, and why this hardware-in-loop validation is
still required:

- **Wall-clock latency on a real CFRunLoop.** Unit tests advance time
  by calling `tick()` 1ms at a time; the production path waits for
  `thread::sleep(1ms)` and locks an `Arc<Mutex<Engine>>` per event.
  Lock contention + scheduler jitter only show up under real load.
- **CGEventTap re-entry behaviour.** The tap callback runs on a
  system-managed thread that the kernel can disable if we're too slow.
  Only a real macOS catches this.
- **Accessibility-pane-blessed code signing.** TCC's silent disable
  race (linked from the RFC §13 references) has bitten Karabiner-Elements
  before; we'll only know if the signed daemon survives a logout/login
  cycle on a real machine.

## Recording results

Once someone runs this, paste outputs into a section here so the
kill-switch decision is auditable:

```
Hardware:    <model + chip>
macOS:       <version>
Daemon SHA:  <git hash>
Profile:     <fixture used>

Test 1 (latency)
  samples: <n>
  min: <x>ms median: <x>ms p95: <x>ms p99: <x>ms max: <x>ms

Test 2 (misfires under fast typing)
  WPM:           <wpm>
  duration:      <minutes>
  taps:          <count>
  misfires:      <count>  (<pct>%)
```
