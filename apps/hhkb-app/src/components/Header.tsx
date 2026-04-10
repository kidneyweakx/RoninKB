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
  Tooltip,
} from '@chakra-ui/react';
import {
  Command,
  ChevronDown,
  Wifi,
  WifiOff,
  AlertCircle,
  Download,
  Upload,
  Check,
  Plus,
} from 'lucide-react';
import { useProfileStore } from '../store/profileStore';
import { useDaemonStore } from '../store/daemonStore';
import { useDeviceStore } from '../store/deviceStore';
import { ConnectButton } from './ConnectButton';
import { ProfileImportExport } from './ProfileImportExport';

export function Header() {
  const profiles = useProfileStore((s) => s.profiles);
  const activeId = useProfileStore((s) => s.activeProfileId);
  const setActive = useProfileStore((s) => s.setActiveProfile);
  const active = profiles.find((p) => p.id === activeId);

  const daemonStatus = useDaemonStore((s) => s.status);
  const daemonVersion = useDaemonStore((s) => s.version);
  const deviceStatus = useDeviceStore((s) => s.status);
  const transport = useDeviceStore((s) => s.transportMode)();

  const [importOpen, setImportOpen] = useState(false);

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
            <Flex
              align="center"
              justify="center"
              w="28px"
              h="28px"
              borderRadius="md"
              bg="accent.primary"
              color="accent.fg"
            >
              <Command size={16} strokeWidth={2.25} />
            </Flex>
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
                  return (
                    <MenuItem
                      key={p.id}
                      onClick={() => {
                        void setActive(p.id);
                      }}
                      icon={
                        <Box w="14px" h="14px" display="inline-flex">
                          {isActive ? (
                            <Check size={14} strokeWidth={2.5} />
                          ) : null}
                        </Box>
                      }
                    >
                      <HStack justify="space-between" w="100%">
                        <Text fontSize="sm">{p.name}</Text>
                        {p.tags && p.tags.length > 0 && (
                          <Text
                            fontSize="10px"
                            color="text.muted"
                            fontFamily="mono"
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
                <MenuItem icon={<Plus size={14} />} isDisabled>
                  New profile
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
              </HStack>
            </Tooltip>
            <ConnectButton />
          </HStack>
        </Flex>
      </Box>

      <ProfileImportExport
        isOpen={importOpen}
        onClose={() => setImportOpen(false)}
      />
    </>
  );
}
