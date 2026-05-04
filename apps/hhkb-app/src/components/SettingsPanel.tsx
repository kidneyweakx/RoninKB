import { useEffect, useState } from 'react';
import {
  Box,
  Button,
  Divider,
  Drawer,
  DrawerBody,
  DrawerCloseButton,
  DrawerContent,
  DrawerHeader,
  DrawerOverlay,
  Flex,
  HStack,
  IconButton,
  Input,
  Switch,
  Text,
  VStack,
} from '@chakra-ui/react';
import { ExternalLink, PlayCircle, Settings, Terminal, Trash2 } from 'lucide-react';
import { useDaemonStore } from '../store/daemonStore';
import { useDeviceStore } from '../store/deviceStore';
import { useSetupStore } from '../store/setupStore';
import { useFlowStore } from '../store/flowStore';
import { useKanataStore } from '../store/kanataStore';

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <Text
      fontSize="10px"
      color="text.muted"
      fontFamily="mono"
      textTransform="uppercase"
      letterSpacing="0.08em"
      mb={2}
    >
      {children}
    </Text>
  );
}

function SettingsRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children?: React.ReactNode;
}) {
  return (
    <Flex justify="space-between" align="center" py={2}>
      <Box flex="1" mr={4}>
        <Text fontSize="sm" color="text.primary" fontWeight={500}>
          {label}
        </Text>
        {description && (
          <Text fontSize="xs" color="text.muted" mt={0.5}>
            {description}
          </Text>
        )}
      </Box>
      {children}
    </Flex>
  );
}

