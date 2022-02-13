use {
    super::{copy_buffer_binding_to_image, copy_image_binding},
    anyhow::Context,
    screen_13::prelude_all::*,
};

#[derive(Debug)]
pub struct ModelLoader<P>
where
    P: SharedPointerKind,
{
    decode_model_u16_static: ComputePipeline<P>,
    device: Shared<Device<P>, P>,
    pool: HashPool<P>,
}

impl<P> ModelLoader<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self {
            decode_model_u16_static: ComputePipeline::create(
                device,
                ComputePipelineInfo::new(crate::res::shader::COMPUTE_DECODE_MODEL_U16_STATIC_COMP),
            )?,
            device: Shared::clone(device),
            pool: HashPool::new(device),
        })
    }

    fn create_image(
        &self,
        bitmap: &BitmapBuf,
        is_srgb: bool,
        is_temporary: bool,
    ) -> anyhow::Result<ImageBinding<P>> {
        Ok(ImageBinding::new(
            Image::create(
                &self.device,
                ImageInfo {
                    ty: ImageType::Texture2D,
                    usage: if is_temporary {
                        vk::ImageUsageFlags::STORAGE
                            | vk::ImageUsageFlags::TRANSFER_DST
                            | vk::ImageUsageFlags::TRANSFER_SRC
                    } else {
                        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST
                    },
                    flags: vk::ImageCreateFlags::MUTABLE_FORMAT,
                    fmt: match bitmap.format() {
                        BitmapFormat::R | BitmapFormat::Rg => {
                            if is_temporary {
                                vk::Format::R8G8_UINT
                            } else if is_srgb {
                                panic!("Unsupported format: R8G8_SRGB");
                            } else {
                                vk::Format::R8G8_UNORM
                            }
                        }
                        BitmapFormat::Rgb | BitmapFormat::Rgba => {
                            if is_temporary {
                                vk::Format::R8G8B8A8_UINT
                            } else if is_srgb {
                                vk::Format::R8G8B8A8_SRGB
                            } else {
                                vk::Format::R8G8B8A8_UNORM
                            }
                        }
                    },
                    extent: uvec3(bitmap.width(), bitmap.height(), 1),
                    tiling: vk::ImageTiling::OPTIMAL,
                    mip_level_count: 1,
                    array_elements: 1,
                    sample_count: SampleCount::X1,
                },
            )
            .context("Unable to create new image")?,
        ))
    }

    pub fn decode_model(
        &mut self,
        model: &ModelBuf,
        idx_buf: &mut BufferBinding<P>,
        idx_offset: usize,
        vertex_buf: &mut BufferBinding<P>,
        vertex_offset: usize,
    ) -> anyhow::Result<()>
    where
        P: SharedPointerKind + 'static,
    {
        use std::slice::from_ref;

        let buf_len = model.indices().len() + model.vertices().len();

        trace!(
            "Decoding model ({} K)",
            buf_len / 1024
        );

        let cmd_buf = self.pool.lease(self.device.queue.family)?;

        // Lease a temporary buffer from the pool
        let mut buf_binding = self.pool.lease(BufferInfo {
            size: buf_len as _,
            usage: vk::BufferUsageFlags::STORAGE_BUFFER,
            can_map: true,
        })?;

        // {
        //     let buf = &mut Buffer::mapped_slice_mut(&mut buf_binding)[0..buf_len];
        //     let indices = model.indices();
        //     let vertices = model.vertices();

        //     {
        //         let dst = &mut buf[0..indices.len()];
        //         dst.copy_from_slice(indices);
        //     }

        //     {
        //         let dst = &mut buf[indices.len()..indices.len() + vertices.len()];
        //         dst.copy_from_slice(vertices);
        //     }
        // }

        // if model.is_animated() {
        //     todo!()
        // } else {
        //     //info!("");

        //     Self::dispatch_compute_pipeline(
        //         cmd_buf,
        //         &mut self.pool,
        //         &self.decode_model_u16_static,
        //         buf_binding,
        //         &mut idx_buf,
        //         idx_offset,
        //         &mut vertex_buf,
        //         vertex_offset,
        //     )?
        // }
        // .submit()?;

        Ok(())
    }

    fn dispatch_compute_pipeline<Ch, Cb>(
        cmd_chain: Ch,
        pool: &mut HashPool<P>,
        pipeline: &ComputePipeline<P>,
        mut buf_binding: Lease<BufferBinding<P>, P>,
        idx_buf_binding: &mut BufferBinding<P>,
        idx_offset: usize,
        vertex_buf_binding: &mut BufferBinding<P>,
        vertex_offset: usize,
    ) -> Result<CommandChain<Cb, P>, anyhow::Error>
    where
        Ch: Into<CommandChain<Cb, P>>,
        Cb: AsRef<CommandBuffer<P>>,
        P: 'static,
    {
        use std::slice::from_ref;

        // Raw vulkan pipeline handles
        let descriptor_set_layout = &pipeline.descriptor_info.layouts[&0];
        let pipeline_layout = pipeline.layout;
        let pipeline = **pipeline;

        // Raw vulkan buffer handle
        let (buf, previous_buf_access, _) =
            buf_binding.access_inner(AccessType::ComputeShaderReadOther);
        let buf = **buf;

        // Raw vulkan buffer handle
        let (idx_buf, previous_idx_buf_access, _) =
        idx_buf_binding.access_inner(AccessType::ComputeShaderWrite);
        let idx_buf = **idx_buf;

        // Raw vulkan image/view handles
        let (vertex_buf, previous_vertex_buf_access, _) =
        vertex_buf_binding.access_inner(AccessType::ComputeShaderWrite);
        let vertex_buf = **vertex_buf;

        // Allocate a single descriptor set from the pool (This set is exclusive for this dispatch)
        let descriptor_pool = pool.lease(DescriptorPoolInfo::new(1).pool_sizes(vec![
            DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 3,
            },
        ]))?;
        let descriptor_set_ref =
            DescriptorPool::allocate_descriptor_set(&descriptor_pool, descriptor_set_layout)?;
        let descriptor_set = *descriptor_set_ref;

        // Write the descriptors for our pixel buffer source and image destination
        unsafe {
            descriptor_pool.device.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(from_ref(&vk::DescriptorBufferInfo {
                            buffer: buf,
                            offset: 0,
                            range: vk::WHOLE_SIZE,
                        }))
                        .build(),
                    vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(from_ref(&vk::DescriptorBufferInfo {
                            buffer: buf,
                            offset: 0,
                            range: vk::WHOLE_SIZE,
                        }))
                        .build(),
                    vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(from_ref(&vk::DescriptorBufferInfo {
                            buffer: buf,
                            offset: 0,
                            range: vk::WHOLE_SIZE,
                        }))
                        .build(),
                ],
                &[],
            )
        }

        Ok(cmd_chain
            .into()
            .push_shared_ref(descriptor_pool)
            .push_shared_ref(descriptor_set_ref)
            .push_shared_ref(buf_binding)
            .push_shared_ref(idx_buf_binding.shared_ref())
            .push_shared_ref(vertex_buf_binding.shared_ref())
            .push_execute(move |device, cmd_buf| unsafe {
                CommandBuffer::buffer_barrier(
                    cmd_buf,
                    previous_buf_access,
                    AccessType::ComputeShaderReadOther,
                    buf,
                    None,
                );
                CommandBuffer::buffer_barrier(
                    cmd_buf,
                    previous_idx_buf_access,
                    AccessType::ComputeShaderReadOther,
                    idx_buf,
                    None,
                );
                CommandBuffer::buffer_barrier(
                    cmd_buf,
                    previous_vertex_buf_access,
                    AccessType::ComputeShaderReadOther,
                    vertex_buf,
                    None,
                );

                device.cmd_bind_pipeline(**cmd_buf, vk::PipelineBindPoint::COMPUTE, pipeline);
                device.cmd_bind_descriptor_sets(
                    **cmd_buf,
                    vk::PipelineBindPoint::COMPUTE,
                    pipeline_layout,
                    0,
                    from_ref(&descriptor_set),
                    &[],
                );

                // if let Some(data) = push_constants {
                //     device.cmd_push_constants(
                //         **cmd_buf,
                //         pipeline_layout,
                //         vk::ShaderStageFlags::COMPUTE,
                //         0,
                //         as_u8_slice(&data),
                //     );
                // }

                device.cmd_dispatch(**cmd_buf, 1, 1, 1);
            }))
    }
}
