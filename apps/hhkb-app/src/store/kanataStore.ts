/**
 * Kanata supervisor store.
 *
 * Tracks the running state of the kanata key-remapping process managed by
 * the daemon. Polls every 10 seconds when the daemon is online; state is
 * also updated reactively via WebSocket events forwarded from daemonStore.
 */

import { create } from 'zustand';
import { useDaemonStore } from './daemonStore';
import type {
  KanataDriverActivateOutcome,
  KanataDriverState,
} from '../hhkb/daemonClient';

export type KanataProcessState = 'not_installed' | 'stopped' | 'running';

interface KanataState {
  installed: boolean;
  processState: KanataProcessState;
  pid: number | null;
  binaryPath: string | null;
  configPath: string | null;
  inputMonitoringGranted: boolean | null;
  /**
   * Legacy bool projection of `driverState`. Kept while v0.1.x components
   * migrate; new UI should read `driverState` for the granular flavour.
   */
  driverActivated: boolean | null;
  /**
   * Granular macOS-only Karabiner DriverKit sysext state — see
   * `KanataDriverState`. `null` on non-macOS or when the daemon couldn't tell.
   */
  driverState: KanataDriverState | null;
  devicePath: string | null;
  stderrTail: string[];
  loading: boolean;
  error: string | null;
  /** True while `driverActivate()` is in flight, so UI can disable buttons. */
  driverActivating: boolean;

  fetchStatus: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  /**
   * Trigger Karabiner sysext registration. Refreshes status afterwards so the
   * panel reflects the new state without waiting for the next poll tick.
   */
  driverActivate: () => Promise<KanataDriverActivateOutcome>;
  /** Open System Settings -> Driver Extensions. macOS-only. */
  driverOpenSettings: () => Promise<void>;
  startPolling: () => void;
  stopPolling: () => void;
}

let pollTimer: ReturnType<typeof setInterval> | null = null;

export const useKanataStore = create<KanataState>((set, get) => ({
  installed: false,
  processState: 'not_installed',
  pid: null,
  binaryPath: null,
  configPath: null,
  inputMonitoringGranted: null,
  driverActivated: null,
  driverState: null,
  devicePath: null,
  stderrTail: [],
  loading: false,
  error: null,
  driverActivating: false,

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
        binaryPath: raw.binaryPath ?? null,
        configPath: raw.path ?? null,
        inputMonitoringGranted: raw.inputMonitoringGranted ?? null,
        driverActivated: raw.driverActivated ?? null,
        driverState: raw.driverState ?? null,
        devicePath: raw.devicePath ?? null,
        stderrTail: raw.stderrTail ?? [],
        error: raw.lastError ?? null,
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

  async driverActivate() {
    const client = useDaemonStore.getState().client;
    if (!client) {
      throw new Error('daemon offline');
    }
    set({ driverActivating: true, error: null });
    try {
      const res = await client.kanataDriverActivate();
      // Optimistic refresh — triggered() returns the post-call state, but
      // refetching reconciles any other fields the user might be watching.
      set({
        driverState: res.driver_state,
        driverActivated: res.driver_state === 'activated',
      });
      void get().fetchStatus();
      return res.result;
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      throw e;
    } finally {
      set({ driverActivating: false });
    }
  },

  async driverOpenSettings() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      await client.kanataDriverOpenSettings();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
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
