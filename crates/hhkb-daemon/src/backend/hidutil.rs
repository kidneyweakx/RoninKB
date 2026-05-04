//! `HidutilBackend` — macOS-only system-wide key remapping via
//! `/usr/bin/hidutil property --set`.
//!
//! Use case: simple key swaps that need to take effect across every USB
//! keyboard the user plugs in (Caps↔Esc on built-in *and* externals
//! simultaneously) — something the EEPROM backend can't do because EEPROM
//! is per-keyboard. Persistence is handled by writing a LaunchAgent that
//! re-applies the mapping at login.
//!
//! Profile shape: `_roninKB.software.engine == "hidutil"`, with `config` as
//! a JSON document of the form
//! ```json
//! { "mappings": [ { "src": "KC_CAPS", "dst": "KC_ESC" }, ... ] }
//! ```
//! Anything else is rejected with `BackendError::ProfileRejected`. The
//! daemon's M4 selection layer routes profiles to the matching backend; this
//! module is fail-loud rather than silently doing the wrong thing.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;

use hhkb_core::ViaProfile;
use serde::{Deserialize, Serialize};

use super::{
    Backend, BackendDiagnostics, BackendError, BackendId, Capabilities, PermissionStatus,
    TapHoldQuality,
};

/// Bundle id used in the LaunchAgent label and plist filename. Distinct from
/// the daemon's own bundle id so the agent shows up as its own row in
/// launchctl listings.
const LAUNCH_AGENT_LABEL: &str = "gg.solidarity.roninkb.hidutil";

/// HID usage page 7 (Keyboard/Keypad) prefix that hidutil's
/// HIDKeyboardModifierMappingSrc/Dst expects: `0x700000000 | usage`.
const USAGE_PAGE_7: u64 = 0x7_0000_0000;

#[derive(Debug, Deserialize, Serialize)]
struct HidutilConfig {
    mappings: Vec<HidutilMapping>,
}

#[derive(Debug, Deserialize, Serialize)]
struct HidutilMapping {
    src: String,
    dst: String,
}

pub struct HidutilBackend;

impl HidutilBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HidutilBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for HidutilBackend {
    fn id(&self) -> BackendId {
        BackendId::Hidutil
    }

