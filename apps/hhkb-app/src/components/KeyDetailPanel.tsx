import {
  Box,
  Heading,
  Text,
  VStack,
  HStack,
  Divider,
  Button,
  Flex,
  Tag,
} from '@chakra-ui/react';
import { useState } from 'react';
import {
  Keyboard as KeyboardIcon,
  Clock,
  List,
  Pencil,
  X,
  MousePointerClick,
  Cpu,
} from 'lucide-react';
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
      <Panel>
        <Flex
          direction="column"
          align="center"
          justify="center"
          h="100%"
          color="text.muted"
          textAlign="center"
          py={12}
          gap={3}
        >
          <Box
            w="56px"
            h="56px"
            borderRadius="lg"
            bg="bg.subtle"
            border="1px solid"
            borderColor="border.subtle"
            display="flex"
            alignItems="center"
            justifyContent="center"
            color="text.muted"
          >
            <MousePointerClick size={24} />
          </Box>
          <VStack spacing={1}>
            <Text fontSize="sm" fontWeight={500} color="text.secondary">
              No key selected
            </Text>
            <Text fontSize="xs" color="text.muted" maxW="220px">
              Click any key on the keyboard to inspect and remap it.
            </Text>
          </VStack>
        </Flex>
      </Panel>
    );
  }

  const meta = HHKB_LAYOUT.find((k) => k.index === selectedIndex);
  const keymap = layer === 'base' ? baseKeymap : fnKeymap;
  const currentCode = keymap?.get(selectedIndex) ?? 0;
  const isDefault = currentCode === 0;
  const hexCode = `0x${currentCode.toString(16).padStart(2, '0').toUpperCase()}`;

  return (
    <Panel>
      <VStack align="stretch" spacing={0} h="100%">
        {/* Header — key identity */}
        <Box px={5} pt={5} pb={4}>
          <HStack spacing={2} mb={3}>
            <Tag size="sm" variant="subtle">
              <HStack spacing={1}>
                <Cpu size={10} />
                <Text fontSize="10px" fontFamily="mono">
                  硬體
                </Text>
              </HStack>
            </Tag>
            <Tag size="sm" variant={layer === 'fn' ? 'accent' : 'subtle'}>
              <Text
                fontSize="10px"
                fontFamily="mono"
                textTransform="uppercase"
                letterSpacing="0.06em"
              >
                {layer} layer
              </Text>
            </Tag>
            <Tag size="sm" variant="subtle">
              <Text fontSize="10px" fontFamily="mono">
                #{selectedIndex}
              </Text>
            </Tag>
          </HStack>

          <Flex align="center" justify="space-between" gap={3}>
            <Box>
              <Text
                fontSize="10px"
                color="text.muted"
                fontFamily="mono"
                textTransform="uppercase"
                letterSpacing="0.08em"
                mb={1}
              >
                Key
              </Text>
              <Heading
                size="lg"
                fontFamily="mono"
                letterSpacing="-0.02em"
                lineHeight="1"
              >
                {meta?.label ?? `#${selectedIndex}`}
              </Heading>
            </Box>
            <Box textAlign="right">
              <Text
                fontSize="10px"
                color="text.muted"
                fontFamily="mono"
                textTransform="uppercase"
                letterSpacing="0.08em"
                mb={1}
              >
                HID
              </Text>
              <Text
                fontFamily="mono"
                fontSize="lg"
                fontWeight={500}
                color={isDefault ? 'text.muted' : 'accent.primary'}
                lineHeight="1"
              >
                {isDefault ? '—' : hexCode}
              </Text>
            </Box>
          </Flex>
        </Box>

        <Divider />

        {/* Binding type — currently only keyboard */}
        <Box px={5} py={4}>
          <Text
            fontSize="10px"
            color="text.muted"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.08em"
            mb={2}
          >
            Binding
          </Text>
          <VStack align="stretch" spacing={1.5}>
            <BindingRow
              icon={<KeyboardIcon size={14} />}
              label="Keyboard"
              value={isDefault ? 'default' : hexCode}
              active
            />
            <BindingRow
              icon={<Clock size={14} />}
              label="Tap-Hold"
              value="not set"
              disabled
            />
            <BindingRow
              icon={<List size={14} />}
              label="Macro"
              value="not set"
              disabled
            />
          </VStack>
        </Box>

        <Divider />

        {/* Edit action */}
        <Box px={5} py={4}>
          <HStack spacing={2}>
            <Button
              leftIcon={
                picking ? <X size={14} /> : <Pencil size={14} />
              }
              variant={picking ? 'outline' : 'solid'}
              size="sm"
              onClick={() => setPicking((v) => !v)}
              isDisabled={!keymap}
              flex="1"
            >
              {picking ? 'Cancel' : 'Remap key'}
            </Button>
            {!isDefault && !picking && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setKeyOverride(selectedIndex, 0, layer)}
              >
                Reset
              </Button>
            )}
          </HStack>

          {picking && (
            <Box mt={3}>
              <KeycodePicker
                onPick={(code) => {
                  setKeyOverride(selectedIndex, code, layer);
                  setPicking(false);
                }}
              />
            </Box>
          )}
        </Box>

        <Box flex="1" />

        <Box px={5} pb={4}>
          <Text
            fontSize="10px"
            color="text.muted"
            fontFamily="mono"
            lineHeight="1.6"
          >
            Key index derived from happy-hacking-gnu protocol reverse
            engineering. See src/data/hhkbLayout.ts
          </Text>
        </Box>
      </VStack>
    </Panel>
  );
}

function Panel({ children }: { children: React.ReactNode }) {
  return (
    <Box
      bg="bg.surface"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="xl"
      h="100%"
      overflow="hidden"
    >
      {children}
    </Box>
  );
}

function BindingRow({
  icon,
  label,
  value,
  active = false,
  disabled = false,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  active?: boolean;
  disabled?: boolean;
}) {
  return (
    <HStack
      justify="space-between"
      px={3}
      py={2}
      borderRadius="md"
      bg={active ? 'accent.subtle' : 'bg.subtle'}
      border="1px solid"
      borderColor={active ? 'accent.primary' : 'border.subtle'}
      opacity={disabled ? 0.5 : 1}
      transition="background-color 0.15s ease, border-color 0.15s ease"
    >
      <HStack spacing={2}>
        <Box color={active ? 'accent.primary' : 'text.muted'} display="flex">
          {icon}
        </Box>
        <Text fontSize="xs" color="text.primary" fontWeight={500}>
          {label}
        </Text>
      </HStack>
      <Text
        fontSize="xs"
        color={active ? 'accent.primary' : 'text.muted'}
        fontFamily="mono"
      >
        {value}
      </Text>
    </HStack>
  );
}
