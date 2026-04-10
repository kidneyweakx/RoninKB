import { describe, expect, it } from 'vitest';
import {
  confirmKeymap,
  getDipSwitch,
  getKeyboardInfo,
  getKeyboardMode,
  getKeymap,
  notifyAppClose,
  notifyAppOpen,
  resetDipsw,
  setKeymapWrite1,
  setKeymapWrite2,
  setKeymapWrite3,
  CMD_NOTIFY_APP_STATE,
  CMD_SET_KEYMAP,
  CMD_GET_KEYMAP,
  parseDipSwitchResponse,
  parseKeyboardInfoResponse,
  parseKeyboardModeResponse,
} from '../commands';
import { KeyboardMode, FirmwareType } from '../types';
import { HhkbResponse } from '../protocol';

function makeResponse(data: number[]): HhkbResponse {
  const padded = new Uint8Array(58);
  padded.set(data.slice(0, 58));
  return {
    command: 0,
    status: 0,
    param: 0,
    length: 0,
    data: padded,
  };
}

describe('command: NotifyAppState', () => {
  it('open request has state byte 0 at index 6', () => {
    const req = notifyAppOpen();
    expect(req[0]).toBe(0xaa);
    expect(req[1]).toBe(0xaa);
    expect(req[2]).toBe(CMD_NOTIFY_APP_STATE);
    expect(req[3]).toBe(0x00);
    expect(req[4]).toBe(0x01);
    expect(req[5]).toBe(0x00); // state byte = open
  });

  it('close request has state byte 1 at index 6', () => {
    const req = notifyAppClose();
    expect(req[2]).toBe(CMD_NOTIFY_APP_STATE);
    expect(req[3]).toBe(0x00);
    expect(req[4]).toBe(0x01);
    expect(req[5]).toBe(0x01);
  });
});

describe('command: GetKeyboardInfo / Mode / DipSwitch', () => {
  it('GetKeyboardInfo has no params', () => {
    const req = getKeyboardInfo();
    expect(req[2]).toBe(0x02);
    for (let i = 3; i < req.length; i++) expect(req[i]).toBe(0);
  });

  it('GetKeyboardMode has no params', () => {
    const req = getKeyboardMode();
    expect(req[2]).toBe(0x06);
    for (let i = 3; i < req.length; i++) expect(req[i]).toBe(0);
  });

  it('GetDipSwitch has no params', () => {
    const req = getDipSwitch();
    expect(req[2]).toBe(0x05);
  });
});

describe('command: GetKeymap', () => {
  it('Mac Mode, base layer', () => {
    const req = getKeymap(KeyboardMode.Mac, false);
    // See spec/protocol/commands.md — after stripping the report ID byte,
    // indices shift down by 1:
    //   [2]=CMD, [3]=0x00, [4]=0x02, [5]=mode, [6]=fn
    expect(req[2]).toBe(CMD_GET_KEYMAP);
    expect(req[3]).toBe(0x00);
    expect(req[4]).toBe(0x02);
    expect(req[5]).toBe(0x01); // Mac
    expect(req[6]).toBe(0x00); // base layer
  });

  it('HHK Mode, Fn layer', () => {
    const req = getKeymap(KeyboardMode.HHK, true);
    expect(req[2]).toBe(0x87);
    expect(req[3]).toBe(0x00);
    expect(req[4]).toBe(0x02);
    expect(req[5]).toBe(0x00); // HHK
    expect(req[6]).toBe(0x01); // Fn layer
  });
});

