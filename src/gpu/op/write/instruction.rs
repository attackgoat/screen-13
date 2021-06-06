use crate::math::Mat4;

// Commands specified by the client become Instructions used by `WriteOp`
#[non_exhaustive]
pub(super) enum Instruction {
    TextureBindDescriptorSet(usize),
    TextureWrite(Mat4),
}
