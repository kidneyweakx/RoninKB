import { useEffect, useState } from 'react';
import {
  Box,
  VStack,
  HStack,
  Text,
  Button,
  Badge,
  Tooltip,
  Spinner,
  Divider,
} from '@chakra-ui/react';
import { useToast } from '@chakra-ui/react';
import { Usb, Zap } from 'lucide-react';
import { useDaemonStore } from '../store/daemonStore';
import { useDeviceStore } from '../store/deviceStore';
import { KeyboardMode, keyboardModeLabel } from '../hhkb/types';

// ---------------------------------------------------------------------------
// DIP switch metadata
// ---------------------------------------------------------------------------

const DIP_HINTS: Record<number, string> = {
  0: 'SW1 — Bluetooth profile select (bit 0)',
  1: 'SW2 — Bluetooth profile select (bit 1)',
  2: 'SW3 — Layout: OFF = Mac / ON = Win',
  3: 'SW4 — Delete key: OFF = BS / ON = Del',
  4: 'SW5 — Extra function 1',
  5: 'SW6 — Extra function 2',
};

// ---------------------------------------------------------------------------
// DIP Switch widget
// ---------------------------------------------------------------------------

interface DipSwitchDto {
  switches: boolean[];
}

function DipSwitchWidget() {
  const daemonStatus = useDaemonStore((s) => s.status);
  const client = useDaemonStore((s) => s.client);

  const [loading, setLoading] = useState(false);
  const [switches, setSwitches] = useState<boolean[] | null>(null);

  useEffect(() => {
    if (!client) {
      setSwitches(null);
      return;
    }
    setLoading(true);
    client
      .deviceDipsw()
      .then((raw) => {
        const dto = raw as DipSwitchDto;
        if (Array.isArray(dto?.switches)) {
          setSwitches(dto.switches.slice(0, 6));
        }
      })
      .catch(() => {
        setSwitches(null);
      })
      .finally(() => setLoading(false));
  }, [client]);

  return (
    <Box>
      <Text
        fontSize="10px"
        color="text.muted"
        fontFamily="mono"
        textTransform="uppercase"
        letterSpacing="0.08em"
        mb={2}
      >
        DIP Switches
      </Text>

      {/* Dark rounded rectangle mimicking HHKB DIP switch bank */}
      <Box
        bg="gray.900"
        borderRadius="lg"
        p={3}
        border="1px solid"
        borderColor="gray.700"
        display="inline-flex"
        alignItems="flex-end"
        gap={2}
      >
        {loading ? (
          <HStack spacing={2} px={2} py={1}>
            {Array.from({ length: 6 }).map((_, i) => (
              <Box
                key={i}
                w="22px"
                h="44px"
                bg="gray.700"
                borderRadius="sm"
                animation="pulse 1.5s ease-in-out infinite"
              />
            ))}
          </HStack>
        ) : daemonStatus !== 'online' || switches === null ? (
          <HStack spacing={2} px={2} py={1}>
            {Array.from({ length: 6 }).map((_, i) => (
              <Box key={i} textAlign="center">
                <Box
                  w="22px"
                  h="44px"
                  bg="gray.700"
                  borderRadius="sm"
                  display="flex"
                  alignItems="center"
                  justifyContent="center"
                >
                  <Text fontSize="8px" color="gray.500" fontFamily="mono">
                    N/A
                  </Text>
                </Box>
                <Text fontSize="8px" color="gray.600" fontFamily="mono" mt={1}>
                  SW{i + 1}
                </Text>
              </Box>
            ))}
          </HStack>
        ) : (
          <HStack spacing={2} px={1} py={1}>
            {switches.map((on, i) => (
              <Tooltip
                key={i}
                label={DIP_HINTS[i] ?? `SW${i + 1}`}
                placement="top"
                hasArrow
                fontSize="xs"
              >
                <Box textAlign="center" cursor="default">
                  {/* Slider track */}
                  <Box
                    w="22px"
                    h="44px"
                    bg="gray.800"
                    borderRadius="sm"
                    border="1px solid"
                    borderColor={on ? 'accent.primary' : 'gray.600'}
                    position="relative"
                    overflow="hidden"
                    transition="border-color 0.2s"
                  >
                    {/* Slider knob — sits at top when ON, bottom when OFF */}
                    <Box
                      position="absolute"
                      left="2px"
                      right="2px"
                      h="18px"
                      bg={on ? 'accent.primary' : 'gray.600'}
                      borderRadius="xs"
                      top={on ? '3px' : undefined}
                      bottom={on ? undefined : '3px'}
                      transition="top 0.2s, bottom 0.2s, background-color 0.2s"
                    />
                  </Box>
                  <Text
                    fontSize="8px"
                    color={on ? 'accent.primary' : 'gray.500'}
                    fontFamily="mono"
                    mt={1}
                    transition="color 0.2s"
                  >
                    SW{i + 1}
                  </Text>
                </Box>
              </Tooltip>
            ))}
          </HStack>
        )}
      </Box>
    </Box>
  );
}