export function SettingsPanel({ isOpen, onClose }: Props) {
  const daemonStatus = useDaemonStore((s) => s.status);
  const daemonVersion = useDaemonStore((s) => s.version);
  const transport = useDeviceStore((s) => s.transportMode)();
  const openSetupWizard = useSetupStore((s) => s.openManually);

  const flowEnabled = useFlowStore((s) => s.enabled);
  const peers = useFlowStore((s) => s.peers);
  const history = useFlowStore((s) => s.history);
  const enableFlow = useFlowStore((s) => s.enable);
  const disableFlow = useFlowStore((s) => s.disable);
  const addPeer = useFlowStore((s) => s.addPeer);
  const removePeer = useFlowStore((s) => s.removePeer);
  const clearHistory = useFlowStore((s) => s.clearHistory);
  const fetchConfig = useFlowStore((s) => s.fetchConfig);
  const fetchHistory = useFlowStore((s) => s.fetchHistory);

  const kanataState = useKanataStore((s) => s.processState);
  const kanataPid = useKanataStore((s) => s.pid);
  const kanataInstalled = useKanataStore((s) => s.installed);
  const kanataBinaryPath = useKanataStore((s) => s.binaryPath);
  const kanataLoading = useKanataStore((s) => s.loading);
  const kanataPath = useKanataStore((s) => s.configPath);
  const kanataInputMonitoring = useKanataStore((s) => s.inputMonitoringGranted);
  const kanataDriverActivated = useKanataStore((s) => s.driverActivated);
  const kanataDevicePath = useKanataStore((s) => s.devicePath);
  const kanataStderrTail = useKanataStore((s) => s.stderrTail);
  const kanataError = useKanataStore((s) => s.error);
  const kanataStart = useKanataStore((s) => s.start);
  const kanataStop = useKanataStore((s) => s.stop);
  const fetchKanataStatus = useKanataStore((s) => s.fetchStatus);

  const [newPeerAddr, setNewPeerAddr] = useState('');
  const [kanataToggling, setKanataToggling] = useState(false);

  useEffect(() => {
    if (isOpen) {
      void fetchConfig();
      void fetchHistory();
      void fetchKanataStatus();
    }
  }, [isOpen, fetchConfig, fetchHistory, fetchKanataStatus]);

  async function handleKanataToggle() {
    setKanataToggling(true);
    try {
      if (kanataState === 'running') {
        await kanataStop();
      } else {
        await kanataStart();
      }
    } finally {
      setKanataToggling(false);
    }
  }

  function handleAddPeer() {
    const trimmed = newPeerAddr.trim();
    if (!trimmed) return;
    // Parse "hostname:port" or "addr:port" — use the addr as both hostname and addr
    const colonIdx = trimmed.lastIndexOf(':');
    const hostname = colonIdx > 0 ? trimmed.slice(0, colonIdx) : trimmed;
    void addPeer(hostname, trimmed);
    setNewPeerAddr('');
  }

  const connectionMode =
    transport === 'webhid'
      ? 'WebHID'
      : transport === 'daemon'
        ? 'Daemon'
        : 'None';

  return (
    <Drawer isOpen={isOpen} onClose={onClose} placement="right" size="sm">
      <DrawerOverlay />
      <DrawerContent bg="bg.surface" borderLeft="1px solid" borderColor="border.subtle">
        <DrawerCloseButton />
        <DrawerHeader borderBottomWidth="1px" borderColor="border.subtle">
          <HStack spacing={2}>
            <Box color="accent.primary">
              <Settings size={16} />
            </Box>
            <Text fontSize="sm" fontWeight={600}>
              Settings
            </Text>
          </HStack>
        </DrawerHeader>

        <DrawerBody px={5} py={5}>
          <VStack align="stretch" spacing={6} divider={<Divider />}>

            {/* App section */}
            <Box>
              <SectionLabel>App</SectionLabel>
              <VStack align="stretch" spacing={0} divider={<Divider />}>
                <SettingsRow label="Version">
                  <Text fontSize="xs" color="text.muted" fontFamily="mono">
                    v0.1.0
                  </Text>
                </SettingsRow>
                <SettingsRow label="Connection">
                  <Text fontSize="xs" color="text.muted" fontFamily="mono">
                    {connectionMode}
                  </Text>
                </SettingsRow>
                <SettingsRow label="Daemon">
                  <Text
                    fontSize="xs"
                    fontFamily="mono"
                    color={
                      daemonStatus === 'online'
                        ? 'success'
                        : daemonStatus === 'offline'
                          ? 'text.muted'
                          : 'warning'
                    }
                  >
                    {daemonStatus === 'online'
                      ? `online v${daemonVersion ?? '—'}`
                      : daemonStatus === 'offline'
                        ? 'offline'
                        : 'checking…'}
                  </Text>
                </SettingsRow>
              </VStack>
            </Box>

            {/* Status bar section */}
            <Box>
              <SectionLabel>Status Bar</SectionLabel>
              <SettingsRow
                label="Show in macOS menu bar"
                description="Requires daemon with --features tray"
              >
                <Switch isDisabled size="sm" />
              </SettingsRow>
            </Box>

            {/* Theme section */}
            <Box>
              <SectionLabel>Theme</SectionLabel>
              <SettingsRow
                label="Dark mode"
                description="RoninKB is dark-only for now"
              >
                <Switch isChecked isDisabled size="sm" />
              </SettingsRow>
            </Box>

            {/* Flow section */}
            <Box>
              <SectionLabel>Flow — 剪貼板同步</SectionLabel>
              <VStack align="stretch" spacing={0} divider={<Divider />}>

                {/* Enable/disable toggle */}
                <SettingsRow
                  label="Flow 同步"
                  description={flowEnabled ? '已啟用，剪貼板變化自動同步到 peers' : '停用中'}
                >
                  <Switch
                    size="sm"
                    isChecked={flowEnabled}
                    isDisabled={daemonStatus !== 'online'}
                    onChange={async (e) => {
                      if (e.target.checked) await enableFlow();
                      else await disableFlow();
                    }}
                  />
                </SettingsRow>

                {/* Peers list + add form */}
                {flowEnabled && (
                  <Box py={3}>
                    <Text fontSize="xs" color="text.muted" mb={2}>
                      Peers（格式：192.168.x.x:7331）
                    </Text>
                    {peers.map((peer) => (
                      <HStack key={peer.id} justify="space-between" py={1}>
                        <VStack align="start" spacing={0}>
                          <Text fontSize="xs" fontFamily="mono">{peer.hostname}</Text>
                          <Text fontSize="10px" color="text.muted" fontFamily="mono">{peer.addr}</Text>
                        </VStack>
                        <HStack spacing={1}>
                          <Box
                            w="6px"
                            h="6px"
                            borderRadius="full"
                            bg={peer.online ? 'success' : 'text.muted'}
                          />
                          <IconButton
                            aria-label="Remove peer"
                            icon={<Trash2 size={12} />}
                            size="xs"
                            variant="ghost"
                            onClick={() => void removePeer(peer.id)}
                          />
                        </HStack>
                      </HStack>
                    ))}
                    {/* Add peer form */}
                    <HStack mt={2}>
                      <Input
                        size="xs"
                        placeholder="192.168.1.x:7331"
                        fontFamily="mono"
                        value={newPeerAddr}
                        onChange={(e) => setNewPeerAddr(e.target.value)}
                      />
                      <Button
                        size="xs"
                        onClick={handleAddPeer}
                        isDisabled={!newPeerAddr.trim()}
                      >
                        Add
                      </Button>
                    </HStack>
                  </Box>
                )}

                {/* History (last 5 entries) */}
                {flowEnabled && history.length > 0 && (
                  <Box py={3}>
                    <HStack justify="space-between" mb={2}>
                      <Text fontSize="xs" color="text.muted">最近同步</Text>
                      <Button size="xs" variant="ghost" onClick={() => void clearHistory()}>
                        清除
                      </Button>
                    </HStack>
                    <VStack align="stretch" spacing={1}>
                      {history.slice(0, 5).map((entry) => (
                        <Box key={entry.id} p={2} bg="bg.subtle" borderRadius="sm">
                          <Text fontSize="10px" fontFamily="mono" color="text.secondary" noOfLines={1}>
                            {entry.content}
                          </Text>
                          <Text fontSize="9px" color="text.muted" mt={0.5}>
                            {entry.source.type === 'local'
                              ? '本機'
                              : `← ${(entry.source as { hostname: string }).hostname}`}
                          </Text>
                        </Box>
                      ))}
                    </VStack>
                  </Box>
                )}

              </VStack>
            </Box>

            {/* Kanata section */}
            <Box>
              <SectionLabel>Kanata — 軟體按鍵</SectionLabel>
              <VStack align="stretch" spacing={0} divider={<Divider />}>

                <SettingsRow
                  label="Status"
                  description={
                    !kanataInstalled
                      ? 'Not installed — run: cargo install kanata'
                      : kanataState === 'running'
                        ? `Running · PID ${kanataPid ?? '?'}`
                        : 'Stopped'
                  }
                >
                  <Box
                    w="8px"
                    h="8px"
                    borderRadius="full"
                    flexShrink={0}
                    bg={
                      !kanataInstalled
                        ? 'text.muted'
                        : kanataState === 'running'
                          ? 'success'
                          : 'warning'
                    }
                  />
                </SettingsRow>

                {kanataPath && (
                  <SettingsRow label="Config">
                    <Text fontSize="10px" color="text.muted" fontFamily="mono" noOfLines={1} maxW="160px">
                      {kanataPath}
                    </Text>
                  </SettingsRow>
                )}

                {kanataBinaryPath && (
                  <SettingsRow label="Binary">
                    <Text fontSize="10px" color="text.muted" fontFamily="mono" noOfLines={1} maxW="160px">
                      {kanataBinaryPath}
                    </Text>
                  </SettingsRow>
                )}

                {kanataInputMonitoring !== null && (
                  <SettingsRow label="Input monitoring">
                    <Text
                      fontSize="10px"
                      fontFamily="mono"
                      color={kanataInputMonitoring ? 'success' : 'warning'}
                    >
                      {kanataInputMonitoring ? 'granted' : 'required'}
                    </Text>
                  </SettingsRow>
                )}

                {kanataDriverActivated !== null && (
                  <SettingsRow
                    label="Driver extension"
                    description={
                      kanataDriverActivated
                        ? undefined
                        : 'Open Karabiner-Elements once and approve the sysext prompt'
                    }
                  >
                    <Text
                      fontSize="10px"
                      fontFamily="mono"
                      color={kanataDriverActivated ? 'success' : 'warning'}
                    >
                      {kanataDriverActivated ? 'activated' : 'not activated'}
                    </Text>
                  </SettingsRow>
                )}

                {kanataDevicePath && (
                  <SettingsRow label="Device">
                    <Text fontSize="10px" color="text.muted" fontFamily="mono" noOfLines={1} maxW="160px">
                      {kanataDevicePath}
                    </Text>
                  </SettingsRow>
                )}

                {kanataInstalled && (
                  <Box py={2}>
                    <Button
                      size="sm"
                      variant={kanataState === 'running' ? 'outline' : 'subtle'}
                      leftIcon={<Terminal size={13} />}
                      isLoading={kanataToggling || kanataLoading}
                      isDisabled={daemonStatus !== 'online'}
                      onClick={() => void handleKanataToggle()}
                      w="100%"
                    >
                      {kanataState === 'running' ? 'Stop Kanata' : 'Start Kanata'}
                    </Button>
                  </Box>
                )}

                {kanataError && (
                  <Box py={2}>
                    <Text fontSize="10px" color="danger" fontFamily="mono">
                      {kanataError}
                    </Text>
                  </Box>
                )}

                {kanataStderrTail.length > 0 && (
                  <Box py={2}>
                    <Text fontSize="10px" color="text.muted" fontFamily="mono" mb={1}>
                      stderr tail
                    </Text>
                    <VStack align="stretch" spacing={1}>
                      {kanataStderrTail.slice(-4).map((line, idx) => (
                        <Text
                          key={`${idx}-${line}`}
                          fontSize="10px"
                          color="text.muted"
                          fontFamily="mono"
                          noOfLines={1}
                        >
                          {line}
                        </Text>
                      ))}
                    </VStack>
                  </Box>
                )}

              </VStack>
            </Box>

            {/* Setup section */}
            <Box>
              <SectionLabel>Setup</SectionLabel>
              <VStack align="stretch" spacing={2}>
                <Text fontSize="xs" color="text.muted">
                  Re-run the first-run wizard to verify browser, daemon, and
                  kanata installation.
                </Text>
                <Button
                  size="sm"
                  variant="subtle"
                  leftIcon={<PlayCircle size={14} />}
                  onClick={() => {
                    openSetupWizard();
                    onClose();
                  }}
                >
                  Run Setup Wizard
                </Button>
              </VStack>
            </Box>

            {/* About section */}
            <Box>
              <SectionLabel>About</SectionLabel>
              <VStack align="stretch" spacing={2}>
                <Text fontSize="sm" color="text.secondary" lineHeight="1.6">
                  RoninKB — Open-source HHKB configurator
                </Text>
                <Box
                  as="a"
                  href="https://github.com/kidneyweakx/RoninKB"
                  target="_blank"
                  rel="noopener noreferrer"
                  display="inline-flex"
                  alignItems="center"
                  gap={1}
                  fontSize="xs"
                  color="accent.primary"
                  _hover={{ textDecoration: 'underline' }}
                >
                  <ExternalLink size={12} />
                  github.com/kidneyweakx/RoninKB
                </Box>
              </VStack>
            </Box>

          </VStack>
        </DrawerBody>
      </DrawerContent>
    </Drawer>
  );
}
