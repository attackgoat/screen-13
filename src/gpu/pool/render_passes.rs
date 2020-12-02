use {
    super::{ColorRenderPassMode, DrawRenderPassMode},
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
    std::ops::Range,
};

fn const_layout(layout: Layout) -> Range<Layout> {
    layout..layout
}

pub fn draw(driver: Driver, mode: DrawRenderPassMode) -> RenderPass {
    enum Attachments {
        Albedo,
        Material,
        Normal,
        Light,
        Depth,
    }

    // Attachments
    let albedo = Attachment {
        format: Some(mode.albedo),
        samples: 1,
        ops: AttachmentOps::PRESERVE,
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: const_layout(Layout::ColorAttachmentOptimal),
    };
    let dont_care = |format| Attachment {
        format: Some(format),
        samples: 1,
        ops: AttachmentOps::DONT_CARE,
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: const_layout(Layout::ColorAttachmentOptimal),
    };
    let material = dont_care(mode.material);
    let normal = dont_care(mode.normal);
    let light = dont_care(mode.light);
    let depth = dont_care(mode.depth);

    // Subpasses
    let draw_meshes = SubpassDesc {
        colors: &[
            (Attachments::Albedo as _, Layout::ColorAttachmentOptimal),
            (Attachments::Normal as _, Layout::ColorAttachmentOptimal),
        ],
        depth_stencil: Some(&(
            Attachments::Depth as _,
            Layout::DepthStencilAttachmentOptimal,
        )),
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };
    let accum_light = SubpassDesc {
        colors: &[(Attachments::Light as _, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (Attachments::Albedo as _, Layout::ShaderReadOnlyOptimal),
            (Attachments::Depth as _, Layout::ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[],
    };
    RenderPass::new(
        #[cfg(debug_assertions)]
        "Draw",
        driver,
        &[albedo, material, normal, light, depth],
        &[draw_meshes, accum_light],
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

pub fn present(driver: &Driver, format: Format) -> RenderPass {
    let present_attachment = 0;
    let present = Attachment {
        format: Some(format),
        samples: 1,
        ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::Store), // TODO: Another render pass for AttachmentLoadOp::Clear when we need to render to a transparent window?
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: Layout::Undefined..Layout::Present,
    };
    let subpass = SubpassDesc {
        colors: &[(present_attachment, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };
    RenderPass::new(
        #[cfg(debug_assertions)]
        "Write",
        Driver::clone(&driver),
        &[present],
        &[subpass],
        &[],
    )
}

pub fn color(driver: Driver, mode: ColorRenderPassMode) -> RenderPass {
    let color_attachment = 0;
    let color = Attachment {
        format: Some(mode.format),
        samples: 1,
        ops: if mode.preserve {
            AttachmentOps::PRESERVE
        } else {
            AttachmentOps {
                load: AttachmentLoadOp::DontCare,
                store: AttachmentStoreOp::Store,
            }
        },
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
    };
    let subpass = SubpassDesc {
        colors: &[(color_attachment, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: if mode.preserve {
            &[(0, Layout::ShaderReadOnlyOptimal)]
        } else {
            &[]
        },
        resolves: &[],
        preserves: &[],
    };

    RenderPass::new(
        #[cfg(debug_assertions)]
        "Color",
        driver,
        &[color],
        &[subpass],
        &[],
    )
}
