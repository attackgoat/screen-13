//! [Vulkan 1.2](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/index.html) interface
//! based on smart pointers.
//!
//! # Resources
//!
//! Each resource contains an opaque Vulkan object handle and an information structure which
//! describes the object. Resources also contain an atomic [`AccessType`] state value which is used to
//! maintain consistency in any system which accesses the resource.
//!
//! The following resources are available:
//!
//! - [`AccelerationStructure`](accel_struct::AccelerationStructure)
//! - [`Buffer`](buffer::Buffer)
//! - [`Image`](image::Image)
//!
//! # Pipelines
//!
//! Pipelines allow you to run shader code which read and write resources using graphics hardware.
//!
//! Each pipeline contains an opaque Vulkan object handle and an information structure which
//! describes the configuration and shaders. They are immutable once created.
//!
//! The following pipelines are available:
//!
//! - [`ComputePipeline`](compute::ComputePipeline)
//! - [`GraphicPipeline`](graphic::GraphicPipeline)
//! - [`RayTracePipeline`](ray_trace::RayTracePipeline)

pub mod accel_struct;
pub mod buffer;
pub mod compute;
pub mod graphic;
pub mod image;
pub mod ray_trace;
pub mod shader;

mod cmd_buf;
mod descriptor_set;
mod descriptor_set_layout;
mod device;
mod instance;
mod physical_device;
mod render_pass;
mod surface;
mod swapchain;

pub use {
    self::{
        device::{Device, FeatureFlags},
        physical_device::PhysicalDevice,
    },
    ash::{self},
    vk_sync::AccessType,
};

pub(crate) use self::{
    cmd_buf::{CommandBuffer, CommandBufferInfo},
    descriptor_set::{DescriptorPool, DescriptorPoolInfo, DescriptorSet},
    descriptor_set_layout::DescriptorSetLayout,
    instance::Instance,
    physical_device::{QueueFamily, QueueFamilyProperties},
    render_pass::{
        AttachmentInfo, AttachmentRef, FramebufferAttachmentImageInfo, FramebufferInfo, RenderPass,
        RenderPassInfo, SubpassDependency, SubpassInfo,
    },
    shader::{DescriptorBinding, DescriptorBindingMap, DescriptorInfo},
    surface::Surface,
    swapchain::{Swapchain, SwapchainImage, SwapchainInfo},
};

use {
    self::{
        buffer::{Buffer, BufferInfo},
        graphic::{DepthStencilMode, GraphicPipeline, VertexInputState},
        image::{Image, ImageInfo, ImageType, SampleCount},
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    log::{debug, info, trace, warn},
    raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle},
    std::{
        cmp::Ordering,
        error::Error,
        ffi::CStr,
        fmt::{Display, Formatter},
        ops::{Deref, Range},
        os::raw::c_char,
        sync::Arc,
    },
    vk_sync::ImageLayout,
};

