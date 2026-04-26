/**
 * High-level key binding data model for RoninKB software remaps.
 *
 * Abstracts the kanata `.kbd` S-expression syntax into three simple binding
 * types that cover the most common use-cases:
 *
 *   - `remap`        — send a different key when this key is pressed
 *   - `tap-hold`     — different action for tap vs hold (e.g. a / Ctrl)
 *   - `layer-switch` — activate a named layer while held or on toggle
 *
 * All kanata syntax generation and parsing is centralised here so the UI
 * components never have to reason about S-expressions directly.
 */

// ─── Binding types ────────────────────────────────────────────────────────────

export interface RemapBinding {
  type: 'remap';
  /** HHKB physical key index (1-based, 1..60). */
  sourceIndex: number;
  /** Kanata keycode token to emit (e.g. `'a'`, `'lctl'`). */
  target: string;
}

export interface TapHoldBinding {
  type: 'tap-hold';
  sourceIndex: number;
  tap: string;
  hold: string;
  /** Timeout in milliseconds (default 200). */
  timeout: number;
}

export interface LayerSwitchBinding {
  type: 'layer-switch';
  sourceIndex: number;
  layerName: string;
  mode: 'while-held' | 'toggle';
}

export type KeyBinding = RemapBinding | TapHoldBinding | LayerSwitchBinding;

// ─── Human-readable keycode table ────────────────────────────────────────────

export interface KanataKeycode {
  token: string;
  label: string;
}

export const KEYCODES: KanataKeycode[] = [
  // Letters
  ...Array.from('abcdefghijklmnopqrstuvwxyz').map((c) => ({
    token: c,
    label: c.toUpperCase(),
  })),
  // Numbers
  ...Array.from('1234567890').map((d) => ({ token: d, label: d })),
  // Special
  { token: 'esc', label: 'Escape' },
  { token: 'tab', label: 'Tab' },
  { token: 'spc', label: 'Space' },
  { token: 'ret', label: 'Return' },
  { token: 'bspc', label: 'Backspace' },
  { token: 'del', label: 'Delete' },
  { token: 'caps', label: 'Caps Lock' },
  // Modifiers
  { token: 'lctl', label: 'L Ctrl' },
  { token: 'rctl', label: 'R Ctrl' },
  { token: 'lsft', label: 'L Shift' },
  { token: 'rsft', label: 'R Shift' },
  { token: 'lalt', label: 'L Alt/Option' },
  { token: 'ralt', label: 'R Alt/Option' },
  { token: 'lmet', label: 'L Cmd/Win' },
  { token: 'rmet', label: 'R Cmd/Win' },
  // Navigation
  { token: 'left', label: '← Left' },
  { token: 'rght', label: '→ Right' },
  { token: 'up', label: '↑ Up' },
  { token: 'down', label: '↓ Down' },
  { token: 'home', label: 'Home' },
  { token: 'end', label: 'End' },
  { token: 'pgup', label: 'Page Up' },
  { token: 'pgdn', label: 'Page Down' },
  // Function keys
  ...Array.from({ length: 12 }, (_, i) => ({
    token: `f${i + 1}`,
    label: `F${i + 1}`,
  })),
  // Media
  { token: 'volu', label: 'Vol Up' },
  { token: 'vold', label: 'Vol Down' },
  { token: 'mute', label: 'Mute' },
  { token: 'pp', label: 'Play/Pause' },
  { token: 'nlck', label: 'Num Lock' },
];

export function tokenToLabel(token: string): string {
  return KEYCODES.find((k) => k.token === token)?.label ?? token;
}

/** Convert a raw kanata token to a short human-readable description. */
export function describeToken(token: string): string {
  // tap-hold: (tap-hold t1 t2 tap hold)
  const th = /^\(tap-hold\s+\d+\s+\d+\s+(\S+)\s+(\S+)\)$/.exec(token);
  if (th) return `${tokenToLabel(th[1])} · ${tokenToLabel(th[2])} (hold)`;

  // layer-while-held: (layer-while-held name)
  const lwh = /^\(layer-while-held\s+(\S+)\)$/.exec(token);
  if (lwh) return `Hold → ${lwh[1]}`;

  // layer-toggle: (layer-toggle name)
  const lt = /^\(layer-toggle\s+(\S+)\)$/.exec(token);
  if (lt) return `Toggle → ${lt[1]}`;

  return tokenToLabel(token);
}

// ─── Parser ──────────────────────────────────────────────────────────────────