    fn human_name(&self) -> &'static str {
        "macOS hidutil"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            per_key_remap: true,
            // hidutil is a flat 1-to-1 source->destination map. No layering.
            layers: 1,
            tap_hold: TapHoldQuality::None,
            leader_keys: false,
            combos: false,
            app_aware: false,
            // hidutil is *system-wide*, not per-keyboard — that's the whole
            // selling point vs EEPROM. So per_device is the wrong word; we
            // treat it as false because the inverse capability ("only one
            // keyboard sees the rule") doesn't apply.
            per_device: false,
            // We re-apply via LaunchAgent at login, so it persists across
            // reboot in practice. The `apply()` itself doesn't survive an
            // un-login; reboot survival is the LaunchAgent's job.
            persistent: true,
            // hidutil writes are immediate; the kernel rebuilds the keymap
            // and the next keypress reflects it.
            hot_reload: true,
            macros: false,
            max_macro_length: 0,
        }
    }

    fn permission_status(&self) -> PermissionStatus {
        // Modifier-only remaps need no permission. Non-modifier remaps need
        // Input Monitoring on Sonoma+. We don't know which yet (depends on
        // the profile), so report Granted and let `apply()` surface a
        // hidutil-side failure if it happens. The native backend's
        // permission_status is the heavyweight check; hidutil is the
        // lightweight fallback.
        PermissionStatus::Granted
    }

    fn apply(&self, profile: &ViaProfile) -> Result<(), BackendError> {
        let config = parse_config(profile)?;
        let json_payload = build_property_payload(&config.mappings);

        let output = Command::new("/usr/bin/hidutil")
            .arg("property")
            .arg("--set")
            .arg(&json_payload)
            .output()
            .map_err(|e| BackendError::Internal(format!("invoke hidutil: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BackendError::Internal(format!(
                "hidutil exited {}: {stderr}",
                output.status
            )));
        }

        // LaunchAgent for reboot survival. We write it next to apply() (not
        // at backend construction time) so the agent always carries the
        // *current* mapping, not a stale one.
        write_launch_agent(&json_payload)?;
        Ok(())
    }

    fn teardown(&self) -> Result<(), BackendError> {
        // Clear the in-kernel mapping...
        let output = Command::new("/usr/bin/hidutil")
            .arg("property")
            .arg("--set")
            .arg(r#"{"UserKeyMapping":[]}"#)
            .output()
            .map_err(|e| BackendError::Internal(format!("invoke hidutil: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(%stderr, "hidutil clear-mapping failed");
        }
        // ...then drop the LaunchAgent so the cleared state survives reboot.
        let _ = remove_launch_agent();
        Ok(())
    }

    fn is_running(&self) -> bool {
        // We treat "running" as "the LaunchAgent is installed" — that's the
        // backend's persisted state. There's no daemonish process to check
        // beyond hidutil's one-shot invocation.
        launch_agent_path().exists()
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        BackendDiagnostics {
            state: if self.is_running() {
                "installed"
            } else {
                "not_installed"
            },
            note: None,
        }
    }
}

fn parse_config(profile: &ViaProfile) -> Result<HidutilConfig, BackendError> {
    let software = profile
        .ronin
        .as_ref()
        .and_then(|r| r.software.as_ref())
        .ok_or_else(|| {
            BackendError::ProfileRejected(
                "profile has no _roninKB.software section; hidutil backend needs one".to_string(),
            )
        })?;

    if software.engine != "hidutil" {
        return Err(BackendError::ProfileRejected(format!(
            "profile.engine = {:?}, expected \"hidutil\"",
            software.engine
        )));
    }

    serde_json::from_str::<HidutilConfig>(&software.config).map_err(|e| {
        BackendError::ProfileRejected(format!(
            "hidutil config JSON did not match schema {{mappings: [...]}}: {e}"
        ))
    })
}

fn build_property_payload(mappings: &[HidutilMapping]) -> String {
    // We can't serialise via serde directly because hidutil's property keys
    // are baroque (HIDKeyboardModifierMappingSrc/Dst with usage-page-7
    // bit-shifted prefixes). Build the JSON by hand for clarity.
    let mut entries: Vec<String> = Vec::with_capacity(mappings.len());
    for m in mappings {
        let Some(src) = via_to_hid_usage(&m.src) else {
            tracing::warn!(src = %m.src, "skipping mapping with unmappable src");
            continue;
        };
        let Some(dst) = via_to_hid_usage(&m.dst) else {
            tracing::warn!(dst = %m.dst, "skipping mapping with unmappable dst");
            continue;
        };
        let src_full = USAGE_PAGE_7 | u64::from(src);
        let dst_full = USAGE_PAGE_7 | u64::from(dst);
        entries.push(format!(
            r#"{{"HIDKeyboardModifierMappingSrc":{src_full},"HIDKeyboardModifierMappingDst":{dst_full}}}"#
        ));
    }
    format!(r#"{{"UserKeyMapping":[{}]}}"#, entries.join(","))
}

fn launch_agent_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist"))
}

fn write_launch_agent(json_payload: &str) -> Result<(), BackendError> {
    let path = launch_agent_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| BackendError::Internal(format!("mkdir LaunchAgents: {e}")))?;
    }

    // The plist embeds the JSON payload as the third hidutil arg. We escape
    // any `<` `>` `&` `"` so a profile that happens to contain them doesn't
    // produce malformed XML.
    let escaped = json_payload
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LAUNCH_AGENT_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/bin/hidutil</string>
    <string>property</string>
    <string>--set</string>
    <string>{escaped}</string>
  </array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#
    );

    std::fs::write(&path, plist)
        .map_err(|e| BackendError::Internal(format!("write LaunchAgent: {e}")))?;
    Ok(())
}

