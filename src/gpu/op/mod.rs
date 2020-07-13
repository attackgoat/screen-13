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

use {
    gfx_hal::{device::Device as _, Backend},
    gfx_impl::Backend as _Backend,
};

#[cfg(debug_assertions)]
use std::time::Instant;

// pub(self) fn mat4_to_mat3_u32_array(val: Mat4) -> [u32; 9] {
//     let val = val.to_cols_array();
//     [
//         val[0].to_bits(),
//         val[1].to_bits(),
//         val[2].to_bits(),
//         val[5].to_bits(),
//         val[6].to_bits(),
//         val[7].to_bits(),
//         val[9].to_bits(),
//         val[10].to_bits(),
//         val[11].to_bits(),
//     ]
// }

pub(self) unsafe fn wait_for_fence(
    device: &<_Backend as Backend>::Device,
    fence: &<_Backend as Backend>::Fence,
) {
    // If the fence was ready or anything happened; just return as if we waited
    // otherwise we might hold up a drop function
    if let Ok(true) | Err(_) = device.wait_for_fence(fence, 0) {
        return;
    }

    #[cfg(debug_assertions)]
    {
        let started = Instant::now();

        // TODO: Improve later
        for _ in 0..100 {
            if let Ok(true) | Err(_) = device.wait_for_fence(fence, 1_000_000) {
                let elapsed = Instant::now() - started;
                warn!("Graphics driver stalled! ({}ms)", elapsed.as_millis());

                return;
            }
        }
    }

    panic!("Graphics driver stalled!");
}

pub trait Op {
    fn wait(&self);
}
