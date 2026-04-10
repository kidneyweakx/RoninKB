/**
 * Full macro editor for RoninKB.
 *
 * Replaces the old 64-character text stub with a proper editor that
 * writes to the active profile's `_roninKB.software.config` (the kanata
 * `.kbd` source). Supports four modes:
 *
 *   1. Tap-Hold       — `(tap-hold <tap> <hold> <timeout> <timeout>)`
 *   2. Macro Sequence — `(defalias ...) + (macro ...)`
 *   3. Layer Switch   — `(layer-while-held X)` / `(layer-toggle X)`
 *   4. Raw .kbd       — monospace textarea with line numbers + parens-check
 *
 * Round-tripping the first three modes is best-effort: the editor tries
 * to populate its controls from the existing config on mount, and any
 * unparseable parts are preserved only in Raw mode.
 */

import { useEffect, useMemo, useState } from 'react';
import {
  Box,
  Button,
  HStack,
  Select,
  Text,
  Textarea,
  VStack,
  Alert,
  AlertIcon,
  useToast,
  Flex,
  Input,
  Tag,
  IconButton,
} from '@chakra-ui/react';
import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  Code2,
  Layers,
  Plus,
  Trash2,
  X,
  Zap,
} from 'lucide-react';
import { HHKB_LAYOUT } from '../data/hhkbLayout';
import { useProfileStore } from '../store/profileStore';
import { useDaemonStore } from '../store/daemonStore';
import type { ViaProfile, RoninExtension } from '../hhkb/via';

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface Props {
  /** Emitted with the generated kanata snippet after a successful save. */
  onSave: (snippet: string) => void;
  onCancel: () => void;
}

type Mode = 'tap-hold' | 'sequence' | 'layer' | 'raw';

const MODES: Array<{ id: Mode; label: string; Icon: typeof Zap }> = [
  { id: 'tap-hold', label: 'Tap-Hold', Icon: Zap },
  { id: 'sequence', label: 'Sequence', Icon: Code2 },
  { id: 'layer', label: 'Layer', Icon: Layers },
  { id: 'raw', label: 'Raw .kbd', Icon: Code2 },
];

// Minimal keycode list used by the editor dropdowns. Mirrors the
// short-name style kanata expects (lowercase, no `KC_` prefix).
const KEYCODE_CHOICES: Array<{ name: string; token: string }> = [
  ...'abcdefghijklmnopqrstuvwxyz'.split('').map((c) => ({
    name: c.toUpperCase(),
    token: c,
  })),
  ...'1234567890'.split('').map((d) => ({ name: d, token: d })),
  { name: 'Esc', token: 'esc' },
  { name: 'Tab', token: 'tab' },
  { name: 'Space', token: 'spc' },
  { name: 'Enter', token: 'ret' },
  { name: 'Backspace', token: 'bspc' },
  { name: 'L Ctrl', token: 'lctl' },
  { name: 'L Shift', token: 'lsft' },
  { name: 'L Alt', token: 'lalt' },
  { name: 'L Cmd', token: 'lmet' },
  { name: 'R Ctrl', token: 'rctl' },
  { name: 'R Shift', token: 'rsft' },
  { name: 'R Alt', token: 'ralt' },
  { name: 'R Cmd', token: 'rmet' },
  { name: 'Left', token: 'left' },
  { name: 'Right', token: 'rght' },
  { name: 'Up', token: 'up' },
  { name: 'Down', token: 'down' },
];

// ---------------------------------------------------------------------------
// Root component
// ---------------------------------------------------------------------------

