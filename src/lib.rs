// REMOVE BEFORE FLIGHT:
#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_code,
    unused_mut,
    unused_unsafe,
    unused_assignments
)]

pub mod driver;
pub mod graph;

#[cfg(feature = "pak")]
pub mod pak;

mod cmd_chain;
mod display;
mod event_loop;
mod frame;
mod hash_pool;
mod input;

pub use self::{
    cmd_chain::{execute, CommandChain, ExecutionError},
    display::{Display, DisplayError},
    event_loop::{EventLoop, EventLoopBuilder, FullscreenMode},
    frame::FrameContext,
    hash_pool::{HashPool, Lease},
};

/// Things, particularly traits, which are used in almost every single _Screen 13_ program.
pub mod prelude {
    pub use {
        super::{
            align_up_u32, align_up_u64, as_u8_slice,
            event_loop::{EventLoop, FullscreenMode},
            execute,
            frame::FrameContext,
            graph::RenderGraph,
            input::{
                update_input, update_keyboard, update_mouse, KeyBuf, KeyMap, MouseBuf, MouseButton,
            },
            into_u8_slice,
            ptr::{ArcK, RcK, Shared, SharedPointerKind},
            CommandChain, ExecutionError,
        },
        glam::*,
        log::{debug, error, info, trace, warn}, // Everyone wants a log
        winit::{
            dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
            event::{Event, VirtualKeyCode},
            monitor::{MonitorHandle, VideoMode},
            window::{Window, WindowBuilder},
        },
    };
}

/// Like [`prelude`], but contains all public exports.
///
/// Use this module for access to all _Screen 13_ resources from either [`std::sync::Arc`] or
/// [`std::rc::Rc`]-backed [`Gpu`] instances.
pub mod prelude_all {
    pub use super::{
        driver::*,
        graph::{
            AccessType, AnyBufferBinding, AnyImageBinding, AnyImageNode, BufferBinding,
            BufferLeaseBinding, BufferLeaseNode, BufferNode, ImageBinding, ImageLayout,
            ImageLeaseBinding, ImageLeaseNode, ImageNode, PassRef, RayTraceAccelerationBinding,
            RayTraceAccelerationNode, RenderGraph, SwapchainImageNode,
        },
        prelude::*,
        Display, DisplayError, EventLoopBuilder, HashPool, Lease,
    }; // TODO: Expand!

    #[cfg(feature = "pak")]
    pub use super::pak::{
        buf::PakBuf,
        compression::{BrotliParams, Compression},
        AnimationBuf, AnimationId, BitmapBuf, BitmapColor, BitmapFontBuf, BitmapFontId,
        BitmapFormat, BitmapId, BlobId, IndexType, MaterialId, MaterialInfo, Mesh, ModelBuf,
        ModelId, Pak, SceneBuf, SceneId,
    };

    #[cfg(feature = "bake")]
    pub use super::pak::buf::Writer;
}

/// Like [`prelude_all`], but specialized for [`std::sync::Arc`]-backed [`Gpu`] instances.
///
/// Use this module if rendering will be done from multiple threads. See the main documentation for
/// each alias for more information.
pub mod prelude_arc {
    pub use super::{
        prelude_all::{self as all, *},
        ptr::ArcK as P,
    };

    pub type Buffer = all::Buffer<P>;
    pub type BufferBinding = all::BufferBinding<P>;
    pub type BufferNode = all::BufferNode<P>;
    pub type Device = all::Device<P>;
    pub type EventLoop = all::EventLoop<P>;
    pub type FrameContext<'a> = all::FrameContext<'a, P>;
    pub type HashPool = all::HashPool<P>;
    pub type Image = all::Image<P>;
    pub type ImageBinding = all::ImageBinding<P>;
    pub type ImageNode = all::ImageNode<P>;
    pub type RayTraceAccelerationNode = all::RayTraceAccelerationNode<P>;
    pub type RenderGraph = all::RenderGraph<P>;
    pub type SwapchainImage = all::SwapchainImage<P>;

    pub type Lease<T> = all::Lease<T, P>;
    pub type Shared<T> = all::Shared<T, P>;
}

/// Like [`prelude_all`], but specialized for [`std::rc::Rc`]-backed [`Gpu`] instances.
///
/// Use this module if rendering will be done from one thread only. See the main documentation for
/// each alias for more information.
pub mod prelude_rc {
    pub use super::{
        prelude_all::{self as all, *},
        ptr::RcK as P,
    };

