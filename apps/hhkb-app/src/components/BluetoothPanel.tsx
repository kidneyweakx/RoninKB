/**
 * BluetoothPanel — unified Connection card.
 *
 * Despite the legacy filename, this component renders the full connection
 * surface for the right sidebar:
 *   - Daemon section (version, status, last poll)
 *   - USB / WebHID section (connected device, transport)
 *   - Bluetooth section (status, connected HHKB, OS-managed list, scan results)
 *
 * Sections share one outer card and are separated by Dividers (no nested
 * borders) so the whole connection story reads as a single object.
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
  Divider,
} from '@chakra-ui/react';
import {
  Bluetooth,
  BluetoothOff,
  RefreshCw,
  Wifi,
  WifiOff,
  AlertCircle,
  Usb,
  Plug,
} from 'lucide-react';
import { useBluetoothStore } from '../store/bluetoothStore';
import { useDaemonStore } from '../store/daemonStore';
import { useDeviceStore } from '../store/deviceStore';

export function BluetoothPanel() {
  const daemonStatus = useDaemonStore((s) => s.status);
  const daemonVersion = useDaemonStore((s) => s.version);
  const lastCheckedAt = useDaemonStore((s) => s.lastCheckedAt);

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
  const systemConnectedDevices = useBluetoothStore((s) => s.systemConnectedDevices);
  const systemSource = useBluetoothStore((s) => s.systemSource);
  const systemMessage = useBluetoothStore((s) => s.systemMessage);

  const deviceStatus = useDeviceStore((s) => s.status);
  const deviceInfo = useDeviceStore((s) => s.info);
  const transport = useDeviceStore((s) => s.transportMode)();
  const usbDeviceLabel = deviceInfo?.typeNumber
    ? `HHKB ${deviceInfo.typeNumber}`
    : null;

  useEffect(() => {
    if (daemonStatus === 'online') void fetch();
  }, [daemonStatus, fetch]);

  const isOnline = daemonStatus === 'online';
  const usbConnected = deviceStatus === 'connected';

  // Overall status dot:
  //   green — any active hardware connection (USB or BT)
  //   amber — daemon online but no device on the wire
  //   red   — daemon offline / unknown
  let overallColor: 'success' | 'warning' | 'danger' = 'danger';
  let overallLabel = 'Disconnected';
  if (isOnline) {
    if (usbConnected || connected) {
      overallColor = 'success';
      overallLabel = 'Active';
    } else {
      overallColor = 'warning';
      overallLabel = 'Idle';
    }
  } else if (daemonStatus === 'unknown') {
    overallColor = 'warning';
    overallLabel = 'Checking';
  }

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
      {/* Card header */}
      <HStack justify="space-between" align="center">
        <HStack spacing={2}>
          <Box
            w="8px"
            h="8px"
            borderRadius="full"
            bg={overallColor}
            flexShrink={0}
          />
          <Text
            fontSize="10px"
            color="text.muted"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.08em"
          >
            Connection
          </Text>
        </HStack>
        <Text
          fontSize="10px"
          color="text.muted"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.08em"
        >
          {overallLabel}
        </Text>
      </HStack>

      {/* ── Daemon section ─────────────────────────────────────────── */}
      <DaemonSection
        status={daemonStatus}
        version={daemonVersion}
        lastCheckedAt={lastCheckedAt}
      />

      <Divider borderColor="border.subtle" />

      {/* ── USB / WebHID section ───────────────────────────────────── */}
      <UsbSection
        connected={usbConnected}
        transport={transport}
        deviceName={usbDeviceLabel}
        daemonOnline={isOnline}
      />

      <Divider borderColor="border.subtle" />

      {/* ── Bluetooth section ──────────────────────────────────────── */}
      <BluetoothSection
        isOnline={isOnline}
        available={available}
        connected={connected}
        name={name}
        address={address}
        battery={battery}
        rssi={rssi}
        scanning={scanning}
        devices={devices}
        systemConnectedDevices={systemConnectedDevices}
        systemSource={systemSource}
        systemMessage={systemMessage}
        onScan={() => void startScan()}
      />

      {/* Footer note */}
      {isOnline && (
        <Text
          fontSize="10px"
          color="text.muted"
          fontFamily="mono"
          lineHeight="1.5"
          pt={1}
        >
          Pair and switch Bluetooth profiles in the OS · USB required for keymap config
        </Text>
      )}
    </VStack>
  );
}

// ─── Sub-sections ────────────────────────────────────────────────────────────

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <Text
      fontSize="10px"
      color="text.muted"
      fontFamily="mono"
      textTransform="uppercase"
      letterSpacing="0.08em"
    >
      {children}
    </Text>
  );
}

