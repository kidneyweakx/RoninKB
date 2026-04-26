import { useId } from 'react';

interface RoninLogoProps {
  connected?: boolean;
  size?: number;
  strokeWidth?: number;
}

/**
 * RoninKB brand mark — sharp vertical lozenge with an inscribed smaller
 * lozenge. Composition inverts on connection state:
 *  - disconnected: hollow outer, solid inner (a glyph standing in waiting)
 *  - connected:    solid outer with a cutout inner (a filled blade)
 */
export function RoninLogo({
  connected = false,
  size = 18,
  strokeWidth = 1.75,
}: RoninLogoProps) {
  const reactId = useId();
  const maskId = `rkb-mark-cut-${reactId.replace(/:/g, '')}`;

  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      aria-hidden="true"
      style={{ display: 'block', overflow: 'visible' }}
    >
      <defs>
        <mask id={maskId} maskUnits="userSpaceOnUse">
          <rect x="0" y="0" width="24" height="24" fill="white" />
          <path
            d="M12 7.5 L16.5 12 L12 16.5 L7.5 12 Z"
            fill="black"
          />
        </mask>
      </defs>

      {/* Outer sharp lozenge */}
      <path
        d="M12 1.5 L21.5 12 L12 22.5 L2.5 12 Z"
        fill={connected ? 'currentColor' : 'none'}
        stroke="currentColor"
        strokeWidth={strokeWidth}
        strokeLinejoin="miter"
        mask={connected ? `url(#${maskId})` : undefined}
        style={{ transition: 'fill 180ms ease' }}
      />

      {/* Inner small lozenge — solid only when disconnected */}
      {!connected && (
        <path
          d="M12 7.5 L16.5 12 L12 16.5 L7.5 12 Z"
          fill="currentColor"
        />
      )}
    </svg>
  );
}
