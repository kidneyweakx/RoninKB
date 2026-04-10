//! Output formatting helpers — hex dumps, human-readable info renders, etc.

use hhkb_core::{DipSwitchState, KeyboardInfo, KeyboardMode, Keymap};

/// Format a 128-byte keymap as a classic 16-columns-per-row hex dump.
pub fn hex_dump(keymap: &Keymap) -> String {
    let bytes = keymap.as_bytes();
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let offset = row * 16;
        out.push_str(&format!("{offset:04x}  "));
        for (i, b) in chunk.iter().enumerate() {
            out.push_str(&format!("{b:02x}"));
            if i == 7 {
                out.push(' ');
            }
            out.push(' ');
        }
        out.push('\n');
    }
    out
}

/// Format a keyboard mode as the short human-readable name used by VIA.
pub fn mode_name(mode: KeyboardMode) -> &'static str {
    match mode {
        KeyboardMode::HHK => "HHK",
        KeyboardMode::Mac => "Mac",
        KeyboardMode::Lite => "Lite",
        KeyboardMode::Secret => "Secret",
    }
}

/// Format a byte slice as a space-separated hex string.
fn bytes_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Render [`KeyboardInfo`] as a multi-line human-readable block.
pub fn render_info(info: &KeyboardInfo) -> String {
    let revision = bytes_hex(&info.revision);
    let serial = bytes_hex(&info.serial);
    let app_fw = bytes_hex(&info.app_firmware);
    let boot_fw = bytes_hex(&info.boot_firmware);

    format!(
        "Type number:    {}\n\
         Revision:       {}\n\
         Serial:         {}\n\
         App firmware:   {}\n\
         Boot firmware:  {}\n\
         Running:        {:?}",
        info.type_number, revision, serial, app_fw, boot_fw, info.running_firmware
    )
}

/// Render a [`DipSwitchState`] as `SW1..SW6: on/off` lines.
pub fn render_dipsw(state: &DipSwitchState) -> String {
    let mut out = String::new();
    for (i, on) in state.switches.iter().enumerate() {
        let label = if *on { "on" } else { "off" };
        out.push_str(&format!("SW{}: {}\n", i + 1, label));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use hhkb_core::{FirmwareType, KeyboardInfo, KeyboardMode, Keymap};

    #[test]
    fn hex_dump_has_8_rows_for_128_bytes() {
        let km = Keymap::new();
        let dump = hex_dump(&km);
        assert_eq!(dump.lines().count(), 8);
        // First row offset.
        assert!(dump.starts_with("0000  "));
        // Last row offset should be 0x0070 (112).
        assert!(dump.lines().last().unwrap().starts_with("0070  "));
    }

    #[test]
    fn hex_dump_shows_byte_values() {
        let mut raw = [0u8; 128];
        raw[0] = 0x1f;
        raw[1] = 0xe0;
        let km = Keymap::from_bytes(raw);
        let dump = hex_dump(&km);
        let first_line = dump.lines().next().unwrap();
        assert!(first_line.contains("1f"));
        assert!(first_line.contains("e0"));
    }

    #[test]
    fn mode_name_maps_all_modes() {
        assert_eq!(mode_name(KeyboardMode::HHK), "HHK");
        assert_eq!(mode_name(KeyboardMode::Mac), "Mac");
        assert_eq!(mode_name(KeyboardMode::Lite), "Lite");
        assert_eq!(mode_name(KeyboardMode::Secret), "Secret");
    }

    #[test]
    fn render_info_contains_fields() {
        let info = KeyboardInfo {
            type_number: "PD-KB800BNS".to_string(),
            revision: [1, 0, 0, 2],
            serial: [0xAA; 16],
            app_firmware: [2, 0, 0, 0, 0, 0, 0, 0],
            boot_firmware: [1, 0, 0, 0, 0, 0, 0, 0],
            running_firmware: FirmwareType::Application,
        };
        let text = render_info(&info);
        assert!(text.contains("PD-KB800BNS"));
        assert!(text.contains("01 00 00 02"));
        assert!(text.contains("aa aa"));
        assert!(text.contains("Application"));
    }

    #[test]
    fn render_dipsw_prints_six_lines() {
        let state = DipSwitchState {
            switches: [true, false, true, false, true, false],
        };
        let text = render_dipsw(&state);
        assert_eq!(text.lines().count(), 6);
        assert!(text.contains("SW1: on"));
        assert!(text.contains("SW2: off"));
        assert!(text.contains("SW6: off"));
    }
}
