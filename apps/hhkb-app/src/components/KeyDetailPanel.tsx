import {
  AlertDialog,
  AlertDialogBody,
  AlertDialogContent,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogOverlay,
  Box,
  Heading,
  Text,
  VStack,
  HStack,
  Divider,
  Button,
  Flex,
  Tag,
  Checkbox,
} from '@chakra-ui/react';
import { useRef, useState } from 'react';
import {
  Keyboard as KeyboardIcon,
  Code2,
  Pencil,
  X,
  MousePointerClick,
  AlertTriangle,
} from 'lucide-react';
import { useDeviceStore } from '../store/deviceStore';
import { useUiStore } from '../store/uiStore';
import { HHKB_LAYOUT } from '../data/hhkbLayout';
import { KeycodePicker } from './KeycodePicker';
import { LayerOriginLabel } from './LayerOriginLabel';
import { KeyBindingModal } from './KeyBindingModal';
import { useKeyOrigin, useSoftwareTokenAt } from '../hooks/useKeyOrigin';
import { describeToken } from '../hhkb/keyBindings';

interface Props {
  selectedIndex: number | null;
  layer: 'base' | 'fn';
}

export function KeyDetailPanel({ selectedIndex, layer }: Props) {
  const baseKeymap = useDeviceStore((s) => s.baseKeymap);
  const fnKeymap = useDeviceStore((s) => s.fnKeymap);
  const setKeyOverride = useDeviceStore((s) => s.setKeyOverride);

  const origin = useKeyOrigin(selectedIndex, layer);
  const swToken = useSoftwareTokenAt(selectedIndex);

  const [picking, setPicking] = useState(false);
  const [bindingModal, setBindingModal] = useState(false);

  // Hardware-edit confirmation state.
  const hwEditAcknowledged = useUiStore((s) => s.hwEditAcknowledged);
  const acknowledgeHwEdit = useUiStore((s) => s.acknowledgeHwEdit);
  const [pendingHwEdit, setPendingHwEdit] = useState<number | null>(null);
  const [dontAskAgain, setDontAskAgain] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);

  if (selectedIndex === null) {
    return (
      <Panel>
        <Flex
          direction="column"
          align="center"
          justify="center"
          h="100%"
          color="text.muted"
          textAlign="center"
          py={12}
          gap={3}
        >
          <Box
            w="56px"
            h="56px"
            borderRadius="lg"
            bg="bg.subtle"
            border="1px solid"
            borderColor="border.subtle"
            display="flex"
            alignItems="center"
            justifyContent="center"
            color="text.muted"
          >
            <MousePointerClick size={24} />
          </Box>
          <VStack spacing={1}>
            <Text fontSize="sm" fontWeight={500} color="text.secondary">
              No key selected
            </Text>
            <Text fontSize="xs" color="text.muted" maxW="220px">
              Click any key on the keyboard to inspect and remap it.
            </Text>
          </VStack>
        </Flex>
      </Panel>
    );
  }

  const meta = HHKB_LAYOUT.find((k) => k.index === selectedIndex);
  const keymap = layer === 'base' ? baseKeymap : fnKeymap;
  const currentCode = keymap?.get(selectedIndex) ?? 0;
  const isDefault = currentCode === 0;
  const hexCode = `0x${currentCode.toString(16).padStart(2, '0').toUpperCase()}`;

  /**
   * Apply a hardware-layer keycode edit. The store update is local-only;
   * the actual EEPROM flash happens via `SyncBanner`'s "Resync now" button
   * (which calls `writeKeymaps` under the hood). If this is the first edit
   * this session we pop a confirmation modal so the user understands the
   * edit path.
   */
  function applyHwEdit(code: number) {
    if (!keymap) return;
    if (!hwEditAcknowledged) {
      setPendingHwEdit(code);
      return;
    }
    setKeyOverride(selectedIndex!, code, layer);
    setPicking(false);
  }

  function confirmHwEdit() {
    if (pendingHwEdit === null) return;
    if (dontAskAgain) acknowledgeHwEdit();
    setKeyOverride(selectedIndex!, pendingHwEdit, layer);
    setPendingHwEdit(null);
    setPicking(false);
  }

  return (
    <Panel>
      <VStack align="stretch" spacing={0} h="100%">
        {/* Header — key identity */}
        <Box px={5} pt={5} pb={4}>
          <HStack spacing={2} mb={3}>
            {origin ? (
              <LayerOriginLabel origin={origin} />
            ) : (
              <Tag size="sm" variant="subtle">
                <Text fontSize="10px" fontFamily="mono">
                  [未定義]
                </Text>
              </Tag>
            )}
            <Tag size="sm" variant={layer === 'fn' ? 'accent' : 'subtle'}>
              <Text
                fontSize="10px"
                fontFamily="mono"
                textTransform="uppercase"
                letterSpacing="0.06em"
              >
                {layer} layer
              </Text>
            </Tag>
            <Tag size="sm" variant="subtle">
              <Text fontSize="10px" fontFamily="mono">
                #{selectedIndex}
              </Text>
            </Tag>
          </HStack>

          <Flex align="center" justify="space-between" gap={3}>
            <Box>
              <Text
                fontSize="10px"
                color="text.muted"
                fontFamily="mono"
                textTransform="uppercase"
                letterSpacing="0.08em"
                mb={1}
              >
                Key
              </Text>
              <Heading
                size="lg"
                fontFamily="mono"
                letterSpacing="-0.02em"
                lineHeight="1"
              >
                {meta?.label ?? `#${selectedIndex}`}
              </Heading>
            </Box>
            <Box textAlign="right">
              <Text
                fontSize="10px"
                color="text.muted"
                fontFamily="mono"
                textTransform="uppercase"
                letterSpacing="0.08em"
                mb={1}
              >
                HID
              </Text>
              <Text
                fontFamily="mono"
                fontSize="lg"
                fontWeight={500}
                color={isDefault ? 'text.muted' : 'accent.primary'}
                lineHeight="1"
              >
                {isDefault ? '—' : hexCode}
              </Text>
            </Box>
          </Flex>
        </Box>

        <Divider />

        {/* Binding rows — unified hardware + software view */}
        <Box px={5} py={4}>
          <Text
            fontSize="10px"
            color="text.muted"
            fontFamily="mono"
            textTransform="uppercase"
            letterSpacing="0.08em"
            mb={2}
          >
            Binding
          </Text>
          <VStack align="stretch" spacing={1.5}>
            <BindingRow
              icon={<KeyboardIcon size={14} />}
              label="Hardware (EEPROM)"
              value={isDefault ? 'default' : hexCode}
              active={origin === 'hw'}
              onClick={() => setPicking((v) => !v)}
            />
            <BindingRow
              icon={<Code2 size={14} />}
              label="Software (kanata)"
              value={swToken ? describeToken(swToken.token) : 'not set'}
              active={origin === 'sw'}
              onClick={() => setBindingModal(true)}
            />
          </VStack>

          {/* Hardware-write warning badge — shown whenever we're inspecting
              a key whose binding lives in (or would live in) the EEPROM. */}
          {(origin === 'hw' || origin === null || picking) && (
            <HStack
              mt={3}
              spacing={2}
              px={3}
              py={2}
              bg="warning.subtle"
              border="1px solid"
              borderColor="warning"
              borderRadius="md"
            >
              <Box color="warning" display="flex" flexShrink={0}>
                <AlertTriangle size={12} />
              </Box>
              <Text fontSize="10px" color="text.secondary" lineHeight="1.4">
                硬體層編輯會寫入 HHKB EEPROM。改動先進入本機快取,按
                <Text as="span" fontFamily="mono" mx={1}>
                  Resync now
                </Text>
                才會燒進鍵盤。
              </Text>
            </HStack>
          )}
        </Box>

        <Divider />

        {/* Hardware-edit action — shows the KeycodePicker inline */}
        <Box px={5} py={4}>
          <HStack spacing={2}>
            <Button
              leftIcon={picking ? <X size={14} /> : <Pencil size={14} />}
              variant={picking ? 'outline' : 'solid'}
              size="sm"
              onClick={() => setPicking((v) => !v)}
              isDisabled={!keymap}
              flex="1"
            >
              {picking ? 'Cancel' : 'Remap hardware key'}
            </Button>
            {!isDefault && !picking && origin === 'hw' && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => applyHwEdit(0)}
              >
                Reset
              </Button>
            )}
          </HStack>

          {picking && (
            <Box mt={3}>
              <KeycodePicker onPick={(code) => applyHwEdit(code)} />
            </Box>
          )}
        </Box>

        <Box flex="1" />

        <Box px={5} pb={4}>
          <Text
            fontSize="10px"
            color="text.muted"
            fontFamily="mono"
            lineHeight="1.6"
          >
            Layers: [硬體] EEPROM • [本機] kanata overlay • [Flow] cross-device
          </Text>
        </Box>
      </VStack>

      {/* Software-binding editor modal */}
      {bindingModal && (
        <KeyBindingModal
          isOpen={bindingModal}
          onClose={() => setBindingModal(false)}
          keyIndex={selectedIndex}
        />
      )}

      {/* First-time hardware-edit confirmation */}
      <AlertDialog
        isOpen={pendingHwEdit !== null}
        leastDestructiveRef={cancelRef}
        onClose={() => setPendingHwEdit(null)}
        isCentered
      >
        <AlertDialogOverlay>
          <AlertDialogContent
            bg="bg.surface"
            border="1px solid"
            borderColor="warning"
          >
            <AlertDialogHeader fontSize="sm" fontWeight={600} pb={2}>
              <HStack spacing={2}>
                <Box color="warning" display="flex">
                  <AlertTriangle size={14} />
                </Box>
                <Text>確定要改硬體層嗎?</Text>
              </HStack>
            </AlertDialogHeader>
            <AlertDialogBody fontSize="xs" color="text.secondary">
              <VStack align="stretch" spacing={2}>
                <Text lineHeight="1.6">
                  這會把 key #{selectedIndex} 改成新的 HID code,寫入 HHKB
                  EEPROM(拔掉 USB 也會留著)。
                </Text>
                <Text lineHeight="1.6">
                  改動會先存在本機快取,等你按 header 的
                  <Text as="span" fontFamily="mono" mx={1}>
                    Resync now
                  </Text>
                  才真的寫入鍵盤。想要只改軟體層請改用 Tap-Hold / Macro /
                  Layer 編輯器(走 kanata,不會碰 EEPROM)。
                </Text>
                <Checkbox
                  mt={2}
                  size="sm"
                  isChecked={dontAskAgain}
                  onChange={(e) => setDontAskAgain(e.target.checked)}
                >
                  <Text fontSize="xs">這次 session 不要再問我</Text>
                </Checkbox>
              </VStack>
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button
                ref={cancelRef}
                size="sm"
                variant="ghost"
                onClick={() => setPendingHwEdit(null)}
              >
                取消
              </Button>
              <Button
                size="sm"
                colorScheme="orange"
                ml={2}
                onClick={confirmHwEdit}
              >
                確認寫入
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    </Panel>
  );
}

