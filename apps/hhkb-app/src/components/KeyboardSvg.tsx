import { useMemo, useState } from 'react';
import { Box, useToken } from '@chakra-ui/react';
import { HHKB_LAYOUT, HhkbKey } from '../data/hhkbLayout';
import { useDeviceStore } from '../store/deviceStore';
import { useHasSoftwareOverride } from '../hooks/useKeyOrigin';
import {
  palette as casePalette,
  useKeyboardThemeStore,
} from '../store/keyboardThemeStore';

// Keyboard geometry — all in SVG user units.
const UNIT = 56;
const GAP = 6;
const PLATE_PAD = 20;
const CASE_PAD_SIDES = 22;
const CASE_PAD_BOTTOM = 28;
const CASE_PAD_TOP = 64; // room for the USB-C "bump" above the plate
// The bump (battery / cable cutout) occupies roughly half the case width,
// matching the PFU product photograph.
const BUMP_RATIO = 0.5;
const BUMP_H = 32;
const KEY_RADIUS = 6;

// Keys that the official HHKB product photo highlights as the Fn-layer
// accent (blue) — used purely for parity with the marketing shot. These are
// *not* functional overrides; overrides still show the app's own accent.
const FN_ACCENT_INDICES = new Set<number>([47, 46, 32, 6]);

interface Props {
  layer: 'base' | 'fn';
  selectedIndex: number | null;
  onSelect: (index: number) => void;
}

/**
 * HHKB Professional HYBRID visual — matches the PFU product photography:
 * a beveled case shell with a cable bump at the top, a title strip above
 * the plate, dual legends on shifted keys, and a white/charcoal toggle.
 */
export function KeyboardSvg({ layer, selectedIndex, onSelect }: Props) {
  const baseKeymap = useDeviceStore((s) => s.baseKeymap);
  const fnKeymap = useDeviceStore((s) => s.fnKeymap);
  const activeKeymap = layer === 'base' ? baseKeymap : fnKeymap;

  // Predicate: "does this HHKB key have an active kanata override in the
  // active profile's software config?" Memoised inside the hook so the
  // parse only reruns when the config string identity changes.
  const hasSoftwareOverride = useHasSoftwareOverride();
  const theme = useKeyboardThemeStore((s) => s.theme);
  const pal = casePalette(theme);

  const [hoverIndex, setHoverIndex] = useState<number | null>(null);

  const [accentSubtle] = useToken('colors', ['accent.subtle']);

  const dims = useMemo(() => {
    const maxCols = Math.max(
      ...HHKB_LAYOUT.map((k) => k.col + (k.width ?? 1)),
    );
    const rows = Math.max(...HHKB_LAYOUT.map((k) => k.row)) + 1;
    const plateW = maxCols * UNIT + (maxCols - 1) * GAP + PLATE_PAD * 2;
    const plateH = rows * UNIT + (rows - 1) * GAP + PLATE_PAD * 2;
    const width = plateW + CASE_PAD_SIDES * 2;
    const height = plateH + CASE_PAD_TOP + CASE_PAD_BOTTOM;
    return { width, height, plateW, plateH };
  }, []);

  return (
    <Box
      w="100%"
      display="flex"
      justifyContent="center"
      bg="bg.surface"
      border="1px solid"
      borderColor="border.subtle"
      borderRadius="xl"
      p={4}
      position="relative"
      overflow="hidden"
      _before={{
        content: '""',
        position: 'absolute',
        inset: 0,
        backgroundImage: `radial-gradient(
          circle at 50% 0%,
          ${accentSubtle},
          transparent 60%
        )`,
        pointerEvents: 'none',
      }}
    >
      <svg
        viewBox={`0 0 ${dims.width} ${dims.height}`}
        width="100%"
        style={{
          maxWidth: `${dims.width}px`,
          display: 'block',
          position: 'relative',
          fontFamily:
            "'Inter Variable', 'Inter', system-ui, sans-serif",
        }}
        role="group"
        aria-label={`HHKB ${layer} layer`}
      >
        <defs>
          <linearGradient id="case-gradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={pal.caseGradientTop} />
            <stop offset="100%" stopColor={pal.caseGradientBottom} />
          </linearGradient>
          <linearGradient id="key-gradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={pal.keyTop} />
            <stop offset="100%" stopColor={pal.keyBottom} />
          </linearGradient>
          <linearGradient id="key-gradient-hover" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={pal.keyBottom} />
            <stop offset="100%" stopColor={pal.keyTop} />
          </linearGradient>
          <linearGradient
            id="key-gradient-selected"
            x1="0"
            y1="0"
            x2="0"
            y2="1"
          >
            <stop offset="0%" stopColor={pal.keyAccent} stopOpacity="1" />
            <stop offset="100%" stopColor={pal.keyAccent} stopOpacity="0.85" />
          </linearGradient>
          <linearGradient
            id="key-gradient-override"
            x1="0"
            y1="0"
            x2="0"
            y2="1"
          >
            <stop offset="0%" stopColor={pal.keyAccent} stopOpacity="0.28" />
            <stop offset="100%" stopColor={pal.keyAccent} stopOpacity="0.18" />
          </linearGradient>
        </defs>

        {/* ------- Case shell ------- */}
        {/* Cable / battery bump (sits above the plate, ~50% of case width) */}
        {(() => {
          const bumpW = Math.round(dims.width * BUMP_RATIO);
          const bumpX = (dims.width - bumpW) / 2;
          const bumpY = CASE_PAD_TOP - BUMP_H - 8;
          return (
            <rect
              x={bumpX}
              y={bumpY}
              width={bumpW}
              height={BUMP_H}
              rx={8}
              ry={8}
              fill={pal.bump}
              stroke={pal.caseStroke}
              strokeWidth={1.5}
            />
          );
        })()}
        {/* Main case body (drawn after the bump so its top edge overlaps
            the bump's lower edge, making it look attached). */}
        <rect
          x={1}
          y={CASE_PAD_TOP - 14}
          width={dims.width - 2}
          height={dims.height - (CASE_PAD_TOP - 14) - 1}
          rx={16}
          ry={16}
          fill="url(#case-gradient)"
          stroke={pal.caseStroke}
          strokeWidth={1.5}
        />
        {/* Plate (recessed area where the keys sit) */}
        <rect
          x={CASE_PAD_SIDES}
          y={CASE_PAD_TOP}
          width={dims.plateW}
          height={dims.plateH}
          rx={8}
          ry={8}
          fill={pal.plate}
          stroke={pal.plateStroke}
          strokeWidth={1}
        />

        {/* ------- Keys ------- */}
        {HHKB_LAYOUT.map((k) =>
          renderKey({
            k,
            pal,
            plateOriginX: CASE_PAD_SIDES,
            plateOriginY: CASE_PAD_TOP,
            selectedIndex,
            hoverIndex,
            hasSoftwareOverride: hasSoftwareOverride(k.index),
            isHardwareModified:
              (activeKeymap?.get(k.index) ?? 0) !== 0,
            onSelect,
            setHoverIndex,
          }),
        )}

        {/* ------- Bottom meta strip ------- */}
        <text
          x={dims.width - CASE_PAD_SIDES}
          y={dims.height - 10}
          textAnchor="end"
          fontSize={9}
          fill={pal.titleMuted}
          fontFamily="'JetBrains Mono Variable', monospace"
          style={{ letterSpacing: '0.08em', textTransform: 'uppercase' }}
        >
          HHKB · {layer.toUpperCase()} LAYER
        </text>
      </svg>
    </Box>
  );
}

