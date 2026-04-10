use crate::error::{Error, Result};

pub const KEYMAP_SIZE: usize = 128;
pub const CHUNK_DATA_OFFSET: usize = 6;
pub const CHUNK1_LEN: usize = 58;
pub const CHUNK2_LEN: usize = 58;
pub const CHUNK3_LEN: usize = 12;

/// HID report size for a single chunk read from the device.
const CHUNK_REPORT_SIZE: usize = 64;

/// A 128-byte keymap covering a single layer (base or Fn) for a single mode.
///
/// Each byte is a HID Usage ID override. A value of `0x00` means "use the
/// firmware default" for that key — i.e. no override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    data: [u8; KEYMAP_SIZE],
}

impl Keymap {
    /// Create a keymap with all zeros (all firmware defaults).
    pub fn new() -> Self {
        Self {
            data: [0u8; KEYMAP_SIZE],
        }
    }

    /// Create from a raw 128-byte array.
    pub fn from_bytes(data: [u8; KEYMAP_SIZE]) -> Self {
        Self { data }
    }

    /// Assemble a keymap from 3 HID response chunks (each 64 bytes).
    ///
    /// Keymap data is extracted starting at [`CHUNK_DATA_OFFSET`] from each
    /// chunk. The split across chunks is 58 / 58 / 12 = 128 bytes.
    ///
    /// Returns [`Error::InvalidKeymapSize`] if any chunk is the wrong size.
    pub fn from_chunks(chunk1: &[u8], chunk2: &[u8], chunk3: &[u8]) -> Result<Self> {
        if chunk1.len() != CHUNK_REPORT_SIZE {
            return Err(Error::InvalidKeymapSize(chunk1.len()));
        }
        if chunk2.len() != CHUNK_REPORT_SIZE {
            return Err(Error::InvalidKeymapSize(chunk2.len()));
        }
        if chunk3.len() != CHUNK_REPORT_SIZE {
            return Err(Error::InvalidKeymapSize(chunk3.len()));
        }

        let mut data = [0u8; KEYMAP_SIZE];
        data[0..CHUNK1_LEN]
            .copy_from_slice(&chunk1[CHUNK_DATA_OFFSET..CHUNK_DATA_OFFSET + CHUNK1_LEN]);
        data[CHUNK1_LEN..CHUNK1_LEN + CHUNK2_LEN]
            .copy_from_slice(&chunk2[CHUNK_DATA_OFFSET..CHUNK_DATA_OFFSET + CHUNK2_LEN]);
        data[CHUNK1_LEN + CHUNK2_LEN..KEYMAP_SIZE]
            .copy_from_slice(&chunk3[CHUNK_DATA_OFFSET..CHUNK_DATA_OFFSET + CHUNK3_LEN]);

        Ok(Self { data })
    }

    /// Split into 3 data slices suitable for the `WriteKeymap` command.
    ///
    /// Returns `(layout[0..57], layout[57..116], layout[116..128])`. These are
    /// the raw data bytes; framing is handled by `command.rs`.
    pub fn to_write_chunks(&self) -> ([u8; 57], [u8; 59], [u8; 12]) {
        let mut a = [0u8; 57];
        let mut b = [0u8; 59];
        let mut c = [0u8; 12];
        a.copy_from_slice(&self.data[0..57]);
        b.copy_from_slice(&self.data[57..116]);
        c.copy_from_slice(&self.data[116..128]);
        (a, b, c)
    }

    /// Get the HID keycode at a given index (0..127).
    pub fn get(&self, index: usize) -> Option<u8> {
        self.data.get(index).copied()
    }

    /// Set the HID keycode at a given index (0..127).
    pub fn set(&mut self, index: usize, value: u8) -> Result<()> {
        if index >= KEYMAP_SIZE {
            return Err(Error::InvalidKeymapSize(index));
        }
        self.data[index] = value;
        Ok(())
    }

    /// Returns `true` if the key at `index` uses the firmware default
    /// (i.e. its stored value is `0x00`). Out-of-range indices also return
    /// `true` since there is no override for them.
    pub fn is_default(&self, index: usize) -> bool {
        self.data.get(index).copied().unwrap_or(0) == 0
    }

    /// Get the raw 128-byte array.
    pub fn as_bytes(&self) -> &[u8; KEYMAP_SIZE] {
        &self.data
    }

