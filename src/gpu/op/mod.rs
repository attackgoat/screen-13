//! A collection of operation implementations used to fulfill the Render API.

pub mod bitmap;
pub mod clear;
pub mod copy;
pub mod draw;
pub mod encode;
pub mod font;
pub mod gradient;
pub mod write;

use {
    super::{Lease, Pool},
    archery::SharedPointerKind,
    std::any::Any,
};

// TODO: `as_any_mut` and `take_pool` will only be used be ops which are part of the `Render`
// system. I should probably create a secondary trait for those bits. See todo!(..) in `Bitmap`.
pub trait Op<P>: Any
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any;
    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P>; // TODO: This should become 'take_cmd'! and
                                                         // include cmd buf too
    unsafe fn wait(&self);
}

// TODO: All the places where we bind descriptor sets blindly allow the number of descriptors to be
// unbounded. Should work in groups beyond the limit so the API doesn't have to change.
// TODO: Like above, the places where we dispatch compute resources should probably also allow for
// batch-sized groups within device limits
