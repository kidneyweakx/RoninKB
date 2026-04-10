/**
 * VIA JSON superset — RoninKB profile serialization format.
 *
 * See `spec/architecture/file-format.md`. RoninKB writes VIA-compatible JSON
 * with an additional `_roninKB` namespace that VIA silently ignores.
 *
 * The implementation intentionally uses a permissive `Record<string, unknown>`
 * passthrough for unknown top-level keys so round-tripping a VIA file keeps
 * fields we don't explicitly model (e.g. `customKeycodes`, `menus`).
 */

export interface ViaMatrix {
  rows: number;
  cols: number;
}

export interface RoninProfileMeta {
  id: string;
  name: string;
  icon?: string;
  tags?: string[];
  created_at?: string;
}

export interface RoninHardwareConfig {
  keyboard_mode: number;
  raw_layers: {
    base: number[];
    fn: number[];
  };
}

export interface RoninSoftwareConfig {
  engine: string;
  engine_version?: string;
  config: string;
}

export interface RoninExtension {
  version: string;
  profile: RoninProfileMeta;
  hardware?: RoninHardwareConfig;
  software?: RoninSoftwareConfig;
}

/**
 * The parsed VIA profile. Unknown keys are preserved in `extras` and are
 * merged back on serialize, so round-tripping is lossless.
 */
export interface ViaProfile {
  name: string;
  vendorId: string;
  productId: string;
  matrix?: ViaMatrix;
  layouts?: unknown;
  keycodes?: string[];
  customKeycodes?: unknown[];
  lighting?: string;
  layers?: string[][];
  _roninKB?: RoninExtension;
  /** Any top-level keys we don't model explicitly. */
  extras?: Record<string, unknown>;
}

const KNOWN_KEYS = new Set([
  'name',
  'vendorId',
  'productId',
  'matrix',
  'layouts',
  'keycodes',
  'customKeycodes',
  'lighting',
  'layers',
  '_roninKB',
]);

export function parseViaProfile(json: string): ViaProfile {
  const raw = JSON.parse(json) as Record<string, unknown>;
  return viaProfileFromObject(raw);
}

export function viaProfileFromObject(raw: Record<string, unknown>): ViaProfile {
  if (typeof raw.name !== 'string') {
    throw new Error('VIA profile missing required `name` field');
  }
  if (typeof raw.vendorId !== 'string') {
    throw new Error('VIA profile missing required `vendorId` field');
  }
  if (typeof raw.productId !== 'string') {
    throw new Error('VIA profile missing required `productId` field');
  }

  const extras: Record<string, unknown> = {};
  for (const key of Object.keys(raw)) {
    if (!KNOWN_KEYS.has(key)) {
      extras[key] = raw[key];
    }
  }

  const profile: ViaProfile = {
    name: raw.name,
    vendorId: raw.vendorId,
    productId: raw.productId,
  };

  if (raw.matrix) profile.matrix = raw.matrix as ViaMatrix;
  if (raw.layouts !== undefined) profile.layouts = raw.layouts;
  if (Array.isArray(raw.keycodes)) profile.keycodes = raw.keycodes as string[];
  if (Array.isArray(raw.customKeycodes)) profile.customKeycodes = raw.customKeycodes;
  if (typeof raw.lighting === 'string') profile.lighting = raw.lighting;
  if (Array.isArray(raw.layers)) profile.layers = raw.layers as string[][];
  if (raw._roninKB) profile._roninKB = raw._roninKB as RoninExtension;
  if (Object.keys(extras).length > 0) profile.extras = extras;

  return profile;
}

/**
 * Serialize to pretty JSON. Key order matches the spec example in
 * `spec/architecture/file-format.md`.
 */
export function serializeViaProfile(p: ViaProfile): string {
  const obj: Record<string, unknown> = {
    name: p.name,
    vendorId: p.vendorId,
    productId: p.productId,
  };
  if (p.matrix) obj.matrix = p.matrix;
  if (p.layouts !== undefined) obj.layouts = p.layouts;
  if (p.keycodes && p.keycodes.length > 0) obj.keycodes = p.keycodes;
  if (p.customKeycodes) obj.customKeycodes = p.customKeycodes;
  if (p.lighting !== undefined) obj.lighting = p.lighting;
  if (p.layers) obj.layers = p.layers;
  if (p.extras) {
    for (const [k, v] of Object.entries(p.extras)) obj[k] = v;
  }
  if (p._roninKB) obj._roninKB = p._roninKB;
  return JSON.stringify(obj, null, 2);
}

/** Returns a pure VIA profile (strips `_roninKB`). */
export function toViaOnly(p: ViaProfile): ViaProfile {
  const { _roninKB: _unused, ...rest } = p;
  void _unused;
  return rest;
}

export function hasRoninExtension(p: ViaProfile): boolean {
  return p._roninKB !== undefined;
}
