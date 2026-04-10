import { useState, useMemo } from 'react';
import {
  Box,
  Input,
  InputGroup,
  InputLeftElement,
  SimpleGrid,
  Button,
  VStack,
  Text,
  HStack,
  Flex,
} from '@chakra-ui/react';
import { Search } from 'lucide-react';

/**
 * A tiny, searchable keycode list. Ships with a minimal set of HID Usage IDs
 * from USB HID Usage Tables 1.4 section 10.
 */
interface KeycodeEntry {
  code: number;
  name: string;
  category: 'letter' | 'number' | 'mod' | 'nav' | 'fn' | 'misc';
}

type Category = KeycodeEntry['category'] | 'all';

const CATEGORY_LABELS: Record<Category, string> = {
  all: 'All',
  letter: 'Letters',
  number: 'Numbers',
  mod: 'Modifiers',
  nav: 'Navigation',
  fn: 'Function',
  misc: 'Misc',
};

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
  { code: 0x4f, name: 'Right', category: 'nav' },
  { code: 0x50, name: 'Left', category: 'nav' },
  { code: 0x51, name: 'Down', category: 'nav' },
  { code: 0x52, name: 'Up', category: 'nav' },
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

const CATEGORY_ORDER: Category[] = [
  'all',
  'letter',
  'number',
  'mod',
  'nav',
  'fn',
  'misc',
];

export function KeycodePicker({ onPick }: { onPick: (code: number) => void }) {
  const [query, setQuery] = useState('');
  const [cat, setCat] = useState<Category>('all');

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return KEYCODES.filter((k) => {
      if (cat !== 'all' && k.category !== cat) return false;
      if (!q) return true;
      return (
        k.name.toLowerCase().includes(q) ||
        k.code.toString(16).includes(q)
      );
    });
  }, [query, cat]);

  return (
    <VStack
      align="stretch"
      spacing={3}
      bg="bg.subtle"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="lg"
      p={3}
    >
      <InputGroup size="sm">
        <InputLeftElement pointerEvents="none" color="text.muted">
          <Search size={14} />
        </InputLeftElement>
        <Input
          placeholder="Search keycode or hex…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          variant="filled"
          fontFamily="mono"
          fontSize="xs"
        />
      </InputGroup>

      <HStack spacing={1} flexWrap="wrap">
        {CATEGORY_ORDER.map((c) => {
          const active = cat === c;
          return (
            <Box
              key={c}
              as="button"
              onClick={() => setCat(c)}
              px={2.5}
              py={1}
              borderRadius="sm"
              fontSize="10px"
              fontWeight={500}
              textTransform="uppercase"
              letterSpacing="0.06em"
              fontFamily="mono"
              bg={active ? 'accent.subtle' : 'transparent'}
              color={active ? 'accent.primary' : 'text.muted'}
              border="1px solid"
              borderColor={active ? 'accent.primary' : 'border.subtle'}
              transition="background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease"
              _hover={{
                bg: active ? 'accent.subtle' : 'bg.elevated',
                color: active ? 'accent.primary' : 'text.primary',
              }}
            >
              {CATEGORY_LABELS[c]}
            </Box>
          );
        })}
      </HStack>

      <Box maxH="260px" overflowY="auto" pr={1} mx={-1} px={1}>
        {filtered.length === 0 ? (
          <Flex
            align="center"
            justify="center"
            h="100px"
            color="text.muted"
            fontSize="xs"
            fontFamily="mono"
          >
            no matches
          </Flex>
        ) : (
          <SimpleGrid columns={{ base: 4, md: 6, lg: 8 }} spacing={1.5}>
            {filtered.map((k) => (
              <Button
                key={k.code}
                size="xs"
                variant="subtle"
                onClick={() => onPick(k.code)}
                title={`0x${k.code.toString(16).padStart(2, '0')}`}
                fontFamily="mono"
                fontSize="10px"
                h="28px"
                px={1}
              >
                {k.name}
              </Button>
            ))}
          </SimpleGrid>
        )}
      </Box>

      <Text fontSize="10px" color="text.muted" fontFamily="mono">
        {filtered.length} codes
      </Text>
    </VStack>
  );
}
