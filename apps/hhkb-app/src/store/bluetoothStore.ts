/**
 * Bluetooth / BLE state store.
 *
 * Replaces the old 60-second polling loop with:
 *  - On-demand scan triggered by the user (startScan)
 *  - Device list updated from WS bluetooth_scan_complete events
 *  - Status polled once on daemon online (fetch)
 *  - Status refreshed again after each scan completes
 */

import { create } from 'zustand';
import { useDaemonStore } from './daemonStore';
import type { BleDevice } from '../hhkb/daemonClient';

interface BluetoothState {
  // Adapter availability (set by fetch)
  available: boolean;
  // Currently connected HHKB BLE device (if any)
  connected: boolean;
  battery: number | null;
  name: string | null;
  address: string | null;
  rssi: number | null;
  // Scan
  scanning: boolean;
  devices: BleDevice[];

  fetch: () => Promise<void>;
  startScan: () => Promise<void>;
  /** Silent scan — does not clear the device list, errors are swallowed */
  startScanSilent: () => Promise<void>;
  /** Called by daemonStore WebSocket event handler */
  onScanComplete: (devices: BleDevice[]) => void;
}

export const useBluetoothStore = create<BluetoothState>((set, get) => ({
  available: false,
  connected: false,
  battery: null,
  name: null,
  address: null,
  rssi: null,
  scanning: false,
  devices: [],

  async fetch() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      const info = await client.bluetooth();
      set({
        available: info.available,
        connected: info.connected,
        battery: info.battery ?? null,
        name: info.name ?? null,
        address: info.address ?? null,
        rssi: info.rssi ?? null,
      });
    } catch {
      // daemon offline — keep existing state
    }
  },

  async startScan() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    set({ scanning: true, devices: [] });
    try {
      await client.bluetoothScan();
      // scanning stays true until WS bluetooth_scan_complete arrives
    } catch {
      set({ scanning: false });
    }
  },

  onScanComplete(devices) {
    set({ scanning: false, devices });
    void get().fetch();
  },

  async startScanSilent() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      await client.bluetoothScan();
    } catch {
      // ignore — silent probe
    }
  },
}));
