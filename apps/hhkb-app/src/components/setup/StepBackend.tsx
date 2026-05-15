/**
 * v0.2.0 setup wizard step: pick a software backend and walk the user
 * through the permissions it needs.
 *
 * Replaces the v0.1.x StepKanata which only knew about kanata. This
 * version reads `/backend/list` and lets the user pick whichever backend
 * matches their preferences (default: macOS users get the native backend
 * pre-selected; everyone else gets kanata). For non-Granted permission
 * states we render the deep-link the daemon embedded in the
 * RequiredPermission record so the user can open the right pane in
 * System Settings with one click.
 *
 * The user-facing "verified" toggle is a self-attestation rather than a
 * real engine probe: we can't synthesise a Caps tap-hold from the
 * browser, so the cheapest reliable check is "user opens any text
 * field, taps Caps Lock, confirms Esc was emitted". M4 §85 calls for a
 * verification step; this is the smallest version of it that works
 * cross-backend.
 */

import { useEffect } from 'react';
import {
  Alert,
  AlertIcon,
  Box,
  Button,
  Checkbox,
  Code,
  Heading,
  HStack,
  Link,
  Text,
  VStack,
} from '@chakra-ui/react';
import { ExternalLink, RefreshCw } from 'lucide-react';
import { useBackendStore } from '../../store/backendStore';
import { useDaemonStore } from '../../store/daemonStore';
import { useSetupStore } from '../../store/setupStore';
import type { BackendInfo, RequiredPermission } from '../../hhkb/daemonClient';

export function StepBackend() {
  const daemonStatus = useDaemonStore((s) => s.status);
  const backends = useBackendStore((s) => s.backends);
  const active = useBackendStore((s) => s.active);
  const loading = useBackendStore((s) => s.loading);
  const selecting = useBackendStore((s) => s.selecting);
  const error = useBackendStore((s) => s.error);
  const fetchList = useBackendStore((s) => s.fetchList);
  const select = useBackendStore((s) => s.select);
  const verifiedBackend = useSetupStore((s) => s.verifiedBackend);
  const markVerified = useSetupStore((s) => s.markVerified);
  const clearVerified = useSetupStore((s) => s.clearVerified);

  // Pull the latest backend snapshot when this step opens, so a permission
  // grant the user just clicked through is reflected without leaving the
  // wizard.
  useEffect(() => {
    if (daemonStatus === 'online') {
      void fetchList();
    }
  }, [daemonStatus, fetchList]);

  if (daemonStatus !== 'online') {
    return (
      <VStack align="stretch" spacing={4}>
        <Box>
          <Heading size="md" mb={1}>
            Choose a backend
          </Heading>
          <Text fontSize="sm" color="text.muted">
            Software backends drive layers, tap-hold, and macros. They live
            inside the daemon process.
          </Text>
        </Box>
        <Alert status="info" borderRadius="md" fontSize="xs">
          <AlertIcon />
          The daemon isn't online yet — start it from the previous step or
          skip to continue with the hardware-only EEPROM path.
        </Alert>
      </VStack>
    );
  }

  return (
    <VStack align="stretch" spacing={4}>
      <Box>
        <Heading size="md" mb={1}>
          Choose a backend
        </Heading>
        <Text fontSize="sm" color="text.muted">
          Pick the software backend you want to drive your keyboard. The
          macOS native backend needs no third-party drivers; kanata gives
          you sub-100ms tap-hold but requires Karabiner-DriverKit on macOS.
        </Text>
      </Box>

      {error && (
        <Alert status="error" borderRadius="md" fontSize="xs">
          <AlertIcon />
          {error}
        </Alert>
      )}

      {loading && backends.length === 0 ? (
        <Text fontSize="xs" color="text.muted">
          Loading backends...
        </Text>
      ) : (
        <VStack align="stretch" spacing={2}>
          {backends.map((b) => (
            <BackendCard
              key={b.id}
              backend={b}
              isActive={b.id === active}
              isSelecting={selecting}
              onSelect={() => {
                clearVerified();
                void select(b.id);
              }}
            />
          ))}
        </VStack>
      )}

      <Box
        border="1px solid"
        borderColor={verifiedBackend === active ? 'success' : 'border.subtle'}
        borderRadius="md"
        p={3}
        bg={verifiedBackend === active ? 'success.subtle' : 'bg.subtle'}
      >
        <Text fontSize="sm" fontWeight={600} mb={1}>
          Verify the binding works
        </Text>
        <Text fontSize="xs" color="text.muted" mb={2}>
          Open any text field, then tap your Caps Lock key. The default
          binding maps tap → Esc and hold → Ctrl. Once you've confirmed
          that, tick the box below to finish setup.
        </Text>
        <Checkbox
          size="sm"
          isChecked={verifiedBackend !== null && verifiedBackend === active}
          isDisabled={active === null}
          onChange={(e) => {
            if (e.target.checked && active) {
              markVerified(active);
            } else {
              clearVerified();
            }
          }}
        >
          <Text fontSize="xs">
            I tested Caps Lock and the {active ?? 'selected'} backend behaves
            as expected.
          </Text>
        </Checkbox>
      </Box>

      <HStack justify="flex-end">
        <Button
          size="xs"
          variant="ghost"
          leftIcon={<RefreshCw size={12} />}
          onClick={() => void fetchList()}
          isLoading={loading}
        >
          Re-check
        </Button>
      </HStack>
    </VStack>
  );
}

