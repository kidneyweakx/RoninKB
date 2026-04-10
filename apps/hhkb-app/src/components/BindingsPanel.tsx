/**
 * VSCode-style software bindings editor.
 *
 * Shows all software (kanata) remaps for the active profile as a flat,
 * human-readable list. Each row can be edited inline — no tabs, no kanata
 * syntax visible. Saves the generated config to the profile and triggers a
 * hot-reload of kanata.
 *
 * Usage:
 *   <BindingsPanel focusKeyIndex={30} onSaved={close} />
 *
 * If `focusKeyIndex` is provided the panel opens that key's binding in edit
 * mode (or adds a new one if the key has no existing binding).
 */

import { useMemo, useState } from 'react';
import {
  Alert,
  AlertIcon,
  Box,
  Button,
  Flex,
  HStack,
  IconButton,
  Input,
  Select,
  Tag,
  Text,
  useToast,
  VStack,
} from '@chakra-ui/react';
import { Check, Pencil, Plus, Trash2, X } from 'lucide-react';
import { HHKB_LAYOUT } from '../data/hhkbLayout';
import { useDaemonStore } from '../store/daemonStore';
import { useProfileStore } from '../store/profileStore';
import {
  KEYCODES,
  generateKanataConfig,
  parseKeyBindings,
  tokenToLabel,
  type KeyBinding,
  type LayerSwitchBinding,
  type RemapBinding,
  type TapHoldBinding,
} from '../hhkb/keyBindings';
import type { RoninExtension, ViaProfile } from '../hhkb/via';

// ─── Helpers ──────────────────────────────────────────────────────────────────

function hhkbKeyLabel(index: number): string {
  return HHKB_LAYOUT.find((k) => k.index === index)?.label ?? `#${index}`;
}

function bindingDescription(b: KeyBinding): string {
  switch (b.type) {
    case 'remap':
      return tokenToLabel(b.target);
    case 'tap-hold':
      return `${tokenToLabel(b.tap)}  ·  ${tokenToLabel(b.hold)} (hold)`;
    case 'layer-switch':
      return b.mode === 'toggle' ? `Toggle → ${b.layerName}` : `Hold → ${b.layerName}`;
  }
}

const TYPE_LABELS: Record<KeyBinding['type'], string> = {
  remap: 'Remap',
  'tap-hold': 'Tap-Hold',
  'layer-switch': 'Layer',
};

/** Stamp a new kanata config onto the profile's `_roninKB.software` extension. */
function applyConfigToProfile(via: ViaProfile, config: string): ViaProfile {
  const cloned: ViaProfile = JSON.parse(JSON.stringify(via)) as ViaProfile;
  const ext: RoninExtension = cloned._roninKB ?? {
    version: '1',
    profile: { id: 'local', name: cloned.name },
  };
  ext.software = {
    engine: ext.software?.engine ?? 'kanata',
    engine_version: ext.software?.engine_version,
    config,
  };
  cloned._roninKB = ext;
  return cloned;
}

// ─── Sentinel ─────────────────────────────────────────────────────────────────

/** Magic value for `editingKey` meaning "the add-new row". */
const NEW_ROW = -1;

// ─── Props ────────────────────────────────────────────────────────────────────

interface Props {
  /**
   * When provided, the panel opens with that key's binding row in edit mode.
   * If no binding exists for the key yet, a new one is prepared.
   */
  focusKeyIndex?: number;
  onSaved?: () => void;
  onCancel?: () => void;
}

// ─── Root component ───────────────────────────────────────────────────────────

