use {
    crate::gpu::{pool::Lease, Data},
    a_r_c_h_e_r_y::SharedPointerKind,
    std::cell::Ref,
};

#[non_exhaustive]
pub enum Instruction<'a, P>
where
    P: SharedPointerKind,
{
    BitmapDescriptor(usize),
    BitmapOutlineDescriptor(usize),
    ScalableDescriptor(usize),
    VertexBind(VertexBindInstruction<'a, P>),
}

pub struct VertexBindInstruction<'a, P>
where
    P: SharedPointerKind,
{
    pub vertex_buf: Ref<'a, Lease<Data, P>>,
    pub vertex_buf_len: u64,
}
