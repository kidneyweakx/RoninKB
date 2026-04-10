import { describe, expect, it } from 'vitest';
import {
  REQUEST_SIZE,
  RESPONSE_SIZE,
  buildRequest,
  parseResponse,
} from '../protocol';

describe('protocol.buildRequest', () => {
  it('produces a 64-byte buffer with magic + command', () => {
    const req = buildRequest(0x02);
    expect(req).toBeInstanceOf(Uint8Array);
    expect(req.length).toBe(REQUEST_SIZE);
    expect(req[0]).toBe(0xaa);
    expect(req[1]).toBe(0xaa);
    expect(req[2]).toBe(0x02);
    // everything else is padded with zeros
    for (let i = 3; i < req.length; i++) expect(req[i]).toBe(0);
  });

  it('writes params starting at offset 3', () => {
    const req = buildRequest(0x87, [0x00, 0x02, 0x01, 0x00]);
    expect(req[2]).toBe(0x87);
    expect(req[3]).toBe(0x00);
    expect(req[4]).toBe(0x02);
    expect(req[5]).toBe(0x01);
    expect(req[6]).toBe(0x00);
    for (let i = 7; i < req.length; i++) expect(req[i]).toBe(0);
  });

  it('magic bytes are always present for any command', () => {
    for (const cmd of [0x00, 0x01, 0x02, 0x87, 0xff]) {
      const req = buildRequest(cmd);
      expect(req[0]).toBe(0xaa);
      expect(req[1]).toBe(0xaa);
      expect(req[2]).toBe(cmd);
    }
  });

  it('rejects params that exceed the available payload space', () => {
    const tooLong = new Uint8Array(62);
    expect(() => buildRequest(0x01, tooLong)).toThrow(/too long/);
  });
});

describe('protocol.parseResponse', () => {
  it('parses a well-formed 64-byte response', () => {
    const buf = new Uint8Array(RESPONSE_SIZE);
    buf[0] = 0x55;
    buf[1] = 0x55;
    buf[2] = 0x87;
    buf[3] = 0x00;
    buf[4] = 0x01;
    buf[5] = 0x04;
    for (let i = 6; i < buf.length; i++) buf[i] = i;

    const resp = parseResponse(buf);
    expect(resp.command).toBe(0x87);
    expect(resp.status).toBe(0x00);
    expect(resp.param).toBe(0x01);
    expect(resp.length).toBe(0x04);
    expect(resp.data.length).toBe(58);
    // first data byte should be 6, last should be 63
    expect(resp.data[0]).toBe(6);
    expect(resp.data[57]).toBe(63);
  });

  it('throws on invalid magic', () => {
    const buf = new Uint8Array(RESPONSE_SIZE);
    buf[0] = 0x00;
    buf[1] = 0x00;
    expect(() => parseResponse(buf)).toThrow(/Invalid response magic/);
  });

  it('throws on truncated responses', () => {
    expect(() => parseResponse(new Uint8Array(0))).toThrow(/too short/);
    expect(() => parseResponse(new Uint8Array(10))).toThrow(/too short/);
  });
});
