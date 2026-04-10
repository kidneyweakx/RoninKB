import { describe, expect, it } from 'vitest';
import {
  ViaProfile,
  hasRoninExtension,
  parseViaProfile,
  serializeViaProfile,
  toViaOnly,
} from '../via';

const PURE_VIA_JSON = `{
  "name": "HHKB Professional Hybrid",
  "vendorId": "0x04FE",
  "productId": "0x0021",
  "layers": [["KC_ESC", "KC_1"], ["KC_F1", "KC_F2"]]
}`;

const RONIN_JSON = `{
  "name": "HHKB Professional Hybrid",
  "vendorId": "0x04FE",
  "productId": "0x0021",
  "matrix": { "rows": 8, "cols": 8 },
  "layers": [["KC_ESC", "KC_1"]],
  "_roninKB": {
    "version": "0.1.0",
    "profile": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Daily Driver",
      "icon": "keyboard",
      "tags": ["work", "coding"]
    },
    "hardware": {
      "keyboard_mode": 0,
      "raw_layers": {
        "base": [41, 30, 31],
        "fn": [58, 59, 60]
      }
    },
    "software": {
      "engine": "kanata",
      "engine_version": "1.7.0",
      "config": "(defsrc a)\\n(deflayer base a)"
    }
  }
}`;

describe('via.parseViaProfile', () => {
  it('parses a pure VIA JSON', () => {
    const p = parseViaProfile(PURE_VIA_JSON);
    expect(p.name).toBe('HHKB Professional Hybrid');
    expect(p.vendorId).toBe('0x04FE');
    expect(p.productId).toBe('0x0021');
    expect(p.layers?.[0]).toEqual(['KC_ESC', 'KC_1']);
    expect(hasRoninExtension(p)).toBe(false);
  });

  it('parses a RoninKB-extended VIA JSON', () => {
    const p = parseViaProfile(RONIN_JSON);
    expect(hasRoninExtension(p)).toBe(true);
    expect(p.matrix).toEqual({ rows: 8, cols: 8 });
    const ronin = p._roninKB!;
    expect(ronin.version).toBe('0.1.0');
    expect(ronin.profile.name).toBe('Daily Driver');
    expect(ronin.hardware?.keyboard_mode).toBe(0);
    expect(ronin.hardware?.raw_layers.base).toEqual([41, 30, 31]);
    expect(ronin.hardware?.raw_layers.fn).toEqual([58, 59, 60]);
    expect(ronin.software?.engine).toBe('kanata');
  });

  it('rejects missing required fields', () => {
    expect(() => parseViaProfile('{}')).toThrow();
  });
});

describe('via.serialize roundtrip', () => {
  it('roundtrip preserves all RoninKB fields', () => {
    const original = parseViaProfile(RONIN_JSON);
    const json = serializeViaProfile(original);
    const decoded = parseViaProfile(json);
    expect(decoded.name).toBe(original.name);
    expect(decoded.vendorId).toBe(original.vendorId);
    expect(decoded.layers).toEqual(original.layers);
    expect(decoded._roninKB).toEqual(original._roninKB);
  });

  it('toViaOnly strips _roninKB', () => {
    const p = parseViaProfile(RONIN_JSON);
    const stripped = toViaOnly(p);
    expect(hasRoninExtension(stripped)).toBe(false);
    const json = serializeViaProfile(stripped);
    expect(json.includes('_roninKB')).toBe(false);
  });

  it('preserves unknown top-level keys (extras passthrough)', () => {
    const input = `{
      "name": "X",
      "vendorId": "0x0000",
      "productId": "0x0000",
      "customKeycodes": [{"name": "MY_MACRO"}],
      "menus": ["custom_panel"]
    }`;
    const parsed = parseViaProfile(input);
    expect(parsed.customKeycodes).toEqual([{ name: 'MY_MACRO' }]);
    const reserialized = serializeViaProfile(parsed);
    expect(reserialized).toContain('customKeycodes');
    expect(reserialized).toContain('menus');
  });

  it('empty profile omits absent fields', () => {
    const p: ViaProfile = {
      name: 'Minimal',
      vendorId: '0x04FE',
      productId: '0x0021',
    };
    const json = serializeViaProfile(p);
    const decoded = parseViaProfile(json);
    expect(decoded.name).toBe('Minimal');
    expect(decoded.layers).toBeUndefined();
    expect(decoded._roninKB).toBeUndefined();
  });
});
