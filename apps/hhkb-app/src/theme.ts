import { extendTheme, type ThemeConfig } from '@chakra-ui/react';

const config: ThemeConfig = {
  initialColorMode: 'dark',
  useSystemColorMode: false,
};

export const theme = extendTheme({
  config,
  colors: {
    brand: {
      50: '#E6F2FF',
      100: '#B3D7FF',
      500: '#2B6CB0',
      600: '#2C5282',
      700: '#2A4365',
    },
  },
  fonts: {
    heading: `'Inter', system-ui, sans-serif`,
    body: `'Inter', system-ui, sans-serif`,
    mono: `'JetBrains Mono', ui-monospace, monospace`,
  },
});