function DaemonSection({
  status,
  version,
  lastCheckedAt,
}: {
  status: 'unknown' | 'online' | 'offline';
  version: string | null;
  lastCheckedAt: number | null;
}) {
  const meta =
    status === 'online'
      ? { Icon: Wifi, color: 'success', label: 'Online' }
      : status === 'offline'
        ? { Icon: WifiOff, color: 'text.muted', label: 'Offline' }
        : { Icon: AlertCircle, color: 'warning', label: 'Checking' };
  const Icon = meta.Icon;

  const lastPoll = lastCheckedAt ? formatRelative(lastCheckedAt) : null;

  return (
    <VStack align="stretch" spacing={1.5}>
      <HStack justify="space-between">
        <HStack spacing={2}>
          <Box color={meta.color} display="flex">
            <Icon size={12} strokeWidth={2.25} />
          </Box>
          <SectionLabel>Daemon</SectionLabel>
        </HStack>
        <Text fontSize="10px" color="text.muted" fontFamily="mono">
          {version ? `v${version}` : '—'}
        </Text>
      </HStack>
      <HStack justify="space-between" pl={5}>
        <Text fontSize="xs" color="text.secondary">
          {meta.label}
        </Text>
        {lastPoll && (
          <Text fontSize="10px" color="text.muted" fontFamily="mono">
            polled {lastPoll}
          </Text>
        )}
      </HStack>
    </VStack>
  );
}

function UsbSection({
  connected,
  transport,
  deviceName,
  daemonOnline,
}: {
  connected: boolean;
  transport: 'webhid' | 'daemon' | 'none';
  deviceName: string | null;
  daemonOnline: boolean;
}) {
  const Icon = connected ? Usb : Plug;
  const transportLabel =
    transport === 'webhid'
      ? 'WebHID'
      : transport === 'daemon'
        ? 'Daemon HID'
        : daemonOnline
          ? 'No transport'
          : 'Daemon offline';
  const tone = connected ? 'success' : 'text.muted';

  return (
    <VStack align="stretch" spacing={1.5}>
      <HStack justify="space-between">
        <HStack spacing={2}>
          <Box color={tone} display="flex">
            <Icon size={12} strokeWidth={2.25} />
          </Box>
          <SectionLabel>USB / Wired</SectionLabel>
        </HStack>
        <Text fontSize="10px" color="text.muted" fontFamily="mono">
          {transportLabel}
        </Text>
      </HStack>
      <HStack justify="space-between" pl={5}>
        <Text fontSize="xs" color={connected ? 'text.primary' : 'text.muted'} noOfLines={1}>
          {connected
            ? (deviceName ?? 'HHKB Pro Hybrid Type-S')
            : 'Not connected'}
        </Text>
      </HStack>
    </VStack>
  );
}

interface BluetoothSectionProps {
  isOnline: boolean;
  available: boolean;
  connected: boolean;
  name: string | null;
  address: string | null;
  battery: number | null;
  rssi: number | null;
  scanning: boolean;
  devices: ReturnType<typeof useBluetoothStore.getState>['devices'];
  systemConnectedDevices: ReturnType<typeof useBluetoothStore.getState>['systemConnectedDevices'];
  systemSource: string | null;
  systemMessage: string | null;
  onScan: () => void;
}

