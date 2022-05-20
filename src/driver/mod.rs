mod accel_struct;
mod buffer;
mod cmd_buf;
mod compute;
mod descriptor_set;
mod descriptor_set_layout;
mod device;
mod graphic;
mod image;
mod instance;
mod physical_device;
mod ray_trace;
mod render_pass;
mod shader;
mod surface;
mod swapchain;

pub use {
    self::{
        accel_struct::{
            AccelerationStructure, AccelerationStructureGeometry,
            AccelerationStructureGeometryData, AccelerationStructureGeometryInfo,
            AccelerationStructureInfo, AccelerationStructureInfoBuilder, DeviceOrHostAddress,
        },
        buffer::{Buffer, BufferInfo, BufferInfoBuilder, BufferSubresource},
        cmd_buf::CommandBuffer,
        compute::{ComputePipeline, ComputePipelineInfo, ComputePipelineInfoBuilder},
        descriptor_set::{
            DescriptorPool, DescriptorPoolInfo, DescriptorPoolInfoBuilder, DescriptorPoolSize,
            DescriptorSet,
        },
        descriptor_set_layout::DescriptorSetLayout,
        device::{Device, FeatureFlags},
        graphic::{
            BlendMode, DepthStencilMode, GraphicPipeline, GraphicPipelineInfo,
            GraphicPipelineInfoBuilder, StencilMode, VertexInputState,
        },
        image::{
            Image, ImageInfo, ImageInfoBuilder, ImageSubresource, ImageType, ImageView,
            ImageViewInfo, ImageViewInfoBuilder, SampleCount,
        },
        instance::Instance,
        physical_device::{PhysicalDevice, QueueFamily, QueueFamilyProperties},
        ray_trace::{
            RayTracePipeline, RayTracePipelineInfo, RayTracePipelineInfoBuilder,
            RayTraceShaderGroup,
        },
        render_pass::{
            AttachmentInfo, AttachmentInfoBuilder, AttachmentRef, FramebufferKey,
            FramebufferKeyAttachment, RenderPass, RenderPassInfo, RenderPassInfoBuilder,
            SubpassDependency, SubpassDependencyBuilder, SubpassInfo,
        },
        shader::{
            DescriptorBinding, DescriptorBindingMap, DescriptorInfo, PipelineDescriptorInfo,
            Shader, ShaderBuilder, SpecializationInfo,
        },
        surface::Surface,
        swapchain::{
            Swapchain, SwapchainError, SwapchainImage, SwapchainInfo, SwapchainInfoBuilder,
        },
    },
    ash::{self, vk},
    vk_sync::{AccessType, ImageLayout},
};

use {
    archery::{SharedPointer, SharedPointerKind},
    derive_builder::Builder,
    log::{debug, info, trace, warn},
    raw_window_handle::HasRawWindowHandle,
    std::{
        error::Error,
        ffi::CStr,
        fmt::{Display, Formatter},
        ops::Range,
        os::raw::c_char,
    },
};

pub type QueueFamilyBuilder = QueueFamily;

#[allow(clippy::reversed_empty_ranges)]
pub fn buffer_copy_subresources(
    regions: &[vk::BufferCopy],
) -> (Range<vk::DeviceSize>, Range<vk::DeviceSize>) {
    let mut src = vk::DeviceSize::MAX..vk::DeviceSize::MIN;
    let mut dst = vk::DeviceSize::MAX..vk::DeviceSize::MIN;
    for region in regions.iter() {
        src.start = src.start.min(region.src_offset);
        src.end = src.end.max(region.src_offset + region.size);

        dst.start = dst.start.min(region.dst_offset);
        dst.end = dst.end.max(region.dst_offset + region.size);
    }

    debug_assert!(src.end > src.start);
    debug_assert!(dst.end > dst.start);

    (src, dst)
}

#[allow(clippy::reversed_empty_ranges)]
pub fn buffer_image_copy_subresource(regions: &[vk::BufferImageCopy]) -> Range<vk::DeviceSize> {
    debug_assert!(!regions.is_empty());

    let mut res = vk::DeviceSize::MAX..vk::DeviceSize::MIN;
    for region in regions.iter() {
        res.start = res.start.min(region.buffer_offset);
        res.end = res.end.max(
            region.buffer_offset
                + (region.buffer_row_length * region.buffer_image_height) as vk::DeviceSize,
        );
    }

    debug_assert!(res.end > res.start);

    res
}

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

