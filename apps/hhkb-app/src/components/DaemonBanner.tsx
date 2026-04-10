import { Box, Button, HStack, Text } from '@chakra-ui/react';
import { useDaemonStore } from '../store/daemonStore';

export function DaemonBanner() {
  const status = useDaemonStore((s) => s.status);
  const bannerDismissed = useDaemonStore((s) => s.bannerDismissed);
  const dismiss = useDaemonStore((s) => s.dismissBanner);

  if (status !== 'offline' || bannerDismissed) return null;

  return (
    <Box
      bg="yellow.600"
      color="gray.900"
      px={6}
      py={3}
      borderBottom="1px solid"
      borderColor="yellow.700"
    >
      <HStack justify="space-between">
        <Text fontSize="sm" fontWeight="semibold">
          Install the RoninKB Daemon for software macros and Flow clipboard
          sync. Basic keymap editing works without it.
        </Text>
        <HStack spacing={2}>
          <Button
            size="sm"
            variant="outline"
            colorScheme="blackAlpha"
            onClick={dismiss}
          >
            Skip
          </Button>
          <Button
            size="sm"
            colorScheme="blackAlpha"
            onClick={() => {
              window.open(
                'https://github.com/roninkb/roninKB/releases',
                '_blank',
              );
            }}
          >
            Download
          </Button>
        </HStack>
      </HStack>
    </Box>
  );
}
