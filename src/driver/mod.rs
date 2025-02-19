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
pub mod render_pass;
pub mod shader;
pub mod surface;
pub mod swapchain;

mod cmd_buf;
mod descriptor_set;
mod descriptor_set_layout;
mod instance;

pub use {
    self::{cmd_buf::CommandBuffer, instance::Instance},
    ash::{self},
    vk_sync::AccessType,
};

/// Specifying depth and stencil resolve modes.
#[deprecated = "Use driver::render_pass::ResolveMode instead"]
pub type ResolveMode = self::render_pass::ResolveMode;

pub(crate) use self::{
    cmd_buf::CommandBufferInfo,
    descriptor_set::{DescriptorPool, DescriptorPoolInfo, DescriptorSet},
    descriptor_set_layout::DescriptorSetLayout,
    render_pass::{
        AttachmentInfo, AttachmentRef, FramebufferAttachmentImageInfo, FramebufferInfo, RenderPass,
        RenderPassInfo, SubpassDependency, SubpassInfo,
    },
    shader::{Descriptor, DescriptorBindingMap, DescriptorInfo},
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
    },
    vk_sync::ImageLayout,
};

pub(super) const fn format_aspect_mask(fmt: vk::Format) -> vk::ImageAspectFlags {
    match fmt {
        vk::Format::D16_UNORM | vk::Format::D32_SFLOAT | vk::Format::X8_D24_UNORM_PACK32 => {
            vk::ImageAspectFlags::DEPTH
        }
        vk::Format::S8_UINT => vk::ImageAspectFlags::STENCIL,
        vk::Format::D16_UNORM_S8_UINT
        | vk::Format::D24_UNORM_S8_UINT
        | vk::Format::D32_SFLOAT_S8_UINT => vk::ImageAspectFlags::from_raw(
            vk::ImageAspectFlags::DEPTH.as_raw() | vk::ImageAspectFlags::STENCIL.as_raw(),
        ),
        _ => vk::ImageAspectFlags::COLOR,
    }
}

