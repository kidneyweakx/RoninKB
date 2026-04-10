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
  Switch,
  Text,
  VStack,
} from '@chakra-ui/react';
import { ExternalLink, PlayCircle, Settings } from 'lucide-react';
import { useDaemonStore } from '../store/daemonStore';
import { useDeviceStore } from '../store/deviceStore';
import { useSetupStore } from '../store/setupStore';

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
                  href="https://github.com/roninKB/roninKB"
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
                  github.com/roninKB/roninKB
                </Box>
              </VStack>
            </Box>

          </VStack>
        </DrawerBody>
      </DrawerContent>
    </Drawer>
  );
}
