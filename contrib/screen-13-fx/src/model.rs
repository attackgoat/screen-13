use screen_13::prelude_all::*;

#[derive(Debug)]
pub struct ModelLoader<P>
where
    P: SharedPointerKind,
{
    decode_model_u16: ComputePipeline<P>,
    decode_model_u32: ComputePipeline<P>,
    device: Shared<Device<P>, P>,
    pool: HashPool<P>,
}

impl<P> ModelLoader<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self {
            decode_model_u16: ComputePipeline::create(
                device,
                ComputePipelineInfo::new(crate::res::shader::COMPUTE_DECODE_MODEL_U16_COMP),
            )?,
            decode_model_u32: ComputePipeline::create(
                device,
                ComputePipelineInfo::new(crate::res::shader::COMPUTE_DECODE_MODEL_U32_COMP),
            )?,
            device: Shared::clone(device),
            pool: HashPool::new(device),
        })
    }

    pub fn decode_model(
        &mut self,
        model: &ModelBuf,
        idx_buf_binding: &mut BufferBinding<P>,
        idx_offset: &mut u64,
        vertex_buf_binding: &mut BufferBinding<P>,
        vertex_offset: &mut u64,
    ) -> anyhow::Result<()>
    where
        P: SharedPointerKind + 'static,
    {
        // use std::slice::from_ref;

        // let indices = model.indices();
        // let indices_len = indices.len();
        // let vertices = model.vertices();
        // let src_buf_len = indices.len() + vertices.len();

        // trace!("Decoding model ({} K)", src_buf_len / 1024);

        // let mut src_buf_binding = self.pool.lease(BufferInfo {
        //     size: src_buf_len as _,
        //     usage: vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_SRC,
        //     can_map: true,
        // })?;

        // {
        //     let src_buf =
        //         &mut Buffer::mapped_slice_mut(src_buf_binding.get_mut().unwrap())[0..src_buf_len];

        //     {
        //         let src_buf = &mut src_buf[0..indices_len];
        //         src_buf.copy_from_slice(indices);
        //     }

        //     {
        //         let src_buf = &mut src_buf[indices_len..indices_len + vertices.len()];
        //         src_buf.copy_from_slice(vertices);
        //     }
        // }

        // let descriptor_pool = self.pool.lease(
        //     DescriptorPoolInfo::new(model.meshes.len() as _).pool_sizes(vec![DescriptorPoolSize {
        //         ty: vk::DescriptorType::STORAGE_BUFFER,
        //         descriptor_count: 3,
        //     }]),
        // )?;

        // let mut cmd_chain = CommandChain::new(self.pool.lease(self.device.queue.family)?);

        // for mesh in &model.meshes {
        //     let (pipeline, idx_count) = if mesh.index_ty == IndexType::U16 {
        //         (&self.decode_model_u16, indices.len() >> 1)
        //     } else {
        //         (&self.decode_model_u32, indices.len() >> 2)
        //     };
        //     let tri_count = idx_count as u32 / 3;
        //     let vertex_write_len = idx_count as u64 * 52;

        //     // Raw vulkan pipeline handles
        //     let descriptor_set_layout = &pipeline.descriptor_info.layouts[&0];
        //     let pipeline_layout = pipeline.layout;
        //     let pipeline = **pipeline;

        //     // Raw vulkan buffer handle
        //     let (src_buf, previous_src_buf_access) =
        //         src_buf_binding.access(AccessType::TransferRead);
        //     let src_buf = **src_buf;

        //     // Raw vulkan image/view handles
        //     let (idx_buf, previous_idx_buf_access) =
        //         idx_buf_binding.access(AccessType::TransferWrite);
        //     let idx_buf = **idx_buf;

        //     // Raw vulkan image/view handles
        //     let (vertex_buf, previous_vertex_buf_access) =
        //         vertex_buf_binding.access(AccessType::ComputeShaderWrite);
        //     let vertex_buf = **vertex_buf;

        //     let descriptor_set_ref =
        //         DescriptorPool::allocate_descriptor_set(&descriptor_pool, descriptor_set_layout)?;
        //     let descriptor_set = *descriptor_set_ref;

        //     unsafe {
        //         descriptor_pool.device.update_descriptor_sets(
        //             &[
        //                 vk::WriteDescriptorSet::builder()
        //                     .dst_set(descriptor_set)
        //                     .dst_binding(0)
        //                     .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        //                     .buffer_info(from_ref(&vk::DescriptorBufferInfo {
        //                         buffer: src_buf,
        //                         offset: 0,
        //                         range: indices_len as _,
        //                     }))
        //                     .build(),
        //                 vk::WriteDescriptorSet::builder()
        //                     .dst_set(descriptor_set)
        //                     .dst_binding(1)
        //                     .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        //                     .buffer_info(from_ref(&vk::DescriptorBufferInfo {
        //                         buffer: src_buf,
        //                         offset: indices_len as _,
        //                         range: vertices.len() as _,
        //                     }))
        //                     .build(),
        //                 vk::WriteDescriptorSet::builder()
        //                     .dst_set(descriptor_set)
        //                     .dst_binding(2)
        //                     .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        //                     .buffer_info(from_ref(&vk::DescriptorBufferInfo {
        //                         buffer: vertex_buf,
        //                         offset: *vertex_offset,
        //                         range: vertex_write_len,
        //                     }))
        //                     .build(),
        //             ],
        //             &[],
        //         )
        //     }

        //     {
        //         let idx_offset = *idx_offset;
        //         let vertex_offset = *vertex_offset;
        //         cmd_chain = cmd_chain.push_shared_ref(descriptor_set_ref).push_execute(
        //             move |device, cmd_buf| unsafe {
        //                 CommandBuffer::buffer_barrier(
        //                     cmd_buf,
        //                     previous_src_buf_access,
        //                     AccessType::ComputeShaderReadOther,
        //                     src_buf,
        //                     Some(0..src_buf_len as _),
        //                 );
        //                 CommandBuffer::buffer_barrier(
        //                     cmd_buf,
        //                     previous_vertex_buf_access,
        //                     AccessType::ComputeShaderReadOther,
        //                     vertex_buf,
        //                     Some(vertex_offset..vertex_offset + vertex_write_len),
        //                 );

        //                 device.cmd_bind_pipeline(
        //                     **cmd_buf,
        //                     vk::PipelineBindPoint::COMPUTE,
        //                     pipeline,
        //                 );
        //                 device.cmd_bind_descriptor_sets(
        //                     **cmd_buf,
        //                     vk::PipelineBindPoint::COMPUTE,
        //                     pipeline_layout,
        //                     0,
        //                     from_ref(&descriptor_set),
        //                     &[],
        //                 );
        //                 device.cmd_dispatch(**cmd_buf, tri_count, 1, 1);

        //                 CommandBuffer::buffer_barrier(
        //                     cmd_buf,
        //                     AccessType::ComputeShaderReadOther,
        //                     AccessType::TransferRead,
        //                     src_buf,
        //                     Some(0..indices_len as _),
        //                 );
        //                 CommandBuffer::buffer_barrier(
        //                     cmd_buf,
        //                     previous_idx_buf_access,
        //                     AccessType::TransferWrite,
        //                     src_buf,
        //                     Some(idx_offset..indices_len as _),
        //                 );
        //                 device.cmd_copy_buffer(
        //                     **cmd_buf,
        //                     src_buf,
        //                     idx_buf,
        //                     from_ref(&vk::BufferCopy {
        //                         src_offset: 0,
        //                         dst_offset: 0,
        //                         size: 0,
        //                     }),
        //                 );
        //             },
        //         );
        //     }

        //     *idx_offset += indices_len as u64;
        //     *vertex_offset += vertex_write_len;
        // }

        // cmd_chain
        //     .push_shared_ref(src_buf_binding)
        //     .push_shared_ref(idx_buf_binding.shared_ref())
        //     .push_shared_ref(vertex_buf_binding.shared_ref())
        //     .push_shared_ref(descriptor_pool)
        //     .submit()?;

        Ok(())
    }
}
