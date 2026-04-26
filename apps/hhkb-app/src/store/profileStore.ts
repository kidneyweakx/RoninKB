/**
 * Profile store.
 *
 * Sources profiles from the daemon when it's reachable. Falls back to
 * `localStorage`-backed in-memory storage otherwise so that profiles survive
 * a page reload even in zero-install mode.
 *
 * Import / export helpers accept and emit VIA JSON (optionally with the
 * `_roninKB` extension). See `src/hhkb/via.ts` for the parser.
 */

import { create } from 'zustand';
import {
  ViaProfile,
  parseViaProfile,
  serializeViaProfile,
} from '../hhkb/via';
import { useDaemonStore } from './daemonStore';
import {
  FACTORY_PROFILES,
  FACTORY_PROFILE_IDS,
} from '../data/factoryDefault';

export interface Profile {
  id: string;
  name: string;
  icon?: string;
  tags?: string[];
  via: ViaProfile;
}

const LS_KEY = 'roninKB.profiles.v1';
const LS_ACTIVE_KEY = 'roninKB.activeProfileId.v1';

interface PersistedState {
  profiles: Profile[];
  activeProfileId: string | null;
}

function loadFromLocalStorage(): PersistedState | null {
  if (typeof localStorage === 'undefined') return null;
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedState;
    if (!Array.isArray(parsed.profiles)) return null;
    return parsed;
  } catch {
    return null;
  }
}

function saveToLocalStorage(state: PersistedState): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(state));
    if (state.activeProfileId) {
      localStorage.setItem(LS_ACTIVE_KEY, state.activeProfileId);
    } else {
      localStorage.removeItem(LS_ACTIVE_KEY);
    }
  } catch {
    // ignore quota or private-mode failures
  }
}

const defaultProfile: Profile = {
  id: 'default',
  name: 'Default',
  icon: 'keyboard',
  tags: [],
  via: {
    name: 'HHKB Professional Hybrid',
    vendorId: '0x04FE',
    productId: '0x0021',
    matrix: { rows: 8, cols: 8 },
    layers: [],
  },
};

interface ProfileState {
  profiles: Profile[];
  activeProfileId: string | null;
  source: 'daemon' | 'local';
  loading: boolean;
  error: string | null;

  addProfile: (p: Profile) => Promise<void>;
  /** Create a new blank or prefilled profile and return it. */
  createNew: (name: string, template?: ViaProfile) => Promise<Profile>;
  removeProfile: (id: string) => Promise<void>;
  setActiveProfile: (id: string) => Promise<void>;
  /**
   * Rename a profile's id locally — used when promoting a local-only
   * profile (e.g. factory default) to the daemon and the daemon assigns
   * a different canonical id.
   */
  rekeyProfile: (oldId: string, newId: string) => void;
  getActive: () => Profile | undefined;

  loadFromDaemon: () => Promise<void>;
  importProfile: (jsonString: string) => Promise<Profile>;
  exportProfile: (id: string) => string;
  exportAllProfiles: () => string;
}

