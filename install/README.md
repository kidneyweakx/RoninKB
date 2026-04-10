# Installing the RoninKB daemon for autostart

The WebHID app works without the daemon, but installing it unlocks the
software-macro layer (Kanata) and Flow (cross-device clipboard). This guide
wires `hhkb-daemon` into each OS's native "run at login" mechanism.

All three platforms end with the same verification step:

```
curl http://127.0.0.1:7331/health
# -> {"ok":true,"device_connected":false}
```

Build the binary first with `cargo build -p hhkb-daemon --release` — the
release binary lives at `target/release/hhkb-daemon`.

---

## macOS (LaunchAgent)

1. Copy the binary somewhere on `PATH`:
   ```bash
   sudo cp target/release/hhkb-daemon /usr/local/bin/hhkb-daemon
   sudo chmod +x /usr/local/bin/hhkb-daemon
   ```
2. Install the LaunchAgent plist:
   ```bash
   mkdir -p ~/Library/LaunchAgents
   cp install/macos/dev.roninKB.daemon.plist ~/Library/LaunchAgents/
   launchctl load ~/Library/LaunchAgents/dev.roninKB.daemon.plist
   ```
3. Verify:
   ```bash
   curl http://127.0.0.1:7331/health
   ```

Logs land in `/tmp/roninKB-daemon.log` and `/tmp/roninKB-daemon.err`.
Uninstall with `launchctl unload ~/Library/LaunchAgents/dev.roninKB.daemon.plist`.

---

## Linux (systemd user unit + udev)

1. Install the binary and the udev rule (udev needs root, the unit does
   not):
   ```bash
   sudo cp target/release/hhkb-daemon /usr/local/bin/hhkb-daemon
   sudo cp install/linux/99-roninKB.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```
   Unplug and replug the keyboard so the new rule takes effect.
2. Install and enable the user unit:
   ```bash
   mkdir -p ~/.config/systemd/user
   cp install/linux/roninKB-daemon.service ~/.config/systemd/user/
   systemctl --user daemon-reload
   systemctl --user enable --now roninKB-daemon.service
   ```
3. Verify:
   ```bash
   systemctl --user status roninKB-daemon.service
   curl http://127.0.0.1:7331/health
   ```

Logs stream via `journalctl --user -u roninKB-daemon -f`.
Uninstall with `systemctl --user disable --now roninKB-daemon.service`.

---

## Windows (Task Scheduler)

1. Copy the binary into place (from an elevated PowerShell):
   ```powershell
   New-Item -ItemType Directory -Force "C:\Program Files\RoninKB" | Out-Null
   Copy-Item target\release\hhkb-daemon.exe "C:\Program Files\RoninKB\"
   ```
2. Register the scheduled task:
   ```powershell
   powershell -ExecutionPolicy Bypass -File install\windows\Install-Task.ps1
   ```
3. Sign out and back in (or run the task manually from
   `taskschd.msc`), then verify:
   ```powershell
   Invoke-WebRequest http://127.0.0.1:7331/health
   ```

Uninstall with `Unregister-ScheduledTask -TaskName "RoninKB Daemon" -Confirm:$false`.

---

## Troubleshooting

* **`health` returns `device_connected: false`** — that's fine; the daemon
  runs even without a keyboard attached, and `/device/*` endpoints lazily
  reconnect.
* **Port 7331 busy** — another instance is already running. `lsof -i :7331`
  on macOS/Linux or `Get-NetTCPConnection -LocalPort 7331` on Windows.
* **Linux: permission denied on hidraw** — the udev rule didn't apply.
  `udevadm info /dev/hidraw0` should show `TAGS=...uaccess...`. Unplug the
  keyboard and replug after reloading the rules.
