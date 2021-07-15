use {
    super::dyn_atlas::DynamicAtlas,
    crate::{
        color::AlphaColor,
        gpu::{
            op::{DataCopyInstruction, DataTransferInstruction, DataWriteInstruction},
            pool::Lease,
            Data,
        },
        math::Mat4,
    },
    archery::SharedPointerKind,
    gfx_hal::VertexCount,
    std::ops::Range,
};

#[non_exhaustive]
pub(super) enum Instruction<'a, P>
where
    P: SharedPointerKind,
{
    BitmapBegin,
    BitmapBindDescriptorSet(usize),
    BitmapColors(AlphaColor, AlphaColor),
    BitmapTransform(Mat4),
    DataTransfer(DataTransferInstruction<'a>),
    TextBegin,
    TextRender(Range<VertexCount>),
    VectorBegin,
    VectorBindDescriptorSet(usize),
    VectorColor(AlphaColor),
    VectorCopyGlyphs(&'a mut DynamicAtlas<P>),
    VectorTransform(Mat4),
    VertexBind(VertexBindInstruction<'a, P>),
    VertexCopy(DataCopyInstruction<'a>),
    VertexWrite(DataWriteInstruction<'a>),
}

pub struct VertexBindInstruction<'a, P>
where
    P: SharedPointerKind,
{
    pub buf: &'a Lease<Data, P>,
    pub buf_len: u64,
}
