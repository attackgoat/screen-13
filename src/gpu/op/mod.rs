mod bitmap;
mod clear;
mod copy;
mod draw;
mod encode;
mod font;
mod gradient;
mod write;

pub use self::{
    bitmap::{Bitmap, BitmapOp},
    clear::ClearOp,
    copy::CopyOp,
    draw::{Command, Compiler, DrawOp, Material},
    encode::EncodeOp,
    font::{Font, FontOp},
    gradient::GradientOp,
    write::{Mode as WriteMode, Write, WriteOp},
};

use {
    super::{Lease, Pool},
    std::any::Any,
};

pub trait Op: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn take_pool(&mut self) -> Option<Lease<Pool>>;
    fn wait(&self);
}

// TODO: All the places where we bind descriptor sets blindly allow the number of descriptors to be unbounded. Should work in groups beyond the limit so the API doesn't have to change.
// TODO: Like above, the places where we dispatch compute resources should probably also allow for batch-sized groups within device limits