export function BindingsPanel({ focusKeyIndex, onSaved, onCancel }: Props) {
  const toast = useToast();
  const activeProfile = useProfileStore((s) => s.getActive)();
  const daemonStatus = useDaemonStore((s) => s.status);
  const daemonClient = useDaemonStore((s) => s.client);

  const initialConfig = useMemo(
    () => activeProfile?.via._roninKB?.software?.config ?? '',
    [activeProfile],
  );

  // ── local state ─────────────────────────────────────────────────────────────

  const [bindings, setBindings] = useState<KeyBinding[]>(() =>
    parseKeyBindings(initialConfig),
  );

  /**
   * Which row is currently in edit mode.
   * `null`    → no row being edited
   * `NEW_ROW` → the "add new binding" row
   * `n`       → the row with `sourceIndex === n`
   */
  const [editingKey, setEditingKey] = useState<number | null>(() => {
    if (focusKeyIndex == null) return null;
    return focusKeyIndex;
  });

  const [draft, setDraft] = useState<KeyBinding | null>(() => {
    if (focusKeyIndex == null) return null;
    const existing = parseKeyBindings(initialConfig).find(
      (b) => b.sourceIndex === focusKeyIndex,
    );
    if (existing) return { ...existing };
    // Default for new: tap-hold is the most useful starting point
    return { type: 'tap-hold', sourceIndex: focusKeyIndex, tap: 'a', hold: 'lctl', timeout: 200 };
  });

  const [saving, setSaving] = useState(false);

  // ── edit helpers ─────────────────────────────────────────────────────────────

  function openEdit(b: KeyBinding) {
    setEditingKey(b.sourceIndex);
    setDraft({ ...b });
  }

  function openAdd() {
    const usedSet = new Set(bindings.map((b) => b.sourceIndex));
    const firstFree =
      [...HHKB_LAYOUT]
        .sort((a, b) => a.index - b.index)
        .find((k) => !usedSet.has(k.index))?.index ?? 1;
    setDraft({ type: 'remap', sourceIndex: firstFree, target: 'a' });
    setEditingKey(NEW_ROW);
  }

  function confirmDraft() {
    if (!draft) return;
    if (editingKey === NEW_ROW) {
      // Add new — replace any accidental collision, then append
      setBindings((prev) => [
        ...prev.filter((b) => b.sourceIndex !== draft.sourceIndex),
        draft,
      ]);
    } else {
      setBindings((prev) =>
        prev.map((b) => (b.sourceIndex === draft.sourceIndex ? draft : b)),
      );
    }
    setEditingKey(null);
    setDraft(null);
  }

  function cancelDraft() {
    setEditingKey(null);
    setDraft(null);
  }

  function deleteBinding(idx: number) {
    setBindings((prev) => prev.filter((b) => b.sourceIndex !== idx));
    if (editingKey === idx) cancelDraft();
  }

  // ── save ─────────────────────────────────────────────────────────────────────

  async function handleSave() {
    if (!activeProfile) return;
    setSaving(true);
    try {
      const config = generateKanataConfig(bindings);
      const nextVia = applyConfigToProfile(activeProfile.via, config);

      if (daemonStatus === 'online' && daemonClient) {
        await daemonClient.updateProfile(activeProfile.id, nextVia);
        await daemonClient.kanataReload(config);
        await useProfileStore.getState().loadFromDaemon();
      } else {
        // Daemon offline — persist locally so the change survives a page reload
        useProfileStore.setState((s) => ({
          profiles: s.profiles.map((p) =>
            p.id === activeProfile.id ? { ...p, via: nextVia } : p,
          ),
        }));
      }

      toast({ title: 'Bindings saved', status: 'success', duration: 2000 });
      onSaved?.();
    } catch (e) {
      toast({
        title: 'Save failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 4000,
      });
    } finally {
      setSaving(false);
    }
  }

  // ── render ────────────────────────────────────────────────────────────────────

  const usedKeys = new Set(bindings.map((b) => b.sourceIndex));
  const noBindings = bindings.length === 0 && editingKey !== NEW_ROW;

  return (
    <VStack align="stretch" spacing={0}>
      {!activeProfile && (
        <Alert status="warning" borderRadius="md" fontSize="xs" py={2} mb={3}>
          <AlertIcon boxSize={3} />
          No active profile — select one from the header first.
        </Alert>
      )}

      {/* Column header */}
      <Flex px={3} py={1.5} mb={1}>
        <Text
          flex="1.2"
          fontSize="9px"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.08em"
          color="text.muted"
        >
          Physical Key
        </Text>
        <Text
          flex="2"
          fontSize="9px"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.08em"
          color="text.muted"
        >
          Binding
        </Text>
        <Text
          flex="0.8"
          fontSize="9px"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.08em"
          color="text.muted"
        >
          Type
        </Text>
        <Box w="52px" />
      </Flex>

      {/* Empty state */}
      {noBindings && (
        <Box
          textAlign="center"
          py={8}
          border="1px dashed"
          borderColor="border.subtle"
          borderRadius="md"
          mb={3}
        >
          <Text fontSize="xs" color="text.muted">
            No software bindings yet.
          </Text>
          <Text fontSize="11px" color="text.muted" mt={1}>
            Click + Add Binding to remap a key without touching EEPROM.
          </Text>
        </Box>
      )}

      {/* Binding rows */}
      {!noBindings && (
        <VStack align="stretch" spacing={1} mb={2}>
          {bindings.map((b) => {
            const isEditing = editingKey === b.sourceIndex;
            return isEditing && draft ? (
              <EditRow
                key={b.sourceIndex}
                draft={draft}
                onChange={setDraft}
                onConfirm={confirmDraft}
                onCancel={cancelDraft}
                usedIndices={usedKeys}
              />
            ) : (
              <BindingRow
                key={b.sourceIndex}
                binding={b}
                faded={editingKey !== null && editingKey !== b.sourceIndex}
                onEdit={() => openEdit(b)}
                onDelete={() => deleteBinding(b.sourceIndex)}
              />
            );
          })}

          {/* Add-new row */}
          {editingKey === NEW_ROW && draft && (
            <EditRow
              draft={draft}
              onChange={setDraft}
              onConfirm={confirmDraft}
              onCancel={cancelDraft}
              usedIndices={usedKeys}
              isNew
            />
          )}
        </VStack>
      )}

      {/* Footer */}
      <Flex
        pt={3}
        borderTop="1px solid"
        borderColor="border.subtle"
        justify="space-between"
        align="center"
      >
        <Button
          size="xs"
          variant="ghost"
          leftIcon={<Plus size={11} />}
          onClick={openAdd}
          isDisabled={editingKey !== null}
        >
          Add Binding
        </Button>
        <HStack spacing={2}>
          {onCancel && (
            <Button size="xs" variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
          )}
          <Button
            size="xs"
            onClick={() => void handleSave()}
            isDisabled={!activeProfile || saving || editingKey !== null}
            isLoading={saving}
          >
            Save & Apply
          </Button>
        </HStack>
      </Flex>
    </VStack>
  );
}