    pub type Buffer = all::Buffer<P>;
    pub type BufferBinding = all::BufferBinding<P>;
    pub type BufferNode = all::BufferNode<P>;
    pub type Device = all::Device<P>;
    pub type EventLoop = all::EventLoop<P>;
    pub type FrameContext<'a> = all::FrameContext<'a, P>;
    pub type HashPool = all::HashPool<P>;
    pub type Image = all::Image<P>;
    pub type ImageBinding = all::ImageBinding<P>;
    pub type ImageNode = all::ImageNode<P>;
    pub type RayTraceAccelerationNode = all::RayTraceAccelerationNode<P>;
    pub type RenderGraph = all::RenderGraph<P>;
    pub type SwapchainImage = all::SwapchainImage<P>;

    pub type Lease<T> = all::Lease<T, P>;
    pub type Shared<T> = all::Shared<T, P>;
}

/// Shared reference (`Arc` and `Rc`) implementation based on
/// [_archery_](https://crates.io/crates/archery).
pub mod ptr {
    pub use archery::{ArcK, RcK, SharedPointerKind};

    use {archery::SharedPointer, std::ops::Deref};

    // TODO: Provide a handy-dandy Mutex stand-in for the 'rc' path (make it zero cost!) "Locked" ?

    /// A shared reference wrapper type, based on either [`std::sync::Arc`] or [`std::rc::Rc`].
    #[derive(Debug, Eq, Ord, PartialOrd)]
    pub struct Shared<T, P>(SharedPointer<T, P>)
    where
        P: SharedPointerKind;

    impl<T, P> Shared<T, P>
    where
        P: SharedPointerKind,
    {
        pub fn new(val: T) -> Self {
            Self(SharedPointer::new(val))
        }

        /// Returns a constant pointer to the value.
        pub fn as_ptr(shared: &Self) -> *const T {
            SharedPointer::as_ptr(&shared.0)
        }

        /// Returns a copy of the value.
        #[allow(clippy::should_implement_trait)]
        pub fn clone(shared: &Self) -> Self {
            shared.clone()
        }

        /// Returns a mutable reference into the given shared pointer, if there are no other
        /// pointers to the same allocation.
        ///
        /// Returns None otherwise, because it is not safe to mutate a shared value.
        pub fn get_mut(shared: &mut Self) -> Option<&mut T> {
            SharedPointer::get_mut(&mut shared.0)
        }

        /// Returns `true` if two `Shared` instances point to the same underlying memory.
        pub fn ptr_eq(lhs: &Self, rhs: &Self) -> bool {
            SharedPointer::ptr_eq(&lhs.0, &rhs.0)
        }
    }

    impl<T, P> Clone for Shared<T, P>
    where
        P: SharedPointerKind,
    {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<T, P> Default for Shared<T, P>
    where
        P: SharedPointerKind,
        T: Default,
    {
        fn default() -> Self {
            Self::new(Default::default())
        }
    }

    impl<T, P> Deref for Shared<T, P>
    where
        P: SharedPointerKind,
    {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T, P> From<&Shared<T, P>> for Shared<T, P>
    where
        P: SharedPointerKind,
    {
        fn from(val: &Self) -> Self {
            val.clone()
        }
    }

    impl<T, P> From<T> for Shared<T, P>
    where
        P: SharedPointerKind,
    {
        fn from(val: T) -> Self {
            Self::new(val)
        }
    }

    impl<T, P> PartialEq for Shared<T, P>
    where
        P: SharedPointerKind,
    {
        fn eq(&self, other: &Self) -> bool {
            Self::ptr_eq(self, other)
        }
    }
}

pub fn align_up_u32(val: u32, atom: u32) -> u32 {
    (val + atom - 1) & !(atom - 1)
}

// TODO: I tried some num traits and it become quite unwieldy, but try again to genericize this
pub fn align_up_u64(val: u64, atom: u64) -> u64 {
    (val + atom - 1) & !(atom - 1)
}

pub fn as_u8_slice<T>(t: &T) -> &[u8]
where
    T: Copy + Sized,
{
    use std::{mem::size_of, slice::from_raw_parts};

    unsafe { from_raw_parts(t as *const T as *const _, size_of::<T>()) }
}

pub fn into_u8_slice<T>(t: &[T]) -> &[u8]
where
    T: Copy + Sized,
{
    use std::{mem::size_of, slice::from_raw_parts};

    unsafe { from_raw_parts(t.as_ptr() as *const _, t.len() * size_of::<T>()) }
}

// Must be aligned.
pub fn as_u32_slice<T>(t: &[T]) -> &[u32]
where
    T: Copy + Sized,
{
    use std::{mem::size_of, slice::from_raw_parts};

    let len = t.len() * size_of::<T>();

    assert!(len % size_of::<u32>() == 0);

    unsafe { from_raw_parts(&t[0] as *const T as *const _, len) }
}
