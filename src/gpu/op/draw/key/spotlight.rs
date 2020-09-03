use {super::BIT, crate::gpu::op::draw::SpotlightCommand, std::ops::Range};

/// Holds the details of a normalized quanitized spotlight. Regular user-supplied spotlights are first
/// normalized (range + radius == 1.0) and then quantized to 24 bits. This allows spotlight meshes to be
/// cached and preserves enough information about a spotlight to generate a _very_ close version of what
/// the user specifies by using a model transformation matrix.
///
/// Note that this type breaks range out into values as opposed to using `Range<u8>` because that type
/// doesn't implement `Ord` and we really don't care what order these are stored in, we just want to order
/// them so we can binary search to find them.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Spotlight(u32);

impl Spotlight {
    /// Returns the normalized and quantized spotlight and the scale needed to undo the normalization.
    pub fn quantize(cmd: &SpotlightCommand) -> (Self, f32) {
        let radius_end = cmd.cone_radius + cmd.penumbra_radius;
        let scale = (cmd.top_radius * cmd.top_radius
            + radius_end * radius_end
            + cmd.range.start * cmd.range.start
            + cmd.range.end * cmd.range.end)
            .sqrt();
        let recip = BIT / scale;
        let radius_start = (cmd.top_radius * recip) as u32;
        let radius_end = (radius_end * recip) as u32;
        let range_end = (cmd.range.end * recip) as u32;
        let range_start = (cmd.range.start * recip) as u32;
        let key = radius_start | radius_end << 8 | range_start << 16 | range_end << 24;

        (Self(key), scale)
    }

    pub fn radius(&self) -> Range<u8> {
        let start = (self.0 & 0xff) as _;
        let end = (self.0 >> 8 & 0xff) as _;

        start..end
    }

    pub fn range(&self) -> Range<u8> {
        let start = (self.0 >> 16 & 0xff) as _;
        let end = (self.0 >> 24 & 0xff) as _;

        start..end
    }
}