export function MacroPanel({ onSave, onCancel }: Props) {
  const toast = useToast();
  const daemonStatus = useDaemonStore((s) => s.status);
  const daemonClient = useDaemonStore((s) => s.client);
  const activeProfile = useProfileStore((s) => s.getActive)();

  const initialConfig = useMemo(
    () => activeProfile?.via._roninKB?.software?.config ?? '',
    [activeProfile],
  );

  const [mode, setMode] = useState<Mode>('tap-hold');
  const [rawText, setRawText] = useState(initialConfig);
  const [saving, setSaving] = useState(false);

  // Tap-Hold state
  const [thKey, setThKey] = useState<number>(30); // default: A
  const [thTap, setThTap] = useState<string>('a');
  const [thHold, setThHold] = useState<string>('lctl');
  const [thTimeout, setThTimeout] = useState<number>(200);

  // Sequence state
  const [seqKey, setSeqKey] = useState<number>(30);
  const [seqName, setSeqName] = useState<string>('my-macro');
  const [seqTokens, setSeqTokens] = useState<string[]>(['h', 'i']);

  // Layer switch state
  const [layerKey, setLayerKey] = useState<number>(6); // Fn key
  const [layerTarget, setLayerTarget] = useState<string>('nav');
  const [layerKind, setLayerKind] = useState<'while-held' | 'toggle'>(
    'while-held',
  );

  // Re-hydrate the raw text whenever the active profile changes.
  useEffect(() => {
    setRawText(initialConfig);
  }, [initialConfig]);

  // Best-effort load: pull the first existing tap-hold / defalias /
  // layer-while-held form out of the current config and seed the edit
  // state so the UI feels round-trippable.
  useEffect(() => {
    if (!initialConfig) return;
    const th = /\(\s*tap-hold\s+(\d+)\s+\d+\s+(\S+)\s+(\S+)\s*\)/.exec(
      initialConfig,
    );
    if (th) {
      setThTimeout(Number(th[1]));
      setThTap(th[2]);
      setThHold(th[3]);
    }
    const def = /\(\s*defalias\s+(\S+)\s+\(\s*macro\s+([^)]+)\)\s*\)/.exec(
      initialConfig,
    );
    if (def) {
      setSeqName(def[1]);
      setSeqTokens(def[2].trim().split(/\s+/));
    }
    const lwh = /\(\s*layer-(while-held|toggle)\s+(\S+)\s*\)/.exec(
      initialConfig,
    );
    if (lwh) {
      setLayerKind(lwh[1] === 'toggle' ? 'toggle' : 'while-held');
      setLayerTarget(lwh[2]);
    }
  }, [initialConfig]);

  // -------------------------------------------------------------------------
  // Build-from-UI-state → kanata text
  // -------------------------------------------------------------------------

  function buildSnippet(currentMode: Mode): string {
    const defsrc = buildDefsrc();
    switch (currentMode) {
      case 'tap-hold': {
        const layer = buildDeflayerWithReplacement(
          thKey,
          `(tap-hold ${thTimeout} ${thTimeout} ${thTap} ${thHold})`,
        );
        return [defsrc, layer].join('\n\n');
      }
      case 'sequence': {
        const alias = `(defalias ${seqName} (macro ${seqTokens.join(' ')}))`;
        const layer = buildDeflayerWithReplacement(seqKey, `@${seqName}`);
        return [defsrc, alias, layer].join('\n\n');
      }
      case 'layer': {
        const kind =
          layerKind === 'while-held' ? 'layer-while-held' : 'layer-toggle';
        const layer = buildDeflayerWithReplacement(
          layerKey,
          `(${kind} ${layerTarget})`,
        );
        const navLayer = `(deflayer ${layerTarget}\n  ${allUnderscores()}\n)`;
        return [defsrc, layer, navLayer].join('\n\n');
      }
      case 'raw':
        return rawText;
    }
  }

  // -------------------------------------------------------------------------
  // Persistence
  // -------------------------------------------------------------------------

  async function persistConfig(newConfig: string) {
    if (!activeProfile) {
      toast({
        title: 'No active profile',
        description: 'Create or select a profile before saving macros.',
        status: 'warning',
        duration: 3500,
      });
      return;
    }
    if (daemonStatus !== 'online' || !daemonClient) {
      toast({
        title: 'Daemon required',
        description: 'Software macros need the RoninKB daemon running.',
        status: 'warning',
        duration: 3500,
      });
      return;
    }

    setSaving(true);
    try {
      const nextVia = applyConfigToProfile(activeProfile.via, newConfig);
      await daemonClient.updateProfile(activeProfile.id, nextVia);
      await daemonClient.kanataReload(newConfig);
      toast({
        title: `Saved to profile '${activeProfile.name}'`,
        status: 'success',
        duration: 2500,
      });
      // Reload profiles from daemon so the local cache reflects the save.
      await useProfileStore.getState().loadFromDaemon();
      onSave(newConfig);
    } catch (e) {
      toast({
        title: 'Save failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
        duration: 5000,
      });
    } finally {
      setSaving(false);
    }
  }

  function handleSave() {
    const snippet =
      mode === 'raw' ? rawText : mergeSnippetIntoConfig(rawText, buildSnippet(mode));
    void persistConfig(snippet);
  }

  const daemonOnline = daemonStatus === 'online';
  const hasProfile = !!activeProfile;
  const disabledReason = !daemonOnline
    ? 'Daemon required for macros'
    : !hasProfile
      ? 'Select a profile first'
      : null;

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <VStack
      align="stretch"
      spacing={3}
      bg="bg.subtle"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="lg"
      p={3}
    >
      {/* Mode switcher */}
      <HStack spacing={1} flexWrap="wrap">
        {MODES.map((m) => {
          const active = mode === m.id;
          return (
            <Box
              key={m.id}
              as="button"
              onClick={() => setMode(m.id)}
              px={2.5}
              py={1}
              borderRadius="sm"
              fontSize="10px"
              fontWeight={500}
              textTransform="uppercase"
              letterSpacing="0.06em"
              fontFamily="mono"
              bg={active ? 'accent.subtle' : 'transparent'}
              color={active ? 'accent.primary' : 'text.muted'}
              border="1px solid"
              borderColor={active ? 'accent.primary' : 'border.subtle'}
            >
              <HStack spacing={1}>
                <m.Icon size={10} />
                <Text as="span">{m.label}</Text>
              </HStack>
            </Box>
          );
        })}
      </HStack>

      {disabledReason && (
        <Alert status="info" borderRadius="md" fontSize="xs" py={2}>
          <AlertIcon boxSize={4} />
          {disabledReason}
        </Alert>
      )}

      {/* Mode-specific editor */}
      {mode === 'tap-hold' && (
        <TapHoldEditor
          keyIndex={thKey}
          onKey={setThKey}
          tap={thTap}
          onTap={setThTap}
          hold={thHold}
          onHold={setThHold}
          timeout={thTimeout}
          onTimeout={setThTimeout}
        />
      )}

      {mode === 'sequence' && (
        <SequenceEditor
          keyIndex={seqKey}
          onKey={setSeqKey}
          aliasName={seqName}
          onAliasName={setSeqName}
          tokens={seqTokens}
          onTokens={setSeqTokens}
        />
      )}

      {mode === 'layer' && (
        <LayerEditor
          keyIndex={layerKey}
          onKey={setLayerKey}
          target={layerTarget}
          onTarget={setLayerTarget}
          kind={layerKind}
          onKind={setLayerKind}
        />
      )}

      {mode === 'raw' && <RawEditor value={rawText} onChange={setRawText} />}

      {/* Preview */}
      {mode !== 'raw' && (
        <Box
          bg="bg.surface"
          border="1px solid"
          borderColor="border.subtle"
          borderRadius="md"
          p={2}
          maxH="120px"
          overflow="auto"
        >
          <Text
            fontFamily="mono"
            fontSize="10px"
            color="text.secondary"
            whiteSpace="pre"
          >
            {buildSnippet(mode)}
          </Text>
        </Box>
      )}

      {/* Footer */}
      <HStack justify="space-between">
        <Text fontSize="10px" color="text.muted" fontFamily="mono">
          {mode === 'raw'
            ? `${rawText.length} chars`
            : `target: key #${mode === 'tap-hold' ? thKey : mode === 'sequence' ? seqKey : layerKey}`}
        </Text>
        <HStack spacing={2}>
          <Button
            size="xs"
            variant="ghost"
            onClick={onCancel}
            leftIcon={<X size={10} />}
          >
            Cancel
          </Button>
          <Button
            size="xs"
            variant="solid"
            onClick={handleSave}
            isDisabled={!!disabledReason || saving}
          >
            {saving ? 'Saving…' : 'Save'}
          </Button>
        </HStack>
      </HStack>
    </VStack>
  );
}

