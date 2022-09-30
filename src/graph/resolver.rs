use {
    super::{
        Area, Attachment, Binding, Bindings, ClearColorValue, Edge, ExecutionPipeline, Node, Pass,
        RenderGraph, Unbind,
    },
    crate::{
        driver::{
            format_aspect_mask, image_access_layout, is_read_access, is_write_access,
            pipeline_stage_access_flags, AccelerationStructure, AttachmentInfo, AttachmentRef,
            Buffer, CommandBuffer, DepthStencilMode, DescriptorBinding, DescriptorInfo,
            DescriptorPool, DescriptorPoolInfo, DescriptorSet, Device, DriverError, FramebufferKey,
            FramebufferKeyAttachment, Image, ImageViewInfo, Queue, QueueFamily, RenderPass,
            RenderPassInfo, SampleCount, SubpassDependency, SubpassInfo,
        },
        pool::{hash::HashPool, lazy::LazyPool, Lease, Pool},
    },
    ash::vk,
    log::{debug, trace},
    std::{
        cell::RefCell,
        collections::{BTreeSet, HashMap, VecDeque},
        iter::repeat,
        mem::take,
        ops::Range,
    },
    vk_sync::{cmd::pipeline_barrier, AccessType, BufferBarrier, GlobalBarrier, ImageBarrier},
};

fn align_up(val: u32, atom: u32) -> u32 {
    (val + atom - 1) & !(atom - 1)
}

#[derive(Debug)]
struct PhysicalPass {
    descriptor_pool: Option<Lease<DescriptorPool>>,
    exec_descriptor_sets: HashMap<usize, Vec<DescriptorSet>>,
    render_pass: Option<Lease<RenderPass>>,
}

impl Drop for PhysicalPass {
    fn drop(&mut self) {
        self.exec_descriptor_sets.clear();
        self.descriptor_pool = None;
    }
}

/// A structure which can read and execute render graphs. This pattern was derived from:
///
/// <http://themaister.net/blog/2017/08/15/render-graphs-and-vulkan-a-deep-dive/>
/// <https://github.com/EmbarkStudios/kajiya>
#[derive(Debug)]
pub struct Resolver {
    pub(super) graph: RenderGraph,
    physical_passes: Vec<PhysicalPass>,
}

impl Resolver {
    pub(super) fn new(graph: RenderGraph) -> Self {
        let physical_passes = Vec::with_capacity(graph.passes.len());

        Self {
            graph,
            physical_passes,
        }
    }

    fn allow_merge_passes(lhs: &Pass, rhs: &Pass) -> bool {
        let lhs_pipeline = lhs
            .execs
            .get(0)
            .map(|exec| exec.pipeline.as_ref())
            .filter(|pipeline| matches!(pipeline, Some(ExecutionPipeline::Graphic(_))))
            .flatten();
        let rhs_pipeline = rhs
            .execs
            .get(0)
            .map(|exec| exec.pipeline.as_ref())
            .filter(|pipeline| matches!(pipeline, Some(ExecutionPipeline::Graphic(_))))
            .flatten();

        // Both must have graphic pipelines
        if lhs_pipeline.is_none() || rhs_pipeline.is_none() {
            if lhs_pipeline.is_none() {
                trace!("  {} is not graphic", lhs.name);
            }

            if rhs_pipeline.is_none() {
                trace!("  {} is not graphic", rhs.name);
            }

            return false;
        }

        let lhs_pipeline = lhs_pipeline.unwrap().unwrap_graphic();
        let rhs_pipeline = rhs_pipeline.unwrap().unwrap_graphic();

        // Must be same general rasterization modes
        if lhs_pipeline.info != rhs_pipeline.info {
            trace!("  different rasterization modes",);

            return false;
        }

        let rhs_first_exec = rhs.execs.first().unwrap();

        // Now we need to know what the subpasses (we may have prior merges) wrote
        for (lhs_resolves, lhs_stores) in lhs
            .execs
            .iter()
            .rev()
            .map(|exec| (&exec.resolves, &exec.stores))
        {
            // Compare individual color/depth+stencil attachments for compatibility
            if !lhs_resolves.are_compatible(&rhs_first_exec.loads)
                || !lhs_stores.are_compatible(&rhs_first_exec.loads)
            {
                trace!("  incompatible attachments");

                return false;
            }

            // Keep color and depth on tile.
            for node_idx in rhs_first_exec.loads.images() {
                if lhs_resolves.contains_image(node_idx) || lhs_stores.contains_image(node_idx) {
                    trace!("  merging due to common image");

                    return true;
                }
            }
        }

        // Keep input on tile
        if rhs_pipeline
            .descriptor_info
            .pool_sizes
            .values()
            .filter_map(|pool_size| pool_size.get(&vk::DescriptorType::INPUT_ATTACHMENT))
            .next()
            .is_some()
        {
            trace!("  merging due to input");

            return true;
        }

        trace!("  not merging");

        // No reason to merge, so don't.
        false
    }

    // See https://vulkan.lunarg.com/doc/view/1.3.204.1/linux/1.3-extensions/vkspec.html#attachment-type-imagelayout
    fn attachment_layout(
        aspect_mask: vk::ImageAspectFlags,
        is_random_access: bool,
        is_input: bool,
    ) -> vk::ImageLayout {
        if aspect_mask.contains(vk::ImageAspectFlags::COLOR) {
            if is_input {
                vk::ImageLayout::GENERAL
            } else {
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
            }
        } else if aspect_mask.contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
        {
            if is_random_access {
                if is_input {
                    vk::ImageLayout::GENERAL
                } else {
                    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                }
            } else {
                vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
            }
        } else if aspect_mask.contains(vk::ImageAspectFlags::DEPTH) {
            if is_random_access {
                if is_input {
                    vk::ImageLayout::GENERAL
                } else {
                    vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                }
            } else {
                vk::ImageLayout::DEPTH_READ_ONLY_OPTIMAL
            }
        } else if aspect_mask.contains(vk::ImageAspectFlags::STENCIL) {
            if is_random_access {
                if is_input {
                    vk::ImageLayout::GENERAL
                } else {
                    vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                }
            } else {
                vk::ImageLayout::STENCIL_READ_ONLY_OPTIMAL
            }
        } else {
            vk::ImageLayout::UNDEFINED
        }
    }

