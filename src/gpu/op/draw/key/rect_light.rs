use {
    super::{Stride, BIT},
    crate::{
        gpu::op::draw::{command::RectLightCommand, geom::RECT_LIGHT_STRIDE},
        math::{Coord8, Extent},
    },
};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct RectLight(u32);

impl RectLight {
    /// Returns the normalized and quantized rectangular light and the scale needed to undo the normalization.
    pub fn quantize(cmd: &RectLightCommand) -> Self {
        let scale = (cmd.dims.x * cmd.dims.x
            + cmd.dims.y * cmd.dims.y
            + cmd.radius * cmd.radius
            + cmd.range * cmd.range)
            .sqrt();
        let recip = BIT / scale;
        let dims: Extent = (cmd.dims * recip).into();
        let radius = (cmd.radius * recip) as u32;
        let range = (cmd.range * recip) as u32;
        let key = range | radius << 8 | dims.x << 16 | dims.y << 24;

        Self(key)
    }

    pub fn dims(&self) -> Coord8 {
        let x = (self.0 >> 16 & 0xff) as _;
        let y = (self.0 >> 24 & 0xff) as _;

        Coord8 { x, y }
    }

    pub fn radius(&self) -> u8 {
        (self.0 >> 8 & 0xff) as _
    }

    pub fn range(&self) -> u8 {
        (self.0 & 0xff) as _
    }
}

impl Stride for RectLight {
    fn stride() -> u64 {
        RECT_LIGHT_STRIDE as _
    }
}
