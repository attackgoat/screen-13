use {
    super::{DepthStencilMode, Device, DriverError, GraphicPipeline, SampleCount},
    crate::ptr::Shared,
    archery::SharedPointerKind,
    ash::vk,
    derive_builder::Builder,
    log::trace,
    parking_lot::Mutex,
    std::{
        cell::RefCell,
        collections::{btree_map::Entry, BTreeMap},
        ops::Deref,
        ptr::null,
        thread::panicking,
    },
};

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned")]
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
    #[allow(clippy::new_ret_no_self)]
    pub fn new(fmt: vk::Format, sample_count: SampleCount) -> AttachmentInfoBuilder {
        AttachmentInfoBuilder {
            flags: Some(vk::AttachmentDescriptionFlags::MAY_ALIAS),
            fmt: Some(fmt),
            sample_count: Some(sample_count),
            initial_layout: Some(vk::ImageLayout::UNDEFINED),
            load_op: Some(vk::AttachmentLoadOp::DONT_CARE),
            stencil_load_op: Some(vk::AttachmentLoadOp::DONT_CARE),
            store_op: Some(vk::AttachmentStoreOp::DONT_CARE),
            stencil_store_op: Some(vk::AttachmentStoreOp::DONT_CARE),
            final_layout: Some(vk::ImageLayout::UNDEFINED),
        }
    }

    pub fn into_vk(self) -> vk::AttachmentDescription {
        vk::AttachmentDescription {
            flags: self.flags,
            format: self.fmt,
            samples: self.sample_count.into_vk(),
            load_op: self.load_op,
            store_op: self.store_op,
            stencil_load_op: self.stencil_load_op,
            stencil_store_op: self.stencil_store_op,
            initial_layout: self.initial_layout,
            final_layout: self.final_layout,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AttachmentRef {
    pub attachment: u32,
    pub layout: vk::ImageLayout,
}

impl AttachmentRef {
    pub fn new(attachment: u32, layout: vk::ImageLayout) -> Self {
        Self { attachment, layout }
    }

    fn into_vk(self) -> vk::AttachmentReference {
        vk::AttachmentReference {
            attachment: self.attachment,
            layout: self.layout,
        }
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
    pipeline: usize,
    depth_stencil: Option<DepthStencilMode>,
}

#[derive(Builder, Clone, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned", derive(Debug))]
pub struct RenderPassInfo {
    pub attachments: Vec<AttachmentInfo>,
    pub subpasses: Vec<SubpassInfo>,
    pub dependencies: Vec<SubpassDependency>,
}

impl RenderPassInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> RenderPassInfoBuilder {
        Default::default()
    }
}

#[derive(Debug)]
pub struct RenderPass<P>
where
    P: SharedPointerKind,
{
    device: Shared<Device<P>, P>,
    framebuffer_cache: Mutex<BTreeMap<FramebufferKey, vk::Framebuffer>>,
    graphic_pipeline_cache: Mutex<BTreeMap<GraphicPipelineKey, vk::Pipeline>>,
    pub info: RenderPassInfo,
    render_pass: vk::RenderPass,
}

impl<P> RenderPass<P>
where
    P: SharedPointerKind,
{
    pub fn create(device: &Shared<Device<P>, P>, info: RenderPassInfo) -> Result<Self, DriverError>
    where
        P: SharedPointerKind,
    {
        trace!("create");

        // HACK:
        // This ends up needing a temporary list because the attachment references need to be
        // hashable for lookup, but the ash ones are not. We could transmute them or something....
        let attachments_ref = Shared::<_, P>::new(RefCell::new(vec![]));
        let attachments_ref_clone = Shared::clone(&attachments_ref);

        let device = Shared::clone(device);
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
        let subpasses = info
            .subpasses
            .iter()
            .map(move |subpass| {
                let mut desc = vk::SubpassDescription {
                    flags: vk::SubpassDescriptionFlags::empty(),
                    pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
                    input_attachment_count: subpass.input_attachments.len() as _,
                    p_input_attachments: null(),
                    color_attachment_count: subpass.color_attachments.len() as _,
                    p_color_attachments: null(),
                    p_resolve_attachments: null(),
                    p_depth_stencil_attachment: null(),
                    preserve_attachment_count: subpass.preserve_attachments.len() as _,
                    p_preserve_attachments: null(),
                };

                let mut attachments_ref = attachments_ref_clone.borrow_mut();

                if !subpass.color_attachments.is_empty() {
                    let idx = attachments_ref.len();
                    attachments_ref.extend(
                        subpass
                            .color_attachments
                            .iter()
                            .copied()
                            .map(|attachment| attachment.into_vk()),
                    );
                    desc.p_color_attachments = attachments_ref[idx..].as_ptr();
                }

                if !subpass.input_attachments.is_empty() {
                    let idx = attachments_ref.len();
                    attachments_ref.extend(
                        subpass
                            .input_attachments
                            .iter()
                            .copied()
                            .map(|attachment| attachment.into_vk()),
                    );
                    desc.p_input_attachments = attachments_ref[idx..].as_ptr();
                }

                if !subpass.resolve_attachments.is_empty() {
                    let idx = attachments_ref.len();
                    attachments_ref.extend(
                        subpass
                            .resolve_attachments
                            .iter()
                            .copied()
                            .map(|attachment| attachment.into_vk()),
                    );
                    desc.p_resolve_attachments = attachments_ref[idx..].as_ptr();
                }

                if !subpass.preserve_attachments.is_empty() {
                    desc.p_preserve_attachments = subpass.preserve_attachments.as_ptr();
                }

                if let Some(depth_stencil_attachment) = subpass.depth_stencil_attachment {
                    let idx = attachments_ref.len();
                    attachments_ref.push(depth_stencil_attachment.into_vk());
                    desc.p_depth_stencil_attachment = attachments_ref[idx..].as_ptr();
                }

                desc
            })
            .collect::<Box<[_]>>();
        let render_pass = unsafe {
            device.create_render_pass(
                &vk::RenderPassCreateInfo::builder()
                    .flags(vk::RenderPassCreateFlags::empty())
                    .attachments(&attachments)
                    .dependencies(&dependencies)
                    .subpasses(&subpasses),
                None,
            )
        }
        .map_err(|_| DriverError::InvalidData)?;

        Ok(Self {
            info,
            device,
            framebuffer_cache: Mutex::new(Default::default()),
            graphic_pipeline_cache: Mutex::new(Default::default()),
            render_pass,
        })
    }

    pub fn framebuffer_ref(&self, info: FramebufferKey) -> Result<vk::Framebuffer, DriverError> {
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
            .layers(1) // TODO!
            .push_next(&mut imageless_info);
        create_info.attachment_count = self.info.attachments.len() as _;

        let framebuffer = unsafe {
            self.device
                .create_framebuffer(&create_info, None)
                .map_err(|_| DriverError::Unsupported)?
        };

        entry.insert(framebuffer);

        Ok(framebuffer)
    }

    pub fn graphic_pipeline_ref(
        &self,
        pipeline: &Shared<GraphicPipeline<P>, P>,
        depth_stencil: Option<DepthStencilMode>,
        subpass_idx: u32,
    ) -> Result<vk::Pipeline, DriverError> {
        use std::slice::from_ref;

        let key = GraphicPipelineKey {
            depth_stencil,
            pipeline: Shared::as_ptr(pipeline) as _, // HACK: We're just storing a pointer!
        };
        let mut cache = self.graphic_pipeline_cache.lock();
        let entry = cache.entry(key);
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
            .alpha_to_coverage_enable(pipeline.state.multisample_state.alpha_to_coverage_enable)
            .alpha_to_one_enable(pipeline.state.multisample_state.alpha_to_one_enable)
            .flags(pipeline.state.multisample_state.flags)
            .min_sample_shading(pipeline.state.multisample_state.min_sample_shading)
            .rasterization_samples(
                pipeline
                    .state
                    .multisample_state
                    .rasterization_samples
                    .into_vk(),
            )
            .sample_shading_enable(pipeline.state.multisample_state.sample_shading_enable)
            .sample_mask(&pipeline.state.multisample_state.sample_mask);
        let stages = pipeline
            .state
            .stages
            .iter()
            .map(|stage| {
                vk::PipelineShaderStageCreateInfo::builder()
                    .module(stage.module)
                    .name(&stage.name)
                    .stage(stage.flags)
                    .build()
            })
            .collect::<Box<[_]>>();
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(
                &pipeline
                    .state
                    .vertex_input_state
                    .vertex_attribute_descriptions,
            )
            .vertex_binding_descriptions(
                &pipeline
                    .state
                    .vertex_input_state
                    .vertex_binding_descriptions,
            );
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            .scissor_count(1)
            .build();
        let graphic_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .color_blend_state(&color_blend_state)
            .stages(&stages)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&pipeline.state.input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&pipeline.state.rasterization_state)
            .multisample_state(&multisample_state)
            .dynamic_state(&dynamic_state)
            .layout(pipeline.state.layout)
            .render_pass(self.render_pass)
            .subpass(subpass_idx);

        let pipeline = unsafe {
            if let Some(depth_stencil) = depth_stencil {
                self.device.create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    from_ref(
                        &graphic_pipeline_info
                            .depth_stencil_state(&depth_stencil.into_vk())
                            .build(),
                    ),
                    None,
                )
            } else {
                self.device.create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    from_ref(&graphic_pipeline_info.build()),
                    None,
                )
            }
        }
        .map_err(|_| DriverError::Unsupported)?[0];

        entry.insert(pipeline);

        Ok(pipeline)
    }
}

impl<P> Deref for RenderPass<P>
where
    P: SharedPointerKind,
{
    type Target = vk::RenderPass;

    fn deref(&self) -> &Self::Target {
        &self.render_pass
    }
}

impl<P> Drop for RenderPass<P>
where
    P: SharedPointerKind,
{
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

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned")]
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
    pub fn into_vk(self) -> vk::SubpassDependency {
        vk::SubpassDependency {
            src_subpass: self.src_subpass,
            dst_subpass: self.dst_subpass,
            src_stage_mask: self.src_stage_mask,
            dst_stage_mask: self.dst_stage_mask,
            src_access_mask: self.src_access_mask,
            dst_access_mask: self.dst_access_mask,
            dependency_flags: self.dependency_flags,
        }
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
}
