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
//! - [`Buffer`]
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
//! - [`GraphicPipeline`]
//! - [`RayTracePipeline`](ray_trace::RayTracePipeline)

pub mod accel_struct;
pub mod buffer;
pub mod compute;
pub mod device;
pub mod graphic;
pub mod image;
pub mod physical_device;
pub mod ray_trace;
pub mod shader;
pub mod surface;
pub mod swapchain;

mod cmd_buf;
mod descriptor_set;
mod descriptor_set_layout;
mod instance;
mod render_pass;

pub use {
    self::{cmd_buf::CommandBuffer, instance::Instance},
    ash::{self},
    vk_sync::AccessType,
};

pub(crate) use self::{
    cmd_buf::CommandBufferInfo,
    descriptor_set::{DescriptorPool, DescriptorPoolInfo, DescriptorSet},
    descriptor_set_layout::DescriptorSetLayout,
    render_pass::{
        AttachmentInfo, AttachmentRef, FramebufferAttachmentImageInfo, FramebufferInfo, RenderPass,
        RenderPassInfo, SubpassDependency, SubpassInfo,
    },
    shader::{DescriptorBinding, DescriptorBindingMap, DescriptorInfo},
    surface::Surface,
};

use {
    self::{
        buffer::{Buffer, BufferInfo},
        graphic::{DepthStencilMode, GraphicPipeline, VertexInputState},
        image::SampleCount,
    },
    ash::vk,
    std::{
        cmp::Ordering,
        error::Error,
        fmt::{Display, Formatter},
        ops::Range,
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
        debug_assert_ne!(0, region.buffer_row_length);
        debug_assert_ne!(0, region.buffer_image_height);

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
