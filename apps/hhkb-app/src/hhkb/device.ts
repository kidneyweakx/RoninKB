/**
 * High-level HHKB device API.
 *
 * Wraps a `WebHidTransport` with a request-response interaction pattern and
 * typed command helpers.
 */

import {
  CMD_GET_KEYBOARD_INFO,
  CMD_GET_KEYBOARD_MODE,
  CMD_GET_DIP_SWITCH,
  CMD_GET_KEYMAP,
  CMD_CONFIRM_KEYMAP,
  CMD_NOTIFY_APP_STATE,
  CMD_RESET_DIPSW,
  CMD_SET_KEYMAP,
  confirmKeymap,
  getDipSwitch,
  getKeyboardInfo,
  getKeyboardMode,
  getKeymap,
  notifyAppClose,
  notifyAppOpen,
  parseDipSwitchResponse,
  parseKeyboardInfoResponse,
  parseKeyboardModeResponse,
  resetDipsw,
  setKeymapWrite1,
  setKeymapWrite2,
  setKeymapWrite3,
} from './commands';
import { Keymap } from './keymap';
import { HhkbResponse, parseResponse } from './protocol';
import { DipSwitchState, KeyboardInfo, KeyboardMode } from './types';
import { WebHidTransport } from './webhid';

/** Used to suppress the unused-constant warning while making the IDs importable. */
const _CMD_REFS = [
  CMD_GET_KEYBOARD_INFO,
  CMD_GET_KEYBOARD_MODE,
  CMD_GET_DIP_SWITCH,
  CMD_GET_KEYMAP,
  CMD_CONFIRM_KEYMAP,
  CMD_NOTIFY_APP_STATE,
  CMD_RESET_DIPSW,
  CMD_SET_KEYMAP,
] as const;
void _CMD_REFS;

export class HhkbDevice {
  constructor(private readonly transport: WebHidTransport) {}

  static async request(): Promise<HhkbDevice> {
    const t = await WebHidTransport.request();
    return new HhkbDevice(t);
  }

  get productName(): string {
    return this.transport.productName;
  }

  get isOpen(): boolean {
    return this.transport.isOpen;
  }

  async close(): Promise<void> {
    await this.transport.close();
  }

  /** Write a request then read and parse a single 64-byte response. */
  private async exchange(request: Uint8Array): Promise<HhkbResponse> {
    // Attach the read promise BEFORE writing so we don't miss a fast reply.
    const readPromise = this.transport.read(1500);
    await this.transport.write(request);
    const raw = await readPromise;
    return parseResponse(raw);
  }

  async notifyAppOpen(): Promise<void> {
    await this.exchange(notifyAppOpen());
  }

  async notifyAppClose(): Promise<void> {
    await this.exchange(notifyAppClose());
  }

  async getKeyboardInfo(): Promise<KeyboardInfo> {
    return parseKeyboardInfoResponse(await this.exchange(getKeyboardInfo()));
  }

  async getKeyboardMode(): Promise<KeyboardMode> {
    return parseKeyboardModeResponse(await this.exchange(getKeyboardMode()));
  }

  async getDipSwitch(): Promise<DipSwitchState> {
    return parseDipSwitchResponse(await this.exchange(getDipSwitch()));
  }

  /**
   * Read a full 128-byte keymap for the given mode / layer.
   *
   * Issues one `GetKeymap` request and then reads 3 input reports, each
   * contributing a slice of the 128-byte layout.
   */
  async getKeymap(mode: KeyboardMode, fnLayer: boolean): Promise<Keymap> {
    // Queue up all three reads first so we don't race the keyboard.
    const r1 = this.transport.read(2000);
    const r2 = this.transport.read(2000);
    const r3 = this.transport.read(2000);
    await this.transport.write(getKeymap(mode, fnLayer));
    const [c1, c2, c3] = await Promise.all([r1, r2, r3]);
    return Keymap.fromChunks(c1, c2, c3);
  }

  /**
   * Write a full 128-byte keymap. Sends 3 write requests, each acknowledged
   * by a response, followed by a commit + dipsw reset.
   */
  async setKeymap(
    mode: KeyboardMode,
    fnLayer: boolean,
    keymap: Keymap,
  ): Promise<void> {
    const [a, b, c] = keymap.toWriteChunks();
    await this.exchange(setKeymapWrite1(mode, fnLayer, a));
    await this.exchange(setKeymapWrite2(b));
    await this.exchange(setKeymapWrite3(c));
    await this.exchange(confirmKeymap());
    await this.exchange(resetDipsw());
  }
}
