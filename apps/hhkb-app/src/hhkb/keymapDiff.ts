/**
 * Helpers for diffing EEPROM keymap bytes against profile-stored layers.
 *
 * The profile's `_roninKB.hardware.raw_layers` stores `base` and `fn` as
 * plain number arrays, typically exactly KEYMAP_SIZE (128) long. The
 * live device store holds Uint8Array buffers of the same size. Trailing
 * zero padding is ignored because both ends may truncate differently.
 */

import { KEYMAP_SIZE } from './keymap';

export interface RawLayers {
  base: number[];
  fn: number[];
}

function trimTrailingZeros(arr: ArrayLike<number>): number[] {
  let end = arr.length;
  while (end > 0 && (arr[end - 1] ?? 0) === 0) end--;
  const out: number[] = [];
  for (let i = 0; i < end; i++) out.push((arr[i] ?? 0) & 0xff);
  return out;
}

/** True if the two byte sequences match ignoring trailing zero padding. */
export function bytesEqualIgnoringTrailingZeros(
  a: ArrayLike<number> | null | undefined,
  b: ArrayLike<number> | null | undefined,
): boolean {
  // Treat missing-vs-allZeros as equal (profile just omits the layer).
  if (!a && !b) return true;
  const ta = a ? trimTrailingZeros(a) : [];
  const tb = b ? trimTrailingZeros(b) : [];
  if (ta.length !== tb.length) return false;
  for (let i = 0; i < ta.length; i++) {
    if (ta[i] !== tb[i]) return false;
  }
  return true;
}

/**
 * Compares the live device bytes against the profile's saved raw layers.
 * Returns `true` if they are identical (modulo trailing zeros), which
 * means the banner should NOT be shown.
 */
export function isInSync(
  liveBase: Uint8Array | null,
  liveFn: Uint8Array | null,
  profileRaw: RawLayers | null | undefined,
): boolean {
  if (!profileRaw) return true; // profile has nothing to diff against
  const baseOk = bytesEqualIgnoringTrailingZeros(
    liveBase,
    profileRaw.base ?? [],
  );
  const fnOk = bytesEqualIgnoringTrailingZeros(liveFn, profileRaw.fn ?? []);
  return baseOk && fnOk;
}

/**
 * Pad / truncate a profile layer array to exactly KEYMAP_SIZE bytes so it
 * can be written back through the device protocol.
 */
export function normalizeRawLayer(layer: number[] | undefined): Uint8Array {
  const out = new Uint8Array(KEYMAP_SIZE);
  if (!layer) return out;
  for (let i = 0; i < Math.min(layer.length, KEYMAP_SIZE); i++) {
    out[i] = (layer[i] ?? 0) & 0xff;
  }
  return out;
}
