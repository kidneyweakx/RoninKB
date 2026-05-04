//! Profile binding shapes used by the PoC. Trimmed to what the Caps→Ctrl/Esc
//! demo needs; M2 will widen this and translate the real RoninKB profile.

use crate::keycode::HidUsage;

/// One PoC binding cell: the source HID usage and what it should resolve to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PocBinding {
    /// Pass through unchanged.
    Passthrough(HidUsage),
    /// Tap-hold: tap=`tap`, hold=`hold`.
    HoldTap {
        src: HidUsage,
        tap: HidUsage,
        hold: HidUsage,
    },
}

/// The single PoC layout: Caps Lock → tap=Esc, hold=LCtrl. Everything else
/// flows through the engine's passthrough set without going through keyberon.
pub fn caps_ctrl_esc_layout() -> Vec<PocBinding> {
    vec![PocBinding::HoldTap {
        src: HidUsage::CAPS_LOCK,
        tap: HidUsage::ESC,
        hold: HidUsage::LEFT_CTRL,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_layout_has_caps_holdtap() {
        let layout = caps_ctrl_esc_layout();
        assert_eq!(layout.len(), 1);
        match layout[0] {
            PocBinding::HoldTap { src, tap, hold } => {
                assert_eq!(src, HidUsage::CAPS_LOCK);
                assert_eq!(tap, HidUsage::ESC);
                assert_eq!(hold, HidUsage::LEFT_CTRL);
            }
            _ => panic!("expected HoldTap binding"),
        }
    }
}
