import { Box, Heading, Text, VStack, HStack, Badge, Divider, Button } from '@chakra-ui/react';
import { useState } from 'react';
import { useDeviceStore } from '../store/deviceStore';
import { HHKB_LAYOUT } from '../data/hhkbLayout';
import { KeycodePicker } from './KeycodePicker';

interface Props {
  selectedIndex: number | null;
  layer: 'base' | 'fn';
}

export function KeyDetailPanel({ selectedIndex, layer }: Props) {
  const baseKeymap = useDeviceStore((s) => s.baseKeymap);
  const fnKeymap = useDeviceStore((s) => s.fnKeymap);
  const setKeyOverride = useDeviceStore((s) => s.setKeyOverride);

  const [picking, setPicking] = useState(false);

  if (selectedIndex === null) {
    return (
      <Box p={5} bg="gray.800" borderRadius="lg" h="100%">
        <Text color="gray.400">Click a key to inspect and edit.</Text>
      </Box>
    );
  }

  const meta = HHKB_LAYOUT.find((k) => k.index === selectedIndex);
  const keymap = layer === 'base' ? baseKeymap : fnKeymap;
  const currentCode = keymap?.get(selectedIndex) ?? 0;
  const isDefault = currentCode === 0;

  return (
    <Box p={5} bg="gray.800" borderRadius="lg" h="100%">
      <VStack align="stretch" spacing={4}>
        <Heading size="md">{meta?.label ?? `Key #${selectedIndex}`}</Heading>
        <Text color="gray.400" fontSize="sm">
          Index {selectedIndex} · {layer === 'base' ? 'Base' : 'Fn'} layer
        </Text>

        <Divider borderColor="gray.600" />

        <HStack>
          <Text fontWeight="semibold">HID code:</Text>
          <Text fontFamily="mono">
            {isDefault ? 'default' : `0x${currentCode.toString(16).padStart(2, '0')}`}
          </Text>
          {!isDefault && <Badge colorScheme="purple">override</Badge>}
        </HStack>

        <Button
          size="sm"
          colorScheme="blue"
          onClick={() => setPicking((v) => !v)}
          isDisabled={!keymap}
        >
          {picking ? 'Cancel' : 'Edit keycode'}
        </Button>

        {picking && (
          <KeycodePicker
            onPick={(code) => {
              setKeyOverride(selectedIndex, code, layer);
              setPicking(false);
            }}
          />
        )}

        <Divider borderColor="gray.600" />
        <Text fontSize="xs" color="gray.500">
          Key index mapping derived from happy-hacking-gnu reverse engineering.
          See{' '}
          <Text as="span" fontFamily="mono">
            src/data/hhkbLayout.ts
          </Text>
          .
        </Text>
      </VStack>
    </Box>
  );
}
