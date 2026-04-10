/**
 * Small session-only UI store for transient flags that don't belong to
 * any domain store. Currently only hosts the sync-banner dismissal flag.
 *
 * This is NOT persisted to localStorage — dismissals should come back on
 * the next reload so the user can't accidentally ignore a drift forever.
 */

import { create } from 'zustand';
import { useDeviceStore } from './deviceStore';

interface UiState {
  syncBannerDismissed: boolean;
  dismissSyncBanner: () => void;
  resetSyncBanner: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  syncBannerDismissed: false,
  dismissSyncBanner() {
    set({ syncBannerDismissed: true });
  },
  resetSyncBanner() {
    set({ syncBannerDismissed: false });
  },
}));

// Whenever the device's keymap changes, reset the dismissal — a new diff
// deserves a fresh decision from the user.
let lastBase: unknown = null;
let lastFn: unknown = null;
useDeviceStore.subscribe((state) => {
  if (state.baseKeymap !== lastBase || state.fnKeymap !== lastFn) {
    lastBase = state.baseKeymap;
    lastFn = state.fnKeymap;
    useUiStore.getState().resetSyncBanner();
  }
});
