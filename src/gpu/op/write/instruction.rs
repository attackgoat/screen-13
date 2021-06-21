use crate::math::{Mat4, RectF};

// Commands specified by the client become Instructions used by `WriteOp`
#[non_exhaustive]
pub(super) enum Instruction {
    TextureBindDescriptorSet(usize),
    TextureWrite(RectF, Mat4),
}
