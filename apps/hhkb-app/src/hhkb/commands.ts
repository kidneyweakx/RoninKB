/**
 * HHKB command builders and response parsers.
 *
 * Ported from `crates/hhkb-core/src/command.rs` and `spec/protocol/commands.md`.
 *
 * All `build*` helpers return a 64-byte Uint8Array ready to pass to
 * `HIDDevice.sendReport(REPORT_ID, buf)`. The leading `0x00` hidapi report-ID
 * prefix is NOT included (WebHID takes the report ID as a separate argument).
 *
 * This means every byte index from the Rust port shifts DOWN by one:
 *   Rust: [1]=0xAA [2]=0xAA [3]=CMD [4]=p1 [5]=p2 [6]=p3 [7]=p4 ...
 *   TS:   [0]=0xAA [1]=0xAA [2]=CMD [3]=p1 [4]=p2 [5]=p3 [6]=p4 ...
 */

import { buildRequest, HhkbResponse } from './protocol';
import {
  DipSwitchState,
  KeyboardInfo,
  KeyboardMode,
  dipSwitchFromBytes,
  keyboardModeFromByte,
  parseKeyboardInfo,
} from './types';

// ---------------------------------------------------------------------------
// Command IDs
// ---------------------------------------------------------------------------

export const CMD_NOTIFY_APP_STATE = 0x01;
export const CMD_GET_KEYBOARD_INFO = 0x02;
export const CMD_RESET_FACTORY = 0x03;
export const CMD_CONFIRM_KEYMAP = 0x04;
export const CMD_GET_DIP_SWITCH = 0x05;
export const CMD_GET_KEYBOARD_MODE = 0x06;
export const CMD_RESET_DIPSW = 0x07;
export const CMD_SET_KEYMAP = 0x86;
export const CMD_GET_KEYMAP = 0x87;

// ---------------------------------------------------------------------------
// Request builders
// ---------------------------------------------------------------------------

/** Notify the keyboard that the configuration application has opened. */
export function notifyAppOpen(): Uint8Array {
  // Rust params = [0x00, 0x01, 0x00] — state byte = 0 means "open".
  return buildRequest(CMD_NOTIFY_APP_STATE, [0x00, 0x01, 0x00]);
}

/** Notify the keyboard that the configuration application has closed. */
export function notifyAppClose(): Uint8Array {
  return buildRequest(CMD_NOTIFY_APP_STATE, [0x00, 0x01, 0x01]);
}

/** Request keyboard identity, firmware, and serial number. */
export function getKeyboardInfo(): Uint8Array {
  return buildRequest(CMD_GET_KEYBOARD_INFO);
}

/** Request the current DIP switch states. */
export function getDipSwitch(): Uint8Array {
  return buildRequest(CMD_GET_DIP_SWITCH);
}

/** Request the current keyboard operating mode. */
export function getKeyboardMode(): Uint8Array {
  return buildRequest(CMD_GET_KEYBOARD_MODE);
}

/**
 * Request the keymap for a given `mode` and layer.
 *
 * `fnLayer = false` reads the base layer, `true` reads the Fn layer.
 */
export function getKeymap(mode: KeyboardMode, fnLayer: boolean): Uint8Array {
  // Rust params = [0x00, 0x02, mode, fn] -> TS params same (slot positions
  // in the param-space are unchanged; only the leading report-ID byte moved).
  return buildRequest(CMD_GET_KEYMAP, [
    0x00,
    0x02,
    mode as number,
    fnLayer ? 1 : 0,
  ]);
}

/**
 * First of three writes that upload a full 128-byte keymap payload.
 * `data` must be exactly 57 bytes (layout[0..57]).
 */
export function setKeymapWrite1(
  mode: KeyboardMode,
  fnLayer: boolean,
  data: Uint8Array,
): Uint8Array {
  if (data.length !== 57) {
    throw new Error(`setKeymapWrite1 data must be 57 bytes, got ${data.length}`);
  }
  const params = new Uint8Array(61);
  params[0] = 65; // offset counter (matches Rust port)
  params[1] = 59; // chunk header + data length
  params[2] = mode as number;
  params[3] = fnLayer ? 1 : 0;
  params.set(data, 4);
  return buildRequest(CMD_SET_KEYMAP, params);
}

/** Second of three keymap writes — 59 bytes of data (layout[57..116]). */
export function setKeymapWrite2(data: Uint8Array): Uint8Array {
  if (data.length !== 59) {
    throw new Error(`setKeymapWrite2 data must be 59 bytes, got ${data.length}`);
  }
  const params = new Uint8Array(61);
  params[0] = 130;
  params[1] = 59;
  params.set(data, 2);
  return buildRequest(CMD_SET_KEYMAP, params);
}

/** Third of three keymap writes — the final 12 bytes (layout[116..128]). */
export function setKeymapWrite3(data: Uint8Array): Uint8Array {
  if (data.length !== 12) {
    throw new Error(`setKeymapWrite3 data must be 12 bytes, got ${data.length}`);
  }
  const params = new Uint8Array(14);
  params[0] = 195;
  params[1] = 12;
  params.set(data, 2);
  return buildRequest(CMD_SET_KEYMAP, params);
}

/** Commit / confirm a previously written keymap. */
export function confirmKeymap(): Uint8Array {
  return buildRequest(CMD_CONFIRM_KEYMAP);
}

/** Reset all DIP switches to their default state. */
export function resetDipsw(): Uint8Array {
  return buildRequest(CMD_RESET_DIPSW);
}

// ---------------------------------------------------------------------------
// Response parsers
// ---------------------------------------------------------------------------

export function parseKeyboardInfoResponse(
  resp: HhkbResponse,
): KeyboardInfo {
  return parseKeyboardInfo(resp.data);
}

export function parseDipSwitchResponse(resp: HhkbResponse): DipSwitchState {
  return dipSwitchFromBytes(resp.data.slice(0, 6));
}

export function parseKeyboardModeResponse(resp: HhkbResponse): KeyboardMode {
  return keyboardModeFromByte(resp.data[0]);
}
