mod buf;
mod cmd_buf;
mod compute_pipeline;
mod descriptor_set;
mod descriptor_set_layout;
mod device;
mod device_api;
mod graphic_pipeline;
mod image;
mod instance;
mod physical_device;
mod ray_trace_pipeline;
mod render_pass;
mod shader;
mod surface;
mod swapchain;

pub use {
    self::{
        buf::{Buffer, BufferInfo, BufferInfoBuilder, BufferSubresource},
        cmd_buf::CommandBuffer,
        compute_pipeline::{ComputePipeline, ComputePipelineInfo, ComputePipelineInfoBuilder},
        descriptor_set::{
            DescriptorPool, DescriptorPoolInfo, DescriptorPoolInfoBuilder, DescriptorPoolSize,
            DescriptorSet,
        },
        descriptor_set_layout::DescriptorSetLayout,
        device::{Device, FeatureFlags},
        graphic_pipeline::{
            DepthStencilMode, GraphicPipeline, GraphicPipelineInfo, GraphicPipelineInfoBuilder,
            StencilMode,
        },
        image::{
            Image, ImageInfo, ImageInfoBuilder, ImageSubresource, ImageType, ImageView,
            ImageViewInfo, ImageViewInfoBuilder, SampleCount,
        },
        instance::Instance,
        physical_device::{PhysicalDevice, QueueFamily, QueueFamilyProperties},
        ray_trace_pipeline::{
            RayTraceAcceleration, RayTraceAccelerationScratchBuffer, RayTraceInstanceInfo,
            RayTracePipeline, RayTracePipelineInfo, RayTraceTopAccelerationInfo,
        },
        render_pass::{
            AttachmentInfo, AttachmentInfoBuilder, AttachmentRef, FramebufferKey,
            FramebufferKeyAttachment, RenderPass, RenderPassInfo, RenderPassInfoBuilder,
            SubpassDependency, SubpassDependencyBuilder, SubpassInfo,
        },
        shader::{
            DescriptorBinding, DescriptorBindingMap, DescriptorInfo, PipelineDescriptorInfo,
            Shader, ShaderBuilder,
        },
        surface::Surface,
        swapchain::{
            Swapchain, SwapchainImage, SwapchainImageError, SwapchainInfo, SwapchainInfoBuilder,
        },
    },
    ash::{self, vk},
    vk_sync::AccessType,
};

use {
    crate::ptr::Shared,
    archery::SharedPointerKind,
    derive_builder::Builder,
    glam::UVec2,
    log::{info, trace},
    raw_window_handle::HasRawWindowHandle,
    std::{
        error::Error,
        ffi::CStr,
        fmt::{Display, Formatter},
        os::raw::c_char,
    },
};

pub type QueueFamilyBuilder = QueueFamily;

pub const fn format_aspect_mask(fmt: vk::Format) -> vk::ImageAspectFlags {
    match fmt {
        vk::Format::D16_UNORM => vk::ImageAspectFlags::DEPTH,
        vk::Format::X8_D24_UNORM_PACK32 => vk::ImageAspectFlags::DEPTH,
        vk::Format::D32_SFLOAT => vk::ImageAspectFlags::DEPTH,
        vk::Format::S8_UINT => vk::ImageAspectFlags::STENCIL,
        vk::Format::D16_UNORM_S8_UINT => vk::ImageAspectFlags::from_raw(
            vk::ImageAspectFlags::DEPTH.as_raw() | vk::ImageAspectFlags::STENCIL.as_raw(),
        ),
        vk::Format::D24_UNORM_S8_UINT => vk::ImageAspectFlags::from_raw(
            vk::ImageAspectFlags::DEPTH.as_raw() | vk::ImageAspectFlags::STENCIL.as_raw(),
        ),
        vk::Format::D32_SFLOAT_S8_UINT => vk::ImageAspectFlags::from_raw(
            vk::ImageAspectFlags::DEPTH.as_raw() | vk::ImageAspectFlags::STENCIL.as_raw(),
        ),
        _ => vk::ImageAspectFlags::COLOR,
    }
}

pub const fn is_read_access(ty: AccessType) -> bool {
    !is_write_access(ty)
}

pub const fn is_write_access(ty: AccessType) -> bool {
    use AccessType::*;
    match ty {
        Nothing
        | CommandBufferReadNVX
        | IndirectBuffer
        | IndexBuffer
        | VertexBuffer
        | VertexShaderReadUniformBuffer
        | VertexShaderReadSampledImageOrUniformTexelBuffer
        | VertexShaderReadOther
        | TessellationControlShaderReadUniformBuffer
        | TessellationControlShaderReadSampledImageOrUniformTexelBuffer
        | TessellationControlShaderReadOther
        | TessellationEvaluationShaderReadUniformBuffer
        | TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer
        | TessellationEvaluationShaderReadOther
        | GeometryShaderReadUniformBuffer
        | GeometryShaderReadSampledImageOrUniformTexelBuffer
        | GeometryShaderReadOther
        | FragmentShaderReadUniformBuffer
        | FragmentShaderReadSampledImageOrUniformTexelBuffer
        | FragmentShaderReadColorInputAttachment
        | FragmentShaderReadDepthStencilInputAttachment
        | FragmentShaderReadOther
        | ColorAttachmentRead
        | DepthStencilAttachmentRead
        | ComputeShaderReadUniformBuffer
        | ComputeShaderReadSampledImageOrUniformTexelBuffer
        | ComputeShaderReadOther
        | AnyShaderReadUniformBuffer
        | AnyShaderReadUniformBufferOrVertexBuffer
        | AnyShaderReadSampledImageOrUniformTexelBuffer
        | AnyShaderReadOther
        | TransferRead
        | HostRead
        | Present
        | RayTracingShaderReadSampledImageOrUniformTexelBuffer
        | RayTracingShaderReadColorInputAttachment
        | RayTracingShaderReadDepthStencilInputAttachment
        | RayTracingShaderReadAccelerationStructure
        | RayTracingShaderReadOther
        | AccelerationStructureBuildRead => false,
        CommandBufferWriteNVX
        | VertexShaderWrite
        | TessellationControlShaderWrite
        | TessellationEvaluationShaderWrite
        | GeometryShaderWrite
        | FragmentShaderWrite
        | ColorAttachmentWrite
        | DepthStencilAttachmentWrite
        | DepthAttachmentWriteStencilReadOnly
        | StencilAttachmentWriteDepthReadOnly
        | ComputeShaderWrite
        | AnyShaderWrite
        | TransferWrite
        | HostWrite
        | ColorAttachmentReadWrite
        | General
        | AccelerationStructureBuildWrite
        | AccelerationStructureBufferWrite => true,
    }
}