const fn access_type_from_u8(access: u8) -> AccessType {
    match access {
        0 => AccessType::Nothing,
        1 => AccessType::CommandBufferReadNVX,
        2 => AccessType::IndirectBuffer,
        3 => AccessType::IndexBuffer,
        4 => AccessType::VertexBuffer,
        5 => AccessType::VertexShaderReadUniformBuffer,
        6 => AccessType::VertexShaderReadSampledImageOrUniformTexelBuffer,
        7 => AccessType::VertexShaderReadOther,
        8 => AccessType::TessellationControlShaderReadUniformBuffer,
        9 => AccessType::TessellationControlShaderReadSampledImageOrUniformTexelBuffer,
        10 => AccessType::TessellationControlShaderReadOther,
        11 => AccessType::TessellationEvaluationShaderReadUniformBuffer,
        12 => AccessType::TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer,
        13 => AccessType::TessellationEvaluationShaderReadOther,
        14 => AccessType::GeometryShaderReadUniformBuffer,
        15 => AccessType::GeometryShaderReadSampledImageOrUniformTexelBuffer,
        16 => AccessType::GeometryShaderReadOther,
        17 => AccessType::FragmentShaderReadUniformBuffer,
        18 => AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
        19 => AccessType::FragmentShaderReadColorInputAttachment,
        20 => AccessType::FragmentShaderReadDepthStencilInputAttachment,
        21 => AccessType::FragmentShaderReadOther,
        22 => AccessType::ColorAttachmentRead,
        23 => AccessType::DepthStencilAttachmentRead,
        24 => AccessType::ComputeShaderReadUniformBuffer,
        25 => AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer,
        26 => AccessType::ComputeShaderReadOther,
        27 => AccessType::AnyShaderReadUniformBuffer,
        28 => AccessType::AnyShaderReadUniformBufferOrVertexBuffer,
        29 => AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer,
        30 => AccessType::AnyShaderReadOther,
        31 => AccessType::TransferRead,
        32 => AccessType::HostRead,
        33 => AccessType::Present,
        34 => AccessType::CommandBufferWriteNVX,
        35 => AccessType::VertexShaderWrite,
        36 => AccessType::TessellationControlShaderWrite,
        37 => AccessType::TessellationEvaluationShaderWrite,
        38 => AccessType::GeometryShaderWrite,
        39 => AccessType::FragmentShaderWrite,
        40 => AccessType::ColorAttachmentWrite,
        41 => AccessType::DepthStencilAttachmentWrite,
        42 => AccessType::DepthAttachmentWriteStencilReadOnly,
        43 => AccessType::StencilAttachmentWriteDepthReadOnly,
        44 => AccessType::ComputeShaderWrite,
        45 => AccessType::AnyShaderWrite,
        46 => AccessType::TransferWrite,
        47 => AccessType::HostWrite,
        48 => AccessType::ColorAttachmentReadWrite,
        49 => AccessType::General,
        50 => AccessType::RayTracingShaderReadSampledImageOrUniformTexelBuffer,
        51 => AccessType::RayTracingShaderReadColorInputAttachment,
        52 => AccessType::RayTracingShaderReadDepthStencilInputAttachment,
        53 => AccessType::RayTracingShaderReadAccelerationStructure,
        54 => AccessType::RayTracingShaderReadOther,
        55 => AccessType::AccelerationStructureBuildWrite,
        56 => AccessType::AccelerationStructureBuildRead,
        57 => AccessType::AccelerationStructureBufferWrite,
        _ => unimplemented!(),
    }
}

const fn access_type_into_u8(access: AccessType) -> u8 {
    match access {
        AccessType::Nothing => 0,
        AccessType::CommandBufferReadNVX => 1,
        AccessType::IndirectBuffer => 2,
        AccessType::IndexBuffer => 3,
        AccessType::VertexBuffer => 4,
        AccessType::VertexShaderReadUniformBuffer => 5,
        AccessType::VertexShaderReadSampledImageOrUniformTexelBuffer => 6,
        AccessType::VertexShaderReadOther => 7,
        AccessType::TessellationControlShaderReadUniformBuffer => 8,
        AccessType::TessellationControlShaderReadSampledImageOrUniformTexelBuffer => 9,
        AccessType::TessellationControlShaderReadOther => 10,
        AccessType::TessellationEvaluationShaderReadUniformBuffer => 11,
        AccessType::TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer => 12,
        AccessType::TessellationEvaluationShaderReadOther => 13,
        AccessType::GeometryShaderReadUniformBuffer => 14,
        AccessType::GeometryShaderReadSampledImageOrUniformTexelBuffer => 15,
        AccessType::GeometryShaderReadOther => 16,
        AccessType::FragmentShaderReadUniformBuffer => 17,
        AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer => 18,
        AccessType::FragmentShaderReadColorInputAttachment => 19,
        AccessType::FragmentShaderReadDepthStencilInputAttachment => 20,
        AccessType::FragmentShaderReadOther => 21,
        AccessType::ColorAttachmentRead => 22,
        AccessType::DepthStencilAttachmentRead => 23,
        AccessType::ComputeShaderReadUniformBuffer => 24,
        AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer => 25,
        AccessType::ComputeShaderReadOther => 26,
        AccessType::AnyShaderReadUniformBuffer => 27,
        AccessType::AnyShaderReadUniformBufferOrVertexBuffer => 28,
        AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer => 29,
        AccessType::AnyShaderReadOther => 30,
        AccessType::TransferRead => 31,
        AccessType::HostRead => 32,
        AccessType::Present => 33,
        AccessType::CommandBufferWriteNVX => 34,
        AccessType::VertexShaderWrite => 35,
        AccessType::TessellationControlShaderWrite => 36,
        AccessType::TessellationEvaluationShaderWrite => 37,
        AccessType::GeometryShaderWrite => 38,
        AccessType::FragmentShaderWrite => 39,
        AccessType::ColorAttachmentWrite => 40,
        AccessType::DepthStencilAttachmentWrite => 41,
        AccessType::DepthAttachmentWriteStencilReadOnly => 42,
        AccessType::StencilAttachmentWriteDepthReadOnly => 43,
        AccessType::ComputeShaderWrite => 44,
        AccessType::AnyShaderWrite => 45,
        AccessType::TransferWrite => 46,
        AccessType::HostWrite => 47,
        AccessType::ColorAttachmentReadWrite => 48,
        AccessType::General => 49,
        AccessType::RayTracingShaderReadSampledImageOrUniformTexelBuffer => 50,
        AccessType::RayTracingShaderReadColorInputAttachment => 51,
        AccessType::RayTracingShaderReadDepthStencilInputAttachment => 52,
        AccessType::RayTracingShaderReadAccelerationStructure => 53,
        AccessType::RayTracingShaderReadOther => 54,
        AccessType::AccelerationStructureBuildWrite => 55,
        AccessType::AccelerationStructureBuildRead => 56,
        AccessType::AccelerationStructureBufferWrite => 57,
    }
}

