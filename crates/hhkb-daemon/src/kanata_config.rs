//! Kanata config validation + VIA-layer fallback generation.
//!
//! This module centralizes:
//! - syntax sanity checks for `.kbd` text used by daemon endpoints
//! - fallback generation from VIA `layers` when `_roninKB.software.config`
//!   is empty but the profile still opts into the `kanata` engine

use hhkb_core::ViaProfile;

/// Validate a kanata `.kbd` config with lightweight structural checks.
///
/// The validator intentionally does not implement full S-expression parsing.
/// It catches the high-signal issues that currently cause immediate startup
/// failures:
/// - empty input
/// - NUL bytes
/// - unbalanced parentheses
/// - missing `(defsrc ...)` and `(defcfg ...)`
pub fn validate_kanata_config(config: &str) -> Result<(), String> {
    if config.trim().is_empty() {
        return Err("kanata config is empty".to_string());
    }
    if config.contains('\0') {
        return Err("kanata config contains NUL byte".to_string());
    }

    let cleaned = strip_line_comments(config);
    ensure_balanced_parentheses(&cleaned)?;

    let has_defsrc = has_top_level_form(&cleaned, "defsrc");
    let has_defcfg = has_top_level_form(&cleaned, "defcfg");
    if !has_defsrc && !has_defcfg {
        return Err("kanata config must include (defsrc ...) or (defcfg ...)".to_string());
    }

    Ok(())
}

/// Resolve the effective config for a profile that uses `software.engine=kanata`.
///
/// Priority:
/// 1. explicit `_roninKB.software.config` when non-empty
/// 2. generated fallback from VIA `layers`
pub fn derive_profile_kanata_config(via: &ViaProfile) -> Result<Option<String>, String> {
    let Some(ronin) = via.ronin.as_ref() else {
        return Ok(None);
    };
    let Some(software) = ronin.software.as_ref() else {
        return Ok(None);
    };
    if !software.engine.eq_ignore_ascii_case("kanata") {
        return Ok(None);
    }

    let cfg = if !software.config.trim().is_empty() {
        software.config.clone()
    } else if let Some(generated) = generate_from_via_layers(&via.layers) {
        generated
    } else {
        return Err(
            "kanata profile has empty config and VIA layers are unavailable for fallback generation"
                .to_string(),
        );
    };

    validate_kanata_config(&cfg)?;
    Ok(Some(cfg))
}

