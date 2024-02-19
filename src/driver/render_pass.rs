use {
    super::{
        device::Device, DepthStencilMode, DriverError, GraphicPipeline, ResolveMode, SampleCount,
    },
    ash::vk,
    log::{trace, warn},
    std::{
        collections::{hash_map::Entry, HashMap},
        ops::Deref,
        sync::Arc,
        thread::panicking,
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AttachmentInfo {
    pub flags: vk::AttachmentDescriptionFlags,
    pub fmt: vk::Format,
    pub sample_count: SampleCount,
    pub load_op: vk::AttachmentLoadOp,
    pub store_op: vk::AttachmentStoreOp,
    pub stencil_load_op: vk::AttachmentLoadOp,
    pub stencil_store_op: vk::AttachmentStoreOp,
    pub initial_layout: vk::ImageLayout,
    pub final_layout: vk::ImageLayout,
}

impl AttachmentInfo {
    pub fn into_vk(self) -> vk::AttachmentDescription2 {
        vk::AttachmentDescription2::builder()
            .flags(self.flags)
            .format(self.fmt)
            .samples(self.sample_count.into_vk())
            .load_op(self.load_op)
            .store_op(self.store_op)
            .stencil_load_op(self.stencil_load_op)
            .stencil_store_op(self.stencil_store_op)
            .initial_layout(self.initial_layout)
            .final_layout(self.final_layout)
            .build()
    }
}

impl Default for AttachmentInfo {
    fn default() -> Self {
        AttachmentInfo {
            flags: vk::AttachmentDescriptionFlags::MAY_ALIAS,
            fmt: vk::Format::UNDEFINED,
            sample_count: SampleCount::Type1,
            initial_layout: vk::ImageLayout::UNDEFINED,
            load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            store_op: vk::AttachmentStoreOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            final_layout: vk::ImageLayout::UNDEFINED,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AttachmentRef {
    pub attachment: u32,
    pub aspect_mask: vk::ImageAspectFlags,
    pub layout: vk::ImageLayout,
}

impl AttachmentRef {
    fn into_vk(self) -> vk::AttachmentReference2Builder<'static> {
        vk::AttachmentReference2::builder()
            .attachment(self.attachment)
            .aspect_mask(self.aspect_mask)
            .layout(self.layout)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FramebufferAttachmentImageInfo {
    pub flags: vk::ImageCreateFlags,
    pub usage: vk::ImageUsageFlags,
    pub width: u32,
    pub height: u32,
    pub layer_count: u32,
    pub view_formats: Vec<vk::Format>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FramebufferInfo {
    pub attachments: Vec<FramebufferAttachmentImageInfo>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct GraphicPipelineKey {
    depth_stencil: Option<DepthStencilMode>,
    layout: vk::PipelineLayout,
    shader_modules: Vec<vk::ShaderModule>,
    subpass_idx: u32,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct RenderPassInfo {
    pub attachments: Vec<AttachmentInfo>,
    pub subpasses: Vec<SubpassInfo>,
    pub dependencies: Vec<SubpassDependency>,
}

#[derive(Debug)]
pub struct RenderPass {
    device: Arc<Device>,
    framebuffers: HashMap<FramebufferInfo, vk::Framebuffer>,
    graphic_pipelines: HashMap<GraphicPipelineKey, vk::Pipeline>,
    pub info: RenderPassInfo,
    render_pass: vk::RenderPass,
}

impl RenderPass {
    #[profiling::function]
    pub fn create(device: &Arc<Device>, info: RenderPassInfo) -> Result<Self, DriverError> {
        //trace!("create: \n{:#?}", &info);
        trace!("create");

        let device = Arc::clone(device);
        let attachments = info
            .attachments
            .iter()
            .map(|attachment| attachment.into_vk())
            .collect::<Box<[_]>>();
        let dependencies = info
            .dependencies
            .iter()
            .map(|dependency| dependency.into_vk())
            .collect::<Box<[_]>>();

        // These vecs must stay alive and not be resized until the create function completes!
        let mut subpass_attachments = Vec::with_capacity(
            info.subpasses
                .iter()
                .map(|subpass| {
                    subpass.color_attachments.len() * 2
                        + subpass.input_attachments.len()
                        + subpass.depth_stencil_attachment.is_some() as usize
                        + subpass.depth_stencil_resolve_attachment.is_some() as usize
                })
                .sum(),
        );
        let mut subpasses = Vec::with_capacity(info.subpasses.len());
        let mut subpass_depth_stencil_resolves = Vec::with_capacity(info.subpasses.len());
        let mut has_viewmasks = false;

        for subpass in &info.subpasses {
            has_viewmasks |= subpass.view_mask != 0;

            let mut subpass_desc = vk::SubpassDescription2::builder()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

            debug_assert_eq!(
                subpass.color_attachments.len(),
                subpass.color_resolve_attachments.len()
            );

            let color_attachments_idx = subpass_attachments.len();
            let input_attachments_idx = color_attachments_idx + subpass.color_attachments.len();
            let resolve_color_attachments_idx =
                input_attachments_idx + subpass.input_attachments.len();
            subpass_attachments.extend(
                subpass
                    .color_attachments
                    .iter()
                    .chain(subpass.input_attachments.iter())
                    .chain(subpass.color_resolve_attachments.iter())
                    .map(|attachment| attachment.into_vk().build()),
            );

            let resolve_depth_stencil_attachment_idx = subpass_attachments.len();
            if let Some((resolve_attachment, depth_resolve_mode, stencil_resolve_mode)) =
                subpass.depth_stencil_resolve_attachment
            {
                subpass_attachments.push(resolve_attachment.into_vk().build());
                subpass_depth_stencil_resolves.push(
                    vk::SubpassDescriptionDepthStencilResolve::builder()
                        .depth_stencil_resolve_attachment(subpass_attachments.last().unwrap())
                        .depth_resolve_mode(ResolveMode::into_vk(depth_resolve_mode))
                        .stencil_resolve_mode(ResolveMode::into_vk(stencil_resolve_mode))
                        .build(),
                );

                subpass_desc =
                    subpass_desc.push_next(subpass_depth_stencil_resolves.last_mut().unwrap());
            }

            if let Some(depth_stencil_attachment) = subpass.depth_stencil_attachment {
                subpass_attachments.push(depth_stencil_attachment.into_vk().build());
                subpass_desc =
                    subpass_desc.depth_stencil_attachment(subpass_attachments.last().unwrap());
            }

            subpasses.push(
                subpass_desc
                    .color_attachments(
                        &subpass_attachments[color_attachments_idx..input_attachments_idx],
                    )
                    .input_attachments(
                        &subpass_attachments[input_attachments_idx..resolve_color_attachments_idx],
                    )
                    .resolve_attachments(
                        &subpass_attachments
                            [resolve_color_attachments_idx..resolve_depth_stencil_attachment_idx],
                    )
                    .preserve_attachments(&subpass.preserve_attachments)
                    .view_mask(subpass.view_mask)
                    .build(),
            );
        }

        let correlated_view_masks = if has_viewmasks {
            info.subpasses
                .iter()
                .map(|subpass| subpass.correlated_view_mask)
                .collect::<Box<_>>()
        } else {
            Box::new([])
        };

        let render_pass = unsafe {
            device.create_render_pass2(
                &vk::RenderPassCreateInfo2::builder()
                    .flags(vk::RenderPassCreateFlags::empty())
                    .attachments(&attachments)
                    .dependencies(&dependencies)
                    .subpasses(&subpasses)
                    .correlated_view_masks(&correlated_view_masks),
                None,
            )
        };

        let render_pass = render_pass.map_err(|_| DriverError::InvalidData)?;

        Ok(Self {
            info,
            device,
            framebuffers: Default::default(),
            graphic_pipelines: Default::default(),
            render_pass,
        })
    }

    #[profiling::function]
    pub fn framebuffer(
        this: &mut Self,
        info: FramebufferInfo,
    ) -> Result<vk::Framebuffer, DriverError> {
        debug_assert!(!info.attachments.is_empty());

        let entry = this.framebuffers.entry(info);
        if let Entry::Occupied(entry) = entry {
            return Ok(*entry.get());
        }

        let entry = match entry {
            Entry::Vacant(entry) => entry,
            _ => unreachable!(),
        };

        let key = entry.key();
        let layers = key
            .attachments
            .iter()
            .map(|attachment| attachment.layer_count)
            .max()
            .unwrap_or(1);
        let attachments = key
            .attachments
            .iter()
            .map(|attachment| {
                vk::FramebufferAttachmentImageInfo::builder()
                    .flags(attachment.flags)
                    .width(attachment.width)
                    .height(attachment.height)
                    .layer_count(attachment.layer_count)
                    .usage(attachment.usage)
                    .view_formats(&attachment.view_formats)
                    .build()
            })
            .collect::<Box<[_]>>();
        let mut imageless_info =
            vk::FramebufferAttachmentsCreateInfoKHR::builder().attachment_image_infos(&attachments);
        let mut create_info = vk::FramebufferCreateInfo::builder()
            .flags(vk::FramebufferCreateFlags::IMAGELESS)
            .render_pass(this.render_pass)
            .width(key.width)
            .height(key.height)
            .layers(layers)
            .push_next(&mut imageless_info);
        create_info.attachment_count = this.info.attachments.len() as _;

        let framebuffer = unsafe {
            this.device
                .create_framebuffer(&create_info, None)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
        };

        entry.insert(framebuffer);

        Ok(framebuffer)
    }

    #[profiling::function]
    pub fn graphic_pipeline(
        this: &mut Self,
        pipeline: &Arc<GraphicPipeline>,
        depth_stencil: Option<DepthStencilMode>,
        subpass_idx: u32,
    ) -> Result<vk::Pipeline, DriverError> {
        use std::slice::from_ref;

        let entry = this.graphic_pipelines.entry(GraphicPipelineKey {
            depth_stencil,
            layout: pipeline.layout,
            shader_modules: pipeline.shader_modules.clone(),
            subpass_idx,
        });
        if let Entry::Occupied(entry) = entry {
            return Ok(*entry.get());
        }

        let entry = match entry {
            Entry::Vacant(entry) => entry,
            _ => unreachable!(),
        };

        let color_blend_attachment_states = this.info.subpasses[subpass_idx as usize]
            .color_attachments
            .iter()
            .map(|_| pipeline.info.blend.into_vk())
            .collect::<Box<[_]>>();
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .attachments(&color_blend_attachment_states);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_states);
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
            .alpha_to_coverage_enable(pipeline.state.multisample.alpha_to_coverage_enable)
            .alpha_to_one_enable(pipeline.state.multisample.alpha_to_one_enable)
            .flags(pipeline.state.multisample.flags)
            .min_sample_shading(pipeline.state.multisample.min_sample_shading)
            .rasterization_samples(pipeline.state.multisample.rasterization_samples.into_vk())
            .sample_shading_enable(pipeline.state.multisample.sample_shading_enable)
            .sample_mask(&pipeline.state.multisample.sample_mask);
        let mut specializations = Vec::with_capacity(pipeline.state.stages.len());
        let stages = pipeline
            .state
            .stages
            .iter()
            .map(|stage| {
                let mut info = vk::PipelineShaderStageCreateInfo::builder()
                    .module(stage.module)
                    .name(&stage.name)
                    .stage(stage.flags);

                if let Some(specialization_info) = &stage.specialization_info {
                    specializations.push(
                        vk::SpecializationInfo::builder()
                            .map_entries(&specialization_info.map_entries)
                            .data(&specialization_info.data)
                            .build(),
                    );

                    info = info.specialization_info(specializations.last().unwrap());
                }

                info.build()
            })
            .collect::<Box<[_]>>();
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(
                &pipeline.state.vertex_input.vertex_attribute_descriptions,
            )
            .vertex_binding_descriptions(&pipeline.state.vertex_input.vertex_binding_descriptions);
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            .scissor_count(1);
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
            topology: pipeline.info.topology,
            ..Default::default()
        };
        let depth_stencil = depth_stencil
            .map(|depth_stencil| depth_stencil.into_vk())
            .unwrap_or_default();
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo {
            front_face: pipeline.info.front_face,
            line_width: 1.0,
            polygon_mode: pipeline.info.polygon_mode,
            cull_mode: pipeline.info.cull_mode,
            ..Default::default()
        };
        let graphic_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .color_blend_state(&color_blend_state)
            .depth_stencil_state(&depth_stencil)
            .dynamic_state(&dynamic_state)
            .input_assembly_state(&input_assembly_state)
            .layout(pipeline.state.layout)
            .multisample_state(&multisample_state)
            .rasterization_state(&rasterization_state)
            .render_pass(this.render_pass)
            .stages(&stages)
            .subpass(subpass_idx)
            .vertex_input_state(&vertex_input_state)
            .viewport_state(&viewport_state);

        let pipeline = unsafe {
            this.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                from_ref(&graphic_pipeline_info),
                None,
            )
        }
        .map_err(|(_, err)| {
            warn!(
                "create_graphics_pipelines: {err}\n{:#?}",
                graphic_pipeline_info.build()
            );

            DriverError::Unsupported
        })?[0];

        entry.insert(pipeline);

        Ok(pipeline)
    }
}

impl Deref for RenderPass {
    type Target = vk::RenderPass;

    fn deref(&self) -> &Self::Target {
        &self.render_pass
    }
}

impl Drop for RenderPass {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            for (_, framebuffer) in self.framebuffers.drain() {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            for (_, pipeline) in self.graphic_pipelines.drain() {
                self.device.destroy_pipeline(pipeline, None);
            }

            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SubpassDependency {
    pub src_subpass: u32,
    pub dst_subpass: u32,
    pub src_stage_mask: vk::PipelineStageFlags,
    pub dst_stage_mask: vk::PipelineStageFlags,
    pub src_access_mask: vk::AccessFlags,
    pub dst_access_mask: vk::AccessFlags,
    pub dependency_flags: vk::DependencyFlags,
}

impl SubpassDependency {
    pub fn new(src_subpass: u32, dst_subpass: u32) -> Self {
        Self {
            src_subpass,
            dst_subpass,
            src_stage_mask: vk::PipelineStageFlags::empty(),
            dst_stage_mask: vk::PipelineStageFlags::empty(),
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::empty(),
            dependency_flags: vk::DependencyFlags::empty(),
        }
    }

    pub fn into_vk(self) -> vk::SubpassDependency2 {
        vk::SubpassDependency2::builder()
            .src_subpass(self.src_subpass)
            .dst_subpass(self.dst_subpass)
            .src_stage_mask(self.src_stage_mask)
            .dst_stage_mask(self.dst_stage_mask)
            .src_access_mask(self.src_access_mask)
            .dst_access_mask(self.dst_access_mask)
            .dependency_flags(self.dependency_flags)
            .build()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SubpassInfo {
    pub color_attachments: Vec<AttachmentRef>,
    pub color_resolve_attachments: Vec<AttachmentRef>,
    pub correlated_view_mask: u32,
    pub depth_stencil_attachment: Option<AttachmentRef>,
    pub depth_stencil_resolve_attachment:
        Option<(AttachmentRef, Option<ResolveMode>, Option<ResolveMode>)>,
    pub input_attachments: Vec<AttachmentRef>,
    pub preserve_attachments: Vec<u32>,
    pub view_mask: u32,
}

impl SubpassInfo {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            color_attachments: Vec::with_capacity(capacity),
            color_resolve_attachments: Vec::with_capacity(capacity),
            correlated_view_mask: 0,
            depth_stencil_attachment: None,
            depth_stencil_resolve_attachment: None,
            input_attachments: Vec::with_capacity(capacity),
            preserve_attachments: Vec::with_capacity(capacity),
            view_mask: 0,
        }
    }
}
