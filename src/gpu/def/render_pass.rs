pub mod draw {
    use super::*;

    // Attachment indexes
    const COLOR_METAL: usize = 0;
    const NORMAL_ROUGH: usize = 1;
    const LIGHT: usize = 2;
    const OUTPUT: usize = 3;
    const DEPTH: usize = 4;

    // Common subpasses
    const FILL_GEOM_BUF: SubpassDesc = SubpassDesc {
        colors: &[
            (COLOR_METAL, Layout::ColorAttachmentOptimal),
            (NORMAL_ROUGH, Layout::ColorAttachmentOptimal),
        ],
        depth_stencil: Some(&(DEPTH, Layout::DepthStencilAttachmentOptimal)),
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };
    const ACCUM_LIGHT: SubpassDesc = SubpassDesc {
        colors: &[(LIGHT, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (NORMAL_ROUGH, Layout::ShaderReadOnlyOptimal),
            (DEPTH, Layout::ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[COLOR_METAL],
    };
    const TONEMAP: SubpassDesc = SubpassDesc {
        colors: &[(OUTPUT, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (COLOR_METAL, Layout::ShaderReadOnlyOptimal),
            (NORMAL_ROUGH, Layout::ShaderReadOnlyOptimal),
            (LIGHT, Layout::ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[],
    };

    fn color_attachment(fmt: Format, ops: AttachmentOps) -> Attachment {
        Attachment {
            format: Some(fmt),
            samples: 1,
            ops,
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: const_layout(Layout::ColorAttachmentOptimal),
        }
    }

    fn depth_stencil_attachment(
        fmt: Format,
        ops: AttachmentOps,
        stencil_ops: AttachmentOps,
    ) -> Attachment {
        Attachment {
            format: Some(fmt),
            samples: 1,
            ops,
            stencil_ops,
            layouts: Layout::DepthStencilAttachmentOptimal..Layout::DepthStencilReadOnlyOptimal,
        }
    }

    pub fn fill_light_tonemap(driver: &Driver, mode: DrawRenderPassMode) -> RenderPass {
        use Subpasses::*;

        /// The list of subpasses used by this render pass, in index order.
        enum Subpasses {
            FillGeometryBuffer,
            AccumulateLight,
            Tonemap,
        }

        // Attachment instances
        let color_metal = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
        let normal_rough = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
        let light = color_attachment(mode.light, ATTACHMENT_OPS_CLEAR);
        let output = color_attachment(mode.output, AttachmentOps::PRESERVE);
        let depth =
            depth_stencil_attachment(mode.depth, ATTACHMENT_OPS_CLEAR, AttachmentOps::DONT_CARE);

        // TODO: These things hurt my brain are they correct how do I tell ugh
        // Subpass-to-Subpass dependencies
        let begin = SubpassDependency {
            passes: None..Some(FillGeometryBuffer as _),
            stages: PipelineStage::BOTTOM_OF_PIPE..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            accesses: Access::MEMORY_READ
                ..Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE,
            flags: Dependencies::BY_REGION,
        };
        let between_fill_and_light = SubpassDependency {
            passes: Some(FillGeometryBuffer as _)..Some(AccumulateLight as _),
            stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::FRAGMENT_SHADER,
            accesses: Access::COLOR_ATTACHMENT_WRITE..Access::SHADER_READ,
            flags: Dependencies::BY_REGION,
        };
        let end = SubpassDependency {
            passes: Some(FillGeometryBuffer as _)..None,
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
            &[FILL_GEOM_BUF, ACCUM_LIGHT, TONEMAP],
            &[begin, between_fill_and_light, end],
        )
    }

    /// Like the draw render pass except it contains a step between filling the geometry buffer and
    /// accumulating light
    pub fn fill_skydome_light_tonemap(driver: &Driver, mode: DrawRenderPassMode) -> RenderPass {
        use Subpasses::*;

        /// The list of subpasses used by this render pass, in index order.
        enum Subpasses {
            FillColorBuffer,
            FillGeometryBuffer,
            AccumulateLight,
            Tonemap,
        }

        // Attachment instances
        let color_metal = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
        let normal_rough = color_attachment(mode.geom_buf, AttachmentOps::DONT_CARE);
        let light = color_attachment(mode.light, ATTACHMENT_OPS_CLEAR);
        let output = color_attachment(mode.output, AttachmentOps::PRESERVE);
        let depth =
            depth_stencil_attachment(mode.depth, ATTACHMENT_OPS_CLEAR, AttachmentOps::DONT_CARE);

        // Subpasses
        let skydome = SubpassDesc {
            colors: &[(COLOR_METAL, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[NORMAL_ROUGH],
        };

        // TODO: These things hurt my brain are they correct how do I tell ugh
        // Subpass-to-Subpass dependencies
        let begin = SubpassDependency {
            passes: None..Some(FillColorBuffer as _),
            stages: PipelineStage::BOTTOM_OF_PIPE..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            accesses: Access::MEMORY_READ
                ..Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE,
            flags: Dependencies::BY_REGION,
        };
        let between_fill_and_light = SubpassDependency {
            passes: Some(FillColorBuffer as _)..Some(AccumulateLight as _),
            stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::FRAGMENT_SHADER,
            accesses: Access::COLOR_ATTACHMENT_WRITE..Access::SHADER_READ,
            flags: Dependencies::BY_REGION,
        };
        let end = SubpassDependency {
            passes: Some(FillColorBuffer as _)..None,
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
            &[FILL_GEOM_BUF, skydome, ACCUM_LIGHT, TONEMAP],
            &[begin, between_fill_and_light, end],
        )
    }

    /// Like the draw render pass except it contains a 'post'-fx step
    pub fn fill_light_tonemap_fx(_driver: &Driver, _mode: DrawRenderPassMode) -> RenderPass {
        todo!();
    }

    /// Like the draw render pass except it contains a 'pre' and 'post'-fx step
    pub fn fill_skydome_light_tonemap_fx(
        _driver: &Driver,
        _mode: DrawRenderPassMode,
    ) -> RenderPass {
        todo!();
    }
}

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

pub fn present(driver: &Driver, fmt: Format) -> RenderPass {
    /// The list of attachments used by this render pass, in index order.
    enum Attachments {
        Color,
    }

    // Attachments
    let color = Attachment {
        format: Some(fmt),
        samples: 1,
        ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::Store), // TODO: Another render pass for AttachmentLoadOp::Clear when we need to render to a transparent window?
        stencil_ops: AttachmentOps::DONT_CARE,
        layouts: Layout::Undefined..Layout::Present,
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
        "Present",
        driver,
        &[color],
        &[subpass],
        &[],
    )
}
