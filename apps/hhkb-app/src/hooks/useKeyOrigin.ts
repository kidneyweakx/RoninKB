/**
 * Hooks that fuse the hardware VIA keymap with the active profile's kanata
 * software config (`_roninKB.software.config`) so the UI can render a
 * single, unified view of "where does this key's binding come from".
 *
 * Pure computation lives in `../hhkb/layerOrigin.ts`; this file only
 * provides the React-flavoured glue (memoised parse, store subscriptions).
 */

import { useMemo } from 'react';
import { useDeviceStore } from '../store/deviceStore';
import { useDaemonStore } from '../store/daemonStore';
import { useKanataStore } from '../store/kanataStore';
import { useProfileStore } from '../store/profileStore';
import {
  computeKeyOrigin,
  hasSoftwareOverride,
  parseKanataConfig,
  type Layer,
  type LayerOrigin,
  type ParsedKanata,
} from '../hhkb/layerOrigin';

/**
 * The active profile's software (kanata) config plus a parsed cache.
 * Re-parses only when the config string identity changes.
 */
export function useActiveSoftwareConfig(): {
  config: string;
  parsed: ParsedKanata;
} {
  const activeProfile = useProfileStore((s) => s.getActive)();
  const config = activeProfile?.via._roninKB?.software?.config ?? '';
  const parsed = useMemo(() => parseKanataConfig(config), [config]);
  return { config, parsed };
}

/**
 * Returns the effective origin (`hw | sw | flow | null`) for a single
 * HHKB key index on the currently-displayed layer. Subscribes to device
 * bytes, daemon status, and the active profile so the UI stays live.
 */
export function useKeyOrigin(
  keyIndex: number | null,
  layer: Layer,
): LayerOrigin {
  const baseKeymap = useDeviceStore((s) => s.baseKeymap);
  const fnKeymap = useDeviceStore((s) => s.fnKeymap);
  const daemonStatus = useDaemonStore((s) => s.status);
  const kanataState = useKanataStore((s) => s.processState);
  const { config } = useActiveSoftwareConfig();

  return useMemo(() => {
    if (keyIndex === null) return null;
    const daemonOnline = daemonStatus === 'online';
    const softwareActive = daemonOnline && kanataState === 'running';
    return computeKeyOrigin(
      keyIndex,
      layer,
      baseKeymap?.asBytes() ?? null,
      fnKeymap?.asBytes() ?? null,
      config,
      daemonOnline,
      softwareActive,
    );
  }, [keyIndex, layer, baseKeymap, fnKeymap, config, daemonStatus, kanataState]);
}

/**
 * Returns a predicate `(keyIndex) => boolean` that tells the caller whether
 * a given HHKB key has an active software override in the current profile.
 * Useful for `KeyboardSvg` which renders 60 keys at once — the predicate is
 * memoised so parse happens once per profile change.
 */
export function useHasSoftwareOverride(): (keyIndex: number) => boolean {
  const { parsed } = useActiveSoftwareConfig();
  const daemonOnline = useDaemonStore((s) => s.status === 'online');
  const kanataRunning = useKanataStore((s) => s.processState === 'running');
  return useMemo(() => {
    if (!daemonOnline || !kanataRunning) {
      return () => false;
    }
    return (keyIndex: number) => hasSoftwareOverride(keyIndex, parsed);
  }, [parsed, daemonOnline, kanataRunning]);
}

/**
 * Extract the raw kanata token that overrides a given HHKB key position
 * across all `deflayer` blocks, returning the first non-passthrough hit.
 * Useful for showing the live binding in the key detail panel.
 *
 * Returns `null` if no layer overrides this slot (all `_`), or if the
 * index is out of range.
 */
export function useSoftwareTokenAt(
  keyIndex: number | null,
): { token: string; layerName: string } | null {
  const { parsed } = useActiveSoftwareConfig();
  return useMemo(() => {
    if (keyIndex === null) return null;
    const pos = keyIndex - 1;
    if (pos < 0) return null;
    for (const layer of parsed.layers) {
      const tok = layer.tokens[pos];
      if (tok === undefined) continue;
      if (tok === '_' || tok === '' || tok === 'XX' || tok === 'xx') continue;
      return { token: tok, layerName: layer.name };
    }
    return null;
  }, [keyIndex, parsed]);
}