function Panel({ children }: { children: React.ReactNode }) {
  return (
    <Box
      bg="bg.surface"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="xl"
      h="100%"
      overflow="hidden"
    >
      {children}
    </Box>
  );
}

function BindingRow({
  icon,
  label,
  value,
  active = false,
  disabled = false,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}) {
  const clickable = !!onClick && !disabled;
  return (
    <Box
      role={clickable ? 'button' : undefined}
      tabIndex={clickable ? 0 : undefined}
      onClick={clickable ? onClick : undefined}
      onKeyDown={
        clickable
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onClick?.();
              }
            }
          : undefined
      }
      px={3}
      py={2}
      borderRadius="md"
      bg={active ? 'accent.subtle' : 'bg.subtle'}
      border="1px solid"
      borderColor={active ? 'accent.primary' : 'border.subtle'}
      opacity={disabled ? 0.5 : 1}
      cursor={clickable ? 'pointer' : 'default'}
      _hover={
        clickable
          ? { borderColor: active ? 'accent.primary' : 'border.strong' }
          : undefined
      }
      transition="background-color 0.15s ease, border-color 0.15s ease"
      textAlign="left"
      w="100%"
    >
      <HStack justify="space-between" spacing={2}>
        <HStack spacing={2} minW={0}>
          <Box
            color={active ? 'accent.primary' : 'text.muted'}
            display="flex"
            flexShrink={0}
          >
            {icon}
          </Box>
          <Text
            fontSize="xs"
            color="text.primary"
            fontWeight={500}
            noOfLines={1}
          >
            {label}
          </Text>
        </HStack>
        <Text
          fontSize="xs"
          color={active ? 'accent.primary' : 'text.muted'}
          fontFamily="mono"
          noOfLines={1}
          maxW="160px"
        >
          {value}
        </Text>
      </HStack>
    </Box>
  );
}
