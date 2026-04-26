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
import { Check, ExternalLink, Pencil, Plus, ShieldAlert, Trash2, X } from 'lucide-react';
import { HHKB_LAYOUT } from '../data/hhkbLayout';
import { useDaemonStore } from '../store/daemonStore';
import { useKanataStore } from '../store/kanataStore';
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
import { DaemonError, type DaemonClient } from '../hhkb/daemonClient';

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Stamp a canonical profile id into the via's `_roninKB.profile.id` so the
 * daemon can round-trip it back to us. The daemon's create endpoint requires
 * the inner id to parse as a UUID — when our local id (e.g. 'default') isn't
 * UUID-shaped we fall back to a fresh one and let the caller sync the local
 * store afterwards.
 */
function stampProfileId(via: ViaProfile, id: string, name: string): ViaProfile {
  const cloned: ViaProfile = JSON.parse(JSON.stringify(via)) as ViaProfile;
  const ext: RoninExtension = cloned._roninKB ?? {
    version: '1',
    profile: { id, name },
  };
  const safeId = UUID_RE.test(id) ? id : crypto.randomUUID();
  ext.profile = { ...ext.profile, id: safeId, name: ext.profile?.name ?? name };
  cloned._roninKB = ext;
  return cloned;
}

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
    profile: { id: crypto.randomUUID(), name: cloned.name },
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
  const kanataProcessState = useKanataStore((s) => s.processState);
  const kanataStart = useKanataStore((s) => s.start);
  const kanataInputMonitoring = useKanataStore((s) => s.inputMonitoringGranted);
  const kanataBinaryPath = useKanataStore((s) => s.binaryPath);
  const kanataLastError = useKanataStore((s) => s.error);
  const kanataInstalled = useKanataStore((s) => s.installed);

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
   *
   * When `focusKeyIndex` points at a key with no existing binding, we open
   * the new-row UI (NEW_ROW) so the EditRow actually mounts — otherwise
   * the existing-row branch would have nothing to map over, leaving the
   * panel stuck in its empty state with `Add Binding` disabled.
   */
  const [editingKey, setEditingKey] = useState<number | null>(() => {
    if (focusKeyIndex == null) return null;
    const existing = parseKeyBindings(initialConfig).find(
      (b) => b.sourceIndex === focusKeyIndex,
    );
    return existing ? focusKeyIndex : NEW_ROW;
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
    // Stage the in-flight draft (if any) before opening a fresh row so
    // the user doesn't have to manually click an "Apply" button first.
    if (editingKey !== null && draft) confirmDraft();
    const stagedBindings = bindingsWithDraft();
    const usedSet = new Set(stagedBindings.map((b) => b.sourceIndex));
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

  /**
   * Returns the list of bindings as if the in-flight draft (when present)
   * were committed. Used to derive a "what the user will save" snapshot
   * without going through React state churn first.
   */
  function bindingsWithDraft(): KeyBinding[] {
    if (editingKey === null || !draft) return bindings;
    if (editingKey === NEW_ROW) {
      return [
        ...bindings.filter((b) => b.sourceIndex !== draft.sourceIndex),
        draft,
      ];
    }
    return bindings.map((b) =>
      b.sourceIndex === draft.sourceIndex ? draft : b,
    );
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
      // Auto-commit any open draft so users don't have to click an
      // intermediate "Apply" before saving.
      const finalBindings = bindingsWithDraft();
      if (editingKey !== null && draft) {
        setBindings(finalBindings);
        setEditingKey(null);
        setDraft(null);
      }
      const config = generateKanataConfig(finalBindings);
      const nextVia = applyConfigToProfile(activeProfile.via, config);

      if (daemonStatus === 'online' && daemonClient) {
        try {
          await daemonClient.updateProfile(activeProfile.id, nextVia);
        } catch (err) {
          // The active profile lives only in local state (factory default,
          // freshly imported, or the legacy 'default' seed). Promote it to
          // the daemon by issuing a create on first save instead.
          if (err instanceof DaemonError && err.status === 404) {
            const stamped = stampProfileId(
              nextVia,
              activeProfile.id,
              activeProfile.name,
            );
            const summary = await daemonClient.createProfile(stamped);
            if (summary.id !== activeProfile.id) {
              useProfileStore
                .getState()
                .rekeyProfile(activeProfile.id, summary.id);
            }
          } else {
            throw err;
          }
        }

        // Ensure kanata is running before attempting reload.
        // If it's stopped (but installed), auto-start it so the bindings
        // take effect immediately.
        if (kanataProcessState === 'stopped') {
          try {
            await kanataStart();
          } catch (e) {
            const raw = e instanceof Error ? e.message : String(e);
            const isPermIssue =
              kanataInputMonitoring === false ||
              raw.includes('Input Monitoring') ||
              raw.includes('kanata_permission_required');
            toast({
              title: isPermIssue
                ? 'Grant Input Monitoring to apply bindings'
                : 'Kanata failed to start',
              description: isPermIssue
                ? 'Bindings saved. macOS needs permission for kanata to capture keys — see the banner above to open System Settings.'
                : `Bindings saved but kanata didn't start: ${raw}`,
              status: 'warning',
              duration: 6000,
              isClosable: true,
            });
            await useProfileStore.getState().loadFromDaemon();
            onSaved?.();
            return;
          }
        }

        if (kanataProcessState !== 'not_installed') {
          try {
            await daemonClient.kanataReload(config);
          } catch (e) {
            // Reload failed — bindings are saved to disk but not active.
            toast({
              title: 'Bindings saved — kanata reload failed',
              description: e instanceof Error ? e.message : String(e),
              status: 'warning',
              duration: 5000,
              isClosable: true,
            });
            await useProfileStore.getState().loadFromDaemon();
            onSaved?.();
            return;
          }
        }

        await useProfileStore.getState().loadFromDaemon();
        toast({
          title: kanataProcessState === 'not_installed'
            ? 'Bindings saved (kanata not installed)'
            : 'Bindings saved & applied',
          status: 'success',
          duration: 2000,
        });
      } else {
        // Daemon offline — persist locally
        useProfileStore.setState((s) => ({
          profiles: s.profiles.map((p) =>
            p.id === activeProfile.id ? { ...p, via: nextVia } : p,
          ),
        }));
        toast({
          title: 'Bindings saved locally',
          description: 'Reconnect to daemon to apply to kanata.',
          status: 'info',
          duration: 3000,
        });
      }
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
  const needsInputMonitoring =
    kanataInstalled &&
    kanataInputMonitoring === false &&
    kanataProcessState !== 'running';

  return (
    <VStack align="stretch" spacing={0}>
      {!activeProfile && (
        <Alert status="warning" borderRadius="md" fontSize="xs" py={2} mb={3}>
          <AlertIcon boxSize={3} />
          No active profile — select one from the header first.
        </Alert>
      )}

      {needsInputMonitoring && (
        <KanataPermissionBanner
          binaryPath={kanataBinaryPath}
          detail={kanataLastError}
          daemonClient={daemonClient}
        />
      )}

      {/* Column header — only shown when there's a list of committed
          bindings. In focus mode (a single EditRow with its own labels)
          the columnar header doesn't align with anything. */}
      {bindings.length > 0 && editingKey !== NEW_ROW && (
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
      )}

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

      {/* Binding rows.
       *
       * "Focus mode" = the editor has nothing else to show but a single
       * EditRow (no other committed bindings, no other open drafts). In that
       * case we hide the in-row Apply/Cancel actions so the footer's
       * Save & Apply is the unambiguous primary action — one click does the
       * whole thing. */}
      {!noBindings && (
        <VStack align="stretch" spacing={1} mb={2}>
          {bindings.map((b) => {
            const isEditing = editingKey === b.sourceIndex;
            const focusOnly = isEditing && bindings.length === 1;
            return isEditing && draft ? (
              <EditRow
                key={b.sourceIndex}
                draft={draft}
                onChange={setDraft}
                onConfirm={confirmDraft}
                onCancel={cancelDraft}
                usedIndices={usedKeys}
                showActions={!focusOnly}
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
              showActions={bindings.length > 0}
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
            isDisabled={!activeProfile || saving}
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
  showActions = true,
}: {
  draft: KeyBinding;
  onChange: (b: KeyBinding) => void;
  onConfirm: () => void;
  onCancel: () => void;
  usedIndices: Set<number>;
  isNew?: boolean;
  /**
   * Hide the in-row Apply/Cancel actions. Used when the surrounding form
   * already has a primary "Save & Apply" button that auto-commits.
   */
  showActions?: boolean;
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
          <>
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
            <TapHoldPreview
              keyLabel={hhkbKeyLabel(draft.sourceIndex)}
              tap={(draft as TapHoldBinding).tap}
              hold={(draft as TapHoldBinding).hold}
              timeoutMs={(draft as TapHoldBinding).timeout}
            />
          </>
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

        {/* Confirm / cancel — only when there are sibling rows to disambiguate
            from. In focus mode the footer's "Save & Apply" auto-commits. */}
        {showActions && (
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
        )}
      </VStack>
    </Box>
  );
}

// ─── Tap-Hold inline preview ─────────────────────────────────────────────────

/**
 * Spell out exactly what a tap-hold binding will do at runtime so users don't
 * misread it as "long-press fires Tap+Hold combined". Tap fires on quick
 * release; Hold engages a modifier-style state that combines with whatever
 * key the user presses next.
 */
function TapHoldPreview({
  keyLabel,
  tap,
  hold,
  timeoutMs,
}: {
  keyLabel: string;
  tap: string;
  hold: string;
  timeoutMs: number;
}) {
  const tapLabel = tokenToLabel(tap);
  const holdLabel = tokenToLabel(hold);
  return (
    <Box
      px={2.5}
      py={1.5}
      borderRadius="sm"
      bg="bg.subtle"
      border="1px dashed"
      borderColor="border.subtle"
    >
      <Text
        fontSize="10px"
        fontFamily="mono"
        color="text.muted"
        lineHeight="1.6"
      >
        Tap <Text as="span" color="text.primary">{keyLabel}</Text> (&lt;{timeoutMs}ms) →{' '}
        <Text as="span" color="accent.primary">{tapLabel}</Text>
        {'  ·  '}
        Hold <Text as="span" color="text.primary">{keyLabel}</Text> →{' '}
        <Text as="span" color="accent.primary">{holdLabel}</Text>
        {'  ·  '}
        Hold <Text as="span" color="text.primary">{keyLabel}</Text> + key →{' '}
        <Text as="span" color="accent.primary">{holdLabel}+key</Text>
      </Text>
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

// ─── Kanata permission banner ─────────────────────────────────────────────────

const IS_MACOS =
  typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform);

function KanataPermissionBanner({
  binaryPath,
  daemonClient,
}: {
  binaryPath: string | null;
  detail: string | null;
  daemonClient: DaemonClient | null;
}) {
  const toast = useToast();

  function openSettings() {
    // Direct deep-link into Privacy & Security → Input Monitoring on macOS.
    // Browsers won't open this URL with `window.open`; an `<a>` click works.
    const a = document.createElement('a');
    a.href = 'x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent';
    a.rel = 'noopener';
    a.click();
  }

  async function showInFinder() {
    if (!daemonClient) return;
    try {
      await daemonClient.kanataReveal();
    } catch (e) {
      toast({
        title: 'Could not open Finder',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 3000,
      });
    }
  }

  return (
    <Box
      mb={3}
      p={3}
      borderRadius="md"
      border="1px solid"
      borderColor="warning"
      bg="warning.subtle"
    >
      <HStack spacing={2} align="flex-start">
        <Box color="warning" mt="2px" flexShrink={0}>
          <ShieldAlert size={14} />
        </Box>
        <Box flex="1" minW={0}>
          <Text fontSize="xs" fontWeight={600} color="text.primary" lineHeight="1.4">
            kanata 還沒拿到權限 — macros 暫時不會生效
          </Text>
          <Text fontSize="11px" color="text.secondary" mt={1} lineHeight="1.5">
            按 <Text as="span" fontWeight={600}>Show in Finder</Text> 把{' '}
            <Text as="span" fontFamily="mono">RoninKB Kanata.app</Text>{' '}
            拖進「系統設定 → 隱私權 → 輸入監控」就生效。
          </Text>
          <HStack spacing={2} mt={2} flexWrap="wrap">
            <Button
              size="xs"
              leftIcon={<ExternalLink size={11} />}
              onClick={() => void showInFinder()}
              isDisabled={!daemonClient || !binaryPath}
              variant="solid"
            >
              Show in Finder
            </Button>
            {IS_MACOS && (
              <Button size="xs" variant="outline" onClick={openSettings}>
                Open Input Monitoring
              </Button>
            )}
          </HStack>
        </Box>
      </HStack>
    </Box>
  );
}
