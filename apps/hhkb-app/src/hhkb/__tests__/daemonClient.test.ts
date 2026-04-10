import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DaemonClient, DaemonError } from '../daemonClient';

type FetchMock = ReturnType<typeof vi.fn>;

function jsonResponse(body: unknown, init: ResponseInit = {}): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
}

describe('DaemonClient', () => {
  let fetchMock: FetchMock;
  let client: DaemonClient;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    client = new DaemonClient('http://localhost:7331');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('health() parses the response', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        status: 'ok',
        version: '0.2.0',
        device_connected: true,
      }),
    );
    const h = await client.health();
    expect(h.status).toBe('ok');
    expect(h.version).toBe('0.2.0');
    expect(h.device_connected).toBe(true);
    expect(fetchMock).toHaveBeenCalledWith(
      'http://localhost:7331/health',
      expect.objectContaining({ headers: expect.any(Object) }),
    );
  });

  it('listProfiles() normalizes the daemon response', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        profiles: [
          {
            id: 'abc',
            name: 'Daily',
            tags: ['work'],
            created_at: 1,
            updated_at: 2,
            via: {},
          },
          {
            id: 'def',
            name: 'Gaming',
            tags: [],
            created_at: 3,
            updated_at: 4,
            via: {},
          },
        ],
      }),
    );
    const out = await client.listProfiles();
    expect(out).toHaveLength(2);
    expect(out[0].id).toBe('abc');
    expect(out[0].tags).toEqual(['work']);
    expect(out[1].name).toBe('Gaming');
  });

  it('createProfile() POSTs and returns the summary', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        id: 'new-id',
        name: 'Created',
        tags: [],
        created_at: 100,
        updated_at: 100,
        via: {},
      }),
    );
    const summary = await client.createProfile({
      name: 'Created',
      vendorId: '0x04FE',
      productId: '0x0021',
    });
    expect(summary.id).toBe('new-id');
    const call = fetchMock.mock.calls[0];
    expect(call[0]).toBe('http://localhost:7331/profiles');
    expect(call[1].method).toBe('POST');
    expect(JSON.parse(call[1].body as string).name).toBe('Created');
  });

  it('getProfile() parses the VIA body', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        id: 'p1',
        name: 'P1',
        tags: ['a'],
        created_at: 1,
        updated_at: 2,
        via: {
          name: 'HHKB',
          vendorId: '0x04FE',
          productId: '0x0021',
          layers: [['KC_ESC']],
        },
      }),
    );
    const rec = await client.getProfile('p1');
    expect(rec.via.name).toBe('HHKB');
    expect(rec.via.layers?.[0]).toEqual(['KC_ESC']);
  });

  it('throws DaemonError on 404', async () => {
    fetchMock.mockResolvedValueOnce(
      new Response('not found', { status: 404 }),
    );
    await expect(client.getProfile('missing')).rejects.toBeInstanceOf(
      DaemonError,
    );
  });

  it('throws DaemonError on 500', async () => {
    fetchMock.mockResolvedValueOnce(
      new Response('boom', { status: 500 }),
    );
    await expect(client.listProfiles()).rejects.toBeInstanceOf(DaemonError);
  });

  it('throws DaemonError on network failure', async () => {
    fetchMock.mockRejectedValueOnce(new TypeError('failed to fetch'));
    await expect(client.health()).rejects.toBeInstanceOf(DaemonError);
  });

  it('setActiveProfile() POSTs the id', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ id: 'xyz' }));
    await client.setActiveProfile('xyz');
    const call = fetchMock.mock.calls[0];
    expect(call[0]).toBe('http://localhost:7331/profiles/active');
    expect(call[1].method).toBe('POST');
    expect(JSON.parse(call[1].body as string)).toEqual({ id: 'xyz' });
  });

  it('readKeymap() builds the query string', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        mode: 'mac',
        fn_layer: true,
        data: new Array(128).fill(0),
      }),
    );
    const res = await client.readKeymap('mac', true);
    expect(res.data).toHaveLength(128);
    const call = fetchMock.mock.calls[0];
    expect(String(call[0])).toContain('mode=mac');
    expect(String(call[0])).toContain('fn_layer=true');
  });
});
