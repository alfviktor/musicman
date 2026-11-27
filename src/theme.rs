use gpui::{Background, linear_color_stop, linear_gradient, rgb};

// Smoked blue glass and brushed gunmetal. Cyan is the display color; amber
// identifies a transport state or control that is engaged.
pub const BG: u32 = 0x0c1824;
pub const GLASS: u32 = 0x08131c;
pub const SIDE: u32 = 0x132334;
pub const PANEL: u32 = 0x213448;
pub const STATUS: u32 = 0x162838;
pub const TITLE: u32 = 0x31485c;
pub const EDGE: u32 = 0x385066;
pub const HOVER: u32 = 0x1c3547;
pub const SELECTED: u32 = 0x254559;
pub const DIM: u32 = 0x9daebc;
pub const TEXT: u32 = 0xd9e5eb;
pub const ACCENT: u32 = 0xf2c781;
pub const PHOSPHOR: u32 = 0x91d7f3;
pub const RAIL: u32 = 0x526d82;
pub const UNLIT: u32 = 0x1a3444;
pub const KEY_HIGHLIGHT: u32 = 0xd3dfe4;
pub const KEY_SHADOW: u32 = 0x869ba9;
pub const MONO: &str = "Menlo";

pub fn finish(top: u32, bottom: u32) -> Background {
    linear_gradient(
        180.,
        linear_color_stop(rgb(top), 0.),
        linear_color_stop(rgb(bottom), 1.),
    )
}
pub fn metal() -> Background {
    finish(TITLE, SIDE)
}
pub fn keycap(lit: bool) -> Background {
    if lit {
        finish(ACCENT, 0xc19b61)
    } else {
        finish(KEY_HIGHLIGHT, KEY_SHADOW)
    }
}
