import { useState } from 'react';
import {
  Box,
  Flex,
  HStack,
  Heading,
  Menu,
  MenuButton,
  MenuDivider,
  MenuItem,
  MenuList,
  Button,
  Text,
  Circle,
  Tag,
} from '@chakra-ui/react';
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

  const daemonColor =
    daemonStatus === 'online'
      ? 'green.400'
      : daemonStatus === 'offline'
        ? 'red.400'
        : 'yellow.400';

  const daemonLabel =
    daemonStatus === 'online'
      ? `connected${daemonVersion ? ` (v${daemonVersion})` : ''}`
      : daemonStatus === 'offline'
        ? 'offline'
        : 'checking...';

  const transportLabel =
    transport === 'webhid'
      ? 'WebHID'
      : transport === 'daemon'
        ? 'Daemon'
        : 'Offline';

  const transportScheme =
    transport === 'webhid'
      ? 'green'
      : transport === 'daemon'
        ? 'blue'
        : 'gray';

  return (
    <>
      <Flex
        as="header"
        align="center"
        justify="space-between"
        px={6}
        py={3}
        bg="gray.800"
        borderBottom="1px solid"
        borderColor="gray.700"
      >
        <HStack spacing={4}>
          <Heading size="md" color="brand.50">
            RoninKB
          </Heading>
          <Menu>
            <MenuButton
              as={Button}
              size="sm"
              variant="outline"
              colorScheme="whiteAlpha"
            >
              {active ? `${active.icon ?? ''} ${active.name}` : 'No profile'}
            </MenuButton>
            <MenuList>
              {profiles.map((p) => (
                <MenuItem
                  key={p.id}
                  onClick={() => {
                    void setActive(p.id);
                  }}
                  color="gray.800"
                >
                  {p.icon ? `${p.icon} ` : ''}
                  {p.name}
                </MenuItem>
              ))}
              {profiles.length > 0 && <MenuDivider />}
              <MenuItem
                color="gray.800"
                onClick={() => setImportOpen(true)}
              >
                Import profile...
              </MenuItem>
              <MenuItem
                color="gray.800"
                onClick={() => setImportOpen(true)}
              >
                Export profile...
              </MenuItem>
            </MenuList>
          </Menu>
        </HStack>

        <HStack spacing={4}>
          <HStack spacing={2}>
            <Circle size="10px" bg={daemonColor} />
            <Text fontSize="sm" color="gray.300">
              Daemon: {daemonLabel}
            </Text>
          </HStack>
          <Tag size="sm" colorScheme={transportScheme}>
            {transportLabel}
          </Tag>
          <Box>
            <ConnectButton />
          </Box>
          <Text fontSize="sm" color="gray.400">
            {deviceStatus}
          </Text>
        </HStack>
      </Flex>

      <ProfileImportExport
        isOpen={importOpen}
        onClose={() => setImportOpen(false)}
      />
    </>
  );
}
