/**
 * BluetoothPanel — shows BLE status, scan, and device management.
 *
 * Placed in the right sidebar below DipSwitchDisplay.
 * BLE is system-managed. RoninKB only shows status and nearby HHKB devices.
 */

import { useEffect } from 'react';
import {
  Box,
  HStack,
  VStack,
  Text,
  IconButton,
  Spinner,
  Tooltip,
  Badge,
} from '@chakra-ui/react';
import { Bluetooth, BluetoothOff, RefreshCw } from 'lucide-react';
import { useBluetoothStore } from '../store/bluetoothStore';
import { useDaemonStore } from '../store/daemonStore';

export function BluetoothPanel() {
  const daemonStatus = useDaemonStore((s) => s.status);
  const fetch = useBluetoothStore((s) => s.fetch);
  const startScan = useBluetoothStore((s) => s.startScan);

  const available = useBluetoothStore((s) => s.available);
  const connected = useBluetoothStore((s) => s.connected);
  const name = useBluetoothStore((s) => s.name);
  const address = useBluetoothStore((s) => s.address);
  const battery = useBluetoothStore((s) => s.battery);
  const rssi = useBluetoothStore((s) => s.rssi);
  const scanning = useBluetoothStore((s) => s.scanning);
  const devices = useBluetoothStore((s) => s.devices);

  useEffect(() => {
    if (daemonStatus === 'online') void fetch();
  }, [daemonStatus, fetch]);

  const isOnline = daemonStatus === 'online';

  return (
    <VStack
      align="stretch"
      spacing={3}
      bg="bg.subtle"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="lg"
      p={4}
    >
      {/* Header */}
      <HStack justify="space-between" align="center">
        <HStack spacing={2}>
          <Box color={available && isOnline ? 'accent.primary' : 'text.muted'}>
            {available && isOnline ? <Bluetooth size={13} /> : <BluetoothOff size={13} />}
          </Box>
          <Text
            fontSize="10px"
            color="text.muted"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.08em"
          >
            Bluetooth
          </Text>
        </HStack>

        {isOnline && available && (
          <Tooltip label={scanning ? 'Scanning…' : 'Scan for HHKB devices'}>
            <IconButton
              aria-label="scan"
              icon={scanning ? <Spinner size="xs" /> : <RefreshCw size={11} />}
              size="xs"
              variant="ghost"
              colorScheme="gray"
              isDisabled={scanning}
              onClick={() => void startScan()}
            />
          </Tooltip>
        )}
      </HStack>

      {/* Adapter unavailable */}
      {(!isOnline || !available) && (
        <Box
          bg="rgba(255,255,255,0.02)"
          border="1px dashed"
          borderColor="border.subtle"
          borderRadius="md"
          py={4}
          textAlign="center"
        >
          <Text fontSize="xs" color="text.muted">
            {!isOnline
              ? 'Daemon offline'
              : 'No Bluetooth adapter detected'}
          </Text>
        </Box>
      )}

      {/* Connected device card */}
      {isOnline && available && connected && (
        <Box
          bg="bg.elevated"
          border="1px solid"
          borderColor="accent.primary"
          borderRadius="md"
          p={3}
        >
          <HStack justify="space-between" align="flex-start">
            <VStack align="flex-start" spacing={0.5}>
              <Text fontSize="xs" fontWeight={600} color="text.primary" isTruncated maxW="180px">
                {name ?? 'HHKB Hybrid'}
              </Text>
              <Text fontSize="10px" color="text.muted" fontFamily="mono">
                {address && address !== '00:00:00:00:00:00' ? address : 'BLE'}
              </Text>
              <Badge variant="subtle" colorScheme="green" mt={1}>
                System managed
              </Badge>
              <HStack spacing={3} mt={1}>
                {battery !== null && (
                  <HStack spacing={1}>
                    <BatteryBar level={battery} />
                    <Text fontSize="10px" color="text.muted" fontFamily="mono">
                      {battery}%
                    </Text>
                  </HStack>
                )}
                {rssi !== null && (
                  <Text fontSize="10px" color="text.muted" fontFamily="mono">
                    {rssi} dBm
                  </Text>
                )}
              </HStack>
            </VStack>
          </HStack>
        </Box>
      )}

      {/* Scan results */}
      {isOnline && available && devices.length > 0 && (
        <VStack align="stretch" spacing={1}>
          <Text fontSize="10px" color="text.muted" fontFamily="mono" textTransform="uppercase" letterSpacing="0.08em">
            {devices.length} device{devices.length !== 1 ? 's' : ''} found
          </Text>
          {devices.map((d) => {
            const isHhkb = d.name?.startsWith('HHKB') ?? false;
            // On macOS addresses are always 00:00:00:00:00:00 — use UUID
            const displayName = d.name
              ?? (d.connected ? 'Unknown (connected)' : 'Unknown');
            const subLabel = d.rssi != null ? `${d.rssi} dBm` : '';
            // Dim nameless, unconnected, non-HHKB devices
            const dimmed = !d.name && !d.connected;
            return (
              <HStack
                key={d.id}
                justify="space-between"
                bg={isHhkb || d.connected ? 'bg.elevated' : 'bg.subtle'}
                border="1px solid"
                borderColor={isHhkb ? 'accent.primary' : d.connected ? 'border.muted' : 'border.subtle'}
                borderRadius="md"
                p={2}
                opacity={dimmed ? 0.45 : 1}
              >
                <VStack align="flex-start" spacing={0}>
                  <HStack spacing={1.5}>
                    <Text fontSize="xs" color={isHhkb ? 'text.primary' : 'text.secondary'}>
                      {displayName}
                    </Text>
                    {d.connected && (
                      <Text fontSize="9px" color="accent.primary" fontFamily="mono" textTransform="uppercase">
                        OS
                      </Text>
                    )}
                  </HStack>
                  <Text fontSize="10px" color="text.muted" fontFamily="mono">{subLabel}</Text>
                </VStack>
                <Text fontSize="10px" color="text.muted" fontFamily="mono">
                  {d.connected ? 'Connected in system settings' : 'Pair in system settings'}
                </Text>
              </HStack>
            );
          })}
        </VStack>
      )}

      {/* Empty scan result */}
      {isOnline && available && !connected && !scanning && devices.length === 0 && (
        <Text fontSize="xs" color="text.muted" textAlign="center">
          Pair HHKB in system Bluetooth settings, then press scan
        </Text>
      )}

      {/* Info note */}
      {isOnline && available && (
        <Text fontSize="10px" color="text.muted" fontFamily="mono" lineHeight="1.5">
          Pair and switch Bluetooth profiles in the OS · USB required for keymap config
        </Text>
      )}
    </VStack>
  );
}

// Simple battery bar: 5 segments
function BatteryBar({ level }: { level: number }) {
  const filled = Math.round((level / 100) * 5);
  const color = level > 20 ? 'accent.primary' : 'red.400';
  return (
    <HStack spacing="2px">
      {Array.from({ length: 5 }).map((_, i) => (
        <Box
          key={i}
          w="4px"
          h="8px"
          borderRadius="1px"
          bg={i < filled ? color : 'border.subtle'}
        />
      ))}
    </HStack>
  );
}
