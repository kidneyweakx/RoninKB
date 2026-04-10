import { extendTheme, type ThemeConfig } from '@chakra-ui/react';
import { mode, type StyleFunctionProps } from '@chakra-ui/theme-tools';

/**
 * RoninKB theme — opinionated, Linear/Raycast/Vercel-inspired.
 *
 * Dark mode is the canonical experience. Light mode is supported but
 * secondary. The palette is deliberately restrained: near-black surfaces,
 * zinc-family grays, and a single violet accent.
 */

const config: ThemeConfig = {
  initialColorMode: 'dark',
  useSystemColorMode: false,
};

const semanticTokens = {
  colors: {
    // Backgrounds
    'bg.primary': { default: '#FAFAFA', _dark: '#0A0A0B' },
    'bg.surface': { default: '#FFFFFF', _dark: '#131316' },
    'bg.elevated': { default: '#FFFFFF', _dark: '#1C1C21' },
    'bg.subtle': { default: '#F4F4F5', _dark: '#232328' },

    // Borders
    'border.subtle': { default: '#E4E4E7', _dark: '#2A2A30' },
    'border.muted': { default: '#D4D4D8', _dark: '#34343C' },
    'border.strong': { default: '#A1A1AA', _dark: '#4A4A55' },

    // Text
    'text.primary': { default: '#18181B', _dark: '#F5F5F7' },
    'text.secondary': { default: '#3F3F46', _dark: '#A1A1AA' },
    'text.muted': { default: '#71717A', _dark: '#71717A' },
    'text.disabled': { default: '#A1A1AA', _dark: '#52525B' },

    // Accent — violet-600, signature color
    'accent.primary': { default: '#7C3AED', _dark: '#7C3AED' },
    'accent.hover': { default: '#8B5CF6', _dark: '#8B5CF6' },
    'accent.subtle': {
      default: 'rgba(124, 58, 237, 0.10)',
      _dark: 'rgba(124, 58, 237, 0.15)',
    },
    'accent.fg': { default: '#FFFFFF', _dark: '#FFFFFF' },

    // Status
    success: { default: '#059669', _dark: '#10B981' },
    warning: { default: '#D97706', _dark: '#F59E0B' },
    danger: { default: '#DC2626', _dark: '#EF4444' },
    info: { default: '#2563EB', _dark: '#3B82F6' },

    'success.subtle': {
      default: 'rgba(16, 185, 129, 0.10)',
      _dark: 'rgba(16, 185, 129, 0.12)',
    },
    'warning.subtle': {
      default: 'rgba(245, 158, 11, 0.10)',
      _dark: 'rgba(245, 158, 11, 0.12)',
    },
    'danger.subtle': {
      default: 'rgba(239, 68, 68, 0.10)',
      _dark: 'rgba(239, 68, 68, 0.12)',
    },
  },
};

const fonts = {
  heading: `'Inter Variable', 'Inter', system-ui, -apple-system, sans-serif`,
  body: `'Inter Variable', 'Inter', system-ui, -apple-system, sans-serif`,
  mono: `'JetBrains Mono Variable', 'JetBrains Mono', 'SF Mono', Menlo, Consolas, monospace`,
};

const radii = {
  sm: '4px',
  md: '6px',
  lg: '8px',
  xl: '12px',
  '2xl': '16px',
};

const shadows = {
  subtle: '0 1px 2px rgba(0,0,0,0.05), 0 2px 4px rgba(0,0,0,0.04)',
  card: '0 1px 3px rgba(0,0,0,0.06), 0 4px 12px rgba(0,0,0,0.04)',
  elevated: '0 4px 20px rgba(0,0,0,0.08), 0 1px 3px rgba(0,0,0,0.04)',
  glow: '0 0 0 1px rgba(124, 58, 237, 0.3), 0 4px 16px rgba(124, 58, 237, 0.2)',
  focus: '0 0 0 2px rgba(124, 58, 237, 0.4)',
};

