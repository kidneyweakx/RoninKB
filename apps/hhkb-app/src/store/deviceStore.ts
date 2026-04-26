/**
 * Device connection state.
 *
 * Holds the live `HhkbDevice`, the last-known keyboard info, the current
 * keyboard mode, and the currently-loaded base-layer Keymap.
 */

import { create } from 'zustand';
import { HhkbDevice } from '../hhkb/device';
import { Keymap } from '../hhkb/keymap';
import { KeyboardInfo, KeyboardMode, keyboardModeLabel } from '../hhkb/types';
import { useDaemonStore } from './daemonStore';

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'error';

/**
 * Indicates how the UI is reading the physical keyboard:
 *
 * - `webhid`: direct Chrome WebHID access (lowest latency, preferred)
 * - `daemon`: proxied through the locally-running daemon
 * - `none`:   neither — keymap editing is limited to profile mutation
 */
export type TransportMode = 'webhid' | 'daemon' | 'none';

interface DeviceState {
  device: HhkbDevice | null;
  status: ConnectionStatus;
  error: string | null;

  info: KeyboardInfo | null;
  mode: KeyboardMode;
  baseKeymap: Keymap | null;
  fnKeymap: Keymap | null;

  // Actions
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  loadKeymaps: () => Promise<void>;
  loadKeymapsFromDaemon: () => Promise<void>;
  /**
   * Hydrate base/fn keymaps from a profile's stored
   * `_roninKB.hardware.raw_layers`. Used in BT-only mode where the EEPROM
   * is not reachable but the user still needs to see the layout.
   * Returns true when keymap was hydrated.
   */
  loadKeymapsFromProfile: (
    rawLayers: { base: number[]; fn: number[] } | undefined,
  ) => boolean;
  setBaseKeymap: (km: Keymap) => void;
  setFnKeymap: (km: Keymap) => void;
  setKeyOverride: (index: number, value: number, layer: 'base' | 'fn') => void;
  /** Derived: which transport is currently serving the keyboard. */
  transportMode: () => TransportMode;
  /** Write the currently-held base+fn keymaps through the daemon proxy. */
  writeKeymaps: () => Promise<void>;
  /**
   * Snapshot the current hardware keymaps into a new named Profile and
   * persist it via the profile store. Useful for saving the current
   * (potentially modified) state before reverting to factory default.
   *
   * Returns the newly-created Profile.
   */
  captureHardwareProfile: (name: string) => Promise<import('../store/profileStore').Profile>;
}

