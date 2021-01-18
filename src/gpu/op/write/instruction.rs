use crate::math::Mat4;

// Commands specified by the client become Instructions used by `WriteOp`
pub(super) enum Instruction {
    TextureDescriptors(usize),
    TextureWrite(Mat4),
}
