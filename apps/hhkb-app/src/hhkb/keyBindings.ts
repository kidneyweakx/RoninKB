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
 * Only reads the `(deflayer base ...)` block — other deflayer blocks and
 * defalias forms are ignored (they round-trip through `generateKanataConfig`
 * as-is when re-saved). Returns an empty array for empty / unparseable configs.
 */
export function parseKeyBindings(config: string): KeyBinding[] {
  if (!config.trim()) return [];
  const cleaned = config.replace(/;;[^\n]*/g, '');

  const block = extractBlock(cleaned, 'deflayer\\s+base');
  if (!block) return [];

  // Strip outer `(deflayer base ...)` wrapper to get the body tokens
  const inner = block
    .replace(/^\(\s*deflayer\s+base\s*/, '')
    .replace(/\)\s*$/, '');
  const tokens = tokenizeBody(inner);

  const bindings: KeyBinding[] = [];

  for (let i = 0; i < Math.min(tokens.length, 60); i++) {
    const tok = tokens[i];
    const sourceIndex = i + 1; // HHKB indices are 1-based

    if (!tok || tok === '_' || tok === 'XX' || tok === 'xx') continue;

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

const KEY_TOKENS = Array.from({ length: 60 }, (_, i) => `k${i + 1}`);

/**
 * Generate a complete kanata `.kbd` config from the given bindings.
 * Emits `(defsrc k1…k60)` + `(deflayer base ...)` + stub deflayer blocks
 * for any layer-switch targets.
 */
export function generateKanataConfig(bindings: KeyBinding[]): string {
  if (bindings.length === 0) return '';

  const slots: string[] = new Array(60).fill('_');

  for (const b of bindings) {
    const pos = b.sourceIndex - 1;
    if (pos < 0 || pos >= 60) continue;

    switch (b.type) {
      case 'remap':
        slots[pos] = b.target;
        break;
      case 'tap-hold':
        slots[pos] = `(tap-hold ${b.timeout} ${b.timeout} ${b.tap} ${b.hold})`;
        break;
      case 'layer-switch': {
        const kind = b.mode === 'toggle' ? 'layer-toggle' : 'layer-while-held';
        slots[pos] = `(${kind} ${b.layerName})`;
        break;
      }
    }
  }

  const defsrc = `(defsrc\n  ${KEY_TOKENS.join(' ')}\n)`;
  const deflayerBase = `(deflayer base\n  ${slots.join(' ')}\n)`;

  // Emit empty stub deflayers for any referenced layer names so kanata
  // doesn't error on missing layer declarations.
  const layerNames = new Set<string>();
  for (const b of bindings) {
    if (b.type === 'layer-switch') layerNames.add(b.layerName);
  }
  const extraLayers = [...layerNames].map(
    (name) => `(deflayer ${name}\n  ${new Array(60).fill('_').join(' ')}\n)`,
  );

  return [defsrc, deflayerBase, ...extraLayers].join('\n\n');
}
