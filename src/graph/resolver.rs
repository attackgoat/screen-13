use {
    super::{
        AttachmentIndex, AttachmentMap, Binding, Bindings, Edge, ExecutionPipeline, Node, Pass,
        Rect, RenderGraph, Subpass, Unbind,
    },
    crate::{
        align_up_u32,
        driver::{
            format_aspect_mask, image_access_layout, is_read_access, is_write_access,
            AttachmentInfo, AttachmentRef, CommandBuffer, DepthStencilMode, DescriptorBinding,
            DescriptorInfo, DescriptorPool, DescriptorPoolInfo, DescriptorPoolSize, DescriptorSet,
            Device, DriverError, FramebufferKey, FramebufferKeyAttachment, Image, ImageViewInfo,
            RenderPass, RenderPassInfo, SampleCount, SubpassDependency, SubpassInfo,
        },
        ptr::Shared,
        HashPool, Lease,
    },
    archery::SharedPointerKind,
    ash::vk,
    glam::{IVec2, UVec2},
    itertools::Itertools,
    log::{debug, trace},
    std::{
        collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
        iter::{once, repeat},
        mem::take,
        ops::Range,
    },
    vk_sync::{cmd::pipeline_barrier, AccessType, BufferBarrier, GlobalBarrier, ImageBarrier},
};

#[derive(Debug)]
struct PhysicalPass<P>
where
    P: SharedPointerKind,
{
    _descriptor_pool: Option<Lease<Shared<DescriptorPool<P>, P>, P>>,
    exec_descriptor_sets: HashMap<usize, Vec<DescriptorSet<P>>>,
    render_pass: Option<Lease<RenderPass<P>, P>>,
}

/// A structure which can read and execute render graphs. This pattern was derived from:
///
/// <http://themaister.net/blog/2017/08/15/render-graphs-and-vulkan-a-deep-dive/>
/// <https://github.com/EmbarkStudios/kajiya>
#[derive(Debug)]
pub struct Resolver<P>
where
    P: SharedPointerKind + Send,
{
    pub(super) graph: RenderGraph<P>,
    physical_passes: Vec<PhysicalPass<P>>,
}

impl<P> Resolver<P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub(super) fn new(graph: RenderGraph<P>) -> Self {
        let physical_passes = Vec::with_capacity(graph.passes.len());

