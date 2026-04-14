/**
 * Daemon detection store.
 *
 * Probes `http://localhost:7331/health` on startup and then every 30 seconds.
 * When the daemon responds OK, a `DaemonClient` is attached to the store and
 * a `DaemonWebSocket` is connected so other stores can react to push events.
 *
 * If the daemon is not reachable, the app still works in WebHID-only mode
 * and shows a banner prompting the user to install it for advanced features.
 */

import { create } from 'zustand';
import {
  DaemonClient,
  DaemonEvent,
  DaemonWebSocket,
} from '../hhkb/daemonClient';
import { useBluetoothStore } from './bluetoothStore';

export const DAEMON_URL = 'http://localhost:7331';
export const DAEMON_HEALTH_URL = `${DAEMON_URL}/health`;

export type DaemonStatus = 'unknown' | 'online' | 'offline';

const POLL_INTERVAL_MS = 30_000;
const MAX_EVENTS = 10;

interface DaemonState {
  status: DaemonStatus;
  bannerDismissed: boolean;
  lastCheckedAt: number | null;

  client: DaemonClient | null;
  version: string | null;
  deviceConnected: boolean;
  events: DaemonEvent[];

  check: () => Promise<void>;
  startAutoPoll: () => void;
  stopAutoPoll: () => void;
  dismissBanner: () => void;
  pushEvent: (e: DaemonEvent) => void;
}

let pollTimer: ReturnType<typeof setInterval> | null = null;
let ws: DaemonWebSocket | null = null;

export const useDaemonStore = create<DaemonState>((set, get) => ({
  status: 'unknown',
  bannerDismissed: false,
  lastCheckedAt: null,
  client: null,
  version: null,
  deviceConnected: false,
  events: [],

  async check() {
    const client = get().client ?? new DaemonClient(DAEMON_URL);
    try {
      const health = await client.health();
      const wasOnline = get().status === 'online';
      set({
        status: 'online',
        client,
        version: health.version,
        deviceConnected: health.device_connected,
        lastCheckedAt: Date.now(),
      });
      if (!wasOnline && !ws) {
        ws = new DaemonWebSocket((e) => get().pushEvent(e));
        ws.connect();
        // Probe for already-connected BLE devices once on transition to online
        void useBluetoothStore.getState().startScanSilent();
      }
    } catch {
      set({
        status: 'offline',
        client: null,
        version: null,
        deviceConnected: false,
        lastCheckedAt: Date.now(),
      });
      if (ws) {
        ws.disconnect();
        ws = null;
      }
    }
  },

  startAutoPoll() {
    if (pollTimer) return;
    pollTimer = setInterval(() => {
      void get().check();
    }, POLL_INTERVAL_MS);
  },

  stopAutoPoll() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    if (ws) {
      ws.disconnect();
      ws = null;
    }
  },

  dismissBanner() {
    set({ bannerDismissed: true });
  },

  pushEvent(e) {
    // Update derived flags that mirror daemon-pushed device state.
    if (e.type === 'device_connected') {
      set({ deviceConnected: true });
    } else if (e.type === 'device_disconnected') {
      set({ deviceConnected: false });
    }
    if (e.type === 'bluetooth_scan_complete') {
      useBluetoothStore.getState().onScanComplete(e.devices);
    }
    // Profile events
    if (e.type === 'profile_changed') {
      void import('./profileStore').then(({ useProfileStore }) => {
        void useProfileStore.getState().loadFromDaemon();
      });
    }
    // Flow events
    if (e.type === 'flow_enabled' || e.type === 'flow_disabled') {
      void import('./flowStore').then(({ useFlowStore }) => {
        useFlowStore.setState({ enabled: e.type === 'flow_enabled' });
      });
    }
    if (e.type === 'flow_peer_discovered') {
      void import('./flowStore').then(({ useFlowStore }) => {
        const s = useFlowStore.getState();
        useFlowStore.setState({ peers: [...s.peers, e.peer] });
      });
    }
    if (e.type === 'flow_peer_lost') {
      void import('./flowStore').then(({ useFlowStore }) => {
        const s = useFlowStore.getState();
        useFlowStore.setState({ peers: s.peers.filter((p) => p.id !== e.peer_id) });
      });
    }
    if (e.type === 'flow_synced') {
      void import('./flowStore').then(({ useFlowStore }) => {
        void useFlowStore.getState().fetchHistory();
      });
    }
    // Kanata events
    if (e.type === 'kanata_started') {
      void import('./kanataStore').then(({ useKanataStore }) => {
        useKanataStore.setState({ processState: 'running', pid: e.pid });
      });
    }
    if (e.type === 'kanata_stopped') {
      void import('./kanataStore').then(({ useKanataStore }) => {
        useKanataStore.setState({ processState: 'stopped', pid: null });
      });
    }
    if (e.type === 'kanata_reloaded') {
      void import('./kanataStore').then(({ useKanataStore }) => {
        void useKanataStore.getState().fetchStatus();
      });
    }
    const next = [...get().events, e];
    if (next.length > MAX_EVENTS) next.splice(0, next.length - MAX_EVENTS);
    set({ events: next });
  },
}));
