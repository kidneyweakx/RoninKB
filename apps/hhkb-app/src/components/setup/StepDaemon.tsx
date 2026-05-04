import { useState, useMemo } from 'react';
import {
  Accordion,
  AccordionButton,
  AccordionIcon,
  AccordionItem,
  AccordionPanel,
  Box,
  Button,
  Code,
  Heading,
  HStack,
  IconButton,
  Text,
  useToast,
  VStack,
} from '@chakra-ui/react';
import {
  CheckCircle2,
  Copy,
  RefreshCw,
  XCircle,
} from 'lucide-react';
import { useDaemonStore } from '../../store/daemonStore';

type OS = 'mac' | 'linux' | 'win' | 'other';

function detectOs(): OS {
  if (typeof navigator === 'undefined') return 'other';
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes('mac')) return 'mac';
  if (ua.includes('linux')) return 'linux';
  if (ua.includes('win')) return 'win';
  return 'other';
}

const INSTALL_CMDS: Record<OS, { label: string; command: string }[]> = {
  mac: [
    {
      label: 'Install via launchctl',
      command:
        'launchctl bootstrap gui/$UID ~/Library/LaunchAgents/com.roninKB.daemon.plist',
    },
  ],
  linux: [
    {
      label: 'Enable systemd user service',
      command:
        'systemctl --user enable --now roninKB-daemon.service',
    },
  ],
  win: [
    {
      label: 'Register Task Scheduler entry',
      command:
        'schtasks /Create /SC ONLOGON /TN "RoninKB Daemon" /TR "%LOCALAPPDATA%\\RoninKB\\hhkb-daemon.exe"',
    },
  ],
  other: [],
};

export function StepDaemon() {
  const toast = useToast();
  const status = useDaemonStore((s) => s.status);
  const version = useDaemonStore((s) => s.version);
  const deviceConnected = useDaemonStore((s) => s.deviceConnected);
  const check = useDaemonStore((s) => s.check);
  const [retrying, setRetrying] = useState(false);

  const os = useMemo(detectOs, []);
  const commands = INSTALL_CMDS[os];

  async function retry() {
    setRetrying(true);
    try {
      await check();
    } finally {
      setRetrying(false);
    }
  }

  function copy(text: string) {
    void navigator.clipboard
      ?.writeText(text)
      .then(() =>
        toast({ title: 'Copied', status: 'success', duration: 1500 }),
      )
      .catch(() =>
        toast({
          title: 'Copy failed',
          status: 'error',
          duration: 2000,
        }),
      );
  }

  const online = status === 'online';

  return (
    <VStack align="stretch" spacing={4}>
      <Box>
        <Heading size="md" mb={1}>
          Daemon detection
        </Heading>
        <Text fontSize="sm" color="text.muted">
          The optional <Code fontSize="xs">hhkb-daemon</Code> unlocks macros,
          profile syncing, and cross-device flow.
        </Text>
      </Box>

      <Box
        border="1px solid"
        borderColor={online ? 'success' : 'border.subtle'}
        borderRadius="md"
        p={4}
        bg={online ? 'success.subtle' : 'bg.subtle'}
      >
        <HStack spacing={3} mb={2}>
          <Box color={online ? 'success' : 'text.muted'}>
            {online ? <CheckCircle2 size={24} /> : <XCircle size={24} />}
          </Box>
          <Box flex="1">
            <Text fontSize="sm" fontWeight={600}>
              {online
                ? `Daemon online v${version ?? '—'}`
                : 'Daemon not detected'}
            </Text>
            <Text fontSize="xs" color="text.muted">
              {online
                ? `Device connected: ${deviceConnected ? 'yes' : 'no'}`
                : 'You can skip this — the app still works in WebHID-only mode.'}
            </Text>
          </Box>
          <Button
            size="xs"
            variant="ghost"
            leftIcon={<RefreshCw size={12} />}
            onClick={retry}
            isLoading={retrying}
          >
            Retry
          </Button>
        </HStack>
      </Box>

      {!online && (
        <Accordion allowToggle>
          <AccordionItem border="1px solid" borderColor="border.subtle" borderRadius="md">
            <h2>
              <AccordionButton fontSize="sm">
                <Box flex="1" textAlign="left" fontWeight={500}>
                  Install instructions for {osLabel(os)}
                </Box>
                <AccordionIcon />
              </AccordionButton>
            </h2>
            <AccordionPanel>
              {commands.length === 0 ? (
                <Text fontSize="xs" color="text.muted">
                  Platform not auto-detected. See
                  github.com/kidneyweakx/RoninKB for manual steps.
                </Text>
              ) : (
                <VStack align="stretch" spacing={3}>
                  {commands.map((c) => (
                    <Box key={c.label}>
                      <Text fontSize="xs" color="text.muted" mb={1}>
                        {c.label}
                      </Text>
                      <HStack
                        bg="bg.surface"
                        border="1px solid"
                        borderColor="border.subtle"
                        borderRadius="md"
                        p={2}
                      >
                        <Code
                          flex="1"
                          bg="transparent"
                          fontSize="11px"
                          whiteSpace="pre-wrap"
                        >
                          {c.command}
                        </Code>
                        <IconButton
                          aria-label="Copy"
                          size="xs"
                          variant="ghost"
                          icon={<Copy size={12} />}
                          onClick={() => copy(c.command)}
                        />
                      </HStack>
                    </Box>
                  ))}
                </VStack>
              )}
            </AccordionPanel>
          </AccordionItem>
        </Accordion>
      )}
    </VStack>
  );
}

function osLabel(os: OS): string {
  switch (os) {
    case 'mac':
      return 'macOS';
    case 'linux':
      return 'Linux';
    case 'win':
      return 'Windows';
    default:
      return 'this platform';
  }
}
