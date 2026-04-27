# RoninKB

Open-source configuration suite for the HHKB Professional Hybrid keyboard.

RoninKB gives you full control over your HHKB without vendor software:

- **`hhkb`** — a cross-platform CLI for inspecting and dumping the keyboard.
- **`hhkb-daemon`** — a background service exposing an HTTP + WebSocket API
  (with an embedded web UI at `http://127.0.0.1:7331/`) so any browser or
  script can manage the keyboard.
- **hhkb-app** — a React frontend for point-and-click remapping, layer
  editing, and Flow (cross-device clipboard sync).

Supports macOS, Linux, and Windows.

## Install

### macOS (Homebrew)

```bash
brew tap kidneyweakx/roninkb https://github.com/kidneyweakx/RoninKB.git
brew install roninkb
brew services start roninkb
open http://127.0.0.1:7331/
```

### macOS / Linux (curl)

```bash
curl -fsSL https://raw.githubusercontent.com/kidneyweakx/RoninKB/main/install.sh | sh
hhkb-daemon &
# then visit http://127.0.0.1:7331/
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/kidneyweakx/RoninKB/main/install.ps1 | iex
```

### From source

```bash
git clone https://github.com/kidneyweakx/RoninKB.git
cd RoninKB/apps/hhkb-app && npm install && npm run build
cd ../..
cargo install --path crates/hhkb-cli    --features hhkb-core/hidapi-backend
cargo install --path crates/hhkb-daemon --features embedded-ui
```

## Quick start

Once installed, start the daemon and open the UI:

```bash
hhkb-daemon &
open http://127.0.0.1:7331/      # macOS
xdg-open http://127.0.0.1:7331/  # Linux
```

Or use the CLI directly:

```bash
hhkb list        # enumerate connected HHKBs
hhkb info        # show current device info
hhkb dump        # dump current keymap
```

## Autostart

Each platform has a native "run at login" recipe in [`install/`](install/):

- **macOS** — LaunchAgent plist (`install/macos/dev.roninKB.daemon.plist`)
- **Linux** — systemd user unit + udev rule (`install/linux/`)
- **Windows** — Task Scheduler script (`install/windows/Install-Task.ps1`)

See [`install/README.md`](install/README.md) for step-by-step instructions.
Homebrew users can skip this — `brew services start roninkb` wires up
launchd automatically.

## Repository layout

```
crates/
  hhkb-core/    # shared keyboard protocol + HID backend
  hhkb-cli/     # `hhkb` CLI binary
  hhkb-daemon/  # `hhkb-daemon` HTTP/WS service (embeds the web UI)
apps/
  hhkb-app/     # React frontend (Vite + TypeScript)
install/        # OS-specific autostart recipes
Formula/        # Homebrew formula (auto-updated by release workflow)
```

## Releasing

See [`RELEASING.md`](RELEASING.md). TL;DR: bump versions, push a `v*.*.*`
tag, and the release workflow builds binaries, publishes a GitHub Release,
and refreshes the Homebrew formula.

## Acknowledgements

RoninKB ships and invokes [kanata](https://github.com/jtroo/kanata) for its
software-binding layer. kanata is licensed under
[LGPL-3.0](THIRD_PARTY_LICENSES/kanata-LICENSE.txt) and is included
unmodified — see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) for the
full attribution and how RoninKB satisfies the LGPL obligations.

## License

MIT. See [`LICENSE`](LICENSE).

Third-party components retain their own licenses — see
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
