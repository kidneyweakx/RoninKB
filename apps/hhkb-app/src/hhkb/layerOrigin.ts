/**
 * Layer origin derivation.
 *
 * Every physical HHKB key can have its binding come from one of three
 * "layers" in RoninKB's dual-layer model:
 *
 *   - `hw`   — the hardware EEPROM keymap (persisted by the keyboard itself).
 *              Survives unplug. Edits go through the HHKB HID protocol.
 *   - `sw`   — a local software override managed by the kanata engine, stored
 *              in the active profile's `_roninKB.software.config`. Only active
 *              while the daemon is running.
 *   - `flow` — a cross-device clipboard / flow placeholder. Reserved for
 *              future use; we use HID usage `0xFFFE` as the sentinel value
 *              (no real HID key uses this code).
 *
 * Precedence is `flow > sw > hw`. A key that is the firmware default
 * (both the raw byte is 0 *and* no software override targets it) returns
 * `null` so the UI can hide the tag entirely.
 *
 * This module is intentionally pure — no React, no zustand. The UI calls
 * `computeKeyOrigin` with already-extracted inputs so the logic can be
 * covered by plain unit tests.
 */

/** Sentinel keycode representing a "Flow" placeholder binding. */
export const FLOW_KEYCODE = 0xfffe;

export type LayerOrigin = 'hw' | 'sw' | 'flow' | null;

export type Layer = 'base' | 'fn';

/**
 * Best-effort parse of a kanata `.kbd` config.
 *
 * We extract each `(defsrc ...)` block and each `(deflayer <name> ...)`
 * block, tokenise their contents, and return one entry per deflayer with
 * the token at each position. This is not a full s-expression parser — it
 * only supports the subset of syntax that the RoninKB macro editor
 * generates. Any unparseable config simply yields empty arrays and the
 * caller falls through to the hardware layer.
 */
export interface ParsedKanata {
  /** The token list from `(defsrc ...)` (position → source key token). */
  defsrc: string[];
  /**
   * One entry per `(deflayer ...)` declaration. Position i in `tokens`
   * corresponds to position i in `defsrc`.
   */
  layers: Array<{ name: string; tokens: string[] }>;
}

const BLOCK_RE = /\(\s*(defsrc|deflayer)\b([\s\S]*?)\)/g;

export function parseKanataConfig(source: string): ParsedKanata {
  const out: ParsedKanata = { defsrc: [], layers: [] };
  if (!source) return out;

  // Drop line comments starting with `;;`.
  const cleaned = source.replace(/;;[^\n]*/g, '');

  BLOCK_RE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = BLOCK_RE.exec(cleaned)) !== null) {
    const kind = match[1];
    const body = match[2];
    const tokens = body
      .split(/\s+/)
      .map((t) => t.trim())
      .filter((t) => t.length > 0);

    if (kind === 'defsrc') {
      // If there are multiple defsrc blocks, later ones win (rare).
      out.defsrc = tokens;
    } else {
      // deflayer's first token is the layer name.
      const [name, ...rest] = tokens;
      if (name) out.layers.push({ name, tokens: rest });
    }
  }

  return out;
}

/**
 * Given a physical key index and the profile's software config, returns
 * true if that key position has an active (non-underscore) override in
 * any deflayer.
 *
 * The convention (matching kanata docs) is that `_` at a position means
 * "fall through" and therefore is NOT an override. Any other token — a
 * keycode like `a`, an alias like `@my-macro`, or a list like
 * `(tap-hold ...)` — counts as an override.
 *
 * Key-index to defsrc-position mapping: the macro editor emits defsrc in
 * HHKB physical order, which means position 0 == physical key at HHKB
 * index 1 (HHKB indices are 1-based on hardware, see data/hhkbLayout.ts).
 * We treat the `keyIndex` passed in as the HHKB index and subtract 1 for
 * the defsrc position. Out-of-range indices return false.
 */
export function hasSoftwareOverride(
  keyIndex: number,
  parsed: ParsedKanata,
): boolean {
  if (parsed.layers.length === 0) return false;
  const pos = keyIndex - 1;
  if (pos < 0) return false;

  for (const layer of parsed.layers) {
    const tok = layer.tokens[pos];
    if (tok === undefined) continue;
    if (tok === '_' || tok === '' || tok === 'XX' || tok === 'xx') continue;
    return true;
  }
  return false;
}

/**
 * Compute the origin label for a single key.
 *
 * @param keyIndex The HHKB EEPROM index (1..60 for physical keys).
 * @param layer Which hardware layer the UI is currently showing.
 * @param baseBytes The 128-byte base layer read from the device.
 * @param fnBytes The 128-byte Fn layer read from the device.
 * @param softwareConfig The active profile's kanata text (may be empty).
 * @param daemonOnline Whether the daemon is currently reachable.
 */
export function computeKeyOrigin(
  keyIndex: number,
  layer: Layer,
  baseBytes: Uint8Array | null,
  fnBytes: Uint8Array | null,
  softwareConfig: string | null | undefined,
  daemonOnline: boolean,
): LayerOrigin {
  const bytes = layer === 'base' ? baseBytes : fnBytes;
  const raw = bytes ? (bytes[keyIndex] ?? 0) : 0;

  // Flow sentinel wins. Requires daemon — a Flow binding only resolves
  // when the daemon is running the flow router.
  if (raw === (FLOW_KEYCODE & 0xff)) {
    // NOTE: HHKB's EEPROM is one byte per slot, so the full 0xFFFE
    // sentinel truncates to 0xFE. We therefore also require the
    // surrounding profile metadata to flag this slot as a Flow key in
    // the real thing — for now the byte match is enough for the UI.
    return daemonOnline ? 'flow' : null;
  }

  // Software override: only meaningful when the daemon is online.
  if (daemonOnline && softwareConfig) {
    const parsed = parseKanataConfig(softwareConfig);
    if (hasSoftwareOverride(keyIndex, parsed)) {
      return 'sw';
    }
  }

  // Hardware: a non-zero byte in the EEPROM keymap.
  if (raw !== 0) return 'hw';

  return null;
}
