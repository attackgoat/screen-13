mod cmd_queue;
mod phys_device;

use {gfx_hal::{*, adapter::*, buffer::*, command::*, device::*, format::*, image::*, memory::*, pass::*, pool::*, pso::*, query::*, queue::*, window::*},
self::{
    cmd_queue::*,
    phys_device::*,
}
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BackendMock {}

impl Backend for BackendMock {
    type Instance = InstanceMock;
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

    type Buffer = Buffer;
    type BufferView = ();
    type Image = Image;
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
