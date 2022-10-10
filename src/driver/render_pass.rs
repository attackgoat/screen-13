use {
    super::{DepthStencilMode, Device, DriverError, GraphicPipeline, SampleCount},
    ash::vk,
    log::{trace, warn},
    parking_lot::Mutex,
    std::{
        collections::{btree_map::Entry, BTreeMap},
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
    pub fn new(fmt: vk::Format, sample_count: SampleCount) -> Self {
        AttachmentInfo {
            flags: vk::AttachmentDescriptionFlags::MAY_ALIAS,
            fmt,
            sample_count,
            initial_layout: vk::ImageLayout::UNDEFINED,
            load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            store_op: vk::AttachmentStoreOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            final_layout: vk::ImageLayout::UNDEFINED,
        }
    }

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

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FramebufferKey {
    pub attachments: Vec<FramebufferKeyAttachment>,
    pub extent_x: u32,
    pub extent_y: u32,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FramebufferKeyAttachment {
    pub flags: vk::ImageCreateFlags,
    pub usage: vk::ImageUsageFlags,
    pub extent_x: u32,
    pub extent_y: u32,
    pub layer_count: u32,
    pub view_fmts: Vec<vk::Format>,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GraphicPipelineKey {
    depth_stencil: Option<DepthStencilMode>,
    layout: vk::PipelineLayout,
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
    framebuffer_cache: Mutex<BTreeMap<FramebufferKey, vk::Framebuffer>>,
    graphic_pipeline_cache: Mutex<BTreeMap<GraphicPipelineKey, vk::Pipeline>>,
    pub info: RenderPassInfo,
    render_pass: vk::RenderPass,
}

impl RenderPass {
    pub fn create(device: &Arc<Device>, info: RenderPassInfo) -> Result<Self, DriverError> {
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

        // This vec must stay alive until the create function completes; it holds the attachments that
        // create info points to (ash builder lifetime)
        let mut subpass_attachments_ref = vec![];

        let mut subpasses = Vec::with_capacity(info.subpasses.len());
        for (subpass, subpass_attachments) in info.subpasses.iter().map(move |subpass| {
            let mut attachments = vec![];

            let mut color_attachments = None;
            if !subpass.color_attachments.is_empty() {
                color_attachments =
                    Some(attachments.len()..attachments.len() + subpass.color_attachments.len());
                attachments.extend(
                    subpass
                        .color_attachments
                        .iter()
                        .copied()
                        .map(|attachment| attachment.into_vk().build()),
                );
            }

            let mut input_attachments = None;
            if !subpass.input_attachments.is_empty() {
                input_attachments =
                    Some(attachments.len()..attachments.len() + subpass.input_attachments.len());
                attachments.extend(
                    subpass
                        .input_attachments
                        .iter()
                        .copied()
                        .map(|attachment| attachment.into_vk().build()),
                );
            }

            let mut resolve_attachments = None;
            if !subpass.resolve_attachments.is_empty() {
                resolve_attachments =
                    Some(attachments.len()..attachments.len() + subpass.resolve_attachments.len());
                attachments.extend(
                    subpass
                        .resolve_attachments
                        .iter()
                        .copied()
                        .map(|attachment| attachment.into_vk().build()),
                );
            }

            let mut subpass_desc = vk::SubpassDescription2::builder()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

            if let Some(depth_stencil_attachment) = subpass.depth_stencil_attachment {
                let idx = attachments.len();
                attachments.push(depth_stencil_attachment.into_vk().build());
                subpass_desc = subpass_desc.depth_stencil_attachment(&attachments[idx]);
            }

            let subpass = subpass_desc
                .color_attachments(&attachments[color_attachments.unwrap_or_default()])
                .input_attachments(&attachments[input_attachments.unwrap_or_default()])
                .resolve_attachments(&attachments[resolve_attachments.unwrap_or_default()])
                .preserve_attachments(&subpass.preserve_attachments)
                .build();

            (subpass, attachments)
        }) {
            subpasses.push(subpass);
            subpass_attachments_ref.push(subpass_attachments);
        }

        #[cfg(not(target_os = "macos"))]
        let render_pass = unsafe {
            device.create_render_pass2(
                &vk::RenderPassCreateInfo2::builder()
                    .flags(vk::RenderPassCreateFlags::empty())
                    .attachments(&attachments)
                    .dependencies(&dependencies)
                    .subpasses(&subpasses),
                None,
            )
        };

        // TODO: This needs some help, above, to get the correct types - also needs fixes in resolver!!!
        #[cfg(target_os = "macos")]
        let render_pass = unsafe {
            device.create_render_pass(
                &vk::RenderPassCreateInfo::builder()
                    .flags(vk::RenderPassCreateFlags::empty())
                    .attachments(&attachments)
                    .dependencies(&dependencies)
                    .subpasses(&subpasses),
                None,
            )
        };

        // Needs contributors (or hardware!) MoltenVK is about to goto 1.2 which makes this better
        #[cfg(target_os = "macos")]
        todo!("There is a description of this issue in the source code that caused this panic");

        let render_pass = render_pass.map_err(|_| DriverError::InvalidData)?;

        Ok(Self {
            info,
            device,
            framebuffer_cache: Mutex::new(Default::default()),
            graphic_pipeline_cache: Mutex::new(Default::default()),
            render_pass,
        })
    }

    pub fn framebuffer_ref(&self, info: FramebufferKey) -> Result<vk::Framebuffer, DriverError> {
        debug_assert!(!info.attachments.is_empty());

        let mut cache = self.framebuffer_cache.lock();
        let entry = cache.entry(info);
        if let Entry::Occupied(entry) = entry {
            return Ok(*entry.get());
        }

        let entry = match entry {
            Entry::Vacant(entry) => entry,
            _ => unreachable!(),
        };

        let key = entry.key();
        let layers = key.attachments.iter().map(|attachment| attachment.layer_count).max().unwrap_or(1);
        let attachments = key
            .attachments
            .iter()
            .map(|attachment| {
                vk::FramebufferAttachmentImageInfo::builder()
                    .flags(attachment.flags)
                    .width(attachment.extent_x)
                    .height(attachment.extent_y)
                    .layer_count(attachment.layer_count)
                    .usage(attachment.usage)
                    .view_formats(attachment.view_fmts.as_slice())
                    .build()
            })
            .collect::<Box<[_]>>();
        let mut imageless_info =
            vk::FramebufferAttachmentsCreateInfoKHR::builder().attachment_image_infos(&attachments);
        let mut create_info = vk::FramebufferCreateInfo::builder()
            .flags(vk::FramebufferCreateFlags::IMAGELESS)
            .render_pass(self.render_pass)
            .width(key.extent_x)
            .height(key.extent_y)
            .layers(layers)
            .push_next(&mut imageless_info);
        create_info.attachment_count = self.info.attachments.len() as _;

        let framebuffer = unsafe {
            self.device
                .create_framebuffer(&create_info, None)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
        };

        entry.insert(framebuffer);

        Ok(framebuffer)
    }

    pub fn graphic_pipeline_ref(
        &self,
        pipeline: &Arc<GraphicPipeline>,
        depth_stencil: Option<DepthStencilMode>,
        subpass_idx: u32,
    ) -> Result<vk::Pipeline, DriverError> {
        use std::slice::from_ref;

        let mut cache = self.graphic_pipeline_cache.lock();
        let entry = cache.entry(GraphicPipelineKey {
            depth_stencil,
            layout: pipeline.layout,
            subpass_idx,
        });
        if let Entry::Occupied(entry) = entry {
            return Ok(*entry.get());
        }

        let entry = match entry {
            Entry::Vacant(entry) => entry,
            _ => unreachable!(),
        };

        let color_blend_attachment_states = self
            .info
            .attachments
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
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
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
            .render_pass(self.render_pass)
            .stages(&stages)
            .subpass(subpass_idx)
            .vertex_input_state(&vertex_input_state)
            .viewport_state(&viewport_state);

        let pipeline = unsafe {
            self.device.create_graphics_pipelines(
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
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            for framebuffer in self.framebuffer_cache.lock().values().copied() {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            for pipeline in self.graphic_pipeline_cache.lock().values().copied() {
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
    pub depth_stencil_attachment: Option<AttachmentRef>,
    pub input_attachments: Vec<AttachmentRef>,
    pub preserve_attachments: Vec<u32>,
    pub resolve_attachments: Vec<AttachmentRef>,
}

impl SubpassInfo {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            color_attachments: Vec::with_capacity(capacity),
            depth_stencil_attachment: None,
            input_attachments: Vec::with_capacity(capacity),
            preserve_attachments: Vec::with_capacity(capacity),
            resolve_attachments: Vec::with_capacity(capacity),
        }
    }

    pub fn has_multiple_attachments(&self) -> bool {
        let count = self.depth_stencil_attachment.is_some() as usize + self.color_attachments.len();

        count > 1
    }
}
