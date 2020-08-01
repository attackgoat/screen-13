use {
    crate::gpu::op::draw::LineCommand,
    gfx_hal::image::PackedColor,
    std::{collections::hash_map::DefaultHasher, hash::Hasher},
};

#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub struct LineKey(u64);

impl LineKey {
    /// Returns the hashed line. This process differs from quantization of lights in that it does not need to
    /// be later reproduced as a line; it only needs to assist with searching the cache for identical lines.
    pub fn hash(cmd: &LineCommand) -> Self {
        let mut hasher = DefaultHasher::default();

        for idx in 0..cmd.vertices.len() {
            hasher.write_u32(PackedColor::from(cmd.vertices[idx].color).0);
            hasher.write_u32(cmd.vertices[idx].pos.x().to_bits());
            hasher.write_u32(cmd.vertices[idx].pos.y().to_bits());
            hasher.write_u32(cmd.vertices[idx].pos.z().to_bits());
        }

        Self(hasher.finish())
    }
}