// ─── Display row ──────────────────────────────────────────────────────────────

function BindingRow({
  binding,
  faded,
  onEdit,
  onDelete,
}: {
  binding: KeyBinding;
  faded: boolean;
  onEdit: () => void;
  onDelete: () => void;
}) {
  return (
    <Flex
      align="center"
      px={3}
      py={2}
      borderRadius="md"
      border="1px solid"
      borderColor="border.subtle"
      bg="bg.subtle"
      opacity={faded ? 0.3 : 1}
      transition="opacity 0.15s ease"
      _hover={{ borderColor: 'border.strong' }}
      role="group"
    >
      {/* Physical key pill */}
      <Box flex="1.2" minW={0}>
        <Tag size="sm" variant="subtle" fontFamily="mono" fontSize="10px">
          {hhkbKeyLabel(binding.sourceIndex)}
        </Tag>
      </Box>

      {/* Binding description */}
      <Text flex="2" fontSize="xs" fontFamily="mono" color="text.secondary" noOfLines={1}>
        {bindingDescription(binding)}
      </Text>

      {/* Type badge */}
      <Box flex="0.8">
        <Tag
          size="sm"
          variant="subtle"
          fontSize="9px"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.04em"
        >
          {TYPE_LABELS[binding.type]}
        </Tag>
      </Box>

      {/* Action buttons — visible on hover */}
      <HStack spacing={0.5} w="52px" justify="flex-end">
        <IconButton
          aria-label="Edit binding"
          icon={<Pencil size={11} />}
          size="xs"
          variant="ghost"
          onClick={onEdit}
          opacity={0}
          _groupHover={{ opacity: 1 }}
          transition="opacity 0.1s ease"
        />
        <IconButton
          aria-label="Delete binding"
          icon={<Trash2 size={11} />}
          size="xs"
          variant="ghost"
          color="red.400"
          onClick={onDelete}
          opacity={0}
          _groupHover={{ opacity: 1 }}
          transition="opacity 0.1s ease"
        />
      </HStack>
    </Flex>
  );
}

