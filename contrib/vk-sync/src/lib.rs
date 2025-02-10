//! In an effort to make Vulkan synchronization more accessible, this library
//! provides a simplification of core synchronization mechanisms such as
//! pipeline barriers and events.
//!
//! Rather than the complex maze of enums and bit flags in Vulkan - many
//! combinations of which are invalid or nonsensical - this library collapses
//! this to a shorter list of distinct usage types, and a couple of options
//! for handling image layouts.
//!
//! Additionally, these usage types provide an easier mapping to other graphics
//! APIs like DirectX 12.
//!
//! Use of other synchronization mechanisms such as semaphores, fences and render
//! passes are not addressed in this library at present.

use ash::vk;

pub mod cmd;

/// Defines all potential resource usages
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AccessType {
    /// No access. Useful primarily for initialization
    Nothing,

    /// Command buffer read operation as defined by `NVX_device_generated_commands`
    CommandBufferReadNVX,

    /// Read as an indirect buffer for drawing or dispatch
    IndirectBuffer,

    /// Read as an index buffer for drawing
    IndexBuffer,

    /// Read as a vertex buffer for drawing
    VertexBuffer,

    /// Read as a uniform buffer in a vertex shader
    VertexShaderReadUniformBuffer,

    /// Read as a sampled image/uniform texel buffer in a vertex shader
    VertexShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as any other resource in a vertex shader
    VertexShaderReadOther,

    /// Read as a uniform buffer in a tessellation control shader
    TessellationControlShaderReadUniformBuffer,

    /// Read as a sampled image/uniform texel buffer in a tessellation control shader
    TessellationControlShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as any other resource in a tessellation control shader
    TessellationControlShaderReadOther,

    /// Read as a uniform buffer in a tessellation evaluation shader
    TessellationEvaluationShaderReadUniformBuffer,

    /// Read as a sampled image/uniform texel buffer in a tessellation evaluation shader
    TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as any other resource in a tessellation evaluation shader
    TessellationEvaluationShaderReadOther,

    /// Read as a uniform buffer in a geometry shader
    GeometryShaderReadUniformBuffer,

    /// Read as a sampled image/uniform texel buffer in a geometry shader
    GeometryShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as any other resource in a geometry shader
    GeometryShaderReadOther,

    /// Read as a uniform buffer in a fragment shader
    FragmentShaderReadUniformBuffer,

    /// Read as a sampled image/uniform texel buffer in a fragment shader
    FragmentShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as an input attachment with a color format in a fragment shader
    FragmentShaderReadColorInputAttachment,

    /// Read as an input attachment with a depth/stencil format in a fragment shader
    FragmentShaderReadDepthStencilInputAttachment,

    /// Read as any other resource in a fragment shader
    FragmentShaderReadOther,

    /// Read by blending/logic operations or subpass load operations
    ColorAttachmentRead,

    /// Read by depth/stencil tests or subpass load operations
    DepthStencilAttachmentRead,

    /// Read as a uniform buffer in a compute shader
    ComputeShaderReadUniformBuffer,

    /// Read as a sampled image/uniform texel buffer in a compute shader
    ComputeShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as any other resource in a compute shader
    ComputeShaderReadOther,

    /// Read as a uniform buffer in any shader
    AnyShaderReadUniformBuffer,

    /// Read as a uniform buffer in any shader, or a vertex buffer
    AnyShaderReadUniformBufferOrVertexBuffer,

    /// Read as a sampled image in any shader
    AnyShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as any other resource (excluding attachments) in any shader
    AnyShaderReadOther,

    /// Read as the source of a transfer operation
    TransferRead,

    /// Read on the host
    HostRead,

    /// Read by the presentation engine (i.e. `vkQueuePresentKHR`)
    Present,

    /// Command buffer write operation as defined by `NVX_device_generated_commands`
    CommandBufferWriteNVX,

    /// Written as any resource in a vertex shader
    VertexShaderWrite,

    /// Written as any resource in a tessellation control shader
    TessellationControlShaderWrite,

    /// Written as any resource in a tessellation evaluation shader
    TessellationEvaluationShaderWrite,

    /// Written as any resource in a geometry shader
    GeometryShaderWrite,

    /// Written as any resource in a fragment shader
    FragmentShaderWrite,

    /// Written as a color attachment during rendering, or via a subpass store op
    ColorAttachmentWrite,

    /// Written as a depth/stencil attachment during rendering, or via a subpass store op
    DepthStencilAttachmentWrite,

    /// Written as a depth aspect of a depth/stencil attachment during rendering, whilst the
    /// stencil aspect is read-only. Requires `VK_KHR_maintenance2` to be enabled.
    DepthAttachmentWriteStencilReadOnly,

    /// Written as a stencil aspect of a depth/stencil attachment during rendering, whilst the
    /// depth aspect is read-only. Requires `VK_KHR_maintenance2` to be enabled.
    StencilAttachmentWriteDepthReadOnly,

    /// Written as any resource in a compute shader
    ComputeShaderWrite,

    /// Read or written as any resource in a compute shader
    ComputeShaderReadWrite,

    /// Written as any resource in any shader
    AnyShaderWrite,

    /// Written as the destination of a transfer operation
    TransferWrite,

    /// Written on the host
    HostWrite,

    /// Read or written as a color attachment during rendering
    ColorAttachmentReadWrite,

    /// Covers any access - useful for debug, generally avoid for performance reasons
    General,

    /// Read as a sampled image/uniform texel buffer in a ray tracing shader
    RayTracingShaderReadSampledImageOrUniformTexelBuffer,

    /// Read as an input attachment with a color format in a ray tracing shader
    RayTracingShaderReadColorInputAttachment,

    /// Read as an input attachment with a depth/stencil format in a ray tracing shader
    RayTracingShaderReadDepthStencilInputAttachment,

    /// Read as an acceleration structure in a ray tracing shader
    RayTracingShaderReadAccelerationStructure,

    /// Read as any other resource in a ray tracing shader
    RayTracingShaderReadOther,

    /// Written as an acceleration structure during acceleration structure building
    AccelerationStructureBuildWrite,

    /// Read as an acceleration structure during acceleration structure building (e.g. a BLAS when building a TLAS)
    AccelerationStructureBuildRead,

    // Written as a buffer during acceleration structure building (e.g. a staging buffer)
    AccelerationStructureBufferWrite,
}