// ---------------------------------------------------------------------------
// Tap-Hold editor
// ---------------------------------------------------------------------------

function TapHoldEditor(props: {
  keyIndex: number;
  onKey: (v: number) => void;
  tap: string;
  onTap: (v: string) => void;
  hold: string;
  onHold: (v: string) => void;
  timeout: number;
  onTimeout: (v: number) => void;
}) {
  return (
    <VStack align="stretch" spacing={2}>
      <FieldRow label="Source key">
        <KeySelect value={props.keyIndex} onChange={props.onKey} />
      </FieldRow>
      <FieldRow label="Tap">
        <TokenSelect value={props.tap} onChange={props.onTap} />
      </FieldRow>
      <FieldRow label="Hold">
        <TokenSelect value={props.hold} onChange={props.onHold} />
      </FieldRow>
      <FieldRow label="Timeout (ms)">
        <Input
          size="xs"
          type="number"
          min={100}
          max={1000}
          value={props.timeout}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (!isNaN(v)) props.onTimeout(Math.max(100, Math.min(1000, v)));
          }}
          fontFamily="mono"
        />
      </FieldRow>
    </VStack>
  );
}

// ---------------------------------------------------------------------------
// Sequence editor
// ---------------------------------------------------------------------------

function SequenceEditor(props: {
  keyIndex: number;
  onKey: (v: number) => void;
  aliasName: string;
  onAliasName: (v: string) => void;
  tokens: string[];
  onTokens: (v: string[]) => void;
}) {
  const [pending, setPending] = useState('a');

  function addToken() {
    props.onTokens([...props.tokens, pending]);
  }
  function removeToken(i: number) {
    props.onTokens(props.tokens.filter((_, idx) => idx !== i));
  }
  function moveUp(i: number) {
    if (i === 0) return;
    const copy = [...props.tokens];
    [copy[i - 1], copy[i]] = [copy[i], copy[i - 1]];
    props.onTokens(copy);
  }
  function moveDown(i: number) {
    if (i >= props.tokens.length - 1) return;
    const copy = [...props.tokens];
    [copy[i + 1], copy[i]] = [copy[i], copy[i + 1]];
    props.onTokens(copy);
  }

  return (
    <VStack align="stretch" spacing={2}>
      <FieldRow label="Source key">
        <KeySelect value={props.keyIndex} onChange={props.onKey} />
      </FieldRow>
      <FieldRow label="Alias name">
        <Input
          size="xs"
          value={props.aliasName}
          onChange={(e) => props.onAliasName(e.target.value.replace(/\s+/g, '-'))}
          fontFamily="mono"
        />
      </FieldRow>
      <Box>
        <Text
          fontSize="10px"
          color="text.muted"
          textTransform="uppercase"
          letterSpacing="0.06em"
          mb={1.5}
          fontFamily="mono"
        >
          Sequence
        </Text>
        <VStack align="stretch" spacing={1}>
          {props.tokens.map((t, i) => (
            <HStack key={`${t}-${i}`} spacing={1}>
              <Tag size="sm" variant="subtle" flex="1">
                <Text fontFamily="mono" fontSize="10px">
                  {i + 1}. {t}
                </Text>
              </Tag>
              <IconButton
                aria-label="up"
                size="xs"
                variant="ghost"
                icon={<ArrowUp size={10} />}
                onClick={() => moveUp(i)}
                isDisabled={i === 0}
              />
              <IconButton
                aria-label="down"
                size="xs"
                variant="ghost"
                icon={<ArrowDown size={10} />}
                onClick={() => moveDown(i)}
                isDisabled={i >= props.tokens.length - 1}
              />
              <IconButton
                aria-label="remove"
                size="xs"
                variant="ghost"
                icon={<Trash2 size={10} />}
                onClick={() => removeToken(i)}
              />
            </HStack>
          ))}
          <HStack>
            <TokenSelect value={pending} onChange={setPending} />
            <IconButton
              aria-label="add"
              size="xs"
              variant="subtle"
              icon={<Plus size={10} />}
              onClick={addToken}
            />
          </HStack>
        </VStack>
      </Box>
    </VStack>
  );
}

