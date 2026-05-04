/**
 * First-run setup wizard state.
 *
 * `completed` is persisted in localStorage so the wizard only pops up
 * automatically once. Users can re-open it manually from the Settings
 * drawer, in which case `open` is set to true without clearing
 * `completed`.
 *
 * v0.2.0 (RFC 0001 §7.2 / M4 §85): the wizard now drives backend
 * selection + verification. The user's verified-binding self-attestation
 * is recorded as `verifiedBackend` so the wizard can refuse to "complete"
 * until something has actually been tested. Cleared whenever the user
 * switches backend mid-wizard so they can't accidentally carry a
 * verification for a different engine.
 */

import { create } from 'zustand';
import type { BackendId } from '../hhkb/daemonClient';

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
  /**
   * The backend the user successfully tested in the verification step.
   * `null` until they tick the verification checkbox. Cleared when they
   * switch to a different backend so verification doesn't leak across
   * engines.
   */
  verifiedBackend: BackendId | null;

  complete: () => void;
  skip: () => void;
  openManually: () => void;
  close: () => void;
  goNext: () => void;
  goBack: () => void;
  setStep: (n: SetupStep) => void;
  markVerified: (id: BackendId) => void;
  clearVerified: () => void;
}

export const useSetupStore = create<SetupState>((set, get) => ({
  completed: loadCompleted(),
  currentStep: 0,
  open: false,
  verifiedBackend: null,

  complete() {
    saveCompleted(true);
    set({ completed: true, open: false, currentStep: 0 });
  },
  skip() {
    saveCompleted(true);
    set({ completed: true, open: false });
  },
  openManually() {
    set({ open: true, currentStep: 0, verifiedBackend: null });
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
  markVerified(id) {
    set({ verifiedBackend: id });
  },
  clearVerified() {
    set({ verifiedBackend: null });
  },
}));
