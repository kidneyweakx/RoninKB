/**
 * Shared types ported from `crates/hhkb-core/src/types.rs`.
 */

export const HHKB_VENDOR_ID = 0x04fe;
export const HHKB_PRODUCT_IDS = [0x0020, 0x0021, 0x0022] as const;
export const HHKB_USAGE_PAGE = 0xff00;

// ---------------------------------------------------------------------------
// KeyboardMode
// ---------------------------------------------------------------------------

export enum KeyboardMode {
  HHK = 0,
  Mac = 1,
  Lite = 2,
  Secret = 3,
}

export function keyboardModeFromByte(b: number): KeyboardMode {
  if (b < 0 || b > 3) {
    throw new Error(`invalid keyboard mode: ${b}`);
  }
  return b as KeyboardMode;
}

export function keyboardModeLabel(mode: KeyboardMode): string {
  switch (mode) {
    case KeyboardMode.HHK:
      return 'HHK';
    case KeyboardMode.Mac:
      return 'Mac';
    case KeyboardMode.Lite:
      return 'Lite';
    case KeyboardMode.Secret:
      return 'Secret';
  }
}

// ---------------------------------------------------------------------------
// FirmwareType
// ---------------------------------------------------------------------------

export enum FirmwareType {
  Application = 0,
  Bootloader = 1,
}

// ---------------------------------------------------------------------------
// DipSwitchState
// ---------------------------------------------------------------------------

export interface DipSwitchState {
  /** 6 switches: SW1..SW6, true = ON, false = OFF */
  switches: readonly [boolean, boolean, boolean, boolean, boolean, boolean];
}

export function dipSwitchFromBytes(bytes: Uint8Array): DipSwitchState {
  const get = (i: number) => (bytes[i] ?? 0) !== 0;
  return {
    switches: [get(0), get(1), get(2), get(3), get(4), get(5)],
  };
}

// ---------------------------------------------------------------------------
// KeyboardInfo
// ---------------------------------------------------------------------------

export interface KeyboardInfo {
  typeNumber: string;
  revision: Uint8Array; // 4 bytes
  serial: Uint8Array; // 16 bytes
  appFirmware: Uint8Array; // 8 bytes
  bootFirmware: Uint8Array; // 8 bytes
  runningFirmware: FirmwareType;
}

/**
 * Parse keyboard info from a response data slice.
 *
 * Layout (offsets within response.data, which starts at byte 6 of the
 * raw HID report — so data[0] is the first byte after the length byte):
 *   [ 0..20) type_number      — 20 bytes ASCII, NUL-padded
 *   [20..24) revision         — 4 bytes
 *   [24..40) serial           — 16 bytes
 *   [40..48) app_firmware     — 8 bytes
 *   [48..56) boot_firmware    — 8 bytes
 *   [56]    running_firmware  — 1 byte
 */
export function parseKeyboardInfo(data: Uint8Array): KeyboardInfo {
  if (data.length < 57) {
    throw new Error(
      `keyboard info too short: ${data.length} bytes (need at least 57)`,
    );
  }

  // ASCII decode, trim trailing NULs.
  let end = 20;
  while (end > 0 && data[end - 1] === 0) end--;
  let typeNumber = '';
  for (let i = 0; i < end; i++) {
    typeNumber += String.fromCharCode(data[i]);
  }

  const revision = data.slice(20, 24);
  const serial = data.slice(24, 40);
  const appFirmware = data.slice(40, 48);
  const bootFirmware = data.slice(48, 56);

  const fw = data[56];
  if (fw !== 0 && fw !== 1) {
    throw new Error(`invalid firmware type: ${fw}`);
  }

  return {
    typeNumber,
    revision,
    serial,
    appFirmware,
    bootFirmware,
    runningFirmware: fw as FirmwareType,
  };
}