// ---------------------------------------------------------------------------
// Layer editor
// ---------------------------------------------------------------------------

function LayerEditor(props: {
  keyIndex: number;
  onKey: (v: number) => void;
  target: string;
  onTarget: (v: string) => void;
  kind: 'while-held' | 'toggle';
  onKind: (v: 'while-held' | 'toggle') => void;
}) {
  return (
    <VStack align="stretch" spacing={2}>
      <FieldRow label="Source key">
        <KeySelect value={props.keyIndex} onChange={props.onKey} />
      </FieldRow>
      <FieldRow label="Layer name">
        <Input
          size="xs"
          value={props.target}
          onChange={(e) => props.onTarget(e.target.value.replace(/\s+/g, '-'))}
          fontFamily="mono"
        />
      </FieldRow>
      <FieldRow label="Kind">
        <Select
          size="xs"
          value={props.kind}
          onChange={(e) => props.onKind(e.target.value as 'while-held' | 'toggle')}
          fontFamily="mono"
        >
          <option value="while-held">layer-while-held</option>
          <option value="toggle">layer-toggle</option>
        </Select>
      </FieldRow>
    </VStack>
  );
}

// ---------------------------------------------------------------------------
// Raw editor
// ---------------------------------------------------------------------------

function RawEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const [validation, setValidation] = useState<
    { ok: true } | { ok: false; message: string } | null
  >(null);

  function validate() {
    const result = validateParensBalance(value);
    setValidation(result);
  }

  const lineCount = value.split('\n').length;

  return (
    <VStack align="stretch" spacing={2}>
      <Flex align="flex-start" gap={2}>
        <Box
          flexShrink={0}
          bg="bg.surface"
          border="1px solid"
          borderColor="border.subtle"
          borderRight="none"
          borderLeftRadius="md"
          fontFamily="mono"
          fontSize="10px"
          color="text.muted"
          px={1.5}
          py={2}
          userSelect="none"
          textAlign="right"
          lineHeight="1.5"
          minW="24px"
        >
          {Array.from({ length: Math.max(lineCount, 1) }, (_, i) => (
            <Box key={i}>{i + 1}</Box>
          ))}
        </Box>
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          size="xs"
          variant="filled"
          fontFamily="mono"
          fontSize="10px"
          rows={12}
          resize="vertical"
          borderLeftRadius="0"
          borderRightRadius="md"
          flex="1"
          spellCheck={false}
          lineHeight="1.5"
        />
      </Flex>
      <HStack justify="space-between">
        <Button
          size="xs"
          variant="outline"
          leftIcon={<CheckCircle2 size={10} />}
          onClick={validate}
        >
          Validate
        </Button>
        {validation && (
          <HStack spacing={1}>
            {validation.ok ? (
              <Tag size="sm" variant="success">
                <HStack spacing={1}>
                  <CheckCircle2 size={10} />
                  <Text fontSize="10px" fontFamily="mono">
                    OK
                  </Text>
                </HStack>
              </Tag>
            ) : (
              <Tag size="sm" variant="danger">
                <HStack spacing={1}>
                  <AlertTriangle size={10} />
                  <Text fontSize="10px" fontFamily="mono">
                    {validation.message}
                  </Text>
                </HStack>
              </Tag>
            )}
          </HStack>
        )}
      </HStack>
    </VStack>
  );
}

