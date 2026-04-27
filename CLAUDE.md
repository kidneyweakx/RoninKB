# RoninKB — Working Notes for Claude

A Rust workspace + a React/Chakra frontend that drives an HHKB Pro Hybrid keyboard over USB-HID and observes its Bluetooth status. Three crates (`hhkb-core`, `hhkb-cli`, `hhkb-daemon`) and one app (`apps/hhkb-app`).

> **Single biggest reason CI fails: the local checks below were skipped.** Run them before claiming done.

## CI parity — what runs on every push

CI (`.github/workflows/ci.yml`) is a 3-OS matrix. Reproduce locally before you push.

### Rust workspace — must pass on macOS / Linux / Windows

```bash
cargo fmt --all -- --check
cargo build --workspace --features hhkb-core/hidapi-backend
cargo test --workspace
cargo clippy --workspace --all-targets --features hhkb-core/hidapi-backend -- -D warnings
```

`-D warnings` is strict — every clippy lint is a fail. Fix (don't `#[allow]`) unless you have a documented reason.

### Feature matrix — also CI-gated

```bash
cargo check -p hhkb-core   --features firmware-write
cargo check -p hhkb-daemon --features tray
cargo check -p hhkb-daemon --features clipboard
cargo check -p hhkb-daemon --features tray,clipboard
# Requires the frontend to be built first (embedded-ui inlines dist/):
( cd apps/hhkb-app && npm ci && npm run build )
cargo check -p hhkb-daemon --features embedded-ui
```

### Frontend — `apps/hhkb-app`

```bash
cd apps/hhkb-app
npm ci
npx tsc --noEmit       # strict TS, no `any` smuggling
npm run test           # vitest
npm run build          # tsc -b && vite build
```

## Linux system deps (CI installs these via apt)

If you're touching `hhkb-daemon` features that pull `dbus`/`tray`/`appindicator`, you need:

```bash
sudo apt-get install -y \
  libudev-dev libxdo-dev pkg-config \
  libdbus-1-dev libglib2.0-dev libgtk-3-dev libayatana-appindicator3-dev
```

Missing any of these → `cargo build` fails on Linux only.

## Platform-specific gotchas (recent CI breakers)

These each took a fix commit — don't re-introduce them.

- **`Info.plist` write must be macOS-only.** The daemon's bundle setup uses macOS-only APIs. Wrap any `Info.plist` / `CFBundle*` / TCC code in `#[cfg(target_os = "macos")]` or it breaks Linux/Windows builds. (Commit `542f1a3`.)
- **Clippy collapsible-match** is enforced. Nested `if let` patterns in `ws.rs` style code must collapse — `cargo clippy --fix` usually does it. (Commit `1958718`.)
- **`defsrc` in kanata configs uses real key tokens**, not placeholders. The parser is `defsrc`-driven. (Commit `7c8e7e9`.)
- **Don't gate on stale TCC state.** Input-Monitoring permission is checked by `kanata` itself, not the daemon's TCC probe. Don't add `inputMonitoringGranted` gates that block the start path.

## Frontend conventions (don't fight Chakra)

- **Chakra UI v2** + Framer Motion + Zustand. No Tailwind. Style via Chakra props or `sx` — semantic tokens live in `apps/hhkb-app/src/theme.ts`.
- **Color tokens are the contract.** Never hardcode hex in components — use `kanata.fg`, `hardware.fg`, `wireless.fg`, `accent.primary`, `bg.surface`, etc. New colors → add a token first.
- **Two-color discipline**: kanata/software = cyan-teal; hardware/EEPROM = amber. These distinguish reversibility (software can be undone, EEPROM is durable).
- **`KeyBinding`/`ViaProfile` types** are the source of truth. The daemon's HTTP DTOs match them — don't drift.
- **WebHID lives only in `deviceStore` / `hhkb/`.** UI components shouldn't import HID APIs directly.

## Useful one-liners

```bash
# Full local CI sweep (Rust + frontend, fail-fast)
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets --features hhkb-core/hidapi-backend -- -D warnings && \
  cargo test --workspace && \
  ( cd apps/hhkb-app && npm run build )

# Rebuild daemon with embedded UI (after frontend changes)
( cd apps/hhkb-app && npm run build ) && cargo build -p hhkb-daemon --features embedded-ui

# Run daemon locally
cargo run -p hhkb-daemon
```

## What NOT to do

- Don't bypass `--no-verify` — pre-commit hooks catch the same things CI does.
- Don't add `#[allow(clippy::...)]` to silence a warning. Fix it.
- Don't introduce new top-level dependencies (Tailwind, styled-components, etc.) — extend Chakra.
- Don't put platform-specific code in `hhkb-core`. Core stays portable; OS bits live in `hhkb-daemon` (with `cfg` gates).
- Don't widen the `firmware-write` / `embedded-ui` feature scopes without a CI matrix update.
- Don't rename color tokens without grepping every component first.

## Third-party licenses (kanata is LGPL-3.0)

RoninKB itself is MIT, but the `bundled-kanata` feature ships a kanata
release binary which is **LGPL-3.0**. The whole project stays MIT — see
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) for why and what
obligations are satisfied.

**Hard rules — don't break the LGPL compliance posture:**

- **Don't modify kanata source** inside this repo. `build.rs` downloads an
  unmodified upstream release; keep it that way. If you need a kanata fork,
  host it separately and adjust `KANATA_VERSION` (or the URL) in
  `crates/hhkb-daemon/build.rs`.
- **Don't statically link kanata as a library.** It must stay a separate
  child process invoked from `kanata.rs`. Linking kanata as a library would
  flip the LGPL combined-work analysis and likely require RoninKB to
  publish under LGPL.
- **Don't delete `THIRD_PARTY_LICENSES/kanata-LICENSE.txt`.** `kanata.rs`
  uses `include_bytes!` on it (when `bundled-kanata` is on) so the LICENSE
  ships next to the extracted binary. Removing or renaming the file breaks
  the compile.
- **Don't drop the kanata attribution** from `README.md` or
  `THIRD_PARTY_NOTICES.md`.
