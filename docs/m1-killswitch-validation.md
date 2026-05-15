# M1 Kill-Switch Validation — Hardware-in-Loop Test Plan

| Status   | Runnable — awaiting a hardware-in-loop run                     |
| -------- | ------------------------------------------------------------- |
| Owner    | Whoever has a Mac + an external HHKB at the time              |
| Related  | [`docs/v0.2.0-plan.md` §M1](v0.2.0-plan.md), [`docs/rfc-0001-macos-native-backend.md` §9](rfc-0001-macos-native-backend.md), [`docs/rfc-0001-macos-native-backend.md` §12 R1](rfc-0001-macos-native-backend.md) |
| Tooling  | [`scripts/measure-tap-latency.sh`](../scripts/measure-tap-latency.sh), [`docs/fixtures/m1-default.json`](fixtures/m1-default.json), [`docs/fixtures/m1-homerow.json`](fixtures/m1-homerow.json) |

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

The tooling is now in-tree: [`scripts/measure-tap-latency.sh`](../scripts/measure-tap-latency.sh)
parses daemon trace logs into latency stats and asserts the kill-switch
thresholds, [`docs/fixtures/m1-default.json`](fixtures/m1-default.json)
is the Caps→Esc/LCtrl HoldTap profile for Test 1, and
[`docs/fixtures/m1-homerow.json`](fixtures/m1-homerow.json) is the
home-row-mods profile for Test 2. The daemon's `MacosNativeBackend`
emits structured `phase=rx` / `phase=tx` trace events on the
`hhkb_daemon::backend::macos_native::latency` target so the script can
compute `tx.t_us - rx.t_us` per pair without depending on tap-callback
log scraping.

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
   `timeout_ms: 200`. Either drive the wizard or POST the fixture
   directly:

   ```bash
   curl -X POST http://127.0.0.1:7331/profiles \
     -H 'Content-Type: application/json' \
     -d @docs/fixtures/m1-default.json
   # …then activate it via the response's profile id…
   curl -X POST http://127.0.0.1:7331/profiles/active \
     -H 'Content-Type: application/json' \
     -d '{"id":"<id from the create response>"}'
   ```

## Test 1 — Tap-hold latency

The cheapest measurement that doesn't require a logic analyzer is the
event-tap timestamp delta. The daemon emits two structured trace events
per owned-key transition:

- `phase = "rx"` — the original Caps Lock press/release observed by the
  CGEventTap callback (`hhkb-daemon/src/backend/macos_native.rs`, tap
  thread).
- `phase = "tx"` — the re-injected Esc press posted by the engine tick
  thread.

Both timestamps come from a shared `Instant` origin captured at
`apply()` time. `Instant` is backed by `CLOCK_MONOTONIC_RAW` on macOS,
so `tx.t_us - rx.t_us` is real wall-clock microseconds within the
daemon process — the delta is the engine's contribution to latency,
not OS HID-stack noise.

Procedure:

1. Run the daemon with `RUST_LOG=hhkb_daemon=trace`. Stderr should
   start emitting `hhkb_daemon::backend::macos_native::latency` lines
   once an owned key fires.
2. Tap Caps Lock 100 times with a steady ~1 Hz rhythm.
3. Pipe the captured stderr into the latency script:

   ```bash
   RUST_LOG=hhkb_daemon=trace cargo run -p hhkb-daemon 2> daemon.log
   # …tap Caps Lock 100 times…
   scripts/measure-tap-latency.sh < daemon.log
   ```

   The script pairs each `tx` line with the most recent unconsumed
   `rx` line and reports `samples / min / median / mean / p95 / p99 /
   max` in milliseconds, then asserts the kill-switch criterion below.
   Exit code is `1` on FAIL, `0` on PASS, `2` if no pairs were found.

**Pass criterion:** p99 < 250ms; max < 300ms (matches plan §3.M1
ceiling).

**Fail action:** record results, file as a v0.2.0 blocker, decide
between (a) raising the default `timeout_ms` floor, (b) shipping with
tap-hold disabled and the backend documented as "remap only", or (c)
descoping the native backend per plan §2 risk.

## Test 2 — Misfire rate under fast typing

Procedure:

1. POST [`docs/fixtures/m1-homerow.json`](fixtures/m1-homerow.json) and
   activate it (Caps→Esc/LCtrl plus `a`→hold-LSft, `s`→hold-LCtl,
   `d`→hold-LAlt, `f`→hold-LGui — all `timeout_ms: 200`).
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
