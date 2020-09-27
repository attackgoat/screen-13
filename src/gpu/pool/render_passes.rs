use {
    crate::gpu::driver::{Driver, RenderPass},
    gfx_hal::{
        format::Format,
        image::{Access, Layout},
        memory::Dependencies,
        pass::{
            Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDependency,
            SubpassDesc,
        },
        pso::PipelineStage,
    },
};

pub fn draw(driver: Driver, format: Format) -> RenderPass {
    // Borrows from https://github.com/SaschaWillems/Vulkan/blob/master/examples/subpasses/subpasses.cpp
    //
    // Attachments:
    // 0: Color
    // 1: Position
    // 2: Normal
    // 3: Material
    // 4: Depth
    //
    // Subpasses:
    // 0: Fill Graphics Buffer (`mesh.frag` called many times)
    // 1: Draw Color Image (`sunlight.frag`, `spotlight.frag`, or other lighting routine called many times)
    // 2: Forward Transparency (`trans.frag` called many times)
    RenderPass::new(
        #[cfg(debug_assertions)]
        "Draw",
        driver,
        &[
            Attachment {
                format: Some(format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
            },
            Attachment {
                format: Some(Format::Rgba16Sfloat),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::DontCare),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
            },
            Attachment {
                format: Some(Format::Rgba16Sfloat),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::DontCare),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
            },
            Attachment {
                format: Some(format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
            },
            Attachment {
                format: Some(Format::D32Sfloat),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::DontCare),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::DepthStencilAttachmentOptimal
                    ..Layout::DepthStencilAttachmentOptimal,
            },
        ],
        &[
            SubpassDesc {
                colors: &[
                    (0, Layout::ColorAttachmentOptimal),
                    (1, Layout::ColorAttachmentOptimal),
                    (2, Layout::ColorAttachmentOptimal),
                    (3, Layout::ColorAttachmentOptimal),
                ],
                depth_stencil: Some(&(4, Layout::DepthStencilAttachmentOptimal)),
                inputs: &[],
                resolves: &[],
                preserves: &[],
            },
            SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[
                    (1, Layout::ShaderReadOnlyOptimal),
                    (2, Layout::ShaderReadOnlyOptimal),
                    (3, Layout::ShaderReadOnlyOptimal),
                ],
                resolves: &[],
                preserves: &[],
            },
            SubpassDesc {
                colors: &[(3, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[
                    (0, Layout::ShaderReadOnlyOptimal),
                    (4, Layout::ShaderReadOnlyOptimal),
                ],
                resolves: &[],
                preserves: &[],
            },
        ],
        &[
            SubpassDependency {
                passes: None..Some(0),
                stages: PipelineStage::BOTTOM_OF_PIPE..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: Access::MEMORY_READ
                    ..Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE,
                flags: Dependencies::BY_REGION,
            },
            SubpassDependency {
                passes: Some(0)..Some(1),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::FRAGMENT_SHADER,
                accesses: Access::COLOR_ATTACHMENT_WRITE..Access::SHADER_READ,
                flags: Dependencies::BY_REGION,
            },
            SubpassDependency {
                passes: Some(1)..Some(2),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::FRAGMENT_SHADER,
                accesses: Access::COLOR_ATTACHMENT_WRITE..Access::SHADER_READ,
                flags: Dependencies::BY_REGION,
            },
            SubpassDependency {
                passes: Some(0)..None,
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::BOTTOM_OF_PIPE,
                accesses: Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE
                    ..Access::MEMORY_READ,
                flags: Dependencies::BY_REGION,
            },
        ],
    )
}

pub fn draw_ms(driver: Driver, format: Format) -> RenderPass {
    RenderPass::new(
        #[cfg(debug_assertions)]
        "Draw Multisampled",
        driver,
        &[Attachment {
            format: Some(format),
            samples: 1,
            ops: AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
        }],
        &[
            SubpassDesc {
                colors: &[
                    (0, Layout::ColorAttachmentOptimal),
                    (1, Layout::ColorAttachmentOptimal),
                    (2, Layout::ColorAttachmentOptimal),
                    (3, Layout::DepthStencilAttachmentOptimal),
                ],
                depth_stencil: None,
                inputs: &[(0, Layout::ShaderReadOnlyOptimal)],
                resolves: &[],
                preserves: &[],
            },
            SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[(0, Layout::ShaderReadOnlyOptimal)],
                resolves: &[],
                preserves: &[],
            },
            SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[(0, Layout::ShaderReadOnlyOptimal)],
                resolves: &[(0, Layout::ShaderReadOnlyOptimal)],
                preserves: &[],
            },
        ],
        &[],
    )
}

pub fn present(driver: &Driver, format: Format) -> RenderPass {
    RenderPass::new(
        #[cfg(debug_assertions)]
        "Write",
        Driver::clone(&driver),
        &[Attachment {
            format: Some(format),
            samples: 1,
            ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::Store), // TODO: Another render pass for AttachmentLoadOp::Clear when we need to render to a transparent window?
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::Undefined..Layout::Present,
        }],
        &[SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[],
        }],
        &[],
    )
}

pub fn read_write(driver: Driver, format: Format) -> RenderPass {
    RenderPass::new(
        #[cfg(debug_assertions)]
        "Read/Write",
        driver,
        &[Attachment {
            format: Some(format),
            samples: 1,
            ops: AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
        }],
        &[SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[(0, Layout::ShaderReadOnlyOptimal)],
            resolves: &[],
            preserves: &[],
        }],
        &[],
    )
}

pub fn read_write_ms(_driver: Driver, _format: Format) -> RenderPass {
    //&self.read_write_ms
    todo!();
}

pub fn write(driver: Driver, format: Format) -> RenderPass {
    RenderPass::new(
        #[cfg(debug_assertions)]
        "Write",
        driver,
        &[Attachment {
            format: Some(format),
            samples: 1,
            ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
        }],
        &[SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[],
        }],
        &[],
    )
}

pub fn write_ms(_driver: Driver, _format: Format) -> RenderPass {
    //&self.write_ms
    todo!();
}
