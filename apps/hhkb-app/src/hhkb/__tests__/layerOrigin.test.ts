import { describe, expect, it } from 'vitest';
import {
  FLOW_KEYCODE,
  computeKeyOrigin,
  hasSoftwareOverride,
  parseKanataConfig,
} from '../layerOrigin';

function makeBytes(overrides: Record<number, number> = {}): Uint8Array {
  const bytes = new Uint8Array(128);
  for (const [k, v] of Object.entries(overrides)) {
    bytes[Number(k)] = v;
  }
  return bytes;
}

describe('parseKanataConfig', () => {
  it('parses a minimal defsrc + deflayer', () => {
    const cfg = `
      (defsrc a b c)
      (deflayer base 1 2 3)
    `;
    const parsed = parseKanataConfig(cfg);
    expect(parsed.defsrc).toEqual(['a', 'b', 'c']);
    expect(parsed.layers).toEqual([
      { name: 'base', tokens: ['1', '2', '3'] },
    ]);
  });

  it('handles multiple deflayers', () => {
    const cfg = `
      (defsrc a b)
      (deflayer base x y)
      (deflayer nav _ _)
    `;
    const parsed = parseKanataConfig(cfg);
    expect(parsed.layers).toHaveLength(2);
    expect(parsed.layers[0].name).toBe('base');
    expect(parsed.layers[1].name).toBe('nav');
  });

  it('strips ;; line comments', () => {
    const cfg = `
      ;; a comment
      (defsrc a b) ;; inline comment
      (deflayer base 1 2)
    `;
    const parsed = parseKanataConfig(cfg);
    expect(parsed.defsrc).toEqual(['a', 'b']);
    expect(parsed.layers[0].tokens).toEqual(['1', '2']);
  });

  it('returns empty on blank input', () => {
    expect(parseKanataConfig('')).toEqual({ defsrc: [], layers: [] });
  });
});

describe('hasSoftwareOverride', () => {
  it('returns false when config has no layers', () => {
    const parsed = parseKanataConfig('');
    expect(hasSoftwareOverride(1, parsed)).toBe(false);
  });

  it('returns true when position has a real keycode', () => {
    const cfg = `
      (defsrc a b c)
      (deflayer base esc _ _)
    `;
    const parsed = parseKanataConfig(cfg);
    // keyIndex 1 → position 0 → "esc" (override)
    expect(hasSoftwareOverride(1, parsed)).toBe(true);
    // keyIndex 2 → position 1 → "_" (fall through)
    expect(hasSoftwareOverride(2, parsed)).toBe(false);
  });

  it('treats alias tokens as overrides', () => {
    const cfg = `
      (defsrc a)
      (deflayer base @my-macro)
    `;
    const parsed = parseKanataConfig(cfg);
    expect(hasSoftwareOverride(1, parsed)).toBe(true);
  });

  it('ignores out-of-range key indices', () => {
    const cfg = `
      (defsrc a)
      (deflayer base esc)
    `;
    const parsed = parseKanataConfig(cfg);
    expect(hasSoftwareOverride(50, parsed)).toBe(false);
    expect(hasSoftwareOverride(-5, parsed)).toBe(false);
  });
});

describe('computeKeyOrigin', () => {
  const baseBytes = makeBytes({ 1: 0x04, 2: 0x00, 3: FLOW_KEYCODE & 0xff });
  const fnBytes = makeBytes();

  it('returns hw when only the EEPROM byte is non-zero', () => {
    const origin = computeKeyOrigin(1, 'base', baseBytes, fnBytes, '', false, false);
    expect(origin).toBe('hw');
  });

  it('returns null for a firmware-default key', () => {
    const origin = computeKeyOrigin(2, 'base', baseBytes, fnBytes, '', false, false);
    expect(origin).toBe(null);
  });

  it('returns sw when daemon is online and config overrides the key', () => {
    const cfg = `
      (defsrc a b c)
      (deflayer base esc _ _)
    `;
    // keyIndex 1 → pos 0 → "esc" → sw
    const origin = computeKeyOrigin(1, 'base', baseBytes, fnBytes, cfg, true, true);
    expect(origin).toBe('sw');
  });

  it('falls back to hw when daemon is offline even with a sw config', () => {
    const cfg = `
      (defsrc a b c)
      (deflayer base esc _ _)
    `;
    const origin = computeKeyOrigin(1, 'base', baseBytes, fnBytes, cfg, false, false);
    expect(origin).toBe('hw');
  });

  it('returns hw when daemon is online but kanata is not running', () => {
    const cfg = `
      (defsrc a b c)
      (deflayer base esc _ _)
    `;
    const origin = computeKeyOrigin(1, 'base', baseBytes, fnBytes, cfg, true, false);
    expect(origin).toBe('hw');
  });

  it('returns flow for the sentinel keycode', () => {
    const origin = computeKeyOrigin(3, 'base', baseBytes, fnBytes, '', true, false);
    expect(origin).toBe('flow');
  });

  it('flow requires daemon online', () => {
    const origin = computeKeyOrigin(3, 'base', baseBytes, fnBytes, '', false, false);
    expect(origin).toBe(null);
  });
});