    fn begin_render_pass(
        &mut self,
        cmd_buf: &CommandBuffer,
        pass: &Pass,
        pass_idx: usize,
        render_area: Area,
    ) -> Result<(), DriverError> {
        trace!("  begin render pass");

        let physical_pass = &self.physical_passes[pass_idx];
        let render_pass = physical_pass.render_pass.as_ref().unwrap();
        let attached_images = {
            let mut attachment_queue =
                (0..render_pass.info.attachments.len()).collect::<VecDeque<_>>();
            let mut res = Vec::with_capacity(attachment_queue.len());
            res.extend(repeat(None).take(attachment_queue.len()));
            while let Some(attachment_idx) = attachment_queue.pop_front() {
                for exec in pass.execs.iter() {
                    if let Some(attachment) = exec.color_attachment(attachment_idx as _) {
                        let image = self.graph.bindings[attachment.target]
                            .as_driver_image()
                            .unwrap();
                        let view_info = ImageViewInfo {
                            array_layer_count: Some(1),
                            aspect_mask: attachment.aspect_mask,
                            base_array_layer: 0,
                            base_mip_level: 0,
                            fmt: attachment.fmt,
                            mip_level_count: Some(1),
                            ty: image.info.ty,
                        };

                        trace!("color attachment {attachment_idx}: {image:?}");

                        res[attachment_idx] = Some((image, view_info));
                        break;
                    } else if let Some(attachment) = exec.depth_stencil_attachment() {
                        let image = self.graph.bindings[attachment.target]
                            .as_driver_image()
                            .unwrap();
                        let view_info = ImageViewInfo {
                            array_layer_count: Some(1),
                            aspect_mask: attachment.aspect_mask,
                            base_array_layer: 0,
                            base_mip_level: 0,
                            fmt: attachment.fmt,
                            mip_level_count: Some(1),
                            ty: image.info.ty,
                        };

                        trace!("depth/stencil attachment {attachment_idx}: {image:?}");

                        res[attachment_idx] = Some((image, view_info));
                        break;
                    }
                }
            }

            res.into_iter()
                .map(|image_info| image_info.unwrap())
                .collect::<Vec<_>>()
        };

        let framebuffer = render_pass.framebuffer_ref(FramebufferKey {
            attachments: attached_images
                .iter()
                .enumerate()
                .map(|(attachment_idx, (image, _))| FramebufferKeyAttachment {
                    flags: image.info.flags,
                    usage: image.info.usage,
                    extent_x: image.info.width,
                    extent_y: image.info.height,
                    layer_count: image.info.array_elements,
                    view_fmts: pass
                        .execs
                        .iter()
                        .map(|exec| {
                            exec.color_attachment(attachment_idx as _)
                                .unwrap_or_else(|| exec.depth_stencil_attachment().unwrap())
                                .fmt
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                })
                .collect(),
            extent_x: render_area.width,
            extent_y: render_area.height,
        })?;

        unsafe {
            cmd_buf.device.cmd_begin_render_pass(
                **cmd_buf,
                &vk::RenderPassBeginInfo::builder()
                    .render_pass(***render_pass)
                    .framebuffer(framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D {
                            x: render_area.x,
                            y: render_area.y,
                        },
                        extent: vk::Extent2D {
                            width: render_area.width,
                            height: render_area.height,
                        },
                    })
                    .clear_values(
                        &pass
                            .execs
                            .get(0)
                            .unwrap()
                            .color_clears
                            .values()
                            .copied()
                            .map(|(_, ClearColorValue(float32))| vk::ClearValue {
                                color: vk::ClearColorValue { float32 },
                            })
                            .chain(repeat(Default::default()))
                            .take(render_pass.info.attachments.len())
                            .collect::<Box<[_]>>(),
                    )
                    .push_next(
                        &mut vk::RenderPassAttachmentBeginInfoKHR::builder().attachments(
                            &attached_images
                                .iter()
                                .map(|(image, view_info)| Image::view_ref(image, *view_info))
                                .collect::<Result<Box<[_]>, _>>()?,
                        ),
                    ),
                vk::SubpassContents::INLINE,
            );
        }

        Ok(())
    }

    fn bind_descriptor_sets(
        &self,
        cmd_buf: &CommandBuffer,
        pipeline: &ExecutionPipeline,
        physical_pass: &PhysicalPass,
        exec_idx: usize,
    ) {
        let descriptor_sets =
            physical_pass
                .exec_descriptor_sets
                .get(&exec_idx)
                .map(|exec_descriptor_sets| {
                    exec_descriptor_sets
                        .iter()
                        .map(|descriptor_set| **descriptor_set)
                        .collect::<Box<[_]>>()
                });
        if descriptor_sets.is_none() {
            return;
        }

        let descriptor_sets = descriptor_sets.as_ref().unwrap();
        if descriptor_sets.is_empty() {
            return;
        }

        trace!("    bind descriptor sets {:?}", descriptor_sets);

        unsafe {
            cmd_buf.device.cmd_bind_descriptor_sets(
                **cmd_buf,
                pipeline.bind_point(),
                pipeline.layout(),
                0,
                descriptor_sets,
                &[],
            );
        }
    }

    fn bind_pipeline(
        &self,
        cmd_buf: &mut CommandBuffer,
        pass_idx: usize,
        exec_idx: usize,
        pipeline: &mut ExecutionPipeline,
        depth_stencil: Option<DepthStencilMode>,
    ) -> Result<(), DriverError> {
        let (ty, name, vk_pipeline) = match pipeline {
            ExecutionPipeline::Compute(pipeline) => {
                ("compute", pipeline.info.name.as_ref(), ***pipeline)
            }
            ExecutionPipeline::Graphic(pipeline) => {
                ("graphic", pipeline.info.name.as_ref(), vk::Pipeline::null())
            }
            ExecutionPipeline::RayTrace(pipeline) => {
                ("ray trace", pipeline.info.name.as_ref(), ***pipeline)
            }
        };
        if let Some(name) = name {
            trace!("    bind {} pipeline {} ({:?})", ty, name, vk_pipeline);
        } else {
            trace!("    bind {} pipeline {:?}", ty, vk_pipeline);
        }

        // We store a shared reference to this pipeline inside the command buffer!
        let physical_pass = &self.physical_passes[pass_idx];
        let pipeline_bind_point = pipeline.bind_point();
        let pipeline = match pipeline {
            ExecutionPipeline::Compute(pipeline) => ***pipeline,
            ExecutionPipeline::Graphic(pipeline) => physical_pass
                .render_pass
                .as_ref()
                .unwrap()
                .graphic_pipeline_ref(pipeline, depth_stencil, exec_idx as _)?,
            ExecutionPipeline::RayTrace(pipeline) => ***pipeline,
        };

        unsafe {
            cmd_buf
                .device
                .cmd_bind_pipeline(**cmd_buf, pipeline_bind_point, pipeline);
        }

        Ok(())
    }

    /// Finds the unique indexes of the passes which write to a given node; with the restriction
    /// to not inspect later passes. Results are returned in the opposite order the dependencies
    /// must be resolved in.
    ///
    /// Dependent upon means that the pass writes to the node.
    fn dependent_passes(
        &self,
        node_idx: usize,
        end_pass_idx: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        // TODO: We could store the nodes of a pass so we don't need to do these horrible things
        self.graph.passes.as_slice()[0..end_pass_idx]
            .iter()
            .enumerate()
            .rev()
            .filter(move |(_, pass)| {
                pass.execs
                    .iter() // <- This is the horrible part BENCHES!
                    .any(|exec| exec.accesses.contains_key(&node_idx))
            })
            .map(|(pass_idx, _)| pass_idx)
    }

    /// Finds the unique indexes of the node bindings which a given pass reads. Results are
    /// returned in the opposite order the dependencies must be resolved in.
    ///
    /// Dependent upon means that the node is read from the pass.
    fn dependent_nodes(&self, pass_idx: usize) -> impl Iterator<Item = usize> + '_ {
        let mut already_seen = BTreeSet::new();
        self.graph.passes[pass_idx]
            .execs
            .iter()
            .flat_map(|exec| exec.accesses.iter())
            .filter_map(move |(node_idx, [early, _])| {
                if is_read_access(early.access) && already_seen.insert(*node_idx) {
                    Some(*node_idx)
                } else {
                    None
                }
            })
    }

    fn end_render_pass(&mut self, cmd_buf: &CommandBuffer) {
        trace!("  end render pass");

        unsafe {
            cmd_buf.device.cmd_end_render_pass(**cmd_buf);
        }
    }

    /// Returns the unique indexes of the passes which are dependent on the given pass.
    fn interdependent_passes(
        &self,
        pass_idx: usize,
        end_pass_idx: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        let mut already_seen = BTreeSet::new();
        already_seen.insert(pass_idx);
        self.dependent_nodes(pass_idx)
            .flat_map(move |node_idx| self.dependent_passes(node_idx, end_pass_idx))
            .filter(move |pass_idx| already_seen.insert(*pass_idx))
    }

    /// Returns `true` when all recorded passes have been submitted to a driver command buffer.
    ///
    /// A fully-resolved graph contains no additional work and may be discarded, although doing so
    /// will stall the GPU while the fences are waited on. It is preferrable to wait a few frame so
    /// that the fences will have already been signalled.
    pub fn is_resolved(&self) -> bool {
        self.graph.passes.is_empty()
    }

    #[allow(clippy::type_complexity)]
    fn lease_descriptor_pool(
        cache: &mut dyn ResolverPool,
        pass: &Pass,
    ) -> Result<Option<Lease<DescriptorPool>>, DriverError> {
        let max_set_idx = pass
            .execs
            .iter()
            .flat_map(|exec| exec.bindings.keys())
            .map(|descriptor| descriptor.set())
            .max()
            .unwrap_or_default();
        let max_sets = pass.execs.len() as u32 * (max_set_idx + 1);
        let mut info = DescriptorPoolInfo {
            max_sets,
            ..Default::default()
        };

        // Find the total count of descriptors per type (there may be multiple pipelines!)
        for pool_sizes in pass.descriptor_pools_sizes() {
            for pool_size in pool_sizes.values() {
                for (descriptor_ty, descriptor_count) in pool_size {
                    debug_assert_ne!(*descriptor_count, 0);

                    match *descriptor_ty {
                        vk::DescriptorType::ACCELERATION_STRUCTURE_KHR => {
                            info.acceleration_structure_count += descriptor_count;
                        }
                        vk::DescriptorType::COMBINED_IMAGE_SAMPLER => {
                            info.combined_image_sampler_count += descriptor_count;
                        }
                        vk::DescriptorType::INPUT_ATTACHMENT => {
                            info.input_attachment_count += descriptor_count;
                        }
                        vk::DescriptorType::SAMPLED_IMAGE => {
                            info.sampled_image_count += descriptor_count;
                        }
                        vk::DescriptorType::STORAGE_BUFFER => {
                            info.storage_buffer_count += descriptor_count;
                        }
                        vk::DescriptorType::STORAGE_BUFFER_DYNAMIC => {
                            info.storage_buffer_dynamic_count += descriptor_count;
                        }
                        vk::DescriptorType::STORAGE_IMAGE => {
                            info.storage_image_count += descriptor_count;
                        }
                        vk::DescriptorType::STORAGE_TEXEL_BUFFER => {
                            info.storage_texel_buffer_count += descriptor_count;
                        }
                        vk::DescriptorType::UNIFORM_BUFFER => {
                            info.uniform_buffer_count += descriptor_count;
                        }
                        vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC => {
                            info.uniform_buffer_dynamic_count += descriptor_count;
                        }
                        vk::DescriptorType::UNIFORM_TEXEL_BUFFER => {
                            info.uniform_texel_buffer_count += descriptor_count;
                        }
                        _ => unimplemented!(),
                    };
                }
            }
        }

        // It's possible to execute a command-only pipeline
        if info.is_empty() {
            return Ok(None);
        }

        // Trivially round up the descriptor counts to increase cache coherence
        const ATOM: u32 = 1 << 5;
        info.acceleration_structure_count = align_up(info.acceleration_structure_count, ATOM);
        info.combined_image_sampler_count = align_up(info.combined_image_sampler_count, ATOM);
        info.input_attachment_count = align_up(info.input_attachment_count, ATOM);
        info.sampled_image_count = align_up(info.sampled_image_count, ATOM);
        info.storage_buffer_count = align_up(info.storage_buffer_count, ATOM);
        info.storage_buffer_dynamic_count = align_up(info.storage_buffer_dynamic_count, ATOM);
        info.storage_image_count = align_up(info.storage_image_count, ATOM);
        info.storage_texel_buffer_count = align_up(info.storage_texel_buffer_count, ATOM);
        info.uniform_buffer_count = align_up(info.uniform_buffer_count, ATOM);
        info.uniform_buffer_dynamic_count = align_up(info.uniform_buffer_dynamic_count, ATOM);
        info.uniform_texel_buffer_count = align_up(info.uniform_texel_buffer_count, ATOM);

        // Notice how all sets are big enough for any other set; TODO: efficiently dont

        // debug!("{:#?}", info);

        let pool = cache.lease(info)?;

        Ok(Some(pool))
    }