// ─── Edit row ─────────────────────────────────────────────────────────────────

function EditRow({
  draft,
  onChange,
  onConfirm,
  onCancel,
  usedIndices,
  isNew = false,
}: {
  draft: KeyBinding;
  onChange: (b: KeyBinding) => void;
  onConfirm: () => void;
  onCancel: () => void;
  usedIndices: Set<number>;
  isNew?: boolean;
}) {
  const sortedKeys = useMemo(
    () => [...HHKB_LAYOUT].sort((a, b) => a.index - b.index),
    [],
  );

  function changeSource(idx: number) {
    onChange({ ...draft, sourceIndex: idx });
  }

  function changeType(type: KeyBinding['type']) {
    const base = { sourceIndex: draft.sourceIndex };
    switch (type) {
      case 'remap':
        onChange({ ...base, type: 'remap', target: 'a' });
        break;
      case 'tap-hold':
        onChange({ ...base, type: 'tap-hold', tap: 'a', hold: 'lctl', timeout: 200 });
        break;
      case 'layer-switch':
        onChange({ ...base, type: 'layer-switch', layerName: 'nav', mode: 'while-held' });
        break;
    }
  }

  const keyConflict = isNew && usedIndices.has(draft.sourceIndex);

  return (
    <Box
      px={3}
      py={3}
      borderRadius="md"
      border="1px solid"
      borderColor="accent.primary"
      bg="accent.subtle"
    >
      <VStack align="stretch" spacing={2.5}>
        {/* Row 1: key selector + type selector */}
        <Flex gap={3} align="flex-end">
          <Box flex="1">
            <Text
              fontSize="9px"
              fontFamily="mono"
              color="text.muted"
              textTransform="uppercase"
              letterSpacing="0.06em"
              mb={1}
            >
              Physical Key
            </Text>
            <Select
              size="xs"
              value={draft.sourceIndex}
              onChange={(e) => changeSource(Number(e.target.value))}
              fontFamily="mono"
              borderColor={keyConflict ? 'orange.400' : undefined}
            >
              {sortedKeys.map((k) => (
                <option
                  key={k.index}
                  value={k.index}
                  disabled={isNew && usedIndices.has(k.index) && k.index !== draft.sourceIndex}
                >
                  {k.label} (#{k.index})
                </option>
              ))}
            </Select>
            {keyConflict && (
              <Text fontSize="9px" color="orange.400" mt={0.5}>
                Already mapped — will be overwritten
              </Text>
            )}
          </Box>
          <Box flex="0.9">
            <Text
              fontSize="9px"
              fontFamily="mono"
              color="text.muted"
              textTransform="uppercase"
              letterSpacing="0.06em"
              mb={1}
            >
              Type
            </Text>
            <Select
              size="xs"
              value={draft.type}
              onChange={(e) => changeType(e.target.value as KeyBinding['type'])}
              fontFamily="mono"
            >
              <option value="remap">Remap</option>
              <option value="tap-hold">Tap-Hold</option>
              <option value="layer-switch">Layer Switch</option>
            </Select>
          </Box>
        </Flex>

        {/* Row 2: binding-specific fields */}
        {draft.type === 'remap' && (
          <Box>
            <Text
              fontSize="9px"
              fontFamily="mono"
              color="text.muted"
              textTransform="uppercase"
              letterSpacing="0.06em"
              mb={1}
            >
              Send Key
            </Text>
            <TokenSelect
              value={(draft as RemapBinding).target}
              onChange={(t) => onChange({ ...(draft as RemapBinding), target: t })}
            />
          </Box>
        )}

        {draft.type === 'tap-hold' && (
          <Flex gap={2} align="flex-end">
            <Box flex="1">
              <Text
                fontSize="9px"
                fontFamily="mono"
                color="text.muted"
                textTransform="uppercase"
                letterSpacing="0.06em"
                mb={1}
              >
                Tap
              </Text>
              <TokenSelect
                value={(draft as TapHoldBinding).tap}
                onChange={(t) => onChange({ ...(draft as TapHoldBinding), tap: t })}
              />
            </Box>
            <Box flex="1">
              <Text
                fontSize="9px"
                fontFamily="mono"
                color="text.muted"
                textTransform="uppercase"
                letterSpacing="0.06em"
                mb={1}
              >
                Hold
              </Text>
              <TokenSelect
                value={(draft as TapHoldBinding).hold}
                onChange={(t) => onChange({ ...(draft as TapHoldBinding), hold: t })}
              />
            </Box>
            <Box>
              <Text
                fontSize="9px"
                fontFamily="mono"
                color="text.muted"
                textTransform="uppercase"
                letterSpacing="0.06em"
                mb={1}
              >
                Timeout
              </Text>
              <HStack spacing={1}>
                <Input
                  size="xs"
                  type="number"
                  min={50}
                  max={2000}
                  w="64px"
                  value={(draft as TapHoldBinding).timeout}
                  onChange={(e) => {
                    const v = Number(e.target.value);
                    if (!isNaN(v))
                      onChange({
                        ...(draft as TapHoldBinding),
                        timeout: Math.max(50, Math.min(2000, v)),
                      });
                  }}
                  fontFamily="mono"
                />
                <Text fontSize="10px" color="text.muted">
                  ms
                </Text>
              </HStack>
            </Box>
          </Flex>
        )}

        {draft.type === 'layer-switch' && (
          <Flex gap={2} align="flex-end">
            <Box flex="1.5">
              <Text
                fontSize="9px"
                fontFamily="mono"
                color="text.muted"
                textTransform="uppercase"
                letterSpacing="0.06em"
                mb={1}
              >
                Layer Name
              </Text>
              <Input
                size="xs"
                value={(draft as LayerSwitchBinding).layerName}
                onChange={(e) =>
                  onChange({
                    ...(draft as LayerSwitchBinding),
                    layerName: e.target.value.replace(/\s+/g, '-'),
                  })
                }
                fontFamily="mono"
                placeholder="e.g. nav"
              />
            </Box>
            <Box flex="1">
              <Text
                fontSize="9px"
                fontFamily="mono"
                color="text.muted"
                textTransform="uppercase"
                letterSpacing="0.06em"
                mb={1}
              >
                Activate
              </Text>
              <Select
                size="xs"
                value={(draft as LayerSwitchBinding).mode}
                onChange={(e) =>
                  onChange({
                    ...(draft as LayerSwitchBinding),
                    mode: e.target.value as 'while-held' | 'toggle',
                  })
                }
                fontFamily="mono"
              >
                <option value="while-held">While Held</option>
                <option value="toggle">Toggle</option>
              </Select>
            </Box>
          </Flex>
        )}

        {/* Confirm / cancel */}
        <Flex justify="flex-end" gap={1.5} pt={0.5}>
          <Button
            size="xs"
            variant="ghost"
            leftIcon={<X size={11} />}
            onClick={onCancel}
          >
            Cancel
          </Button>
          <Button
            size="xs"
            leftIcon={<Check size={11} />}
            onClick={onConfirm}
          >
            {isNew ? 'Add' : 'Apply'}
          </Button>
        </Flex>
      </VStack>
    </Box>
  );
}

// ─── Token select ─────────────────────────────────────────────────────────────

function TokenSelect({
  value,
  onChange,
}: {
  value: string;
  onChange: (t: string) => void;
}) {
  return (
    <Select
      size="xs"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      fontFamily="mono"
    >
      {KEYCODES.map((k) => (
        <option key={k.token} value={k.token}>
          {k.label}
        </option>
      ))}
    </Select>
  );
}
