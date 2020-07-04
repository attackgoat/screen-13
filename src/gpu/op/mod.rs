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
    write::{BlendMode, Mode as WriteMode, WriteOp},
};

use {
    crate::math::{Mat3, Mat4},
    gfx_hal::{device::Device as _, Backend},
    gfx_impl::Backend as _Backend,
    std::time::Instant,
};

pub(self) fn mat4_to_u32_array(m: Mat4) -> [u32; 16] {
    let m = m.to_cols_array();
    [
        m[0].to_bits(),
        m[1].to_bits(),
        m[2].to_bits(),
        m[3].to_bits(),
        m[4].to_bits(),
        m[5].to_bits(),
        m[6].to_bits(),
        m[7].to_bits(),
        m[8].to_bits(),
        m[9].to_bits(),
        m[10].to_bits(),
        m[11].to_bits(),
        m[12].to_bits(),
        m[13].to_bits(),
        m[14].to_bits(),
        m[15].to_bits(),
    ]
}

pub(self) fn mat4_to_mat3_u32_array(m: Mat4) -> [u32; 9] {
    let m = m.to_cols_array();
    [
        m[0].to_bits(),
        m[1].to_bits(),
        m[2].to_bits(),
        m[5].to_bits(),
        m[6].to_bits(),
        m[7].to_bits(),
        m[9].to_bits(),
        m[10].to_bits(),
        m[11].to_bits(),
    ]
}

pub(self) fn mat3_to_u32_array(m: Mat3) -> [u32; 9] {
    let m = m.to_cols_array();
    [
        m[0].to_bits(),
        m[1].to_bits(),
        m[2].to_bits(),
        m[3].to_bits(),
        m[4].to_bits(),
        m[5].to_bits(),
        m[6].to_bits(),
        m[7].to_bits(),
        m[8].to_bits(),
    ]
}

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
