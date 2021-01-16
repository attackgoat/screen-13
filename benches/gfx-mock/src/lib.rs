mod buffer;
mod cmd_buf;
mod cmd_pool;
mod cmd_queue;
mod desc_pool;
mod device;
mod image;
mod instance;
mod memory;
mod phys_device;
mod queue_family;
mod surface;
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
    type DescriptorSetLayout = DescriptorSetLayoutMock;
    type DescriptorPool = DescriptorPoolMock;
    type DescriptorSet = DescriptorSetMock;

    type Fence = ();
    type Semaphore = ();
    type Event = ();
    type QueryPool = ();
}

#[derive(Debug)]
pub struct DescriptorSetLayoutMock {
    pub(crate) name: String,
}

#[derive(Debug)]
pub struct DescriptorSetMock {
    pub(crate) name: String,
}
