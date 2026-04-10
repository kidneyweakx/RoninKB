import { Box, Heading, HStack, Text, VStack } from '@chakra-ui/react';
import { CheckCircle2, XCircle, Chrome } from 'lucide-react';

export function StepBrowser() {
  const hasHid =
    typeof navigator !== 'undefined' && 'hid' in navigator;

  return (
    <VStack align="stretch" spacing={5}>
      <Box>
        <Heading size="md" mb={1}>
          Browser check
        </Heading>
        <Text fontSize="sm" color="text.muted">
          RoninKB speaks to the keyboard via the WebHID API.
        </Text>
      </Box>

      <Box
        border="1px solid"
        borderColor={hasHid ? 'success' : 'danger'}
        borderRadius="md"
        p={4}
        bg={hasHid ? 'success.subtle' : 'danger.subtle'}
      >
        <HStack spacing={3}>
          <Box color={hasHid ? 'success' : 'danger'}>
            {hasHid ? <CheckCircle2 size={24} /> : <XCircle size={24} />}
          </Box>
          <Box>
            <Text fontSize="sm" fontWeight={600}>
              {hasHid ? 'WebHID ready' : 'WebHID unavailable'}
            </Text>
            <Text fontSize="xs" color="text.muted">
              {hasHid
                ? 'Your browser supports direct USB HID access.'
                : 'Use Chrome or Edge (or another Chromium-based browser) to enable direct keyboard access.'}
            </Text>
          </Box>
        </HStack>
      </Box>

      {!hasHid && (
        <HStack
          spacing={2}
          p={3}
          bg="bg.subtle"
          border="1px solid"
          borderColor="border.subtle"
          borderRadius="md"
        >
          <Chrome size={14} />
          <Text fontSize="xs" color="text.muted">
            You can still continue — the daemon can proxy HID access over
            the local REST API if installed.
          </Text>
        </HStack>
      )}
    </VStack>
  );
}