    fn lease_render_pass(
        &self,
        cache: &mut dyn ResolverPool,
        pass_idx: usize,
    ) -> Result<Lease<RenderPass>, DriverError> {
        // TODO: We're building a RenderPassInfo here (the 3x Vec<_>s), but we could use TLS if:
        // - leasing used impl Into instead of an instance
        // - RenderPass didn't require an Info instance: who cares it's OURS for like five seconds
        //   and then poof
        let pass = &self.graph.passes[pass_idx];
        let attachment_count = pass
            .execs
            .iter()
            .map(|exec| exec.color_attachment_count())
            .max()
            .unwrap_or_default()
            + pass
                .execs
                .iter()
                .any(|exec| exec.has_depth_stencil_attachment()) as usize;
        let mut attachments = Vec::with_capacity(attachment_count);
        let mut subpasses = Vec::<SubpassInfo>::with_capacity(pass.execs.len());

        while attachments.len() < attachment_count {
            attachments.push(AttachmentInfo::new(vk::Format::UNDEFINED, SampleCount::X1).build());
        }

        // Add attachments: format, sample count, load ops, and initial layout (using the first
        // execution)
        {
            let first_exec = &pass.execs[0];

            // Cleared color attachments
            for attachment_idx in first_exec.color_clears.keys().copied() {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                attachment.load_op = vk::AttachmentLoadOp::CLEAR;
            }

            // Cleared depth/stencil attachment
            if first_exec.depth_stencil_clear.is_some() {
                // Note: Layout will be set if (..when..) we're resolved or stored
                // We don't set depth/stencil initial layout here because we don't
                // know the view aspect flags yet - we let the store or resolve op
                // set the initial layout
                let attachment = attachments.last_mut().unwrap();
                attachment.load_op = vk::AttachmentLoadOp::CLEAR;
                attachment.stencil_load_op = vk::AttachmentLoadOp::CLEAR;
            }

            // Loaded color attachments
            for (attachment_idx, loaded_attachment) in first_exec.loads.colors() {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.fmt = loaded_attachment.fmt;
                attachment.sample_count = loaded_attachment.sample_count;
                attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                attachment.load_op = vk::AttachmentLoadOp::LOAD;
            }

            // Loaded depth/stencil attachment
            if let Some(loaded_attachment) = first_exec.loads.depth_stencil() {
                let is_random_access = first_exec.stores.depth_stencil().is_some()
                    || first_exec.resolves.depth_stencil().is_some();
                let attachment = attachments.last_mut().unwrap();
                attachment.fmt = loaded_attachment.fmt;
                attachment.sample_count = loaded_attachment.sample_count;
                attachment.initial_layout = if loaded_attachment
                    .aspect_mask
                    .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                {
                    attachment.load_op = vk::AttachmentLoadOp::LOAD;
                    attachment.stencil_load_op = vk::AttachmentLoadOp::LOAD;

                    if is_random_access {
                        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                    } else {
                        vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
                    }
                } else if loaded_attachment
                    .aspect_mask
                    .contains(vk::ImageAspectFlags::DEPTH)
                {
                    attachment.load_op = vk::AttachmentLoadOp::LOAD;

                    if is_random_access {
                        vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                    } else {
                        vk::ImageLayout::DEPTH_READ_ONLY_OPTIMAL
                    }
                } else if is_random_access {
                    attachment.stencil_load_op = vk::AttachmentLoadOp::LOAD;

                    vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                } else {
                    attachment.stencil_load_op = vk::AttachmentLoadOp::LOAD;

                    vk::ImageLayout::STENCIL_READ_ONLY_OPTIMAL
                };
            }

            // Resolved color attachments
            for (attachment_idx, resolved_attachment) in first_exec.resolves.colors() {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.fmt = resolved_attachment.fmt;
                attachment.sample_count = resolved_attachment.sample_count;

                // Set layout here bc we did not set it above, if we handled a clear op
                attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
            }

            // Resolved depth/stencil attachment
            if let Some(resolved_attachment) = first_exec.resolves.depth_stencil() {
                let attachment = attachments.last_mut().unwrap();
                attachment.fmt = resolved_attachment.fmt;
                attachment.sample_count = resolved_attachment.sample_count;

                // Set layout here bc we did not set it above, if we handled a clear op
                // We only set this if a load didn't set it
                if attachment.initial_layout == vk::ImageLayout::UNDEFINED {
                    attachment.initial_layout = if resolved_attachment
                        .aspect_mask
                        .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                    {
                        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                    } else if resolved_attachment
                        .aspect_mask
                        .contains(vk::ImageAspectFlags::DEPTH)
                    {
                        vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                    } else {
                        vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                    };
                }
            }

            // Stored color attachments
            for (attachment_idx, stored_attachment) in first_exec.stores.colors() {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.fmt = stored_attachment.fmt;
                attachment.sample_count = stored_attachment.sample_count;

                // Set layout here bc we did not set it above, if we handled a clear op
                attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
            }

            // Stored depth/stencil attachment
            if let Some(stored_attachment) = first_exec.stores.depth_stencil() {
                let attachment = attachments.last_mut().unwrap();
                attachment.fmt = stored_attachment.fmt;
                attachment.sample_count = stored_attachment.sample_count;

                // Set layout here bc we did not set it above, if we handled a clear op
                // We only set this if a load didn't set it
                if attachment.initial_layout == vk::ImageLayout::UNDEFINED {
                    attachment.initial_layout = if stored_attachment
                        .aspect_mask
                        .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                    {
                        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                    } else if stored_attachment
                        .aspect_mask
                        .contains(vk::ImageAspectFlags::DEPTH)
                    {
                        vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                    } else {
                        vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                    };
                }
            }
        }

        // Add attachments: store ops and final layout (using the last pass)
        {
            let last_exec = pass.execs.last().unwrap();

            // Resolved color attachments
            for (attachment_idx, _) in last_exec.resolves.colors() {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
            }

            // Resolved depth/stencil attachment
            if let Some(resolved_attachment) = last_exec.resolves.depth_stencil() {
                let attachment = attachments.last_mut().unwrap();
                attachment.final_layout = if resolved_attachment
                    .aspect_mask
                    .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                {
                    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                } else if resolved_attachment
                    .aspect_mask
                    .contains(vk::ImageAspectFlags::DEPTH)
                {
                    vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                } else {
                    vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                };
            }

            // Stored color attachments
            for (attachment_idx, _) in last_exec.stores.colors() {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.store_op = vk::AttachmentStoreOp::STORE;

                // Set layout here bc we did not set it above, if we handled a clear op
                attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
            }

            // Stored depth/stencil attachment
            if let Some(stored_attachment) = last_exec.stores.depth_stencil() {
                let attachment = attachments.last_mut().unwrap();
                attachment.stencil_store_op = vk::AttachmentStoreOp::STORE;

                // Set layout here bc we did not set it above, if we handled a clear op
                attachment.final_layout = if stored_attachment
                    .aspect_mask
                    .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                {
                    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                } else if stored_attachment
                    .aspect_mask
                    .contains(vk::ImageAspectFlags::DEPTH)
                {
                    vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                } else {
                    vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                };
            }
        }

        // Add subpasses
        for (exec_idx, exec) in pass.execs.iter().enumerate() {
            let pipeline = exec
                .pipeline
                .as_ref()
                .map(|pipeline| pipeline.unwrap_graphic())
                .unwrap();
            let mut subpass_info = SubpassInfo::with_capacity(attachment_count);

            // TODO: TLS a sorted vec so we don't need to iter.find the input attachments later!
            // Add input attachments
            for (_, (descriptor_info, _)) in pipeline.descriptor_bindings.iter() {
                if let &DescriptorInfo::InputAttachment(_, attachment_idx) = descriptor_info {
                    debug_assert!(
                        !exec.color_clears.contains_key(&attachment_idx),
                        "cannot clear color attachment index {attachment_idx} because it uses subpass input"
                    );

                    let exec_attachment = exec
                        .color_attachment(attachment_idx)
                        .expect("subpass input attachment index not loaded, resolved, or stored");
                    let is_random_access = exec.resolves.contains_color(attachment_idx)
                        || exec.stores.contains_color(attachment_idx);
                    subpass_info.input_attachments.push(AttachmentRef {
                        attachment: attachment_idx,
                        aspect_mask: exec_attachment.aspect_mask,
                        layout: Self::attachment_layout(
                            exec_attachment.aspect_mask,
                            is_random_access,
                            true,
                        ),
                    });

                    // We should preserve the attachment in the previous subpasses as needed
                    // (We're asserting that any input renderpasses are actually real subpasses
                    // here with prior passes..)
                    for prev_exec_idx in (0..exec_idx - 1).rev() {
                        let prev_exec = &pass.execs[prev_exec_idx];
                        if prev_exec.resolves.contains_color(attachment_idx)
                            || prev_exec.stores.contains_color(attachment_idx)
                        {
                            break;
                        }

                        let prev_subpass = &mut subpasses[prev_exec_idx];
                        prev_subpass.preserve_attachments.push(attachment_idx);
                    }
                }
            }

            // Color attachments
            for attachment_idx in 0..exec.color_attachment_count() as _ {
                let is_input = subpass_info
                    .input_attachments
                    .iter()
                    .any(|input| input.attachment == attachment_idx);
                subpass_info.color_attachments.push(AttachmentRef {
                    attachment: attachment_idx,
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    layout: Self::attachment_layout(vk::ImageAspectFlags::COLOR, true, is_input),
                });
            }

            // Set depth/stencil attachment
            let depth_stencil = exec.depth_stencil_attachment();
            if let Some(depth_stencil) = depth_stencil {
                let is_random_access = exec.stores.depth_stencil().is_some()
                    || exec.resolves.depth_stencil().is_some();
                subpass_info.depth_stencil_attachment = Some(AttachmentRef {
                    attachment: exec.color_attachment_count() as _,
                    aspect_mask: depth_stencil.aspect_mask,
                    layout: Self::attachment_layout(
                        depth_stencil.aspect_mask,
                        is_random_access,
                        false,
                    ),
                });
            }

            // Set resolves to defaults
            subpass_info.resolve_attachments.extend(
                repeat(AttachmentRef {
                    attachment: vk::ATTACHMENT_UNUSED,
                    aspect_mask: vk::ImageAspectFlags::empty(),
                    layout: vk::ImageLayout::UNDEFINED,
                })
                .take(subpass_info.color_attachments.len() + depth_stencil.is_some() as usize),
            );

            // Set any used resolve attachments now
            for (attachment, resolve) in exec.resolves.colors() {
                let is_input = subpass_info
                    .input_attachments
                    .iter()
                    .any(|input| input.attachment == attachment);
                subpass_info.resolve_attachments[attachment as usize] = AttachmentRef {
                    attachment,
                    aspect_mask: resolve.aspect_mask,
                    layout: Self::attachment_layout(resolve.aspect_mask, true, is_input),
                };
            }

            subpasses.push(subpass_info);
        }

        // Add dependencies
        let dependencies =
            {
                let mut dependencies = HashMap::with_capacity(attachment_count);
                for (exec_idx, exec) in pass.execs.iter().enumerate() {
                    // Check accesses
                    'accesses: for (node_idx, [early, _]) in exec.accesses.iter() {
                        let (mut curr_stages, mut curr_access) =
                            pipeline_stage_access_flags(early.access);

                        // First look for through earlier executions of this pass (in reverse order)
                        for (prev_exec_idx, prev_exec) in
                            pass.execs[0..exec_idx].iter().enumerate().rev()
                        {
                            if let Some([_, late]) = prev_exec.accesses.get(node_idx) {
                                // Is this previous execution access dependent on anything the current
                                // execution access is dependent upon?
                                let (prev_stages, prev_access) =
                                    pipeline_stage_access_flags(late.access);

                                // This happens if you specfiy too broard of a read/write access in
                                // a secondary pass. Maybe that should not be possible. For now just
                                // specify the actual stages used with AccessType::Fragement*, etc.
                                // Optionally we could detect this and break the pass up - but no...
                                debug_assert!(
                                    !curr_stages.contains(vk::PipelineStageFlags::ALL_COMMANDS)
                                        && !prev_stages
                                            .contains(vk::PipelineStageFlags::ALL_COMMANDS),
                                    "exec {prev_exec_idx} {:?} -> {exec_idx} {:?}",
                                    late.access,
                                    early.access
                                );

                                let common_stages = curr_stages & prev_stages;
                                if common_stages.is_empty() {
                                    // No common dependencies
                                    continue;
                                }

                                let dep = dependencies
                                    .entry((prev_exec_idx, exec_idx))
                                    .or_insert_with(|| {
                                        SubpassDependency::new(prev_exec_idx as _, exec_idx as _)
                                    });

                                // Wait for ...
                                dep.src_stage_mask |= common_stages;
                                dep.src_access_mask |= prev_access;

                                // ... before we:
                                dep.dst_stage_mask |= curr_stages;
                                dep.dst_access_mask |= curr_access;

                                // Do the source and destination stage masks both include
                                // framebuffer-space stages?
                                if (prev_stages | curr_stages).intersects(
                                    vk::PipelineStageFlags::FRAGMENT_SHADER
                                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                                        | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                ) {
                                    dep.dependency_flags |= vk::DependencyFlags::BY_REGION;
                                }

                                // Does the execution have more than one view?
                                if subpasses[exec_idx].has_multiple_attachments() {
                                    dep.dependency_flags |= vk::DependencyFlags::VIEW_LOCAL;
                                }

                                curr_stages &= !common_stages;
                                curr_access &= !prev_access;

                                // Have we found all dependencies for this stage? If so no need to
                                // check external passes
                                if curr_stages.is_empty() {
                                    continue 'accesses;
                                }
                            }
                        }

                        // Second look in previous passes of the entire render graph
                        for prev_subpass in self
                            .dependent_passes(*node_idx, pass_idx)
                            .flat_map(|pass_idx| self.graph.passes[pass_idx].execs.iter().rev())
                        {
                            if let Some([_, late]) = prev_subpass.accesses.get(node_idx) {
                                // Is this previous subpass access dependent on anything the current
                                // subpass access is dependent upon?
                                let (prev_stages, prev_access) =
                                    pipeline_stage_access_flags(late.access);
                                let common_stages = curr_stages & prev_stages;
                                if common_stages.is_empty() {
                                    // No common dependencies
                                    continue;
                                }

                                let dep = dependencies
                                    .entry((vk::SUBPASS_EXTERNAL as _, exec_idx))
                                    .or_insert_with(|| {
                                        SubpassDependency::new(
                                            vk::SUBPASS_EXTERNAL as _,
                                            exec_idx as _,
                                        )
                                    });

                                // Wait for ...
                                dep.src_stage_mask |= common_stages;
                                dep.src_access_mask |= prev_access;

                                // ... before we:
                                dep.dst_stage_mask |=
                                    curr_stages.min(vk::PipelineStageFlags::ALL_GRAPHICS);
                                dep.dst_access_mask |= curr_access;

                                // If the source and destination stage masks both include
                                // framebuffer-space stages then we need the BY_REGION flag
                                if (prev_stages | curr_stages).intersects(
                                    vk::PipelineStageFlags::FRAGMENT_SHADER
                                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                                        | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                ) {
                                    dep.dependency_flags |= vk::DependencyFlags::BY_REGION;
                                }

                                // If the subpass has more than one view we need the VIEW_LOCAL flag
                                if subpasses[exec_idx].has_multiple_attachments() {
                                    dep.dependency_flags |= vk::DependencyFlags::VIEW_LOCAL;
                                }

                                curr_stages &= !common_stages;
                                curr_access &= !prev_access;

                                // If we found all dependencies for this stage there is no need to check
                                // external passes
                                if curr_stages.is_empty() {
                                    continue 'accesses;
                                }
                            }
                        }

                        // Fall back to external dependencies
                        if !curr_stages.is_empty() {
                            let dep = dependencies
                                .entry((vk::SUBPASS_EXTERNAL as _, exec_idx))
                                .or_insert_with(|| {
                                    SubpassDependency::new(vk::SUBPASS_EXTERNAL as _, exec_idx as _)
                                });

                            // Wait for ...
                            dep.src_stage_mask |= curr_stages;
                            dep.src_access_mask |= curr_access;

                            // ... before we:
                            dep.dst_stage_mask |= vk::PipelineStageFlags::TOP_OF_PIPE;
                            dep.dst_access_mask = vk::AccessFlags::empty();
                        }
                    }

                    // Look for attachments of this exec being read or written in other execs of the
                    // same pass
                    for (other_idx, other) in pass.execs[0..exec_idx].iter().enumerate() {
                        // Look for color attachments we're reading
                        for (attachment_idx, _) in exec.loads.colors() {
                            // Look for writes in the other exec
                            if other.color_clears.contains_key(&attachment_idx)
                                || other.stores.contains_color(attachment_idx)
                                || other.resolves.contains_color(attachment_idx)
                            {
                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |=
                                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
                                dep.src_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_WRITE;

                                // ... before we:
                                dep.dst_stage_mask |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS;
                                dep.dst_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_READ;
                            }

                            // look for reads in the other exec
                            if other.loads.contains_color(attachment_idx) {
                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |= vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
                                dep.src_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_READ;

                                // ... before we:
                                dep.dst_stage_mask |= vk::PipelineStageFlags::FRAGMENT_SHADER;
                                dep.dst_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_READ;
                            }
                        }

                        // Look for a depth/stencil attachment read
                        if exec.loads.depth_stencil().is_some() {
                            // Look for writes in the other exec
                            if other.depth_stencil_clear.is_some()
                                || other.stores.depth_stencil().is_some()
                                || other.resolves.depth_stencil().is_some()
                            {
                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |=
                                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
                                dep.src_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_WRITE;

                                // ... before we:
                                dep.dst_stage_mask |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS;
                                dep.dst_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_READ;
                            }

                            // look for reads in the other exec
                            if other.loads.depth_stencil().is_some() {
                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |= vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
                                dep.src_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_READ;

                                // ... before we:
                                dep.dst_stage_mask |= vk::PipelineStageFlags::FRAGMENT_SHADER;
                                dep.dst_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_READ;
                            }
                        }

                        // Look for color attachments we're writing
                        for attachment_idx in exec
                            .color_clears
                            .keys()
                            .copied()
                            .chain(
                                exec.resolves
                                    .colors()
                                    .map(|(attachment_idx, _)| attachment_idx),
                            )
                            .chain(
                                exec.stores
                                    .colors()
                                    .map(|(attachment_idx, _)| attachment_idx),
                            )
                        {
                            // Attachments will always be loaded or resolved/stored if they are cleared
                            let Attachment { aspect_mask, .. } =
                                exec.color_attachment(attachment_idx).unwrap();
                            let stage = match aspect_mask {
                                mask if mask.contains(vk::ImageAspectFlags::COLOR) => {
                                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                                }
                                mask if mask.intersects(
                                    vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                ) =>
                                {
                                    vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                                }
                                _ => vk::PipelineStageFlags::ALL_GRAPHICS,
                            };

                            // Look for writes in the other exec
                            if other.color_clears.contains_key(&attachment_idx)
                                || other.stores.contains_color(attachment_idx)
                                || other.resolves.contains_color(attachment_idx)
                            {
                                let access = match aspect_mask {
                                    mask if mask.contains(vk::ImageAspectFlags::COLOR) => {
                                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                                    }
                                    mask if mask.intersects(
                                        vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                    ) =>
                                    {
                                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                                    }
                                    _ => {
                                        vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE
                                    }
                                };

                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |= stage;
                                dep.src_access_mask |= access;

                                // ... before we:
                                dep.dst_stage_mask |= stage;
                                dep.dst_access_mask |= access;
                            }

                            // look for reads in the other exec
                            if other.loads.contains_color(attachment_idx) {
                                let (src_access, dst_access) = match aspect_mask {
                                    mask if mask.contains(vk::ImageAspectFlags::COLOR) => (
                                        vk::AccessFlags::COLOR_ATTACHMENT_READ,
                                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                                    ),
                                    mask if mask.intersects(
                                        vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                    ) =>
                                    {
                                        (
                                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
                                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                                        )
                                    }
                                    _ => (
                                        vk::AccessFlags::MEMORY_READ
                                            | vk::AccessFlags::MEMORY_WRITE,
                                        vk::AccessFlags::MEMORY_READ
                                            | vk::AccessFlags::MEMORY_WRITE,
                                    ),
                                };

                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS;
                                dep.src_access_mask |= src_access;

                                // ... before we:
                                dep.dst_stage_mask |= stage;
                                dep.dst_access_mask |= dst_access;
                            }
                        }

                        // Look for a depth/stencil attachment write
                        if exec.depth_stencil_clear.is_some()
                            || exec.resolves.depth_stencil().is_some()
                            || exec.stores.depth_stencil().is_some()
                        {
                            // Attachments will always be loaded or resolved/stored if they are cleared
                            let Attachment { aspect_mask, .. } =
                                exec.depth_stencil_attachment().unwrap();
                            let stage = match aspect_mask {
                                mask if mask.contains(vk::ImageAspectFlags::COLOR) => {
                                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                                }
                                mask if mask.intersects(
                                    vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                ) =>
                                {
                                    vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                                }
                                _ => vk::PipelineStageFlags::ALL_GRAPHICS,
                            };

                            // Look for writes in the other exec
                            if other.depth_stencil_clear.is_some()
                                || other.stores.depth_stencil().is_some()
                                || other.resolves.depth_stencil().is_some()
                            {
                                let access = match aspect_mask {
                                    mask if mask.contains(vk::ImageAspectFlags::COLOR) => {
                                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                                    }
                                    mask if mask.intersects(
                                        vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                    ) =>
                                    {
                                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                                    }
                                    _ => {
                                        vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE
                                    }
                                };

                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |= stage;
                                dep.src_access_mask |= access;

                                // ... before we:
                                dep.dst_stage_mask |= stage;
                                dep.dst_access_mask |= access;
                            }

                            // look for reads in the other exec
                            if other.loads.depth_stencil().is_some() {
                                let (src_access, dst_access) = match aspect_mask {
                                    mask if mask.contains(vk::ImageAspectFlags::COLOR) => (
                                        vk::AccessFlags::COLOR_ATTACHMENT_READ,
                                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                                    ),
                                    mask if mask.intersects(
                                        vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                    ) =>
                                    {
                                        (
                                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
                                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                                        )
                                    }
                                    _ => (
                                        vk::AccessFlags::MEMORY_READ
                                            | vk::AccessFlags::MEMORY_WRITE,
                                        vk::AccessFlags::MEMORY_READ
                                            | vk::AccessFlags::MEMORY_WRITE,
                                    ),
                                };

                                let dep = dependencies.entry((other_idx, exec_idx)).or_insert_with(
                                    || SubpassDependency::new(other_idx as _, exec_idx as _),
                                );

                                // Wait for ...
                                dep.src_stage_mask |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS;
                                dep.src_access_mask |= src_access;

                                // ... before we:
                                dep.dst_stage_mask |= stage;
                                dep.dst_access_mask |= dst_access;
                            }
                        }
                    }
                }

                dependencies.into_values().collect::<Vec<_>>()
            };