impl Default for AccessType {
    fn default() -> Self {
        AccessType::Nothing
    }
}

/// Defines a handful of layout options for images.
/// Rather than a list of all possible image layouts, this reduced list is
/// correlated with the access types to map to the correct Vulkan layouts.
/// `Optimal` is usually preferred.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ImageLayout {
    /// Choose the most optimal layout for each usage. Performs layout transitions as appropriate for the access.
    Optimal,

    /// Layout accessible by all Vulkan access types on a device - no layout transitions except for presentation
    General,

    /// Similar to `General`, but also allows presentation engines to access it - no layout transitions.
    /// Requires `VK_KHR_shared_presentable_image` to be enabled, and this can only be used for shared presentable
    /// images (i.e. single-buffered swap chains).
    GeneralAndPresentation,
}

impl Default for ImageLayout {
    fn default() -> Self {
        ImageLayout::Optimal
    }
}

/// Global barriers define a set of accesses on multiple resources at once.
/// If a buffer or image doesn't require a queue ownership transfer, or an image
/// doesn't require a layout transition (e.g. you're using one of the
/// `ImageLayout::General*` layouts) then a global barrier should be preferred.
///
/// Simply define the previous and next access types of resources affected.
#[derive(Debug, Default, Clone)]
pub struct GlobalBarrier<'a> {
    pub previous_accesses: &'a [AccessType],
    pub next_accesses: &'a [AccessType],
}