const styles = {
  global: (props: StyleFunctionProps) => ({
    'html, body, #root': {
      bg: 'bg.primary',
      color: 'text.primary',
      fontFeatureSettings: '"cv11", "ss01", "ss03"',
      WebkitFontSmoothing: 'antialiased',
      MozOsxFontSmoothing: 'grayscale',
    },
    body: {
      // Subtle radial gradient + noise feel via layered bg.
      backgroundImage: mode(
        'radial-gradient(ellipse at top, rgba(124, 58, 237, 0.04), transparent 60%)',
        'radial-gradient(ellipse at top, rgba(124, 58, 237, 0.08), transparent 55%)',
      )(props),
      backgroundAttachment: 'fixed',
    },
    '*, *::before, *::after': {
      borderColor: 'border.subtle',
    },
    '*::selection': {
      backgroundColor: 'rgba(124, 58, 237, 0.35)',
      color: 'text.primary',
    },
    // Reduced-motion respect
    '@media (prefers-reduced-motion: reduce)': {
      '*, *::before, *::after': {
        animationDuration: '0.01ms !important',
        transitionDuration: '0.01ms !important',
      },
    },
    // Scrollbars
    '::-webkit-scrollbar': { width: '10px', height: '10px' },
    '::-webkit-scrollbar-track': { background: 'transparent' },
    '::-webkit-scrollbar-thumb': {
      background: mode('#D4D4D8', '#2A2A30')(props),
      borderRadius: '8px',
      border: mode(
        '2px solid #FAFAFA',
        '2px solid #0A0A0B',
      )(props),
    },
    '::-webkit-scrollbar-thumb:hover': {
      background: mode('#A1A1AA', '#4A4A55')(props),
    },
  }),
};

const Button = {
  baseStyle: {
    fontWeight: 500,
    borderRadius: 'md',
    letterSpacing: '-0.01em',
    transition:
      'background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease',
    _focusVisible: {
      boxShadow: 'focus',
      outline: 'none',
    },
  },
  sizes: {
    xs: { h: '26px', fontSize: 'xs', px: 2 },
    sm: { h: '32px', fontSize: 'sm', px: 3 },
    md: { h: '38px', fontSize: 'sm', px: 4 },
    lg: { h: '44px', fontSize: 'md', px: 5 },
  },
  variants: {
    solid: {
      bg: 'accent.primary',
      color: 'accent.fg',
      _hover: {
        bg: 'accent.hover',
        _disabled: { bg: 'accent.primary' },
      },
      _active: { bg: 'accent.primary' },
    },
    subtle: {
      bg: 'bg.subtle',
      color: 'text.primary',
      border: '1px solid',
      borderColor: 'border.subtle',
      _hover: {
        bg: 'bg.elevated',
        borderColor: 'border.muted',
      },
      _active: { bg: 'bg.subtle' },
    },
    ghost: {
      bg: 'transparent',
      color: 'text.secondary',
      _hover: {
        bg: 'bg.subtle',
        color: 'text.primary',
      },
      _active: { bg: 'bg.subtle' },
    },
    outline: {
      bg: 'transparent',
      color: 'text.primary',
      border: '1px solid',
      borderColor: 'border.muted',
      _hover: {
        bg: 'bg.subtle',
        borderColor: 'border.strong',
      },
    },
    danger: {
      bg: 'danger.subtle',
      color: 'danger',
      border: '1px solid',
      borderColor: 'transparent',
      _hover: {
        bg: 'danger',
        color: 'white',
      },
    },
    link: {
      bg: 'transparent',
      color: 'accent.primary',
      h: 'auto',
      px: 0,
      _hover: { textDecoration: 'underline' },
    },
  },
  defaultProps: {
    variant: 'subtle',
    size: 'sm',
  },
};

const Input = {
  baseStyle: {
    field: {
      fontFamily: 'body',
    },
  },
  variants: {
    outline: {
      field: {
        bg: 'bg.surface',
        borderColor: 'border.muted',
        color: 'text.primary',
        borderRadius: 'sm',
        _hover: { borderColor: 'border.strong' },
        _focusVisible: {
          borderColor: 'accent.primary',
          boxShadow: '0 0 0 1px #7C3AED',
        },
        _placeholder: { color: 'text.muted' },
      },
    },
    filled: {
      field: {
        bg: 'bg.subtle',
        borderColor: 'transparent',
        color: 'text.primary',
        borderRadius: 'sm',
        _hover: { bg: 'bg.elevated' },
        _focusVisible: {
          bg: 'bg.surface',
          borderColor: 'accent.primary',
        },
      },
    },
  },
  defaultProps: { variant: 'outline', size: 'sm' },
};