pub const fn image_access_layout(access: AccessType) -> ImageLayout {
    if matches!(access, AccessType::Present | AccessType::ComputeShaderWrite) {
        ImageLayout::General
    } else {
        ImageLayout::Optimal
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

pub const fn pipeline_stage_access_flags(
    access_type: AccessType,
) -> (vk::PipelineStageFlags, vk::AccessFlags) {
    use {
        vk::{AccessFlags as access, PipelineStageFlags as stage},
        AccessType as ty,
    };

    match access_type {
        ty::Nothing => (stage::empty(), access::empty()),
        ty::CommandBufferReadNVX => (
            stage::COMMAND_PREPROCESS_NV,
            access::COMMAND_PREPROCESS_READ_NV,
        ),
        ty::IndirectBuffer => (stage::DRAW_INDIRECT, access::INDIRECT_COMMAND_READ),
        ty::IndexBuffer => (stage::VERTEX_INPUT, access::INDEX_READ),
        ty::VertexBuffer => (stage::VERTEX_INPUT, access::VERTEX_ATTRIBUTE_READ),
        ty::VertexShaderReadUniformBuffer => (stage::VERTEX_SHADER, access::SHADER_READ),
        ty::VertexShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::VERTEX_SHADER, access::SHADER_READ)
        }
        ty::VertexShaderReadOther => (stage::VERTEX_SHADER, access::SHADER_READ),
        ty::TessellationControlShaderReadUniformBuffer => {
            (stage::TESSELLATION_CONTROL_SHADER, access::UNIFORM_READ)
        }
        ty::TessellationControlShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::TESSELLATION_CONTROL_SHADER, access::SHADER_READ)
        }
        ty::TessellationControlShaderReadOther => {
            (stage::TESSELLATION_CONTROL_SHADER, access::SHADER_READ)
        }
        ty::TessellationEvaluationShaderReadUniformBuffer => {
            (stage::TESSELLATION_EVALUATION_SHADER, access::UNIFORM_READ)
        }
        ty::TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::TESSELLATION_EVALUATION_SHADER, access::SHADER_READ)
        }
        ty::TessellationEvaluationShaderReadOther => {
            (stage::TESSELLATION_EVALUATION_SHADER, access::SHADER_READ)
        }
        ty::GeometryShaderReadUniformBuffer => (stage::GEOMETRY_SHADER, access::UNIFORM_READ),
        ty::GeometryShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::GEOMETRY_SHADER, access::SHADER_READ)
        }
        ty::GeometryShaderReadOther => (stage::GEOMETRY_SHADER, access::SHADER_READ),
        ty::FragmentShaderReadUniformBuffer => (stage::FRAGMENT_SHADER, access::UNIFORM_READ),
        ty::FragmentShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::FRAGMENT_SHADER, access::SHADER_READ)
        }
        ty::FragmentShaderReadColorInputAttachment => {
            (stage::FRAGMENT_SHADER, access::INPUT_ATTACHMENT_READ)
        }
        ty::FragmentShaderReadDepthStencilInputAttachment => {
            (stage::FRAGMENT_SHADER, access::INPUT_ATTACHMENT_READ)
        }
        ty::FragmentShaderReadOther => (stage::FRAGMENT_SHADER, access::SHADER_READ),
        ty::ColorAttachmentRead => (
            stage::COLOR_ATTACHMENT_OUTPUT,
            access::COLOR_ATTACHMENT_READ,
        ),
        ty::DepthStencilAttachmentRead => (
            stage::from_raw(
                stage::EARLY_FRAGMENT_TESTS.as_raw()
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS.as_raw(),
            ),
            access::DEPTH_STENCIL_ATTACHMENT_READ,
        ),
        ty::ComputeShaderReadUniformBuffer => (stage::COMPUTE_SHADER, access::UNIFORM_READ),
        ty::ComputeShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::COMPUTE_SHADER, access::SHADER_READ)
        }
        ty::ComputeShaderReadOther => (stage::COMPUTE_SHADER, access::SHADER_READ),
        ty::AnyShaderReadUniformBuffer => (stage::ALL_COMMANDS, access::UNIFORM_READ),
        ty::AnyShaderReadUniformBufferOrVertexBuffer => (
            stage::ALL_COMMANDS,
            access::from_raw(
                access::UNIFORM_READ.as_raw() | vk::AccessFlags::VERTEX_ATTRIBUTE_READ.as_raw(),
            ),
        ),
        ty::AnyShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::ALL_COMMANDS, access::SHADER_READ)
        }
        ty::AnyShaderReadOther => (stage::ALL_COMMANDS, access::SHADER_READ),
        ty::TransferRead => (stage::TRANSFER, access::TRANSFER_READ),
        ty::HostRead => (stage::HOST, access::HOST_READ),
        ty::Present => (stage::empty(), access::empty()),
        ty::CommandBufferWriteNVX => (
            stage::COMMAND_PREPROCESS_NV,
            access::COMMAND_PREPROCESS_WRITE_NV,
        ),
        ty::VertexShaderWrite => (stage::VERTEX_SHADER, access::SHADER_WRITE),
        ty::TessellationControlShaderWrite => {
            (stage::TESSELLATION_CONTROL_SHADER, access::SHADER_WRITE)
        }
        ty::TessellationEvaluationShaderWrite => {
            (stage::TESSELLATION_EVALUATION_SHADER, access::SHADER_WRITE)
        }
        ty::GeometryShaderWrite => (stage::GEOMETRY_SHADER, access::SHADER_WRITE),
        ty::FragmentShaderWrite => (stage::FRAGMENT_SHADER, access::SHADER_WRITE),
        ty::ColorAttachmentWrite => (
            stage::COLOR_ATTACHMENT_OUTPUT,
            access::COLOR_ATTACHMENT_WRITE,
        ),
        ty::DepthStencilAttachmentWrite => (
            stage::from_raw(
                stage::EARLY_FRAGMENT_TESTS.as_raw()
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS.as_raw(),
            ),
            access::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ),
        ty::DepthAttachmentWriteStencilReadOnly => (
            stage::from_raw(
                stage::EARLY_FRAGMENT_TESTS.as_raw()
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS.as_raw(),
            ),
            access::from_raw(
                access::DEPTH_STENCIL_ATTACHMENT_WRITE.as_raw()
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ.as_raw(),
            ),
        ),
        ty::StencilAttachmentWriteDepthReadOnly => (
            stage::from_raw(
                stage::EARLY_FRAGMENT_TESTS.as_raw()
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS.as_raw(),
            ),
            access::from_raw(
                access::DEPTH_STENCIL_ATTACHMENT_WRITE.as_raw()
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ.as_raw(),
            ),
        ),
        ty::ComputeShaderWrite => (stage::COMPUTE_SHADER, access::SHADER_WRITE),
        ty::AnyShaderWrite => (stage::ALL_COMMANDS, access::SHADER_WRITE),
        ty::TransferWrite => (stage::TRANSFER, access::TRANSFER_WRITE),
        ty::HostWrite => (stage::HOST, access::HOST_WRITE),
        ty::ColorAttachmentReadWrite => (
            stage::COLOR_ATTACHMENT_OUTPUT,
            access::from_raw(
                access::COLOR_ATTACHMENT_READ.as_raw()
                    | vk::AccessFlags::COLOR_ATTACHMENT_WRITE.as_raw(),
            ),
        ),
        ty::General => (
            stage::ALL_COMMANDS,
            access::from_raw(access::MEMORY_READ.as_raw() | vk::AccessFlags::MEMORY_WRITE.as_raw()),
        ),
        ty::RayTracingShaderReadSampledImageOrUniformTexelBuffer => {
            (stage::RAY_TRACING_SHADER_KHR, access::SHADER_READ)
        }
        ty::RayTracingShaderReadColorInputAttachment => {
            (stage::RAY_TRACING_SHADER_KHR, access::INPUT_ATTACHMENT_READ)
        }
        ty::RayTracingShaderReadDepthStencilInputAttachment => {
            (stage::RAY_TRACING_SHADER_KHR, access::INPUT_ATTACHMENT_READ)
        }
        ty::RayTracingShaderReadAccelerationStructure => (
            stage::RAY_TRACING_SHADER_KHR,
            access::ACCELERATION_STRUCTURE_READ_KHR,
        ),
        ty::RayTracingShaderReadOther => (stage::RAY_TRACING_SHADER_KHR, access::SHADER_READ),
        ty::AccelerationStructureBuildWrite => (
            stage::ACCELERATION_STRUCTURE_BUILD_KHR,
            access::ACCELERATION_STRUCTURE_WRITE_KHR,
        ),
        ty::AccelerationStructureBuildRead => (
            stage::ACCELERATION_STRUCTURE_BUILD_KHR,
            access::ACCELERATION_STRUCTURE_READ_KHR,
        ),
        ty::AccelerationStructureBufferWrite => (
            stage::ACCELERATION_STRUCTURE_BUILD_KHR,
            access::TRANSFER_WRITE,
        ),
    }
}