        Self {
            graph,
            physical_passes,
        }
    }

    fn allow_merge_passes(lhs: &Pass<P>, rhs: &Pass<P>) -> bool {
        // Don't attempt merge on secondary resolves (it is unlikely to succeed)
        if !rhs.subpasses.is_empty() {
            trace!("{} has already been merged", rhs.name);

            return false;
        }

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
                trace!("{} is not graphic", lhs.name);
            }

            if rhs_pipeline.is_none() {
                trace!("{} is not graphic", rhs.name);
            }

            return false;
        }

        let lhs_pipeline = lhs_pipeline.unwrap().unwrap_graphic();
        let rhs_pipeline = rhs_pipeline.unwrap().unwrap_graphic();

        // Must be same general rasterization modes
        if lhs_pipeline.info != rhs_pipeline.info {
            trace!("Different rasterization modes",);

            return false;
        }

        // Now we need to know what the subpasses (we may have prior merges) wrote
        for (lhs_attachments_resolved, lhs_attachments_stored) in lhs
            .subpasses
            .iter()
            .rev()
            .map(|subpass| (&subpass.resolve_attachments, &subpass.store_attachments))
            .chain(once((&lhs.resolve_attachments, &lhs.store_attachments)))
        {
            // Compare individual color/depth+stencil attachments for compatibility
            if !AttachmentMap::are_compatible(lhs_attachments_resolved, &rhs.load_attachments)
                || !AttachmentMap::are_compatible(lhs_attachments_stored, &rhs.load_attachments)
            {
                trace!("Incompatible attachments");

                return false;
            }

            // Keep color and depth on tile.
            for node_idx in rhs.load_attachments.images() {
                if lhs_attachments_resolved.contains_image(node_idx)
                    || lhs_attachments_stored.contains_image(node_idx)
                {
                    trace!("Merging due to common image");

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
            trace!("Merging due to input");

            return true;
        }

        // No reason to merge, so don't.
        false
    }

    fn begin_render_pass(
        &mut self,
        cmd_buf: &CommandBuffer<P>,
        pass: &Pass<P>,
        physical_pass_idx: usize,
        render_area: Rect<UVec2, IVec2>,
    ) -> Result<(), DriverError> {
        trace!("begin_render_pass");

        let physical_pass = &self.physical_passes[physical_pass_idx];
        let render_pass = physical_pass.render_pass.as_ref().unwrap();
        let attached_images = {
            let mut attachment_queue =
                (0..render_pass.info.attachments.len()).collect::<VecDeque<_>>();
            let mut res = Vec::with_capacity(attachment_queue.len());
            res.extend(repeat(None).take(attachment_queue.len()));
            while let Some(attachment_idx) = attachment_queue.pop_front() {
                for subpass in pass.subpasses.iter() {
                    if let Some(attachment) = subpass.attachment(attachment_idx as _) {
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
                    extent_x: image.info.extent.x,
                    extent_y: image.info.extent.y,
                    layer_count: image.info.array_elements,
                    view_fmts: pass
                        .subpasses
                        .iter()
                        .map(|subpass| subpass.attachment(attachment_idx as _).unwrap().fmt)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                })
                .collect(),
            extent_x: render_area.extent.x,
            extent_y: render_area.extent.y,
        })?;

        unsafe {
            cmd_buf.device.cmd_begin_render_pass(
                **cmd_buf,
                &vk::RenderPassBeginInfo::builder()
                    .render_pass(***render_pass)
                    .framebuffer(framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D {
                            x: render_area.offset.x,
                            y: render_area.offset.y,
                        },
                        extent: vk::Extent2D {
                            width: render_area.extent.x,
                            height: render_area.extent.y,
                        },
                    })
                    .clear_values(
                        &pass
                            .execs
                            .get(0)
                            .unwrap()
                            .clears
                            .values()
                            .copied()
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
        cmd_buf: &CommandBuffer<P>,
        pipeline: &ExecutionPipeline<P>,
        physical_pass: &PhysicalPass<P>,
        exec_idx: usize,
    ) {
        trace!("bind_descriptor_sets");

        unsafe {
            cmd_buf.device.cmd_bind_descriptor_sets(
                **cmd_buf,
                pipeline.bind_point(),
                pipeline.layout(),
                0,
                &physical_pass.exec_descriptor_sets[&exec_idx]
                    .iter()
                    .map(|descriptor_set| **descriptor_set)
                    .collect::<Box<[_]>>(),
                &[],
            );
        }
    }

    fn bind_pipeline(
        &self,
        cmd_buf: &mut CommandBuffer<P>,
        physical_pass_idx: usize,
        subpass_idx: u32,
        pipeline: &mut ExecutionPipeline<P>,
        depth_stencil: Option<DepthStencilMode>,
    ) -> Result<(), DriverError> {
        trace!("bind_pipeline");

        // We store a shared reference to this pipeline inside the command buffer!
        let physical_pass = &self.physical_passes[physical_pass_idx];
        let pipeline_bind_point = pipeline.bind_point();
        let pipeline = match pipeline {
            ExecutionPipeline::Compute(pipeline) => {
                CommandBuffer::push_fenced_drop(cmd_buf, Shared::clone(pipeline));
                ***pipeline
            }
            ExecutionPipeline::Graphic(pipeline) => {
                CommandBuffer::push_fenced_drop(cmd_buf, Shared::clone(pipeline));
                physical_pass
                    .render_pass
                    .as_ref()
                    .unwrap()
                    .graphic_pipeline_ref(pipeline, depth_stencil, subpass_idx)?
            }
            ExecutionPipeline::RayTrace(pipeline) => {
                CommandBuffer::push_fenced_drop(cmd_buf, Shared::clone(pipeline));
                ***pipeline
            }
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
        max_pass_idx: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        // TODO: We could store the nodes of a pass so we don't need to do these horrible things
        self.graph.passes.as_slice()[0..max_pass_idx]
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
            .filter_map(move |(node_idx, accesses)| {
                if is_read_access(accesses[0].access) && already_seen.insert(*node_idx) {
                    Some(*node_idx)
                } else {
                    None
                }
            })
    }

    fn end_render_pass(&mut self, cmd_buf: &CommandBuffer<P>) {
        trace!("end_render_pass");

        unsafe {
            cmd_buf.device.cmd_end_render_pass(**cmd_buf);
        }
    }

    /// Returns the unique indexes of the passes which are dependent on the given pass.
    fn interdependent_passes(
        &self,
        pass_idx: usize,
        max_pass_idx: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        let mut already_seen = BTreeSet::new();
        already_seen.insert(pass_idx);
        self.dependent_nodes(pass_idx)
            .flat_map(move |node_idx| self.dependent_passes(node_idx, max_pass_idx))
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
        cache: &mut HashPool<P>,
        pass: &Pass<P>,
    ) -> Result<Option<Lease<Shared<DescriptorPool<P>, P>, P>>, DriverError> {
        let mut max_pool_sizes = BTreeMap::new();
        let max_descriptor_set_idx = pass
            .execs
            .iter()
            .filter_map(|exec| exec.bindings.keys().last())
            .map(|descriptor| descriptor.set())
            .max()
            .unwrap_or_default();

        // Find the total count of descriptors per type (there may be multiple pipelines!)
        for pool_sizes in pass.descriptor_pools_sizes() {
            for pool_size in pool_sizes.values() {
                for (descriptor_ty, descriptor_count) in pool_size.iter() {
                    assert_ne!(*descriptor_count, 0);

                    *max_pool_sizes.entry(*descriptor_ty).or_default() += descriptor_count;
                }
            }
        }

        // It's possible to execute a command-only pipeline
        if max_pool_sizes.is_empty() {
            return Ok(None);
        }

        // Notice how all sets are big enough for any other set; TODO: efficiently dont
        let info = DescriptorPoolInfo::new(pass.execs.len() as u32 * (max_descriptor_set_idx + 1))
            .pool_sizes(
                max_pool_sizes
                    .into_iter()
                    .map(|(descriptor_ty, descriptor_count)| DescriptorPoolSize {
                        ty: descriptor_ty,
                        // Trivially round up the descriptor counts to increase cache coherence
                        descriptor_count: align_up_u32(descriptor_count, 1 << 5),
                    })
                    .collect(),
            );

        // debug!("{:#?}", info);

        let pool = cache.lease(info)?;

        Ok(Some(pool))
    }

    fn lease_render_pass(
        cache: &mut HashPool<P>,
        pass: &mut Pass<P>,
    ) -> Result<Lease<RenderPass<P>, P>, DriverError> {
        let attachment_count = pass
            .subpasses
            .iter()
            .map(|pass| pass.attachment_count())
            .max()
            .unwrap_or_default();
        let mut attachments = Vec::with_capacity(attachment_count);
        let mut dependencies = Vec::with_capacity(attachment_count);
        let mut subpasses = Vec::with_capacity(pass.subpasses.len() + 1);

        while attachments.len() < attachment_count {
            attachments.push(
                AttachmentInfo::new(vk::Format::UNDEFINED, SampleCount::X1)
                    .build()
                    .unwrap(),
            );
        }

        // Add attachments: format, sample count, load op, and initial layout (using the 1st pass)
        {
            let first_pass = &pass.subpasses[0];
            let first_exec = &pass.execs[0];
            let depth_stencil = first_pass
                .load_attachments
                .depth_stencil()
                .or_else(|| first_pass.resolve_attachments.depth_stencil())
                .or_else(|| first_pass.store_attachments.depth_stencil())
                .map(|(attachment_idx, _)| attachment_idx);

            // Cleared attachments
            for attachment_idx in first_exec.clears.keys().copied() {
                let attachment = &mut attachments[attachment_idx as usize];
                if matches!(depth_stencil, Some(depth_stencil_attachment_idx) if depth_stencil_attachment_idx == attachment_idx)
                {
                    // DEPTH/STENCIL
                    // Note: Layout will be set if (..when..) we're resolved or stored
                    // We don't set depth/stencil initial layout here because we don't
                    // know the view aspect flags yet - we let the store or resolve op
                    // set the initial layout

                    attachment.stencil_load_op = vk::AttachmentLoadOp::CLEAR;
                } else {
                    // COLOR
                    attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                    attachment.load_op = vk::AttachmentLoadOp::CLEAR;
                }
            }

            // Loaded attachments
            for (attachment_idx, loaded_attachment) in first_pass
                .load_attachments
                .attached
                .iter()
                .enumerate()
                .filter_map(|(attachment_idx, attachment)| {
                    attachment.map(|attachment| (attachment_idx as AttachmentIndex, attachment))
                })
            {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.fmt = loaded_attachment.fmt;
                attachment.sample_count = loaded_attachment.sample_count;

                if matches!(depth_stencil, Some(depth_stencil_attachment_idx) if depth_stencil_attachment_idx == attachment_idx)
                {
                    // DEPTH/STENCIL
                    let is_random_access = first_pass
                        .store_attachments
                        .contains_attachment(attachment_idx)
                        || first_pass
                            .resolve_attachments
                            .contains_attachment(attachment_idx);
                    attachment.initial_layout = if loaded_attachment
                        .aspect_mask
                        .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                    {
                        if is_random_access {
                            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                        } else {
                            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
                        }
                    } else if loaded_attachment
                        .aspect_mask
                        .contains(vk::ImageAspectFlags::DEPTH)
                    {
                        if is_random_access {
                            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                        } else {
                            vk::ImageLayout::DEPTH_READ_ONLY_OPTIMAL
                        }
                    } else if is_random_access {
                        vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                    } else {
                        vk::ImageLayout::STENCIL_READ_ONLY_OPTIMAL
                    };
                    attachment.stencil_load_op = vk::AttachmentLoadOp::LOAD;
                } else {
                    // COLOR
                    attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                    attachment.load_op = vk::AttachmentLoadOp::LOAD;
                }
            }

            // Resolved attachments
            for (attachment_idx, resolved_attachment) in first_pass
                .resolve_attachments
                .attached
                .iter()
                .enumerate()
                .filter_map(|(attachment_idx, attachment)| {
                    attachment.map(|attachment| (attachment_idx as AttachmentIndex, attachment))
                })
            {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.fmt = resolved_attachment.fmt;
                attachment.sample_count = resolved_attachment.sample_count;

                // Set layout here bc we did not set it above, if we handled a clear op
                if matches!(depth_stencil, Some(depth_stencil_attachment_idx) if depth_stencil_attachment_idx == attachment_idx)
                {
                    // DEPTH/STENCIL

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
                } else {
                    // COLOR
                    attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                }
            }

            // Stored attachments
            for (attachment_idx, stored_attachment) in first_pass
                .store_attachments
                .attached
                .iter()
                .enumerate()
                .filter_map(|(attachment_idx, attachment)| {
                    attachment.map(|attachment| (attachment_idx as AttachmentIndex, attachment))
                })
            {
                let attachment = &mut attachments[attachment_idx as usize];
                attachment.fmt = stored_attachment.fmt;
                attachment.sample_count = stored_attachment.sample_count;

                // Set layout here bc we did not set it above, if we handled a clear op
                if matches!(depth_stencil, Some(depth_stencil_attachment_idx) if depth_stencil_attachment_idx == attachment_idx)
                {
                    // DEPTH/STENCIL

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
                } else {
                    // COLOR
                    attachment.initial_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                }
            }
        }

        // Add attachments: store op and final layout (using the last pass)
        {
            let last_pass = pass.subpasses.last().unwrap();
            let depth_stencil = last_pass
                .load_attachments
                .depth_stencil()
                .or_else(|| last_pass.resolve_attachments.depth_stencil())
                .or_else(|| last_pass.store_attachments.depth_stencil());

            // Resolved attachments
            for (attachment_idx, resolved_attachment) in last_pass
                .resolve_attachments
                .attached
                .iter()
                .enumerate()
                .filter_map(|(attachment_idx, attachment)| {
                    attachment.map(|attachment| (attachment_idx as AttachmentIndex, attachment))
                })
            {
                let attachment = &mut attachments[attachment_idx as usize];

                if matches!(depth_stencil, Some((depth_stencil_attachment_idx, _)) if depth_stencil_attachment_idx == attachment_idx)
                {
                    // DEPTH/STENCIL
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
                } else {
                    // COLOR
                    attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                }
            }

            // Stored attachments
            for (attachment_idx, stored_attachment) in last_pass
                .store_attachments
                .attached
                .iter()
                .enumerate()
                .filter_map(|(attachment_idx, attachment)| {
                    attachment.map(|attachment| (attachment_idx as AttachmentIndex, attachment))
                })
            {
                let attachment = &mut attachments[attachment_idx as usize];

                // Set layout here bc we did not set it above, if we handled a clear op
                if matches!(depth_stencil, Some((depth_stencil_attachment_idx, _)) if depth_stencil_attachment_idx == attachment_idx)
                {
                    // DEPTH/STENCIL
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
                    attachment.stencil_store_op = vk::AttachmentStoreOp::STORE;
                } else {
                    // COLOR
                    attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                    attachment.store_op = vk::AttachmentStoreOp::STORE;
                }
            }
        }

        // Add subpasses
        for (subpass_idx, subpass) in pass.subpasses.iter().enumerate() {
            let exec = &pass.execs[subpass.exec_idx];
            let depth_stencil = subpass
                .load_attachments
                .depth_stencil()
                .or_else(|| subpass.resolve_attachments.depth_stencil())
                .or_else(|| subpass.store_attachments.depth_stencil());
            let mut subpass_info = SubpassInfo::with_capacity(attachment_count);

            // Color attachments prior to the depth attachment
            {
                let depth_stencil_attachment_idx = depth_stencil
                    .map(|(attachment_idx, _)| attachment_idx as usize)
                    .unwrap_or_default();
                for attachment_idx in 0..depth_stencil_attachment_idx {
                    subpass_info.color_attachments.push(AttachmentRef::new(
                        attachment_idx as _,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    ));
                }
            }

            // Color attachments after the depth attachment
            {
                let after_depth_stencil_attachment_idx = depth_stencil
                    .map(|(attachment_idx, _)| attachment_idx as usize + 1)
                    .unwrap_or_default();
                for attachment_idx in after_depth_stencil_attachment_idx..attachment_count {
                    subpass_info.color_attachments.push(AttachmentRef::new(
                        attachment_idx as _,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    ));
                }
            }

            // Set resolves to defaults for now
            subpass_info.resolve_attachments.extend(
                repeat(AttachmentRef::new(
                    vk::ATTACHMENT_UNUSED,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                ))
                .take(subpass_info.color_attachments.len()),
            );

            // Set depth/stencil attachment
            if let Some((depth_stencil_attachment_idx, _)) = depth_stencil {
                let used_depth_stencil_attachment = subpass
                    .load_attachments
                    .attached
                    .get(depth_stencil_attachment_idx as usize)
                    .or_else(|| {
                        subpass
                            .resolve_attachments
                            .attached
                            .get(depth_stencil_attachment_idx as usize)
                    })
                    .or_else(|| {
                        subpass
                            .store_attachments
                            .attached
                            .get(depth_stencil_attachment_idx as usize)
                    });
                if let Some(Some(used_depth_stencil_attachment)) = used_depth_stencil_attachment {
                    let is_random_access = subpass
                        .store_attachments
                        .contains_attachment(depth_stencil_attachment_idx)
                        || subpass
                            .resolve_attachments
                            .contains_attachment(depth_stencil_attachment_idx);
                    subpass_info.depth_stencil_attachment = Some(AttachmentRef::new(
                        depth_stencil_attachment_idx as _,
                        if used_depth_stencil_attachment
                            .aspect_mask
                            .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                        {
                            if is_random_access {
                                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
                            } else {
                                vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
                            }
                        } else if used_depth_stencil_attachment
                            .aspect_mask
                            .contains(vk::ImageAspectFlags::DEPTH)
                        {
                            if is_random_access {
                                vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                            } else {
                                vk::ImageLayout::DEPTH_READ_ONLY_OPTIMAL
                            }
                        } else if is_random_access {
                            vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL
                        } else {
                            vk::ImageLayout::STENCIL_READ_ONLY_OPTIMAL
                        },
                    ));
                }
            }

            // Look for any input attachments and handle those too
            if let Some(pipeline) = exec
                .pipeline
                .as_ref()
                .map(|pipeline| pipeline.unwrap_graphic())
            {
                for (_, descriptor_info) in pipeline.descriptor_bindings.iter() {
                    if let DescriptorInfo::InputAttachment(_, attachment_idx) = descriptor_info {
                        subpass_info.input_attachments.push(AttachmentRef::new(
                            *attachment_idx,
                            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        ));

                        // We should preserve the attachment in the previous subpass
                        // (We're asserting that any input renderpasses are actually
                        // real subpasses here with prior passes..)
                        let t: &mut SubpassInfo = &mut subpasses[subpass_idx - 1];
                        t.preserve_attachments.push(0);
                    }
                }
            }

            // Set any resolve attachments now
            for (attachment_idx, _attachment) in subpass
                .resolve_attachments
                .attached
                .iter()
                .enumerate()
                .filter_map(|(attachment_idx, attachment)| {
                    attachment.map(|attachment| (attachment_idx, attachment))
                })
            {
                subpass_info.resolve_attachments[attachment_idx].attachment = attachment_idx as _;
                // TODO ?
            }

            subpasses.push(subpass_info);
        }

        // Add dependencies (TODO!)
        {
            dependencies.push(SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 0,
                src_stage_mask: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                dst_stage_mask: vk::PipelineStageFlags::TOP_OF_PIPE,
                src_access_mask: vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                dst_access_mask: vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                dependency_flags: vk::DependencyFlags::empty(),
            });
            dependencies.push(SubpassDependency {
                src_subpass: pass.subpasses.len() as u32 - 1,
                dst_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                dst_stage_mask: vk::PipelineStageFlags::TOP_OF_PIPE,
                src_access_mask: vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                dst_access_mask: vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                dependency_flags: vk::DependencyFlags::empty(),
            });
        }

        cache.lease(
            RenderPassInfo::new()
                .attachments(attachments)
                .dependencies(dependencies)
                .subpasses(subpasses),
        )
    }

    fn lease_scheduled_resources(
        &mut self,
        cache: &mut HashPool<P>,
        schedule: &[usize],
    ) -> Result<(), DriverError> {
        for pass_idx in schedule.iter().copied() {
            // At the time this function runs the pass will already have been optimized into a
            // larger pass made out of anything that might have been merged into it - so we
            // only care about one pass at a time here
            let pass = &mut self.graph.passes[pass_idx];

            // First, let's make this pass into a big bunch of subpasses
            pass.subpasses.insert(
                0,
                Subpass {
                    load_attachments: take(&mut pass.load_attachments),
                    resolve_attachments: take(&mut pass.resolve_attachments),
                    store_attachments: take(&mut pass.store_attachments),
                    exec_idx: 0,
                },
            );

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
                Some(Self::lease_render_pass(cache, pass)?)
            } else {
                None
            };

            self.physical_passes.push(PhysicalPass {
                _descriptor_pool: descriptor_pool, // Used above; but we must keep until done
                exec_descriptor_sets,
                render_pass,
            });
        }

        Ok(())
    }

    // Merges passes which are graphic with common-ish attachments - note that scheduled pass order
    // is final during this function and so we must merge contiguous groups of passes
    fn merge_scheduled_passes<'s>(&mut self, mut schedule: &'s mut [usize]) -> &'s mut [usize] {
        // There must be company
        if schedule.len() < 2 {
            trace!("Cannot merge");

            return schedule;
        }

        let mut passes = self.graph.passes.drain(..).map(Some).collect::<Vec<_>>();
        let mut idx = 0;

        debug!(
            "Attempting to merge {} of {} passes",
            schedule.len(),
            passes.len()
        );

        while idx < schedule.len() {
            // Find candidates
            let mut pass = passes[schedule[idx]].take().unwrap();

            let start = idx + 1;
            let mut end = start;
            while end < schedule.len() {
                let other = passes[schedule[end]].as_ref().unwrap();
                debug!(
                    "Attempting to merge [{idx}: {}] with [{end}: {}]",
                    pass.name, other.name
                );
                if Self::allow_merge_passes(&pass, other) {
                    end += 1;
                } else {
                    break;
                }
            }

            if start == end {
                debug!("Unable to merge [{idx}: {}]", pass.name);
            } else {
                trace!("Merging {} passes into [{idx}: {}]", end - start, pass.name);
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

                let first_exec_idx = pass.execs.len();
                pass.execs.append(&mut other.execs);

                pass.subpasses.push(Subpass {
                    load_attachments: other.load_attachments,
                    resolve_attachments: other.resolve_attachments,
                    store_attachments: other.store_attachments,
                    exec_idx: first_exec_idx,
                });
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

    fn next_subpass(cmd_buf: &CommandBuffer<P>) {
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
    pub fn node_stage_mask(&self, node: impl Node<P>) -> vk::PipelineStageFlags {
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
        cmd_buf: &CommandBuffer<P>,
        bindings: &mut [Binding<P>],
        pass: &mut Pass<P>,
        exec_idx: usize,
    ) {
        use std::slice::from_ref;

        let accesses = pass.execs[exec_idx].accesses.iter();

        // trace!("record_execution_barriers: {:#?}", accesses);

        for (node_idx, accesses) in accesses {
            let next_access = accesses[0].access;
            let next_subresource = &accesses[0].subresource;
            let binding = &mut bindings[*node_idx];
            let previous_access = binding.access(accesses[1].access);

            let mut global_barrier = None;
            let mut buf_barrier = None;
            let mut image_barrier = None;

            if let Some(subresource) = &next_subresource {
                if let Some(buf) = binding.as_driver_buffer() {
                    let subresource = subresource.unwrap_buffer();
                    buf_barrier = Some(BufferBarrier {
                        previous_accesses: from_ref(&previous_access),
                        next_accesses: from_ref(&next_access),
                        src_queue_family_index: cmd_buf.device.queue.family.idx,
                        dst_queue_family_index: cmd_buf.device.queue.family.idx,
                        buffer: **buf,
                        offset: subresource.start as _,
                        size: (subresource.end - subresource.start) as _,
                    });
                } else if let Some(image) = binding.as_driver_image() {
                    image_barrier = Some(ImageBarrier {
                        previous_accesses: from_ref(&previous_access),
                        next_accesses: from_ref(&next_access),
                        previous_layout: image_access_layout(previous_access),
                        next_layout: image_access_layout(next_access),
                        discard_contents: previous_access == AccessType::Nothing
                            || is_write_access(next_access),
                        src_queue_family_index: cmd_buf.device.queue.family.idx,
                        dst_queue_family_index: cmd_buf.device.queue.family.idx,
                        image: **image,
                        range: subresource.unwrap_image().into_vk(),
                    });
                }
            }

            if let Some(barrier) = &buf_barrier {
                trace!(
                    "buffer barrier {:?} {}..{}",
                    barrier.buffer,
                    barrier.offset,
                    barrier.offset + barrier.size
                );
            } else if let Some(barrier) = &image_barrier {
                trace!(
                    "image barrier {:?} {:?} -> {:?} (layout {:?} -> {:?})",
                    barrier.image,
                    barrier.previous_accesses[0],
                    barrier.next_accesses[0],
                    barrier.previous_layout,
                    barrier.next_layout,
                );
            } else {
                trace!("global barrier {:?} -> {:?}", previous_access, next_access);
                global_barrier = Some(GlobalBarrier {
                    previous_accesses: from_ref(&previous_access),
                    next_accesses: from_ref(&next_access),
                });
            }

            pipeline_barrier(
                &cmd_buf.device,
                **cmd_buf,
                global_barrier,
                buf_barrier.as_ref().map(from_ref).unwrap_or_default(),
                image_barrier.as_ref().map(from_ref).unwrap_or_default(),
            );
        }
    }

    /// Records any pending render graph passes that are required by the given node, but does not
    /// record any passes that actually contain the given node.
    ///
    /// As a side effect, the graph is optimized for the given node. Future calls may further optimize
    /// the graph, but only on top of the existing optimizations. This only matters if you are pulling
    /// multiple images out and you care - in that case pull the "most important" image first.
    pub fn record_node_dependencies(
        &mut self,
        cache: &mut HashPool<P>,
        cmd_buf: &mut CommandBuffer<P>,
        node: impl Node<P>,
    ) -> Result<(), DriverError>
    where
        P: 'static,
    {
        let node_idx = node.index();

        assert!(self.graph.bindings.get(node_idx).is_some());

        // We record up to but not including the first pass which accesses the target node
        let end_pass_idx = self
            .graph
            .first_node_access_pass_index(node)
            .unwrap_or_default()
            .min(self.graph.passes.len());

        self.record_node_passes(cache, cmd_buf, node_idx, end_pass_idx)
    }

    /// Records any pending render graph passes that the given node requires.
    pub fn record_node(
        &mut self,
        cache: &mut HashPool<P>,
        cmd_buf: &mut CommandBuffer<P>,
        node: impl Node<P>,
    ) -> Result<(), DriverError>
    where
        P: 'static,
    {
        let node_idx = node.index();

        assert!(self.graph.bindings.get(node_idx).is_some());

        self.record_node_passes(cache, cmd_buf, node_idx, self.graph.passes.len())
    }

    fn record_node_passes(
        &mut self,
        cache: &mut HashPool<P>,
        cmd_buf: &mut CommandBuffer<P>,
        node_idx: usize,
        end_pass_idx: usize,
    ) -> Result<(), DriverError> {
        //trace!("record_passes: end_pass_idx = {}", end_pass_idx);

        // Build a schedule for this node
        let mut schedule = self.schedule_node_passes(node_idx, end_pass_idx);

        self.record_scheduled_passes(cache, cmd_buf, &mut schedule, end_pass_idx)
    }

    fn record_scheduled_passes(
        &mut self,
        cache: &mut HashPool<P>,
        cmd_buf: &mut CommandBuffer<P>,
        mut schedule: &mut [usize],
        end_pass_idx: usize,
    ) -> Result<(), DriverError> {
        // Print some handy details or hit a breakpoint if you set the flag
        #[cfg(debug_assertions)]
        if self.graph.debug {
            log::info!("Input {:#?}", self.graph);
        }

        // Optimize the schedule; leasing the required stuff it needs
        self.reorder_scheduled_passes(schedule, end_pass_idx);
        schedule = self.merge_scheduled_passes(schedule);
        self.lease_scheduled_resources(cache, schedule)?;

        let mut passes = take(&mut self.graph.passes);
        for (physical_pass_idx, pass_idx) in schedule.iter().copied().enumerate() {
            let pass = &mut passes[pass_idx];
            let is_graphic = self.physical_passes[physical_pass_idx]
                .render_pass
                .is_some();

            trace!("record_passes: begin \"{}\"", &pass.name);

            if !self.physical_passes[physical_pass_idx]
                .exec_descriptor_sets
                .is_empty()
            {
                self.write_descriptor_sets(cmd_buf, pass, physical_pass_idx)?;
            }

            Self::record_execution_barriers(cmd_buf, &mut self.graph.bindings, pass, 0);

            let render_area = if is_graphic {
                let render_area = self.render_area(pass);
                self.begin_render_pass(cmd_buf, pass, physical_pass_idx, render_area)?;
                Some(render_area)
            } else {
                None
            };

            for subpass_idx in 0..pass.subpasses.len() {
                let exec_idx = pass.subpasses[subpass_idx].exec_idx;

                if is_graphic && subpass_idx > 0 {
                    Self::next_subpass(cmd_buf);
                }

                {
                    let exec = &mut pass.execs[exec_idx];
                    if let Some(pipeline) = exec.pipeline.as_mut() {
                        self.bind_pipeline(
                            cmd_buf,
                            physical_pass_idx,
                            subpass_idx as u32,
                            pipeline,
                            pass.depth_stencil,
                        )?;

                        if is_graphic {
                            if pass.viewport.is_none() {
                                // Viewport was not set by user
                                let render_area_extent = render_area.unwrap().extent.as_vec2();
                                Self::set_viewport(
                                    cmd_buf,
                                    render_area_extent.x,
                                    render_area_extent.y,
                                    pass.depth_stencil
                                        .map(|depth_stencil| {
                                            let min = depth_stencil.min.0;
                                            let max = depth_stencil.max.0;
                                            min..max
                                        })
                                        .unwrap_or(0.0..1.0),
                                );
                            }

                            if pass.scissor.is_none() {
                                // Scissor region was not set by user
                                let render_area = render_area.unwrap();
                                Self::set_scissor(
                                    cmd_buf,
                                    render_area.extent.x,
                                    render_area.extent.y,
                                )
                            }
                        }

                        self.bind_descriptor_sets(
                            cmd_buf,
                            pipeline,
                            &self.physical_passes[physical_pass_idx],
                            exec_idx,
                        );
                    }
                };

                if exec_idx > 0 {
                    Self::record_execution_barriers(
                        cmd_buf,
                        &mut self.graph.bindings,
                        pass,
                        exec_idx,
                    );
                }

                {
                    // debug!("execute");

                    let exec = &mut pass.execs[exec_idx];
                    let exec_func = exec.func.take().unwrap().0;
                    let bindings = Bindings {
                        exec,
                        graph: &self.graph,
                    };
                    exec_func(&cmd_buf.device, **cmd_buf, bindings);
                }
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

        Ok(())
    }

    /// Records any pending render graph passes that have not been previously scheduled.
    pub fn record_unscheduled_passes(
        &mut self,
        cache: &mut HashPool<P>,
        cmd_buf: &mut CommandBuffer<P>,
    ) -> Result<(), DriverError>
    where
        P: 'static,
    {
        let mut schedule = (0..self.graph.passes.len()).collect::<Vec<_>>();

        self.record_scheduled_passes(cache, cmd_buf, &mut schedule, self.graph.passes.len())
    }

    fn render_area(&self, pass: &Pass<P>) -> Rect<UVec2, IVec2> {
        pass.render_area.unwrap_or_else(|| {
            // set_render_area was not specified so we're going to guess using the extent
            // of the first attachment we find, by lowest attachment index order
            let first_pass = &pass.subpasses[0];
            let extent = first_pass
                .load_attachments
                .attached
                .iter()
                .chain(first_pass.resolve_attachments.attached.iter())
                .chain(first_pass.store_attachments.attached.iter())
                .filter_map(|attachment| attachment.as_ref())
                .find_map(|attachment| self.graph.bindings[attachment.target].as_extent_2d())
                .unwrap();

            Rect {
                extent,
                offset: IVec2::ZERO,
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

        // Schedule the first set of passes for the node we're trying to resolve
        for pass_idx in self.dependent_passes(node_idx, self.graph.passes.len()) {
            if pass_idx < end_pass_idx {
                schedule.push(pass_idx);
            }

            unscheduled.remove(&pass_idx);
            for node_idx in self.dependent_nodes(pass_idx) {
                unresolved.push_back((node_idx, pass_idx));
            }
        }

        // Now schedule all nodes that are required, going through the tree to find them
        while let Some((node_idx, max_pass_idx)) = unresolved.pop_front() {
            for pass_idx in self.dependent_passes(node_idx, max_pass_idx) {
                if unscheduled.remove(&pass_idx) {
                    schedule.push(pass_idx);
                    for node_idx in self.dependent_nodes(pass_idx) {
                        unresolved.push_back((node_idx, pass_idx));
                    }
                }
            }
        }

        schedule.reverse();

        debug!(
            "Schedule: {}",
            schedule
                .iter()
                .copied()
                .map(|idx| format!("{} {}", idx, self.graph.passes[idx].name))
                .join(", ")
        );
        trace!(
            "Skipping: {}",
            unscheduled
                .iter()
                .copied()
                .map(|idx| format!("{} {}", idx, self.graph.passes[idx].name))
                .join(", ")
        );

        schedule
    }

    fn set_scissor(cmd_buf: &CommandBuffer<P>, width: u32, height: u32) {
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

    fn set_viewport(cmd_buf: &CommandBuffer<P>, width: f32, height: f32, depth: Range<f32>) {
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

    pub fn submit(mut self, cache: &mut HashPool<P>) -> Result<(), DriverError>
    where
        P: 'static,
    {
        use std::slice::from_ref;

        trace!("submit");

        let mut cmd_buf = cache.lease(cache.device.queue.family)?;

        unsafe {
            Device::wait_for_fence(&cache.device, &cmd_buf.fence)
                .map_err(|_| DriverError::OutOfMemory)?;

            cache
                .device
                .reset_command_pool(cmd_buf.pool, vk::CommandPoolResetFlags::RELEASE_RESOURCES)
                .map_err(|_| DriverError::OutOfMemory)?;
            cache
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
            cache
                .device
                .end_command_buffer(**cmd_buf)
                .map_err(|_| DriverError::OutOfMemory)?;
            cache
                .device
                .reset_fences(from_ref(&cmd_buf.fence))
                .map_err(|_| DriverError::OutOfMemory)?;
            cache
                .device
                .queue_submit(
                    *cache.device.queue,
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
        cmd_buf: &CommandBuffer<P>,
        pass: &Pass<P>,
        physical_pass_idx: usize,
    ) -> Result<(), DriverError> {
        use std::slice::from_ref;

        let physical_pass = &self.physical_passes[physical_pass_idx];
        let mut descriptor_writes = vec![];
        let mut buffer_infos = vec![];
        let mut image_infos = vec![];
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
            for (descriptor, (node_idx, view_info)) in exec.bindings.iter() {
                let (descriptor_set_idx, binding_idx, binding_offset) = descriptor.into_tuple();
                let descriptor_info = *pipeline
                    .descriptor_bindings()
                    .get(&DescriptorBinding(descriptor_set_idx, binding_idx))
                    .unwrap_or_else(|| panic!("Descriptor {descriptor_set_idx}.{binding_idx}[{binding_offset}] specified in recorded execution of pass \"{}\" was not discovered through shader reflection.", &pass.name));
                let descriptor_ty = descriptor_info.into();

                //trace!("write_descriptor_sets {descriptor_set_idx}.{binding_idx}[{binding_offset}] = {:?}", descriptor_info);

                let write_descriptor_set = vk::WriteDescriptorSet::builder()
                    .dst_set(
                        *physical_pass.exec_descriptor_sets[&exec_idx][descriptor_set_idx as usize],
                    )
                    .dst_binding(binding_idx)
                    .dst_array_element(binding_offset);
                let bound_node = &self.graph.bindings[*node_idx];
                if let Some(image) = bound_node.as_driver_image() {
                    if let Some(view_info) = view_info {
                        let mut image_view_info = *view_info.as_image().unwrap();

                        // Handle default views which did not specify a particaular aspect
                        if image_view_info.aspect_mask.is_empty() {
                            image_view_info.aspect_mask = format_aspect_mask(image.info.fmt);
                        }

                        let sampler = descriptor_info.sampler().unwrap_or_else(vk::Sampler::null);
                        let image_view = Image::view_ref(image, image_view_info)?;
                        let image_layout = match descriptor_ty {
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

                        // trace!(
                        //     "{}.{}[{}] layout={:?} view={:?} sampler={:?}",
                        //     descriptor_set_idx,
                        //     binding_idx,
                        //     binding_offset,
                        //     image_layout,
                        //     image_view,
                        //     sampler
                        // );

                        image_infos.push(vk::DescriptorImageInfo {
                            image_layout,
                            image_view,
                            sampler,
                        });

                        descriptor_writes.push(
                            write_descriptor_set
                                .descriptor_type(descriptor_ty)
                                .image_info(from_ref(image_infos.last().unwrap()))
                                .build(),
                        );
                    } else {
                        // Coming very soon!
                        unimplemented!();
                    }
                } else if let Some(buffer) = bound_node.as_driver_buffer() {
                    if let Some(view_info) = view_info {
                        let buffer_view_info = view_info.as_buffer().unwrap();

                        // trace!("BVI: {}..{}", buffer_view_info.start, buffer_view_info.end);

                        buffer_infos.push(vk::DescriptorBufferInfo {
                            buffer: **buffer,
                            offset: buffer_view_info.start,
                            range: buffer_view_info.end - buffer_view_info.start,
                        });

                        descriptor_writes.push(
                            write_descriptor_set
                                .descriptor_type(descriptor_ty)
                                .buffer_info(from_ref(buffer_infos.last().unwrap()))
                                .build(),
                        );
                    } else {
                        // Coming very soon!
                        unimplemented!();
                    }
                } else {
                    // Coming very soon!
                    unimplemented!();
                }
            }
        }

        if descriptor_writes.is_empty() {
            return Ok(());
        }

        trace!("writing {:#?} descriptors ", descriptor_writes.len());

        unsafe {
            cmd_buf
                .device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[])
        }

        Ok(())
    }
}
