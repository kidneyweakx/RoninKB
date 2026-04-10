/**
 * Dialog for creating a new profile or saving a hardware snapshot.
 *
 * Invoked from the Header profile menu in two modes:
 *   - "blank"    → createNew(name) with a clean VIA template
 *   - "capture"  → captureHardwareProfile(name) — snapshots the live device
 */

import {
  AlertDialog,
  AlertDialogBody,
  AlertDialogContent,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogOverlay,
  Box,
  Button,
  HStack,
  Input,
  Text,
  VStack,
} from '@chakra-ui/react';
import { useRef, useState } from 'react';
import { Camera, FileText } from 'lucide-react';
import { useDeviceStore } from '../store/deviceStore';
import { useProfileStore } from '../store/profileStore';

export type NewProfileMode = 'blank' | 'capture';

interface Props {
  isOpen: boolean;
  mode: NewProfileMode;
  onClose: () => void;
  onCreated?: (id: string) => void;
}

const MODE_META: Record<
  NewProfileMode,
  { title: string; description: string; placeholder: string; Icon: typeof Camera }
> = {
  blank: {
    title: 'New profile',
    description: 'Create an empty profile. Hardware and software bindings start blank.',
    placeholder: 'e.g. Work, Gaming, Vim…',
    Icon: FileText,
  },
  capture: {
    title: 'Capture hardware snapshot',
    description:
      'Save the current HHKB EEPROM keymap as a new profile — useful before reverting to factory defaults.',
    placeholder: 'e.g. My current layout, Backup 2026-04…',
    Icon: Camera,
  },
};

export function NewProfileDialog({ isOpen, mode, onClose, onCreated }: Props) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const createNew = useProfileStore((s) => s.createNew);
  const captureHardwareProfile = useDeviceStore((s) => s.captureHardwareProfile);
  const deviceStatus = useDeviceStore((s) => s.status);

  const meta = MODE_META[mode];
  const canCapture = mode === 'blank' || deviceStatus === 'connected';

  function handleClose() {
    setName('');
    setError(null);
    onClose();
  }

  async function handleCreate() {
    const trimmed = name.trim();
    if (!trimmed) {
      setError('Profile name is required.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      let profile;
      if (mode === 'capture') {
        profile = await captureHardwareProfile(trimmed);
      } else {
        profile = await createNew(trimmed);
      }
      onCreated?.(profile.id);
      handleClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <AlertDialog
      isOpen={isOpen}
      leastDestructiveRef={cancelRef}
      onClose={handleClose}
      isCentered
    >
      <AlertDialogOverlay>
        <AlertDialogContent
          bg="bg.surface"
          border="1px solid"
          borderColor="border.subtle"
        >
          <AlertDialogHeader fontSize="sm" fontWeight={600} pb={2}>
            <HStack spacing={2}>
              <Box color="accent.primary" display="flex">
                <meta.Icon size={14} />
              </Box>
              <Text>{meta.title}</Text>
            </HStack>
          </AlertDialogHeader>
          <AlertDialogBody>
            <VStack align="stretch" spacing={3}>
              <Text fontSize="xs" color="text.muted" lineHeight="1.6">
                {meta.description}
              </Text>
              {mode === 'capture' && deviceStatus !== 'connected' && (
                <Text
                  fontSize="xs"
                  color="warning"
                  bg="warning.subtle"
                  px={3}
                  py={2}
                  borderRadius="md"
                  border="1px solid"
                  borderColor="warning"
                >
                  Keyboard not connected — snapshot will be empty.
                </Text>
              )}
              <Input
                size="sm"
                placeholder={meta.placeholder}
                value={name}
                onChange={(e) => {
                  setName(e.target.value);
                  setError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void handleCreate();
                }}
                autoFocus
                fontFamily="mono"
              />
              {error && (
                <Text fontSize="xs" color="danger">
                  {error}
                </Text>
              )}
            </VStack>
          </AlertDialogBody>
          <AlertDialogFooter>
            <Button
              ref={cancelRef}
              size="sm"
              variant="ghost"
              onClick={handleClose}
              isDisabled={busy}
            >
              Cancel
            </Button>
            <Button
              size="sm"
              ml={2}
              onClick={() => void handleCreate()}
              isLoading={busy}
              isDisabled={!canCapture}
            >
              Create
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialogOverlay>
    </AlertDialog>
  );
}