/// See [Representation and Texel Block Size](https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#texel-block-size)
pub const fn format_texel_block_size(fmt: vk::Format) -> u32 {
    match fmt {
        vk::Format::R4G4_UNORM_PACK8
        | vk::Format::R8_UNORM
        | vk::Format::R8_SNORM
        | vk::Format::R8_USCALED
        | vk::Format::R8_SSCALED
        | vk::Format::R8_UINT
        | vk::Format::R8_SINT
        | vk::Format::R8_SRGB => 1,
        vk::Format::A1B5G5R5_UNORM_PACK16_KHR
        | vk::Format::R10X6_UNORM_PACK16
        | vk::Format::R12X4_UNORM_PACK16
        | vk::Format::A4R4G4B4_UNORM_PACK16
        | vk::Format::A4B4G4R4_UNORM_PACK16
        | vk::Format::R4G4B4A4_UNORM_PACK16
        | vk::Format::B4G4R4A4_UNORM_PACK16
        | vk::Format::R5G6B5_UNORM_PACK16
        | vk::Format::B5G6R5_UNORM_PACK16
        | vk::Format::R5G5B5A1_UNORM_PACK16
        | vk::Format::B5G5R5A1_UNORM_PACK16
        | vk::Format::A1R5G5B5_UNORM_PACK16
        | vk::Format::R8G8_UNORM
        | vk::Format::R8G8_SNORM
        | vk::Format::R8G8_USCALED
        | vk::Format::R8G8_SSCALED
        | vk::Format::R8G8_UINT
        | vk::Format::R8G8_SINT
        | vk::Format::R8G8_SRGB
        | vk::Format::R16_UNORM
        | vk::Format::R16_SNORM
        | vk::Format::R16_USCALED
        | vk::Format::R16_SSCALED
        | vk::Format::R16_UINT
        | vk::Format::R16_SINT
        | vk::Format::R16_SFLOAT => 2,
        vk::Format::A8_UNORM_KHR => 1,
        vk::Format::R8G8B8_UNORM
        | vk::Format::R8G8B8_SNORM
        | vk::Format::R8G8B8_USCALED
        | vk::Format::R8G8B8_SSCALED
        | vk::Format::R8G8B8_UINT
        | vk::Format::R8G8B8_SINT
        | vk::Format::R8G8B8_SRGB
        | vk::Format::B8G8R8_UNORM
        | vk::Format::B8G8R8_SNORM
        | vk::Format::B8G8R8_USCALED
        | vk::Format::B8G8R8_SSCALED
        | vk::Format::B8G8R8_UINT
        | vk::Format::B8G8R8_SINT
        | vk::Format::B8G8R8_SRGB => 3,
        vk::Format::R10X6G10X6_UNORM_2PACK16
        | vk::Format::R12X4G12X4_UNORM_2PACK16
        | vk::Format::R16G16_S10_5_NV
        | vk::Format::R8G8B8A8_UNORM
        | vk::Format::R8G8B8A8_SNORM
        | vk::Format::R8G8B8A8_USCALED
        | vk::Format::R8G8B8A8_SSCALED
        | vk::Format::R8G8B8A8_UINT
        | vk::Format::R8G8B8A8_SINT
        | vk::Format::R8G8B8A8_SRGB
        | vk::Format::B8G8R8A8_UNORM
        | vk::Format::B8G8R8A8_SNORM
        | vk::Format::B8G8R8A8_USCALED
        | vk::Format::B8G8R8A8_SSCALED
        | vk::Format::B8G8R8A8_UINT
        | vk::Format::B8G8R8A8_SINT
        | vk::Format::B8G8R8A8_SRGB
        | vk::Format::A8B8G8R8_UNORM_PACK32
        | vk::Format::A8B8G8R8_SNORM_PACK32
        | vk::Format::A8B8G8R8_USCALED_PACK32
        | vk::Format::A8B8G8R8_SSCALED_PACK32
        | vk::Format::A8B8G8R8_UINT_PACK32
        | vk::Format::A8B8G8R8_SINT_PACK32
        | vk::Format::A8B8G8R8_SRGB_PACK32
        | vk::Format::A2R10G10B10_UNORM_PACK32
        | vk::Format::A2R10G10B10_SNORM_PACK32
        | vk::Format::A2R10G10B10_USCALED_PACK32
        | vk::Format::A2R10G10B10_SSCALED_PACK32
        | vk::Format::A2R10G10B10_UINT_PACK32
        | vk::Format::A2R10G10B10_SINT_PACK32
        | vk::Format::A2B10G10R10_UNORM_PACK32
        | vk::Format::A2B10G10R10_SNORM_PACK32
        | vk::Format::A2B10G10R10_USCALED_PACK32
        | vk::Format::A2B10G10R10_SSCALED_PACK32
        | vk::Format::A2B10G10R10_UINT_PACK32
        | vk::Format::A2B10G10R10_SINT_PACK32
        | vk::Format::R16G16_UNORM
        | vk::Format::R16G16_SNORM
        | vk::Format::R16G16_USCALED
        | vk::Format::R16G16_SSCALED
        | vk::Format::R16G16_UINT
        | vk::Format::R16G16_SINT
        | vk::Format::R16G16_SFLOAT
        | vk::Format::R32_UINT
        | vk::Format::R32_SINT
        | vk::Format::R32_SFLOAT
        | vk::Format::B10G11R11_UFLOAT_PACK32
        | vk::Format::E5B9G9R9_UFLOAT_PACK32 => 4,
        vk::Format::R16G16B16_UNORM
        | vk::Format::R16G16B16_SNORM
        | vk::Format::R16G16B16_USCALED
        | vk::Format::R16G16B16_SSCALED
        | vk::Format::R16G16B16_UINT
        | vk::Format::R16G16B16_SINT
        | vk::Format::R16G16B16_SFLOAT => 6,
        vk::Format::R16G16B16A16_UNORM
        | vk::Format::R16G16B16A16_SNORM
        | vk::Format::R16G16B16A16_USCALED
        | vk::Format::R16G16B16A16_SSCALED
        | vk::Format::R16G16B16A16_UINT
        | vk::Format::R16G16B16A16_SINT
        | vk::Format::R16G16B16A16_SFLOAT
        | vk::Format::R32G32_UINT
        | vk::Format::R32G32_SINT
        | vk::Format::R32G32_SFLOAT
        | vk::Format::R64_UINT
        | vk::Format::R64_SINT
        | vk::Format::R64_SFLOAT => 8,
        vk::Format::R32G32B32_UINT | vk::Format::R32G32B32_SINT | vk::Format::R32G32B32_SFLOAT => {
            12
        }
        vk::Format::R32G32B32A32_UINT
        | vk::Format::R32G32B32A32_SINT
        | vk::Format::R32G32B32A32_SFLOAT
        | vk::Format::R64G64_UINT
        | vk::Format::R64G64_SINT
        | vk::Format::R64G64_SFLOAT => 16,
        vk::Format::R64G64B64_UINT | vk::Format::R64G64B64_SINT | vk::Format::R64G64B64_SFLOAT => {
            24
        }
        vk::Format::R64G64B64A64_UINT
        | vk::Format::R64G64B64A64_SINT
        | vk::Format::R64G64B64A64_SFLOAT => 32,
        vk::Format::D16_UNORM => 2,
        vk::Format::X8_D24_UNORM_PACK32 => 4,
        vk::Format::D32_SFLOAT => 4,
        vk::Format::S8_UINT => 1,
        vk::Format::D16_UNORM_S8_UINT => 3,
        vk::Format::D24_UNORM_S8_UINT => 4,
        vk::Format::D32_SFLOAT_S8_UINT => 5,
        _ => {
            // Remaining formats should be implemented in the future
            unimplemented!()
        }
    }
}