function genId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  return `p-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
}

function profileFromVia(via: ViaProfile, fallbackId?: string): Profile {
  const ronin = via._roninKB?.profile;
  return {
    id: ronin?.id ?? fallbackId ?? genId(),
    name: ronin?.name ?? via.name,
    icon: ronin?.icon,
    tags: ronin?.tags ?? [],
    via,
  };
}

const initial = loadFromLocalStorage();

/** Merge factory profiles in — they are never stored (always live in code). */
function mergeFactoryProfiles(profiles: Profile[]): Profile[] {
  const ids = new Set(profiles.map((p) => p.id));
  const missing = FACTORY_PROFILES.filter((fp) => !ids.has(fp.id));
  return missing.length > 0 ? [...missing, ...profiles] : profiles;
}

export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: mergeFactoryProfiles(initial?.profiles ?? [defaultProfile]),
  activeProfileId: initial?.activeProfileId ?? defaultProfile.id,
  source: 'local',
  loading: false,
  error: null,

  async addProfile(p) {
    const daemon = useDaemonStore.getState().client;
    if (daemon) {
      try {
        const summary = await daemon.createProfile(p.via);
        const created: Profile = { ...p, id: summary.id };
        set((s) => ({
          profiles: [...s.profiles, created],
          source: 'daemon',
        }));
        return;
      } catch (e) {
        set({
          error:
            e instanceof Error
              ? `daemon createProfile failed: ${e.message}`
              : 'daemon createProfile failed',
        });
        // Fall through to local-only add.
      }
    }
    set((s) => {
      const next = { profiles: [...s.profiles, p] };
      saveToLocalStorage({
        profiles: next.profiles,
        activeProfileId: s.activeProfileId,
      });
      return next;
    });
  },

  async createNew(name, template) {
    const id = genId();
    const via: ViaProfile = template
      ? { ...template, name }
      : {
          name,
          vendorId: '0x04FE',
          productId: '0x0021',
          matrix: { rows: 8, cols: 8 },
          layers: [],
          _roninKB: {
            version: '1.0',
            profile: { id, name },
          },
        };
    // Stamp the generated UUID into _roninKB.profile.id so the daemon
    // always receives a valid UUID regardless of what the template carried.
    if (via._roninKB) {
      via._roninKB = { ...via._roninKB, profile: { ...via._roninKB.profile, id } };
    }
    const profile: Profile = { id, name, tags: [], via };
    await get().addProfile(profile);
    return profile;
  },

  async removeProfile(id) {
    // Factory profiles are immutable — silently ignore.
    if (FACTORY_PROFILE_IDS.has(id)) return;

    const daemon = useDaemonStore.getState().client;
    if (daemon) {
      try {
        await daemon.deleteProfile(id);
      } catch (e) {
        set({
          error:
            e instanceof Error
              ? `daemon deleteProfile failed: ${e.message}`
              : 'daemon deleteProfile failed',
        });
      }
    }
    set((s) => {
      const next = {
        profiles: s.profiles.filter((p) => p.id !== id),
        activeProfileId:
          s.activeProfileId === id ? null : s.activeProfileId,
      };
      saveToLocalStorage({
        profiles: next.profiles,
        activeProfileId: next.activeProfileId,
      });
      return next;
    });
  },

  rekeyProfile(oldId, newId) {
    if (oldId === newId) return;
    set((s) => {
      const next = {
        profiles: s.profiles.map((p) =>
          p.id === oldId
            ? {
                ...p,
                id: newId,
                via: {
                  ...p.via,
                  _roninKB: p.via._roninKB
                    ? {
                        ...p.via._roninKB,
                        profile: { ...p.via._roninKB.profile, id: newId },
                      }
                    : undefined,
                },
              }
            : p,
        ),
        activeProfileId:
          s.activeProfileId === oldId ? newId : s.activeProfileId,
      };
      saveToLocalStorage({
        profiles: next.profiles,
        activeProfileId: next.activeProfileId,
      });
      return next;
    });
  },

  async setActiveProfile(id) {
    const daemon = useDaemonStore.getState().client;
    if (daemon) {
      try {
        await daemon.setActiveProfile(id);
      } catch (e) {
        set({
          error:
            e instanceof Error
              ? `daemon setActiveProfile failed: ${e.message}`
              : 'daemon setActiveProfile failed',
        });
      }
    }
    set((s) => {
      saveToLocalStorage({
        profiles: s.profiles,
        activeProfileId: id,
      });
      return { activeProfileId: id };
    });
  },

  getActive() {
    const { profiles, activeProfileId } = get();
    return profiles.find((p) => p.id === activeProfileId);
  },

  async loadFromDaemon() {
    const daemon = useDaemonStore.getState().client;
    if (!daemon) return;
    set({ loading: true, error: null });
    try {
      const summaries = await daemon.listProfiles();
      const profiles: Profile[] = [];
      for (const s of summaries) {
        try {
          const rec = await daemon.getProfile(s.id);
          profiles.push({
            id: rec.id,
            name: rec.name,
            icon: rec.via._roninKB?.profile.icon,
            tags: rec.tags,
            via: rec.via,
          });
        } catch {
          // Skip profiles we can't fetch, but keep going.
        }
      }
      let activeId: string | null = null;
      try {
        const act = await daemon.getActiveProfile();
        activeId = act.id;
      } catch {
        // ignore
      }
      set({
        profiles: mergeFactoryProfiles(
          profiles.length > 0 ? profiles : get().profiles,
        ),
        activeProfileId: activeId ?? get().activeProfileId,
        source: 'daemon',
        loading: false,
      });
    } catch (e) {
      set({
        loading: false,
        error:
          e instanceof Error
            ? `loadFromDaemon failed: ${e.message}`
            : 'loadFromDaemon failed',
      });
    }
  },

  async importProfile(jsonString) {
    let via: ViaProfile;
    try {
      via = parseViaProfile(jsonString);
    } catch (e) {
      const msg =
        e instanceof Error ? e.message : String(e);
      throw new Error(`invalid VIA JSON: ${msg}`);
    }
    const profile = profileFromVia(via);
    await get().addProfile(profile);
    return profile;
  },

  exportProfile(id) {
    const profile = get().profiles.find((p) => p.id === id);
    if (!profile) throw new Error(`no profile with id ${id}`);
    return serializeViaProfile(profile.via);
  },

  exportAllProfiles() {
    const all = get().profiles.map((p) => JSON.parse(serializeViaProfile(p.via)));
    return JSON.stringify(all, null, 2);
  },
}));
