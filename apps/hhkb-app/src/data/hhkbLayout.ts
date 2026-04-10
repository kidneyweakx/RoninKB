/**
 * HHKB Professional Hybrid (60-key US ANSI) physical layout.
 *
 * Each entry is positioned by `row` / `col` / `width`, measured in 1u
 * keyboard units. `index` is the byte offset into the 128-byte HHKB EEPROM
 * keymap buffer (`keymap[1..=60]` for the 60 physical keys; 0 and 61..=127
 * are reserved / unused).
 *
 * Labels mirror the printed legends on the official PFU HHKB Professional
 * HYBRID Type-S US Layout (Non-Printed is the "legendless" SKU but we show
 * the same implied labels so users know which physical key they're editing).
 *
 * KEY INDEX MAPPING [CONFIRMED]
 * -----------------------------
 * Derived from happy-hacking-gnu `hhkb_print_layout_ansi()`
 * (see `vendor-reference/src/functions.h`):
 *
 *   Row 0 (number row, 15 keys): index 60 → 46, left to right
 *   Row 1 (Q row,      14 keys): index 45 → 32, left to right
 *   Row 2 (A row,      13 keys): index 31 → 19, left to right
 *   Row 3 (Z row,      13 keys): index 18 →  6, left to right
 *   Row 4 (modifier,    5 keys): index  5 →  1, left to right
 *
 *   Total: 15 + 14 + 13 + 13 + 5 = 60 keys ✓
 */

export interface HhkbKey {
  /** Index into the 128-byte HHKB EEPROM keymap (1..=60 for the 60 physical keys). */
  index: number;
  /** Primary printed glyph (unshifted / lower legend). */
  label: string;
  /** Optional secondary glyph (shifted / upper legend). */
  shift?: string;
  /** Optional subscript annotation (e.g. "R" on right-hand modifiers). */
  sub?: string;
  /** Row (0 = number row, 4 = bottom modifier row). */
  row: number;
  /** Leftmost column in 1u steps (floats allowed). */
  col: number;
  /** Width in 1u steps. Defaults to 1.0. */
  width?: number;
}

// ---------- internal helpers ----------

interface KeySpec {
  index: number;
  label: string;
  shift?: string;
  sub?: string;
  w?: number;
}

function buildRow(rowIndex: number, startCol: number, keys: KeySpec[]): HhkbKey[] {
  const out: HhkbKey[] = [];
  let col = startCol;
  for (const k of keys) {
    const w = k.w ?? 1;
    out.push({
      index: k.index,
      label: k.label,
      shift: k.shift,
      sub: k.sub,
      row: rowIndex,
      col,
      width: w,
    });
    col += w;
  }
  return out;
}

// ---------- rows (top → bottom) ----------
//
// The labels below match the silkscreen on the HHKB Pro HYBRID US layout in
// the PFU product photo: top-left is Esc, the grave/tilde key lives at the
// position right before Backspace, and Backspace is labeled "BS".

// Row 0: number row — index 60 → 46 (15 × 1u = 15u)
const row0 = buildRow(0, 0, [
  { index: 60, label: 'Esc' },
  { index: 59, label: '1', shift: '!' },
  { index: 58, label: '2', shift: '@' },
  { index: 57, label: '3', shift: '#' },
  { index: 56, label: '4', shift: '$' },
  { index: 55, label: '5', shift: '%' },
  { index: 54, label: '6', shift: '^' },
  { index: 53, label: '7', shift: '&' },
  { index: 52, label: '8', shift: '*' },
  { index: 51, label: '9', shift: '(' },
  { index: 50, label: '0', shift: ')' },
  { index: 49, label: '-', shift: '_' },
  { index: 48, label: '=', shift: '+' },
  { index: 47, label: '`', shift: '~' },
  { index: 46, label: 'BS' },
]);

// Row 1: Q row — index 45 → 32 (1.5 + 12×1 + 1.5 = 15u)
const row1 = buildRow(1, 0, [
  { index: 45, label: 'Tab', w: 1.5 },
  { index: 44, label: 'Q' },
  { index: 43, label: 'W' },
  { index: 42, label: 'E' },
  { index: 41, label: 'R' },
  { index: 40, label: 'T' },
  { index: 39, label: 'Y' },
  { index: 38, label: 'U' },
  { index: 37, label: 'I' },
  { index: 36, label: 'O' },
  { index: 35, label: 'P' },
  { index: 34, label: '[', shift: '{' },
  { index: 33, label: ']', shift: '}' },
  { index: 32, label: '\\', shift: '|', w: 1.5 },
]);

// Row 2: A row — index 31 → 19 (1.75 + 11×1 + 2.25 = 15u)
const row2 = buildRow(2, 0, [
  { index: 31, label: 'Control', w: 1.75 },
  { index: 30, label: 'A' },
  { index: 29, label: 'S' },
  { index: 28, label: 'D' },
  { index: 27, label: 'F' },
  { index: 26, label: 'G' },
  { index: 25, label: 'H' },
  { index: 24, label: 'J' },
  { index: 23, label: 'K' },
  { index: 22, label: 'L' },
  { index: 21, label: ';', shift: ':' },
  { index: 20, label: "'", shift: '"' },
  { index: 19, label: 'Return', w: 2.25 },
]);

// Row 3: Z row — index 18 → 6 (2.25 + 10×1 + 1.75 + 1 = 15u)
const row3 = buildRow(3, 0, [
  { index: 18, label: 'Shift', w: 2.25 },
  { index: 17, label: 'Z' },
  { index: 16, label: 'X' },
  { index: 15, label: 'C' },
  { index: 14, label: 'V' },
  { index: 13, label: 'B' },
  { index: 12, label: 'N' },
  { index: 11, label: 'M' },
  { index: 10, label: ',', shift: '<' },
  { index: 9, label: '.', shift: '>' },
  { index: 8, label: '/', shift: '?' },
  { index: 7, label: 'Shift', sub: 'R', w: 1.75 },
  { index: 6, label: 'Fn' },
]);

// Row 4: modifier row — index 5 → 1
// Approximate: 1.5u left pad + 1 + 1.5 + 6 + 1.5 + 1 = 12.5u
const row4 = buildRow(4, 1.5, [
  { index: 5, label: '◇' },
  { index: 4, label: 'Alt', shift: 'Opt', w: 1.5 },
  { index: 3, label: 'Space', w: 6 },
  { index: 2, label: 'Alt', shift: 'Opt', sub: 'R', w: 1.5 },
  { index: 1, label: '◇', sub: 'R' },
]);

/** Full HHKB ANSI layout, row-major top → bottom. */
export const HHKB_LAYOUT: HhkbKey[] = [...row0, ...row1, ...row2, ...row3, ...row4];

/** Total number of physical keys (60 for HHKB Pro). */
export const HHKB_KEY_COUNT = HHKB_LAYOUT.length;
