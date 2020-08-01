use {
    super::BIT,
    crate::{gpu::op::draw::RectLightCommand, math::Coord8},
};

// TODO: Investigate custom Ord implementations for this and SpotlightKey; either store a separate "compare" u32 field or just store all data in one u32 and offer functions to get the individual fields back
#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub struct RectLightKey {
    dims: Coord8,
    radius: u8,
    range: u8,
}

impl RectLightKey {
    pub fn dims(&self) -> Coord8 {
        self.dims
    }

    pub fn radius(&self) -> u8 {
        self.radius
    }

    pub fn range(&self) -> u8 {
        self.range
    }

    /// Returns the normalized and quantized rectangular light and the scale needed to undo the normalization.
    pub fn quantize(cmd: &RectLightCommand) -> (Self, f32) {
        let scale = (cmd.dims.x * cmd.dims.x
            + cmd.dims.y * cmd.dims.y
            + cmd.radius * cmd.radius
            + cmd.range * cmd.range)
            .sqrt();
        let recip = BIT / scale;
        let key = Self {
            dims: (cmd.dims * recip).into(),
            radius: (cmd.radius * recip) as _,
            range: (cmd.range * recip) as _,
        };

        (key, scale)
    }
}
