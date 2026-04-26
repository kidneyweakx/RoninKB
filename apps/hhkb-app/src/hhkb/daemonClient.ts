/**
 * Daemon REST + WebSocket client.
 *
 * Talks to the optional `hhkb-daemon` over `http://localhost:7331`. The
 * daemon exposes a small REST surface for health, profile CRUD, and device
 * proxy operations, plus a WebSocket that pushes events (device and profile
 * changes) back to subscribed clients.
 *
 * All methods throw descriptive errors on non-2xx responses or network
 * failures. Callers should catch and downgrade the UI when the daemon is
 * unavailable — the app must remain fully functional in its "zero install"
 * mode even without this client.
 */

import { ViaProfile, viaProfileFromObject } from './via';

export const DAEMON_URL = 'http://localhost:7331';
export const DAEMON_WS_URL = 'ws://localhost:7331/ws';

export interface HealthResponse {
  status: string;
  version: string;
  device_connected: boolean;
}

export interface ProfileSummary {
  id: string;
  name: string;
  tags: string[];
  created_at: number;
  updated_at: number;
}

export interface ProfileRecord extends ProfileSummary {
  via: ViaProfile;
}

interface ProfileListResponse {
  profiles: Array<ProfileSummary & { via: unknown }>;
}

interface ActiveResponse {
  id: string | null;
}

interface KeymapDto {
  mode: string;
  fn_layer: boolean;
  data: number[];
}

export class DaemonError extends Error {
  constructor(
    message: string,
    readonly status?: number,
  ) {
    super(message);
    this.name = 'DaemonError';
  }
}

async function requestJson<T>(
  url: string,
  init: RequestInit = {},
  timeoutMs = 5000,
): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  let res: Response;
  try {
    res = await fetch(url, {
      ...init,
      signal: controller.signal,
      headers: {
        'Content-Type': 'application/json',
        ...(init.headers ?? {}),
      },
    });
  } catch (e) {
    clearTimeout(timer);
    const msg = e instanceof Error ? e.message : String(e);
    throw new DaemonError(`network error: ${msg}`);
  }
  clearTimeout(timer);

  if (!res.ok) {
    let body = '';
    try {
      body = await res.text();
    } catch {
      // ignore
    }
    throw new DaemonError(
      `${res.status} ${res.statusText}${body ? `: ${body}` : ''}`,
      res.status,
    );
  }

  // Caller expects JSON; if body is empty, return an empty object cast.
  const text = await res.text();
  if (!text) return {} as T;
  try {
    return JSON.parse(text) as T;
  } catch (e) {
    throw new DaemonError(
      `invalid JSON response: ${e instanceof Error ? e.message : String(e)}`,
    );
  }
}

export class DaemonClient {
  constructor(public readonly baseUrl: string = DAEMON_URL) {}

  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  async health(): Promise<HealthResponse> {
    return requestJson<HealthResponse>(`${this.baseUrl}/health`, {}, 1500);
  }

  // -------------------------------------------------------------------------
  // Profiles
  // -------------------------------------------------------------------------

  async listProfiles(): Promise<ProfileSummary[]> {
    const resp = await requestJson<ProfileListResponse>(
      `${this.baseUrl}/profiles`,
    );
    return resp.profiles.map((p) => ({
      id: p.id,
      name: p.name,
      tags: p.tags ?? [],
      created_at: p.created_at,
      updated_at: p.updated_at,
    }));
  }

  async getProfile(id: string): Promise<ProfileRecord> {
    const raw = await requestJson<{
      id: string;
      name: string;
      tags: string[];
      created_at: number;
      updated_at: number;
      via: unknown;
    }>(`${this.baseUrl}/profiles/${encodeURIComponent(id)}`);
    return {
      id: raw.id,
      name: raw.name,
      tags: raw.tags ?? [],
      created_at: raw.created_at,
      updated_at: raw.updated_at,
      via: viaProfileFromObject(raw.via as Record<string, unknown>),
    };
  }

  async createProfile(profile: ViaProfile): Promise<ProfileSummary> {
    const raw = await requestJson<ProfileSummary>(`${this.baseUrl}/profiles`, {
      method: 'POST',
      body: JSON.stringify(profile),
    });
    return {
      id: raw.id,
      name: raw.name,
      tags: raw.tags ?? [],
      created_at: raw.created_at,
      updated_at: raw.updated_at,
    };
  }