/// Buffer barriers should only be used when a queue family ownership transfer
/// is required - prefer global barriers at all other times.
///
/// Access types are defined in the same way as for a global memory barrier, but
/// they only affect the buffer range identified by `buffer`, `offset` and `size`,
/// rather than all resources.
///
/// `src_queue_family_index` and `dst_queue_family_index` will be passed unmodified
/// into a buffer memory barrier.
///
/// A buffer barrier defining a queue ownership transfer needs to be executed
/// twice - once by a queue in the source queue family, and then once again by a
/// queue in the destination queue family, with a semaphore guaranteeing
/// execution order between them.
#[derive(Debug, Default, Clone)]
pub struct BufferBarrier<'a> {
    pub previous_accesses: &'a [AccessType],
    pub next_accesses: &'a [AccessType],
    pub src_queue_family_index: u32,
    pub dst_queue_family_index: u32,
    pub buffer: vk::Buffer,
    pub offset: usize,
    pub size: usize,
}

/// Image barriers should only be used when a queue family ownership transfer
/// or an image layout transition is required - prefer global barriers at all
/// other times.
///
/// In general it is better to use image barriers with `ImageLayout::Optimal`
/// than it is to use global barriers with images using either of the
/// `ImageLayout::General*` layouts.
///
/// Access types are defined in the same way as for a global memory barrier, but
/// they only affect the image subresource range identified by `image` and
/// `range`, rather than all resources.
///
/// `src_queue_family_index`, `dst_queue_family_index`, `image`, and `range` will
/// be passed unmodified into an image memory barrier.
///
/// An image barrier defining a queue ownership transfer needs to be executed
/// twice - once by a queue in the source queue family, and then once again by a
/// queue in the destination queue family, with a semaphore guaranteeing
/// execution order between them.
///
/// If `discard_contents` is set to true, the contents of the image become
/// undefined after the barrier is executed, which can result in a performance
/// boost over attempting to preserve the contents. This is particularly useful
/// for transient images where the contents are going to be immediately overwritten.
/// A good example of when to use this is when an application re-uses a presented
/// image after acquiring the next swap chain image.
#[derive(Debug, Default, Clone)]
pub struct ImageBarrier<'a> {
    pub previous_accesses: &'a [AccessType],
    pub next_accesses: &'a [AccessType],
    pub previous_layout: ImageLayout,
    pub next_layout: ImageLayout,
    pub discard_contents: bool,
    pub src_queue_family_index: u32,
    pub dst_queue_family_index: u32,
    pub image: vk::Image,
    pub range: vk::ImageSubresourceRange,
}

/// Mapping function that translates a global barrier into a set of source and
/// destination pipeline stages, and a memory barrier, that can be used with
/// Vulkan synchronization methods.
pub fn get_memory_barrier<'a>(
    barrier: &GlobalBarrier,
) -> (
    vk::PipelineStageFlags,
    vk::PipelineStageFlags,
    vk::MemoryBarrier<'a>,
) {
    let mut src_stages = vk::PipelineStageFlags::empty();
    let mut dst_stages = vk::PipelineStageFlags::empty();

    let mut memory_barrier = vk::MemoryBarrier::default();

    for previous_access in barrier.previous_accesses {
        let previous_info = get_access_info(*previous_access);

        src_stages |= previous_info.stage_mask;

        // Add appropriate availability operations - for writes only.
        if is_write_access(*previous_access) {
            memory_barrier.src_access_mask |= previous_info.access_mask;
        }
    }

    for next_access in barrier.next_accesses {
        let next_info = get_access_info(*next_access);

        dst_stages |= next_info.stage_mask;

        // Add visibility operations as necessary.
        // If the src access mask, this is a WAR hazard (or for some reason a "RAR"),
        // so the dst access mask can be safely zeroed as these don't need visibility.
        if memory_barrier.src_access_mask != vk::AccessFlags::empty() {
            memory_barrier.dst_access_mask |= next_info.access_mask;
        }
    }

    // Ensure that the stage masks are valid if no stages were determined
    if src_stages == vk::PipelineStageFlags::empty() {
        src_stages = vk::PipelineStageFlags::TOP_OF_PIPE;
    }

    if dst_stages == vk::PipelineStageFlags::empty() {
        dst_stages = vk::PipelineStageFlags::BOTTOM_OF_PIPE;
    }

    (src_stages, dst_stages, memory_barrier)
}