// ---------------------------------------------------------------------------
// Mode selector
// ---------------------------------------------------------------------------

const MODES = [KeyboardMode.HHK, KeyboardMode.Mac, KeyboardMode.Lite] as const;

function ModeSelector() {
  const daemonStatus = useDaemonStore((s) => s.status);
  const client = useDaemonStore((s) => s.client);
  const mode = useDeviceStore((s) => s.mode);
  const [pending, setPending] = useState<KeyboardMode | null>(null);
  const toast = useToast();

  const disabled = daemonStatus !== 'online' || !client;

  async function handleSelect(m: KeyboardMode) {
    if (!client || m === mode) return;
    setPending(m);
    try {
      await client.setMode(keyboardModeLabel(m).toLowerCase());
    } catch (e) {
      toast({
        title: 'Failed to set mode',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setPending(null);
    }
  }

  return (
    <Box>
      <Text
        fontSize="10px"
        color="text.muted"
        fontFamily="mono"
        textTransform="uppercase"
        letterSpacing="0.08em"
        mb={2}
      >
        Keyboard Mode
      </Text>
      <HStack spacing={1.5}>
        {MODES.map((m) => {
          const active = mode === m;
          const loading = pending === m;
          return (
            <Button
              key={m}
              size="sm"
              variant={active ? 'solid' : 'outline'}
              colorScheme={active ? 'purple' : undefined}
              isDisabled={disabled || (pending !== null && !loading)}
              isLoading={loading}
              onClick={() => void handleSelect(m)}
              fontFamily="mono"
              fontSize="xs"
              px={4}
              h="28px"
              borderRadius="full"
            >
              {keyboardModeLabel(m)}
            </Button>
          );
        })}
      </HStack>
    </Box>
  );
}

// ---------------------------------------------------------------------------
// Bluetooth info row
// ---------------------------------------------------------------------------

function BluetoothInfo() {
  const deviceConnected = useDaemonStore((s) => s.deviceConnected);
  const daemonStatus = useDaemonStore((s) => s.status);

  return (
    <Box>
      <Text
        fontSize="10px"
        color="text.muted"
        fontFamily="mono"
        textTransform="uppercase"
        letterSpacing="0.08em"
        mb={2}
      >
        Connection
      </Text>
      <HStack spacing={2} align="center">
        <Box color="text.muted" display="flex">
          <Usb size={14} />
        </Box>
        <Box flex="1" minW={0}>
          <Text fontSize="xs" color="text.secondary">
            HHKB Hybrid: USB = config + typing | BT = typing only
          </Text>
        </Box>
        {daemonStatus === 'online' ? (
          <Badge
            colorScheme={deviceConnected ? 'green' : 'gray'}
            variant="subtle"
            fontSize="10px"
            px={2}
            py={0.5}
            borderRadius="full"
            flexShrink={0}
          >
            {deviceConnected ? 'USB connected' : 'No USB device'}
          </Badge>
        ) : (
          <Badge
            colorScheme="gray"
            variant="subtle"
            fontSize="10px"
            px={2}
            py={0.5}
            borderRadius="full"
            flexShrink={0}
          >
            Daemon offline
          </Badge>
        )}
      </HStack>
    </Box>
  );
}

// ---------------------------------------------------------------------------
// Flash keymap button
// ---------------------------------------------------------------------------

function FlashKeymapButton() {
  const writeKeymaps = useDeviceStore((s) => s.writeKeymaps);
  const client = useDaemonStore((s) => s.client);
  const daemonStatus = useDaemonStore((s) => s.status);

  const [writing, setWriting] = useState(false);
  const toast = useToast();

  const disabled = daemonStatus !== 'online' || !client;

  async function handleFlash() {
    setWriting(true);
    try {
      await writeKeymaps();
      toast({
        title: 'Keymap flashed',
        description: 'Keymap successfully written to keyboard.',
        status: 'success',
        duration: 3000,
        isClosable: true,
      });
    } catch (e) {
      toast({
        title: 'Flash failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setWriting(false);
    }
  }

  return (
    <Button
      colorScheme="purple"
      size="sm"
      isDisabled={disabled}
      isLoading={writing}
      loadingText="Writing…"
      leftIcon={writing ? <Spinner size="xs" /> : <Zap size={14} />}
      onClick={() => void handleFlash()}
    >
      Flash keymap to keyboard
    </Button>
  );
}

// ---------------------------------------------------------------------------
// DeviceStatusPanel
// ---------------------------------------------------------------------------

export function DeviceStatusPanel() {
  return (
    <Box
      bg="bg.surface"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="xl"
      p={5}
    >
      <Text
        fontSize="10px"
        color="text.muted"
        fontFamily="mono"
        textTransform="uppercase"
        letterSpacing="0.08em"
        mb={4}
      >
        Device Status
      </Text>

      <VStack align="stretch" spacing={4}>
        <DipSwitchWidget />

        <Divider />

        <ModeSelector />

        <Divider />

        <BluetoothInfo />

        <Divider />

        <Box>
          <FlashKeymapButton />
        </Box>
      </VStack>
    </Box>
  );
}
