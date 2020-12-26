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

const ATTACHMENT_OPS_CLEAR: AttachmentOps = AttachmentOps {
    load: AttachmentLoadOp::Clear,
    store: AttachmentStoreOp::DontCare,
};
const ATTACHMENT_OPS_STORE: AttachmentOps = AttachmentOps {
    load: AttachmentLoadOp::DontCare,
    store: AttachmentStoreOp::Store,
};

fn const_layout(layout: Layout) -> Range<Layout> {
    layout..layout
}

pub fn color(driver: &Driver, mode: ColorRenderPassMode) -> RenderPass {
    /// The list of attachments used by this render pass, in index order.
    enum Attachments {
        Color,
    }
    // Attachments
    let color = Attachment {
        format: Some(mode.fmt),
        samples: 1,
        ops: if mode.preserve {
            AttachmentOps::PRESERVE
        } else {
            ATTACHMENT_OPS_STORE
        },
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
    };

    // Subpasses
    let subpass = SubpassDesc {
        colors: &[(Attachments::Color as _, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };

    RenderPass::new(
        #[cfg(feature = "debug-names")]
        "Color",
        driver,
        &[color],
        &[subpass],
        &[],
    )
}

/// The 'core' drawing render pass
pub fn draw(driver: &Driver, mode: DrawRenderPassMode) -> RenderPass {
    let color_attachment = |format, ops| Attachment {
        format: Some(format),
        samples: 1,
        ops,
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: const_layout(Layout::ColorAttachmentOptimal),
    };
    let depth_stencil_attachment = |format, ops, stencil_ops| Attachment {
        format: Some(format),
        samples: 1,
        ops,
        stencil_ops,
        layouts: Layout::DepthStencilAttachmentOptimal..Layout::DepthStencilReadOnlyOptimal,
    };

    /// The list of attachments used by this render pass, in index order.
    enum Attachments {
        ColorMetal,
        NormalRough,
        Light,
        Output,
        Depth,
    }

    // Attachments
    let color_metal = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
    let normal_rough = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
    let light = color_attachment(mode.light, ATTACHMENT_OPS_CLEAR);
    let output = color_attachment(mode.output, AttachmentOps::PRESERVE);
    let depth =
        depth_stencil_attachment(mode.depth, ATTACHMENT_OPS_CLEAR, AttachmentOps::DONT_CARE);

    /// The list of subpasses used by this render pass, in index order.
    enum Subpasses {
        FillGeometryBuffer,
        AccumulateLight,
        Tonemap,
    }

    let fill_geom_buf_depth_stencil_desc = (
        Attachments::Depth as _,
        Layout::DepthStencilAttachmentOptimal,
    );

    // Subpasses
    let fill_geom_buf = SubpassDesc {
        colors: &[
            (Attachments::ColorMetal as _, Layout::ColorAttachmentOptimal),
            (
                Attachments::NormalRough as _,
                Layout::ColorAttachmentOptimal,
            ),
        ],
        depth_stencil: Some(&fill_geom_buf_depth_stencil_desc),
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };
    let accum_light = SubpassDesc {
        colors: &[(Attachments::Light as _, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (Attachments::NormalRough as _, Layout::ShaderReadOnlyOptimal),
            (Attachments::Depth as _, Layout::ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[Attachments::ColorMetal as _],
    };
    let tonemap = SubpassDesc {
        colors: &[(Attachments::Output as _, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (Attachments::ColorMetal as _, Layout::ShaderReadOnlyOptimal),
            (Attachments::NormalRough as _, Layout::ShaderReadOnlyOptimal),
            (Attachments::Light as _, Layout::ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[],
    };

    // TODO: These things hurt my brain are they correct how do I tell ugh
    // Subpass-to-Subpass dependencies
    let begin = SubpassDependency {
        passes: None..Some(Subpasses::FillGeometryBuffer as _),
        stages: PipelineStage::BOTTOM_OF_PIPE..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
        accesses: Access::MEMORY_READ
            ..Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE,
        flags: Dependencies::BY_REGION,
    };
    let between_fill_and_light = SubpassDependency {
        passes: Some(Subpasses::FillGeometryBuffer as _)..Some(Subpasses::AccumulateLight as _),
        stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::FRAGMENT_SHADER,
        accesses: Access::COLOR_ATTACHMENT_WRITE..Access::SHADER_READ,
        flags: Dependencies::BY_REGION,
    };
    let end = SubpassDependency {
        passes: Some(Subpasses::FillGeometryBuffer as _)..None,
        stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::BOTTOM_OF_PIPE,
        accesses: Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE
            ..Access::MEMORY_READ,
        flags: Dependencies::BY_REGION,
    };

    RenderPass::new(
        #[cfg(feature = "debug-names")]
        "Draw",
        driver,
        &[color_metal, normal_rough, light, output, depth],
        &[fill_geom_buf, accum_light, tonemap],
        &[begin, between_fill_and_light, end],
    )
}

/// Like the draw render pass except it contains a 'pre'-fx step (used to draw skydome)
pub fn draw_pre(driver: &Driver, mode: DrawRenderPassMode) -> RenderPass {
    let color_attachment = |format, ops| Attachment {
        format: Some(format),
        samples: 1,
        ops,
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: const_layout(Layout::ColorAttachmentOptimal),
    };
    let depth_stencil_attachment = |format, ops, stencil_ops| Attachment {
        format: Some(format),
        samples: 1,
        ops,
        stencil_ops,
        layouts: Layout::DepthStencilAttachmentOptimal..Layout::DepthStencilReadOnlyOptimal,
    };

    /// The list of attachments used by this render pass, in index order.
    enum Attachments {
        ColorMetal,
        NormalRough,
        Light,
        Output,
        Depth,
    }

    // Attachments
    let color_metal = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
    let normal_rough = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
    let light = color_attachment(mode.light, ATTACHMENT_OPS_CLEAR);
    let output = color_attachment(mode.output, AttachmentOps::PRESERVE);
    let depth =
        depth_stencil_attachment(mode.depth, ATTACHMENT_OPS_CLEAR, AttachmentOps::DONT_CARE);

    /// The list of subpasses used by this render pass, in index order.
    enum Subpasses {
        FillColorBuffer,
        FillGeometryBuffer,
        AccumulateLight,
        Tonemap,
    }

    // Subpasses
    let fill_color_buf = SubpassDesc {
        colors: &[(Attachments::ColorMetal as _, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[],
        resolves: &[],
        preserves: &[Attachments::NormalRough as _, Attachments::Depth as _],
    };
    let fill_geom_buf = SubpassDesc {
        colors: &[
            (Attachments::ColorMetal as _, Layout::ColorAttachmentOptimal),
            (
                Attachments::NormalRough as _,
                Layout::ColorAttachmentOptimal,
            ),
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
            (Attachments::NormalRough as _, Layout::ShaderReadOnlyOptimal),
            (Attachments::Depth as _, Layout::ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[Attachments::ColorMetal as _],
    };
    let tonemap = SubpassDesc {
        colors: &[(Attachments::Output as _, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (Attachments::ColorMetal as _, Layout::ShaderReadOnlyOptimal),
            (Attachments::NormalRough as _, Layout::ShaderReadOnlyOptimal),
            (Attachments::Light as _, Layout::ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[],
    };

    // TODO: These things hurt my brain are they correct how do I tell ugh
    // Subpass-to-Subpass dependencies
    let begin = SubpassDependency {
        passes: None..Some(Subpasses::FillColorBuffer as _),
        stages: PipelineStage::BOTTOM_OF_PIPE..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
        accesses: Access::MEMORY_READ
            ..Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE,
        flags: Dependencies::BY_REGION,
    };
    let between_fill_and_light = SubpassDependency {
        passes: Some(Subpasses::FillColorBuffer as _)..Some(Subpasses::AccumulateLight as _),
        stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::FRAGMENT_SHADER,
        accesses: Access::COLOR_ATTACHMENT_WRITE..Access::SHADER_READ,
        flags: Dependencies::BY_REGION,
    };
    let end = SubpassDependency {
        passes: Some(Subpasses::FillColorBuffer as _)..None,
        stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::BOTTOM_OF_PIPE,
        accesses: Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE
            ..Access::MEMORY_READ,
        flags: Dependencies::BY_REGION,
    };

    RenderPass::new(
        #[cfg(feature = "debug-names")]
        "Draw",
        driver,
        &[color_metal, normal_rough, light, output, depth],
        &[fill_color_buf, fill_geom_buf, accum_light, tonemap],
        &[begin, between_fill_and_light, end],
    )
}

/// Like the draw render pass except it contains a 'post'-fx step
pub fn draw_post(_driver: &Driver, _mode: DrawRenderPassMode) -> RenderPass {
    todo!();
}

/// Like the draw render pass except it contains a 'pre' and 'post'-fx step
pub fn draw_pre_post(_driver: &Driver, _mode: DrawRenderPassMode) -> RenderPass {
    todo!();
}

pub fn present(driver: &Driver, fmt: Format) -> RenderPass {
    let present_attachment = 0;
    let present = Attachment {
        format: Some(fmt),
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
        #[cfg(feature = "debug-names")]
        "Present",
        driver,
        &[present],
        &[subpass],
        &[],
    )
}