/// Mapping function that translates a buffer barrier into a set of source and
/// destination pipeline stages, and a buffer memory barrier, that can be used
/// with Vulkan synchronization methods.
pub fn get_buffer_memory_barrier<'a>(
    barrier: &BufferBarrier,
) -> (
    vk::PipelineStageFlags,
    vk::PipelineStageFlags,
    vk::BufferMemoryBarrier<'a>,
) {
    let mut src_stages = vk::PipelineStageFlags::empty();
    let mut dst_stages = vk::PipelineStageFlags::empty();

    let mut buffer_barrier = vk::BufferMemoryBarrier {
        src_queue_family_index: barrier.src_queue_family_index,
        dst_queue_family_index: barrier.dst_queue_family_index,
        buffer: barrier.buffer,
        offset: barrier.offset as u64,
        size: barrier.size as u64,
        ..Default::default()
    };

    for previous_access in barrier.previous_accesses {
        let previous_info = get_access_info(*previous_access);

        src_stages |= previous_info.stage_mask;

        // Add appropriate availability operations - for writes only.
        if is_write_access(*previous_access) {
            buffer_barrier.src_access_mask |= previous_info.access_mask;
        }
    }

    for next_access in barrier.next_accesses {
        let next_info = get_access_info(*next_access);

        dst_stages |= next_info.stage_mask;

        // Add visibility operations as necessary.
        // If the src access mask, this is a WAR hazard (or for some reason a "RAR"),
        // so the dst access mask can be safely zeroed as these don't need visibility.
        if buffer_barrier.src_access_mask != vk::AccessFlags::empty() {
            buffer_barrier.dst_access_mask |= next_info.access_mask;
        }
    }

    // Ensure that the stage masks are valid if no stages were determined
    if src_stages == vk::PipelineStageFlags::empty() {
        src_stages = vk::PipelineStageFlags::TOP_OF_PIPE;
    }

    if dst_stages == vk::PipelineStageFlags::empty() {
        dst_stages = vk::PipelineStageFlags::BOTTOM_OF_PIPE;
    }

    (src_stages, dst_stages, buffer_barrier)
}

