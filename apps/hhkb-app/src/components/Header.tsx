import { useState } from 'react';
import {
  Box,
  Flex,
  HStack,
  VStack,
  Menu,
  MenuButton,
  MenuDivider,
  MenuItem,
  MenuList,
  Button,
  Text,
  Tag,
  Tooltip,
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverBody,
  Divider,
  useColorModeValue,
  useToast,
} from '@chakra-ui/react';
import {
  ChevronDown,
  Wifi,
  WifiOff,
  AlertCircle,
  Download,
  Upload,
  Check,
  Plus,
  Camera,
  Factory,
  Battery,
  Play,
  Square,
  FolderOpen,
  Copy,
  ShieldCheck,
  ShieldAlert,
  ShieldQuestion,
} from 'lucide-react';
import { useProfileStore } from '../store/profileStore';
import { useDaemonStore } from '../store/daemonStore';
import { useDeviceStore } from '../store/deviceStore';
import { useBluetoothStore } from '../store/bluetoothStore';
import { useKanataStore } from '../store/kanataStore';
import { ConnectButton } from './ConnectButton';
import { ProfileImportExport } from './ProfileImportExport';
import { NewProfileDialog, type NewProfileMode } from './NewProfileDialog';
import { RoninLogo } from './RoninLogo';
import { FACTORY_PROFILE_IDS } from '../data/factoryDefault';

