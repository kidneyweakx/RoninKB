/**
 * HHKB Professional HYBRID — factory default profiles.
 *
 * The HHKB EEPROM protocol uses `0x00` at a key index to mean "use the
 * keyboard firmware's built-in default for this key". Therefore a 128-byte
 * array of all zeros perfectly represents the factory state.
 *
 * We ship one factory profile per hardware mode so users can always
 * revert to the PFU out-of-box experience. Each profile carries a fixed
 * UUID so it can be identified even if the user renames it, and is
 * automatically seeded into the profile store on first launch.
 */

import type { Profile } from '../store/profileStore';

const ZERO_LAYER = new Array<number>(128).fill(0);

/**
 * HHKB factory default in Mac mode (SW1=OFF SW2=OFF, `keyboard_mode=1`).
 * This is the mode HHKB ships in out of the box.
 */
export const FACTORY_DEFAULT_MAC: Profile = {
  id: 'hhkb-factory-default-mac',
  name: 'HHKB Factory Default (Mac)',
  icon: 'factory',
  tags: ['factory', 'mac'],
  via: {
    name: 'HHKB Professional Hybrid',
    vendorId: '0x04FE',
    productId: '0x0021',
    matrix: { rows: 8, cols: 8 },
    layers: [],
    _roninKB: {
      version: '1.0',
      profile: {
        id: 'hhkb-factory-default-mac',
        name: 'HHKB Factory Default (Mac)',
        icon: 'factory',
        tags: ['factory', 'mac'],
      },
      hardware: {
        keyboard_mode: 1, // Mac mode
        raw_layers: {
          base: ZERO_LAYER,
          fn: ZERO_LAYER,
        },
      },
    },
  },
};

/** All built-in factory profiles that should always be present. */
export const FACTORY_PROFILES: Profile[] = [FACTORY_DEFAULT_MAC];
/** Fixed IDs of factory profiles, so the store knows not to delete them. */
export const FACTORY_PROFILE_IDS = new Set(FACTORY_PROFILES.map((p) => p.id));
