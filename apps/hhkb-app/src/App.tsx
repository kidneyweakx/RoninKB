import { useEffect, useRef, useState } from 'react';
import { Box, Flex, VStack } from '@chakra-ui/react';
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
  const device = useDeviceStore((s) => s.device);
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
    <Flex direction="column" minH="100vh" bg="gray.900" color="white">
      <Header />
      <DaemonBanner />

      <Flex flex="1" p={6} gap={6}>
        {device ? (
          <>
            <Box flex="2">
              <VStack align="stretch" spacing={4}>
                <LayerTabs layer={layer} onChange={setLayer} />
                <KeyboardSvg
                  layer={layer}
                  selectedIndex={selectedIndex}
                  onSelect={setSelectedIndex}
                />
              </VStack>
            </Box>
            <Box flex="1" minW="320px">
              <KeyDetailPanel
                selectedIndex={selectedIndex}
                layer={layer}
              />
            </Box>
          </>
        ) : (
          <EmptyState />
        )}
      </Flex>

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
    <Flex gap={2}>
      {(['base', 'fn'] as const).map((l) => (
        <Box
          key={l}
          as="button"
          px={4}
          py={2}
          borderRadius="md"
          bg={layer === l ? 'brand.500' : 'gray.700'}
          color="white"
          fontWeight="semibold"
          onClick={() => onChange(l)}
        >
          {l === 'base' ? 'Base Layer' : 'Fn Layer'}
        </Box>
      ))}
    </Flex>
  );
}
