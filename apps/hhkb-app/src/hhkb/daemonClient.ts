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

  async deviceDipsw(): Promise<unknown> {
    return requestJson<unknown>(`${this.baseUrl}/device/dipsw`);
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
  // Kanata supervisor
  // -------------------------------------------------------------------------

  async kanataStatus(): Promise<KanataStatus> {
    const raw = await requestJson<{
      installed: boolean;
      config_path?: string;
      state?: string;
      pid?: number;
    }>(`${this.baseUrl}/kanata/status`);
    return {
      installed: raw.installed,
      path: raw.config_path,
      state: raw.state,
      pid: raw.pid,
    };
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
}

export interface KanataStatus {
  installed: boolean;
  path?: string;
  version?: string;
  state?: string;
  pid?: number;
}

// ---------------------------------------------------------------------------
// WebSocket helper
// ---------------------------------------------------------------------------

export type DaemonEvent =
  | { type: 'device_connected' }
  | { type: 'device_disconnected' }
  | { type: 'profile_changed'; id: string };

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
