import { useEffect, useRef, useState } from 'react';
import { Box, Flex, HStack, Text, VStack } from '@chakra-ui/react';
import { Layers } from 'lucide-react';
import { Header } from './components/Header';
import { KeyboardSvg } from './components/KeyboardSvg';
import { KeyDetailPanel } from './components/KeyDetailPanel';
import { DaemonBanner } from './components/DaemonBanner';
import { EmptyState } from './components/EmptyState';
import { EventLog } from './components/EventLog';
import { useDeviceStore } from './store/deviceStore';
import { useDaemonStore } from './store/daemonStore';
import { useProfileStore } from './store/profileStore';

export default function App() {
  const deviceStatus = useDeviceStore((s) => s.status);
  const checkDaemon = useDaemonStore((s) => s.check);
  const startAutoPoll = useDaemonStore((s) => s.startAutoPoll);
  const stopAutoPoll = useDaemonStore((s) => s.stopAutoPoll);
  const daemonStatus = useDaemonStore((s) => s.status);
  const events = useDaemonStore((s) => s.events);
  const loadProfilesFromDaemon = useProfileStore((s) => s.loadFromDaemon);

  const lastEventCountRef = useRef(0);

  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [layer, setLayer] = useState<'base' | 'fn'>('base');

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

  return (
    <Flex direction="column" minH="100vh" bg="bg.primary" color="text.primary">
      <Header />
      <DaemonBanner />

      <Box maxW="1280px" w="100%" mx="auto" px={{ base: 4, md: 6 }} py={6}>
        {deviceStatus === 'connected' ? (
          <Flex
            gap={6}
            direction={{ base: 'column', xl: 'row' }}
            align="stretch"
          >
            <Box flex="1 1 auto" minW={0}>
              <VStack align="stretch" spacing={4}>
                <Flex align="center" justify="space-between">
                  <HStack spacing={2}>
                    <Box color="text.muted" display="flex">
                      <Layers size={14} />
                    </Box>
                    <Text
                      fontSize="10px"
                      color="text.muted"
                      fontFamily="mono"
                      textTransform="uppercase"
                      letterSpacing="0.08em"
                    >
                      Layer
                    </Text>
                  </HStack>
                  <LayerTabs layer={layer} onChange={setLayer} />
                </Flex>
                <KeyboardSvg
                  layer={layer}
                  selectedIndex={selectedIndex}
                  onSelect={setSelectedIndex}
                />
              </VStack>
            </Box>
            <Box w={{ base: '100%', xl: '360px' }} flexShrink={0}>
              <KeyDetailPanel
                selectedIndex={selectedIndex}
                layer={layer}
              />
            </Box>
          </Flex>
        ) : (
          <EmptyState />
        )}
      </Box>

      <EventLog />
    </Flex>
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