/// Default minimal safe config (used when no profile-level config is available).
pub fn default_minimal_config(slot_count: usize) -> String {
    let count = slot_count.clamp(1, 60);
    let keys = (1..=count)
        .map(|i| format!("k{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let blanks = std::iter::repeat_n("_", count)
        .collect::<Vec<_>>()
        .join(" ");
    format!("(defsrc\n  {keys}\n)\n\n(deflayer base\n  {blanks}\n)\n")
}

/// Generate a kanata config from VIA keycode layers.
///
/// - layer 0 => `base`
/// - layer 1 => `fn`
/// - layer 2+ => `layer{n}`
pub fn generate_from_via_layers(layers: &[Vec<String>]) -> Option<String> {
    if layers.is_empty() {
        return None;
    }

    let width = layers
        .iter()
        .map(|l| l.len())
        .max()
        .unwrap_or(0)
        .clamp(1, 60);
    if width == 0 {
        return None;
    }

    let defsrc_keys = (1..=width)
        .map(|i| format!("k{i}"))
        .collect::<Vec<_>>()
        .join(" ");

    let mut blocks = vec![format!("(defsrc\n  {defsrc_keys}\n)")];

    for (idx, layer) in layers.iter().enumerate() {
        let name = match idx {
            0 => "base".to_string(),
            1 => "fn".to_string(),
            n => format!("layer{n}"),
        };

        let slots = (0..width)
            .map(|pos| {
                layer
                    .get(pos)
                    .and_then(|raw| via_keycode_to_kanata(raw))
                    .unwrap_or_else(|| "_".to_string())
            })
            .collect::<Vec<_>>()
            .join(" ");

        blocks.push(format!("(deflayer {name}\n  {slots}\n)"));
    }

    Some(format!("{}\n", blocks.join("\n\n")))
}

fn strip_line_comments(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            if let Some(idx) = line.find(";;") {
                &line[..idx]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_balanced_parentheses(input: &str) -> Result<(), String> {
    let mut depth = 0usize;
    for ch in input.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Err("kanata config has unmatched ')'".to_string());
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("kanata config has unmatched '('".to_string());
    }
    Ok(())
}

fn has_top_level_form(input: &str, name: &str) -> bool {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'(' {
            i += 1;
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + name.len() <= bytes.len()
            && &input[i..i + name.len()] == name
            && input[i + name.len()..]
                .chars()
                .next()
                .map(|c| c.is_whitespace() || c == ')')
                .unwrap_or(true)
        {
            return true;
        }
    }
    false
}

fn via_keycode_to_kanata(raw: &str) -> Option<String> {
    let upper = raw.trim().to_ascii_uppercase();
    if upper.is_empty() {
        return None;
    }
    if matches!(
        upper.as_str(),
        "KC_NO" | "KC_TRNS" | "KC_TRANSPARENT" | "_______" | "XXXXXXX"
    ) {
        return None;
    }

    if let Some(ch) = upper.strip_prefix("KC_") {
        if ch.len() == 1 && ch.as_bytes()[0].is_ascii_alphabetic() {
            return Some(ch.to_ascii_lowercase());
        }
        if ch.len() == 1 && ch.as_bytes()[0].is_ascii_digit() {
            return Some(ch.to_ascii_lowercase());
        }
    }

    let mapped = match upper.as_str() {
        "KC_ESC" => "esc",
        "KC_TAB" => "tab",
        "KC_SPC" | "KC_SPACE" => "spc",
        "KC_ENT" | "KC_ENTER" => "ret",
        "KC_BSPC" => "bspc",
        "KC_DEL" | "KC_DELETE" => "del",
        "KC_CAPS" | "KC_CAPSLOCK" => "caps",
        "KC_LCTL" => "lctl",
        "KC_RCTL" => "rctl",
        "KC_LSFT" => "lsft",
        "KC_RSFT" => "rsft",
        "KC_LALT" => "lalt",
        "KC_RALT" => "ralt",
        "KC_LGUI" | "KC_LCMD" | "KC_LWIN" => "lmet",
        "KC_RGUI" | "KC_RCMD" | "KC_RWIN" => "rmet",
        "KC_LEFT" => "left",
        "KC_RGHT" | "KC_RIGHT" => "rght",
        "KC_UP" => "up",
        "KC_DOWN" => "down",
        "KC_HOME" => "home",
        "KC_END" => "end",
        "KC_PGUP" => "pgup",
        "KC_PGDN" => "pgdn",
        "KC_VOLU" => "volu",
        "KC_VOLD" => "vold",
        "KC_MUTE" => "mute",
        "KC_MPLY" | "KC_MSTP" => "pp",
        _ => {
            if let Some(rest) = upper.strip_prefix("KC_F") {
                if rest.chars().all(|c| c.is_ascii_digit()) {
                    return Some(format!("f{rest}"));
                }
            }
            if let Some(rest) = upper.strip_prefix("KC_") {
                // Best-effort fallback for less common aliases.
                return Some(rest.to_ascii_lowercase());
            }
            return Some(upper.to_ascii_lowercase());
        }
    };
    Some(mapped.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hhkb_core::via::{ProfileMeta, RoninExtension, SoftwareConfig, ViaProfile};
    use uuid::Uuid;

    fn sample_profile(config: &str, layers: Vec<Vec<String>>) -> ViaProfile {
        ViaProfile {
            name: "p".to_string(),
            vendor_id: "0x0".to_string(),
            product_id: "0x0".to_string(),
            matrix: None,
            layouts: None,
            layers,
            lighting: None,
            keycodes: vec![],
            ronin: Some(RoninExtension {
                version: "1".to_string(),
                profile: ProfileMeta {
                    id: Uuid::new_v4(),
                    name: "p".to_string(),
                    icon: None,
                    tags: vec![],
                },
                hardware: None,
                software: Some(SoftwareConfig {
                    engine: "kanata".to_string(),
                    engine_version: None,
                    config: config.to_string(),
                }),
            }),
        }
    }

    #[test]
    fn validate_rejects_unbalanced_parentheses() {
        let err = validate_kanata_config("(defsrc a b\n(deflayer base a b))(").unwrap_err();
        assert!(err.contains("unmatched"));
    }

    #[test]
    fn validate_requires_defsrc_or_defcfg() {
        let err = validate_kanata_config("(deflayer base a b)").unwrap_err();
        assert!(err.contains("defsrc"));
    }

    #[test]
    fn generate_from_layers_creates_base_and_fn() {
        let cfg = generate_from_via_layers(&[
            vec!["KC_ESC".to_string(), "KC_A".to_string()],
            vec!["KC_F1".to_string(), "KC_TRNS".to_string()],
        ])
        .expect("generated");
        assert!(cfg.contains("(deflayer base"));
        assert!(cfg.contains("(deflayer fn"));
        assert!(cfg.contains("esc a"));
        assert!(cfg.contains("f1 _"));
    }

    #[test]
    fn derive_uses_generated_fallback_when_software_config_empty() {
        let via = sample_profile("", vec![vec!["KC_ESC".to_string()]]);
        let cfg = derive_profile_kanata_config(&via).unwrap().expect("some");
        assert!(cfg.contains("(defsrc"));
        assert!(cfg.contains("(deflayer base"));
    }

    #[test]
    fn derive_rejects_empty_without_layers() {
        let via = sample_profile("", vec![]);
        let err = derive_profile_kanata_config(&via).unwrap_err();
        assert!(err.contains("empty config"));
    }
}
