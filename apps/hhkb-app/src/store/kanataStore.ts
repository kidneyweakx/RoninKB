/**
 * Kanata supervisor store.
 *
 * Tracks the running state of the kanata key-remapping process managed by
 * the daemon. Polls every 10 seconds when the daemon is online; state is
 * also updated reactively via WebSocket events forwarded from daemonStore.
 */

import { create } from 'zustand';
import { useDaemonStore } from './daemonStore';

export type KanataProcessState = 'not_installed' | 'stopped' | 'running';

interface KanataState {
  installed: boolean;
  processState: KanataProcessState;
  pid: number | null;
  configPath: string | null;
  loading: boolean;
  error: string | null;

  fetchStatus: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  startPolling: () => void;
  stopPolling: () => void;
}

let pollTimer: ReturnType<typeof setInterval> | null = null;

export const useKanataStore = create<KanataState>((set, get) => ({
  installed: false,
  processState: 'not_installed',
  pid: null,
  configPath: null,
  loading: false,
  error: null,

  async fetchStatus() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    set({ loading: true, error: null });
    try {
      const raw = await client.kanataStatus();
      let processState: KanataProcessState = 'not_installed';
      if (raw.installed) {
        processState = raw.state === 'running' ? 'running' : 'stopped';
      }
      set({
        installed: raw.installed,
        processState,
        pid: raw.pid ?? null,
        configPath: raw.path ?? null,
      });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    } finally {
      set({ loading: false });
    }
  },

  async start() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    set({ loading: true, error: null });
    try {
      const pid = await client.kanataStart();
      set({ processState: 'running', pid, loading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e), loading: false });
      throw e;
    }
  },

  async stop() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    set({ loading: true, error: null });
    try {
      await client.kanataStop();
      set({ processState: 'stopped', pid: null, loading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e), loading: false });
      throw e;
    }
  },

  startPolling() {
    if (pollTimer) return;
    void get().fetchStatus();
    pollTimer = setInterval(() => {
      void get().fetchStatus();
    }, 10_000);
  },

  stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  },
}));
