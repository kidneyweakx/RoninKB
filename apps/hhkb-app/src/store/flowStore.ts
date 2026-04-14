/**
 * Flow clipboard-sync store.
 *
 * Wraps the daemon's `/flow/*` endpoints. Silent-fails when the daemon is
 * offline — the store simply retains its last-known state rather than
 * surfacing an error to the user.
 */

import { create } from 'zustand';
import { type FlowEntry, type FlowPeer } from '../hhkb/daemonClient';
import { useDaemonStore } from './daemonStore';

interface FlowState {
  enabled: boolean;
  peers: FlowPeer[];
  history: FlowEntry[];
  loading: boolean;

  fetchConfig: () => Promise<void>;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
  addPeer: (hostname: string, addr: string) => Promise<void>;
  removePeer: (id: string) => Promise<void>;
  fetchHistory: () => Promise<void>;
  syncClipboard: (content: string) => Promise<void>;
  clearHistory: () => Promise<void>;
}

export const useFlowStore = create<FlowState>((set, _get) => ({
  enabled: false,
  peers: [],
  history: [],
  loading: false,

  async fetchConfig() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    set({ loading: true });
    try {
      const config = await client.flowConfig();
      const peers = await client.flowPeers();
      set({ enabled: config.enabled, peers });
    } catch {
      // daemon offline — keep existing state
    } finally {
      set({ loading: false });
    }
  },

  async enable() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      await client.flowEnable();
      set({ enabled: true });
    } catch {
      // silent fail
    }
  },

  async disable() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      await client.flowDisable();
      set({ enabled: false });
    } catch {
      // silent fail
    }
  },

  async addPeer(hostname, addr) {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      const peer = await client.flowAddPeer(hostname, addr);
      set((s) => ({ peers: [...s.peers, peer] }));
    } catch {
      // silent fail
    }
  },

  async removePeer(id) {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      await client.flowRemovePeer(id);
      set((s) => ({ peers: s.peers.filter((p) => p.id !== id) }));
    } catch {
      // silent fail
    }
  },

  async fetchHistory() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      const history = await client.flowHistory();
      set({ history });
    } catch {
      // silent fail
    }
  },

  async syncClipboard(content) {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      const entry = await client.flowSync(content);
      set((s) => ({ history: [entry, ...s.history] }));
    } catch {
      // silent fail
    }
  },

  async clearHistory() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      await client.flowClearHistory();
      set({ history: [] });
    } catch {
      // silent fail
    }
  },
}));