/**
 * Tokenize a deflayer body, correctly handling nested parentheses as single
 * tokens (e.g. `(tap-hold 200 200 a lctl)` is one token, not five).
 */
function tokenizeBody(src: string): string[] {
  const tokens: string[] = [];
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    // Skip whitespace
    if (/\s/.test(ch)) { i++; continue; }
    // Skip line comments
    if (ch === ';' && src[i + 1] === ';') {
      while (i < src.length && src[i] !== '\n') i++;
      continue;
    }
    // Nested paren form → capture whole balanced expression
    if (ch === '(') {
      let depth = 0;
      const start = i;
      while (i < src.length) {
        if (src[i] === '(') depth++;
        else if (src[i] === ')') {
          depth--;
          if (depth === 0) { i++; break; }
        }
        i++;
      }
      tokens.push(src.slice(start, i));
    } else {
      // Plain token (no parens)
      const start = i;
      while (i < src.length && !/[\s()]/.test(src[i])) i++;
      tokens.push(src.slice(start, i));
    }
  }
  return tokens;
}

/**
 * Extract the outer balanced `(keyword ...)` block from `src`.
 * Unlike a simple regex, this correctly handles nested parentheses.
 */
function extractBlock(src: string, keywordPattern: string): string | null {
  const re = new RegExp(`\\(\\s*${keywordPattern}\\b`);
  const hit = re.exec(src);
  if (!hit) return null;
  let depth = 0;
  let i = hit.index;
  while (i < src.length) {
    if (src[i] === '(') depth++;
    else if (src[i] === ')') {
      depth--;
      if (depth === 0) return src.slice(hit.index, i + 1);
    }
    i++;
  }
  return null;
}

/**
 * Parse a kanata `.kbd` config string into the high-level `KeyBinding` model.
 *
 * Reads `(defsrc ...)` to learn each slot's physical key (translated back via
 * `HHKB_INDEX_TO_KANATA`) and pairs it with the matching action in
 * `(deflayer base ...)`. Other deflayer blocks and defalias forms are
 * ignored — they round-trip through `generateKanataConfig` only for keys
 * actually bound. Returns `[]` for empty / unparseable configs.
 */
export function parseKeyBindings(config: string): KeyBinding[] {
  if (!config.trim()) return [];
  const cleaned = config.replace(/;;[^\n]*/g, '');

  const baseBlock = extractBlock(cleaned, 'deflayer\\s+base');
  const defsrcBlock = extractBlock(cleaned, 'defsrc');
  if (!baseBlock || !defsrcBlock) return [];

  const baseInner = baseBlock
    .replace(/^\(\s*deflayer\s+base\s*/, '')
    .replace(/\)\s*$/, '');
  const defsrcInner = defsrcBlock
    .replace(/^\(\s*defsrc\s*/, '')
    .replace(/\)\s*$/, '');

  const actions = tokenizeBody(baseInner);
  const sources = tokenizeBody(defsrcInner);

  const bindings: KeyBinding[] = [];

  for (let i = 0; i < Math.min(sources.length, actions.length); i++) {
    const srcTok = sources[i];
    const tok = actions[i];
    if (!tok || tok === '_' || tok === 'XX' || tok === 'xx') continue;

    const sourceIndex = KANATA_TOKEN_TO_HHKB_INDEX[srcTok];
    if (!sourceIndex) continue; // unknown defsrc token — drop silently

    // tap-hold: (tap-hold timeout timeout tap hold)
    const th = /^\(tap-hold\s+(\d+)\s+\d+\s+(\S+)\s+(\S+)\)$/.exec(tok);
    if (th) {
      bindings.push({
        type: 'tap-hold',
        sourceIndex,
        timeout: Number(th[1]),
        tap: th[2],
        hold: th[3],
      });
      continue;
    }

    // layer-while-held: (layer-while-held name)
    const lwh = /^\(layer-while-held\s+(\S+)\)$/.exec(tok);
    if (lwh) {
      bindings.push({ type: 'layer-switch', sourceIndex, layerName: lwh[1], mode: 'while-held' });
      continue;
    }

    // layer-toggle: (layer-toggle name)
    const lt = /^\(layer-toggle\s+(\S+)\)$/.exec(tok);
    if (lt) {
      bindings.push({ type: 'layer-switch', sourceIndex, layerName: lt[1], mode: 'toggle' });
      continue;
    }

    // Simple remap (any other single token — alias references like `@foo` included)
    bindings.push({ type: 'remap', sourceIndex, target: tok });
  }

  return bindings;
}

// ─── Code generator ──────────────────────────────────────────────────────────