// ---------------------------------------------------------------------------
// Shared subcomponents
// ---------------------------------------------------------------------------

function FieldRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <Flex align="center" gap={2}>
      <Text
        fontSize="10px"
        color="text.muted"
        fontFamily="mono"
        textTransform="uppercase"
        letterSpacing="0.06em"
        minW="90px"
      >
        {label}
      </Text>
      <Box flex="1">{children}</Box>
    </Flex>
  );
}

function KeySelect({
  value,
  onChange,
}: {
  value: number;
  onChange: (v: number) => void;
}) {
  const options = useMemo(
    () =>
      [...HHKB_LAYOUT].sort((a, b) => a.index - b.index),
    [],
  );
  return (
    <Select
      size="xs"
      value={value}
      onChange={(e) => onChange(Number(e.target.value))}
      fontFamily="mono"
    >
      {options.map((k) => (
        <option key={k.index} value={k.index}>
          #{k.index} — {k.label}
        </option>
      ))}
    </Select>
  );
}

function TokenSelect({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <Select
      size="xs"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      fontFamily="mono"
    >
      {KEYCODE_CHOICES.map((c) => (
        <option key={c.token} value={c.token}>
          {c.name} ({c.token})
        </option>
      ))}
    </Select>
  );
}

// ---------------------------------------------------------------------------
// Kanata text helpers
// ---------------------------------------------------------------------------