describe('command: SetKeymap (3 chunks)', () => {
  it('write1 frames mode, fn, and 57 bytes of data', () => {
    const data = new Uint8Array(57);
    for (let i = 0; i < 57; i++) data[i] = i;

    const req = setKeymapWrite1(KeyboardMode.Mac, true, data);
    expect(req[2]).toBe(CMD_SET_KEYMAP);
    expect(req[3]).toBe(65); // offset counter
    expect(req[4]).toBe(59); // chunk length
    expect(req[5]).toBe(KeyboardMode.Mac);
    expect(req[6]).toBe(1); // fn layer true
    for (let i = 0; i < 57; i++) expect(req[7 + i]).toBe(i);
  });

  it('write1 rejects wrong data length', () => {
    expect(() =>
      setKeymapWrite1(KeyboardMode.HHK, false, new Uint8Array(56)),
    ).toThrow();
  });

  it('write2 frames 59 bytes of data', () => {
    const data = new Uint8Array(59);
    for (let i = 0; i < 59; i++) data[i] = (i + 100) & 0xff;

    const req = setKeymapWrite2(data);
    expect(req[2]).toBe(CMD_SET_KEYMAP);
    expect(req[3]).toBe(130);
    expect(req[4]).toBe(59);
    for (let i = 0; i < 59; i++) expect(req[5 + i]).toBe((i + 100) & 0xff);
  });

  it('write3 frames 12 bytes of data and zero-pads the rest', () => {
    const data = Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    const req = setKeymapWrite3(data);
    expect(req[2]).toBe(CMD_SET_KEYMAP);
    expect(req[3]).toBe(195);
    expect(req[4]).toBe(12);
    for (let i = 0; i < 12; i++) expect(req[5 + i]).toBe(i + 1);
    for (let i = 17; i < req.length; i++) expect(req[i]).toBe(0);
  });
});

describe('command: ConfirmKeymap / ResetDipsw', () => {
  it('confirmKeymap has CMD 0x04 and no params', () => {
    const req = confirmKeymap();
    expect(req[2]).toBe(0x04);
    for (let i = 3; i < req.length; i++) expect(req[i]).toBe(0);
  });
  it('resetDipsw has CMD 0x07', () => {
    const req = resetDipsw();
    expect(req[2]).toBe(0x07);
  });
});

describe('all commands carry magic bytes', () => {
  it('every builder emits 0xAA 0xAA at the start', () => {
    const reqs = [
      notifyAppOpen(),
      notifyAppClose(),
      getKeyboardInfo(),
      getKeyboardMode(),
      getDipSwitch(),
      getKeymap(KeyboardMode.HHK, false),
      getKeymap(KeyboardMode.Mac, true),
      setKeymapWrite1(KeyboardMode.HHK, false, new Uint8Array(57)),
      setKeymapWrite2(new Uint8Array(59)),
      setKeymapWrite3(new Uint8Array(12)),
      confirmKeymap(),
      resetDipsw(),
    ];
    for (const req of reqs) {
      expect(req.length).toBe(64);
      expect(req[0]).toBe(0xaa);
      expect(req[1]).toBe(0xaa);
    }
  });
});

describe('response parsers', () => {
  it('parses keyboard info', () => {
    const data = new Array<number>(58).fill(0);
    const name = 'PD-KB800BNS';
    for (let i = 0; i < name.length; i++) data[i] = name.charCodeAt(i);
    data[20] = 1;
    data[21] = 2;
    data[22] = 3;
    data[23] = 4;
    for (let i = 24; i < 40; i++) data[i] = 0x5a;
    for (let i = 0; i < 8; i++) data[40 + i] = i;
    for (let i = 0; i < 8; i++) data[48 + i] = 10 + i;
    data[56] = 0; // Application

    const info = parseKeyboardInfoResponse(makeResponse(data));
    expect(info.typeNumber).toBe('PD-KB800BNS');
    expect(Array.from(info.revision)).toEqual([1, 2, 3, 4]);
    expect(info.serial.every((b) => b === 0x5a)).toBe(true);
    expect(info.runningFirmware).toBe(FirmwareType.Application);
  });

  it('parses keyboard mode', () => {
    const mode = parseKeyboardModeResponse(makeResponse([1]));
    expect(mode).toBe(KeyboardMode.Mac);
  });

  it('parses dip switch state', () => {
    const state = parseDipSwitchResponse(makeResponse([0, 1, 0, 1, 0, 0]));
    expect(state.switches).toEqual([false, true, false, true, false, false]);
  });
});