/// Mapping function that translates an image barrier into a set of source and
/// destination pipeline stages, and an image memory barrier, that can be used
/// with Vulkan synchronization methods.
pub fn get_image_memory_barrier<'a>(
    barrier: &ImageBarrier,
) -> (
    vk::PipelineStageFlags,
    vk::PipelineStageFlags,
    vk::ImageMemoryBarrier<'a>,
) {
    let mut src_stages = vk::PipelineStageFlags::empty();
    let mut dst_stages = vk::PipelineStageFlags::empty();

    let mut image_barrier = vk::ImageMemoryBarrier {
        src_queue_family_index: barrier.src_queue_family_index,
        dst_queue_family_index: barrier.dst_queue_family_index,
        image: barrier.image,
        subresource_range: barrier.range,
        ..Default::default()
    };

    for previous_access in barrier.previous_accesses {
        let previous_info = get_access_info(*previous_access);

        src_stages |= previous_info.stage_mask;

        // Add appropriate availability operations - for writes only.
        if is_write_access(*previous_access) {
            image_barrier.src_access_mask |= previous_info.access_mask;
        }

        if barrier.discard_contents {
            image_barrier.old_layout = vk::ImageLayout::UNDEFINED;
        } else {
            let layout = match barrier.previous_layout {
                ImageLayout::General => {
                    if *previous_access == AccessType::Present {
                        vk::ImageLayout::PRESENT_SRC_KHR
                    } else {
                        vk::ImageLayout::GENERAL
                    }
                }
                ImageLayout::Optimal => previous_info.image_layout,
                ImageLayout::GeneralAndPresentation => {
                    unimplemented!()
                    // TODO: layout = vk::ImageLayout::VK_IMAGE_LAYOUT_SHARED_PRESENT_KHR
                }
            };

            image_barrier.old_layout = layout;
        }
    }

    for next_access in barrier.next_accesses {
        let next_info = get_access_info(*next_access);

        dst_stages |= next_info.stage_mask;

        // Add appropriate availability operations - in all cases beccause otherwise
        // we get WAW and RAWs.
        image_barrier.dst_access_mask |= next_info.access_mask;

        let layout = match barrier.next_layout {
            ImageLayout::General => {
                if *next_access == AccessType::Present {
                    vk::ImageLayout::PRESENT_SRC_KHR
                } else {
                    vk::ImageLayout::GENERAL
                }
            }
            ImageLayout::Optimal => next_info.image_layout,
            ImageLayout::GeneralAndPresentation => {
                unimplemented!()
                // TODO: layout = vk::ImageLayout::VK_IMAGE_LAYOUT_SHARED_PRESENT_KHR
            }
        };

        image_barrier.new_layout = layout;
    }

    // Ensure that the stage masks are valid if no stages were determined
    if src_stages == vk::PipelineStageFlags::empty() {
        src_stages = vk::PipelineStageFlags::TOP_OF_PIPE;
    }

    if dst_stages == vk::PipelineStageFlags::empty() {
        dst_stages = vk::PipelineStageFlags::BOTTOM_OF_PIPE;
    }

    (src_stages, dst_stages, image_barrier)
}

pub(crate) struct AccessInfo {
    pub(crate) stage_mask: vk::PipelineStageFlags,
    pub(crate) access_mask: vk::AccessFlags,
    pub(crate) image_layout: vk::ImageLayout,
}

