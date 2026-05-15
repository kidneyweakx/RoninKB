/**
 * v0.2.0 backend selection store (RFC 0001).
 *
 * Mirrors `/backend/list` + `/backend/select` from the daemon. Coexists with
 * `kanataStore` while the v0.1.x routes still drive the actual key-grab path
 * — the user-visible job today is "show the available backends and let the
 * user pick one"; deeper plumbing follows as the daemon's routing migrates.
 */

import { create } from 'zustand';
import { useDaemonStore } from './daemonStore';
import type { BackendId, BackendInfo } from '../hhkb/daemonClient';

interface BackendStoreState {
  backends: BackendInfo[];
  active: BackendId | null;
  loading: boolean;
  /** True while a `select` request is in flight, so the UI can disable the picker. */
  selecting: boolean;
  error: string | null;

  fetchList: () => Promise<void>;
  select: (id: BackendId) => Promise<void>;
}

export const useBackendStore = create<BackendStoreState>((set, get) => ({
  backends: [],
  active: null,
  loading: false,
  selecting: false,
  error: null,

  async fetchList() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    set({ loading: true, error: null });
    try {
      const res = await client.backendList();
      set({ backends: res.backends, active: res.active });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    } finally {
      set({ loading: false });
    }
  },

  async select(id: BackendId) {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    set({ selecting: true, error: null });
    try {
      const res = await client.backendSelect(id);
      set({ active: res.active });
      // Refresh list so capability + permission state matches the new active
      // backend without waiting for a manual refetch.
      void get().fetchList();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      throw e;
    } finally {
      set({ selecting: false });
    }
  },
}));
