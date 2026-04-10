/**
 * Collapsible daemon event log — a tucked-away developer debug panel.
 */

import { useState } from 'react';
import { Box, Button, Flex, HStack, Text, VStack } from '@chakra-ui/react';
import { ChevronUp, ChevronDown, Terminal, Trash2 } from 'lucide-react';
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
      bottom={4}
      right={4}
      bg="rgba(19, 19, 22, 0.9)"
      backdropFilter="blur(12px)"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="lg"
      boxShadow="elevated"
      minW="240px"
      maxW="380px"
      zIndex={40}
      overflow="hidden"
    >
      <Flex
        as="button"
        w="100%"
        px={3}
        py={2}
        align="center"
        justify="space-between"
        onClick={() => setOpen((v) => !v)}
        _hover={{ bg: 'bg.subtle' }}
        transition="background-color 0.15s ease"
      >
        <HStack spacing={2}>
          <Box color="text.muted" display="flex">
            <Terminal size={12} />
          </Box>
          <Text
            fontSize="10px"
            color="text.secondary"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.08em"
          >
            events
          </Text>
          <Text fontSize="10px" color="text.muted" fontFamily="mono">
            {events.length}
          </Text>
        </HStack>
        <Box color="text.muted" display="flex">
          {open ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
        </Box>
      </Flex>

      {open && (
        <Box borderTop="1px solid" borderColor="border.subtle">
          <Flex
            align="center"
            justify="space-between"
            px={3}
            py={1.5}
            borderBottom="1px solid"
            borderColor="border.subtle"
          >
            <HStack spacing={1}>
              <Box
                w="6px"
                h="6px"
                borderRadius="full"
                bg={
                  status === 'online'
                    ? 'success'
                    : status === 'offline'
                      ? 'danger'
                      : 'warning'
                }
              />
              <Text
                fontSize="10px"
                color="text.muted"
                fontFamily="mono"
              >
                {status}
              </Text>
            </HStack>
            <Button
              size="xs"
              variant="ghost"
              leftIcon={<Trash2 size={10} />}
              onClick={() => useDaemonStore.setState({ events: [] })}
              h="20px"
              fontSize="10px"
              px={1.5}
            >
              clear
            </Button>
          </Flex>
          <Box px={3} py={2} maxH="240px" overflowY="auto">
            {events.length === 0 ? (
              <Text
                fontSize="10px"
                color="text.muted"
                fontFamily="mono"
                textAlign="center"
                py={3}
              >
                (no events yet)
              </Text>
            ) : (
              <VStack align="stretch" spacing={0.5}>
                {events.map((e, i) => (
                  <Text
                    key={i}
                    fontSize="10px"
                    fontFamily="mono"
                    color="text.secondary"
                    px={1.5}
                    py={0.5}
                    borderRadius="sm"
                    _hover={{ bg: 'bg.subtle', color: 'text.primary' }}
                    transition="background-color 0.15s ease, color 0.15s ease"
                  >
                    <Text
                      as="span"
                      color="text.muted"
                      mr={2}
                    >{`[${String(i).padStart(3, '0')}]`}</Text>
                    {formatEvent(e)}
                  </Text>
                ))}
              </VStack>
            )}
          </Box>
        </Box>
      )}
    </Box>
  );
}
