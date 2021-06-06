use {
    super::command::{PointLightIter, RectLightCommand, SpotlightCommand, SunlightIter},
    crate::{
        gpu::{
            def::CalcVertexAttrsComputeMode,
            model::MeshIter,
            op::{DataCopyInstruction, DataTransferInstruction, DataWriteInstruction},
            pool::Lease,
            Data,
        },
        math::Mat4,
        pak::IndexType,
    },
    archery::SharedPointerKind,
    std::{
        cell::{Ref, RefMut},
        ops::Range,
    },
};

pub(super) struct DataComputeInstruction {
    pub base_idx: u32,
    pub base_vertex: u32,
    pub dispatch: u32,
}

/// Writes the range of cpu-side data to the gpu-side.
pub(super) struct DataWriteRefInstruction<'a, P>
where
    P: SharedPointerKind,
{
    pub buf: RefMut<'a, Lease<Data, P>>,
    pub range: Range<u64>,
}

// Commands specified by the client become Instructions used by `DrawOp`
#[non_exhaustive]
pub(super) enum Instruction<'a, P>
where
    P: 'static + SharedPointerKind,
{
    DataTransfer(DataTransferInstruction<'a>),
    IndexWriteRef(DataWriteRefInstruction<'a, P>),
    LightBegin,
    LightBind(LightBindInstruction<'a>),
    LineDraw(LineDrawInstruction<'a>),
    MeshBegin,
    MeshBindBuffers(MeshBindBuffersInstruction<'a, P>),
    MeshBindDescriptorSet(usize),
    MeshDraw(MeshDrawInstruction<'a, P>),
    PointLightDraw(PointLightDrawInstruction<'a, P>),
    RectLightBegin,
    RectLightDraw(RectLightDrawInstruction<'a>),
    SpotlightBegin,
    SpotlightDraw(SpotlightDrawInstruction<'a>),
    SunlightDraw(SunlightIter<'a, P>),
    VertexAttrsBegin(CalcVertexAttrsComputeMode),
    VertexAttrsCalc(DataComputeInstruction),
    VertexAttrsDescriptors(VertexAttrsDescriptorsInstruction),
    VertexCopy(DataCopyInstruction<'a>),
    VertexWrite(DataWriteInstruction<'a>),
    VertexWriteRef(DataWriteRefInstruction<'a, P>),
}

pub(super) struct LightBindInstruction<'a> {
    pub buf: &'a Data,
    pub buf_len: u64,
}

pub(super) struct LineDrawInstruction<'a> {
    pub buf: &'a mut Data, // TODO: Mut??
    pub line_count: u32,
}

pub(super) struct MeshBindBuffersInstruction<'a, P>
where
    P: SharedPointerKind,
{
    pub idx_buf: Ref<'a, Lease<Data, P>>,
    pub idx_buf_len: u64,
    pub idx_ty: IndexType,
    pub vertex_buf: Ref<'a, Lease<Data, P>>,
    pub vertex_buf_len: u64,
}

pub(super) struct MeshDrawInstruction<'a, P>
where
    P: SharedPointerKind,
{
    pub meshes: MeshIter<'a, P>,
    pub transform: Mat4,
}

pub(super) struct PointLightDrawInstruction<'a, P>
where
    P: 'static + SharedPointerKind,
{
    pub buf: &'a Data,
    pub lights: PointLightIter<'a, P>,
}

pub(super) struct RectLightDrawInstruction<'a> {
    pub light: &'a RectLightCommand,
    pub offset: u32,
}

pub(super) struct SpotlightDrawInstruction<'a> {
    pub light: &'a SpotlightCommand,
    pub offset: u32,
}

pub(super) struct VertexAttrsDescriptorsInstruction {
    pub desc_set: usize,
    pub mode: CalcVertexAttrsComputeMode,
}
