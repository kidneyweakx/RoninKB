import { Box, Heading, Text, VStack } from '@chakra-ui/react';
import { ConnectButton } from './ConnectButton';
import { useDeviceStore } from '../store/deviceStore';

export function EmptyState() {
  const error = useDeviceStore((s) => s.error);

  return (
    <Box
      flex="1"
      display="flex"
      alignItems="center"
      justifyContent="center"
      bg="gray.800"
      borderRadius="lg"
      p={10}
    >
      <VStack spacing={4} maxW="480px" textAlign="center">
        <Heading size="lg">No HHKB connected</Heading>
        <Text color="gray.400">
          Plug in your HHKB Professional Hybrid via USB and click Connect. Chrome
          will prompt you to select the device. WebHID requires HTTPS or localhost.
        </Text>
        <ConnectButton />
        {error && (
          <Text color="red.300" fontSize="sm">
            {error}
          </Text>
        )}
      </VStack>
    </Box>
  );
}
