import { useEffect, useRef, useState } from 'react';
import { Box, Button, Flex, HStack, Text, Tooltip, useToast, VStack } from '@chakra-ui/react';
import { Bluetooth, Zap } from 'lucide-react';
import { Header } from './components/Header';
import { KeyboardSvg } from './components/KeyboardSvg';
import { KeyDetailPanel } from './components/KeyDetailPanel';
import { DaemonBanner } from './components/DaemonBanner';
import { SyncBanner } from './components/SyncBanner';
import { DipSwitchDisplay } from './components/DipSwitchDisplay';
import { BluetoothPanel } from './components/BluetoothPanel';
import { EmptyState } from './components/EmptyState';
import { EventLog } from './components/EventLog';
import { useDeviceStore } from './store/deviceStore';
import { useDaemonStore } from './store/daemonStore';
import { useProfileStore } from './store/profileStore';
import { useBluetoothStore } from './store/bluetoothStore';
import { useKeyboardThemeStore } from './store/keyboardThemeStore';
import { useKanataStore } from './store/kanataStore';

export default function App() {
  const deviceStatus = useDeviceStore((s) => s.status);
  const loadKeymapsFromProfile = useDeviceStore((s) => s.loadKeymapsFromProfile);
  const checkDaemon = useDaemonStore((s) => s.check);
  const startAutoPoll = useDaemonStore((s) => s.startAutoPoll);
  const stopAutoPoll = useDaemonStore((s) => s.stopAutoPoll);
  const daemonStatus = useDaemonStore((s) => s.status);
  const events = useDaemonStore((s) => s.events);
  const loadProfilesFromDaemon = useProfileStore((s) => s.loadFromDaemon);
  const activeProfile = useProfileStore((s) =>
    s.profiles.find((p) => p.id === s.activeProfileId),
  );
  const btConnected = useBluetoothStore((s) => s.connected);
  const startKanataPoll = useKanataStore((s) => s.startPolling);
  const stopKanataPoll = useKanataStore((s) => s.stopPolling);

  const lastEventCountRef = useRef(0);

  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [layer, setLayer] = useState<'base' | 'fn'>('base');

  // Two ways the editor becomes reachable:
  //  - USB / WebHID: status === 'connected' (live keymap from device)
  //  - BT only: keyboard paired via OS, no USB, but daemon knows the
  //    profile's last-captured raw_layers.
  const isUsbConnected = deviceStatus === 'connected';
  const isBtOnly = !isUsbConnected && btConnected;
  const showEditor =
    daemonStatus === 'online' && (isUsbConnected || isBtOnly);

  // Initial health probe + 30s auto poll.
  useEffect(() => {
    void checkDaemon();
    startAutoPoll();
    return () => stopAutoPoll();
  }, [checkDaemon, startAutoPoll, stopAutoPoll]);

  // When the daemon comes online, hydrate the profile store from it.
  useEffect(() => {
    if (daemonStatus === 'online') {
      void loadProfilesFromDaemon();
    }
  }, [daemonStatus, loadProfilesFromDaemon]);

  // Start/stop kanata polling based on daemon availability.
  useEffect(() => {
    if (daemonStatus === 'online') {
      startKanataPoll();
    } else {
      stopKanataPoll();
    }
    return () => stopKanataPoll();
  }, [daemonStatus, startKanataPoll, stopKanataPoll]);

  // Refresh profiles on `profile_changed` WebSocket events.
  useEffect(() => {
    if (events.length === lastEventCountRef.current) return;
    const newEvents = events.slice(lastEventCountRef.current);
    lastEventCountRef.current = events.length;
    for (const e of newEvents) {
      if (e.type === 'profile_changed') {
        void loadProfilesFromDaemon();
        break;
      }
    }
  }, [events, loadProfilesFromDaemon]);

  // BT-only mode: hydrate the keymap from the active profile so the
  // editor has something to render. USB transports do their own load.
  useEffect(() => {
    if (!isBtOnly) return;
    loadKeymapsFromProfile(activeProfile?.via._roninKB?.hardware?.raw_layers);
  }, [isBtOnly, activeProfile, loadKeymapsFromProfile]);

  return (
    <Flex direction="column" minH="100vh" bg="bg.primary" color="text.primary">
      <Header />
      <DaemonBanner />
      <SyncBanner />

      <Box maxW="1280px" w="100%" mx="auto" px={{ base: 4, md: 6 }} py={6}>
        <Flex
          gap={6}
          direction={{ base: 'column', xl: 'row' }}
          align="stretch"
        >
          {/* ---- Left: keyboard + controls (or empty state) ---- */}
          <Box flex="1 1 auto" minW={0}>
            {showEditor ? (
              <VStack align="stretch" spacing={4}>
                {/* Title + transport badge */}
                <Flex align="center" justify="space-between" pt={1}>
                  <Box>
                    <Text
                      fontSize={{ base: 'sm', md: 'md' }}
                      fontWeight={500}
                      color="text.primary"
                      letterSpacing="-0.005em"
                    >
                      HHKB Professional HYBRID Type-S
                    </Text>
                    <Text fontSize="11px" color="text.muted" fontFamily="mono">
                      US Layout · Non-Printed
                      {isBtOnly && ' · Bluetooth (software bindings only)'}
                    </Text>
                  </Box>
                  <WriteToHHKBButton isUsbConnected={isUsbConnected} />
                </Flex>

                {isBtOnly && (
                  <HStack
                    spacing={2}
                    px={3}
                    py={2}
                    bg="bg.subtle"
                    border="1px solid"
                    borderColor="border.subtle"
                    borderRadius="md"
                  >
                    <Box color="accent.primary" display="flex">
                      <Bluetooth size={12} />
                    </Box>
                    <Text fontSize="11px" color="text.secondary" lineHeight="1.5">
                      連著藍牙 — 可以改軟體層 / kanata 宏。要改硬體 keymap 請接 USB-C。
                    </Text>
                  </HStack>
                )}

                {/* Toolbar */}
                <Flex align="center" justify="space-between" wrap="wrap" gap={2}>
                  {/* Legend */}
                  <HStack spacing={3}>
                    <HStack spacing={1.5}>
                      <Box
                        w="8px" h="8px" borderRadius="full"
                        bg="kanata.fg" flexShrink={0}
                      />
                      <Text fontSize="10px" color="text.muted" fontFamily="mono">
                        SW override
                      </Text>
                    </HStack>
                    <HStack spacing={1.5}>
                      <Box
                        w="8px" h="8px" borderRadius="full"
                        bg="hardware.fg" flexShrink={0}
                      />
                      <Text fontSize="10px" color="text.muted" fontFamily="mono">
                        HW modified
                      </Text>
                    </HStack>
                  </HStack>
                  <HStack spacing={3}>
                    <KeyboardThemeToggle />
                    <LayerTabs layer={layer} onChange={setLayer} />
                  </HStack>
                </Flex>

                <KeyboardSvg
                  layer={layer}
                  selectedIndex={selectedIndex}
                  onSelect={setSelectedIndex}
                />
              </VStack>
            ) : (
              <EmptyState />
            )}
          </Box>

          {/* ---- Right: always visible when daemon is online ---- */}
          <Box w={{ base: '100%', xl: '360px' }} flexShrink={0}>
            <VStack align="stretch" spacing={4}>
              {showEditor && (
                <>
                  <KeyDetailPanel
                    selectedIndex={selectedIndex}
                    layer={layer}
                    isUsbConnected={isUsbConnected}
                  />
                  {isUsbConnected && <DipSwitchDisplay />}
                </>
              )}
              <BluetoothPanel />
            </VStack>
          </Box>
        </Flex>
      </Box>

      <EventLog />
    </Flex>
  );
}

