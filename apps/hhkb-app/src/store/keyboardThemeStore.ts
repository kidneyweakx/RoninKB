/**
 * Visual theme for the on-screen keyboard — charcoal (PFU black model) or
 * ivory (PFU white model). Persisted in localStorage so the user's choice
 * survives reloads.
 */

import { create } from 'zustand';

export type KeyboardCaseTheme = 'charcoal' | 'ivory';

const STORAGE_KEY = 'roninKB.keyboardTheme';

function loadInitial(): KeyboardCaseTheme {
  if (typeof window === 'undefined') return 'charcoal';
  const raw = window.localStorage.getItem(STORAGE_KEY);
  return raw === 'ivory' ? 'ivory' : 'charcoal';
}

interface State {
  theme: KeyboardCaseTheme;
  setTheme: (t: KeyboardCaseTheme) => void;
  toggle: () => void;
}

export const useKeyboardThemeStore = create<State>((set, get) => ({
  theme: loadInitial(),
  setTheme(t) {
    set({ theme: t });
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(STORAGE_KEY, t);
    }
  },
  toggle() {
    const next: KeyboardCaseTheme = get().theme === 'charcoal' ? 'ivory' : 'charcoal';
    get().setTheme(next);
  },
}));

/**
 * Resolved palette for a given case theme. Returned as plain strings so SVG
 * attributes can consume them without going through Chakra tokens. Colors
 * approximate the PFU HHKB Pro HYBRID product photography.
 */
export interface CasePalette {
  caseOuter: string;
  caseGradientTop: string;
  caseGradientBottom: string;
  caseStroke: string;
  bump: string;
  plate: string;
  plateStroke: string;
  keyTop: string;
  keyBottom: string;
  keyStroke: string;
  keyStrokeSoft: string;
  keyShadow: string;
  keyLabel: string;
  keyLabelShift: string;
  keySub: string;
  keyAccent: string;
  keyAccentLabel: string;
  title: string;
  titleMuted: string;
}

const CHARCOAL: CasePalette = {
  caseOuter: '#0a0a0b',
  caseGradientTop: '#2a2a2c',
  caseGradientBottom: '#161618',
  caseStroke: '#444448',
  bump: '#3a3a3c',
  plate: '#151517',
  plateStroke: '#353539',
  keyTop: '#3c3c3f',
  keyBottom: '#2e2e31',
  keyStroke: '#4a4a4d',
  keyStrokeSoft: 'rgba(255,255,255,0.06)',
  keyShadow: 'rgba(0,0,0,0.55)',
  keyLabel: '#e3e3e6',
  keyLabelShift: '#a8a8ad',
  keySub: '#6a9bff',
  keyAccent: '#1e8fff',
  keyAccentLabel: '#ffffff',
  title: '#e3e3e6',
  titleMuted: '#8f8f94',
};

const IVORY: CasePalette = {
  caseOuter: '#bfb8a8',
  caseGradientTop: '#efeae0',
  caseGradientBottom: '#d9d3c5',
  caseStroke: '#9f9889',
  bump: '#cdc6b5',
  plate: '#e8e2d3',
  plateStroke: '#bfb8a8',
  keyTop: '#f8f4ea',
  keyBottom: '#ece6d6',
  keyStroke: '#bfb8a8',
  keyStrokeSoft: 'rgba(0,0,0,0.06)',
  keyShadow: 'rgba(120,100,60,0.18)',
  keyLabel: '#3a3a3a',
  keyLabelShift: '#6a6a6a',
  keySub: '#1e8fff',
  keyAccent: '#1e8fff',
  keyAccentLabel: '#ffffff',
  title: '#2a2a2a',
  titleMuted: '#6a6a6a',
};

export function palette(theme: KeyboardCaseTheme): CasePalette {
  return theme === 'ivory' ? IVORY : CHARCOAL;
}
