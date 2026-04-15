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
import type {
  BleDevice,
  DaemonClient,
  SystemBluetoothDevice,
} from '../hhkb/daemonClient';

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
  // Host OS connected list (macOS system_profiler)
  systemConnectedDevices: SystemBluetoothDevice[];
  systemSource: string | null;
  systemMessage: string | null;

  fetch: () => Promise<void>;
  startScan: () => Promise<void>;
  /** Silent scan — does not clear the device list, errors are swallowed */
  startScanSilent: () => Promise<void>;
  /** Called by daemonStore WebSocket event handler */
  onScanComplete: (devices: BleDevice[]) => void;
}

const MAC_PRIVACY_ADDR = '00:00:00:00:00:00';

function pickConnectedDevice(devices: BleDevice[]): BleDevice | undefined {
  return devices.find((d) => d.connected);
}

function inferConnectedNameAddress(
  status: { name: string | null; address: string | null },
  devices: BleDevice[],
): { name: string | null; address: string | null } {
  if (status.name && status.name.trim()) return status;
  const fromDevices = pickConnectedDevice(devices);
  if (!fromDevices) return status;
  return {
    name: fromDevices.name ?? status.name,
    address:
      status.address && status.address !== MAC_PRIVACY_ADDR
        ? status.address
        : fromDevices.address ?? status.address,
  };
}

async function pollScanSnapshot(client: DaemonClient): Promise<{
  scanning: boolean;
  devices: BleDevice[];
}> {
  let latestDevices: BleDevice[] = [];
  for (let i = 0; i < 8; i += 1) {
    const snap = await client.bluetoothDevices();
    latestDevices = snap.devices ?? [];
    if (!snap.scanning) {
      return { scanning: false, devices: latestDevices };
    }
    await new Promise((resolve) => setTimeout(resolve, 700));
  }
  return { scanning: false, devices: latestDevices };
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
  systemConnectedDevices: [],
  systemSource: null,
  systemMessage: null,

  async fetch() {
    const client = useDaemonStore.getState().client;
    if (!client) return;
    try {
      const [info, deviceSnap, systemSnap] = await Promise.all([
        client.bluetooth(),
        client.bluetoothDevices().catch(() => ({
          available: false,
          scanning: false,
          devices: [] as BleDevice[],
        })),
        client.bluetoothSystemDevices().catch(() => ({
          available: false,
          source: 'error',
          devices: [] as SystemBluetoothDevice[],
          message: 'system devices unavailable',
        })),
      ]);
      const sysDev = systemSnap.devices ?? [];
      // If btleplug can't see an OS-managed connection (macOS Core Bluetooth
      // limitation), fall back to system_profiler data for HHKB.
      const hhkbSys = sysDev.find((d) => d.name?.startsWith('HHKB'));
      const derivedConnected = info.connected || !!hhkbSys;
      const derivedName = info.name ?? hhkbSys?.name ?? null;
      const derivedBattery = info.battery ?? hhkbSys?.battery ?? null;
      const derivedAddress = info.address ?? hhkbSys?.address ?? null;

      const inferred = inferConnectedNameAddress(
        { name: derivedName, address: derivedAddress },
        deviceSnap.devices ?? [],
      );
      set({
        available: info.available,
        connected: derivedConnected,
        battery: derivedBattery,
        name: inferred.name,
        address: inferred.address,
        rssi: info.rssi ?? null,
        scanning: Boolean(deviceSnap.scanning),
        devices: deviceSnap.devices ?? [],
        systemConnectedDevices: sysDev,
        systemSource: systemSnap.source ?? null,
        systemMessage: systemSnap.message ?? null,
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
      // WS event is preferred, but poll as a fallback so UI still updates
      // when websocket delivery is delayed or dropped.
      const snap = await pollScanSnapshot(client);
      set({ scanning: snap.scanning, devices: snap.devices });
      void get().fetch();
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
      const snap = await pollScanSnapshot(client);
      set({ scanning: snap.scanning, devices: snap.devices });
      void get().fetch();
    } catch {
      // ignore — silent probe
    }
  },
}));
