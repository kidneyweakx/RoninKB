/**
 * Visual DIP switch readout for the HHKB Professional Hybrid.
 *
 * Polls `GET /device/dipsw` through the daemon whenever the daemon is online
 * and the device is connected, and renders a skeuomorphic bank of six
 * switches matching the physical layout on the bottom of the keyboard.
 *
 * The visual intentionally echoes the on-keyboard silkscreen: numbered
 * header 1..6, black switch bodies, a white rocker per switch pushed to
 * the top (ON) or bottom (OFF), and an "ON ↑ OFF" axis label on the left.
 */

import { useCallback, useEffect, useState } from 'react';
import { Box, HStack, Text, VStack, useToast } from '@chakra-ui/react';
import { useDaemonStore } from '../store/daemonStore';

type SwitchArray = readonly [boolean, boolean, boolean, boolean, boolean, boolean];

export function DipSwitchDisplay() {
  const client = useDaemonStore((s) => s.client);
  const daemonStatus = useDaemonStore((s) => s.status);
  const deviceConnected = useDaemonStore((s) => s.deviceConnected);
  const events = useDaemonStore((s) => s.events);
  const toast = useToast();

  const [switches, setSwitches] = useState<SwitchArray | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!client || daemonStatus !== 'online' || !deviceConnected) return;
    setLoading(true);
    setError(null);
    try {
      const raw = await client.deviceDipsw();
      if (
        !raw ||
        !Array.isArray(raw.switches) ||
        raw.switches.length !== 6
      ) {
        throw new Error('Invalid /device/dipsw response shape');
      }
      setSwitches(raw.switches as SwitchArray);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [client, daemonStatus, deviceConnected]);

  // Initial fetch + refresh on reconnect.
  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Auto-refresh when the daemon broadcasts a device_connected event —
  // DIP switches can change between power cycles.
  useEffect(() => {
    const latest = events[events.length - 1];
    if (latest?.type === 'device_connected') {
      void refresh();
    }
  }, [events, refresh]);

  const ready = switches !== null;

  // ----- Container ---------------------------------------------------------
  return (
    <VStack
      align="stretch"
      spacing={3}
      bg="bg.subtle"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="lg"
      p={4}
      role="group"
      aria-label="DIP switch state"
    >
      <HStack justify="space-between" align="center">
        <Text
          fontSize="10px"
          color="text.muted"
          fontFamily="mono"
          textTransform="uppercase"
          letterSpacing="0.08em"
        >
          DIP Switches
        </Text>
        {loading && (
          <Text fontSize="10px" color="text.muted" fontFamily="mono">
            reading…
          </Text>
        )}
        {error && (
          <Text
            fontSize="10px"
            color="danger"
            fontFamily="mono"
            title={error}
            onClick={() => toast({ title: error, status: 'error' })}
            cursor="help"
          >
            error
          </Text>
        )}
      </HStack>

      {ready ? (
        <SwitchBank switches={switches} />
      ) : (
        <Box
          bg="rgba(255,255,255,0.02)"
          border="1px dashed"
          borderColor="border.subtle"
          borderRadius="md"
          py={6}
          textAlign="center"
        >
          <Text fontSize="xs" color="text.muted">
            {daemonStatus !== 'online'
              ? 'Daemon offline — cannot read DIP switches'
              : !deviceConnected
                ? 'Device disconnected'
                : 'Reading DIP switches…'}
          </Text>
        </Box>
      )}

      <HStack justify="center">
        <Text
          fontSize="11px"
          color="text.muted"
          fontFamily="mono"
          letterSpacing="0.08em"
        >
          {ready ? describe(switches) : ''}
        </Text>
      </HStack>
    </VStack>
  );
}

// ---------------------------------------------------------------------------
// SwitchBank — pure presentational visualization
// ---------------------------------------------------------------------------