/**
 * Map HHKB EEPROM key index (1..60) → kanata key token.
 *
 * kanata's `defsrc` requires real OS-recognised key names so its event tap
 * can match incoming HID codes; placeholder tokens like `k1` are rejected
 * with `Unknown key in defsrc`.
 *
 * Index numbering matches `apps/hhkb-app/src/data/hhkbLayout.ts`. The HHKB
 * physical Fn key (index 6) is intentionally absent — the keyboard handles
 * Fn internally and the OS never sees it as a discrete keycode, so kanata
 * has nothing to bind to.
 */
const HHKB_INDEX_TO_KANATA: Readonly<Record<number, string>> = {
  // Row 0 — number row
  60: 'esc', 59: '1', 58: '2', 57: '3', 56: '4', 55: '5',
  54: '6', 53: '7', 52: '8', 51: '9', 50: '0', 49: '-',
  48: '=', 47: 'grv', 46: 'bspc',
  // Row 1 — Q row
  45: 'tab', 44: 'q', 43: 'w', 42: 'e', 41: 'r', 40: 't',
  39: 'y', 38: 'u', 37: 'i', 36: 'o', 35: 'p',
  34: '[', 33: ']', 32: '\\',
  // Row 2 — A row (HHKB ships Control here, not Caps)
  31: 'lctl', 30: 'a', 29: 's', 28: 'd', 27: 'f', 26: 'g',
  25: 'h', 24: 'j', 23: 'k', 22: 'l', 21: ';', 20: "'",
  19: 'ret',
  // Row 3 — Z row (index 6 = HHKB-internal Fn, omitted)
  18: 'lsft', 17: 'z', 16: 'x', 15: 'c', 14: 'v', 13: 'b',
  12: 'n', 11: 'm', 10: ',', 9: '.', 8: '/', 7: 'rsft',
  // Row 4 — modifier row
  5: 'lmet', 4: 'lalt', 3: 'spc', 2: 'ralt', 1: 'rmet',
};

/** Reverse of `HHKB_INDEX_TO_KANATA`, used by the parser to map a kanata
 *  defsrc token back to its physical HHKB key index on round-trip. */
const KANATA_TOKEN_TO_HHKB_INDEX: Readonly<Record<string, number>> = (() => {
  const out: Record<string, number> = {};
  for (const [idx, tok] of Object.entries(HHKB_INDEX_TO_KANATA)) {
    out[tok] = Number(idx);
  }
  return out;
})();

export function hhkbIndexToKanata(index: number): string | undefined {
  return HHKB_INDEX_TO_KANATA[index];
}

/**
 * Generate a complete kanata `.kbd` config from the given bindings.
 *
 * Only HHKB keys with an explicit binding go into `defsrc` — anything else
 * is passed through to the OS unchanged. Stub `deflayer` blocks are emitted
 * for layer-switch targets so kanata doesn't error on undefined layers.
 */
export function generateKanataConfig(bindings: KeyBinding[]): string {
  if (bindings.length === 0) return '';

  // Keep deterministic ordering so a config diff is reviewable.
  const sorted = [...bindings].sort((a, b) => a.sourceIndex - b.sourceIndex);

  const tokens: string[] = [];
  const actions: string[] = [];

  for (const b of sorted) {
    const src = HHKB_INDEX_TO_KANATA[b.sourceIndex];
    if (!src) continue; // unbindable physical key (e.g. HHKB Fn) — silently skip
    tokens.push(src);

    switch (b.type) {
      case 'remap':
        actions.push(b.target);
        break;
      case 'tap-hold':
        actions.push(`(tap-hold ${b.timeout} ${b.timeout} ${b.tap} ${b.hold})`);
        break;
      case 'layer-switch': {
        const kind = b.mode === 'toggle' ? 'layer-toggle' : 'layer-while-held';
        actions.push(`(${kind} ${b.layerName})`);
        break;
      }
    }
  }

  if (tokens.length === 0) return '';

  const defsrc = `(defsrc\n  ${tokens.join(' ')}\n)`;
  const deflayerBase = `(deflayer base\n  ${actions.join(' ')}\n)`;

  const layerNames = new Set<string>();
  for (const b of bindings) {
    if (b.type === 'layer-switch') layerNames.add(b.layerName);
  }
  const blanks = new Array(tokens.length).fill('_').join(' ');
  const extraLayers = [...layerNames].map(
    (name) => `(deflayer ${name}\n  ${blanks}\n)`,
  );

  return [defsrc, deflayerBase, ...extraLayers].join('\n\n');
}