/**
 * Build a default `defsrc` covering all 60 HHKB keys in index order.
 * We emit numeric placeholders `k1` … `k60` so the text is valid-ish and
 * easy to spot in Raw mode.
 */
function buildDefsrc(): string {
  const tokens: string[] = [];
  for (let i = 1; i <= 60; i++) tokens.push(`k${i}`);
  return `(defsrc\n  ${tokens.join(' ')}\n)`;
}

function allUnderscores(): string {
  return new Array(60).fill('_').join(' ');
}

/**
 * Produce a `(deflayer base ...)` block with every slot set to `_`
 * except the requested key index (1..60), which gets `token`.
 */
function buildDeflayerWithReplacement(keyIndex: number, token: string): string {
  const slots: string[] = [];
  for (let i = 1; i <= 60; i++) {
    slots.push(i === keyIndex ? token : '_');
  }
  return `(deflayer base\n  ${slots.join(' ')}\n)`;
}

/**
 * Merge a freshly-generated snippet into an existing raw config. The
 * current strategy is "replace if we already have a base layer, else
 * append". Good enough for the B3 scope; users who need finer control
 * live in Raw mode.
 */
function mergeSnippetIntoConfig(existing: string, snippet: string): string {
  if (!existing.trim()) return snippet;
  if (/\(\s*deflayer\s+base\b/.test(existing)) {
    // Replace the whole file — aggressive but predictable. The user's
    // pre-existing comments survive in git history; Raw mode is the
    // recommended path for non-destructive edits.
    return snippet;
  }
  return `${existing}\n\n${snippet}`;
}

/**
 * Deep-clone the profile's VIA structure and stamp a new software config
 * onto its `_roninKB.software` extension. Creates the extension if it
 * was missing.
 */
function applyConfigToProfile(via: ViaProfile, config: string): ViaProfile {
  const cloned: ViaProfile = JSON.parse(JSON.stringify(via));
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

/** Cheap parens-balance sanity check for the Raw-mode Validate button. */
function validateParensBalance(
  text: string,
): { ok: true } | { ok: false; message: string } {
  let depth = 0;
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === '"') {
      inString = !inString;
      continue;
    }
    if (inString) continue;
    if (ch === '(') depth++;
    else if (ch === ')') {
      depth--;
      if (depth < 0) {
        return { ok: false, message: `unexpected ')' at char ${i}` };
      }
    }
  }
  if (depth > 0) return { ok: false, message: `${depth} unclosed '('` };
  return { ok: true };
}