#[derive(Debug)]
pub struct Driver<P>
where
    P: SharedPointerKind,
{
    pub device: SharedPointer<Device<P>, P>,
    pub swapchain: Swapchain<P>,
}

impl<P> Driver<P>
where
    P: SharedPointerKind,
{
    pub fn new(
        window: &impl HasRawWindowHandle,
        cfg: DriverConfig,
        width: u32,
        height: u32,
    ) -> Result<Self, DriverError> {
        trace!("new {:?}", cfg);

        let required_extensions = ash_window::enumerate_required_extensions(window)
            .map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?
            .iter()
            .map(|ext| unsafe { CStr::from_ptr(*ext as *const _) });
        let instance = SharedPointer::new(Instance::new(cfg.debug, required_extensions)?);
        let surface = Surface::new(&instance, window)?;
        let physical_devices = Instance::physical_devices(&instance)?
            .filter(|physical_device| {
                // Filters this list down to only supported devices
                if cfg.presentation
                    && !PhysicalDevice::has_presentation_support(
                        physical_device,
                        &instance,
                        &surface,
                    )
                {
                    info!("{:?} lacks presentation support", unsafe {
                        CStr::from_ptr(physical_device.props.device_name.as_ptr() as *const c_char)
                    });

                    return false;
                }

                if cfg.ray_tracing && !PhysicalDevice::has_ray_tracing_support(physical_device) {
                    info!("{:?} lacks ray tracing support", unsafe {
                        CStr::from_ptr(physical_device.props.device_name.as_ptr() as *const c_char)
                    });

                    return false;
                }

                // TODO: Check vkGetPhysicalDeviceFeatures for samplerAnisotropy (it should exist, but to be sure)

                true
            })
            .collect::<Vec<_>>();

        for physical_device in &physical_devices {
            debug!("supported: {:?}", physical_device);
        }

        let physical_device = physical_devices
            .into_iter()
            // If there are multiple devices with the same score, `max_by_key` would choose the last,
            // and we want to preserve the order of devices from `enumerate_physical_devices`.
            .rev()
            .max_by_key(PhysicalDevice::score_device_type)
            .ok_or(DriverError::Unsupported)?;

        debug!("selected: {:?}", physical_device);

        let device = SharedPointer::new(Device::create(&instance, physical_device, cfg)?);
        let surface_formats = Device::surface_formats(&device, &surface)?;

        for fmt in &surface_formats {
            debug!("surface: {:#?} ({:#?})", fmt.format, fmt.color_space);
        }

        // TODO: Explicitly fallback to BGRA_UNORM
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
                height,
                sync_display: cfg.sync_display,
                width,
            },
        )?;

        info!("OK");

        Ok(Self { device, swapchain })
    }

    fn select_swapchain_format(format: vk::SurfaceFormatKHR) -> bool {
        // TODO: Properly handle the request for SRGB and swapchain image usage flags: The device may not support SRGB and only in that case do we fall back to UNORM
        format.format == vk::Format::B8G8R8A8_UNORM
            && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    }
}

