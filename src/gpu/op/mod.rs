pub(crate) mod draw;

mod bitmap;
mod clear;
mod copy;
mod encode;
mod font;
mod gradient;
mod write;

pub use self::{
    bitmap::{Bitmap, BitmapOp},
    clear::ClearOp,
    copy::CopyOp,
    //draw::{Command, Compiler, DrawOp, Material},
    encode::EncodeOp,
    font::{Font, FontOp},
    gradient::GradientOp,
    write::{Mode as WriteMode, Write, WriteOp},
};

pub trait Op {
    fn wait(&self);
}
