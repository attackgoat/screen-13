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
///
/// ## Available Colors
///
/// | idx | Name          | Hexadecimal Value |
/// |-----|---------------|-------------------|
/// |  0  | Black         | [`0x000000`](https://miquelvir.github.io/color/#000000)        |
/// |  1  | Dim Blue      | [`0x0000a8`](https://miquelvir.github.io/color/#0000a8)        |
/// |  2  | Dim Green     | [`0x00a800`](https://miquelvir.github.io/color/#00a800)        |
/// |  3  | Dim Cyan      | [`0x00a8a8`](https://miquelvir.github.io/color/#00a8a8)        |
/// |  4  | Dim Red       | [`0xa80000`](https://miquelvir.github.io/color/#a80000)        |
/// |  5  | Dim Purple    | [`0xa800a8`](https://miquelvir.github.io/color/#a800a8)        |
/// |  6  | Brown    | [`0xa85400`](https://miquelvir.github.io/color/#a85400)        |
/// |  7  | Dim White     | [`0xa8a8a8`](https://miquelvir.github.io/color/#a8a8a8)        |
/// |  8  | Gray          | [`0x545454`](https://miquelvir.github.io/color/#545454)        |
/// |  9  | Light Blue   | [`0x5454fc`](https://miquelvir.github.io/color/#5454fc)        |
/// | 10  | Light Green  | [`0x54fc54`](https://miquelvir.github.io/color/#54fc54)        |
/// | 11  | Light Cyan   | [`0x54fcfc`](https://miquelvir.github.io/color/#54fcfc)        |
/// | 12  | Light Red    | [`0xfc5454`](https://miquelvir.github.io/color/#fc5454)        |
/// | 13  | Light Purple | [`0xfc54fc`](https://miquelvir.github.io/color/#fc54fc)        |
/// | 14  | Light Yellow        | [`0xfcfc54`](https://miquelvir.github.io/color/#fcfc54)        |
/// | 15  | Light White  | [`0xfcfcfc`](https://miquelvir.github.io/color/#fcfcfc)        |
pub const fn color(idx: usize) -> Color {
    // & 15 to make sure we don't index out of this
    QBASIC_COLORS[idx & 15]
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn color_idx() {
        assert_eq!(color(0), color(16));
    }
}
