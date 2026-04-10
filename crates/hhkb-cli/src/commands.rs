//! Subcommand implementations.
//!
//! Every command that needs to talk to a keyboard goes through
//! [`with_device`], which handles `HidApiTransport::open()`,
//! `open_session()` on entry, and `close_session()` on exit.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use hhkb_core::device::HhkbDevice;
use hhkb_core::transport::{HhkbDeviceInfo, HidApiTransport};
use hhkb_core::via::{HardwareConfig, ProfileMeta, RawLayers, RoninExtension};
use hhkb_core::{KeyboardMode, Keymap, ViaProfile, HHKB_VENDOR_ID};
use uuid::Uuid;

use crate::format;

// ---------------------------------------------------------------------------
// Device helper
// ---------------------------------------------------------------------------

/// Open the first attached HHKB, run a closure, then close the session.
///
/// The `open_session` / `close_session` pair is always emitted regardless of
/// whether the closure succeeds: if it fails, the error is returned and we
/// still make a best-effort attempt to close the session.
fn with_device<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&HhkbDevice<HidApiTransport>) -> Result<R>,
{
    let transport = HidApiTransport::open()
        .context("failed to open HHKB device (no matching device found?)")?;
    let device = HhkbDevice::new(transport);

    device
        .open_session()
        .context("failed to open device session")?;

    let result = f(&device);

    // Always try to close the session; log (but don't clobber) close errors
    // if the primary operation already failed.
    match device.close_session() {
        Ok(()) => {}
        Err(e) if result.is_ok() => {
            return Err(anyhow::Error::new(e).context("failed to close device session"));
        }
        Err(e) => {
            eprintln!("warning: close_session failed after earlier error: {e}");
        }
    }

    result
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

pub fn list() -> Result<()> {
    let devices = HidApiTransport::list().context("failed to enumerate HID devices")?;
    if devices.is_empty() {
        println!("No HHKB devices found.");
        return Ok(());
    }

    println!("Found {} HHKB device(s):", devices.len());
    for (i, d) in devices.iter().enumerate() {
        print_device(i, d);
    }
    Ok(())
}

fn print_device(index: usize, d: &HhkbDeviceInfo) {
    println!("[{index}]");
    println!("  vendor_id:    0x{:04x}", d.vendor_id);
    println!("  product_id:   0x{:04x}", d.product_id);
    println!("  manufacturer: {}", d.manufacturer);
    println!("  product:      {}", d.product);
    if !d.serial.is_empty() {
        println!("  serial:       {}", d.serial);
    }
    println!("  path:         {}", d.path.to_string_lossy());
}

// ---------------------------------------------------------------------------
// info / mode / dipsw
// ---------------------------------------------------------------------------

pub fn info() -> Result<()> {
    with_device(|device| {
        let info = device.get_info().context("get_info failed")?;
        println!("{}", format::render_info(&info));
        Ok(())
    })
}

pub fn mode() -> Result<()> {
    with_device(|device| {
        let mode = device.get_mode().context("get_mode failed")?;
        println!("Mode: {}", format::mode_name(mode));
        Ok(())
    })
}

pub fn dipsw() -> Result<()> {
    with_device(|device| {
        let state = device.get_dip_switch().context("get_dip_switch failed")?;
        print!("{}", format::render_dipsw(&state));
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// dump
// ---------------------------------------------------------------------------

pub fn dump() -> Result<()> {
    with_device(|device| {
        let info = device.get_info().context("get_info failed")?;
        let mode = device.get_mode().context("get_mode failed")?;
        let dipsw = device.get_dip_switch().context("get_dip_switch failed")?;
        let keymap = device
            .read_keymap(mode, false)
            .context("read_keymap (base layer) failed")?;
        let fn_keymap = device
            .read_keymap(mode, true)
            .context("read_keymap (fn layer) failed")?;

        println!("== Keyboard info ==");
        println!("{}", format::render_info(&info));
        println!();
        println!("== Mode ==");
        println!("{}", format::mode_name(mode));
        println!();
        println!("== DIP switches ==");
        print!("{}", format::render_dipsw(&dipsw));
        println!();
        println!(
            "== Base keymap ({} overrides) ==",
            keymap.overridden_count()
        );
        print!("{}", format::hex_dump(&keymap));
        println!();
        println!(
            "== Fn keymap ({} overrides) ==",
            fn_keymap.overridden_count()
        );
        print!("{}", format::hex_dump(&fn_keymap));
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// read-keymap
// ---------------------------------------------------------------------------

pub fn read_keymap(mode: KeyboardMode, fn_layer: bool, output: Option<&Path>) -> Result<()> {
    let keymap = with_device(|device| {
        device
            .read_keymap(mode, fn_layer)
            .context("read_keymap failed")
    })?;

    let json = keymap_to_json(&keymap, mode, fn_layer)?;
    write_output(output, &json)
}

/// Build the JSON representation of a keymap read (used by `read-keymap`).
///
/// Format:
/// ```json
/// {
///   "mode": "Mac",
///   "layer": "base",
///   "bytes": [0, 1, 2, ...]
/// }
/// ```
fn keymap_to_json(keymap: &Keymap, mode: KeyboardMode, fn_layer: bool) -> Result<String> {
    let value = serde_json::json!({
        "mode": format::mode_name(mode),
        "layer": if fn_layer { "fn" } else { "base" },
        "bytes": keymap.as_bytes().to_vec(),
    });
    serde_json::to_string_pretty(&value).context("failed to serialize keymap JSON")
}

// ---------------------------------------------------------------------------
// write-keymap
// ---------------------------------------------------------------------------

pub fn write_keymap(file: &Path, mode: KeyboardMode, fn_layer: bool, yes: bool) -> Result<()> {
    let keymap = load_keymap_from_file(file)
        .with_context(|| format!("failed to load keymap from {}", file.display()))?;

    if !yes && !confirm_prompt("This will write to EEPROM. Continue? [y/N] ")? {
        println!("Aborted.");
        return Ok(());
    }

    with_device(|device| {
        device
            .write_keymap(mode, fn_layer, &keymap)
            .context("write_keymap failed")
    })?;

    println!(
        "Wrote 128-byte keymap to {} layer ({}).",
        if fn_layer { "fn" } else { "base" },
        format::mode_name(mode)
    );
    Ok(())
}

/// Accept either of two JSON shapes:
///
/// 1. The `read-keymap` output: `{ "bytes": [128 ints], ... }`.
/// 2. A raw array of 128 integers: `[0, 0, 0, ..., 0]`.
fn load_keymap_from_file(path: &Path) -> Result<Keymap> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).context("failed to parse JSON")?;

    let bytes_value: &serde_json::Value = match &value {
        serde_json::Value::Array(_) => &value,
        serde_json::Value::Object(obj) => obj
            .get("bytes")
            .context("JSON object has no `bytes` field")?,
        _ => bail!("JSON must be an array or object with `bytes` field"),
    };

    let arr = bytes_value
        .as_array()
        .context("`bytes` field is not an array")?;
    if arr.len() != 128 {
        bail!("expected 128 bytes, got {}", arr.len());
    }

    let mut raw = [0u8; 128];
    for (i, v) in arr.iter().enumerate() {
        let n = v
            .as_u64()
            .with_context(|| format!("byte {i} is not a non-negative integer"))?;
        if n > 0xFF {
            bail!("byte {i} out of range: {n} > 255");
        }
        raw[i] = n as u8;
    }
    Ok(Keymap::from_bytes(raw))
}

fn confirm_prompt(message: &str) -> Result<bool> {
    print!("{message}");
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("failed to read confirmation")?;
    let trimmed = line.trim().to_ascii_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}

// ---------------------------------------------------------------------------
// export-via
// ---------------------------------------------------------------------------

pub fn export_via(output: Option<&Path>) -> Result<()> {
    // Snapshot the actual product_id BEFORE opening the session, so the
    // exported VIA JSON reflects the real attached device rather than
    // a hardcoded guess.
    let product_id = HidApiTransport::list()
        .ok()
        .and_then(|devs| devs.into_iter().next().map(|d| d.product_id))
        .unwrap_or(hhkb_core::HHKB_PRODUCT_IDS[0]);

    let (info, mode, base_keymap, fn_keymap) = with_device(|device| {
        let info = device.get_info().context("get_info failed")?;
        let mode = device.get_mode().context("get_mode failed")?;
        let base = device
            .read_keymap(mode, false)
            .context("read_keymap (base) failed")?;
        let fn_km = device
            .read_keymap(mode, true)
            .context("read_keymap (fn) failed")?;
        Ok((info, mode, base, fn_km))
    })?;

    let profile = build_via_profile(&info, product_id, mode, &base_keymap, &fn_keymap);
    let json = profile.to_json().context("failed to serialize VIA JSON")?;
    write_output(output, &json)
}

fn build_via_profile(
    info: &hhkb_core::KeyboardInfo,
    product_id: u16,
    mode: KeyboardMode,
    base: &Keymap,
    fn_km: &Keymap,
) -> ViaProfile {
    let product_id_str = format!("0x{product_id:04X}");

    ViaProfile {
        name: info.type_number.clone(),
        vendor_id: format!("0x{HHKB_VENDOR_ID:04X}"),
        product_id: product_id_str,
        matrix: None,
        layouts: None,
        layers: Vec::new(),
        lighting: None,
        keycodes: Vec::new(),
        ronin: Some(RoninExtension {
            version: "1.0".to_string(),
            profile: ProfileMeta {
                id: Uuid::new_v4(),
                name: format!("Exported from {}", info.type_number),
                icon: None,
                tags: Vec::new(),
            },
            hardware: Some(HardwareConfig {
                keyboard_mode: u8::from(mode),
                raw_layers: RawLayers {
                    base: base.as_bytes().to_vec(),
                    r#fn: fn_km.as_bytes().to_vec(),
                },
            }),
            software: None,
        }),
    }
}

// ---------------------------------------------------------------------------
// output helpers
// ---------------------------------------------------------------------------

fn write_output(path: Option<&Path>, content: &str) -> Result<()> {
    match path {
        Some(p) => {
            fs::write(p, content).with_context(|| format!("failed to write {}", p.display()))?;
            println!("Wrote {}", p.display());
        }
        None => {
            println!("{content}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// discover-key  /  discover-keymap-table
// ---------------------------------------------------------------------------

/// The HHKB Professional Hybrid stores its 60 physical keys in indices
/// 1..=60 of the 128-byte keymap. The mapping below (confirmed from
/// happy-hacking-gnu `hhkb_print_layout_ansi()`) lists the byte indices per
/// row, in physical left → right order.
///
/// See `spec/protocol/keymap-encoding.md` and
/// `apps/hhkb-app/src/data/hhkbLayout.ts`.
const HHKB_LAYOUT_ROWS: &[&[u8]] = &[
    // Row 0 (number row, 15 keys): 60..=46
    &[60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46],
    // Row 1 (Q row, 14 keys): 45..=32
    &[45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32],
    // Row 2 (A row, 13 keys): 31..=19
    &[31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19],
    // Row 3 (Z row, 13 keys): 18..=6
    &[18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6],
    // Row 4 (modifier row, 5 keys): 5..=1
    &[5, 4, 3, 2, 1],
];

// Labels describe the FACTORY HHKB Pro 2 ANSI physical positions
// (left → right within each row). Your stored bytes may differ if you
// have a customized keymap — these are just orientation hints.
const HHKB_ROW_LABELS: &[&str] = &[
    "Row 0 (top, 60..46)   `  1  2  3  4  5  6  7  8  9  0  -  =  \\  BSpc",
    "Row 1 (Q,   45..32)   Tab  Q  W  E  R  T  Y  U  I  O  P  [  ]  \\",
    "Row 2 (A,   31..19)   Ctrl  A  S  D  F  G  H  J  K  L  ;  '  Enter",
    "Row 3 (Z,   18..6 )   LShift  Z  X  C  V  B  N  M  ,  .  /  RShift  Fn",
    "Row 4 (mod, 5..1  )   LMeta  LAlt  Space  RAlt  RMeta   (Mac: Meta=Cmd)",
];

/// Render the 128-byte keymap as an HHKB ANSI-shaped grid: for each row we
/// print the byte indices followed by the bytes found at those indices.
/// Indices outside 1..=60 are implicitly ignored (they are reserved/unused
/// on the 60-key HHKB).
pub fn render_keymap_grid(keymap: &Keymap) -> String {
    let mut out = String::new();
    for (row_idx, indices) in HHKB_LAYOUT_ROWS.iter().enumerate() {
        out.push_str(HHKB_ROW_LABELS[row_idx]);
        out.push('\n');
        // Line 1: indices
        out.push_str("  idx:  ");
        for (i, idx) in indices.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&format!("{idx:>3}"));
        }
        out.push('\n');
        // Line 2: values at those indices
        out.push_str("  val:  ");
        for (i, idx) in indices.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let byte = keymap.get(*idx as usize).unwrap_or(0);
            out.push_str(&format!(" {byte:02x}"));
        }
        out.push('\n');
        out.push('\n');
    }
    out
}

pub fn discover_keymap_table(mode: KeyboardMode, fn_layer: bool) -> Result<()> {
    let keymap = with_device(|device| {
        device
            .read_keymap(mode, fn_layer)
            .context("read_keymap failed")
    })?;

    println!(
        "== Keymap grid ({}, {} layer) ==",
        format::mode_name(mode),
        if fn_layer { "fn" } else { "base" }
    );
    println!();
    print!("{}", render_keymap_grid(&keymap));
    println!("Note: 0x00 = firmware default (no override).");
    println!("Indices 0 and 61..=127 are reserved/unused on the 60-key HHKB.");
    Ok(())
}

pub fn discover_key(
    index: u8,
    mode: KeyboardMode,
    fn_layer: bool,
    sentinel: u8,
    wait_secs: u64,
) -> Result<()> {
    if index as usize >= hhkb_core::keymap::KEYMAP_SIZE {
        bail!(
            "index {index} out of range (must be 0..{})",
            hhkb_core::keymap::KEYMAP_SIZE
        );
    }

    with_device(|device| {
        // 1. Read + back up the current keymap.
        let original = device
            .read_keymap(mode, fn_layer)
            .context("read_keymap (backup) failed")?;
        let original_byte = original.get(index as usize).unwrap_or(0);

        println!(
            "Backed up current keymap ({} layer, mode {}).",
            if fn_layer { "fn" } else { "base" },
            format::mode_name(mode)
        );
        println!("  keymap[{index}] = 0x{original_byte:02x} (will be restored automatically)");

        // 2. Build modified keymap with the sentinel.
        let mut modified = original.clone();
        modified
            .set(index as usize, sentinel)
            .with_context(|| format!("failed to set keymap[{index}] = {sentinel}"))?;

        // 3. Write the modified keymap.
        device
            .write_keymap(mode, fn_layer, &modified)
            .context("write_keymap (sentinel) failed")?;
        println!(
            "Wrote sentinel 0x{sentinel:02x} to keymap[{index}]. Press ENTER to start \
             the {wait_secs}-second probe window."
        );

        // 4. Wait for user to hit ENTER.
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .context("failed to read prompt input")?;
        println!(
            "Now press keys on the keyboard. The physical key mapped to byte index \
             {index} should produce the HID keycode 0x{sentinel:02x}."
        );
        println!("Restoring original keymap in {wait_secs} seconds...");

        // 5. Sleep for the probe window.
        thread::sleep(Duration::from_secs(wait_secs));

        // 6. Restore. We attempt this even if subsequent steps fail.
        let restore_result = device
            .write_keymap(mode, fn_layer, &original)
            .context("write_keymap (restore) failed");

        match &restore_result {
            Ok(()) => println!("Original keymap restored."),
            Err(e) => eprintln!(
                "WARNING: failed to restore original keymap: {e:#}. \
                 Use `hhkb write-keymap` manually to recover."
            ),
        }

        println!();
        println!("== Summary ==");
        println!(
            "  index:    {index}  ({}, {} layer)",
            format::mode_name(mode),
            if fn_layer { "fn" } else { "base" }
        );
        println!("  sentinel: 0x{sentinel:02x}");
        println!("  original: 0x{original_byte:02x} (restored)");
        println!(
            "If pressing a key produced the sentinel keycode, that key is at byte \
             index {index}."
        );

        restore_result
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keymap_to_json_round_trips_via_load() {
        let mut raw = [0u8; 128];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(3);
        }
        let km = Keymap::from_bytes(raw);
        let json = keymap_to_json(&km, KeyboardMode::Mac, false).unwrap();

        // Write to a temp file and reload.
        let dir = tempdir();
        let path = dir.join("km.json");
        fs::write(&path, &json).unwrap();
        let loaded = load_keymap_from_file(&path).unwrap();
        assert_eq!(loaded, km);
    }

    #[test]
    fn load_keymap_accepts_bare_array() {
        let arr: Vec<u8> = (0..128u8).collect();
        let json = serde_json::to_string(&arr).unwrap();
        let dir = tempdir();
        let path = dir.join("arr.json");
        fs::write(&path, json).unwrap();
        let loaded = load_keymap_from_file(&path).unwrap();
        for i in 0..128 {
            assert_eq!(loaded.get(i), Some(i as u8));
        }
    }

    #[test]
    fn load_keymap_rejects_wrong_length() {
        let arr = vec![0u8; 100];
        let json = serde_json::to_string(&arr).unwrap();
        let dir = tempdir();
        let path = dir.join("short.json");
        fs::write(&path, json).unwrap();
        let err = load_keymap_from_file(&path).unwrap_err();
        assert!(err.to_string().contains("expected 128 bytes"));
    }

    #[test]
    fn load_keymap_rejects_out_of_range() {
        let json = "[300, 0, 0]";
        let dir = tempdir();
        let path = dir.join("bad.json");
        fs::write(&path, json).unwrap();
        let err = load_keymap_from_file(&path).unwrap_err();
        assert!(
            err.to_string().contains("out of range")
                || err.to_string().contains("expected 128 bytes")
        );
    }

    #[test]
    fn build_via_profile_embeds_raw_layers() {
        let info = hhkb_core::KeyboardInfo {
            type_number: "PD-KB800BNS".to_string(),
            revision: [1, 0, 0, 0],
            serial: [0; 16],
            app_firmware: [0; 8],
            boot_firmware: [0; 8],
            running_firmware: hhkb_core::FirmwareType::Application,
        };
        let mut base_raw = [0u8; 128];
        base_raw[0] = 0x29;
        let mut fn_raw = [0u8; 128];
        fn_raw[1] = 0x3A;

        let base = Keymap::from_bytes(base_raw);
        let fn_km = Keymap::from_bytes(fn_raw);

        let profile = build_via_profile(&info, 0x0021, KeyboardMode::Mac, &base, &fn_km);
        assert_eq!(profile.name, "PD-KB800BNS");
        assert_eq!(profile.vendor_id, "0x04FE");
        // Verify the actual product_id propagates (regression: A1 from
        // hardware-baseline session — used to be hardcoded to 0x0020).
        assert_eq!(profile.product_id, "0x0021");
        let ronin = profile.ronin.as_ref().unwrap();
        assert_eq!(ronin.version, "1.0");
        let hw = ronin.hardware.as_ref().unwrap();
        assert_eq!(hw.keyboard_mode, u8::from(KeyboardMode::Mac));
        assert_eq!(hw.raw_layers.base.len(), 128);
        assert_eq!(hw.raw_layers.r#fn.len(), 128);
        assert_eq!(hw.raw_layers.base[0], 0x29);
        assert_eq!(hw.raw_layers.r#fn[1], 0x3A);
    }

    #[test]
    fn hhkb_layout_rows_cover_exactly_60_physical_keys() {
        let mut seen = std::collections::BTreeSet::new();
        for row in HHKB_LAYOUT_ROWS {
            for idx in *row {
                assert!((1..=60).contains(idx), "index {idx} out of 1..=60");
                assert!(seen.insert(*idx), "duplicate index {idx}");
            }
        }
        assert_eq!(seen.len(), 60);
        // Row widths match spec.
        assert_eq!(HHKB_LAYOUT_ROWS[0].len(), 15);
        assert_eq!(HHKB_LAYOUT_ROWS[1].len(), 14);
        assert_eq!(HHKB_LAYOUT_ROWS[2].len(), 13);
        assert_eq!(HHKB_LAYOUT_ROWS[3].len(), 13);
        assert_eq!(HHKB_LAYOUT_ROWS[4].len(), 5);
    }

    #[test]
    fn render_keymap_grid_shows_values_at_indices() {
        let mut raw = [0u8; 128];
        raw[60] = 0x35; // backtick
        raw[3] = 0x2c; // space
        raw[30] = 0x04; // A
        let km = Keymap::from_bytes(raw);
        let text = render_keymap_grid(&km);

        // Row 0 / row 4 labels present.
        assert!(text.contains("Row 0 (top"));
        assert!(text.contains("Row 4 (mod"));

        // Value 35 (backtick) should appear in row 0 line.
        let row0_block: String = text
            .lines()
            .skip_while(|l| !l.starts_with("Row 0"))
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            row0_block.contains(" 35"),
            "row 0 block missing 35: {row0_block}"
        );

        // Value 2c (space) should appear in row 4 line.
        let row4_block: String = text
            .lines()
            .skip_while(|l| !l.starts_with("Row 4"))
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            row4_block.contains(" 2c"),
            "row 4 block missing 2c: {row4_block}"
        );

        // Value 04 (A) should appear in row 2 line.
        let row2_block: String = text
            .lines()
            .skip_while(|l| !l.starts_with("Row 2"))
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            row2_block.contains(" 04"),
            "row 2 block missing 04: {row2_block}"
        );
    }

    // -- tiny throwaway temp directory helper --
    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let unique = format!(
            "hhkb-cli-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        p.push(unique);
        fs::create_dir_all(&p).unwrap();
        p
    }
}