function SwitchBank({ switches }: { switches: SwitchArray }) {
  return (
    <Box
      bg="#1a1a1e"
      border="1px solid rgba(255,255,255,0.08)"
      borderRadius="md"
      px={4}
      py={3}
    >
      <HStack spacing={3} align="stretch">
        {/* ON ↑ OFF axis label */}
        <VStack spacing={0} justify="space-between" minW="30px">
          {/* top alignment of the 1..6 header */}
          <Box h="14px" />
          <VStack spacing={0} flex="1" justify="space-between" py={0.5}>
            <Text
              fontSize="11px"
              fontFamily="mono"
              color="rgba(255,255,255,0.85)"
              lineHeight="1"
            >
              ON
            </Text>
            <Text
              fontSize="11px"
              fontFamily="mono"
              color="rgba(255,255,255,0.55)"
              lineHeight="1"
            >
              ↑
            </Text>
            <Text
              fontSize="11px"
              fontFamily="mono"
              color="rgba(255,255,255,0.85)"
              lineHeight="1"
            >
              OFF
            </Text>
          </VStack>
        </VStack>

        {/* Switch column grid */}
        <HStack spacing={2} flex="1" justify="center" align="stretch">
          {switches.map((on, i) => (
            <VStack
              key={i}
              spacing={1}
              align="center"
              title={`SW${i + 1} — ${SWITCH_DESCRIPTIONS[i]}`}
            >
              <Text
                fontSize="11px"
                fontFamily="mono"
                color="rgba(255,255,255,0.85)"
                lineHeight="1"
                h="14px"
              >
                {i + 1}
              </Text>
              <Switch on={on} />
            </VStack>
          ))}
        </HStack>
      </HStack>
    </Box>
  );
}

// Single switch body — black rectangle with a white rocker pushed up or down.
function Switch({ on }: { on: boolean }) {
  const BODY_W = 26;
  const BODY_H = 58;
  const ROCKER_H = 20;
  const PAD = 3;
  return (
    <Box
      position="relative"
      w={`${BODY_W}px`}
      h={`${BODY_H}px`}
      bg="#0a0a0b"
      borderRadius="sm"
      border="1px solid rgba(255,255,255,0.15)"
      title={on ? 'ON' : 'OFF'}
      role="img"
      aria-label={on ? 'ON' : 'OFF'}
    >
      <Box
        position="absolute"
        left={`${PAD}px`}
        right={`${PAD}px`}
        top={on ? `${PAD}px` : `${BODY_H - ROCKER_H - PAD}px`}
        h={`${ROCKER_H}px`}
        bg="rgba(245,245,245,0.95)"
        borderRadius="2px"
        boxShadow={
          on
            ? 'inset 0 -1px 0 rgba(0,0,0,0.25), 0 1px 2px rgba(0,0,0,0.3)'
            : 'inset 0 1px 0 rgba(0,0,0,0.25), 0 -1px 2px rgba(0,0,0,0.3)'
        }
        transition="top 0.25s ease"
      />
    </Box>
  );
}

// ---------------------------------------------------------------------------
// describe — summarize the 6-bit DIP state using the HHKB manual's mapping
// ---------------------------------------------------------------------------

function describe(s: SwitchArray): string {
  const mode = decodeMode(s[0], s[1]);
  const swapCmdOpt = s[3] ? 'Cmd↔Opt swap' : null;
  const delBs = s[2] ? 'Delete' : 'BS';
  const wireless = s[4] ? 'power saver' : null;
  return [mode, delBs, swapCmdOpt, wireless].filter(Boolean).join(' · ');
}

// Per PFU HHKB Professional HYBRID manual:
//   SW1 SW2 → mode
//   off off → HHK
//   on  off → Lite Ext
//   off on  → Mac
//   on  on  → Special
function decodeMode(sw1: boolean, sw2: boolean): string {
  if (!sw1 && !sw2) return 'HHK';
  if (sw1 && !sw2) return 'Lite';
  if (!sw1 && sw2) return 'Mac';
  return 'Special';
}

// Hover text for each DIP switch — matches the PFU HHKB Professional HYBRID
// user manual (first Japanese edition, chapter 3 "Bottom DIP switches").
const SWITCH_DESCRIPTIONS: readonly string[] = [
  'Keyboard mode bit 1 (pairs with SW2)',
  'Keyboard mode bit 2 (pairs with SW1) — ON selects Mac mode',
  'Delete / Backspace behavior — ON sends Delete',
  'Left ⌘ / ⌥ swap — ON swaps Cmd and Option',
  'Wireless power saver — ON reduces radio wake-ups',
  'Reserved — leave OFF',
];
