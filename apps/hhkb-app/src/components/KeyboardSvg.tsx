import { Box } from '@chakra-ui/react';
import { HHKB_LAYOUT } from '../data/hhkbLayout';
import { useDeviceStore } from '../store/deviceStore';

const UNIT_PX = 56;
const GAP_PX = 4;

interface Props {
  layer: 'base' | 'fn';
  selectedIndex: number | null;
  onSelect: (index: number) => void;
}

/**
 * Visual HHKB keymap — an absolutely-positioned grid of key buttons.
 * Good enough for v1; a real SVG path set can replace this later.
 */
export function KeyboardSvg({ layer, selectedIndex, onSelect }: Props) {
  const baseKeymap = useDeviceStore((s) => s.baseKeymap);
  const fnKeymap = useDeviceStore((s) => s.fnKeymap);
  const keymap = layer === 'base' ? baseKeymap : fnKeymap;

  const maxCols = Math.max(
    ...HHKB_LAYOUT.map((k) => k.col + (k.width ?? 1)),
  );
  const rows = Math.max(...HHKB_LAYOUT.map((k) => k.row)) + 1;

  const width = maxCols * UNIT_PX + (maxCols - 1) * GAP_PX;
  const height = rows * UNIT_PX + (rows - 1) * GAP_PX;

  return (
    <Box
      position="relative"
      w={`${width}px`}
      h={`${height}px`}
      bg="gray.800"
      borderRadius="lg"
      p={3}
      boxShadow="lg"
    >
      {HHKB_LAYOUT.map((k) => {
        const w = k.width ?? 1;
        const left = k.col * (UNIT_PX + GAP_PX);
        const top = k.row * (UNIT_PX + GAP_PX);
        const keyWidth = w * UNIT_PX + (w - 1) * GAP_PX;
        const override = keymap?.get(k.index) ?? 0;
        const isOverridden = override !== 0;
        const isSelected = k.index === selectedIndex;

        return (
          <Box
            key={`${k.row}-${k.col}-${k.label}`}
            as="button"
            position="absolute"
            left={`${left}px`}
            top={`${top}px`}
            w={`${keyWidth}px`}
            h={`${UNIT_PX}px`}
            bg={isSelected ? 'brand.500' : isOverridden ? 'purple.700' : 'gray.700'}
            color="white"
            borderRadius="md"
            fontSize="sm"
            fontWeight="semibold"
            border="2px solid"
            borderColor={isSelected ? 'brand.50' : 'gray.600'}
            onClick={() => onSelect(k.index)}
            _hover={{
              bg: isSelected ? 'brand.600' : 'gray.600',
            }}
          >
            {k.label}
          </Box>
        );
      })}
    </Box>
  );
}