#[allow(clippy::reversed_empty_ranges)]
pub(super) fn buffer_copy_subresources(
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
pub(super) fn buffer_image_copy_subresource(
    regions: &[vk::BufferImageCopy],
) -> Range<vk::DeviceSize> {
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

pub(super) const fn format_aspect_mask(fmt: vk::Format) -> vk::ImageAspectFlags {
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

pub(super) const fn image_access_layout(access: AccessType) -> ImageLayout {
    if matches!(access, AccessType::Present | AccessType::ComputeShaderWrite) {
        ImageLayout::General
    } else {
        ImageLayout::Optimal
    }
}

pub(super) const fn is_framebuffer_access(ty: AccessType) -> bool {
    matches!(
        ty,
        AccessType::ColorAttachmentRead
            | AccessType::ColorAttachmentReadWrite
            | AccessType::ColorAttachmentWrite
            | AccessType::DepthAttachmentWriteStencilReadOnly
            | AccessType::DepthStencilAttachmentRead
            | AccessType::DepthStencilAttachmentWrite
            | AccessType::FragmentShaderReadColorInputAttachment
            | AccessType::FragmentShaderReadDepthStencilInputAttachment
            | AccessType::StencilAttachmentWriteDepthReadOnly
    )
}

pub(super) const fn is_read_access(ty: AccessType) -> bool {
    !is_write_access(ty)
}

pub(super) const fn is_write_access(ty: AccessType) -> bool {
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

// Convert overlapping push constant regions such as this:
// VERTEX 0..64
// FRAGMENT 0..80
//
// To this:
// VERTEX | FRAGMENT 0..64
// FRAGMENT 64..80
//
// We do this so that submission doesn't need to check for overlaps
// See https://github.com/KhronosGroup/Vulkan-Docs/issues/609
fn merge_push_constant_ranges(pcr: &[vk::PushConstantRange]) -> Vec<vk::PushConstantRange> {
    // Each specified range must be for a single stage and each stage must be specified once
    #[cfg(debug_assertions)]
    {
        let mut stage_flags = vk::ShaderStageFlags::empty();
        for item in pcr.iter() {
            assert_eq!(item.stage_flags.as_raw().count_ones(), 1);
            assert!(!stage_flags.contains(item.stage_flags));
            assert!(item.size > 0);

            stage_flags |= item.stage_flags;
        }
    }

    match pcr.len() {
        0 => vec![],
        1 => vec![pcr[0]],
        _ => {
            let mut res = pcr.to_vec();
            let sort_fn = |lhs: &vk::PushConstantRange, rhs: &vk::PushConstantRange| match lhs
                .offset
                .cmp(&rhs.offset)
            {
                Ordering::Equal => lhs.size.cmp(&rhs.size),
                res => res,
            };

            res.sort_unstable_by(sort_fn);

            let mut i = 0;
            let mut j = 1;

            while j < res.len() {
                let lhs = res[i];
                let rhs = res[j];

                if lhs.offset == rhs.offset && lhs.size == rhs.size {
                    res[i].stage_flags |= rhs.stage_flags;
                    res.remove(j);
                } else if lhs.offset == rhs.offset {
                    res[i].stage_flags |= rhs.stage_flags;
                    res[j].offset += lhs.size;
                    res[j].size -= lhs.size;
                    res[j..].sort_unstable_by(sort_fn);
                } else if lhs.offset + lhs.size > rhs.offset + rhs.size {
                    res[i].size = rhs.offset - lhs.offset;
                    res[j].stage_flags = lhs.stage_flags;
                    res[j].offset += rhs.size;
                    res[j].size = (lhs.offset + lhs.size) - (rhs.offset + rhs.size);
                    res.insert(
                        j,
                        vk::PushConstantRange {
                            stage_flags: lhs.stage_flags | rhs.stage_flags,
                            offset: rhs.offset,
                            size: rhs.size,
                        },
                    );
                    i += 1;
                    j += 1;
                } else if lhs.offset + lhs.size == rhs.offset + rhs.size {
                    res[i].size -= rhs.size;
                    res[j].stage_flags |= lhs.stage_flags;
                    i += 1;
                    j += 1;
                } else if lhs.offset + lhs.size > rhs.offset
                    && lhs.offset + lhs.size < rhs.offset + rhs.size
                {
                    res[i].size = rhs.offset - lhs.offset;
                    res[j].offset = lhs.offset + lhs.size;
                    res[j].size = (rhs.offset + rhs.size) - (lhs.offset + lhs.size);
                    res.insert(
                        j,
                        vk::PushConstantRange {
                            stage_flags: lhs.stage_flags | rhs.stage_flags,
                            offset: rhs.offset,
                            size: (lhs.offset + lhs.size) - rhs.offset,
                        },
                    );
                    res[j..].sort_unstable_by(sort_fn);
                } else {
                    i += 1;
                    j += 1;
                }
            }

            res
        }
    }
}

pub(super) const fn pipeline_stage_access_flags(
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

/// Holds a constructed graphics driver.
#[derive(Debug)]
pub struct Driver {
    /// The current device.
    pub device: Arc<Device>,

    pub(crate) swapchain: Swapchain,
}

impl Driver {
    /// Constructs a new `Driver` from the given configuration.
    pub fn new(
        display_window: &(impl HasRawDisplayHandle + HasRawWindowHandle),
        cfg: DriverConfig,
        width: u32,
        height: u32,
    ) -> Result<Self, DriverError> {
        trace!("new {:?}", cfg);

        let required_extensions =
            ash_window::enumerate_required_extensions(display_window.raw_display_handle())
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
                .iter()
                .map(|ext| unsafe { CStr::from_ptr(*ext as *const _) });
        let instance = Arc::new(Instance::new(cfg.debug, required_extensions)?);
        let surface = Surface::new(&instance, display_window)?;
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

        let device = Arc::new(Device::create(&instance, physical_device, cfg)?);
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
#[builder(
    pattern = "owned",
    build_fn(private, name = "fallible_build", error = "DriverConfigBuilderError"),
    derive(Debug)
)]
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

    /// The desired, but not guaranteed, number of queues that will be available.
    ///
    /// Additional queues are useful for submission from secondary threads.
    #[builder(default = "1")]
    pub desired_queue_count: usize,

    /// The desired, but not guaranteed, number of images that will be in the created swapchain.
    ///
    /// More images introduces more display lag, but smoother animation.
    #[builder(default = "3")]
    pub desired_swapchain_image_count: u32,

    /// Determines if frames will be submitted to the display in a synchronous fashion or if they
    /// should be displayed as fast as possible instead.
    ///
    /// Turn on to eliminate visual tearing at the expense of latency.
    #[builder(default = "true")]
    pub sync_display: bool,

    /// Used to select devices which support presentation to the display.
    ///
    /// The default value is `true`.
    #[builder(default = "true")]
    pub presentation: bool,

    /// Used to select devices which support the [KHR ray tracing] extension.
    ///
    /// The default is `false`.
    ///
    /// [KHR ray tracing]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#ray-tracing
    #[builder(default)]
    pub ray_tracing: bool,
}

impl DriverConfig {
    /// Specifies a default driver configuration.
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

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl DriverConfigBuilder {
    /// Builds a new `DriverConfig`.
    pub fn build(self) -> DriverConfig {
        self.fallible_build().unwrap()
    }
}

#[derive(Debug)]
struct DriverConfigBuilderError;

impl From<UninitializedFieldError> for DriverConfigBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

/// Describes the general category of all graphics driver failure cases.
///
/// In the event of a failure you should follow the _Screen 13_ code to the responsible Vulkan API
/// and then to the `Ash` stub call; it will generally contain a link to the appropriate
/// specification. The specifications provide a table of possible error conditions which can be a
/// good starting point to debug the issue.
///
/// Feel free to open an issue on GitHub, [here](https://github.com/attackgoat/screen-13/issues) for
/// help debugging the issue.
#[derive(Debug)]
pub enum DriverError {
    /// The input data, or referenced data, is not valid for the current state.
    InvalidData,

    /// The requested feature, or input configuration, is not supported for the current state.
    Unsupported,

    /// The device has run out of physical memory.
    ///
    /// Many drivers return this value for generic or unhandled error conditions.
    OutOfMemory,
}

impl Display for DriverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for DriverError {}

/// Structure describing depth/stencil resolve properties that can be supported by an
/// implementation.
#[derive(Debug)]
pub struct PhysicalDeviceDepthStencilResolveProperties {
    /// A bitmask indicating the set of supported depth resolve modes.
    ///
    /// `VK_RESOLVE_MODE_SAMPLE_ZERO_BIT` must be included in the set but implementations may
    /// support additional modes.
    pub supported_depth_resolve_modes: vk::ResolveModeFlags,

    /// A bitmask of indicating the set of supported stencil resolve modes.
    ///
    /// `VK_RESOLVE_MODE_SAMPLE_ZERO_BIT` must be included in the set but implementations may
    /// support additional modes. `VK_RESOLVE_MODE_AVERAGE_BIT` must not be included in the set.
    pub supported_stencil_resolve_modes: vk::ResolveModeFlags,

    /// `true` if the implementation supports setting the depth and stencil resolve modes to
    /// different values when one of those modes is `VK_RESOLVE_MODE_NONE`. Otherwise the
    /// implementation only supports setting both modes to the same value.
    pub independent_resolve_none: bool,

    /// `true` if the implementation supports all combinations of the supported depth and stencil
    /// resolve modes, including setting either depth or stencil resolve mode to
    /// `VK_RESOLVE_MODE_NONE`.
    ///
    /// An implementation that supports `independent_resolve` must also support
    /// `independent_resolve_none`.
    pub independent_resolve: bool,
}

impl From<vk::PhysicalDeviceDepthStencilResolveProperties>
    for PhysicalDeviceDepthStencilResolveProperties
{
    fn from(props: vk::PhysicalDeviceDepthStencilResolveProperties) -> Self {
        Self {
            supported_depth_resolve_modes: props.supported_depth_resolve_modes,
            supported_stencil_resolve_modes: props.supported_stencil_resolve_modes,
            independent_resolve_none: props.independent_resolve_none == vk::TRUE,
            independent_resolve: props.independent_resolve == vk::TRUE,
        }
    }
}

/// Structure describing descriptor indexing features that can be supported by an implementation.
pub struct PhysicalDeviceDescriptorIndexingFeatures {
    /// Indicates whether arrays of input attachments can be indexed by dynamically uniform integer
    /// expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_INPUT_ATTACHMENT must be indexed only by constant integral expressions
    /// when aggregated into arrays in shader code. This also indicates whether shader modules can
    /// declare the InputAttachmentArrayDynamicIndexing capability.
    pub shader_input_attachment_array_dynamic_indexing: bool,

    /// Indicates whether arrays of uniform texel buffers can be indexed by dynamically uniform
    /// integer expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_UNIFORM_TEXEL_BUFFER must be indexed only by constant integral
    /// expressions when aggregated into arrays in shader code. This also indicates whether shader
    /// modules can declare the UniformTexelBufferArrayDynamicIndexing capability.
    pub shader_uniform_texel_buffer_array_dynamic_indexing: bool,

    /// Indicates whether arrays of storage texel buffers can be indexed by dynamically uniform
    /// integer expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_STORAGE_TEXEL_BUFFER must be indexed only by constant integral
    /// expressions when aggregated into arrays in shader code. This also indicates whether shader
    /// modules can declare the StorageTexelBufferArrayDynamicIndexing capability.
    pub shader_storage_texel_buffer_array_dynamic_indexing: bool,

    /// Indicates whether arrays of uniform buffers can be indexed by non-uniform integer
    /// expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER or VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER_DYNAMIC must not be
    /// indexed by non-uniform integer expressions when aggregated into arrays in shader code. This
    /// also indicates whether shader modules can declare the UniformBufferArrayNonUniformIndexing
    /// capability.
    pub shader_uniform_buffer_array_non_uniform_indexing: bool,

    /// Indicates whether arrays of samplers or sampled images can be indexed by non-uniform integer
    /// expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_SAMPLER, VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, or
    /// VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE must not be indexed by non-uniform integer expressions when
    /// aggregated into arrays in shader code. This also indicates whether shader modules can
    /// declare the SampledImageArrayNonUniformIndexing capability.
    pub shader_sampled_image_array_non_uniform_indexing: bool,

    /// Indicates whether arrays of storage buffers can be indexed by non-uniform integer
    /// expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_STORAGE_BUFFER or VK_DESCRIPTOR_TYPE_STORAGE_BUFFER_DYNAMIC must not be
    /// indexed by non-uniform integer expressions when aggregated into arrays in shader code. This
    /// also indicates whether shader modules can declare the StorageBufferArrayNonUniformIndexing
    /// capability.
    pub shader_storage_buffer_array_non_uniform_indexing: bool,

    /// Indicates whether arrays of storage images can be indexed by non-uniform integer expressions
    /// in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_STORAGE_IMAGE must not be indexed by non-uniform integer expressions when
    /// aggregated into arrays in shader code. This also indicates whether shader modules can
    /// declare the StorageImageArrayNonUniformIndexing capability.
    pub shader_storage_image_array_non_uniform_indexing: bool,

    /// Indicates whether arrays of input attachments can be indexed by non-uniform integer
    /// expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_INPUT_ATTACHMENT must not be indexed by non-uniform integer expressions
    /// when aggregated into arrays in shader code. This also indicates whether shader modules can
    /// declare the InputAttachmentArrayNonUniformIndexing capability.
    pub shader_input_attachment_array_non_uniform_indexing: bool,

    /// Indicates whether arrays of uniform texel buffers can be indexed by non-uniform integer
    /// expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_UNIFORM_TEXEL_BUFFER must not be indexed by non-uniform integer
    /// expressions when aggregated into arrays in shader code. This also indicates whether shader
    /// modules can declare the UniformTexelBufferArrayNonUniformIndexing capability.
    pub shader_uniform_texel_buffer_array_non_uniform_indexing: bool,

    /// Indicates whether arrays of storage texel buffers can be indexed by non-uniform integer
    /// expressions in shader code.
    ///
    /// If this feature is not enabled, resources with a descriptor type of
    /// VK_DESCRIPTOR_TYPE_STORAGE_TEXEL_BUFFER must not be indexed by non-uniform integer
    /// expressions when aggregated into arrays in shader code. This also indicates whether shader
    /// modules can declare the StorageTexelBufferArrayNonUniformIndexing capability.
    pub shader_storage_texel_buffer_array_non_uniform_indexing: bool,

    // Unused:
    // pub descriptor_binding_uniform_buffer_update_after_bind: bool,
    // pub descriptor_binding_sampled_image_update_after_bind: bool,
    // pub descriptor_binding_storage_image_update_after_bind: bool,
    // pub descriptor_binding_storage_buffer_update_after_bind: bool,
    // pub descriptor_binding_uniform_texel_buffer_update_after_bind: bool,
    // pub descriptor_binding_storage_texel_buffer_update_after_bind: bool,
    // pub descriptor_binding_update_unused_while_pending: bool,
    /// Indicates whether the implementation supports statically using a descriptor set binding in
    /// which some descriptors are not valid. If this feature is not enabled,
    /// VK_DESCRIPTOR_BINDING_PARTIALLY_BOUND_BIT must not be used.
    pub descriptor_binding_partially_bound: bool,

    /// Indicates whether the implementation supports descriptor sets with a variable-sized last
    /// binding. If this feature is not enabled, VK_DESCRIPTOR_BINDING_VARIABLE_DESCRIPTOR_COUNT_BIT
    /// must not be used.
    pub descriptor_binding_variable_descriptor_count: bool,

    /// Indicates whether the implementation supports the SPIR-V RuntimeDescriptorArray capability.
    ///
    /// If this feature is not enabled, descriptors must not be declared in runtime arrays.
    pub runtime_descriptor_array: bool,
}

impl From<vk::PhysicalDeviceDescriptorIndexingFeatures>
    for PhysicalDeviceDescriptorIndexingFeatures
{
    fn from(features: vk::PhysicalDeviceDescriptorIndexingFeatures) -> Self {
        Self {
            shader_input_attachment_array_dynamic_indexing: features
                .shader_input_attachment_array_dynamic_indexing
                == vk::TRUE,
            shader_uniform_texel_buffer_array_dynamic_indexing: features
                .shader_uniform_texel_buffer_array_dynamic_indexing
                == vk::TRUE,
            shader_storage_texel_buffer_array_dynamic_indexing: features
                .shader_storage_texel_buffer_array_dynamic_indexing
                == vk::TRUE,
            shader_uniform_buffer_array_non_uniform_indexing: features
                .shader_uniform_buffer_array_non_uniform_indexing
                == vk::TRUE,
            shader_sampled_image_array_non_uniform_indexing: features
                .shader_sampled_image_array_non_uniform_indexing
                == vk::TRUE,
            shader_storage_buffer_array_non_uniform_indexing: features
                .shader_storage_buffer_array_non_uniform_indexing
                == vk::TRUE,
            shader_storage_image_array_non_uniform_indexing: features
                .shader_storage_image_array_non_uniform_indexing
                == vk::TRUE,
            shader_input_attachment_array_non_uniform_indexing: features
                .shader_input_attachment_array_non_uniform_indexing
                == vk::TRUE,
            shader_uniform_texel_buffer_array_non_uniform_indexing: features
                .shader_uniform_texel_buffer_array_non_uniform_indexing
                == vk::TRUE,
            shader_storage_texel_buffer_array_non_uniform_indexing: features
                .shader_storage_texel_buffer_array_non_uniform_indexing
                == vk::TRUE,
            // descriptor_binding_uniform_buffer_update_after_bind: features
            //     .descriptor_binding_uniform_buffer_update_after_bind
            //     == vk::TRUE,
            // descriptor_binding_sampled_image_update_after_bind: features
            //     .descriptor_binding_sampled_image_update_after_bind
            //     == vk::TRUE,
            // descriptor_binding_storage_image_update_after_bind: features
            //     .descriptor_binding_storage_image_update_after_bind
            //     == vk::TRUE,
            // descriptor_binding_storage_buffer_update_after_bind: features
            //     .descriptor_binding_storage_buffer_update_after_bind
            //     == vk::TRUE,
            // descriptor_binding_uniform_texel_buffer_update_after_bind: features
            //     .descriptor_binding_uniform_texel_buffer_update_after_bind
            //     == vk::TRUE,
            // descriptor_binding_storage_texel_buffer_update_after_bind: features
            //     .descriptor_binding_storage_texel_buffer_update_after_bind
            //     == vk::TRUE,
            // descriptor_binding_update_unused_while_pending: features
            //     .descriptor_binding_update_unused_while_pending
            //     == vk::TRUE,
            descriptor_binding_partially_bound: features.descriptor_binding_partially_bound
                == vk::TRUE,
            descriptor_binding_variable_descriptor_count: features
                .descriptor_binding_variable_descriptor_count
                == vk::TRUE,
            runtime_descriptor_array: features.runtime_descriptor_array == vk::TRUE,
        }
    }
}

/// Properties of the physical device for ray tracing.
#[derive(Debug)]
pub struct PhysicalDeviceRayTracePipelineProperties {
    /// The size in bytes of the shader header.
    pub shader_group_handle_size: u32,

    /// The maximum number of levels of ray recursion allowed in a trace command.
    pub max_ray_recursion_depth: u32,

    /// The maximum stride in bytes allowed between shader groups in the shader binding table.
    pub max_shader_group_stride: u32,

    /// The required alignment in bytes for the base of the shader binding table.
    pub shader_group_base_alignment: u32,

    /// The number of bytes for the information required to do capture and replay for shader group
    /// handles.
    pub shader_group_handle_capture_replay_size: u32,

    /// The maximum number of ray generation shader invocations which may be produced by a single
    /// vkCmdTraceRaysIndirectKHR or vkCmdTraceRaysKHR command.
    pub max_ray_dispatch_invocation_count: u32,

    /// The required alignment in bytes for each shader binding table entry.
    ///
    /// The value must be a power of two.
    pub shader_group_handle_alignment: u32,

    /// The maximum size in bytes for a ray attribute structure.
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

/// An execution queue.
pub struct Queue {
    queue: vk::Queue,

    /// Properties of the family which this queue belongs to.
    pub family: QueueFamily,
}

impl Deref for Queue {
    type Target = vk::Queue;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}

/// Specifying depth and stencil resolve modes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolveMode {
    /// The result of the resolve operation is the average of the sample values.
    Average,

    /// The result of the resolve operation is the maximum of the sample values.
    Maximum,

    /// The result of the resolve operation is the minimum of the sample values.
    Minimum,

    /// The result of the resolve operation is equal to the value of sample `0`.
    SampleZero,
}

impl ResolveMode {
    fn into_vk(mode: Option<ResolveMode>) -> vk::ResolveModeFlags {
        match mode {
            None => vk::ResolveModeFlags::NONE,
            Some(ResolveMode::Average) => vk::ResolveModeFlags::AVERAGE,
            Some(ResolveMode::Maximum) => vk::ResolveModeFlags::MAX,
            Some(ResolveMode::Minimum) => vk::ResolveModeFlags::MIN,
            Some(ResolveMode::SampleZero) => vk::ResolveModeFlags::SAMPLE_ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct SamplerDesc {
    pub address_modes: vk::SamplerAddressMode,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub texel_filter: vk::Filter,
}

#[cfg(test)]
mod tests {
    use {super::merge_push_constant_ranges, ash::vk};

    macro_rules! assert_pcr_eq {
        ($lhs: expr, $rhs: expr,) => {
            assert_eq!($lhs.stage_flags, $rhs.stage_flags, "Stages flags not equal");
            assert_eq!($lhs.offset, $rhs.offset, "Offset not equal");
            assert_eq!($lhs.size, $rhs.size, "Size not equal");
        };
    }

    #[test]
    pub fn push_constant_ranges_complex() {
        let res = merge_push_constant_ranges(&[
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 8,
                size: 16,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::GEOMETRY,
                offset: 20,
                size: 48,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::TESSELLATION_CONTROL,
                offset: 24,
                size: 8,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::TESSELLATION_EVALUATION,
                offset: 28,
                size: 32,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 40,
                size: 128,
            },
        ]);

        assert_eq!(res.len(), 8);
        assert_pcr_eq!(
            res[0],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 8,
                size: 12,
            },
        );
        assert_pcr_eq!(
            res[1],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::GEOMETRY,
                offset: 20,
                size: 4,
            },
        );
        assert_pcr_eq!(
            res[2],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::TESSELLATION_CONTROL
                    | vk::ShaderStageFlags::GEOMETRY,
                offset: 24,
                size: 4,
            },
        );
        assert_pcr_eq!(
            res[3],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::TESSELLATION_CONTROL
                    | vk::ShaderStageFlags::TESSELLATION_EVALUATION
                    | vk::ShaderStageFlags::GEOMETRY,
                offset: 28,
                size: 4,
            },
        );
        assert_pcr_eq!(
            res[4],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::GEOMETRY
                    | vk::ShaderStageFlags::TESSELLATION_EVALUATION,
                offset: 32,
                size: 8,
            },
        );
        assert_pcr_eq!(
            res[5],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::GEOMETRY
                    | vk::ShaderStageFlags::TESSELLATION_EVALUATION
                    | vk::ShaderStageFlags::FRAGMENT,
                offset: 40,
                size: 20,
            },
        );
        assert_pcr_eq!(
            res[6],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::GEOMETRY,
                offset: 60,
                size: 8,
            },
        );
        assert_pcr_eq!(
            res[7],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 68,
                size: 100,
            },
        );
    }

    #[test]
    pub fn push_constant_ranges_disjoint() {
        let res = merge_push_constant_ranges(&[
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 32,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 32,
                size: 64,
            },
        ]);

        assert_eq!(res.len(), 2);
        assert_pcr_eq!(
            res[0],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 32,
            },
        );
        assert_pcr_eq!(
            res[1],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 32,
                size: 64,
            },
        );
    }

    #[test]
    pub fn push_constant_ranges_equal() {
        let res = merge_push_constant_ranges(&[
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 32,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 0,
                size: 32,
            },
        ]);

        assert_eq!(res.len(), 1);
        assert_pcr_eq!(
            res[0],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                offset: 0,
                size: 32,
            },
        );
    }

    #[test]
    pub fn push_constant_ranges_overlap() {
        let res = merge_push_constant_ranges(&[
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 24,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::GEOMETRY,
                offset: 8,
                size: 24,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 20,
                size: 28,
            },
        ]);

        assert_eq!(res.len(), 5);
        assert_pcr_eq!(
            res[0],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 8,
            },
        );
        assert_pcr_eq!(
            res[1],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::GEOMETRY,
                offset: 8,
                size: 12,
            },
        );
        assert_pcr_eq!(
            res[2],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX
                    | vk::ShaderStageFlags::GEOMETRY
                    | vk::ShaderStageFlags::FRAGMENT,
                offset: 20,
                size: 4,
            },
        );
        assert_pcr_eq!(
            res[3],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::GEOMETRY | vk::ShaderStageFlags::FRAGMENT,
                offset: 24,
                size: 8,
            },
        );
        assert_pcr_eq!(
            res[4],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 32,
                size: 16,
            },
        );
    }

    #[test]
    pub fn push_constant_ranges_subset() {
        let res = merge_push_constant_ranges(&[
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 64,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 16,
                size: 8,
            },
        ]);

        assert_eq!(res.len(), 3);
        assert_pcr_eq!(
            res[0],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 16,
            },
        );
        assert_pcr_eq!(
            res[1],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                offset: 16,
                size: 8,
            },
        );
        assert_pcr_eq!(
            res[2],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 24,
                size: 40,
            },
        );
    }

    #[test]
    pub fn push_constant_ranges_superset() {
        let res = merge_push_constant_ranges(&[
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: 64,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 0,
                size: 80,
            },
        ]);

        assert_eq!(res.len(), 2);
        assert_pcr_eq!(
            res[0],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                offset: 0,
                size: 64,
            },
        );
        assert_pcr_eq!(
            res[1],
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: 64,
                size: 16,
            },
        );
    }
}
