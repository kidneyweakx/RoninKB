import { HStack, Tag, Text } from '@chakra-ui/react';
import { Cpu, Layers, Radio } from 'lucide-react';
import type { LayerOrigin } from '../hhkb/layerOrigin';

/**
 * Tiny badge indicating which layer a key's binding originates from.
 * Rendered next to the keycode display in KeyDetailPanel.
 *
 * - `[硬體]` (hardware) — slate background, Cpu icon
 * - `[本機]` (software) — accent purple, Layers icon
 * - `[Flow]` (cross-device) — cyan-ish info, Radio icon
 * - `null` → renders nothing
 */
export function LayerOriginLabel({ origin }: { origin: LayerOrigin }) {
  if (!origin) return null;

  const spec = ORIGIN_SPEC[origin];
  return (
    <Tag size="sm" variant={spec.variant} title={spec.tooltip}>
      <HStack spacing={1}>
        <spec.Icon size={10} />
        <Text
          fontSize="10px"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.06em"
        >
          {spec.label}
        </Text>
      </HStack>
    </Tag>
  );
}

interface OriginSpec {
  label: string;
  tooltip: string;
  variant: 'subtle' | 'accent' | 'success' | 'warning' | 'danger';
  Icon: typeof Cpu;
}

const ORIGIN_SPEC: Record<'hw' | 'sw' | 'flow', OriginSpec> = {
  hw: {
    label: '[硬體]',
    tooltip: 'Stored in HHKB EEPROM — survives unplug',
    variant: 'subtle',
    Icon: Cpu,
  },
  sw: {
    label: '[本機]',
    tooltip: 'Software layer via kanata — requires daemon',
    variant: 'accent',
    Icon: Layers,
  },
  flow: {
    label: '[Flow]',
    tooltip: 'Cross-device flow binding — requires daemon',
    variant: 'success',
    Icon: Radio,
  },
};
