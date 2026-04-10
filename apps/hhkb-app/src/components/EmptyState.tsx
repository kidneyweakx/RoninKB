import {
  Box,
  Flex,
  Heading,
  Text,
  VStack,
  HStack,
  Kbd,
} from '@chakra-ui/react';
import { Keyboard, AlertTriangle } from 'lucide-react';
import { ConnectButton } from './ConnectButton';
import { useDeviceStore } from '../store/deviceStore';

export function EmptyState() {
  const error = useDeviceStore((s) => s.error);

  return (
    <Flex
      flex="1"
      align="center"
      justify="center"
      bg="bg.surface"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="xl"
      p={{ base: 8, md: 16 }}
      position="relative"
      overflow="hidden"
    >
      {/* Decorative grid background */}
      <Box
        position="absolute"
        inset={0}
        opacity={0.4}
        pointerEvents="none"
        backgroundImage={`linear-gradient(to right, rgba(255,255,255,0.02) 1px, transparent 1px),
          linear-gradient(to bottom, rgba(255,255,255,0.02) 1px, transparent 1px)`}
        backgroundSize="40px 40px"
        sx={{
          maskImage:
            'radial-gradient(ellipse at center, black 30%, transparent 70%)',
        }}
      />

      <VStack spacing={6} maxW="440px" textAlign="center" position="relative">
        <Box
          w="72px"
          h="72px"
          borderRadius="2xl"
          bg="accent.subtle"
          border="1px solid"
          borderColor="accent.primary"
          display="flex"
          alignItems="center"
          justifyContent="center"
          color="accent.primary"
          boxShadow="glow"
        >
          <Keyboard size={32} strokeWidth={1.75} />
        </Box>

        <VStack spacing={2}>
          <Heading size="md" letterSpacing="-0.02em">
            No keyboard connected
          </Heading>
          <Text fontSize="sm" color="text.secondary" lineHeight="1.6">
            Plug in your HHKB Professional Hybrid via USB-C and authorize
            WebHID access. RoninKB talks to the keyboard directly — no
            drivers, no cloud.
          </Text>
        </VStack>

        <ConnectButton />

        <HStack
          spacing={2}
          fontSize="11px"
          color="text.muted"
          fontFamily="mono"
        >
          <Text>Requires</Text>
          <Kbd fontSize="10px">HTTPS</Kbd>
          <Text>or</Text>
          <Kbd fontSize="10px">localhost</Kbd>
        </HStack>

        {error && (
          <HStack
            spacing={2}
            px={3}
            py={2}
            bg="danger.subtle"
            border="1px solid"
            borderColor="danger"
            borderRadius="md"
            color="danger"
            fontSize="xs"
            maxW="360px"
          >
            <AlertTriangle size={14} />
            <Text fontFamily="mono">{error}</Text>
          </HStack>
        )}
      </VStack>
    </Flex>
  );
}
