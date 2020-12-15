use {
    super::compiler::PointLightIter,
    crate::{
        gpu::{data::CopyRange, model::MeshIter, pool::Lease, Data},
        math::Mat4,
        pak::IndexType,
    },
    std::{
        cell::{Ref, RefMut},
        ops::Range,
    },
};

pub struct DataCopyInstruction<'a> {
    pub buf: &'a mut Data,
    pub ranges: &'a [CopyRange],
}

pub struct DataTransferInstruction<'a> {
    pub dst: &'a mut Data,
    pub src: &'a mut Data,
}

pub struct DataWriteInstruction<'a> {
    pub buf: &'a mut Data,
    pub range: Range<u64>,
}

pub struct DataWriteRefInstruction<'a> {
    pub buf: RefMut<'a, Lease<Data>>,
    pub range: Range<u64>,
}

// Commands specified by the client become Instructions used by `DrawOp`
pub enum Instruction<'a> {
    DataTransfer(DataTransferInstruction<'a>),

    // DrawRectLightBegin(&'a mut Data),
    // DrawRectLight(),
    // DrawRectLightEnd,
    IndexWriteRef(DataWriteRefInstruction<'a>),

    LineDraw((&'a mut Data, u32)),

    MeshBegin,
    MeshBind(MeshBindInstruction<'a>),
    MeshDescriptorSet(usize),
    MeshDraw(MeshDrawInstruction<'a>),

    PointLightDraw(PointLightDrawInstruction<'a>),

    // Spotlight(SpotlightCommand),
    // Sunlight(SunlightCommand),
    VertexCopy(DataCopyInstruction<'a>),
    VertexWrite(DataWriteInstruction<'a>),
    VertexWriteRef(DataWriteRefInstruction<'a>),
}

pub struct MeshBindInstruction<'a> {
    pub idx_buf: Ref<'a, Lease<Data>>,
    pub idx_ty: IndexType,
    pub vertex_buf: Ref<'a, Lease<Data>>,
}

pub struct MeshDrawInstruction<'a> {
    pub meshes: MeshIter<'a>,
    pub transform: Mat4,
}

pub struct PointLightDrawInstruction<'a> {
    pub buf: &'a Data,
    pub point_lights: PointLightIter<'a>,
}