function WriteToHHKBButton({ isUsbConnected }: { isUsbConnected: boolean }) {
  const toast = useToast();
  const writeKeymaps = useDeviceStore((s) => s.writeKeymaps);
  const daemonStatus = useDaemonStore((s) => s.status);
  const [writing, setWriting] = useState(false);
  const disabled = daemonStatus !== 'online' || !isUsbConnected;
  const tooltipLabel = !isUsbConnected
    ? 'Connect USB-C to flash hardware keymap'
    : daemonStatus !== 'online'
      ? 'Daemon required to write EEPROM'
      : 'Flash current keymap to HHKB EEPROM';

  async function handleWrite() {
    setWriting(true);
    try {
      await writeKeymaps();
      toast({
        title: 'Written to HHKB',
        description: 'Keymap flushed to keyboard EEPROM.',
        status: 'success',
        duration: 3000,
      });
    } catch (e) {
      toast({
        title: 'Write failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 5000,
      });
    } finally {
      setWriting(false);
    }
  }

  return (
    <Tooltip
      label={tooltipLabel}
      hasArrow={false}
      placement="bottom-end"
      openDelay={300}
    >
      <Button
        size="sm"
        variant="outline"
        leftIcon={<Zap size={13} />}
        isLoading={writing}
        isDisabled={disabled}
        onClick={() => void handleWrite()}
        fontFamily="mono"
        fontSize="11px"
      >
        Write to HHKB
      </Button>
    </Tooltip>
  );
}

function KeyboardThemeToggle() {
  const theme = useKeyboardThemeStore((s) => s.theme);
  const setTheme = useKeyboardThemeStore((s) => s.setTheme);
  return (
    <HStack
      spacing={0}
      p="3px"
      bg="bg.subtle"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="md"
      role="group"
      aria-label="Keyboard case color"
    >
      {(
        [
          { id: 'charcoal', label: 'Charcoal' },
          { id: 'ivory', label: 'White' },
        ] as const
      ).map((t) => {
        const active = theme === t.id;
        return (
          <Box
            key={t.id}
            as="button"
            px={3}
            py={1}
            borderRadius="sm"
            bg={active ? 'bg.elevated' : 'transparent'}
            color={active ? 'text.primary' : 'text.muted'}
            fontWeight={500}
            fontSize="11px"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.08em"
            border="1px solid"
            borderColor={active ? 'border.subtle' : 'transparent'}
            transition="background-color 0.15s ease, color 0.15s ease, border-color 0.15s ease"
            _hover={{ color: 'text.primary' }}
            onClick={() => setTheme(t.id)}
          >
            {t.label}
          </Box>
        );
      })}
    </HStack>
  );
}

function LayerTabs({
  layer,
  onChange,
}: {
  layer: 'base' | 'fn';
  onChange: (l: 'base' | 'fn') => void;
}) {
  return (
    <HStack
      spacing={0}
      p="3px"
      bg="bg.subtle"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="md"
    >
      {(['base', 'fn'] as const).map((l) => {
        const active = layer === l;
        return (
          <Box
            key={l}
            as="button"
            px={3}
            py={1}
            borderRadius="sm"
            bg={active ? 'bg.elevated' : 'transparent'}
            color={active ? 'text.primary' : 'text.muted'}
            fontWeight={500}
            fontSize="11px"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.08em"
            border="1px solid"
            borderColor={active ? 'border.subtle' : 'transparent'}
            transition="background-color 0.15s ease, color 0.15s ease, border-color 0.15s ease"
            _hover={{
              color: 'text.primary',
            }}
            onClick={() => onChange(l)}
          >
            {l === 'base' ? 'Base' : 'Fn'}
          </Box>
        );
      })}
    </HStack>
  );
}