pub(super) const fn image_subresource_range_from_layers(
    vk::ImageSubresourceLayers {
        aspect_mask,
        mip_level,
        base_array_layer,
        layer_count,
    }: vk::ImageSubresourceLayers,
) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask,
        base_mip_level: mip_level,
        level_count: 1,
        base_array_layer,
        layer_count,
    }
}

pub(super) const fn image_access_layout(access: AccessType) -> ImageLayout {
    if matches!(access, AccessType::Present | AccessType::ComputeShaderWrite) {
        ImageLayout::General
    } else {
        ImageLayout::Optimal
    }
}

pub(super) const fn initial_image_layout_access(ty: AccessType) -> AccessType {
    use AccessType::*;
    match ty {
        DepthStencilAttachmentReadWrite => DepthStencilAttachmentRead,
        _ => ty,
    }
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
        | DepthStencilAttachmentReadWrite
        | DepthAttachmentWriteStencilReadOnly
        | StencilAttachmentWriteDepthReadOnly
        | ComputeShaderWrite
        | AnyShaderWrite
        | TransferWrite
        | HostWrite
        | ColorAttachmentReadWrite
        | General
        | AccelerationStructureBuildWrite
        | AccelerationStructureBufferWrite
        | ComputeShaderReadWrite => true,
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
#[profiling::function]
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
                    let _ = res.remove(j);
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
        ty::DepthStencilAttachmentReadWrite => (
            stage::from_raw(
                stage::EARLY_FRAGMENT_TESTS.as_raw()
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS.as_raw(),
            ),
            access::from_raw(
                access::DEPTH_STENCIL_ATTACHMENT_WRITE.as_raw()
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ.as_raw(),
            ),
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
        ty::ComputeShaderReadWrite => (
            stage::COMPUTE_SHADER,
            access::from_raw(access::SHADER_WRITE.as_raw() | access::SHADER_READ.as_raw()),
        ),
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