// ---------------------------------------------------------------------------
// Key rendering — extracted so the main component stays readable
// ---------------------------------------------------------------------------

interface RenderKeyArgs {
  k: HhkbKey;
  pal: ReturnType<typeof casePalette>;
  plateOriginX: number;
  plateOriginY: number;
  selectedIndex: number | null;
  hoverIndex: number | null;
  hasSoftwareOverride: boolean;
  /** True when the EEPROM byte for this key is non-zero (changed from factory). */
  isHardwareModified: boolean;
  onSelect: (index: number) => void;
  setHoverIndex: (i: number | null) => void;
}

function renderKey({
  k,
  pal,
  plateOriginX,
  plateOriginY,
  selectedIndex,
  hoverIndex,
  hasSoftwareOverride,
  isHardwareModified,
  onSelect,
  setHoverIndex,
}: RenderKeyArgs) {
  const w = k.width ?? 1;
  const x = plateOriginX + PLATE_PAD + k.col * (UNIT + GAP);
  const y = plateOriginY + PLATE_PAD + k.row * (UNIT + GAP);
  const keyWidth = w * UNIT + (w - 1) * GAP;

  // A key is considered "overridden" when the active profile's kanata
  // software config has a non-passthrough token at this slot. The
  // hardware EEPROM value is deliberately *not* part of the signal —
  // differences against the firmware default are surfaced via the
  // page-level `<SyncBanner />` instead.
  const isOverridden = hasSoftwareOverride;
  const isSelected = k.index === selectedIndex;
  const isHover = k.index === hoverIndex;
  const isFnAccent = FN_ACCENT_INDICES.has(k.index);

  const fill = isSelected
    ? 'url(#key-gradient-selected)'
    : isHover
      ? 'url(#key-gradient-hover)'
      : isFnAccent
        ? 'url(#key-gradient-selected)'
        : isOverridden
          ? 'url(#key-gradient-override)'
          : 'url(#key-gradient)';

  const stroke =
    isSelected || isFnAccent || isOverridden ? pal.keyAccent : pal.keyStroke;

  const labelColor =
    isSelected || isFnAccent ? pal.keyAccentLabel : pal.keyLabel;
  const shiftColor =
    isSelected || isFnAccent ? pal.keyAccentLabel : pal.keyLabelShift;

  // Font sizing heuristic
  const label = k.label;
  const labelLen = label.length;
  const bigFont = 14;
  const medFont = 12;
  const smallFont = 10;
  const primaryFont =
    labelLen <= 1 ? bigFont : labelLen <= 3 ? medFont : smallFont;
  const shiftFont = 9;
  const subFont = 8;

  // Layout: if we have a shift legend, stack shift on top and label below
  // (classic ANSI legend layout). Otherwise center the primary label.
  const hasShift = !!k.shift;
  const cx = x + keyWidth / 2;
  const cy = y + UNIT / 2;
  const primaryY = hasShift ? y + UNIT * 0.68 : cy;
  const shiftY = y + UNIT * 0.32;

  return (
    <g
      key={`${k.row}-${k.col}-${k.label}-${k.index}`}
      onClick={() => onSelect(k.index)}
      onMouseEnter={() => setHoverIndex(k.index)}
      onMouseLeave={() => setHoverIndex(null)}
      style={{ cursor: 'pointer' }}
      role="button"
      aria-label={`${label} (index ${k.index})`}
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onSelect(k.index);
        }
      }}
    >
      {/* Shadow */}
      <rect
        x={x}
        y={y + 2.5}
        width={keyWidth}
        height={UNIT}
        rx={KEY_RADIUS}
        ry={KEY_RADIUS}
        fill={pal.keyShadow}
      />
      {/* Key body */}
      <rect
        x={x}
        y={y}
        width={keyWidth}
        height={UNIT}
        rx={KEY_RADIUS}
        ry={KEY_RADIUS}
        fill={fill}
        stroke={stroke}
        strokeWidth={isSelected ? 1.5 : 1}
        style={{
          transition: 'fill 0.15s ease, stroke 0.15s ease',
        }}
      />
      {/* Inner highlight line */}
      <rect
        x={x + 1.5}
        y={y + 1.5}
        width={keyWidth - 3}
        height={UNIT - 3}
        rx={KEY_RADIUS - 1}
        ry={KEY_RADIUS - 1}
        fill="none"
        stroke={pal.keyStrokeSoft}
        strokeWidth={1}
        pointerEvents="none"
      />

      {/* Shift legend (top) */}
      {hasShift && (
        <text
          x={cx}
          y={shiftY}
          textAnchor="middle"
          dominantBaseline="central"
          fontSize={shiftFont}
          fontWeight={500}
          fill={shiftColor}
          style={{
            userSelect: 'none',
            letterSpacing: '-0.01em',
          }}
          pointerEvents="none"
        >
          {k.shift}
        </text>
      )}

      {/* Primary label (bottom if shift present, center otherwise) */}
      <text
        x={cx}
        y={primaryY}
        textAnchor="middle"
        dominantBaseline="central"
        fontSize={primaryFont}
        fontWeight={500}
        fill={labelColor}
        style={{
          userSelect: 'none',
          transition: 'fill 0.15s ease',
          letterSpacing: '-0.01em',
        }}
        pointerEvents="none"
      >
        {label}
      </text>

      {/* Subscript annotation (bottom-right, e.g. "R") */}
      {k.sub && (
        <text
          x={x + keyWidth - 5}
          y={y + UNIT - 5}
          textAnchor="end"
          fontSize={subFont}
          fontWeight={600}
          fill={pal.keySub}
          style={{ userSelect: 'none' }}
          pointerEvents="none"
        >
          {k.sub}
        </text>
      )}

      {/* Software-override dot — top-left corner, accent purple */}
      {isOverridden && !isSelected && (
        <circle
          cx={x + 6}
          cy={y + 6}
          r={2.5}
          fill={pal.keyAccent}
          pointerEvents="none"
        />
      )}

      {/* Hardware-modified dot — top-right corner, orange warning color.
          Shown when the EEPROM byte is non-zero (changed from factory default).
          Mirrors the blue highlight in the official HHKB Keymap Tool. */}
      {isHardwareModified && !isSelected && (
        <circle
          cx={x + keyWidth - 6}
          cy={y + 6}
          r={2.5}
          fill="#f97316"
          pointerEvents="none"
        />
      )}
    </g>
  );
}
