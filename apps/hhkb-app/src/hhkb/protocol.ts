/**
 * HHKB HID protocol primitives.
 *
 * See `spec/protocol/usb-control.md` and `spec/protocol/commands.md`.
 *
 * IMPORTANT: WebHID's `HIDDevice.sendReport(reportId, data)` takes the report
 * ID SEPARATELY from the data, so our payload is 64 bytes and begins at the
 * magic bytes (no leading 0x00 report-ID prefix like hidapi uses).
 *
 *   Rust (hidapi): [0]=0x00  [1]=0xAA [2]=0xAA [3]=CMD [4..]=params  (65 bytes)
 *   TS   (WebHID): [0]=0xAA  [1]=0xAA [2]=CMD  [3..]=params           (64 bytes)
 */

export const MAGIC_REQUEST = [0xaa, 0xaa] as const;
export const MAGIC_RESPONSE = [0x55, 0x55] as const;
export const REQUEST_SIZE = 64;
export const RESPONSE_SIZE = 64;

/** The vendor HID interface uses no report IDs, so we send with report ID 0. */
export const REPORT_ID = 0;

export interface HhkbResponse {
  command: number;
  status: number;
  param: number;
  length: number;
  /** bytes [6..64] — always 58 elements */
  data: Uint8Array;
}

/**
 * Build a 64-byte HID request buffer.
 *
 * Layout:
 *   [0..2] = 0xAA 0xAA    magic
 *   [2]    = command ID
 *   [3..]  = params, zero-padded to fill the remaining 61 bytes
 */
export function buildRequest(
  command: number,
  params: ArrayLike<number> = [],
): Uint8Array {
  if (params.length > REQUEST_SIZE - 3) {
    throw new Error(
      `params too long: max ${REQUEST_SIZE - 3} bytes, got ${params.length}`,
    );
  }
  const buf = new Uint8Array(REQUEST_SIZE);
  buf[0] = MAGIC_REQUEST[0];
  buf[1] = MAGIC_REQUEST[1];
  buf[2] = command;
  for (let i = 0; i < params.length; i++) {
    buf[i + 3] = params[i] & 0xff;
  }
  return buf;
}

/**
 * Parse a 64-byte HID response from the device.
 *
 * Throws if the buffer is too short or the magic bytes are wrong.
 */
export function parseResponse(data: Uint8Array): HhkbResponse {
  if (data.length < RESPONSE_SIZE) {
    throw new Error(
      `response too short: expected ${RESPONSE_SIZE} bytes, got ${data.length}`,
    );
  }
  if (data[0] !== MAGIC_RESPONSE[0] || data[1] !== MAGIC_RESPONSE[1]) {
    throw new Error(
      `Invalid response magic: 0x${data[0].toString(16)} 0x${data[1]
        .toString(16)} (expected 0x55 0x55)`,
    );
  }
  return {
    command: data[2],
    status: data[3],
    param: data[4],
    length: data[5],
    data: data.slice(6, RESPONSE_SIZE),
  };
}
