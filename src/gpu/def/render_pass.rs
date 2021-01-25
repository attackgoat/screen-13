pub mod draw {
    use super::*;

    // Attachment indexes
    const COLOR_METAL: usize = 0;
    const NORMAL_ROUGH: usize = 1;
    const LIGHT: usize = 2;
    const OUTPUT: usize = 3;
    const DEPTH: usize = 4;

    // Common subpasses
    const FILL_GEOM_BUF_DESC: SubpassDesc = SubpassDesc {
        colors: &[
            (COLOR_METAL, ColorAttachmentOptimal),
            (NORMAL_ROUGH, ColorAttachmentOptimal),
        ],
        depth_stencil: Some(&(DEPTH, DepthStencilAttachmentOptimal)),
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };
    const ACCUM_LIGHT_DESC: SubpassDesc = SubpassDesc {
        colors: &[(LIGHT, ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (NORMAL_ROUGH, ShaderReadOnlyOptimal),
            (DEPTH, ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[COLOR_METAL],
    };
    const TONEMAP_DESC: SubpassDesc = SubpassDesc {
        colors: &[(OUTPUT, ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[
            (COLOR_METAL, ShaderReadOnlyOptimal),
            (NORMAL_ROUGH, ShaderReadOnlyOptimal),
            (LIGHT, ShaderReadOnlyOptimal),
        ],
        resolves: &[],
        preserves: &[],
    };

    fn color_attachment(fmt: Format, ops: AttachmentOps) -> Attachment {
        Attachment {
            format: Some(fmt),
            samples: 1,
            ops,
            stencil_ops: DONT_CARE,
            layouts: const_layout(ColorAttachmentOptimal),
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
            layouts: DepthStencilAttachmentOptimal..DepthStencilReadOnlyOptimal,
        }
    }

    pub(in crate::gpu) unsafe fn fill_light_tonemap(mode: DrawRenderPassMode) -> RenderPass {
        // Subpass indexes
        const FILL_GEOM_BUF_IDX: u8 = 0;
        const ACCUM_LIGHT_IDX: u8 = 1;
        const TONEMAP_IDX: u8 = 2;

        // Attachment instances
        let color_metal = color_attachment(mode.geom_buf, DONT_CARE);
        let normal_rough = color_attachment(mode.geom_buf, DONT_CARE);
        let light = color_attachment(mode.light, CLEAR_DONT_CARE);
        let output = color_attachment(mode.output, PRESERVE);
        let depth = depth_stencil_attachment(mode.depth, CLEAR_DONT_CARE, DONT_CARE);

        // TODO: These things hurt my brain are they correct how do I tell ugh
        // Subpass-to-Subpass dependencies
        let begin = SubpassDependency {
            passes: None..Some(FILL_GEOM_BUF_IDX),
            stages: BOTTOM_OF_PIPE..COLOR_ATTACHMENT_OUTPUT,
            accesses: MEMORY_READ..COLOR_ATTACHMENT_READ | COLOR_ATTACHMENT_WRITE,
            flags: Dependencies::BY_REGION,
        };
        let between_fill_and_light = SubpassDependency {
            passes: Some(FILL_GEOM_BUF_IDX)..Some(ACCUM_LIGHT_IDX),
            stages: COLOR_ATTACHMENT_OUTPUT..FRAGMENT_SHADER,
            accesses: COLOR_ATTACHMENT_WRITE..SHADER_READ,
            flags: Dependencies::BY_REGION,
        };
        let end = SubpassDependency {
            passes: Some(FILL_GEOM_BUF_IDX)..None,
            stages: COLOR_ATTACHMENT_OUTPUT..BOTTOM_OF_PIPE,
            accesses: COLOR_ATTACHMENT_READ | COLOR_ATTACHMENT_WRITE..MEMORY_READ,
            flags: Dependencies::BY_REGION,
        };

        RenderPass::new_dependencies(
            #[cfg(feature = "debug-names")]
            "Draw",
            vec![
                // attachments
                color_metal,
                normal_rough,
                light,
                output,
                depth,
            ],
            vec![
                // subpasses
                FILL_GEOM_BUF_DESC,
                ACCUM_LIGHT_DESC,
                TONEMAP_DESC,
            ],
            vec![
                // dependencies
                begin,
                between_fill_and_light,
                end,
            ],
        )
    }

    /// Like the draw render pass except it contains a step between filling the geometry buffer and
    /// accumulating light
    pub(in crate::gpu) unsafe fn fill_skydome_light_tonemap(
        mode: DrawRenderPassMode,
    ) -> RenderPass {
        // Subpass indexes
        const FILL_GEOM_BUF_IDX: u8 = 0;
        const SKYDOME_IDX: u8 = 0;
        const ACCUM_LIGHT_IDX: u8 = 1;
        const TONEMAP_IDX: u8 = 2;

        // Attachment instances
        let color_metal = color_attachment(mode.geom_buf, DONT_CARE);
        let normal_rough = color_attachment(mode.geom_buf, DONT_CARE);
        let light = color_attachment(mode.light, CLEAR_DONT_CARE);
        let output = color_attachment(mode.output, PRESERVE);
        let depth = depth_stencil_attachment(mode.depth, CLEAR_DONT_CARE, DONT_CARE);

        // Subpasses
        let skydome_subpass_desc = SubpassDesc {
            colors: &[(COLOR_METAL, ColorAttachmentOptimal)],
            depth_stencil: Some(&(DEPTH, DepthStencilAttachmentOptimal)),
            inputs: &[],
            resolves: &[],
            preserves: &[NORMAL_ROUGH],
        };

        // TODO: These things hurt my brain are they correct how do I tell ugh
        // Subpass-to-Subpass dependencies
        let begin = SubpassDependency {
            passes: None..Some(FILL_GEOM_BUF_IDX),
            stages: BOTTOM_OF_PIPE..COLOR_ATTACHMENT_OUTPUT,
            accesses: MEMORY_READ..COLOR_ATTACHMENT_READ | COLOR_ATTACHMENT_WRITE,
            flags: Dependencies::BY_REGION,
        };
        let between_fill_and_light = SubpassDependency {
            passes: Some(FILL_GEOM_BUF_IDX)..Some(ACCUM_LIGHT_IDX),
            stages: COLOR_ATTACHMENT_OUTPUT..FRAGMENT_SHADER,
            accesses: COLOR_ATTACHMENT_WRITE..SHADER_READ,
            flags: Dependencies::BY_REGION,
        };
        let end = SubpassDependency {
            passes: Some(FILL_GEOM_BUF_IDX)..None,
            stages: COLOR_ATTACHMENT_OUTPUT..BOTTOM_OF_PIPE,
            accesses: COLOR_ATTACHMENT_READ | COLOR_ATTACHMENT_WRITE..MEMORY_READ,
            flags: Dependencies::BY_REGION,
        };

        RenderPass::new_dependencies(
            #[cfg(feature = "debug-names")]
            "Draw",
            vec![
                // attachments
                color_metal,
                normal_rough,
                light,
                output,
                depth,
            ],
            vec![
                // subpassess
                FILL_GEOM_BUF_DESC,
                skydome_subpass_desc,
                ACCUM_LIGHT_DESC,
                TONEMAP_DESC,
            ],
            vec![
                // dependencies
                begin,
                between_fill_and_light,
                end,
            ],
        )
    }

    /// Like the draw render pass except it contains a 'post'-fx step
    pub(in crate::gpu) fn fill_light_tonemap_fx(_mode: DrawRenderPassMode) -> RenderPass {
        todo!();
    }

    /// Like the draw render pass except it contains a 'pre' and 'post'-fx step
    pub(in crate::gpu) fn fill_skydome_light_tonemap_fx(_mode: DrawRenderPassMode) -> RenderPass {
        todo!();
    }
}

use {
    super::{ColorRenderPassMode, DrawRenderPassMode},
    crate::gpu::driver::RenderPass,
    gfx_hal::{
        format::Format,
        image::{Access, Layout, Layout::*},
        memory::Dependencies,
        pass::{
            Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDependency,
            SubpassDesc,
        },
        pso::PipelineStage,
    },
    std::{
        iter::{once},
        ops::Range,
    },
};

// Image Access helpers (pulled from v0.)
const INPUT_ATTACHMENT_READ: Access = Access::INPUT_ATTACHMENT_READ;
const SHADER_READ: Access = Access::SHADER_READ;
const SHADER_WRITE: Access = Access::SHADER_WRITE;
const COLOR_ATTACHMENT_READ: Access = Access::COLOR_ATTACHMENT_READ;
const COLOR_ATTACHMENT_WRITE: Access = Access::COLOR_ATTACHMENT_WRITE;
const DEPTH_STENCIL_ATTACHMENT_READ: Access = Access::DEPTH_STENCIL_ATTACHMENT_READ;
const DEPTH_STENCIL_ATTACHMENT_WRITE: Access = Access::DEPTH_STENCIL_ATTACHMENT_WRITE;
const TRANSFER_READ: Access = Access::TRANSFER_READ;
const TRANSFER_WRITE: Access = Access::TRANSFER_WRITE;
const HOST_READ: Access = Access::HOST_READ;
const HOST_WRITE: Access = Access::HOST_WRITE;
const MEMORY_READ: Access = Access::MEMORY_READ;
const MEMORY_WRITE: Access = Access::MEMORY_WRITE;

// PipelineStage helpers
const TOP_OF_PIPE: PipelineStage = PipelineStage::TOP_OF_PIPE;
const DRAW_INDIRECT: PipelineStage = PipelineStage::DRAW_INDIRECT;
const VERTEX_INPUT: PipelineStage = PipelineStage::VERTEX_INPUT;
const VERTEX_SHADER: PipelineStage = PipelineStage::VERTEX_SHADER;
const HULL_SHADER: PipelineStage = PipelineStage::HULL_SHADER;
const DOMAIN_SHADER: PipelineStage = PipelineStage::DOMAIN_SHADER;
const GEOMETRY_SHADER: PipelineStage = PipelineStage::GEOMETRY_SHADER;
const FRAGMENT_SHADER: PipelineStage = PipelineStage::FRAGMENT_SHADER;
const EARLY_FRAGMENT_TESTS: PipelineStage = PipelineStage::EARLY_FRAGMENT_TESTS;
const LATE_FRAGMENT_TESTS: PipelineStage = PipelineStage::LATE_FRAGMENT_TESTS;
const COLOR_ATTACHMENT_OUTPUT: PipelineStage = PipelineStage::COLOR_ATTACHMENT_OUTPUT;
const COMPUTE_SHADER: PipelineStage = PipelineStage::COMPUTE_SHADER;
const TRANSFER: PipelineStage = PipelineStage::TRANSFER;
const BOTTOM_OF_PIPE: PipelineStage = PipelineStage::BOTTOM_OF_PIPE;
const HOST: PipelineStage = PipelineStage::HOST;
const TASK_SHADER: PipelineStage = PipelineStage::TASK_SHADER;
const MESH_SHADER: PipelineStage = PipelineStage::MESH_SHADER;

// AttachmentOps helpers
const CLEAR_DONT_CARE: AttachmentOps = AttachmentOps {
    load: AttachmentLoadOp::Clear,
    store: AttachmentStoreOp::DontCare,
};
const DONT_CARE_STORE: AttachmentOps = AttachmentOps {
    load: AttachmentLoadOp::DontCare,
    store: AttachmentStoreOp::Store,
};
const DONT_CARE: AttachmentOps = AttachmentOps::DONT_CARE;
const INIT: AttachmentOps = AttachmentOps::INIT;
const PRESERVE: AttachmentOps = AttachmentOps::PRESERVE;

fn const_layout(layout: Layout) -> Range<Layout> {
    layout..layout
}

pub(in crate::gpu) unsafe fn color(mode: ColorRenderPassMode) -> RenderPass {
    const ATTACHMENT: usize = 0;

    let attachment = Attachment {
        format: Some(mode.fmt),
        samples: 1,
        ops: if mode.preserve {
            PRESERVE
        } else {
            DONT_CARE_STORE
        },
        stencil_ops: DONT_CARE,
        layouts: ColorAttachmentOptimal..ColorAttachmentOptimal,
    };
    let subpass_desc = SubpassDesc {
        colors: &[(ATTACHMENT, ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };

    RenderPass::new(
        #[cfg(feature = "debug-names")]
        "Color",
        once(attachment),
        once(subpass_desc),
    )
}

pub unsafe fn present(fmt: Format) -> RenderPass {
    const ATTACHMENT: usize = 0;

    let attachment = Attachment {
        format: Some(fmt),
        samples: 1,
        ops: DONT_CARE_STORE,
        stencil_ops: DONT_CARE,
        layouts: Undefined..Present,
    };
    let subpass_desc = SubpassDesc {
        colors: &[(ATTACHMENT, ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[],
        resolves: &[],
        preserves: &[],
    };

    RenderPass::new(
        #[cfg(feature = "debug-names")]
        "Present",
        once(attachment),
        once(subpass_desc),
    )
}
