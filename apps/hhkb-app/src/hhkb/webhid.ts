/**
 * WebHID transport wrapper for HHKB vendor interface.
 *
 * Uses `navigator.hid` directly — no external HID libraries.
 */

import { HHKB_PRODUCT_IDS, HHKB_USAGE_PAGE, HHKB_VENDOR_ID } from './types';
import { REPORT_ID } from './protocol';

/** Error thrown when WebHID is not available (non-Chrome, insecure context). */
export class WebHidUnavailableError extends Error {
  constructor() {
    super(
      'WebHID is not available in this browser. Use Chrome or Edge over HTTPS (or localhost).',
    );
    this.name = 'WebHidUnavailableError';
  }
}

export function isWebHidAvailable(): boolean {
  return typeof navigator !== 'undefined' && 'hid' in navigator;
}

export class WebHidTransport {
  constructor(public readonly device: HIDDevice) {}

  /**
   * Prompt the user to pick an HHKB device (must be called from a user gesture).
   */
  static async request(): Promise<WebHidTransport> {
    if (!isWebHidAvailable()) {
      throw new WebHidUnavailableError();
    }

    const filters: HIDDeviceFilter[] = HHKB_PRODUCT_IDS.map((pid) => ({
      vendorId: HHKB_VENDOR_ID,
      productId: pid,
      usagePage: HHKB_USAGE_PAGE,
    }));

    const devices = await navigator.hid.requestDevice({ filters });
    if (devices.length === 0) {
      throw new Error('No HHKB device selected');
    }
    const device = devices[0];
    if (!device.opened) {
      await device.open();
    }
    return new WebHidTransport(device);
  }

  /**
   * Return any previously-authorized HHKB devices already accessible without
   * prompting (useful for auto-reconnect).
   */
  static async getAlreadyAuthorized(): Promise<WebHidTransport[]> {
    if (!isWebHidAvailable()) return [];
    const devices = await navigator.hid.getDevices();
    const matching = devices.filter(
      (d) =>
        d.vendorId === HHKB_VENDOR_ID &&
        (HHKB_PRODUCT_IDS as readonly number[]).includes(d.productId),
    );
    const transports: WebHidTransport[] = [];
    for (const d of matching) {
      if (!d.opened) {
        try {
          await d.open();
        } catch {
          continue;
        }
      }
      transports.push(new WebHidTransport(d));
    }
    return transports;
  }

  /** Write a 64-byte HID output report. */
  async write(data: Uint8Array): Promise<void> {
    // The WebHID `sendReport` expects a `BufferSource`. Recent TS lib updates
    // type generic Uint8Arrays as `Uint8Array<ArrayBufferLike>`, which doesn't
    // match `ArrayBufferView<ArrayBuffer>` in strict mode, so we pass the
    // underlying ArrayBuffer slice explicitly.
    const view = data.buffer.slice(
      data.byteOffset,
      data.byteOffset + data.byteLength,
    ) as ArrayBuffer;
    await this.device.sendReport(REPORT_ID, view);
  }

  /**
   * Wait for the next HID input report, resolving with the raw byte buffer.
   * Rejects if no report arrives within `timeoutMs`.
   */
  read(timeoutMs = 1000): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
      const handler = (e: HIDInputReportEvent) => {
        clearTimeout(timer);
        this.device.removeEventListener('inputreport', handler);
        const buf = new Uint8Array(e.data.buffer, e.data.byteOffset, e.data.byteLength);
        // Copy so the caller owns the memory.
        resolve(new Uint8Array(buf));
      };
      const timer = setTimeout(() => {
        this.device.removeEventListener('inputreport', handler);
        reject(new Error('HID read timeout'));
      }, timeoutMs);
      this.device.addEventListener('inputreport', handler);
    });
  }

  async close(): Promise<void> {
    if (this.device.opened) {
      await this.device.close();
    }
  }

  get isOpen(): boolean {
    return this.device.opened;
  }

  get productName(): string {
    return this.device.productName;
  }
}