pub(crate) fn get_access_info(access_type: AccessType) -> AccessInfo {
    match access_type {
        AccessType::Nothing => AccessInfo {
            stage_mask: vk::PipelineStageFlags::empty(),
            access_mask: vk::AccessFlags::empty(),
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::CommandBufferReadNVX => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COMMAND_PREPROCESS_NV,
            access_mask: vk::AccessFlags::COMMAND_PREPROCESS_READ_NV,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::IndirectBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::DRAW_INDIRECT,
            access_mask: vk::AccessFlags::INDIRECT_COMMAND_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::IndexBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::VERTEX_INPUT,
            access_mask: vk::AccessFlags::INDEX_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::VertexBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::VERTEX_INPUT,
            access_mask: vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::VertexShaderReadUniformBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::VertexShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::VertexShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::TessellationControlShaderReadUniformBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
            access_mask: vk::AccessFlags::UNIFORM_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::TessellationControlShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::TessellationControlShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::TessellationEvaluationShaderReadUniformBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
            access_mask: vk::AccessFlags::UNIFORM_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer => {
            AccessInfo {
                stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
                access_mask: vk::AccessFlags::SHADER_READ,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }
        }
        AccessType::TessellationEvaluationShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::GeometryShaderReadUniformBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
            access_mask: vk::AccessFlags::UNIFORM_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::GeometryShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::GeometryShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::FragmentShaderReadUniformBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            access_mask: vk::AccessFlags::UNIFORM_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::FragmentShaderReadColorInputAttachment => AccessInfo {
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::FragmentShaderReadDepthStencilInputAttachment => AccessInfo {
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        },
        AccessType::FragmentShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::ColorAttachmentRead => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        },
        AccessType::DepthStencilAttachmentRead => AccessInfo {
            stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        },
        AccessType::ComputeShaderReadUniformBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
            access_mask: vk::AccessFlags::UNIFORM_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::ComputeShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::AnyShaderReadUniformBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            access_mask: vk::AccessFlags::UNIFORM_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::AnyShaderReadUniformBufferOrVertexBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            access_mask: vk::AccessFlags::UNIFORM_READ | vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::AnyShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::TransferRead => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TRANSFER,
            access_mask: vk::AccessFlags::TRANSFER_READ,
            image_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        },
        AccessType::HostRead => AccessInfo {
            stage_mask: vk::PipelineStageFlags::HOST,
            access_mask: vk::AccessFlags::HOST_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::Present => AccessInfo {
            stage_mask: vk::PipelineStageFlags::empty(),
            access_mask: vk::AccessFlags::empty(),
            image_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        },
        AccessType::CommandBufferWriteNVX => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COMMAND_PREPROCESS_NV,
            access_mask: vk::AccessFlags::COMMAND_PREPROCESS_WRITE_NV,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::VertexShaderWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
            access_mask: vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::TessellationControlShaderWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
            access_mask: vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::TessellationEvaluationShaderWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
            access_mask: vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::GeometryShaderWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
            access_mask: vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::FragmentShaderWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            access_mask: vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::ColorAttachmentWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        },
        AccessType::DepthStencilAttachmentWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            image_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        },
        AccessType::DepthAttachmentWriteStencilReadOnly => AccessInfo {
            stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
        },
        AccessType::StencilAttachmentWriteDepthReadOnly => AccessInfo {
            stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::DEPTH_READ_ONLY_STENCIL_ATTACHMENT_OPTIMAL,
        },
        AccessType::ComputeShaderWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
            access_mask: vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::ComputeShaderReadWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
            access_mask: vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::AnyShaderWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            access_mask: vk::AccessFlags::SHADER_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::TransferWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::TRANSFER,
            access_mask: vk::AccessFlags::TRANSFER_WRITE,
            image_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        },
        AccessType::HostWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::HOST,
            access_mask: vk::AccessFlags::HOST_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::ColorAttachmentReadWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
                | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        },
        AccessType::General => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            access_mask: vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::RayTracingShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
            stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::RayTracingShaderReadColorInputAttachment => AccessInfo {
            stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
            access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        },
        AccessType::RayTracingShaderReadDepthStencilInputAttachment => AccessInfo {
            stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
            access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
            image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        },
        AccessType::RayTracingShaderReadAccelerationStructure => AccessInfo {
            stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
            access_mask: vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::RayTracingShaderReadOther => AccessInfo {
            stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
            access_mask: vk::AccessFlags::SHADER_READ,
            image_layout: vk::ImageLayout::GENERAL,
        },
        AccessType::AccelerationStructureBuildWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            access_mask: vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::AccelerationStructureBuildRead => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            access_mask: vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
        AccessType::AccelerationStructureBufferWrite => AccessInfo {
            stage_mask: vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            access_mask: vk::AccessFlags::TRANSFER_WRITE,
            image_layout: vk::ImageLayout::UNDEFINED,
        },
    }
}

pub(crate) fn is_write_access(access_type: AccessType) -> bool {
    match access_type {
        AccessType::CommandBufferWriteNVX => true,
        AccessType::VertexShaderWrite => true,
        AccessType::TessellationControlShaderWrite => true,
        AccessType::TessellationEvaluationShaderWrite => true,
        AccessType::GeometryShaderWrite => true,
        AccessType::FragmentShaderWrite => true,
        AccessType::ColorAttachmentWrite => true,
        AccessType::DepthStencilAttachmentWrite => true,
        AccessType::DepthAttachmentWriteStencilReadOnly => true,
        AccessType::StencilAttachmentWriteDepthReadOnly => true,
        AccessType::ComputeShaderWrite => true,
        AccessType::ComputeShaderReadWrite => true,
        AccessType::AnyShaderWrite => true,
        AccessType::TransferWrite => true,
        AccessType::HostWrite => true,
        AccessType::ColorAttachmentReadWrite => true,
        AccessType::General => true,
        _ => false,
    }
}