    /// Count how many keys have non-default (non-zero) mappings.
    pub fn overridden_count(&self) -> usize {
        self.data.iter().filter(|b| **b != 0).count()
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_all_zeros() {
        let km = Keymap::new();
        assert_eq!(km.as_bytes(), &[0u8; KEYMAP_SIZE]);
    }

    #[test]
    fn test_from_bytes() {
        let mut raw = [0u8; KEYMAP_SIZE];
        raw[0] = 0x1F;
        raw[5] = 0x2C;
        raw[127] = 0xE0;
        let km = Keymap::from_bytes(raw);
        assert_eq!(km.as_bytes(), &raw);
    }

    #[test]
    fn test_from_chunks_real_data() {
        let mut chunk1 = [0u8; 64];
        let mut chunk2 = [0u8; 64];
        let mut chunk3 = [0u8; 64];

        // Header bytes [0..6] are ignored; data starts at offset 6.
        chunk1[6] = 0x1F;
        chunk1[7] = 0x1E;
        chunk1[8] = 0x29;
        // chunk2 and chunk3 all zeros.
        let _ = &mut chunk2;
        let _ = &mut chunk3;

        let km = Keymap::from_chunks(&chunk1, &chunk2, &chunk3).expect("assemble");
        assert_eq!(km.get(0), Some(0x1F));
        assert_eq!(km.get(1), Some(0x1E));
        assert_eq!(km.get(2), Some(0x29));
        for i in 3..KEYMAP_SIZE {
            assert_eq!(km.get(i), Some(0x00), "byte {} should be zero", i);
        }
    }

    #[test]
    fn test_from_chunks_wrong_size() {
        let short = [0u8; 32];
        let ok = [0u8; 64];

        let err = Keymap::from_chunks(&short, &ok, &ok).unwrap_err();
        assert!(matches!(err, Error::InvalidKeymapSize(32)));

        let err = Keymap::from_chunks(&ok, &short, &ok).unwrap_err();
        assert!(matches!(err, Error::InvalidKeymapSize(32)));

        let err = Keymap::from_chunks(&ok, &ok, &short).unwrap_err();
        assert!(matches!(err, Error::InvalidKeymapSize(32)));
    }

    #[test]
    fn test_to_write_chunks_roundtrip() {
        let mut raw = [0u8; KEYMAP_SIZE];
        for (i, byte) in raw.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(3).wrapping_add(1);
        }
        let km = Keymap::from_bytes(raw);
        let (a, b, c) = km.to_write_chunks();
        assert_eq!(a.len(), 57);
        assert_eq!(b.len(), 59);
        assert_eq!(c.len(), 12);

        let mut reassembled = [0u8; KEYMAP_SIZE];
        reassembled[0..57].copy_from_slice(&a);
        reassembled[57..116].copy_from_slice(&b);
        reassembled[116..128].copy_from_slice(&c);
        assert_eq!(reassembled, raw);
    }

    #[test]
    fn test_get_set() {
        let mut km = Keymap::new();
        km.set(3, 0x2C).expect("in range");
        assert_eq!(km.get(3), Some(0x2C));
    }

    #[test]
    fn test_get_out_of_bounds() {
        let km = Keymap::new();
        assert_eq!(km.get(128), None);
        assert_eq!(km.get(usize::MAX), None);
    }

    #[test]
    fn test_set_out_of_bounds() {
        let mut km = Keymap::new();
        let err = km.set(128, 0xFF).unwrap_err();
        assert!(matches!(err, Error::InvalidKeymapSize(128)));
    }

    #[test]
    fn test_is_default() {
        let mut km = Keymap::new();
        assert!(km.is_default(0));
        km.set(0, 0x04).unwrap();
        assert!(!km.is_default(0));
        assert!(km.is_default(1));
    }

    #[test]
    fn test_overridden_count() {
        let mut km = Keymap::new();
        assert_eq!(km.overridden_count(), 0);
        km.set(0, 0x1F).unwrap();
        km.set(10, 0x2C).unwrap();
        km.set(127, 0xE0).unwrap();
        assert_eq!(km.overridden_count(), 3);
    }

    #[test]
    fn test_default_trait_matches_new() {
        assert_eq!(Keymap::default(), Keymap::new());
    }
}
