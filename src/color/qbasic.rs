use super::{Color, BLACK};

const QBASIC_COLORS: [Color; 16] = [
    BLACK,
    Color::rgb(0, 0, 0xa8),
    Color::rgb(0, 0xa8, 0),
    Color::rgb(0, 0xa8, 0xa8),
    Color::rgb(0xa8, 0, 0),
    Color::rgb(0xa8, 0, 0xa8),
    Color::rgb(0xa8, 0x54, 0),
    Color::rgb(0xa8, 0xa8, 0xa8),
    Color::rgb(0x54, 0x54, 0x54),
    Color::rgb(0x54, 0x54, 0xfc),
    Color::rgb(0x54, 0xfc, 0x54),
    Color::rgb(0x54, 0xfc, 0xfc),
    Color::rgb(0xfc, 0x54, 0x54),
    Color::rgb(0xfc, 0x54, 0xfc),
    Color::rgb(0xfc, 0xfc, 0x54),
    Color::rgb(0xfc, 0xfc, 0xfc),
];

/// Sets the screen display colors.
///
/// idx: A number that sets the foreground screen color
pub const fn color(idx: usize) -> Color {
    // & 15 to make sure we don't index out of this
    QBASIC_COLORS[idx & 15]
}
