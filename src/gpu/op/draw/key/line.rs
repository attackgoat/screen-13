use {
    super::Stride,
    crate::gpu::op::draw::{command::LineCommand, geom::LINE_STRIDE},
    gfx_hal::image::PackedColor,
    std::{collections::hash_map::DefaultHasher, hash::Hasher},
};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Line(u64);

impl Line {
    /// Returns the hashed line. This process differs from quantization of lights in that it does not need to
    /// be later reproduced as a line; it only needs to assist with searching the cache for identical lines.
    pub fn hash(cmd: &LineCommand) -> Self {
        let mut hasher = DefaultHasher::default();

        for idx in 0..cmd.vertices.len() {
            hasher.write_u32(PackedColor::from(cmd.vertices[idx].color).0);
            hasher.write_u32(cmd.vertices[idx].pos.x.to_bits());
            hasher.write_u32(cmd.vertices[idx].pos.y.to_bits());
            hasher.write_u32(cmd.vertices[idx].pos.z.to_bits());
        }

        Self(hasher.finish())
    }
}

impl Stride for Line {
    fn stride() -> u64 {
        LINE_STRIDE as _
    }
}
