use gpui::{Background, linear_color_stop, linear_gradient, rgb};

// Blue-black metal, smoked glass, and phosphor readouts across the player.
pub const BG: u32 = 0x12181a;
pub const GLASS: u32 = 0x0b1211;
pub const SIDE: u32 = 0x222630;
pub const PANEL: u32 = 0x2c303b;
pub const STATUS: u32 = 0x202932;
pub const TITLE: u32 = 0x404654;
pub const EDGE: u32 = 0x515662;
pub const HOVER: u32 = 0x303d3e;
pub const SELECTED: u32 = 0x3e4a55;
pub const DIM: u32 = 0xa4afa4;
pub const TEXT: u32 = 0xdadcd2;
pub const ACCENT: u32 = 0xf1c464;
pub const PHOSPHOR: u32 = 0xb1e6a3;
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
        finish(0xf4cf87, 0xbc9155)
    } else {
        finish(0xd6dbdd, 0x969ca6)
    }
}