function BluetoothSection(props: BluetoothSectionProps) {
  const {
    isOnline,
    available,
    connected,
    name,
    address,
    battery,
    rssi,
    scanning,
    devices,
    systemConnectedDevices,
    systemSource,
    systemMessage,
    onScan,
  } = props;

  const btReady = isOnline && available;
  const wirelessTone = btReady ? 'wireless.fg' : 'text.muted';

  return (
    <VStack align="stretch" spacing={2}>
      <HStack justify="space-between" align="center">
        <HStack spacing={2}>
          <Box color={wirelessTone} display="flex">
            {btReady ? <Bluetooth size={12} strokeWidth={2.25} /> : <BluetoothOff size={12} strokeWidth={2.25} />}
          </Box>
          <SectionLabel>Bluetooth</SectionLabel>
        </HStack>

        {btReady && (
          <Tooltip label={scanning ? 'Scanning…' : 'Scan for HHKB devices'}>
            <IconButton
              aria-label="scan"
              icon={scanning ? <Spinner size="xs" /> : <RefreshCw size={11} />}
              size="xs"
              variant="ghost"
              colorScheme="gray"
              isDisabled={scanning}
              onClick={onScan}
            />
          </Tooltip>
        )}
      </HStack>

      {/* Adapter unavailable */}
      {!btReady && (
        <Text fontSize="xs" color="text.muted" pl={5}>
          {!isOnline ? 'Daemon offline' : 'No Bluetooth adapter detected'}
        </Text>
      )}

      {/* Connected device — flat block, subtle bg shift, no colored border */}
      {btReady && connected && (
        <Box
          bg="wireless.subtle"
          borderRadius="md"
          px={3}
          py={2}
        >
          <VStack align="flex-start" spacing={0.5}>
            <HStack spacing={2} w="100%">
              <Text fontSize="xs" fontWeight={600} color="text.primary" isTruncated maxW="180px">
                {name ?? 'HHKB Hybrid'}
              </Text>
              <Badge variant="subtle" colorScheme="blue" fontSize="9px">
                System
              </Badge>
            </HStack>
            <Text fontSize="10px" color="text.muted" fontFamily="mono">
              {address && address !== '00:00:00:00:00:00' ? address : 'BLE'}
            </Text>
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
        </Box>
      )}

      {/* OS-managed device list — flat rows, no per-row card border */}
      {btReady && systemConnectedDevices.length > 0 && (
        <VStack align="stretch" spacing={1} pt={1}>
          <Text fontSize="10px" color="text.muted" fontFamily="mono" textTransform="uppercase" letterSpacing="0.08em">
            macOS connected ({systemConnectedDevices.length})
          </Text>
          {systemConnectedDevices.map((d, i) => {
            const addressLabel =
              d.address && d.address !== '00:00:00:00:00:00'
                ? d.address
                : 'address hidden';
            const extras = [d.kind, d.battery != null ? `${d.battery}%` : null]
              .filter(Boolean)
              .join(' · ');
            return (
              <HStack
                key={`${d.name}-${d.address ?? 'na'}-${i}`}
                justify="space-between"
                px={2}
                py={1.5}
                borderRadius="sm"
                _hover={{ bg: 'bg.elevated' }}
              >
                <VStack align="flex-start" spacing={0}>
                  <Text fontSize="xs" color="text.primary">{d.name}</Text>
                  <Text fontSize="10px" color="text.muted" fontFamily="mono">
                    {extras ? `${addressLabel} · ${extras}` : addressLabel}
                  </Text>
                </VStack>
                <Badge variant="subtle" colorScheme="blue">macOS</Badge>
              </HStack>
            );
          })}
        </VStack>
      )}

      {btReady && systemSource === 'system_profiler' && systemConnectedDevices.length === 0 && (
        <Text fontSize="10px" color="text.muted" fontFamily="mono" textAlign="center" pt={1}>
          No connected devices reported by macOS
        </Text>
      )}

      {btReady && systemMessage && systemConnectedDevices.length === 0 && (
        <Text fontSize="10px" color="text.muted" fontFamily="mono" textAlign="center" pt={1}>
          {systemMessage}
        </Text>
      )}

      {/* Scan results */}
      {btReady && devices.length > 0 && (
        <VStack align="stretch" spacing={1} pt={1}>
          <Text fontSize="10px" color="text.muted" fontFamily="mono" textTransform="uppercase" letterSpacing="0.08em">
            {devices.length} device{devices.length !== 1 ? 's' : ''} found
          </Text>
          {devices.map((d) => {
            const isHhkb = d.name?.startsWith('HHKB') ?? false;
            const displayName =
              d.name ?? (d.connected ? 'Unknown (connected)' : 'Unknown');
            const idLabel =
              d.address && d.address !== '00:00:00:00:00:00' ? d.address : d.id;
            const subLabel =
              d.rssi != null ? `${d.rssi} dBm · ${idLabel}` : idLabel;
            const dimmed = !d.name && !d.connected;
            return (
              <HStack
                key={d.id}
                justify="space-between"
                px={2}
                py={1.5}
                borderRadius="sm"
                bg={isHhkb ? 'wireless.subtle' : 'transparent'}
                opacity={dimmed ? 0.45 : 1}
                _hover={{ bg: isHhkb ? 'wireless.subtle' : 'bg.elevated' }}
              >
                <VStack align="flex-start" spacing={0}>
                  <HStack spacing={1.5}>
                    <Text
                      fontSize="xs"
                      color={isHhkb ? 'text.primary' : 'text.secondary'}
                    >
                      {displayName}
                    </Text>
                    {d.connected && (
                      <Text
                        fontSize="9px"
                        color="wireless.fg"
                        fontFamily="mono"
                        textTransform="uppercase"
                      >
                        OS
                      </Text>
                    )}
                  </HStack>
                  <Text fontSize="10px" color="text.muted" fontFamily="mono">
                    {subLabel}
                  </Text>
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
      {btReady && !connected && !scanning && devices.length === 0 && systemConnectedDevices.length === 0 && (
        <Text fontSize="xs" color="text.muted" textAlign="center" pt={1}>
          Pair HHKB in system Bluetooth settings, then press scan
        </Text>
      )}
    </VStack>
  );
}

// Simple battery bar: 5 segments
function BatteryBar({ level }: { level: number }) {
  const filled = Math.round((level / 100) * 5);
  const color = level > 20 ? 'wireless.fg' : 'danger';
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

function formatRelative(ts: number): string {
  const diff = Math.max(0, Date.now() - ts);
  if (diff < 5_000) return 'just now';
  if (diff < 60_000) return `${Math.floor(diff / 1000)}s ago`;
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  return `${Math.floor(diff / 3_600_000)}h ago`;
}
