import { useEffect, useState } from 'react';
import {
  Alert,
  AlertIcon,
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
  AlertTriangle,
  CheckCircle2,
  Copy,
  RefreshCw,
} from 'lucide-react';
import { useDaemonStore } from '../../store/daemonStore';
import type { KanataStatus } from '../../hhkb/daemonClient';

export function StepKanata() {
  const toast = useToast();
  const daemonStatus = useDaemonStore((s) => s.status);
  const client = useDaemonStore((s) => s.client);
  const [status, setStatus] = useState<KanataStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function recheck() {
    if (!client) return;
    setChecking(true);
    setError(null);
    try {
      const s = await client.kanataStatus();
      setStatus(s);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setChecking(false);
    }
  }

  useEffect(() => {
    if (daemonStatus === 'online') {
      void recheck();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [daemonStatus]);

  function copy(text: string) {
    void navigator.clipboard
      ?.writeText(text)
      .then(() =>
        toast({ title: 'Copied', status: 'success', duration: 1500 }),
      )
      .catch(() =>
        toast({ title: 'Copy failed', status: 'error', duration: 2000 }),
      );
  }

  if (daemonStatus !== 'online') {
    return (
      <VStack align="stretch" spacing={4}>
        <Box>
          <Heading size="md" mb={1}>
            Kanata check
          </Heading>
          <Text fontSize="sm" color="text.muted">
            Kanata powers the software-layer engine.
          </Text>
        </Box>
        <Alert status="info" borderRadius="md" fontSize="xs">
          <AlertIcon />
          Daemon required for software layer — skip to continue with
          hardware-only.
        </Alert>
      </VStack>
    );
  }

  const installed = status?.installed === true;

  return (
    <VStack align="stretch" spacing={4}>
      <Box>
        <Heading size="md" mb={1}>
          Kanata check
        </Heading>
        <Text fontSize="sm" color="text.muted">
          Kanata is a cross-platform keyboard remapper that runs as a
          userspace process.
        </Text>
      </Box>

      <Box
        border="1px solid"
        borderColor={installed ? 'success' : 'warning'}
        borderRadius="md"
        p={4}
        bg={installed ? 'success.subtle' : 'warning.subtle'}
      >
        <HStack spacing={3}>
          <Box color={installed ? 'success' : 'warning'}>
            {installed ? (
              <CheckCircle2 size={24} />
            ) : (
              <AlertTriangle size={24} />
            )}
          </Box>
          <Box flex="1">
            <Text fontSize="sm" fontWeight={600}>
              {installed ? 'Kanata installed' : 'Kanata not installed'}
            </Text>
            <Text fontSize="xs" color="text.muted">
              {installed
                ? status?.path ?? 'Detected by daemon'
                : 'The daemon could not find kanata on your PATH.'}
            </Text>
            {installed && status?.version && (
              <Text fontSize="xs" color="text.muted">
                version {status.version}
              </Text>
            )}
          </Box>
          <Button
            size="xs"
            variant="ghost"
            leftIcon={<RefreshCw size={12} />}
            onClick={recheck}
            isLoading={checking}
          >
            Re-check
          </Button>
        </HStack>
      </Box>

      {error && (
        <Alert status="error" borderRadius="md" fontSize="xs">
          <AlertIcon />
          {error}
        </Alert>
      )}

      {!installed && (
        <Box>
          <Text fontSize="xs" color="text.muted" mb={1}>
            Install via cargo
          </Text>
          <HStack
            bg="bg.surface"
            border="1px solid"
            borderColor="border.subtle"
            borderRadius="md"
            p={2}
          >
            <Code flex="1" bg="transparent" fontSize="11px">
              cargo install kanata
            </Code>
            <IconButton
              aria-label="Copy"
              size="xs"
              variant="ghost"
              icon={<Copy size={12} />}
              onClick={() => copy('cargo install kanata')}
            />
          </HStack>
        </Box>
      )}
    </VStack>
  );
}