#[derive(Debug)]
pub struct Driver<P>
where
    P: SharedPointerKind,
{
    pub device: Shared<Device<P>, P>,
    pub swapchain: Swapchain<P>,
}

impl<P> Driver<P>
where
    P: SharedPointerKind,
{
    pub fn new(
        window: &impl HasRawWindowHandle,
        cfg: DriverConfig,
        desired_resolution: UVec2,
    ) -> Result<Self, DriverError> {
        trace!("new {:?}", cfg);

        let required_extensions = ash_window::enumerate_required_extensions(window)
            .map_err(|_| DriverError::Unsupported)?;
        let instance = Shared::new(Instance::new(cfg.debug, required_extensions.into_iter())?);
        let surface = Surface::new(&instance, window)?;
        let physical_devices = Instance::physical_devices(&instance)?
            .filter(|physical_device| {
                // Filters this list down to only supported devices
                #[allow(clippy::if_same_then_else)]
                if cfg.presentation
                    && !PhysicalDevice::has_presentation_support(
                        physical_device,
                        &instance,
                        &surface,
                    )
                {
                    return false;
                } else if cfg.ray_tracing
                    && !PhysicalDevice::has_ray_tracing_support(physical_device)
                {
                    return false;
                }

                true
            })
            .collect::<Vec<_>>();

        info!(
            "Supported GPUs: {:#?}",
            physical_devices
                .iter()
                .map(|physical_device| unsafe {
                    CStr::from_ptr(physical_device.props.device_name.as_ptr() as *const c_char)
                })
                .collect::<Vec<_>>()
        );

        let physical_device = physical_devices
            .into_iter()
            // If there are multiple devices with the same score, `max_by_key` would choose the last,
            // and we want to preserve the order of devices from `enumerate_physical_devices`.
            .rev()
            .max_by_key(|physical_device| Self::score_device_type(physical_device))
            .ok_or(DriverError::Unsupported)?;

        info!("Selected GPU: {:#?}", physical_device);

        let device = Shared::new(Device::create(&instance, physical_device, cfg)?);
        let surface_formats = Device::surface_formats(&device, &surface)?;

        info!("Surface formats: {:#?}", surface_formats);

        let format = surface_formats
            .into_iter()
            .find(|format| Self::select_swapchain_format(*format))
            .ok_or(DriverError::Unsupported)?;
        let swapchain = Swapchain::new(
            &device,
            surface,
            SwapchainInfo {
                desired_image_count: cfg.desired_swapchain_image_count,
                format,
                extent: desired_resolution,
                sync_display: cfg.sync_display,
            },
        )?;

        info!("OK");

        Ok(Self { device, swapchain })
    }

    fn select_swapchain_format(format: vk::SurfaceFormatKHR) -> bool {
        format.format == vk::Format::B8G8R8A8_UNORM
            && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    }

    fn score_device_type(physical_device: &PhysicalDevice) -> usize {
        match physical_device.props.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
            vk::PhysicalDeviceType::INTEGRATED_GPU => 200,
            vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
            _ => 0,
        }
    }
}

/// A list of required features. Features that are supported but not required will not be
/// available.
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned", derive(Debug))]
pub struct DriverConfig {
    #[builder(default)]
    pub debug: bool,
    #[builder(default = "3")]
    pub desired_swapchain_image_count: u32,
    #[builder(default = "true")]
    pub sync_display: bool,
    #[builder(default)]
    pub dlss: bool,
    #[builder(default = "true")]
    pub presentation: bool,
    #[builder(default)]
    pub ray_tracing: bool,
}

impl DriverConfig {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> DriverConfigBuilder {
        Default::default()
    }

    fn features(self) -> FeatureFlags {
        let mut res = FeatureFlags::PRESENTATION;

        if self.dlss {
            res |= FeatureFlags::DLSS;
        }

        if self.ray_tracing {
            res |= FeatureFlags::RAY_TRACING;
        }

        res
    }
}

// TODO: A more robust error type and some proper vk error mapping
#[derive(Debug)]
pub enum DriverError {
    InvalidData,
    Unsupported,
    OutOfMemory,
}

impl Display for DriverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for DriverError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SamplerDesc {
    pub texel_filter: vk::Filter,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub address_modes: vk::SamplerAddressMode,
}
