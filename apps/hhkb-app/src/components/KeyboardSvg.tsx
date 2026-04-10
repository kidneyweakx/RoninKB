import { useMemo, useState } from 'react';
import { Box, useToken } from '@chakra-ui/react';
import { HHKB_LAYOUT } from '../data/hhkbLayout';
import { useDeviceStore } from '../store/deviceStore';

const UNIT = 56;
const GAP = 6;
const PAD = 20;
const KEY_RADIUS = 6;

interface Props {
  layer: 'base' | 'fn';
  selectedIndex: number | null;
  onSelect: (index: number) => void;
}

/**
 * Real SVG rendering of the HHKB Pro layout. Every key is a `<rect>` with
 * a `<text>` label positioned in the middle. Hover and selected states are
 * implemented via color changes only — no transforms, no layout shift.
 */
export function KeyboardSvg({ layer, selectedIndex, onSelect }: Props) {
  const baseKeymap = useDeviceStore((s) => s.baseKeymap);
  const fnKeymap = useDeviceStore((s) => s.fnKeymap);
  const keymap = layer === 'base' ? baseKeymap : fnKeymap;
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);

  const [
    bgSurface,
    bgSubtle,
    bgElevated,
    borderSubtle,
    borderMuted,
    accent,
    accentSubtle,
    textPrimary,
    textMuted,
  ] = useToken('colors', [
    'bg.surface',
    'bg.subtle',
    'bg.elevated',
    'border.subtle',
    'border.muted',
    'accent.primary',
    'accent.subtle',
    'text.primary',
    'text.muted',
  ]);

  const dims = useMemo(() => {
    const maxCols = Math.max(
      ...HHKB_LAYOUT.map((k) => k.col + (k.width ?? 1)),
    );
    const rows = Math.max(...HHKB_LAYOUT.map((k) => k.row)) + 1;
    const width = maxCols * UNIT + (maxCols - 1) * GAP + PAD * 2;
    const height = rows * UNIT + (rows - 1) * GAP + PAD * 2;
    return { width, height };
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
          <linearGradient id="key-gradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={bgElevated} />
            <stop offset="100%" stopColor={bgSubtle} />
          </linearGradient>
          <linearGradient id="key-gradient-hover" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={bgSubtle} />
            <stop offset="100%" stopColor={bgElevated} />
          </linearGradient>
          <linearGradient id="key-gradient-selected" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={accent} stopOpacity="1" />
            <stop offset="100%" stopColor={accent} stopOpacity="0.85" />
          </linearGradient>
          <linearGradient id="key-gradient-override" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={accentSubtle} />
            <stop offset="100%" stopColor={accentSubtle} />
          </linearGradient>
        </defs>

        {/* Keyboard chassis */}
        <rect
          x={0.5}
          y={0.5}
          width={dims.width - 1}
          height={dims.height - 1}
          rx={12}
          ry={12}
          fill={bgSurface}
          stroke={borderSubtle}
          strokeWidth={1}
        />

        {HHKB_LAYOUT.map((k) => {
          const w = k.width ?? 1;
          const x = PAD + k.col * (UNIT + GAP);
          const y = PAD + k.row * (UNIT + GAP);
          const keyWidth = w * UNIT + (w - 1) * GAP;
          const override = keymap?.get(k.index) ?? 0;
          const isOverridden = override !== 0;
          const isSelected = k.index === selectedIndex;
          const isHover = k.index === hoverIndex;

          const fill = isSelected
            ? 'url(#key-gradient-selected)'
            : isHover
              ? 'url(#key-gradient-hover)'
              : isOverridden
                ? 'url(#key-gradient-override)'
                : 'url(#key-gradient)';

          const stroke = isSelected
            ? accent
            : isOverridden
              ? accent
              : borderMuted;

          const labelColor = isSelected
            ? '#FFFFFF'
            : isOverridden
              ? accent
              : textPrimary;

          // Font sizing heuristic: shorter labels get larger font.
          const label = k.label;
          const isLong = label.length > 2;
          const fontSize = isLong ? 11 : 13;

          return (
            <g
              key={`${k.row}-${k.col}-${k.label}`}
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
              {/* Shadow / depth */}
              <rect
                x={x}
                y={y + 2}
                width={keyWidth}
                height={UNIT}
                rx={KEY_RADIUS}
                ry={KEY_RADIUS}
                fill="rgba(0,0,0,0.25)"
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
                  transition:
                    'fill 0.15s ease, stroke 0.15s ease',
                }}
              />
              {/* Inner highlight */}
              <rect
                x={x + 1.5}
                y={y + 1.5}
                width={keyWidth - 3}
                height={UNIT - 3}
                rx={KEY_RADIUS - 1}
                ry={KEY_RADIUS - 1}
                fill="none"
                stroke="rgba(255,255,255,0.04)"
                strokeWidth={1}
                pointerEvents="none"
              />
              {/* Label */}
              <text
                x={x + keyWidth / 2}
                y={y + UNIT / 2}
                textAnchor="middle"
                dominantBaseline="central"
                fontSize={fontSize}
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
              {/* Override dot indicator */}
              {isOverridden && !isSelected && (
                <circle
                  cx={x + keyWidth - 6}
                  cy={y + 6}
                  r={2}
                  fill={accent}
                  pointerEvents="none"
                />
              )}
            </g>
          );
        })}

        {/* Bottom meta strip */}
        <text
          x={dims.width - PAD}
          y={dims.height - 6}
          textAnchor="end"
          fontSize={9}
          fill={textMuted}
          fontFamily="'JetBrains Mono Variable', monospace"
          style={{ letterSpacing: '0.08em', textTransform: 'uppercase' }}
        >
          HHKB · {layer.toUpperCase()} LAYER
        </text>
      </svg>
    </Box>
  );
}
