use {
    super::Stride,
    crate::{
        gpu::op::draw::{command::RectLightCommand, geom::RECT_LIGHT_STRIDE},
        math::{Coord8, Extent},
    },
};

/// Holds the details of a normalized quantized rectangular light.
///
/// **_NOTE:_** Regular user-supplied rectangular lights are first normalized
/// (`dims + range + radius == 1.0`) and then quantized to 32 bits. This allows light volume meshes
/// to be cached and preserves enough information about a light to generate a _very_ close version
/// of what the user specified, using a model transformation matrix.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct RectLight(u32);

impl RectLight {
    /// Returns the normalized and quantized rectangular light and the scale needed to undo the
    /// normalization.
    pub fn quantize(cmd: &RectLightCommand) -> (Self, f32) {
        let scale = cmd.dims.x + cmd.dims.y + cmd.radius + cmd.range;
        let recip = ((1 << 8) - 1) as f32 / scale;
        let dims: Extent = (cmd.dims * recip).into();
        let radius = (cmd.radius * recip) as u32;
        let range = (cmd.range * recip) as u32;
        let key = range | radius << 8 | dims.x << 16 | dims.y << 24;

        (Self(key), scale)
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

#[cfg(test)]
mod test {
    use super::*;

    /// This extreme value accounts for 10:1 differences in scale between the light geometry terms.
    /// That is unlikely to be normal and this does not have to be very accurate anyways.
    const EPSILON: f32 = 0.1;

    fn assert_abs_eps(scale: f32, quant: u8, val: f32) {
        assert!((scale * quant as f32 / 255.0 - val).abs() < EPSILON);
    }

    fn assert_spotlight_cmd(cmd: RectLightCommand) {
        let (key, scale) = RectLight::quantize(&cmd);

        assert_abs_eps(scale, key.dims().x, cmd.dims.x);
        assert_abs_eps(scale, key.dims().y, cmd.dims.y);
        assert_abs_eps(scale, key.radius(), cmd.radius);
        assert_abs_eps(scale, key.range(), cmd.range);
    }

    #[test]
    fn spotlight_works() {
        assert_spotlight_cmd(RectLightCommand {
            dims: (0.5, 0.5).into(),
            radius: 1.0,
            range: 1.0,
            ..Default::default()
        });
        assert_spotlight_cmd(RectLightCommand {
            dims: (0.1, 1.0).into(),
            radius: 0.1,
            range: 1.0,
            ..Default::default()
        });
        assert_spotlight_cmd(RectLightCommand {
            dims: (4.2, 5.0).into(),
            radius: 2.8,
            range: 2.15,
            ..Default::default()
        });
    }
}