const Menu = {
  baseStyle: {
    list: {
      bg: 'bg.elevated',
      borderColor: 'border.subtle',
      borderRadius: 'lg',
      boxShadow: 'elevated',
      py: 1,
      minW: '220px',
    },
    item: {
      bg: 'transparent',
      color: 'text.primary',
      fontSize: 'sm',
      px: 3,
      py: 2,
      borderRadius: 'sm',
      mx: 1,
      transition: 'background-color 0.15s ease, color 0.15s ease',
      _hover: { bg: 'bg.subtle' },
      _focus: { bg: 'bg.subtle' },
    },
    divider: {
      borderColor: 'border.subtle',
      my: 1,
    },
    groupTitle: {
      color: 'text.muted',
      fontSize: 'xs',
      fontWeight: 500,
      textTransform: 'uppercase',
      letterSpacing: '0.05em',
      px: 3,
      py: 1,
    },
  },
};

const Modal = {
  baseStyle: {
    overlay: {
      bg: 'rgba(0, 0, 0, 0.6)',
      backdropFilter: 'blur(8px)',
    },
    dialog: {
      bg: 'bg.elevated',
      color: 'text.primary',
      borderRadius: 'xl',
      border: '1px solid',
      borderColor: 'border.subtle',
      boxShadow: 'elevated',
    },
    header: {
      fontSize: 'md',
      fontWeight: 600,
      letterSpacing: '-0.01em',
      borderBottom: '1px solid',
      borderColor: 'border.subtle',
      px: 5,
      py: 4,
    },
    body: { px: 5, py: 5 },
    footer: {
      borderTop: '1px solid',
      borderColor: 'border.subtle',
      px: 5,
      py: 4,
    },
    closeButton: {
      color: 'text.muted',
      _hover: { bg: 'bg.subtle', color: 'text.primary' },
    },
  },
};

const Tag = {
  baseStyle: {
    container: {
      fontWeight: 500,
      letterSpacing: '0.01em',
      borderRadius: 'sm',
      fontSize: 'xs',
    },
  },
  variants: {
    subtle: {
      container: {
        bg: 'bg.subtle',
        color: 'text.secondary',
        border: '1px solid',
        borderColor: 'border.subtle',
      },
    },
    accent: {
      container: {
        bg: 'accent.subtle',
        color: 'accent.primary',
      },
    },
    success: {
      container: {
        bg: 'success.subtle',
        color: 'success',
      },
    },
    warning: {
      container: {
        bg: 'warning.subtle',
        color: 'warning',
      },
    },
    danger: {
      container: {
        bg: 'danger.subtle',
        color: 'danger',
      },
    },
  },
  defaultProps: { variant: 'subtle' },
};

const Badge = {
  baseStyle: {
    fontWeight: 500,
    textTransform: 'none',
    letterSpacing: '0.01em',
    borderRadius: 'sm',
    px: 1.5,
    py: 0.5,
    fontSize: '10px',
  },
  variants: {
    subtle: {
      bg: 'bg.subtle',
      color: 'text.secondary',
      border: '1px solid',
      borderColor: 'border.subtle',
    },
    accent: {
      bg: 'accent.subtle',
      color: 'accent.primary',
    },
  },
  defaultProps: { variant: 'subtle' },
};

const Heading = {
  baseStyle: {
    fontWeight: 600,
    letterSpacing: '-0.02em',
    color: 'text.primary',
  },
};

const Text = {
  baseStyle: {
    color: 'text.primary',
  },
};

const Divider = {
  baseStyle: {
    borderColor: 'border.subtle',
    opacity: 1,
  },
};

const Tooltip = {
  baseStyle: {
    bg: 'bg.elevated',
    color: 'text.primary',
    borderRadius: 'sm',
    border: '1px solid',
    borderColor: 'border.subtle',
    fontSize: 'xs',
    px: 2,
    py: 1,
    boxShadow: 'elevated',
  },
};

export const theme = extendTheme({
  config,
  semanticTokens,
  fonts,
  radii,
  shadows,
  styles,
  components: {
    Button,
    Input,
    Menu,
    Modal,
    Tag,
    Badge,
    Heading,
    Text,
    Divider,
    Tooltip,
  },
});
