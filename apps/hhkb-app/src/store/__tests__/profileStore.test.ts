import { beforeEach, describe, expect, it } from 'vitest';
import { useProfileStore } from '../profileStore';
import { useDaemonStore } from '../daemonStore';

const VALID_VIA = JSON.stringify({
  name: 'HHKB Professional Hybrid',
  vendorId: '0x04FE',
  productId: '0x0021',
  layers: [['KC_ESC', 'KC_1']],
  _roninKB: {
    version: '0.1.0',
    profile: {
      id: '550e8400-e29b-41d4-a716-446655440000',
      name: 'Imported',
      icon: 'keyboard',
      tags: ['test'],
    },
  },
});

const INVALID_VIA = '{"not": "a via profile"}';

function resetStores(): void {
  localStorage.clear();
  useDaemonStore.setState({ status: 'offline', client: null });
  useProfileStore.setState({
    profiles: [
      {
        id: 'default',
        name: 'Default',
        icon: 'keyboard',
        tags: [],
        via: {
          name: 'HHKB Professional Hybrid',
          vendorId: '0x04FE',
          productId: '0x0021',
          layers: [],
        },
      },
    ],
    activeProfileId: 'default',
    source: 'local',
    error: null,
  });
}

describe('profileStore.importProfile', () => {
  beforeEach(resetStores);

  it('imports a valid VIA profile', async () => {
    const profile = await useProfileStore.getState().importProfile(VALID_VIA);
    expect(profile.name).toBe('Imported');
    expect(profile.tags).toEqual(['test']);
    expect(useProfileStore.getState().profiles).toHaveLength(2);
  });

  it('rejects invalid VIA JSON', async () => {
    await expect(
      useProfileStore.getState().importProfile(INVALID_VIA),
    ).rejects.toThrow(/invalid VIA JSON/);
    expect(useProfileStore.getState().profiles).toHaveLength(1);
  });

  it('rejects malformed JSON', async () => {
    await expect(
      useProfileStore.getState().importProfile('not json {'),
    ).rejects.toThrow();
  });
});

describe('profileStore.exportProfile', () => {
  beforeEach(resetStores);

  it('exports the active profile as parseable JSON', () => {
    const json = useProfileStore.getState().exportProfile('default');
    const parsed = JSON.parse(json);
    expect(parsed.name).toBe('HHKB Professional Hybrid');
    expect(parsed.vendorId).toBe('0x04FE');
  });

  it('throws for unknown id', () => {
    expect(() => useProfileStore.getState().exportProfile('nope')).toThrow();
  });

  it('exportAllProfiles returns a JSON array', async () => {
    await useProfileStore.getState().importProfile(VALID_VIA);
    const json = useProfileStore.getState().exportAllProfiles();
    const arr = JSON.parse(json);
    expect(Array.isArray(arr)).toBe(true);
    expect(arr).toHaveLength(2);
    expect(arr[0].name).toBeDefined();
  });
});

describe('profileStore localStorage fallback', () => {
  beforeEach(resetStores);

  it('persists profiles to localStorage when daemon is unavailable', async () => {
    await useProfileStore.getState().importProfile(VALID_VIA);
    const raw = localStorage.getItem('roninKB.profiles.v1');
    expect(raw).toBeTruthy();
    const parsed = JSON.parse(raw!);
    expect(parsed.profiles.length).toBeGreaterThanOrEqual(2);
  });

  it('persists active profile id on setActiveProfile', async () => {
    await useProfileStore.getState().importProfile(VALID_VIA);
    const [, imported] = useProfileStore.getState().profiles;
    await useProfileStore.getState().setActiveProfile(imported.id);
    expect(useProfileStore.getState().activeProfileId).toBe(imported.id);
    const raw = localStorage.getItem('roninKB.profiles.v1');
    expect(JSON.parse(raw!).activeProfileId).toBe(imported.id);
  });

  it('removeProfile clears active pointer and persists', async () => {
    await useProfileStore.getState().importProfile(VALID_VIA);
    const [, imported] = useProfileStore.getState().profiles;
    await useProfileStore.getState().setActiveProfile(imported.id);
    await useProfileStore.getState().removeProfile(imported.id);
    expect(useProfileStore.getState().activeProfileId).toBeNull();
    expect(useProfileStore.getState().profiles).toHaveLength(1);
  });
});
