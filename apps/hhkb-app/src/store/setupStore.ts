/**
 * First-run setup wizard state.
 *
 * `completed` is persisted in localStorage so the wizard only pops up
 * automatically once. Users can re-open it manually from the Settings
 * drawer, in which case `open` is set to true without clearing
 * `completed`.
 */

import { create } from 'zustand';

const LS_KEY = 'roninKB.setupCompleted';

function loadCompleted(): boolean {
  if (typeof localStorage === 'undefined') return false;
  try {
    return localStorage.getItem(LS_KEY) === '1';
  } catch {
    return false;
  }
}

function saveCompleted(v: boolean): void {
  if (typeof localStorage === 'undefined') return;
  try {
    if (v) localStorage.setItem(LS_KEY, '1');
    else localStorage.removeItem(LS_KEY);
  } catch {
    // ignore
  }
}

export type SetupStep = 0 | 1 | 2 | 3 | 4;
export const TOTAL_STEPS = 5;

interface SetupState {
  completed: boolean;
  currentStep: SetupStep;
  open: boolean;

  complete: () => void;
  skip: () => void;
  openManually: () => void;
  close: () => void;
  goNext: () => void;
  goBack: () => void;
  setStep: (n: SetupStep) => void;
}

export const useSetupStore = create<SetupState>((set, get) => ({
  completed: loadCompleted(),
  currentStep: 0,
  open: false,

  complete() {
    saveCompleted(true);
    set({ completed: true, open: false, currentStep: 0 });
  },
  skip() {
    saveCompleted(true);
    set({ completed: true, open: false });
  },
  openManually() {
    set({ open: true, currentStep: 0 });
  },
  close() {
    set({ open: false });
  },
  goNext() {
    const next = Math.min(get().currentStep + 1, TOTAL_STEPS - 1) as SetupStep;
    set({ currentStep: next });
  },
  goBack() {
    const prev = Math.max(get().currentStep - 1, 0) as SetupStep;
    set({ currentStep: prev });
  },
  setStep(n) {
    set({ currentStep: n });
  },
}));
