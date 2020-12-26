use {
    super::command::{PointLightIter, RectLightCommand, SpotlightCommand, SunlightIter},
    crate::{
        gpu::{
            data::CopyRange, def::CalcVertexAttrsComputeMode, model::MeshIter, pool::Lease, Data,
        },
        math::Mat4,
        pak::IndexType,
    },
    std::{
        cell::{Ref, RefMut},
        ops::Range,
    },
};

pub struct DataComputeInstruction {
    pub base_idx: u32,
    pub base_vertex: u32,
    pub dispatch: u32,
}

/// Copies the gpu-side data from the given range to the cpu-side
pub struct DataCopyInstruction<'a> {
    pub buf: &'a mut Data,
    pub ranges: &'a [CopyRange],
}

/// Transfers the gpu-side data from the source range of one Data to another.
pub struct DataTransferInstruction<'a> {
    pub dst: &'a mut Data,
    pub src: &'a mut Data,
    pub src_range: Range<u64>,
}

/// Writes the range of cpu-side data to the gpu-side.
pub struct DataWriteInstruction<'a> {
    pub buf: &'a mut Data,
    pub range: Range<u64>,
}

/// Writes the range of cpu-side data to the gpu-side.
pub struct DataWriteRefInstruction<'a> {
    pub buf: RefMut<'a, Lease<Data>>,
    pub range: Range<u64>,
}

// Commands specified by the client become Instructions used by `DrawOp`
pub enum Instruction<'a> {
    DataTransfer(DataTransferInstruction<'a>),
    IndexWriteRef(DataWriteRefInstruction<'a>),
    LightBegin,
    LightBind(LightBindInstruction<'a>),
    LineDraw(LineDrawInstruction<'a>),
    MeshBegin,
    MeshBind(MeshBindInstruction<'a>),
    MeshDescriptors(usize),
    MeshDraw(MeshDrawInstruction<'a>),
    PointLightDraw(PointLightDrawInstruction<'a>),
    RectLightBegin,
    RectLightDraw(RectLightDrawInstruction<'a>),
    SpotlightBegin,
    SpotlightDraw(SpotlightDrawInstruction<'a>),
    SunlightBegin,
    SunlightDraw(SunlightIter<'a>),
    VertexAttrsBegin(CalcVertexAttrsComputeMode),
    VertexAttrsCalc(DataComputeInstruction),
    VertexAttrsDescriptors(VertexAttrsDescriptorsInstruction),
    VertexCopy(DataCopyInstruction<'a>),
    VertexWrite(DataWriteInstruction<'a>),
    VertexWriteRef(DataWriteRefInstruction<'a>),
}

pub struct LightBindInstruction<'a> {
    pub buf: &'a Data,
    pub buf_len: u64,
}

pub struct LineDrawInstruction<'a> {
    pub buf: &'a mut Data, // TODO: Mut??
    pub line_count: u32,
}

pub struct MeshBindInstruction<'a> {
    pub idx_buf: Ref<'a, Lease<Data>>,
    pub idx_buf_len: u64,
    pub idx_ty: IndexType,
    pub vertex_buf: Ref<'a, Lease<Data>>,
    pub vertex_buf_len: u64,
}

pub struct MeshDrawInstruction<'a> {
    pub meshes: MeshIter<'a>,
    pub transform: Mat4,
}

pub struct PointLightDrawInstruction<'a> {
    pub buf: &'a Data,
    pub lights: PointLightIter<'a>,
}

pub struct RectLightDrawInstruction<'a> {
    pub light: &'a RectLightCommand,
    pub offset: u32,
}

pub struct SpotlightDrawInstruction<'a> {
    pub light: &'a SpotlightCommand,
    pub offset: u32,
}

pub struct VertexAttrsDescriptorsInstruction {
    pub desc_set: usize,
    pub mode: CalcVertexAttrsComputeMode,
}