function BackendCard({
  backend,
  isActive,
  isSelecting,
  onSelect,
}: {
  backend: BackendInfo;
  isActive: boolean;
  isSelecting: boolean;
  onSelect: () => void;
}) {
  const granted = backend.permission_status.kind === 'granted';
  return (
    <Box
      borderRadius="md"
      borderWidth="1px"
      borderColor={isActive ? 'accent.primary' : 'border.subtle'}
      bg={isActive ? 'bg.subtle' : 'bg.surface'}
      px={3}
      py={2}
    >
      <HStack justify="space-between" align="flex-start" mb={1}>
        <Box>
          <Text fontSize="sm" fontWeight={isActive ? 700 : 500}>
            {backend.human_name}
          </Text>
          <Text fontSize="10px" fontFamily="mono" color="text.muted">
            {backend.id} · tap-hold {backend.capabilities.tap_hold} · layers{' '}
            {backend.capabilities.layers}
          </Text>
        </Box>
        {isActive ? (
          <Text fontSize="10px" fontFamily="mono" color="accent.primary">
            active
          </Text>
        ) : (
          <Button
            size="xs"
            variant="subtle"
            isDisabled={isSelecting}
            isLoading={isSelecting}
            onClick={onSelect}
          >
            Select
          </Button>
        )}
      </HStack>
      {!granted && (
        <PermissionList
          permissions={
            backend.permission_status.kind === 'required'
              ? backend.permission_status.permissions
              : []
          }
        />
      )}
      {granted && (
        <Text fontSize="10px" color="success" mt={1}>
          Permissions granted.
        </Text>
      )}
    </Box>
  );
}

function PermissionList({
  permissions,
}: {
  permissions: RequiredPermission[];
}) {
  if (permissions.length === 0) return null;
  return (
    <VStack align="stretch" spacing={1} mt={1}>
      {permissions.map((p, i) => (
        <PermissionRow key={i} permission={p} />
      ))}
    </VStack>
  );
}

function PermissionRow({ permission }: { permission: RequiredPermission }) {
  switch (permission.type) {
    case 'input_monitoring':
    case 'accessibility': {
      const label =
        permission.type === 'input_monitoring'
          ? 'Input Monitoring'
          : 'Accessibility';
      return (
        <HStack fontSize="11px" color="warning" spacing={2}>
          <Text>Needs {label} permission.</Text>
          <Link href={permission.deep_link} isExternal>
            <HStack spacing={1}>
              <Text>Open System Settings</Text>
              <ExternalLink size={10} />
            </HStack>
          </Link>
        </HStack>
      );
    }
    case 'system_extension':
      return (
        <HStack fontSize="11px" color="warning" spacing={2}>
          <Text>Needs system extension {permission.bundle_id}.</Text>
          {permission.install_command && (
            <Code fontSize="10px" bg="bg.surface">
              {permission.install_command}
            </Code>
          )}
        </HStack>
      );
    case 'user_action':
      return (
        <HStack fontSize="11px" color="warning" spacing={2}>
          <Text>{permission.description}</Text>
          {permission.deep_link && (
            <Link href={permission.deep_link} isExternal>
              <HStack spacing={1}>
                <Text>Open</Text>
                <ExternalLink size={10} />
              </HStack>
            </Link>
          )}
        </HStack>
      );
  }
}