export function Header() {
  const profiles = useProfileStore((s) => s.profiles);
  const activeId = useProfileStore((s) => s.activeProfileId);
  const setActive = useProfileStore((s) => s.setActiveProfile);
  const active = profiles.find((p) => p.id === activeId);

  const daemonStatus = useDaemonStore((s) => s.status);
  const daemonVersion = useDaemonStore((s) => s.version);
  const deviceStatus = useDeviceStore((s) => s.status);
  const transport = useDeviceStore((s) => s.transportMode)();
  const btConnected = useBluetoothStore((s) => s.connected);
  const battery = useBluetoothStore((s) => s.battery);

  const kanataState = useKanataStore((s) => s.processState);
  const kanataPid = useKanataStore((s) => s.pid);
  const kanataInstalled = useKanataStore((s) => s.installed);
  const kanataLoading = useKanataStore((s) => s.loading);
  const kanataStart = useKanataStore((s) => s.start);
  const kanataStop = useKanataStore((s) => s.stop);
  const kanataBinaryPath = useKanataStore((s) => s.binaryPath);
  const kanataConfigPath = useKanataStore((s) => s.configPath);
  const kanataInputMonitoring = useKanataStore((s) => s.inputMonitoringGranted);
  const kanataDevicePath = useKanataStore((s) => s.devicePath);
  const kanataStderr = useKanataStore((s) => s.stderrTail);
  const kanataError = useKanataStore((s) => s.error);

  const toast = useToast();

  const [importOpen, setImportOpen] = useState(false);
  const [newProfileMode, setNewProfileMode] = useState<NewProfileMode | null>(null);

  async function handleKanataToggle() {
    try {
      if (kanataState === 'running') {
        await kanataStop();
      } else {
        await kanataStart();
      }
    } catch {
      // errors are tracked in kanataStore.error
    }
  }

  async function handleKanataReveal() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      await client.kanataReveal();
    } catch (e) {
      toast({
        title: 'Reveal failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 4000,
        isClosable: true,
      });
    }
  }

  async function copyToClipboard(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      toast({
        title: `${label} copied`,
        status: 'success',
        duration: 1600,
        isClosable: true,
      });
    } catch {
      // ignore — clipboard not available
    }
  }

  const daemonMeta =
    daemonStatus === 'online'
      ? {
          Icon: Wifi,
          label: `Daemon v${daemonVersion ?? '—'}`,
          color: 'success',
        }
      : daemonStatus === 'offline'
        ? {
            Icon: WifiOff,
            label: 'Daemon offline',
            color: 'text.muted',
          }
        : {
            Icon: AlertCircle,
            label: 'Checking…',
            color: 'warning',
          };

  const transportLabel =
    transport === 'webhid'
      ? 'WebHID'
      : transport === 'daemon'
        ? 'Daemon'
        : 'Disconnected';

  const batteryColor =
    battery === null
      ? 'text.muted'
      : battery >= 50
        ? 'success'
        : battery >= 20
          ? 'warning'
          : 'error';
  const logoConnected = deviceStatus === 'connected' || btConnected;
  const logoColor = useColorModeValue('text.primary', 'accent.primary');
  const logoBorderColor = logoConnected ? 'transparent' : 'border.subtle';

  const DaemonIcon = daemonMeta.Icon;

  return (
    <>
      <Box
        as="header"
        position="sticky"
        top={4}
        mx={4}
        mt={4}
        zIndex={50}
      >
        <Flex
          align="center"
          justify="space-between"
          px={4}
          py={2.5}
          bg="rgba(19, 19, 22, 0.75)"
          backdropFilter="blur(20px) saturate(140%)"
          border="1px solid"
          borderColor="border.subtle"
          borderRadius="xl"
          boxShadow="elevated"
          sx={{
            '@supports not (backdrop-filter: blur(20px))': {
              bg: 'bg.surface',
            },
          }}
        >
          {/* Left — brand */}
          <HStack spacing={3} flex="0 0 auto">
            <Tooltip
              label={
                logoConnected
                  ? deviceStatus === 'connected'
                    ? 'Keyboard linked'
                    : 'Bluetooth paired'
                  : 'No keyboard'
              }
              placement="bottom-start"
              hasArrow={false}
              openDelay={400}
            >
              <Flex
                align="center"
                justify="center"
                w="28px"
                h="28px"
                borderRadius="sm"
                color={logoColor}
                border="1px solid"
                borderColor={logoBorderColor}
                transition="border-color 180ms ease, color 180ms ease"
              >
                <RoninLogo connected={logoConnected} size={18} strokeWidth={1.75} />
              </Flex>
            </Tooltip>
            <Box>
              <Text
                fontSize="sm"
                fontWeight={600}
                letterSpacing="-0.01em"
                lineHeight="1"
              >
                RoninKB
              </Text>
              <Text fontSize="10px" color="text.muted" lineHeight="1.4">
                HHKB Pro Hybrid
              </Text>
            </Box>
          </HStack>

          {/* Center — profile switcher */}
          <HStack spacing={2} flex="1 1 auto" justify="center">
            <Menu placement="bottom">
              <MenuButton
                as={Button}
                variant="subtle"
                size="sm"
                rightIcon={<ChevronDown size={14} />}
                fontFamily="mono"
                fontSize="xs"
                minW="200px"
                justifyContent="space-between"
              >
                {active ? active.name : 'No profile'}
              </MenuButton>
              <MenuList>
                {profiles.length === 0 && (
                  <Text px={3} py={2} fontSize="xs" color="text.muted">
                    No profiles yet
                  </Text>
                )}
                {profiles.map((p) => {
                  const isActive = p.id === activeId;
                  const isFactory = FACTORY_PROFILE_IDS.has(p.id);
                  return (
                    <MenuItem
                      key={p.id}
                      onClick={() => {
                        void setActive(p.id);
                      }}
                      icon={
                        <Box w="14px" h="14px" display="inline-flex" alignItems="center">
                          {isActive ? (
                            <Check size={14} strokeWidth={2.5} />
                          ) : isFactory ? (
                            <Factory size={12} />
                          ) : null}
                        </Box>
                      }
                    >
                      <HStack justify="space-between" w="100%" spacing={2}>
                        <Text fontSize="sm" noOfLines={1}>{p.name}</Text>
                        {isFactory && (
                          <Tag size="sm" variant="subtle" flexShrink={0}>
                            <Text fontSize="9px" fontFamily="mono" textTransform="uppercase">
                              factory
                            </Text>
                          </Tag>
                        )}
                        {!isFactory && p.tags && p.tags.length > 0 && (
                          <Text
                            fontSize="10px"
                            color="text.muted"
                            fontFamily="mono"
                            flexShrink={0}
                          >
                            {p.tags[0]}
                          </Text>
                        )}
                      </HStack>
                    </MenuItem>
                  );
                })}
                <MenuDivider />
                <MenuItem
                  icon={<Plus size={14} />}
                  onClick={() => setNewProfileMode('blank')}
                >
                  New blank profile
                </MenuItem>
                <MenuItem
                  icon={<Camera size={14} />}
                  onClick={() => setNewProfileMode('capture')}
                >
                  Capture hardware as profile
                </MenuItem>
                <MenuDivider />
                <MenuItem
                  icon={<Upload size={14} />}
                  onClick={() => setImportOpen(true)}
                >
                  Import profile
                </MenuItem>
                <MenuItem
                  icon={<Download size={14} />}
                  onClick={() => setImportOpen(true)}
                >
                  Export profile
                </MenuItem>
              </MenuList>
            </Menu>
          </HStack>

          {/* Right — status + connect */}
          <HStack spacing={3} flex="0 0 auto">
            <Tooltip
              label={`${daemonMeta.label} · ${transportLabel} · ${deviceStatus}`}
              placement="bottom-end"
              hasArrow={false}
              openDelay={300}
            >
              <HStack
                spacing={2}
                px={2.5}
                h="28px"
                borderRadius="sm"
                bg="bg.subtle"
                border="1px solid"
                borderColor="border.subtle"
                cursor="default"
              >
                <Box color={daemonMeta.color} display="flex">
                  <DaemonIcon size={12} strokeWidth={2.5} />
                </Box>
                <Text
                  fontSize="10px"
                  color="text.secondary"
                  fontFamily="mono"
                  textTransform="uppercase"
                  letterSpacing="0.06em"
                >
                  {transportLabel}
                </Text>
                {btConnected && battery !== null && (
                  <>
                    <Box w="1px" h="12px" bg="border.subtle" />
                    <Battery size={12} strokeWidth={2.5} color={batteryColor} />
                    <Text fontSize="10px" color="text.secondary" fontFamily="mono">{battery}%</Text>
                  </>
                )}
              </HStack>
            </Tooltip>
            {kanataInstalled && (
              <Popover placement="bottom-end" gutter={8} closeOnBlur>
                <PopoverTrigger>
                  <HStack
                    as="button"
                    spacing={1.5}
                    px={2.5}
                    h="28px"
                    borderRadius="sm"
                    bg="bg.subtle"
                    border="1px solid"
                    borderColor={
                      kanataState === 'running'
                        ? 'kanata.border'
                        : 'border.subtle'
                    }
                    cursor="pointer"
                    _hover={{ borderColor: 'border.strong' }}
                  >
                    <Box
                      w="6px"
                      h="6px"
                      borderRadius="full"
                      bg={
                        kanataState === 'running'
                          ? 'success'
                          : kanataState === 'stopped'
                            ? 'warning'
                            : 'text.muted'
                      }
                      flexShrink={0}
                    />
                    <Text
                      fontSize="10px"
                      color={
                        kanataState === 'running'
                          ? 'kanata.fg'
                          : 'text.secondary'
                      }
                      fontFamily="mono"
                      textTransform="uppercase"
                      letterSpacing="0.06em"
                    >
                      Kanata
                    </Text>
                  </HStack>
                </PopoverTrigger>
                <PopoverContent
                  w="320px"
                  bg="bg.surface"
                  borderColor="border.subtle"
                  boxShadow="elevated"
                  _focus={{ outline: 'none', boxShadow: 'elevated' }}
                >
                  <PopoverBody p={3}>
                    <KanataPopoverBody
                      processState={kanataState}
                      pid={kanataPid}
                      binaryPath={kanataBinaryPath}
                      configPath={kanataConfigPath}
                      devicePath={kanataDevicePath}
                      inputMonitoring={kanataInputMonitoring}
                      stderrTail={kanataStderr}
                      lastError={kanataError}
                      loading={kanataLoading}
                      onToggle={() => void handleKanataToggle()}
                      onReveal={() => void handleKanataReveal()}
                      onCopy={(v, l) => void copyToClipboard(v, l)}
                    />
                  </PopoverBody>
                </PopoverContent>
              </Popover>
            )}
            <ConnectButton />
          </HStack>
        </Flex>
      </Box>

      <ProfileImportExport
        isOpen={importOpen}
        onClose={() => setImportOpen(false)}
      />

      <NewProfileDialog
        isOpen={newProfileMode !== null}
        mode={newProfileMode ?? 'blank'}
        onClose={() => setNewProfileMode(null)}
        onCreated={(id) => {
          void setActive(id);
          setNewProfileMode(null);
        }}
      />
    </>
  );
}

