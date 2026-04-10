import { Box, Button, Flex, HStack, IconButton, Text } from '@chakra-ui/react';
import { Info, X, ExternalLink } from 'lucide-react';
import { useDaemonStore } from '../store/daemonStore';

export function DaemonBanner() {
  const status = useDaemonStore((s) => s.status);
  const bannerDismissed = useDaemonStore((s) => s.bannerDismissed);
  const dismiss = useDaemonStore((s) => s.dismissBanner);

  if (status !== 'offline' || bannerDismissed) return null;

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
        borderLeftColor="info"
        borderLeftWidth="3px"
      >
        <Box color="info" display="flex" flexShrink={0}>
          <Info size={16} />
        </Box>
        <Box flex="1" minW={0}>
          <Text fontSize="xs" color="text.primary" fontWeight={500}>
            Optional: Install the RoninKB daemon for macros and clipboard
            sync
          </Text>
          <Text fontSize="11px" color="text.muted">
            Basic keymap editing works without it.
          </Text>
        </Box>
        <HStack spacing={1} flexShrink={0}>
          <Button
            size="xs"
            variant="ghost"
            rightIcon={<ExternalLink size={12} />}
            onClick={() => {
              window.open(
                'https://github.com/roninkb/roninKB/releases',
                '_blank',
              );
            }}
          >
            Download
          </Button>
          <IconButton
            aria-label="Dismiss"
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
