//! VIA JSON profile format with RoninKB superset extensions.
//!
//! RoninKB uses VIA's JSON format as the primary on-disk representation for
//! keyboard profiles. RoninKB-specific data lives under the `_roninKB` key,
//! which VIA ignores. This means:
//!
//! - Any VIA JSON can be opened in RoninKB.
//! - RoninKB JSON can be opened in VIA (the `_roninKB` key is silently dropped
//!   by VIA since it is not part of VIA's known schema).
//!
//! # Limitation
//!
//! Because this module uses strongly-typed structs rather than a free-form
//! `serde_json::Value`, unknown VIA fields (for example `customKeycodes`) are
//! **not** preserved across a parse/serialize round-trip. Fields we explicitly
//! model are retained; everything else is dropped. If lossless passthrough of
//! unknown keys becomes a requirement, switch to a `#[serde(flatten)]`
//! `HashMap<String, serde_json::Value>` catch-all.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Top-level VIA profile format (RoninKB superset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViaProfile {
    pub name: String,

    #[serde(rename = "vendorId")]
    pub vendor_id: String,

    #[serde(rename = "productId")]
    pub product_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<Matrix>,

    /// VIA layout data — passed through as opaque JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layouts: Option<serde_json::Value>,

    /// VIA keycode layers, e.g. `[["KC_ESC", "KC_1", ...], [...]]`.
    #[serde(default)]
    pub layers: Vec<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keycodes: Vec<String>,

    /// RoninKB extension — `None` for pure VIA files.
    #[serde(rename = "_roninKB", skip_serializing_if = "Option::is_none", default)]
    pub ronin: Option<RoninExtension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matrix {
    pub rows: u8,
    pub cols: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoninExtension {
    pub version: String,
    pub profile: ProfileMeta,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hardware: Option<HardwareConfig>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub software: Option<SoftwareConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub id: Uuid,
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub icon: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareConfig {
    pub keyboard_mode: u8,
    pub raw_layers: RawLayers,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLayers {
    pub base: Vec<u8>,
    // `fn` is a Rust keyword, so the field is raw-identified.
    // Serde serializes it as `fn` thanks to the raw identifier handling.
    pub r#fn: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareConfig {
    /// Engine identifier, e.g. `"kanata"`.
    pub engine: String,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub engine_version: Option<String>,

    /// Raw configuration content (e.g. the contents of a `.kbd` file).
    pub config: String,
}

impl ViaProfile {
    /// Returns `true` if this profile carries RoninKB extensions.
    pub fn has_ronin_extension(&self) -> bool {
        self.ronin.is_some()
    }

    /// Returns a clone of this profile with the RoninKB extension stripped,
    /// yielding a pure VIA-compatible profile.
    pub fn to_via_only(&self) -> ViaProfile {
        let mut cloned = self.clone();
        cloned.ronin = None;
        cloned
    }

    /// Parse a `ViaProfile` from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize a `ViaProfile` to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ronin_profile() -> ViaProfile {
        ViaProfile {
            name: "HHKB Professional Hybrid".to_string(),
            vendor_id: "0x04FE".to_string(),
            product_id: "0x0021".to_string(),
            matrix: Some(Matrix { rows: 8, cols: 8 }),
            layouts: None,
            layers: vec![
                vec!["KC_ESC".to_string(), "KC_1".to_string()],
                vec!["KC_F1".to_string(), "KC_F2".to_string()],
            ],
            lighting: None,
            keycodes: vec![],
            ronin: Some(RoninExtension {
                version: "1.0".to_string(),
                profile: ProfileMeta {
                    id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
                    name: "Daily Driver".to_string(),
                    icon: Some("keyboard".to_string()),
                    tags: vec!["work".to_string(), "coding".to_string()],
                },
                hardware: Some(HardwareConfig {
                    keyboard_mode: 0,
                    raw_layers: RawLayers {
                        base: vec![0x29, 0x1E, 0x1F],
                        r#fn: vec![0x3A, 0x3B, 0x3C],
                    },
                }),
                software: Some(SoftwareConfig {
                    engine: "kanata".to_string(),
                    engine_version: Some("1.7.0".to_string()),
                    config: "(defsrc a b c)\n(deflayer base a b c)".to_string(),
                }),
            }),
        }
    }

    #[test]
    fn test_parse_pure_via_json() {
        let json = r#"{
            "name": "HHKB Professional Hybrid",
            "vendorId": "0x04FE",
            "productId": "0x0021",
            "layers": [["KC_ESC", "KC_1"], ["KC_F1", "KC_F2"]]
        }"#;

        let profile = ViaProfile::from_json(json).expect("should parse pure VIA JSON");

        assert_eq!(profile.name, "HHKB Professional Hybrid");
        assert_eq!(profile.vendor_id, "0x04FE");
        assert_eq!(profile.product_id, "0x0021");
        assert_eq!(profile.layers.len(), 2);
        assert_eq!(profile.layers[0], vec!["KC_ESC", "KC_1"]);
        assert_eq!(profile.layers[1], vec!["KC_F1", "KC_F2"]);
        assert!(profile.ronin.is_none());
        assert!(!profile.has_ronin_extension());
    }

    #[test]
    fn test_parse_ronin_extended() {
        let json = r#"{
            "name": "HHKB Professional Hybrid",
            "vendorId": "0x04FE",
            "productId": "0x0021",
            "matrix": { "rows": 8, "cols": 8 },
            "layers": [["KC_ESC", "KC_1"]],
            "_roninKB": {
                "version": "1.0",
                "profile": {
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "name": "Daily Driver",
                    "icon": "keyboard",
                    "tags": ["work", "coding"]
                },
                "hardware": {
                    "keyboard_mode": 0,
                    "raw_layers": {
                        "base": [41, 30, 31],
                        "fn": [58, 59, 60]
                    }
                },
                "software": {
                    "engine": "kanata",
                    "engine_version": "1.7.0",
                    "config": "(defsrc a)\n(deflayer base a)"
                }
            }
        }"#;

        let profile = ViaProfile::from_json(json).expect("should parse RoninKB JSON");

        assert!(profile.has_ronin_extension());
        assert_eq!(profile.name, "HHKB Professional Hybrid");
        assert_eq!(profile.matrix, Some(Matrix { rows: 8, cols: 8 }));

        let ronin = profile.ronin.as_ref().unwrap();
        assert_eq!(ronin.version, "1.0");
        assert_eq!(ronin.profile.name, "Daily Driver");
        assert_eq!(
            ronin.profile.id,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        assert_eq!(ronin.profile.icon.as_deref(), Some("keyboard"));
        assert_eq!(ronin.profile.tags, vec!["work", "coding"]);

        let hw = ronin.hardware.as_ref().expect("hardware present");
        assert_eq!(hw.keyboard_mode, 0);
        assert_eq!(hw.raw_layers.base, vec![41, 30, 31]);
        assert_eq!(hw.raw_layers.r#fn, vec![58, 59, 60]);

        let sw = ronin.software.as_ref().expect("software present");
        assert_eq!(sw.engine, "kanata");
        assert_eq!(sw.engine_version.as_deref(), Some("1.7.0"));
        assert_eq!(sw.config, "(defsrc a)\n(deflayer base a)");
    }

    #[test]
    fn test_roundtrip_serialization() {
        let original = sample_ronin_profile();
        let json = original.to_json().expect("serialize");
        let decoded = ViaProfile::from_json(&json).expect("deserialize");

        assert_eq!(decoded.name, original.name);
        assert_eq!(decoded.vendor_id, original.vendor_id);
        assert_eq!(decoded.product_id, original.product_id);
        assert_eq!(decoded.matrix, original.matrix);
        assert_eq!(decoded.layers, original.layers);
        assert!(decoded.has_ronin_extension());

        let orig_ronin = original.ronin.as_ref().unwrap();
        let dec_ronin = decoded.ronin.as_ref().unwrap();
        assert_eq!(dec_ronin.version, orig_ronin.version);
        assert_eq!(dec_ronin.profile.id, orig_ronin.profile.id);
        assert_eq!(dec_ronin.profile.name, orig_ronin.profile.name);
        assert_eq!(dec_ronin.profile.icon, orig_ronin.profile.icon);
        assert_eq!(dec_ronin.profile.tags, orig_ronin.profile.tags);

        let orig_hw = orig_ronin.hardware.as_ref().unwrap();
        let dec_hw = dec_ronin.hardware.as_ref().unwrap();
        assert_eq!(dec_hw.keyboard_mode, orig_hw.keyboard_mode);
        assert_eq!(dec_hw.raw_layers, orig_hw.raw_layers);

        let orig_sw = orig_ronin.software.as_ref().unwrap();
        let dec_sw = dec_ronin.software.as_ref().unwrap();
        assert_eq!(dec_sw.engine, orig_sw.engine);
        assert_eq!(dec_sw.engine_version, orig_sw.engine_version);
        assert_eq!(dec_sw.config, orig_sw.config);
    }

    #[test]
    fn test_to_via_only_strips_extension() {
        let original = sample_ronin_profile();
        assert!(original.has_ronin_extension());

        let via_only = original.to_via_only();

        assert!(!via_only.has_ronin_extension());
        assert!(via_only.ronin.is_none());
        assert_eq!(via_only.name, original.name);
        assert_eq!(via_only.vendor_id, original.vendor_id);
        assert_eq!(via_only.product_id, original.product_id);
        assert_eq!(via_only.matrix, original.matrix);
        assert_eq!(via_only.layers, original.layers);

        // Original should still have its extension.
        assert!(original.has_ronin_extension());
    }

    #[test]
    fn test_via_only_serialization_no_ronin_key() {
        let profile = sample_ronin_profile().to_via_only();
        let json = profile.to_json().expect("serialize");

        assert!(
            !json.contains("_roninKB"),
            "to_via_only() output must not contain `_roninKB`, got: {json}"
        );
        // Sanity: the VIA fields are still there.
        assert!(json.contains("\"vendorId\""));
        assert!(json.contains("\"productId\""));
    }

    #[test]
    fn test_unknown_fields_preserved() {
        // NOTE: This is a documentation test for a known limitation.
        // Because ViaProfile is strongly-typed (no `#[serde(flatten)]`
        // catch-all), unknown VIA fields such as `customKeycodes` are
        // silently dropped on parse and therefore NOT preserved on
        // re-serialization. If lossless round-tripping becomes required,
        // add a flattened `HashMap<String, serde_json::Value>` extras field.
        let json = r#"{
            "name": "X",
            "vendorId": "0x0000",
            "productId": "0x0000",
            "customKeycodes": [{"name": "MY_MACRO", "title": "macro", "shortName": "MCR"}]
        }"#;

        let profile = ViaProfile::from_json(json).expect("parse succeeds");
        let reserialized = profile.to_json().expect("serialize");

        // Asserting the current (lossy) behavior so the limitation is
        // detected if it ever changes.
        assert!(
            !reserialized.contains("customKeycodes"),
            "strongly-typed ViaProfile drops unknown fields — update this test \
             if flatten-based passthrough is added"
        );
    }

    #[test]
    fn test_empty_layers() {
        let json = r#"{
            "name": "Minimal",
            "vendorId": "0x04FE",
            "productId": "0x0021"
        }"#;

        let profile = ViaProfile::from_json(json).expect("parse");
        assert!(profile.layers.is_empty());
        assert!(profile.keycodes.is_empty());
        assert!(profile.matrix.is_none());
        assert!(profile.layouts.is_none());
        assert!(profile.lighting.is_none());
        assert!(profile.ronin.is_none());
    }

    #[test]
    fn test_hardware_config_raw_layers() {
        let raw = RawLayers {
            base: vec![1, 2, 3, 4],
            r#fn: vec![10, 20, 30, 40],
        };

        let json = serde_json::to_string(&raw).expect("serialize raw layers");
        // The JSON key must be the literal `fn`, not `r#fn`.
        assert!(
            json.contains("\"fn\""),
            "expected JSON key `fn`, got: {json}"
        );
        assert!(!json.contains("r#fn"));
        assert!(json.contains("\"base\""));

        let decoded: RawLayers = serde_json::from_str(&json).expect("deserialize raw layers");
        assert_eq!(decoded, raw);

        // And the same through a full HardwareConfig.
        let hw_json = r#"{
            "keyboard_mode": 2,
            "raw_layers": {
                "base": [41, 30, 31, 32],
                "fn": [58, 59, 60, 61]
            }
        }"#;
        let hw: HardwareConfig = serde_json::from_str(hw_json).expect("parse hardware");
        assert_eq!(hw.keyboard_mode, 2);
        assert_eq!(hw.raw_layers.base, vec![41, 30, 31, 32]);
        assert_eq!(hw.raw_layers.r#fn, vec![58, 59, 60, 61]);

        let reserialized = serde_json::to_string(&hw).expect("serialize hardware");
        assert!(reserialized.contains("\"fn\""));
        assert!(!reserialized.contains("r#fn"));
    }
}
