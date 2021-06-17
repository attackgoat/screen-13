//! A collection of operation implementations used to fulfill the Render API.

pub mod bitmap;
pub mod clear;
pub mod copy;
pub mod draw;
pub mod encode;
pub mod gradient;
pub mod text;
pub mod write;

use {
    super::{data::CopyRange, Data, Lease, Pool},
    archery::SharedPointerKind,
    std::any::Any,
    std::ops::Range,
};

/// Copies the gpu-side data from the given ranges to the gpu-side destinations.
pub(super) struct DataCopyInstruction<'a> {
    pub buf: &'a mut Data,
    pub ranges: &'a [CopyRange],
}

/// Transfers the gpu-side data from the source range of one Data to another.
struct DataTransferInstruction<'a> {
    pub dst: &'a mut Data,
    pub src: &'a mut Data,
    pub src_range: Range<u64>,
}

/// Writes the range of cpu-side data to the gpu-side.
pub(super) struct DataWriteInstruction<'a> {
    pub buf: &'a mut Data,
    pub range: Range<u64>,
}

// TODO: `as_any_mut` and `take_pool` will only be used be ops which are part of the `Render`
// system. I should probably create a secondary trait for those bits. See todo!(..) in `Bitmap`.
pub trait Op<P>: Any
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any;

    unsafe fn is_complete(&self) -> bool;

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P>; // TODO: This should become 'take_cmd'! and
                                                         // include cmd buf too

    unsafe fn wait(&self);
}

// TODO: All the places where we bind descriptor sets blindly allow the number of descriptors to be
// unbounded. Should work in groups beyond the limit so the API doesn't have to change.
// TODO: Like above, the places where we dispatch compute resources should probably also allow for
// batch-sized groups within device limits
