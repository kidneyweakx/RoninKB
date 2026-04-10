import { useState, useMemo } from 'react';
import { Box, Input, SimpleGrid, Button, VStack, Text } from '@chakra-ui/react';

/**
 * A tiny, searchable keycode list. Ships with a minimal set of HID Usage IDs
 * from USB HID Usage Tables 1.4 section 10.
 */
interface KeycodeEntry {
  code: number;
  name: string;
  category: 'letter' | 'number' | 'mod' | 'nav' | 'fn' | 'misc';
}

const KEYCODES: KeycodeEntry[] = [
  // Letters
  ...Array.from({ length: 26 }, (_, i) => ({
    code: 0x04 + i,
    name: String.fromCharCode(65 + i),
    category: 'letter' as const,
  })),
  // Numbers 1..0
  ...['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'].map((n, i) => ({
    code: 0x1e + i,
    name: n,
    category: 'number' as const,
  })),
  // Navigation / control
  { code: 0x28, name: 'Enter', category: 'nav' },
  { code: 0x29, name: 'Escape', category: 'nav' },
  { code: 0x2a, name: 'Backspace', category: 'nav' },
  { code: 0x2b, name: 'Tab', category: 'nav' },
  { code: 0x2c, name: 'Space', category: 'nav' },
  { code: 0x39, name: 'Caps Lock', category: 'nav' },
  { code: 0x4f, name: '→ Right', category: 'nav' },
  { code: 0x50, name: '← Left', category: 'nav' },
  { code: 0x51, name: '↓ Down', category: 'nav' },
  { code: 0x52, name: '↑ Up', category: 'nav' },
  // Modifiers
  { code: 0xe0, name: 'L Ctrl', category: 'mod' },
  { code: 0xe1, name: 'L Shift', category: 'mod' },
  { code: 0xe2, name: 'L Alt', category: 'mod' },
  { code: 0xe3, name: 'L Cmd', category: 'mod' },
  { code: 0xe4, name: 'R Ctrl', category: 'mod' },
  { code: 0xe5, name: 'R Shift', category: 'mod' },
  { code: 0xe6, name: 'R Alt', category: 'mod' },
  { code: 0xe7, name: 'R Cmd', category: 'mod' },
  // F1..F12
  ...Array.from({ length: 12 }, (_, i) => ({
    code: 0x3a + i,
    name: `F${i + 1}`,
    category: 'fn' as const,
  })),
  // Default
  { code: 0x00, name: 'Default', category: 'misc' },
];

export function KeycodePicker({ onPick }: { onPick: (code: number) => void }) {
  const [query, setQuery] = useState('');
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return KEYCODES;
    return KEYCODES.filter(
      (k) =>
        k.name.toLowerCase().includes(q) ||
        k.code.toString(16).includes(q),
    );
  }, [query]);

  return (
    <VStack align="stretch" spacing={3}>
      <Input
        size="sm"
        placeholder="Search keycode..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <Box maxH="280px" overflowY="auto" pr={1}>
        <SimpleGrid columns={4} spacing={2}>
          {filtered.map((k) => (
            <Button
              key={k.code}
              size="xs"
              variant="outline"
              colorScheme="whiteAlpha"
              onClick={() => onPick(k.code)}
              title={`0x${k.code.toString(16).padStart(2, '0')}`}
            >
              {k.name}
            </Button>
          ))}
        </SimpleGrid>
        {filtered.length === 0 && (
          <Text color="gray.400" fontSize="sm">
            No matches.
          </Text>
        )}
      </Box>
    </VStack>
  );
}
