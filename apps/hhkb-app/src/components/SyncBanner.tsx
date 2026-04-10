import { Box, Button, Flex, HStack, IconButton, Text, useToast } from '@chakra-ui/react';
import { AlertTriangle, RotateCw, X } from 'lucide-react';
import { useDeviceStore } from '../store/deviceStore';
import { useProfileStore } from '../store/profileStore';
import { useUiStore } from '../store/uiStore';
import { isInSync } from '../hhkb/keymapDiff';

/**
 * Sticky banner shown when the active profile's stored hardware layers
 * drift from what's actually on the physical keyboard. Gives the user
 * a one-click "Resync now" that pushes the current in-memory keymap
 * (which the profile considers canonical) back to the device.
 *
 * Dismissal is session-only. The `uiStore` resets dismissal whenever
 * the device keymap object identity changes (see store/uiStore.ts).
 */
export function SyncBanner() {
  const toast = useToast();
  const baseKeymap = useDeviceStore((s) => s.baseKeymap);
  const fnKeymap = useDeviceStore((s) => s.fnKeymap);
  const writeKeymaps = useDeviceStore((s) => s.writeKeymaps);
  const deviceStatus = useDeviceStore((s) => s.status);
  const activeProfile = useProfileStore((s) => s.getActive)();
  const dismissed = useUiStore((s) => s.syncBannerDismissed);
  const dismiss = useUiStore((s) => s.dismissSyncBanner);

  if (dismissed) return null;
  if (deviceStatus !== 'connected') return null;
  if (!baseKeymap || !fnKeymap) return null;

  const rawLayers = activeProfile?.via._roninKB?.hardware?.raw_layers;
  if (!rawLayers) return null;

  const inSync = isInSync(
    baseKeymap.asBytes(),
    fnKeymap.asBytes(),
    rawLayers,
  );
  if (inSync) return null;

  async function handleResync() {
    try {
      await writeKeymaps();
      toast({
        title: 'Hardware re-synced',
        description: `Wrote keymap to device`,
        status: 'success',
        duration: 3000,
      });
      dismiss();
    } catch (e) {
      toast({
        title: 'Resync failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 5000,
      });
    }
  }

  return (
    <Box mx={4} mt={3}>
      <Flex
        align="center"
        gap={3}
        px={4}
        py={2.5}
        bg="bg.surface"
        border="1px solid"
        borderColor="border.subtle"
        borderRadius="lg"
        borderLeftColor="warning"
        borderLeftWidth="3px"
      >
        <Box color="warning" display="flex" flexShrink={0}>
          <AlertTriangle size={16} />
        </Box>
        <Box flex="1" minW={0}>
          <Text fontSize="xs" color="text.primary" fontWeight={500}>
            Hardware differs from active profile
          </Text>
          <Text fontSize="11px" color="text.muted">
            The physical keyboard has keycodes that don't match{' '}
            <Text as="span" fontFamily="mono" color="text.secondary">
              {activeProfile?.name ?? 'active profile'}
            </Text>
            .
          </Text>
        </Box>
        <HStack spacing={1} flexShrink={0}>
          <Button
            size="xs"
            variant="solid"
            leftIcon={<RotateCw size={12} />}
            onClick={() => {
              void handleResync();
            }}
          >
            Resync now
          </Button>
          <IconButton
            aria-label="Dismiss sync banner"
            icon={<X size={14} />}
            size="xs"
            variant="ghost"
            onClick={dismiss}
          />
        </HStack>
      </Flex>
    </Box>
  );
}
