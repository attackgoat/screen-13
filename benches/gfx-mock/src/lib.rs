#[cfg(not(tarpaulin_include))]
mod buffer;

#[cfg(not(tarpaulin_include))]
mod cmd_buf;

#[cfg(not(tarpaulin_include))]
mod cmd_pool;

#[cfg(not(tarpaulin_include))]
mod cmd_queue;

#[cfg(not(tarpaulin_include))]
mod desc_pool;

#[cfg(not(tarpaulin_include))]
mod device;

#[cfg(not(tarpaulin_include))]
mod image;

#[cfg(not(tarpaulin_include))]
mod instance;

#[cfg(not(tarpaulin_include))]
mod memory;

#[cfg(not(tarpaulin_include))]
mod phys_device;

#[cfg(not(tarpaulin_include))]
mod queue_family;

#[cfg(not(tarpaulin_include))]
mod surface;

#[cfg(not(tarpaulin_include))]
mod swapchain;

pub use self::instance::Instance;

use {
    self::{
        buffer::*, cmd_buf::*, cmd_pool::*, cmd_queue::*, desc_pool::*, device::*, image::*,
        memory::*, phys_device::*, queue_family::*, surface::*, swapchain::*,
    },
    gfx_hal::{
        adapter::*, buffer::*, command::*, device::*, format::*, image::*, memory::*, pass::*,
        pool::*, pso::*, query::*, queue::*, window::*, *,
    },
};

const QUEUE_FAMILY_ID: queue::QueueFamilyId = queue::QueueFamilyId(0);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Backend {}

impl gfx_hal::Backend for Backend {
    type Instance = Instance;
    type PhysicalDevice = PhysicalDeviceMock;
    type Device = DeviceMock;
    type Surface = SurfaceMock;

    type QueueFamily = QueueFamilyMock;
    type CommandQueue = CommandQueueMock;
    type CommandBuffer = CommandBufferMock;

    type Memory = MemoryMock;
    type CommandPool = CommandPoolMock;

    type ShaderModule = ();
    type RenderPass = ();
    type Framebuffer = ();

    type Buffer = BufferMock;
    type BufferView = ();
    type Image = ImageMock;
    type ImageView = ();
    type Sampler = ();

    type ComputePipeline = ();
    type GraphicsPipeline = ();
    type PipelineCache = ();
    type PipelineLayout = ();
    type DescriptorSetLayout = ();
    type DescriptorPool = DescriptorPoolMock;
    type DescriptorSet = ();

    type Fence = ();
    type Semaphore = ();
    type Event = ();
    type QueryPool = ();
}