/// A list of required features. Features that are supported but not required will not be
/// available.
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned", derive(Debug))]
pub struct DriverConfig {
    /// Enables Vulkan validation layers.
    ///
    /// This requires a Vulkan SDK installation and will cause validation errors to introduce
    /// panics as they happen.
    ///
    /// _NOTE:_ Consider turning OFF debug if you discover an unknown issue. Often the validation
    /// layers will throw an error before other layers can provide additional context such as the
    /// API dump info or other messages. You might find the "actual" issue is detailed in those
    /// subsequent details.
    #[builder(default)]
    pub debug: bool,

    #[builder(default = "3")]
    pub desired_swapchain_image_count: u32,

    /// Determines if frames will be submitted to the display in a synchronous fashion or if they
    /// should be displayed as fast as possible instead.
    ///
    /// Turn on to eliminate visual tearing at the expense of latency.
    #[builder(default = "true")]
    pub sync_display: bool,

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
        FeatureFlags {
            presentation: true,
            ray_tracing: self.ray_tracing,
        }
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

#[derive(Debug)]
pub struct PhysicalDeviceRayTracePipelineProperties {
    pub shader_group_handle_size: u32,
    pub max_ray_recursion_depth: u32,
    pub max_shader_group_stride: u32,
    pub shader_group_base_alignment: u32,
    pub shader_group_handle_capture_replay_size: u32,
    pub max_ray_dispatch_invocation_count: u32,
    pub shader_group_handle_alignment: u32,
    pub max_ray_hit_attribute_size: u32,
}

impl From<vk::PhysicalDeviceRayTracingPipelinePropertiesKHR>
    for PhysicalDeviceRayTracePipelineProperties
{
    fn from(props: vk::PhysicalDeviceRayTracingPipelinePropertiesKHR) -> Self {
        Self {
            shader_group_handle_size: props.shader_group_handle_size,
            max_ray_recursion_depth: props.max_ray_recursion_depth,
            max_shader_group_stride: props.max_shader_group_stride,
            shader_group_base_alignment: props.shader_group_base_alignment,
            shader_group_handle_capture_replay_size: props.shader_group_handle_capture_replay_size,
            max_ray_dispatch_invocation_count: props.max_ray_dispatch_invocation_count,
            shader_group_handle_alignment: props.shader_group_handle_alignment,
            max_ray_hit_attribute_size: props.max_ray_hit_attribute_size,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SamplerDesc {
    pub address_modes: vk::SamplerAddressMode,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub texel_filter: vk::Filter,
}