  async updateProfile(
    id: string,
    profile: ViaProfile,
  ): Promise<ProfileSummary> {
    const raw = await requestJson<ProfileSummary>(
      `${this.baseUrl}/profiles/${encodeURIComponent(id)}`,
      {
        method: 'PUT',
        body: JSON.stringify(profile),
      },
    );
    return {
      id: raw.id,
      name: raw.name,
      tags: raw.tags ?? [],
      created_at: raw.created_at,
      updated_at: raw.updated_at,
    };
  }

  async deleteProfile(id: string): Promise<void> {
    await requestJson<unknown>(
      `${this.baseUrl}/profiles/${encodeURIComponent(id)}`,
      { method: 'DELETE' },
    );
  }

  async getActiveProfile(): Promise<{ id: string | null }> {
    const r = await requestJson<ActiveResponse>(
      `${this.baseUrl}/profiles/active`,
    );
    return { id: r.id };
  }

  async setActiveProfile(id: string): Promise<void> {
    await requestJson<ActiveResponse>(`${this.baseUrl}/profiles/active`, {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
  }

  // -------------------------------------------------------------------------
  // Device (proxied via the daemon when WebHID is unavailable)
  // -------------------------------------------------------------------------

  async deviceInfo(): Promise<unknown> {
    return requestJson<unknown>(`${this.baseUrl}/device/info`);
  }

  async deviceMode(): Promise<unknown> {
    return requestJson<unknown>(`${this.baseUrl}/device/mode`);
  }

  async bluetooth(): Promise<BluetoothInfo> {
    return requestJson<BluetoothInfo>(`${this.baseUrl}/device/bluetooth`, {}, 3000);
  }

  async deviceDipsw(): Promise<DipSwitchState> {
    return requestJson<DipSwitchState>(`${this.baseUrl}/device/dipsw`);
  }

  async readKeymap(
    mode: string,
    fnLayer: boolean,
  ): Promise<{ data: number[] }> {
    const q = new URLSearchParams({
      mode,
      fn_layer: String(fnLayer),
    });
    const resp = await requestJson<KeymapDto>(
      `${this.baseUrl}/device/keymap?${q.toString()}`,
    );
    return { data: resp.data };
  }

  async writeKeymap(
    mode: string,
    fnLayer: boolean,
    data: number[],
  ): Promise<void> {
    const q = new URLSearchParams({
      mode,
      fn_layer: String(fnLayer),
    });
    await requestJson<unknown>(
      `${this.baseUrl}/device/keymap?${q.toString()}`,
      {
        method: 'PUT',
        body: JSON.stringify({ data }),
      },
    );
  }

  /**
   * HHKB keyboard mode is controlled by the physical DIP switches (SW1/SW2);
   * there is no firmware command to set it from software. This stub exists
   * so the UI can optimistically update its local mode preference without a
   * daemon round-trip. Callers must also update `useDeviceStore.mode`
   * directly for the keymap view to switch.
   */
  async setMode(_mode: string): Promise<void> {
    return;
  }

  // -------------------------------------------------------------------------
  // Flow — clipboard sync
  // -------------------------------------------------------------------------

  async flowConfig(): Promise<FlowConfig> {
    return requestJson<FlowConfig>(`${this.baseUrl}/flow/config`);
  }

  async flowUpdateConfig(config: FlowConfig): Promise<FlowConfig> {
    return requestJson<FlowConfig>(`${this.baseUrl}/flow/config`, {
      method: 'PUT',
      body: JSON.stringify(config),
    });
  }

  async flowEnable(): Promise<void> {
    await requestJson<unknown>(`${this.baseUrl}/flow/enable`, { method: 'POST' });
  }

  async flowDisable(): Promise<void> {
    await requestJson<unknown>(`${this.baseUrl}/flow/disable`, { method: 'POST' });
  }

  async flowPeers(): Promise<FlowPeer[]> {
    const resp = await requestJson<{ peers: FlowPeer[] }>(`${this.baseUrl}/flow/peers`);
    return resp.peers;
  }

  async flowAddPeer(hostname: string, addr: string): Promise<FlowPeer> {
    return requestJson<FlowPeer>(`${this.baseUrl}/flow/peers`, {
      method: 'POST',
      body: JSON.stringify({ hostname, addr }),
    });
  }

  async flowRemovePeer(id: string): Promise<void> {
    await requestJson<unknown>(
      `${this.baseUrl}/flow/peers/${encodeURIComponent(id)}`,
      { method: 'DELETE' },
    );
  }

  async flowHistory(): Promise<FlowEntry[]> {
    const resp = await requestJson<{ entries: FlowEntry[] }>(`${this.baseUrl}/flow/history`);
    return resp.entries;
  }

  async flowSync(content: string): Promise<FlowEntry> {
    const resp = await requestJson<{ entry: FlowEntry }>(`${this.baseUrl}/flow/sync`, {
      method: 'POST',
      body: JSON.stringify({ content }),
    });
    return resp.entry;
  }

  async flowClearHistory(): Promise<void> {
    await requestJson<unknown>(`${this.baseUrl}/flow/history`, { method: 'DELETE' });
  }

  // -------------------------------------------------------------------------
  // BT control
  // -------------------------------------------------------------------------

  async bluetoothScan(): Promise<BleScanStartedResponse> {
    return requestJson<BleScanStartedResponse>(
      `${this.baseUrl}/device/bluetooth/scan`,
      { method: 'POST' },
    );
  }

  async bluetoothDevices(): Promise<BleDevicesResponse> {
    return requestJson<BleDevicesResponse>(
      `${this.baseUrl}/device/bluetooth/devices`,
    );
  }

  async bluetoothSystemDevices(): Promise<SystemBluetoothDevicesResponse> {
    return requestJson<SystemBluetoothDevicesResponse>(
      `${this.baseUrl}/device/bluetooth/system`,
    );
  }

  // -------------------------------------------------------------------------
  // Kanata supervisor
  // -------------------------------------------------------------------------

  async kanataStatus(): Promise<KanataStatus> {
    const raw = await requestJson<{
      installed: boolean;
      binary_path?: string;
      config_path?: string;
      state?: string;
      pid?: number;
      input_monitoring_granted?: boolean | null;
      last_error?: string | null;
      stderr_tail?: string[];
      device_path?: string | null;
    }>(`${this.baseUrl}/kanata/status`);
    return {
      installed: raw.installed,
      binaryPath: raw.binary_path,
      path: raw.config_path,
      state: raw.state,
      pid: raw.pid,
      inputMonitoringGranted: raw.input_monitoring_granted ?? null,
      lastError: raw.last_error ?? null,
      stderrTail: raw.stderr_tail ?? [],
      devicePath: raw.device_path ?? null,
    };
  }

  /** Spawn a new kanata child. Fails if already running or no binary. */
  async kanataStart(): Promise<number> {
    const res = await requestJson<{ pid: number }>(
      `${this.baseUrl}/kanata/start`,
      { method: 'POST' },
    );
    return res.pid;
  }

  /** Terminate the running kanata child. Fails if nothing is running. */
  async kanataStop(): Promise<void> {
    await requestJson<unknown>(`${this.baseUrl}/kanata/stop`, {
      method: 'POST',
    });
  }

  /** Read the on-disk .kbd config. Returns an empty string when absent. */
  async kanataGetConfig(): Promise<{ config: string; path: string }> {
    return requestJson<{ config: string; path: string }>(
      `${this.baseUrl}/kanata/config`,
    );
  }

  /**
   * Hot-reload kanata with a new config. The daemon writes `config` to its
   * managed `.kbd` file and signals the running child (SIGUSR1 on unix,
   * restart on windows). Safe to call when kanata is stopped — the config
   * is just written for next start.
   */
  async kanataReload(config: string): Promise<void> {
    await requestJson<unknown>(`${this.baseUrl}/kanata/reload`, {
      method: 'POST',
      body: JSON.stringify({ config }),
    });
  }

  /**
   * Open Finder/Explorer with the kanata binary (or the wrapping `.app`
   * bundle on macOS) selected, so the user can drag it into the Input
   * Monitoring picker without retyping a path.
   */
  async kanataReveal(): Promise<{ path: string; bundle: string | null }> {
    return requestJson<{ path: string; bundle: string | null }>(
      `${this.baseUrl}/kanata/reveal`,
      { method: 'POST' },
    );
  }
}

// ---------------------------------------------------------------------------
// Flow types
// ---------------------------------------------------------------------------

export interface FlowConfig {
  enabled: boolean;
  auto_sync: boolean;
  instance_id: string;
  instance_name: string;
  history_limit: number;
}

export interface FlowPeer {
  id: string;
  hostname: string;
  addr: string;
  last_seen: number;
  online: boolean;
}

export interface FlowEntry {
  id: string;
  content: string;
  source: { type: 'local' } | { type: 'peer'; peer_id: string; hostname: string };
  timestamp: number;
  mime: string;
}

export interface BluetoothInfo {
  available: boolean;
  connected: boolean;
  name?: string;
  address?: string;
  battery?: number;   // 0–100
  rssi?: number;      // dBm, negative
}

export interface BleDevice {
  id: string;
  name?: string;
  address: string;
  battery?: number;
  connected: boolean;
  rssi?: number;
}

export interface BleDevicesResponse {
  available: boolean;
  scanning: boolean;
  devices: BleDevice[];
}

export interface SystemBluetoothDevice {
  name: string;
  address?: string | null;
  kind?: string | null;
  battery?: number | null;
  services?: string | null;
}

export interface SystemBluetoothDevicesResponse {
  available: boolean;
  source: string;
  devices: SystemBluetoothDevice[];
  message?: string | null;
}

export interface BleScanStartedResponse {
  scanning: boolean;
  message: string;
}

export interface KanataStatus {
  installed: boolean;
  binaryPath?: string;
  path?: string;
  version?: string;
  state?: string;
  pid?: number;
  inputMonitoringGranted?: boolean | null;
  lastError?: string | null;
  stderrTail?: string[];
  devicePath?: string | null;
}

/**
 * HHKB Professional Hybrid has 6 physical DIP switches on the bottom.
 * `switches[i]` is `true` when switch i+1 is in the ON position.
 *
 * Per the PFU HHKB Professional HYBRID user manual, the switches map to:
 *   SW1, SW2 — keyboard mode combination (HHK / Lite Ext / Mac / special)
 *   SW3       — Delete / BS swap
 *   SW4       — Left ⌘ / ⌥ swap
 *   SW5       — Power saver (wireless mode)
 *   SW6       — Reserved
 */
export interface DipSwitchState {
  switches: [boolean, boolean, boolean, boolean, boolean, boolean];
}

// ---------------------------------------------------------------------------
// WebSocket helper
// ---------------------------------------------------------------------------

export type DaemonEvent =
  | { type: 'device_connected' }
  | { type: 'device_disconnected' }
  | { type: 'profile_changed'; id: string }
  | { type: 'kanata_started'; pid: number }
  | { type: 'kanata_stopped' }
  | { type: 'kanata_reloaded'; profile_id: string }
  | { type: 'flow_enabled' }
  | { type: 'flow_disabled' }
  | { type: 'flow_peer_discovered'; peer: FlowPeer }
  | { type: 'flow_peer_lost'; peer_id: string }
  | { type: 'flow_synced'; entry_id: string; source: FlowEntry['source'] }
  | { type: 'bluetooth_scan_complete'; devices: BleDevice[] };

export class DaemonWebSocket {
  private ws: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private shouldReconnect = false;

  constructor(
    private onEvent: (e: DaemonEvent) => void,
    private url: string = DAEMON_WS_URL,
  ) {}

  connect(): void {
    this.shouldReconnect = true;
    this.openSocket();
  }

  disconnect(): void {
    this.shouldReconnect = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      try {
        this.ws.close();
      } catch {
        // ignore
      }
      this.ws = null;
    }
  }

  private openSocket(): void {
    try {
      this.ws = new WebSocket(this.url);
    } catch {
      this.scheduleReconnect();
      return;
    }

    this.ws.onmessage = (msg) => {
      try {
        const data = JSON.parse(String(msg.data)) as DaemonEvent;
        if (data && typeof data === 'object' && 'type' in data) {
          this.onEvent(data);
        }
      } catch {
        // ignore malformed frames
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
      if (this.shouldReconnect) this.scheduleReconnect();
    };

    this.ws.onerror = () => {
      // Let onclose drive the reconnect loop.
    };
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      if (this.shouldReconnect) this.openSocket();
    }, 5000);
  }
}
