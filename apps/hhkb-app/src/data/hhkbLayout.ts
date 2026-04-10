/**
 * HHKB Professional Hybrid (60-key US ANSI) physical layout.
 *
 * Each entry is positioned by `row` / `col` / `width`, measured in 1u
 * keyboard units. `index` is the byte offset into the 128-byte HHKB EEPROM
 * keymap buffer (`keymap[1..=60]` for the 60 physical keys; 0 and 61..=127
 * are reserved / unused).
 *
 * KEY INDEX MAPPING [CONFIRMED]
 * -----------------------------
 * Derived from happy-hacking-gnu `hhkb_print_layout_ansi()`
 * (see `vendor-reference/src/functions.h` and `spec/protocol/keymap-encoding.md`):
 *
 *   Row 0 (number row, 15 keys): index 60 → 46, left to right
 *     60=`  59=1 58=2 57=3 56=4 55=5 54=6 53=7 52=8 51=9
 *     50=0  49=- 48==  47=\  46=Backspace
 *
 *   Row 1 (Q row, 14 keys): index 45 → 32, left to right
 *     45=Tab(1.5u)  44=Q 43=W 42=E 41=R 40=T 39=Y 38=U 37=I 36=O
 *     35=P 34=[ 33=]  32=\(1.5u)
 *
 *   Row 2 (A row, 13 keys): index 31 → 19, left to right
 *     31=Control(1.75u)  30=A 29=S 28=D 27=F 26=G 25=H 24=J 23=K
 *     22=L 21=; 20='  19=Enter(2.25u)
 *
 *   Row 3 (Z row, 13 keys): index 18 → 6, left to right
 *     18=LShift(2.25u) 17=Z 16=X 15=C 14=V 13=B 12=N 11=M
 *     10=, 9=. 8=/  7=RShift(1.75u)  6=Fn
 *
 *   Row 4 (modifier row, 5 keys): index 5 → 1, left to right
 *     5=LCmd  4=LAlt(1.5u)  3=Space(6u)  2=RAlt(1.5u)  1=RCmd
 *
 *   Total: 15 + 14 + 13 + 13 + 5 = 60 keys ✓
 *
 * Width approximation note: the HHKB's real bottom row uses slightly non-
 * standard key widths. We pick values that sum neatly to 15u across all rows
 * so the rendered keyboard looks rectangular, while still matching the real
 * hardware's relative proportions (LAlt/RAlt ~1.5u, Space ~6u).
 */

export interface HhkbKey {
  /** Index into the 128-byte HHKB EEPROM keymap (1..=60 for the 60 physical keys). */
  index: number;
  /** Default label shown on the physical keycap (ANSI Mac mode). */
  label: string;
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
      row: rowIndex,
      col,
      width: w,
    });
    col += w;
  }
  return out;
}

// ---------- rows (top → bottom) ----------

// Row 0: number row — index 60 → 46 (15 × 1u = 15u)
const row0 = buildRow(0, 0, [
  { index: 60, label: '`' },
  { index: 59, label: '1' },
  { index: 58, label: '2' },
  { index: 57, label: '3' },
  { index: 56, label: '4' },
  { index: 55, label: '5' },
  { index: 54, label: '6' },
  { index: 53, label: '7' },
  { index: 52, label: '8' },
  { index: 51, label: '9' },
  { index: 50, label: '0' },
  { index: 49, label: '-' },
  { index: 48, label: '=' },
  { index: 47, label: '\\' },
  { index: 46, label: 'Delete' },
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
  { index: 34, label: '[' },
  { index: 33, label: ']' },
  { index: 32, label: '\\', w: 1.5 },
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
  { index: 21, label: ';' },
  { index: 20, label: "'" },
  { index: 19, label: 'Enter', w: 2.25 },
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
  { index: 10, label: ',' },
  { index: 9, label: '.' },
  { index: 8, label: '/' },
  { index: 7, label: 'Shift', w: 1.75 },
  { index: 6, label: 'Fn' },
]);

// Row 4: modifier row — index 5 → 1
// Approximate: 1.5u left pad + 1 + 1.5 + 6 + 1.5 + 1 = 12.5u  (then ~2.5u trailing gap)
const row4 = buildRow(4, 1.5, [
  { index: 5, label: '◆' },
  { index: 4, label: 'Alt', w: 1.5 },
  { index: 3, label: 'Space', w: 6 },
  { index: 2, label: 'Alt', w: 1.5 },
  { index: 1, label: '◆' },
]);

/** Full HHKB ANSI layout, row-major top → bottom. */
export const HHKB_LAYOUT: HhkbKey[] = [...row0, ...row1, ...row2, ...row3, ...row4];

/** Total number of physical keys (60 for HHKB Pro). */
export const HHKB_KEY_COUNT = HHKB_LAYOUT.length;
