import { useState } from 'react';
import {
  Box,
  Flex,
  HStack,
  Menu,
  MenuButton,
  MenuDivider,
  MenuItem,
  MenuList,
  Button,
  Text,
  Tag,
  Tooltip,
  useColorModeValue,
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
              <Tooltip
                label={
                  kanataState === 'running'
                    ? `Kanata running · PID ${kanataPid ?? '?'}`
                    : 'Kanata stopped — click to start'
                }
                placement="bottom"
                hasArrow={false}
                openDelay={300}
              >
                <HStack
                  as="button"
                  spacing={1.5}
                  px={2.5}
                  h="28px"
                  borderRadius="sm"
                  bg="bg.subtle"
                  border="1px solid"
                  borderColor="border.subtle"
                  cursor={kanataLoading ? 'wait' : 'pointer'}
                  opacity={kanataLoading ? 0.7 : 1}
                  onClick={() => void handleKanataToggle()}
                  _hover={{ borderColor: 'border.strong' }}
                >
                  <Box
                    w="6px"
                    h="6px"
                    borderRadius="full"
                    bg={kanataState === 'running' ? 'success' : 'warning'}
                    flexShrink={0}
                  />
                  <Text
                    fontSize="10px"
                    color="text.secondary"
                    fontFamily="mono"
                    textTransform="uppercase"
                    letterSpacing="0.06em"
                  >
                    Kanata
                  </Text>
                </HStack>
              </Tooltip>
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