export const useDeviceStore = create<DeviceState>((set, get) => ({
  device: null,
  status: 'disconnected',
  error: null,
  info: null,
  mode: KeyboardMode.Mac,
  baseKeymap: null,
  fnKeymap: null,

  async connect() {
    set({ status: 'connecting', error: null });

    // Prefer the daemon transport when it's reachable and already holds the
    // device. The macOS kernel gives exclusive access to an HID usage page
    // to the first process that opens it, so if the daemon has the keyboard
    // open via hidapi, a parallel WebHID `device.open()` will fail with
    // "Failed to open the device". Routing through the daemon avoids the
    // contention entirely.
    const daemon = useDaemonStore.getState();
    if (daemon.status === 'online' && daemon.client && daemon.deviceConnected) {
      try {
        set({ device: null, info: null, status: 'connected' });
        await get().loadKeymapsFromDaemon();
        return;
      } catch (e) {
        set({
          status: 'error',
          error: e instanceof Error ? e.message : String(e),
          device: null,
        });
        return;
      }
    }

    try {
      const device = await HhkbDevice.request();
      await device.notifyAppOpen();
      const info = await device.getKeyboardInfo();
      const mode = await device.getKeyboardMode();
      set({ device, info, mode, status: 'connected' });
      await get().loadKeymaps();
    } catch (e) {
      set({
        status: 'error',
        error: e instanceof Error ? e.message : String(e),
        device: null,
      });
    }
  },

  async disconnect() {
    const { device } = get();
    if (device) {
      try {
        await device.notifyAppClose();
      } catch {
        // best effort
      }
      try {
        await device.close();
      } catch {
        // ignore
      }
    }
    set({
      device: null,
      status: 'disconnected',
      info: null,
      baseKeymap: null,
      fnKeymap: null,
    });
  },

  async loadKeymaps() {
    const { device, mode } = get();
    if (!device) {
      // Fall back to daemon proxy if WebHID isn't connected but the
      // daemon is available.
      if (useDaemonStore.getState().client) {
        await get().loadKeymapsFromDaemon();
      }
      return;
    }
    const base = await device.getKeymap(mode, false);
    const fn = await device.getKeymap(mode, true);
    set({ baseKeymap: base, fnKeymap: fn });
  },

  async loadKeymapsFromDaemon() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    const { mode } = get();
    const label = keyboardModeLabel(mode).toLowerCase();
    try {
      const base = await client.readKeymap(label, false);
      const fn = await client.readKeymap(label, true);
      if (base.data.length === 128 && fn.data.length === 128) {
        const baseBytes = new Uint8Array(base.data);
        const fnBytes = new Uint8Array(fn.data);
        set({
          baseKeymap: new Keymap(baseBytes),
          fnKeymap: new Keymap(fnBytes),
        });
      }
    } catch (e) {
      set({
        error:
          e instanceof Error
            ? `daemon keymap read failed: ${e.message}`
            : 'daemon keymap read failed',
      });
    }
  },

  loadKeymapsFromProfile(raw) {
    if (!raw || raw.base.length !== 128 || raw.fn.length !== 128) return false;
    set({
      baseKeymap: new Keymap(new Uint8Array(raw.base)),
      fnKeymap: new Keymap(new Uint8Array(raw.fn)),
    });
    return true;
  },

  transportMode() {
    if (get().device) return 'webhid';
    if (useDaemonStore.getState().client) return 'daemon';
    return 'none';
  },

  setBaseKeymap(km) {
    set({ baseKeymap: km });
  },

  setFnKeymap(km) {
    set({ fnKeymap: km });
  },

  setKeyOverride(index, value, layer) {
    const state = get();
    const source = layer === 'base' ? state.baseKeymap : state.fnKeymap;
    if (!source) return;
    const next = source.clone();
    next.set(index, value);
    if (layer === 'base') set({ baseKeymap: next });
    else set({ fnKeymap: next });
  },

  async captureHardwareProfile(name: string) {
    const { baseKeymap, fnKeymap, mode } = get();
    const baseArr = baseKeymap ? Array.from(baseKeymap.asBytes()) : new Array<number>(128).fill(0);
    const fnArr = fnKeymap ? Array.from(fnKeymap.asBytes()) : new Array<number>(128).fill(0);
    const via = {
      name,
      vendorId: '0x04FE',
      productId: '0x0021',
      matrix: { rows: 8, cols: 8 },
      layers: [],
      _roninKB: {
        version: '1.0',
        profile: { id: crypto.randomUUID(), name },
        hardware: {
          keyboard_mode: mode as number,
          raw_layers: { base: baseArr, fn: fnArr },
        },
      },
    };
    const { useProfileStore } = await import('./profileStore');
    const profile = await useProfileStore.getState().createNew(name, via);
    return profile;
  },

  async writeKeymaps() {
    const { baseKeymap, fnKeymap, mode } = get();
    const client = useDaemonStore.getState().client;
    if (!client) {
      throw new Error('Daemon offline — cannot flash keymaps');
    }
    const label = keyboardModeLabel(mode).toLowerCase();
    if (baseKeymap) {
      await client.writeKeymap(label, false, Array.from(baseKeymap.asBytes()));
    }
    if (fnKeymap) {
      await client.writeKeymap(label, true, Array.from(fnKeymap.asBytes()));
    }
  },
}));
