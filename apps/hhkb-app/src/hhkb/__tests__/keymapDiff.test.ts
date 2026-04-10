import { describe, expect, it } from 'vitest';
import {
  bytesEqualIgnoringTrailingZeros,
  isInSync,
  normalizeRawLayer,
} from '../keymapDiff';

describe('bytesEqualIgnoringTrailingZeros', () => {
  it('returns true for identical buffers', () => {
    expect(
      bytesEqualIgnoringTrailingZeros(new Uint8Array([1, 2, 3]), [1, 2, 3]),
    ).toBe(true);
  });

  it('ignores trailing zero padding on either side', () => {
    expect(
      bytesEqualIgnoringTrailingZeros(
        new Uint8Array([1, 2, 3, 0, 0, 0]),
        [1, 2, 3],
      ),
    ).toBe(true);
    expect(
      bytesEqualIgnoringTrailingZeros(
        [1, 2, 3],
        new Uint8Array([1, 2, 3, 0]),
      ),
    ).toBe(true);
  });

  it('returns false when content differs', () => {
    expect(
      bytesEqualIgnoringTrailingZeros([1, 2, 3], [1, 2, 4]),
    ).toBe(false);
  });

  it('returns false when non-trailing length differs', () => {
    expect(bytesEqualIgnoringTrailingZeros([1, 2, 3], [1, 2])).toBe(false);
  });

  it('treats null/undefined as empty', () => {
    expect(bytesEqualIgnoringTrailingZeros(null, [])).toBe(true);
    expect(bytesEqualIgnoringTrailingZeros(null, [0, 0, 0])).toBe(true);
    expect(bytesEqualIgnoringTrailingZeros(null, [0, 1, 0])).toBe(false);
  });
});

describe('isInSync', () => {
  const base = new Uint8Array([1, 2, 3]);
  const fn = new Uint8Array([4, 5, 6]);

  it('returns true when profile raw_layers is missing', () => {
    expect(isInSync(base, fn, null)).toBe(true);
    expect(isInSync(base, fn, undefined)).toBe(true);
  });

  it('returns true when both layers match (with padding)', () => {
    expect(
      isInSync(base, fn, { base: [1, 2, 3, 0, 0], fn: [4, 5, 6] }),
    ).toBe(true);
  });

  it('returns false when base differs', () => {
    expect(isInSync(base, fn, { base: [1, 2, 9], fn: [4, 5, 6] })).toBe(
      false,
    );
  });

  it('returns false when fn differs', () => {
    expect(isInSync(base, fn, { base: [1, 2, 3], fn: [4, 5, 9] })).toBe(
      false,
    );
  });
});

describe('normalizeRawLayer', () => {
  it('pads short arrays to 128 bytes', () => {
    const out = normalizeRawLayer([1, 2, 3]);
    expect(out.length).toBe(128);
    expect(out[0]).toBe(1);
    expect(out[3]).toBe(0);
  });

  it('truncates over-long arrays to 128 bytes', () => {
    const long = new Array(200).fill(0).map((_, i) => i & 0xff);
    const out = normalizeRawLayer(long);
    expect(out.length).toBe(128);
    expect(out[127]).toBe(127);
  });

  it('handles undefined', () => {
    const out = normalizeRawLayer(undefined);
    expect(out.length).toBe(128);
    expect(Array.from(out).every((b) => b === 0)).toBe(true);
  });
});