fn remove_launch_agent() -> std::io::Result<()> {
    let path = launch_agent_path();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// VIA keycode string → HID usage byte (page 0x07). Returns `None` for
/// unknown / transparent / no-op keycodes; callers skip those entries
/// instead of failing the whole profile.
///
/// This is a deliberately small subset — hidutil profiles are inherently
/// system-wide modifier swaps, not full keymaps. Common keys covered;
/// callers who need an exotic keycode get a tracing warning and the entry
/// is dropped.
fn via_to_hid_usage(raw: &str) -> Option<u8> {
    let upper = raw.trim().to_ascii_uppercase();
    let suffix = upper.strip_prefix("KC_")?;

    // Single-letter alphabet
    if suffix.len() == 1 {
        let b = suffix.as_bytes()[0];
        if b.is_ascii_alphabetic() {
            return Some(0x04 + (b.to_ascii_uppercase() - b'A'));
        }
    }

    // F1..F24
    if let Some(rest) = suffix.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=12).contains(&n) {
                return Some(0x3A + (n - 1));
            }
            if (13..=24).contains(&n) {
                return Some(0x68 + (n - 13));
            }
        }
    }

    // 1..0 row (with HID's "0 is 0x27" quirk)
    if suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_digit() {
        let d = suffix.as_bytes()[0] - b'0';
        if d == 0 {
            return Some(0x27);
        }
        return Some(0x1E + (d - 1));
    }

    Some(match suffix {
        "ESC" => 0x29,
        "TAB" => 0x2B,
        "SPC" | "SPACE" => 0x2C,
        "ENT" | "ENTER" => 0x28,
        "BSPC" => 0x2A,
        "DEL" | "DELETE" => 0x4C,
        "CAPS" | "CAPSLOCK" => 0x39,
        "LCTL" => 0xE0,
        "LSFT" => 0xE1,
        "LALT" => 0xE2,
        "LGUI" | "LCMD" | "LWIN" => 0xE3,
        "RCTL" => 0xE4,
        "RSFT" => 0xE5,
        "RALT" => 0xE6,
        "RGUI" | "RCMD" | "RWIN" => 0xE7,
        "LEFT" => 0x50,
        "RGHT" | "RIGHT" => 0x4F,
        "UP" => 0x52,
        "DOWN" => 0x51,
        "HOME" => 0x4A,
        "END" => 0x4D,
        "PGUP" => 0x4B,
        "PGDN" => 0x4E,
        "MINS" | "MINUS" => 0x2D,
        "EQL" | "EQUAL" => 0x2E,
        "LBRC" => 0x2F,
        "RBRC" => 0x30,
        "BSLS" => 0x31,
        "SCLN" => 0x33,
        "QUOT" => 0x34,
        "GRV" => 0x35,
        "COMM" => 0x36,
        "DOT" => 0x37,
        "SLSH" => 0x38,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_software(engine: &str, config: &str) -> ViaProfile {
        use hhkb_core::via::{ProfileMeta, RoninExtension, SoftwareConfig};
        ViaProfile {
            name: "p".into(),
            vendor_id: "0x0".into(),
            product_id: "0x0".into(),
            matrix: None,
            layouts: None,
            layers: vec![],
            lighting: None,
            keycodes: vec![],
            ronin: Some(RoninExtension {
                version: "1".into(),
                profile: ProfileMeta {
                    id: uuid::Uuid::new_v4(),
                    name: "p".into(),
                    icon: None,
                    tags: vec![],
                },
                hardware: None,
                software: Some(SoftwareConfig {
                    engine: engine.to_string(),
                    engine_version: None,
                    config: config.to_string(),
                }),
            }),
        }
    }

    #[test]
    fn id_and_capabilities_are_stable() {
        let b = HidutilBackend::new();
        assert_eq!(b.id(), BackendId::Hidutil);
        assert_eq!(b.id().as_str(), "hidutil");

        let caps = b.capabilities();
        assert!(caps.per_key_remap);
        assert_eq!(caps.layers, 1);
        assert_eq!(caps.tap_hold, TapHoldQuality::None);
        assert!(caps.persistent);
        assert!(!caps.macros);
    }

    #[test]
    fn via_to_hid_usage_covers_common_remaps() {
        // The Caps↔Esc swap is the canonical hidutil example; if this breaks
        // every user's first-attempt profile fails.
        assert_eq!(via_to_hid_usage("KC_CAPS"), Some(0x39));
        assert_eq!(via_to_hid_usage("KC_ESC"), Some(0x29));

        // Modifiers map to page-7 0xE0..0xE7 — verify both halves.
        assert_eq!(via_to_hid_usage("KC_LCTL"), Some(0xE0));
        assert_eq!(via_to_hid_usage("KC_RGUI"), Some(0xE7));

        // 0 sits at 0x27, not 0x1E + 9 — common spec gotcha.
        assert_eq!(via_to_hid_usage("KC_0"), Some(0x27));
        assert_eq!(via_to_hid_usage("KC_1"), Some(0x1E));

        assert_eq!(via_to_hid_usage("KC_F1"), Some(0x3A));
        assert_eq!(via_to_hid_usage("KC_F12"), Some(0x45));
    }

    #[test]
    fn via_to_hid_usage_rejects_garbage() {
        assert_eq!(via_to_hid_usage(""), None);
        assert_eq!(via_to_hid_usage("KC_NOPE"), None);
        assert_eq!(via_to_hid_usage("FOO"), None);
    }

    #[test]
    fn build_payload_emits_usage_page_7_prefix() {
        let payload = build_property_payload(&[HidutilMapping {
            src: "KC_CAPS".into(),
            dst: "KC_ESC".into(),
        }]);
        // 0x700000000 | 0x39 = 30064771129
        assert!(payload.contains("\"HIDKeyboardModifierMappingSrc\":30064771129"));
        // 0x700000000 | 0x29 = 30064771113
        assert!(payload.contains("\"HIDKeyboardModifierMappingDst\":30064771113"));
        assert!(payload.starts_with("{\"UserKeyMapping\":["));
    }

    #[test]
    fn build_payload_drops_unmappable_entries() {
        let payload = build_property_payload(&[
            HidutilMapping {
                src: "KC_CAPS".into(),
                dst: "KC_ESC".into(),
            },
            HidutilMapping {
                src: "KC_GARBAGE".into(),
                dst: "KC_A".into(),
            },
        ]);
        // Only one entry should appear — the broken one is dropped.
        let entry_count = payload.matches("HIDKeyboardModifierMappingSrc").count();
        assert_eq!(entry_count, 1, "expected 1 entry, got payload: {payload}");
    }

    #[test]
    fn parse_config_rejects_wrong_engine() {
        let p = with_software("kanata", "{}");
        let err = parse_config(&p).unwrap_err();
        assert!(matches!(err, BackendError::ProfileRejected(_)));
    }

    #[test]
    fn parse_config_rejects_missing_software() {
        let p = ViaProfile {
            name: "p".into(),
            vendor_id: "0x0".into(),
            product_id: "0x0".into(),
            matrix: None,
            layouts: None,
            layers: vec![],
            lighting: None,
            keycodes: vec![],
            ronin: None,
        };
        let err = parse_config(&p).unwrap_err();
        assert!(matches!(err, BackendError::ProfileRejected(_)));
    }

    #[test]
    fn parse_config_accepts_valid_hidutil_profile() {
        let p = with_software(
            "hidutil",
            r#"{"mappings":[{"src":"KC_CAPS","dst":"KC_ESC"}]}"#,
        );
        let cfg = parse_config(&p).expect("valid config");
        assert_eq!(cfg.mappings.len(), 1);
        assert_eq!(cfg.mappings[0].src, "KC_CAPS");
    }

    #[test]
    fn parse_config_rejects_malformed_json() {
        let p = with_software("hidutil", "not-json-at-all");
        let err = parse_config(&p).unwrap_err();
        assert!(matches!(err, BackendError::ProfileRejected(_)));
    }
}
