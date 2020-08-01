use {super::BIT, crate::gpu::op::draw::SpotlightCommand, std::ops::Range};

/// Holds the details of a normalized quanitized spotlight. Regular user-supplied spotlights are first
/// normalized (range + radius == 1.0) and then quantized to 24 bits. This allows spotlight meshes to be
/// cached and preserves enough information about a spotlight to generate a _very_ close version of what
/// the user specifies by using a model transformation matrix.
///
/// Note that this type breaks range out into values as opposed to using `Range<u8>` because that type
/// doesn't implement `Ord` and we really don't care what order these are stored in, we just want to order
/// them so we can binary search to find them.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub struct SpotlightKey {
    radius_end: u8,
    radius_start: u8,
    range_end: u8,
    range_start: u8,
}

impl SpotlightKey {
    pub fn radius(&self) -> Range<u8> {
        Range {
            end: self.radius_end,
            start: self.radius_start,
        }
    }

    pub fn range(&self) -> Range<u8> {
        Range {
            end: self.range_end,
            start: self.range_start,
        }
    }

    /// Returns the normalized and quantized spotlight and the scale needed to undo the normalization.
    pub fn quantize(cmd: &SpotlightCommand) -> (Self, f32) {
        let radius_end = cmd.cone_radius + cmd.penumbra_radius;
        let scale = (cmd.top_radius * cmd.top_radius
            + radius_end * radius_end
            + cmd.range.start * cmd.range.start
            + cmd.range.end * cmd.range.end)
            .sqrt();
        let recip = BIT / scale;
        let key = Self {
            radius_start: (cmd.top_radius * recip) as _,
            radius_end: (radius_end * recip) as _,
            range_end: (cmd.range.end * recip) as _,
            range_start: (cmd.range.start * recip) as _,
        };

        (key, scale)
    }
}
