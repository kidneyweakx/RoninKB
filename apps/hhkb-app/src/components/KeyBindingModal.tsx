/**
 * Modal wrapper around `BindingsPanel` — the software-binding editor.
 *
 * `KeyDetailPanel` opens this when the user clicks the software binding row
 * for a selected key. The selected key index is forwarded so the panel opens
 * that key in edit mode immediately.
 */

import {
  Box,
  Modal,
  ModalBody,
  ModalCloseButton,
  ModalContent,
  ModalHeader,
  ModalOverlay,
  Tag,
  Text,
  HStack,
} from '@chakra-ui/react';
import { Layers } from 'lucide-react';
import { BindingsPanel } from './BindingsPanel';
import { HHKB_LAYOUT } from '../data/hhkbLayout';

interface Props {
  isOpen: boolean;
  onClose: () => void;
  keyIndex: number;
}

export function KeyBindingModal({ isOpen, onClose, keyIndex }: Props) {
  const keyMeta = HHKB_LAYOUT.find((k) => k.index === keyIndex);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      size="lg"
      scrollBehavior="inside"
      isCentered
    >
      <ModalOverlay backdropFilter="blur(4px)" />
      <ModalContent bg="bg.surface" border="1px solid" borderColor="border.subtle">
        <ModalHeader pb={2}>
          <HStack spacing={2}>
            <Box color="accent.primary" display="flex">
              <Layers size={16} />
            </Box>
            <Text fontSize="sm" fontWeight={600} color="text.primary">
              Software Bindings
            </Text>
            <Tag size="sm" variant="accent">
              <Text fontSize="10px" fontFamily="mono">
                #{keyIndex}
                {keyMeta ? ` — ${keyMeta.label}` : ''}
              </Text>
            </Tag>
          </HStack>
          <Text fontSize="11px" color="text.muted" fontFamily="mono" mt={1.5}>
            Writes to the active profile's kanata config. Hardware EEPROM is not touched.
          </Text>
        </ModalHeader>
        <ModalCloseButton />
        <ModalBody pb={5}>
          <BindingsPanel
            focusKeyIndex={keyIndex}
            onSaved={onClose}
            onCancel={onClose}
          />
        </ModalBody>
      </ModalContent>
    </Modal>
  );
}