// ─── Kanata popover body ────────────────────────────────────────────────────

interface KanataPopoverBodyProps {
  processState: 'not_installed' | 'stopped' | 'running';
  pid: number | null;
  binaryPath: string | null;
  configPath: string | null;
  devicePath: string | null;
  inputMonitoring: boolean | null;
  stderrTail: string[];
  lastError: string | null;
  loading: boolean;
  onToggle: () => void;
  onReveal: () => void;
  onCopy: (value: string, label: string) => void;
}

function KanataPopoverBody(props: KanataPopoverBodyProps) {
  const {
    processState,
    pid,
    binaryPath,
    configPath,
    devicePath,
    inputMonitoring,
    stderrTail,
    lastError,
    loading,
    onToggle,
    onReveal,
    onCopy,
  } = props;

  const [stderrOpen, setStderrOpen] = useState(false);

  const running = processState === 'running';
  const stateLabel = running
    ? 'Running'
    : processState === 'stopped'
      ? 'Stopped'
      : 'Not installed';
  const stateColor = running
    ? 'success'
    : processState === 'stopped'
      ? 'warning'
      : 'text.muted';

  const ImIcon =
    inputMonitoring === true
      ? ShieldCheck
      : inputMonitoring === false
        ? ShieldAlert
        : ShieldQuestion;
  const imColor =
    inputMonitoring === true
      ? 'success'
      : inputMonitoring === false
        ? 'warning'
        : 'text.muted';
  const imLabel =
    inputMonitoring === true
      ? 'Input Monitoring granted'
      : inputMonitoring === false
        ? 'Input Monitoring not granted'
        : 'Input Monitoring unknown';

  return (
    <VStack align="stretch" spacing={3}>
      {/* Header — state */}
      <HStack justify="space-between" align="center">
        <HStack spacing={2}>
          <Box
            w="8px"
            h="8px"
            borderRadius="full"
            bg={stateColor}
            flexShrink={0}
          />
          <Text
            fontSize="11px"
            color="text.secondary"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.06em"
          >
            Kanata · {stateLabel}
          </Text>
        </HStack>
        {pid !== null && (
          <Text fontSize="10px" color="text.muted" fontFamily="mono">
            PID {pid}
          </Text>
        )}
      </HStack>

      {/* Big primary action */}
      <Button
        size="sm"
        onClick={onToggle}
        isLoading={loading}
        leftIcon={running ? <Square size={12} /> : <Play size={12} />}
        bg={running ? 'kanata.subtle' : 'accent.primary'}
        color={running ? 'kanata.fg' : 'accent.fg'}
        borderWidth="1px"
        borderColor={running ? 'kanata.border' : 'accent.primary'}
        _hover={{
          bg: running ? 'kanata.subtle' : 'accent.hover',
          opacity: 0.92,
        }}
        _active={{ opacity: 0.85 }}
        fontFamily="mono"
        fontSize="xs"
        textTransform="uppercase"
        letterSpacing="0.06em"
      >
        {running ? 'Stop kanata' : 'Start kanata'}
      </Button>

      {/* Input Monitoring row + Reveal */}
      <HStack
        justify="space-between"
        align="center"
        bg={inputMonitoring === false ? 'warning.subtle' : 'kanata.subtle'}
        borderRadius="md"
        px={2.5}
        py={2}
      >
        <HStack spacing={2}>
          <Box color={imColor} display="flex">
            <ImIcon size={13} strokeWidth={2.25} />
          </Box>
          <Text fontSize="11px" color="text.secondary">
            {imLabel}
          </Text>
        </HStack>
        {(inputMonitoring === false || inputMonitoring === null) && (
          <Button
            size="xs"
            variant="ghost"
            leftIcon={<FolderOpen size={11} />}
            onClick={onReveal}
            fontSize="10px"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.06em"
          >
            Reveal
          </Button>
        )}
      </HStack>

      <Divider borderColor="border.subtle" />

      {/* Diagnostic rows */}
      <VStack align="stretch" spacing={1}>
        <DiagnosticRow
          label="Binary"
          value={binaryPath}
          onCopy={(v) => onCopy(v, 'Binary path')}
        />
        <DiagnosticRow
          label="Config"
          value={configPath}
          onCopy={(v) => onCopy(v, 'Config path')}
        />
        {devicePath && (
          <DiagnosticRow
            label="Device"
            value={devicePath}
            onCopy={(v) => onCopy(v, 'Device path')}
          />
        )}
      </VStack>

      {/* Last error */}
      {lastError && (
        <Box
          bg="danger.subtle"
          border="1px solid"
          borderColor="border.subtle"
          borderRadius="md"
          px={2.5}
          py={1.5}
        >
          <Text
            fontSize="10px"
            color="danger"
            fontFamily="mono"
            lineHeight="1.4"
            wordBreak="break-word"
          >
            {lastError}
          </Text>
        </Box>
      )}

      {/* Stderr tail — collapsible */}
      {stderrTail.length > 0 && (
        <VStack align="stretch" spacing={1.5}>
          <HStack
            as="button"
            justify="space-between"
            onClick={() => setStderrOpen((v) => !v)}
            _hover={{ color: 'text.primary' }}
          >
            <Text
              fontSize="10px"
              color="text.muted"
              fontFamily="mono"
              textTransform="uppercase"
              letterSpacing="0.08em"
            >
              stderr ({stderrTail.length})
            </Text>
            <Text
              fontSize="10px"
              color="text.muted"
              fontFamily="mono"
            >
              {stderrOpen ? 'hide' : 'show'}
            </Text>
          </HStack>
          {stderrOpen && (
            <Box
              bg="bg.subtle"
              border="1px solid"
              borderColor="border.subtle"
              borderRadius="md"
              p={2}
              maxH="140px"
              overflowY="auto"
              fontFamily="mono"
              fontSize="10px"
              color="text.secondary"
              lineHeight="1.5"
              whiteSpace="pre-wrap"
              wordBreak="break-word"
            >
              {stderrTail.join('\n')}
            </Box>
          )}
        </VStack>
      )}
    </VStack>
  );
}

