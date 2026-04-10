import { describe, expect, it } from 'vitest';
import { Keymap, KEYMAP_SIZE } from '../keymap';

describe('Keymap', () => {
  it('new() returns all zeros', () => {
    const km = new Keymap();
    expect(km.asBytes().length).toBe(KEYMAP_SIZE);
    expect(km.overriddenCount()).toBe(0);
    for (let i = 0; i < KEYMAP_SIZE; i++) {
      expect(km.get(i)).toBe(0);
      expect(km.isDefault(i)).toBe(true);
    }
  });

  it('fromChunks assembles 58 + 58 + 12 from the data offsets', () => {
    const chunk1 = new Uint8Array(64);
    const chunk2 = new Uint8Array(64);
    const chunk3 = new Uint8Array(64);

    // Header bytes [0..6] are ignored; data starts at offset 6.
    chunk1[6] = 0x1f;
    chunk1[7] = 0x1e;
    chunk1[8] = 0x29;

    chunk2[6] = 0xaa;
    chunk2[7] = 0xbb;

    chunk3[6] = 0xe0;

    const km = Keymap.fromChunks(chunk1, chunk2, chunk3);
    expect(km.get(0)).toBe(0x1f);
    expect(km.get(1)).toBe(0x1e);
    expect(km.get(2)).toBe(0x29);
    expect(km.get(58)).toBe(0xaa);
    expect(km.get(59)).toBe(0xbb);
    expect(km.get(116)).toBe(0xe0);
    // last Fn bytes are fine
    expect(km.get(127)).toBe(0);
  });

  it('fromChunks rejects wrong-size chunks', () => {
    const ok = new Uint8Array(64);
    const short = new Uint8Array(32);
    expect(() => Keymap.fromChunks(short, ok, ok)).toThrow();
    expect(() => Keymap.fromChunks(ok, short, ok)).toThrow();
    expect(() => Keymap.fromChunks(ok, ok, short)).toThrow();
  });

  it('toWriteChunks splits into 57 / 59 / 12 and roundtrips', () => {
    const raw = new Uint8Array(KEYMAP_SIZE);
    for (let i = 0; i < KEYMAP_SIZE; i++) raw[i] = (i * 3 + 1) & 0xff;
    const km = new Keymap(raw);

    const [a, b, c] = km.toWriteChunks();
    expect(a.length).toBe(57);
    expect(b.length).toBe(59);
    expect(c.length).toBe(12);

    const reassembled = new Uint8Array(KEYMAP_SIZE);
    reassembled.set(a, 0);
    reassembled.set(b, 57);
    reassembled.set(c, 116);
    expect(Array.from(reassembled)).toEqual(Array.from(raw));
  });

  it('chunk roundtrip via HID frame simulation', () => {
    // Build a known keymap, write-chunk it, then simulate the 3 HID reads
    // (with 6 header bytes) and assert from-chunks == original.
    const raw = new Uint8Array(KEYMAP_SIZE);
    for (let i = 0; i < KEYMAP_SIZE; i++) raw[i] = (i * 7) & 0xff;
    const original = new Keymap(raw);

    // Simulate response chunks: 6 bytes of header (ignored) then data.
    const chunk1 = new Uint8Array(64);
    const chunk2 = new Uint8Array(64);
    const chunk3 = new Uint8Array(64);
    chunk1.set(raw.slice(0, 58), 6);
    chunk2.set(raw.slice(58, 116), 6);
    chunk3.set(raw.slice(116, 128), 6);

    const roundtripped = Keymap.fromChunks(chunk1, chunk2, chunk3);
    expect(Array.from(roundtripped.asBytes())).toEqual(
      Array.from(original.asBytes()),
    );
  });

  it('set / get / isDefault behave', () => {
    const km = new Keymap();
    km.set(3, 0x2c);
    expect(km.get(3)).toBe(0x2c);
    expect(km.isDefault(3)).toBe(false);
    expect(km.isDefault(4)).toBe(true);
  });

  it('set throws on out-of-range indices', () => {
    const km = new Keymap();
    expect(() => km.set(-1, 0)).toThrow();
    expect(() => km.set(128, 0)).toThrow();
  });

  it('overriddenCount counts non-zero bytes', () => {
    const km = new Keymap();
    expect(km.overriddenCount()).toBe(0);
    km.set(0, 1);
    km.set(10, 2);
    km.set(127, 3);
    expect(km.overriddenCount()).toBe(3);
  });

  it('clone is an independent copy', () => {
    const km = new Keymap();
    km.set(0, 0x41);
    const dup = km.clone();
    dup.set(0, 0x42);
    expect(km.get(0)).toBe(0x41);
    expect(dup.get(0)).toBe(0x42);
  });
});
