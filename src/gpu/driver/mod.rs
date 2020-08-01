mod buffer;
mod command_pool;
mod compute_pipeline;
mod desc_pool;
mod desc_set_layout;
mod device;
mod fence;
mod framebuffer;
mod graphics_pipeline;
mod help;
mod image;
mod image_view;
mod memory;
mod pipeline_layout;
mod render_pass;
mod sampler;
mod semaphore;
mod shader_module;
mod surface;
mod swapchain;

pub use self::{
    buffer::Buffer,
    command_pool::CommandPool,
    compute_pipeline::ComputePipeline,
    desc_pool::DescriptorPool,
    desc_set_layout::DescriptorSetLayout,
    device::{Device, PhysicalDevice},
    fence::Fence,
    graphics_pipeline::GraphicsPipeline,
    help::{
        bind_compute_descriptor_set, bind_graphics_descriptor_set, buffer_copy,
        change_channel_type, descriptor_range_desc, descriptor_set_layout_binding,
    },
    image_view::ImageView,
    memory::Memory,
    pipeline_layout::PipelineLayout,
    render_pass::RenderPass,
    sampler::Sampler,
    semaphore::Semaphore,
    shader_module::ShaderModule,
    surface::Surface,
    swapchain::Swapchain,
};

use {
    self::{framebuffer::Framebuffer, image::Image},
    gfx_hal::Backend,
    gfx_impl::Backend as _Backend,
    std::{cell::RefCell, rc::Rc},
    typenum::{U1, U2, U3},
};

pub type Driver = Rc<RefCell<Device>>;
pub type Framebuffer2d = Framebuffer<U2>;
pub type Image2d = Image<U2>;

pub fn open<'i, I>(
    physical_device: <_Backend as Backend>::PhysicalDevice,
    queue_families: I,
) -> Driver
where
    I: Iterator<Item = &'i <_Backend as Backend>::QueueFamily>,
{
    Driver::new(RefCell::new(
        Device::new(physical_device, queue_families).unwrap(),
    ))
}

pub trait Dim {}

// TODO: Implement if we use 1D images or just remove it
impl Dim for U1 {}

impl Dim for U2 {}

// TODO: Implement 3D images
impl Dim for U3 {}