function DiagnosticRow({
  label,
  value,
  onCopy,
}: {
  label: string;
  value: string | null;
  onCopy: (value: string) => void;
}) {
  if (!value) {
    return (
      <HStack justify="space-between" align="center">
        <Text
          fontSize="10px"
          color="text.muted"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.06em"
          minW="48px"
        >
          {label}
        </Text>
        <Text fontSize="10px" color="text.muted" fontFamily="mono">
          —
        </Text>
      </HStack>
    );
  }
  return (
    <HStack
      align="center"
      spacing={2}
      _hover={{ '> button': { opacity: 1 } }}
    >
      <Text
        fontSize="10px"
        color="text.muted"
        fontFamily="mono"
        textTransform="uppercase"
        letterSpacing="0.06em"
        minW="48px"
        flexShrink={0}
      >
        {label}
      </Text>
      <Tooltip label="Click to copy" placement="top" openDelay={400} hasArrow={false}>
        <Text
          as="button"
          flex="1"
          fontSize="10px"
          color="text.secondary"
          fontFamily="mono"
          textAlign="left"
          isTruncated
          onClick={() => onCopy(value)}
          _hover={{ color: 'text.primary' }}
        >
          {value}
        </Text>
      </Tooltip>
      <Box
        as="button"
        opacity={0}
        transition="opacity 120ms ease"
        onClick={() => onCopy(value)}
        color="text.muted"
        _hover={{ color: 'text.primary' }}
        flexShrink={0}
      >
        <Copy size={10} />
      </Box>
    </HStack>
  );
}
