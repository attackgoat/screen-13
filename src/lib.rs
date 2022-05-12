// HACK: I'm having trouble supressing the lint at src/graph/mod.rs:650
#![allow(clippy::match_ref_pats)]

pub mod driver;
pub mod graph;

mod device_api;
mod display;
mod event_loop;
mod frame;
mod hash_pool;
mod input;

pub use self::{
    display::{Display, DisplayError},
    event_loop::{run, EventLoop, EventLoopBuilder, FullscreenMode},
    frame::FrameContext,
    hash_pool::{HashPool, Lease},
};

/// Things which are used in almost every single _Screen 13_ program.
pub mod prelude {
    pub use {
        super::{
            event_loop::{run, EventLoop, EventLoopBuilder, FullscreenMode},
            frame::{center_cursor, set_cursor_position, FrameContext},
            graph::RenderGraph,
            input::{
                update_input, update_keyboard, update_mouse, KeyBuf, KeyMap, MouseBuf, MouseButton,
            },
        },
        log::{debug, error, info, logger, trace, warn}, // Everyone wants a log
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
/// [`std::rc::Rc`]-backed instances.
pub mod prelude_all {
    pub use super::{
        driver::*,
        graph::{
            AccelerationStructureBinding, AccelerationStructureLeaseBinding,
            AccelerationStructureLeaseNode, AccelerationStructureNode,
            AnyAccelerationStructureNode, AnyBufferBinding, AnyBufferNode, AnyImageBinding,
            AnyImageNode, BufferBinding, BufferLeaseBinding, BufferLeaseNode, BufferNode,
            ImageBinding, ImageLeaseBinding, ImageLeaseNode, ImageNode, PassRef, PipelinePassRef,
            RenderGraph, SwapchainImageNode,
        },
        prelude::*,
        Display, DisplayError, HashPool, Lease,
    }; // TODO: Expand!
}

/// Like [`prelude_all`], but specialized for [`std::sync::Arc`]-backed use cases.
///
/// Use this module if rendering will be done from multiple threads. See the main documentation for
/// each alias for more information.
pub mod prelude_arc {
    pub use super::prelude_all::{self as all, *};

    use archery::ArcK as P;

    pub type AccelerationStructure = all::AccelerationStructure<P>;
    pub type AnyBufferBinding<'a> = all::AnyBufferBinding<'a, P>;
    pub type AnyBufferNode = all::AnyBufferNode<P>;
    pub type AnyImageBinding<'a> = all::AnyImageBinding<'a, P>;
    pub type AnyImageNode = all::AnyImageNode<P>;
    pub type Buffer = all::Buffer<P>;
    pub type BufferBinding = all::BufferBinding<P>;
    pub type BufferLeaseNode = all::BufferLeaseNode<P>;
    pub type BufferNode = all::BufferNode<P>;
    pub type ComputePipeline = all::ComputePipeline<P>;
    pub type Device = all::Device<P>;
    pub type EventLoop = all::EventLoop<P>;
    pub type FrameContext<'a> = all::FrameContext<'a, P>;
    pub type GraphicPipeline = all::GraphicPipeline<P>;
    pub type HashPool = all::HashPool<P>;
    pub type Image = all::Image<P>;
    pub type ImageBinding = all::ImageBinding<P>;
    pub type ImageNode = all::ImageNode<P>;
    pub type PipelinePassRef<'a, T> = all::PipelinePassRef<'a, T, P>;
    pub type RayTracePipeline = all::RayTracePipeline<P>;
    pub type RenderGraph = all::RenderGraph<P>;
    pub type SwapchainImage = all::SwapchainImage<P>;

    pub type Lease<T> = all::Lease<T, P>;
    pub type Shared<T> = archery::SharedPointer<T, P>;
}

/// Like [`prelude_all`], but specialized for [`std::rc::Rc`]-backed use cases.
///
/// Use this module if rendering will be done from one thread only. See the main documentation for
/// each alias for more information.
pub mod prelude_rc {
    pub use super::prelude_all::{self as all, *};

    use archery::RcK as P;

    pub type AccelerationStructure = all::AccelerationStructure<P>;
    pub type AnyBufferBinding<'a> = all::AnyBufferBinding<'a, P>;
    pub type AnyBufferNode = all::AnyBufferNode<P>;
    pub type AnyImageBinding<'a> = all::AnyImageBinding<'a, P>;
    pub type AnyImageNode = all::AnyImageNode<P>;
    pub type Buffer = all::Buffer<P>;
    pub type BufferBinding = all::BufferBinding<P>;
    pub type BufferLeaseNode = all::BufferLeaseNode<P>;
    pub type BufferNode = all::BufferNode<P>;
    pub type ComputePipeline = all::ComputePipeline<P>;
    pub type Device = all::Device<P>;
    pub type EventLoop = all::EventLoop<P>;
    pub type FrameContext<'a> = all::FrameContext<'a, P>;
    pub type GraphicPipeline = all::GraphicPipeline<P>;
    pub type HashPool = all::HashPool<P>;
    pub type Image = all::Image<P>;
    pub type ImageBinding = all::ImageBinding<P>;
    pub type ImageNode = all::ImageNode<P>;
    pub type PipelinePassRef<'a, T> = all::PipelinePassRef<'a, T, P>;
    pub type RayTracePipeline = all::RayTracePipeline<P>;
    pub type RenderGraph = all::RenderGraph<P>;
    pub type SwapchainImage = all::SwapchainImage<P>;

    pub type Lease<T> = all::Lease<T, P>;
    pub type Shared<T> = archery::SharedPointer<T, P>;
}
