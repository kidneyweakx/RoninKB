# hhkb-app

RoninKB WebHID frontend for HHKB Professional Hybrid keyboards. React + Vite +
TypeScript SPA. No external HID libraries — uses `navigator.hid` directly.

## Quick start

```bash
npm install
npm run dev     # dev server on http://localhost:5173
npm run build   # tsc + vite build
npm run test    # vitest unit tests
```

## Browser support

Requires WebHID. Tested in Chrome / Edge / Opera on HTTPS or `localhost` origins.

## Structure

```
src/
├── hhkb/        TypeScript port of the HHKB protocol (crates/hhkb-core)
├── store/       Zustand stores (device, profile, daemon)
├── components/  React UI (Chakra UI v2)
└── data/        Physical HHKB key layout
```

## Protocol notes

WebHID differs from hidapi in one important way: `HIDDevice.sendReport(reportId, data)`
takes the report ID separately, so request buffers are 64 bytes — NOT 65 with a
leading `0x00`. Every byte index in `spec/protocol/commands.md` shifts down by
one from the Rust implementation in `crates/hhkb-core`.

## Known limitations

- **Key-to-byte index mapping is a placeholder.** `src/data/hhkbLayout.ts` uses
  sequential indices 0..59. The real byte offsets inside the 128-byte keymap
  buffer have not been fully reverse-engineered (see the TODO there and
  `spec/protocol/keymap-encoding.md`). Do not trust visual edits when writing
  to real hardware until that mapping is verified.
- Daemon detection only checks `localhost:7331/health`. Actual daemon
  integration (profile sync, Kanata control) is not implemented yet.
- No persistent profile storage — the profile store is in-memory only.
