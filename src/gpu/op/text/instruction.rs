use {
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
    BitmapGlyphBegin,
    BitmapGlyphBind(BitmapBindInstruction<'a, P>),
    BitmapGlyphColor(AlphaColor),
    BitmapGlyphTransform(Mat4),
    BitmapOutlineBegin,
    BitmapOutlineBind(BitmapBindInstruction<'a, P>),
    BitmapOutlineColors(AlphaColor, AlphaColor),
    BitmapOutlineTransform(Mat4),
    DataTransfer(DataTransferInstruction<'a>),
    RenderBegin,
    RenderText(Range<VertexCount>),
    ScalableBegin,
    ScalableBind(ScalableBindInstruction<'a, P>),
    ScalableColor(AlphaColor),
    ScalableTransform(Mat4),
    VertexCopy(DataCopyInstruction<'a>),
    VertexWrite(DataWriteInstruction<'a>),
}

pub struct BitmapBindInstruction<'a, P>
where
    P: SharedPointerKind,
{
    pub buf: &'a Lease<Data, P>,
    pub buf_len: u64,
    pub desc_set: usize,
}

pub struct ScalableBindInstruction<'a, P>
where
    P: SharedPointerKind,
{
    pub buf: &'a Lease<Data, P>,
    pub buf_len: u64,
}
