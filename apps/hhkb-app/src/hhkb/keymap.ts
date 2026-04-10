/**
 * A 128-byte HHKB keymap (one layer, one mode).
 *
 * Each byte is a HID Usage ID override. `0x00` means "use the firmware
 * default" for that key — i.e. no override. Ported from
 * `crates/hhkb-core/src/keymap.rs`.
 */

export const KEYMAP_SIZE = 128;
export const CHUNK_DATA_OFFSET = 6;
export const CHUNK1_LEN = 58;
export const CHUNK2_LEN = 58;
export const CHUNK3_LEN = 12;
export const CHUNK_REPORT_SIZE = 64;

export class Keymap {
  private readonly bytes: Uint8Array;

  constructor(data?: Uint8Array) {
    if (data) {
      if (data.length !== KEYMAP_SIZE) {
        throw new Error(
          `Keymap must be ${KEYMAP_SIZE} bytes, got ${data.length}`,
        );
      }
      this.bytes = new Uint8Array(data);
    } else {
      this.bytes = new Uint8Array(KEYMAP_SIZE);
    }
  }

  /**
   * Assemble a keymap from 3 HID response chunks (each 64 bytes raw).
   *
   * Keymap data is extracted starting at `CHUNK_DATA_OFFSET` from each
   * chunk. The split is 58 / 58 / 12 = 128 bytes.
   */
  static fromChunks(
    chunk1: Uint8Array,
    chunk2: Uint8Array,
    chunk3: Uint8Array,
  ): Keymap {
    for (const [i, c] of [chunk1, chunk2, chunk3].entries()) {
      if (c.length !== CHUNK_REPORT_SIZE) {
        throw new Error(
          `keymap chunk ${i + 1} must be ${CHUNK_REPORT_SIZE} bytes, got ${c.length}`,
        );
      }
    }
    const data = new Uint8Array(KEYMAP_SIZE);
    data.set(
      chunk1.slice(CHUNK_DATA_OFFSET, CHUNK_DATA_OFFSET + CHUNK1_LEN),
      0,
    );
    data.set(
      chunk2.slice(CHUNK_DATA_OFFSET, CHUNK_DATA_OFFSET + CHUNK2_LEN),
      CHUNK1_LEN,
    );
    data.set(
      chunk3.slice(CHUNK_DATA_OFFSET, CHUNK_DATA_OFFSET + CHUNK3_LEN),
      CHUNK1_LEN + CHUNK2_LEN,
    );
    return new Keymap(data);
  }

  /**
   * Split into 3 data slices suitable for the `WriteKeymap` command.
   *
   * Returns `[layout[0..57], layout[57..116], layout[116..128]]`. These are
   * the raw data bytes; framing is handled by commands.ts.
   */
  toWriteChunks(): [Uint8Array, Uint8Array, Uint8Array] {
    return [
      this.bytes.slice(0, 57),
      this.bytes.slice(57, 116),
      this.bytes.slice(116, 128),
    ];
  }

  /** Get the HID keycode at a given index (0..127), or undefined if OOB. */
  get(index: number): number | undefined {
    if (index < 0 || index >= KEYMAP_SIZE) return undefined;
    return this.bytes[index];
  }

  /** Set the HID keycode at a given index (0..127). Throws if OOB. */
  set(index: number, value: number): void {
    if (index < 0 || index >= KEYMAP_SIZE) {
      throw new Error(`keymap index out of range: ${index}`);
    }
    this.bytes[index] = value & 0xff;
  }

  /** True if the key at `index` uses the firmware default (value 0x00). */
  isDefault(index: number): boolean {
    return (this.get(index) ?? 0) === 0;
  }

  /** Raw 128-byte view. Returns a copy to preserve encapsulation. */
  asBytes(): Uint8Array {
    return new Uint8Array(this.bytes);
  }

  /** Count how many keys have non-default (non-zero) mappings. */
  overriddenCount(): number {
    let n = 0;
    for (const b of this.bytes) if (b !== 0) n++;
    return n;
  }

  clone(): Keymap {
    return new Keymap(this.bytes);
  }
}
