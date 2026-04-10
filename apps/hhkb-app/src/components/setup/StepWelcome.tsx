import { Box, HStack, Heading, Icon, List, ListIcon, ListItem, Text, VStack } from '@chakra-ui/react';
import { CheckCircle2, Keyboard, Layers, Sparkles } from 'lucide-react';

export function StepWelcome() {
  return (
    <VStack align="stretch" spacing={5}>
      <Box>
        <Heading size="md" mb={1}>
          Welcome to RoninKB
        </Heading>
        <Text fontSize="sm" color="text.muted">
          An open-source configurator for the HHKB Professional Hybrid.
        </Text>
      </Box>
      <List spacing={3} fontSize="sm">
        <ListItem>
          <ListIcon as={Keyboard} color="accent.primary" />
          Remap any key on the physical keyboard via WebHID — no install
          required.
        </ListItem>
        <ListItem>
          <ListIcon as={Layers} color="accent.primary" />
          Layer software macros, tap-hold, and sequences on top using the
          optional daemon + kanata engine.
        </ListItem>
        <ListItem>
          <ListIcon as={Sparkles} color="accent.primary" />
          Store and sync profiles as VIA-compatible JSON, with RoninKB
          extensions preserved losslessly.
        </ListItem>
      </List>
      <Box
        bg="bg.subtle"
        border="1px solid"
        borderColor="border.subtle"
        borderRadius="md"
        p={3}
      >
        <HStack spacing={1.5} align="flex-start">
          <Icon as={CheckCircle2} boxSize={3} mt="2px" color="text.muted" />
          <Text fontSize="xs" color="text.muted">
            Takes about a minute. You can re-run this wizard any time from
            Settings.
          </Text>
        </HStack>
      </Box>
    </VStack>
  );
}
