use {
    crate::gpu::op::{
        draw::{command::SpotlightCommand, geom::SPOTLIGHT_STRIDE},
        Stride,
    },
    std::ops::Range,
};

/// Holds the details of a normalized quantized spotlight.
///
/// **_NOTE:_** Regular user-supplied spotlights are first normalized (`range + radius == 1.0`) and
/// then quantized to 32 bits. This allows light volume meshes to be cached and preserves enough
/// information about a light to generate a _very_ close version of what the user specified,
/// using a model transformation matrix.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Spotlight(u32);

impl Spotlight {
    /// Returns the normalized and quantized spotlight and the scale needed to undo the
    /// normalization.
    pub fn quantize(cmd: &SpotlightCommand) -> (Self, f32) {
        let scale = cmd.radius + cmd.range.end;
        let recip = ((1 << 8) - 1) as f32 / scale;
        let radius_start = (cmd.radius_start * recip) as u32;
        let radius_end = (cmd.radius * recip) as u32;
        let range_start = (cmd.range.start * recip) as u32;
        let range_end = (cmd.range.end * recip) as u32;
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

impl Stride for Spotlight {
    fn stride() -> u64 {
        SPOTLIGHT_STRIDE as _
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

    fn assert_spotlight_cmd(cmd: SpotlightCommand) {
        let (key, scale) = Spotlight::quantize(&cmd);

        assert_abs_eps(scale, key.radius().start, cmd.radius_start);
        assert_abs_eps(scale, key.radius().end, cmd.radius);
        assert_abs_eps(scale, key.range().start, cmd.range.start);
        assert_abs_eps(scale, key.range().end, cmd.range.end);
    }

    #[test]
    fn spotlight_works() {
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 0.0,
            radius: 1.0,
            range: 0.0..1.0,
            ..Default::default()
        });
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 0.0,
            radius: 1.0,
            range: 0.0..10.0,
            ..Default::default()
        });
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 0.0,
            radius: 10.0,
            range: 0.0..10.0,
            ..Default::default()
        });
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 0.0,
            radius: 10.0,
            range: 0.0..1.0,
            ..Default::default()
        });
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 0.5,
            radius: 1.0,
            range: 0.5..1.0,
            ..Default::default()
        });
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 1.0,
            radius: 1.0,
            range: 1.0..10.0,
            ..Default::default()
        });
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 4.0,
            radius: 10.0,
            range: 6.0..10.0,
            ..Default::default()
        });
        assert_spotlight_cmd(SpotlightCommand {
            radius_start: 0.0,
            radius: 10.0,
            range: 0.5..1.0,
            ..Default::default()
        });
    }
}
