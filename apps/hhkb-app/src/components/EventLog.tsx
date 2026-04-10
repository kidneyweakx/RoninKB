/**
 * Collapsible daemon event log — used during development to confirm
 * WebSocket events arrive as expected.
 */

import { useState } from 'react';
import { Box, Button, HStack, Text, VStack, Code } from '@chakra-ui/react';
import { useDaemonStore } from '../store/daemonStore';

function formatEvent(e: { type: string } & Record<string, unknown>): string {
  if (e.type === 'profile_changed' && 'id' in e) {
    return `profile_changed id=${String(e.id)}`;
  }
  return e.type;
}

export function EventLog() {
  const events = useDaemonStore((s) => s.events);
  const status = useDaemonStore((s) => s.status);
  const [open, setOpen] = useState(false);

  return (
    <Box
      position="fixed"
      bottom={0}
      right={0}
      m={3}
      bg="gray.800"
      border="1px solid"
      borderColor="gray.700"
      borderRadius="md"
      boxShadow="lg"
      minW="220px"
      maxW="360px"
      zIndex={100}
    >
      <HStack
        as="button"
        w="100%"
        px={3}
        py={2}
        onClick={() => setOpen((v) => !v)}
        justify="space-between"
      >
        <Text fontSize="xs" color="gray.300" fontWeight="semibold">
          Daemon events ({events.length})
        </Text>
        <Text fontSize="xs" color="gray.500">
          {open ? 'hide' : 'show'}
        </Text>
      </HStack>
      {open && (
        <Box px={3} pb={3} maxH="240px" overflowY="auto">
          <Text fontSize="xs" color="gray.500" mb={2}>
            status: {status}
          </Text>
          {events.length === 0 ? (
            <Text fontSize="xs" color="gray.500">
              (no events yet)
            </Text>
          ) : (
            <VStack align="stretch" spacing={1}>
              {events.map((e, i) => (
                <Code
                  key={i}
                  fontSize="xs"
                  bg="gray.900"
                  color="gray.200"
                  px={2}
                  py={1}
                >
                  {formatEvent(e)}
                </Code>
              ))}
            </VStack>
          )}
          <Button
            mt={2}
            size="xs"
            variant="ghost"
            onClick={() => useDaemonStore.setState({ events: [] })}
          >
            clear
          </Button>
        </Box>
      )}
    </Box>
  );
}