        cache.lease(RenderPassInfo {
            attachments,
            dependencies,
            subpasses,
        })
    }

    fn lease_scheduled_resources(
        &mut self,
        cache: &mut dyn ResolverPool,
        schedule: &[usize],
    ) -> Result<(), DriverError> {
        for pass_idx in schedule.iter().copied() {
            // At the time this function runs the pass will already have been optimized into a
            // larger pass made out of anything that might have been merged into it - so we
            // only care about one pass at a time here
            let pass = &mut self.graph.passes[pass_idx];

            trace!("leasing [{pass_idx}: {}]", pass.name);

            let descriptor_pool = Self::lease_descriptor_pool(cache, pass)?;
            let mut exec_descriptor_sets = HashMap::with_capacity(
                descriptor_pool
                    .as_ref()
                    .map(|descriptor_pool| descriptor_pool.info.max_sets as usize)
                    .unwrap_or_default(),
            );
            if let Some(descriptor_pool) = descriptor_pool.as_ref() {
                for (exec_idx, pipeline) in
                    pass.execs
                        .iter()
                        .enumerate()
                        .filter_map(|(exec_idx, exec)| {
                            exec.pipeline.as_ref().map(|pipeline| (exec_idx, pipeline))
                        })
                {
                    let layouts = pipeline.descriptor_info().layouts.values();
                    let mut descriptor_sets = Vec::with_capacity(layouts.len());
                    for descriptor_set_layout in layouts {
                        descriptor_sets.push(DescriptorPool::allocate_descriptor_set(
                            descriptor_pool,
                            descriptor_set_layout,
                        )?);
                    }
                    exec_descriptor_sets.insert(exec_idx, descriptor_sets);
                }
            }

            // Note that as a side effect of merging compatible passes all input passes should
            // be globbed onto their preceeding passes by now. This allows subpasses to use
            // input attachments without really doing anything, so we are provided a pass that
            // starts with input we just blow up b/c we can't provide it, or at least shouldn't.
            assert!(!pass.execs.is_empty());
            assert!(
                pass.execs[0].pipeline.is_none()
                    || !pass.execs[0].pipeline.as_ref().unwrap().is_graphic()
                    || pass.execs[0]
                        .pipeline
                        .as_ref()
                        .unwrap()
                        .unwrap_graphic()
                        .descriptor_info
                        .pool_sizes
                        .values()
                        .filter_map(|pool| pool.get(&vk::DescriptorType::INPUT_ATTACHMENT))
                        .next()
                        .is_none()
            );

            // Also the renderpass may just be None if the pass contained no graphic ops.
            let render_pass = if pass.execs[0]
                .pipeline
                .as_ref()
                .map(|pipeline| pipeline.is_graphic())
                .unwrap_or_default()
            {
                Some(self.lease_render_pass(cache, pass_idx)?)
            } else {
                None
            };

            self.physical_passes.push(PhysicalPass {
                descriptor_pool,
                exec_descriptor_sets,
                render_pass,
            });
        }

        Ok(())
    }

    // Merges passes which are graphic with common-ish attachments - note that scheduled pass order
    // is final during this function and so we must merge contiguous groups of passes
    fn merge_scheduled_passes<'s>(&mut self, mut schedule: &'s mut [usize]) -> &'s mut [usize] {
        let mut passes = self.graph.passes.drain(..).map(Some).collect::<Vec<_>>();
        let mut idx = 0;

        // debug!("attempting to merge {} passes", schedule.len(),);

        while idx < schedule.len() {
            let mut pass = passes[schedule[idx]].take().unwrap();

            // Find candidates
            let start = idx + 1;
            let mut end = start;
            while end < schedule.len() {
                let other = passes[schedule[end]].as_ref().unwrap();
                debug!(
                    "attempting to merge [{idx}: {}] with [{end}: {}]",
                    pass.name, other.name
                );
                if Self::allow_merge_passes(&pass, other) {
                    end += 1;
                } else {
                    break;
                }
            }

            if start != end {
                trace!("merging {} passes into [{idx}: {}]", end - start, pass.name);
            }

            // Grow the merged pass once, not per merge
            {
                let mut name_additional = 0;
                let mut execs_additional = 0;
                for idx in start..end {
                    let other = passes[schedule[idx]].as_ref().unwrap();
                    name_additional += other.name.len();
                    execs_additional += other.execs.len();
                }

                pass.name.reserve(name_additional);
                pass.execs.reserve(execs_additional);
            }

            for idx in start..end {
                let mut other = passes[schedule[idx]].take().unwrap();
                pass.name.push_str(" + ");
                pass.name.push_str(other.name.as_str());
                pass.execs.append(&mut other.execs);
            }

            self.graph.passes.push(pass);
            idx += 1 + end - start;
        }

        // Reschedule passes
        schedule = &mut schedule[0..self.graph.passes.len()];
        for (idx, pass_idx) in schedule.iter_mut().enumerate() {
            *pass_idx = idx;
        }

        // Add the remaining passes back into the graph for later
        for pass in passes.into_iter().flatten() {
            self.graph.passes.push(pass);
        }

        schedule
    }

    fn next_subpass(cmd_buf: &CommandBuffer) {
        trace!("next_subpass");

        unsafe {
            cmd_buf
                .device
                .cmd_next_subpass(**cmd_buf, vk::SubpassContents::INLINE);
        }
    }

    /// Returns the stages that process the given node.
    ///
    /// Note that this value must be retrieved before resolving a node as there will be no
    /// data left to inspect afterwards!
    pub fn node_pipeline_stages(&self, node: impl Node) -> vk::PipelineStageFlags {
        let node_idx = node.index();
        let mut res = Default::default();

        'pass: for pass in self.graph.passes.iter() {
            for exec in pass.execs.iter() {
                if exec.accesses.contains_key(&node_idx) {
                    res |= pass
                        .execs
                        .iter()
                        .filter_map(|exec| exec.pipeline.as_ref())
                        .map(|pipeline| pipeline.stage())
                        .reduce(|j, k| j | k)
                        .unwrap_or(vk::PipelineStageFlags::TRANSFER);

                    // The execution pipelines of a pass are always the same type
                    continue 'pass;
                }
            }
        }

        assert_ne!(
            res,
            Default::default(),
            "The given node was not accessed in this graph"
        );

        res
    }

    fn record_execution_barriers(
        trace_pad: &'static str,
        cmd_buf: &CommandBuffer,
        bindings: &mut [Binding],
        pass: &mut Pass,
        exec_idx: usize,
    ) {
        use std::slice::from_ref;

        // TODO: Notice the very common case where we have previously barriered on something which
        // has not had any access since the previous barrier

        // We store a Barriers in TLS to save an alloc; contents are POD
        thread_local! {
            static BARRIERS: RefCell<Barriers> = Default::default();
        }

        struct Barrier<T> {
            next_access: AccessType,
            prev_access: AccessType,
            resource: T,
        }

        #[derive(Default)]
        struct Barriers {
            buffers: Vec<Barrier<BufferResource>>,
            images: Vec<Barrier<ImageResource>>,
            next_accesses: Vec<AccessType>,
            prev_accesses: Vec<AccessType>,
        }

        struct BufferResource {
            buffer: vk::Buffer,
            offset: usize,
            size: usize,
        }

        struct ImageResource {
            image: vk::Image,
            range: vk::ImageSubresourceRange,
        }

        enum Resource {
            Buffer(BufferResource),
            Image(ImageResource),
        }

        BARRIERS.with(|barriers| {
            // Initialize TLS from a previous call
            let mut barriers = barriers.borrow_mut();
            barriers.buffers.clear();
            barriers.images.clear();
            barriers.next_accesses.clear();
            barriers.prev_accesses.clear();

            // Map remaining accesses into vk_sync barriers (some accesses may have been removed by the
            // render pass leasing function)
            let barriers = pass.execs[exec_idx]
                .accesses
                .iter()
                .map(|(node_idx, [early, late])| {
                    let binding = &mut bindings[*node_idx];
                    let next_access = early.access;
                    let prev_access = if let Some(buffer) = binding.as_driver_buffer() {
                        Buffer::access(buffer, late.access)
                    } else if let Some(image) = binding.as_driver_image() {
                        Image::access(image, late.access)
                    } else if let Some(accel_struct) = binding.as_driver_acceleration_structure() {
                        AccelerationStructure::access(accel_struct, late.access)
                    } else {
                        unimplemented!();
                    };

                    // If we find a subresource then it must have a resource attached
                    if let Some(subresource) = early.subresource {
                        if let Some(buf) = binding.as_driver_buffer() {
                            let range = subresource.unwrap_buffer();

                            trace!(
                                "{trace_pad}buffer {:?} {}..{} {:?} -> {:?}",
                                binding.as_driver_buffer().unwrap(),
                                range.start,
                                range.end,
                                next_access,
                                prev_access,
                            );

                            return Barrier {
                                next_access,
                                prev_access,
                                resource: Some(Resource::Buffer(BufferResource {
                                    buffer: **buf,
                                    offset: range.start as _,
                                    size: (range.end - range.start) as _,
                                })),
                            };
                        } else if let Some(image) = binding.as_driver_image() {
                            let range = subresource.unwrap_image().into_vk();

                            trace!(
                                "{trace_pad}image {:?} {:?}-{:?} -> {:?}-{:?}",
                                binding.as_driver_image().unwrap(),
                                prev_access,
                                image_access_layout(prev_access),
                                next_access,
                                image_access_layout(next_access),
                            );

                            return Barrier {
                                next_access,
                                prev_access,
                                resource: Some(Resource::Image(ImageResource {
                                    image: **image,
                                    range,
                                })),
                            };
                        }
                    }

                    Barrier {
                        next_access,
                        prev_access,
                        resource: None,
                    }
                })
                .fold(barriers, |mut barriers, barrier| {
                    let Barrier {
                        next_access,
                        prev_access,
                        resource,
                    } = barrier;
                    match resource {
                        Some(Resource::Buffer(resource)) => {
                            barriers.buffers.push(Barrier {
                                next_access,
                                prev_access,
                                resource,
                            });
                        }
                        Some(Resource::Image(resource)) => {
                            barriers.images.push(Barrier {
                                next_access,
                                prev_access,
                                resource,
                            });
                        }
                        None => {
                            // HACK: It would be nice if AccessType was PartialOrd..
                            if !barriers.next_accesses.contains(&next_access) {
                                barriers.next_accesses.push(next_access);
                            }

                            if !barriers.prev_accesses.contains(&prev_access) {
                                barriers.prev_accesses.push(prev_access);
                            }
                        }
                    }
                    barriers
                });
            let global_barrier = if !barriers.next_accesses.is_empty() {
                // No resource attached - we use a global barrier for these
                trace!(
                    "{trace_pad}barrier {:?} -> {:?}",
                    barriers.next_accesses,
                    barriers.prev_accesses
                );

                Some(GlobalBarrier {
                    next_accesses: barriers.next_accesses.as_slice(),
                    previous_accesses: barriers.prev_accesses.as_slice(),
                })
            } else {
                None
            };
            let buffer_barriers = barriers.buffers.iter().map(
                |Barrier {
                     next_access,
                     prev_access,
                     resource,
                 }| {
                    let BufferResource {
                        buffer,
                        offset,
                        size,
                    } = *resource;
                    BufferBarrier {
                        next_accesses: from_ref(next_access),
                        previous_accesses: from_ref(prev_access),
                        src_queue_family_index: cmd_buf.device.queue.family.idx,
                        dst_queue_family_index: cmd_buf.device.queue.family.idx,
                        buffer,
                        offset,
                        size,
                    }
                },
            );
            let image_barriers = barriers.images.iter().map(
                |Barrier {
                     next_access,
                     prev_access,
                     resource,
                 }| {
                    let ImageResource { image, range } = *resource;
                    ImageBarrier {
                        next_accesses: from_ref(next_access),
                        next_layout: image_access_layout(*next_access),
                        previous_accesses: from_ref(prev_access),
                        previous_layout: image_access_layout(*prev_access),
                        discard_contents: *prev_access == AccessType::Nothing
                            || is_write_access(*next_access),
                        src_queue_family_index: cmd_buf.device.queue.family.idx,
                        dst_queue_family_index: cmd_buf.device.queue.family.idx,
                        image,
                        range,
                    }
                },
            );

            pipeline_barrier(
                &cmd_buf.device,
                **cmd_buf,
                global_barrier,
                &buffer_barriers.collect::<Box<[_]>>(),
                &image_barriers.collect::<Box<[_]>>(),
            );
        });
    }

    /// Records any pending render graph passes that are required by the given node, but does not
    /// record any passes that actually contain the given node.
    ///
    /// As a side effect, the graph is optimized for the given node. Future calls may further optimize
    /// the graph, but only on top of the existing optimizations. This only matters if you are pulling
    /// multiple images out and you care - in that case pull the "most important" image first.
    pub fn record_node_dependencies(
        &mut self,
        cache: &mut dyn ResolverPool,
        cmd_buf: &mut CommandBuffer,
        node: impl Node,
    ) -> Result<(), DriverError> {
        let node_idx = node.index();

        assert!(self.graph.bindings.get(node_idx).is_some());

        // We record up to but not including the first pass which accesses the target node
        let end_pass_idx = self
            .graph
            .first_node_access_pass_index(node)
            .unwrap_or_default()
            .min(self.graph.passes.len());
        self.record_node_passes(cache, cmd_buf, node_idx, end_pass_idx)?;

        Ok(())
    }

    /// Records any pending render graph passes that the given node requires.
    pub fn record_node(
        &mut self,
        cache: &mut dyn ResolverPool,
        cmd_buf: &mut CommandBuffer,
        node: impl Node,
    ) -> Result<(), DriverError> {
        let node_idx = node.index();

        assert!(self.graph.bindings.get(node_idx).is_some());

        let end_pass_idx = self.graph.passes.len();
        self.record_node_passes(cache, cmd_buf, node_idx, end_pass_idx)?;

        Ok(())
    }

    fn record_node_passes(
        &mut self,
        cache: &mut dyn ResolverPool,
        cmd_buf: &mut CommandBuffer,
        node_idx: usize,
        end_pass_idx: usize,
    ) -> Result<(), DriverError> {
        // Build a schedule for this node
        let mut schedule = self.schedule_node_passes(node_idx, end_pass_idx);

        self.record_scheduled_passes(cache, cmd_buf, &mut schedule, end_pass_idx)
    }

    fn record_scheduled_passes(
        &mut self,
        cache: &mut dyn ResolverPool,
        cmd_buf: &mut CommandBuffer,
        mut schedule: &mut [usize],
        end_pass_idx: usize,
    ) -> Result<(), DriverError> {
        if end_pass_idx == 0 {
            return Ok(());
        }

        // Print some handy details or hit a breakpoint if you set the flag
        #[cfg(debug_assertions)]
        if self.graph.debug {
            debug!("resolving the following graph:\n\n{:#?}\n\n", self.graph);
        }

        // Optimize the schedule; leasing the required stuff it needs
        self.reorder_scheduled_passes(schedule, end_pass_idx);
        schedule = self.merge_scheduled_passes(schedule);
        self.lease_scheduled_resources(cache, schedule)?;

        let mut passes = take(&mut self.graph.passes);
        for pass_idx in schedule.iter().copied() {
            let pass = &mut passes[pass_idx];
            let is_graphic = self.physical_passes[pass_idx].render_pass.is_some();

            trace!("recording pass [{}: {}]", pass_idx, pass.name);

            if !self.physical_passes[pass_idx]
                .exec_descriptor_sets
                .is_empty()
            {
                self.write_descriptor_sets(cmd_buf, pass, pass_idx)?;
            }

            Self::record_execution_barriers("  ", cmd_buf, &mut self.graph.bindings, pass, 0);

            let render_area = if is_graphic {
                let render_area = self.render_area(pass);
                self.begin_render_pass(cmd_buf, pass, pass_idx, render_area)?;
                Some(render_area)
            } else {
                None
            };

            for exec_idx in 0..pass.execs.len() {
                if is_graphic && exec_idx > 0 {
                    Self::next_subpass(cmd_buf);
                }

                if let Some(pipeline) = &mut pass.execs[exec_idx].pipeline.as_mut() {
                    self.bind_pipeline(cmd_buf, pass_idx, exec_idx, pipeline, pass.depth_stencil)?;

                    if is_graphic && pass.render_area.is_none() {
                        let render_area = render_area.unwrap();
                        // In this case we set the viewport and scissor for the user
                        Self::set_viewport(
                            cmd_buf,
                            render_area.width as _,
                            render_area.height as _,
                            pass.depth_stencil
                                .map(|depth_stencil| {
                                    let min = depth_stencil.min.0;
                                    let max = depth_stencil.max.0;
                                    min..max
                                })
                                .unwrap_or(0.0..1.0),
                        );
                        Self::set_scissor(cmd_buf, render_area.width, render_area.height);
                    }

                    self.bind_descriptor_sets(
                        cmd_buf,
                        pipeline,
                        &self.physical_passes[pass_idx],
                        exec_idx,
                    );
                }

                if exec_idx > 0 && !is_graphic {
                    Self::record_execution_barriers(
                        "    ",
                        cmd_buf,
                        &mut self.graph.bindings,
                        pass,
                        exec_idx,
                    );
                }

                trace!("    > exec[{exec_idx}]");

                let exec = &mut pass.execs[exec_idx];
                let exec_func = exec.func.take().unwrap().0;
                exec_func(
                    &cmd_buf.device,
                    **cmd_buf,
                    Bindings {
                        exec,
                        graph: &self.graph,
                    },
                );
            }

            if is_graphic {
                self.end_render_pass(cmd_buf);
            }
        }

        // We have to keep the bindings and pipelines alive until the gpu is done
        schedule.sort_unstable();
        while let Some(schedule_idx) = schedule.last().copied() {
            if passes.is_empty() {
                break;
            }

            while let (Some(pass), pass_idx) = (passes.pop(), passes.len()) {
                if pass_idx == schedule_idx {
                    // This was a scheduled pass - store it!
                    CommandBuffer::push_fenced_drop(cmd_buf, pass);
                    CommandBuffer::push_fenced_drop(cmd_buf, self.physical_passes.pop().unwrap());
                    let end = schedule.len() - 1;
                    schedule = &mut schedule[0..end];
                    break;
                } else {
                    debug_assert!(pass_idx > schedule_idx);

                    self.graph.passes.push(pass);
                }
            }
        }

        debug_assert!(self.physical_passes.is_empty());

        // Put the other passes back for future resolves
        passes.reverse();
        self.graph.passes.extend(passes);
        self.graph.passes.reverse();

        // log::trace!("OK");

        Ok(())
    }

    /// Records any pending render graph passes that have not been previously scheduled.
    pub fn record_unscheduled_passes(
        &mut self,
        cache: &mut dyn ResolverPool,
        cmd_buf: &mut CommandBuffer,
    ) -> Result<(), DriverError> {
        if self.graph.passes.is_empty() {
            return Ok(());
        }

        let mut schedule = (0..self.graph.passes.len()).collect::<Vec<_>>();

        self.record_scheduled_passes(cache, cmd_buf, &mut schedule, self.graph.passes.len())
    }

    fn render_area(&self, pass: &Pass) -> Area {
        pass.render_area.unwrap_or_else(|| {
            // set_render_area was not specified so we're going to guess using the extent
            // of the first attachment we find, by lowest attachment index order
            let first_exec = pass.execs.first().unwrap();

            // We must be able to find the render area because render passes require at least one
            // image to be attached
            let (width, height) = first_exec
                .loads
                .colors()
                .chain(first_exec.resolves.colors())
                .chain(first_exec.stores.colors())
                .find_map(|(_, attachment)| {
                    self.graph.bindings[attachment.target]
                        .as_driver_image()
                        .map(|image| (image.info.width, image.info.height))
                })
                .expect("invalid attachments");

            Area {
                height,
                width,
                x: 0,
                y: 0,
            }
        })
    }

    fn reorder_scheduled_passes(&mut self, schedule: &mut [usize], end_pass_idx: usize) {
        // It must be a party
        if schedule.len() < 3 {
            return;
        }

        let mut scheduled = 0;
        let mut unscheduled = schedule.iter().copied().collect::<BTreeSet<_>>();

        // Re-order passes by maximizing the distance between dependent nodes
        while !unscheduled.is_empty() {
            let mut best_idx = scheduled;
            let pass_idx = schedule[best_idx];
            let mut best_overlap_factor =
                self.interdependent_passes(pass_idx, end_pass_idx).count();

            for (idx, pass_idx) in schedule.iter().enumerate().skip(scheduled + 1) {
                let overlap_factor = self.interdependent_passes(*pass_idx, end_pass_idx).count();
                if overlap_factor > best_overlap_factor {
                    // TODO: These iterators double the work, could be like the schedule function does it
                    if self
                        .interdependent_passes(*pass_idx, end_pass_idx)
                        .any(|other_pass_idx| unscheduled.contains(&other_pass_idx))
                    {
                        // This pass can't be the candidate because it depends on unfinished work
                        continue;
                    }

                    best_idx = idx;
                    best_overlap_factor = overlap_factor;
                }
            }

            unscheduled.remove(&schedule[best_idx]);
            schedule.swap(scheduled, best_idx);
            scheduled += 1;
        }
    }

    /// Returns a vec of pass indexes that are required to be executed, in order, for the given
    /// node.
    fn schedule_node_passes(&self, node_idx: usize, end_pass_idx: usize) -> Vec<usize> {
        let mut schedule = vec![];
        let mut unscheduled = self.graph.passes[0..end_pass_idx]
            .iter()
            .enumerate()
            .map(|(idx, _)| idx)
            .collect::<BTreeSet<_>>();
        let mut unresolved = VecDeque::new();

        //trace!("scheduling node {node_idx}");

        // Schedule the first set of passes for the node we're trying to resolve
        for pass_idx in self.dependent_passes(node_idx, end_pass_idx) {
            // trace!(
            //     "  pass [{pass_idx}: {}] is dependent",
            //     self.graph.passes[pass_idx].name
            // );

            schedule.push(pass_idx);
            unscheduled.remove(&pass_idx);
            for node_idx in self.dependent_nodes(pass_idx) {
                // trace!("    node {node_idx} is dependent");

                unresolved.push_back((node_idx, pass_idx));
            }
        }

        //trace!("secondary passes below");

        // Now schedule all nodes that are required, going through the tree to find them
        while let Some((node_idx, end_pass_idx)) = unresolved.pop_front() {
            for pass_idx in self.dependent_passes(node_idx, end_pass_idx) {
                // trace!(
                //     "  pass [{pass_idx}: {}] is dependent",
                //     self.graph.passes[pass_idx].name
                // );

                if unscheduled.remove(&pass_idx) {
                    schedule.push(pass_idx);
                    for node_idx in self.dependent_nodes(pass_idx) {
                        // trace!("    node {node_idx} is dependent");

                        unresolved.push_back((node_idx, pass_idx));
                    }
                }
            }
        }

        schedule.reverse();

        if !schedule.is_empty() {
            // These are the indexes of the passes this thread is about to resolve
            debug!(
                "schedule: {}",
                schedule
                    .iter()
                    .copied()
                    .map(|idx| format!("[{}: {}]", idx, self.graph.passes[idx].name))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            if !unscheduled.is_empty() {
                // These passes are within the range of passes we thought we had to do
                // right now, but it turns out that nothing in "schedule" relies on them
                trace!(
                    "delaying: {}",
                    unscheduled
                        .iter()
                        .copied()
                        .map(|idx| format!("[{}: {}]", idx, self.graph.passes[idx].name))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            if end_pass_idx < self.graph.passes.len() {
                // These passes existing on the graph but are not being considered right
                // now because we've been told to stop work at the "end_pass_idx" point
                trace!(
                    "ignoring: {}",
                    self.graph.passes[end_pass_idx..]
                        .iter()
                        .enumerate()
                        .map(|(idx, pass)| format!("[{}: {}]", idx + end_pass_idx, pass.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }

        // if schedule.is_empty() && unscheduled.is_empty() && end_pass_idx < self.graph.passes.len() {
        //     // This may be totally normal in some situations, not sure if this will stay
        //     warn!("Unable to schedule any render passes");
        // }

        schedule
    }

    fn set_scissor(cmd_buf: &CommandBuffer, width: u32, height: u32) {
        use std::slice::from_ref;

        unsafe {
            cmd_buf.device.cmd_set_scissor(
                **cmd_buf,
                0,
                from_ref(&vk::Rect2D {
                    extent: vk::Extent2D { width, height },
                    offset: vk::Offset2D { x: 0, y: 0 },
                }),
            );
        }
    }

    fn set_viewport(cmd_buf: &CommandBuffer, width: f32, height: f32, depth: Range<f32>) {
        use std::slice::from_ref;

        unsafe {
            cmd_buf.device.cmd_set_viewport(
                **cmd_buf,
                0,
                from_ref(&vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                    min_depth: depth.start,
                    max_depth: depth.end,
                }),
            );
        }
    }

    pub fn submit(
        mut self,
        queue: &Queue,
        cache: &mut impl ResolverPool,
    ) -> Result<(), DriverError> {
        use std::slice::from_ref;

        trace!("submit");

        let mut cmd_buf = cache.lease(queue.family)?;

        unsafe {
            Device::wait_for_fence(&cmd_buf.device, &cmd_buf.fence)
                .map_err(|_| DriverError::OutOfMemory)?;

            cmd_buf
                .device
                .reset_command_pool(cmd_buf.pool, vk::CommandPoolResetFlags::RELEASE_RESOURCES)
                .map_err(|_| DriverError::OutOfMemory)?;
            cmd_buf
                .device
                .begin_command_buffer(
                    **cmd_buf,
                    &vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(|_| DriverError::OutOfMemory)?;
        }

        self.record_unscheduled_passes(cache, &mut cmd_buf)?;

        unsafe {
            cmd_buf
                .device
                .end_command_buffer(**cmd_buf)
                .map_err(|_| DriverError::OutOfMemory)?;
            cmd_buf
                .device
                .reset_fences(from_ref(&cmd_buf.fence))
                .map_err(|_| DriverError::OutOfMemory)?;
            cmd_buf
                .device
                .queue_submit(
                    **queue,
                    from_ref(&vk::SubmitInfo::builder().command_buffers(from_ref(&cmd_buf))),
                    cmd_buf.fence,
                )
                .map_err(|_| DriverError::OutOfMemory)?;
        }

        // This graph contains references to buffers, images, and other resources which must be kept
        // alive until this graph execution completes on the GPU. Once those references are dropped
        // they will return to the pool for other things to use. The drop will happen the next time
        // someone tries to lease a command buffer and we notice this one has returned and the fence
        // has been signalled.
        CommandBuffer::push_fenced_drop(&mut cmd_buf, self);

        Ok(())
    }

    pub fn unbind_node<N>(&mut self, node: N) -> <N as Edge<Self>>::Result
    where
        N: Edge<Self>,
        N: Unbind<Self, <N as Edge<Self>>::Result>,
    {
        node.unbind(self)
    }

    fn write_descriptor_sets(
        &self,
        cmd_buf: &CommandBuffer,
        pass: &Pass,
        pass_idx: usize,
    ) -> Result<(), DriverError> {
        thread_local! {
            static WRITES: RefCell<Writes> = Default::default();
        }

        struct IndexWrite {
            idx: usize,
            write: vk::WriteDescriptorSet,
        }

        #[derive(Default)]
        struct Writes {
            accel_struct_infos: Vec<vk::WriteDescriptorSetAccelerationStructureKHR>,
            accel_struct_writes: Vec<IndexWrite>,
            buffer_infos: Vec<vk::DescriptorBufferInfo>,
            buffer_writes: Vec<IndexWrite>,
            descriptors: Vec<vk::WriteDescriptorSet>,
            image_infos: Vec<vk::DescriptorImageInfo>,
            image_writes: Vec<IndexWrite>,
        }

        WRITES.with(|writes| {
            // Initialize TLS from a previous call
            let Writes {
                accel_struct_infos,
                accel_struct_writes,
                buffer_infos,
                buffer_writes,
                descriptors,
                image_infos,
                image_writes,
            } = &mut *writes.borrow_mut();
            accel_struct_infos.clear();
            accel_struct_writes.clear();
            buffer_infos.clear();
            buffer_writes.clear();
            descriptors.clear();
            image_infos.clear();
            image_writes.clear();

            let descriptor_sets = &self.physical_passes[pass_idx].exec_descriptor_sets;
            for (exec_idx, exec, pipeline) in pass
                .execs
                .iter()
                .enumerate()
                .filter_map(|(exec_idx, exec)| {
                    exec.pipeline
                        .as_ref()
                        .map(|pipeline| (exec_idx, exec, pipeline))
                })
                .filter(|(.., pipeline)| !pipeline.descriptor_info().layouts.is_empty())
            {
                let descriptor_sets = &descriptor_sets[&exec_idx];

                // Write the manually bound things (access, read, and write functions)
                for (descriptor, (node_idx, view_info)) in exec.bindings.iter() {
                    let (descriptor_set_idx, dst_binding, binding_offset) = descriptor.into_tuple();
                    let (descriptor_info, _) = *pipeline
                        .descriptor_bindings()
                        .get(&DescriptorBinding(descriptor_set_idx, dst_binding))
                        .unwrap_or_else(|| panic!("descriptor {descriptor_set_idx}.{dst_binding}[{binding_offset}] specified in recorded execution of pass \"{}\" was not discovered through shader reflection", &pass.name));
                    let descriptor_type = descriptor_info.into();
                    let bound_node = &self.graph.bindings[*node_idx];
                    if let Some(image) = bound_node.as_driver_image() {
                        let view_info = view_info.as_ref().unwrap();
                        let mut image_view_info = *view_info.as_image().unwrap();

                        // Handle default views which did not specify a particaular aspect
                        if image_view_info.aspect_mask.is_empty() {
                            image_view_info.aspect_mask = format_aspect_mask(image.info.fmt);
                        }

                        let sampler = descriptor_info.sampler().unwrap_or_default();
                        let image_view = Image::view_ref(image, image_view_info)?;
                        let image_layout = match descriptor_type {
                            vk::DescriptorType::COMBINED_IMAGE_SAMPLER => {
                                if image_view_info.aspect_mask.contains(
                                    vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                                ) {
                                    vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
                                } else if image_view_info
                                    .aspect_mask
                                    .contains(vk::ImageAspectFlags::DEPTH)
                                {
                                    vk::ImageLayout::DEPTH_READ_ONLY_OPTIMAL
                                } else if image_view_info
                                    .aspect_mask
                                    .contains(vk::ImageAspectFlags::STENCIL)
                                {
                                    vk::ImageLayout::STENCIL_READ_ONLY_OPTIMAL
                                } else {
                                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                                }
                            }
                            vk::DescriptorType::STORAGE_IMAGE => vk::ImageLayout::GENERAL,
                            _ => unimplemented!(),
                        };

                        if binding_offset == 0 {
                            image_writes.push(IndexWrite {
                                idx: image_infos.len(),
                                write: vk::WriteDescriptorSet {
                                        dst_set: *descriptor_sets[descriptor_set_idx as usize],
                                        dst_binding,
                                        descriptor_type,
                                        descriptor_count: 1,
                                        ..Default::default()
                                    },
                                }
                            );
                        } else {
                            image_writes.last_mut().unwrap().write.descriptor_count += 1;
                        }

                        image_infos.push(vk::DescriptorImageInfo {
                            image_layout,
                            image_view,
                            sampler,
                        });
                    } else if let Some(buffer) = bound_node.as_driver_buffer() {
                        let view_info = view_info.as_ref().unwrap();
                        let buffer_view_info = view_info.as_buffer().unwrap();

                        if binding_offset == 0 {
                            buffer_writes.push(IndexWrite {
                                idx: buffer_infos.len(),
                                write: vk::WriteDescriptorSet {
                                        dst_set: *descriptor_sets[descriptor_set_idx as usize],
                                        dst_binding,
                                        descriptor_type,
                                        descriptor_count: 1,
                                        ..Default::default()
                                    },
                                }
                            );
                        } else {
                            buffer_writes.last_mut().unwrap().write.descriptor_count += 1;
                        }

                        buffer_infos.push(vk::DescriptorBufferInfo {
                            buffer: **buffer,
                            offset: buffer_view_info.start,
                            range: buffer_view_info.end - buffer_view_info.start,
                        });
                    } else if let Some(accel_struct) = bound_node.as_driver_acceleration_structure() {
                        if binding_offset == 0 {
                            accel_struct_writes.push(IndexWrite {
                                idx: accel_struct_infos.len(),
                                write: vk::WriteDescriptorSet {
                                    dst_set: *descriptor_sets[descriptor_set_idx as usize],
                                    dst_binding,
                                    descriptor_type,
                                    descriptor_count: 1,
                                    ..Default::default()
                                },
                            });
                        } else {
                            accel_struct_writes.last_mut().unwrap().write.descriptor_count += 1;
                        }

                        accel_struct_infos.push(vk::WriteDescriptorSetAccelerationStructureKHR::builder().acceleration_structures(std::slice::from_ref(accel_struct)).build());
                    } else {
                        unimplemented!();
                    }
                }

                // Write graphic render pass input attachments (they're automatic)
                if exec_idx > 0 && pipeline.is_graphic() {
                    let pipeline = pipeline.unwrap_graphic();
                    for (&DescriptorBinding(descriptor_set_idx, dst_binding), (descriptor_info, _)) in
                        &pipeline.descriptor_bindings
                    {
                        if let &DescriptorInfo::InputAttachment(_, attachment_idx) = descriptor_info {
                            let is_random_access = exec.resolves.contains_color(attachment_idx)
                                || exec.stores.contains_color(attachment_idx);
                            let (attachment, write_exec) = pass.execs[0..exec_idx]
                                .iter()
                                .rev()
                                .find_map(|exec| {
                                    exec.stores
                                        .color(attachment_idx)
                                        .map(|attachment| {
                                            (attachment, exec)
                                        })
                                        .or_else(|| {
                                            exec.resolves.color(attachment_idx).map(
                                                |attachment| {
                                                    (attachment, exec)
                                                },
                                            )
                                        })
                                })
                                .expect("input attachment not written");
                            let [_, late] = &write_exec.accesses[&attachment.target];
                            let image_subresource = late.subresource.as_ref().unwrap().unwrap_image();
                            let image_binding = &self.graph.bindings[attachment.target];
                            let image = image_binding.as_driver_image().unwrap();
                            let image_view_info = ImageViewInfo {
                                array_layer_count: image_subresource.array_layer_count,
                                aspect_mask: attachment.aspect_mask,
                                base_array_layer: image_subresource.base_array_layer,
                                base_mip_level: image_subresource.base_mip_level,
                                fmt: attachment.fmt,
                                mip_level_count: image_subresource.mip_level_count,
                                ty: image.info.ty,
                            };
                            let image_view = Image::view_ref(image, image_view_info)?;
                            let sampler = descriptor_info.sampler().unwrap_or_else(vk::Sampler::null);

                            image_writes.push(IndexWrite {
                                idx: image_infos.len(),
                                write: vk::WriteDescriptorSet {
                                        dst_set: *descriptor_sets[descriptor_set_idx as usize],
                                        dst_binding,
                                        descriptor_type: vk::DescriptorType::INPUT_ATTACHMENT,
                                        descriptor_count: 1,
                                        ..Default::default()
                                    },
                                }
                            );

                            image_infos.push(vk::DescriptorImageInfo {
                                image_layout: Self::attachment_layout(
                                    attachment.aspect_mask,
                                    is_random_access,
                                    true,
                                ),
                                image_view,
                                sampler,
                            });
                        }
                    }
                }
            }

            // NOTE: We assign the below pointers after the above insertions so they remain stable!

            descriptors.extend(accel_struct_writes.drain(..).map(|IndexWrite { idx, mut write }| unsafe {
                write.p_next = accel_struct_infos.as_ptr().add(idx) as *const _;
                write
            }));
            descriptors.extend(buffer_writes.drain(..).map(|IndexWrite { idx, mut write }| unsafe {
                write.p_buffer_info = buffer_infos.as_ptr().add(idx);
                write
            }));
            descriptors.extend(image_writes.drain(..).map(|IndexWrite { idx, mut write }| unsafe {
                write.p_image_info = image_infos.as_ptr().add(idx);
                write
            }));

            if !descriptors.is_empty() {
                trace!("  writing {} descriptors ({} buffers, {} images)", descriptors.len(), buffer_infos.len(), image_infos.len());

                unsafe {
                    cmd_buf
                        .device
                        .update_descriptor_sets(descriptors.as_slice(), &[]);
                }
            }

            Ok(())
        })
    }
}

pub trait ResolverPool:
    Pool<DescriptorPoolInfo, DescriptorPool>
    + Pool<RenderPassInfo, RenderPass>
    + Pool<QueueFamily, CommandBuffer>
{
}

impl ResolverPool for HashPool {}

impl ResolverPool for LazyPool {}
