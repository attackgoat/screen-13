// // HACK: I'm having trouble supressing the lint at src/graph/mod.rs:650
// #![allow(clippy::match_ref_pats)]

pub mod driver;
pub mod graph;

mod device_api;
mod display;
mod event_loop;
mod frame;
mod hash_pool;
mod input;

/// Things which are used in almost every single _Screen 13_ program.
pub mod prelude {
    pub use {
        super::{
            display::{Display, DisplayError},
            driver::*,
            event_loop::{run, EventLoop, EventLoopBuilder, FullscreenMode},
            frame::{center_cursor, set_cursor_position, FrameContext},
            graph::{
                AccelerationStructureBinding, AccelerationStructureLeaseBinding,
                AccelerationStructureLeaseNode, AccelerationStructureNode,
                AnyAccelerationStructureNode, AnyBufferBinding, AnyBufferNode, AnyImageBinding,
                AnyImageNode, Bind, BufferBinding, BufferLeaseBinding, BufferLeaseNode, BufferNode,
                ImageBinding, ImageLeaseBinding, ImageLeaseNode, ImageNode, PassRef,
                PipelinePassRef, RenderGraph, SwapchainImageNode,
            },
            hash_pool::HashPool,
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

pub use self::display::{Display, DisplayError};
