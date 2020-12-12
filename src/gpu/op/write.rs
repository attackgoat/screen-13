use {
    super::Op,
    crate::{
        color::TRANSPARENT_BLACK,
        gpu::{
            driver::{
                bind_graphics_descriptor_set, CommandPool, Device, Driver, Fence, Framebuffer2d,
                PhysicalDevice,
            },
            pool::{ColorRenderPassMode, Graphics, GraphicsMode, Lease, Pool, RenderPassMode},
            BlendMode, Texture2d,
        },
        math::{vec3, Area, CoordF, Mat4, RectF, Vec2},
    },
    gfx_hal::{
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        device::Device as _,
        format::Aspects,
        image::{Access, Layout, Offset, SubresourceLayers, Usage},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        iter::{empty, once},
        u8,
    },
};

#[derive(Clone, Copy, Hash, PartialEq)]
pub enum Mode {
    Blend((u8, BlendMode)),
    Texture,
}

#[repr(C)]
struct VertexConsts {
    offset: Vec2,
    scale: Vec2,
    transform: Mat4,
}

impl AsRef<[u32; 20]> for VertexConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 20] {
        unsafe { &*(self as *const Self as *const [u32; 20]) }
    }
}

/// An expressive type which allows specification of individual texture writes. Texture writes may either specify the
/// entire source texture or a tile sub-portion. Tiles are always specified using integer texel coordinates.
pub struct Write<'s> {
    src: &'s Texture2d,
    src_region: Area,
    transform: Mat4,
}

// TODO: Add multi-sampled builder function
impl<'s> Write<'s> {
    /// Writes the whole source texture to the destination at the given position.
    pub fn position<D: Into<CoordF>>(src: &'s Texture2d, dst: D) -> Self {
        Self::tile_position(src, src.borrow().dims().into(), dst)
    }

    /// Writes the whole source texture to the destination at the given rectangle.
    pub fn region<D: Into<RectF>>(src: &'s Texture2d, dst: D) -> Self {
        Self::tile_region(src, src.borrow().dims().into(), dst)
    }

    /// Writes a tile area of the source texture to the destination at the given position.
    pub fn tile_position<D: Into<CoordF>>(src: &'s Texture2d, src_tile: Area, dst: D) -> Self {
        Self::tile_region(
            src,
            src_tile,
            RectF {
                dims: src.borrow().dims().into(),
                pos: dst.into(),
            },
        )
    }

    /// Writes a tile area of the source texture to the destination at the given rectangle.
    pub fn tile_region<D: Into<RectF>>(src: &'s Texture2d, src_tile: Area, dst: D) -> Self {
        let dst = dst.into();
        let src_dims: CoordF = src.borrow().dims().into();
        let dst_transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
            * Mat4::from_scale(vec3(
                dst.dims.x * 2.0 / src_dims.x,
                dst.dims.y * 2.0 / src_dims.y,
                1.0,
            ))
            * Mat4::from_translation(vec3(dst.pos.x / dst.dims.x, dst.pos.y / dst.dims.y, 0.0));

        Self::tile_transform(src, src_tile, dst_transform)
    }

    /// Writes a tile area of the source texture to the destination using the given transformation matrix.
    pub fn tile_transform(src: &'s Texture2d, src_tile: Area, dst: Mat4) -> Self {
        Self {
            src,
            src_region: src_tile,
            transform: dst,
        }
    }

    /// Writes the whole source texture to the destination using the given transformation matrix.
    pub fn transform(src: &'s Texture2d, dst: Mat4) -> Self {
        Self::tile_transform(src, src.borrow().dims().into(), dst)
    }
}

pub struct WriteOp<'a> {
    back_buf: Lease<Texture2d>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: Texture2d,
    dst_preserve: bool,
    fence: Lease<Fence>,
    frame_buf: Option<Framebuffer2d>,
    graphics: Option<Lease<Graphics>>,
    mode: Mode,

    #[cfg(debug_assertions)]
    name: String,

    pool: &'a mut Pool,
    src_textures: Vec<Texture2d>,
}

impl<'a> WriteOp<'a> {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        pool: &'a mut Pool,
        dst: Texture2d,
        mode: Mode,
    ) -> Self {
        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(&driver, family);
        let (dims, fmt) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };
        let fence = pool.fence(
            #[cfg(debug_assertions)]
            name,
            &driver,
        );

        Self {
            back_buf: pool.texture(
                #[cfg(debug_assertions)]
                &format!("{} backbuffer", name),
                &driver,
                dims,
                fmt,
                Layout::Undefined,
                Usage::COLOR_ATTACHMENT
                    | Usage::INPUT_ATTACHMENT
                    | Usage::TRANSFER_DST
                    | Usage::TRANSFER_SRC,
                1,
                1,
                1,
            ),
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            driver,
            dst,
            dst_preserve: false,
            fence,
            frame_buf: None,
            graphics: None,
            mode,
            #[cfg(debug_assertions)]
            name: name.to_owned(),
            pool,
            src_textures: Default::default(),
        }
    }

    /// Preserves the contents of the destination texture. Without calling this function the existing
    /// contents of the destination texture will not be composited into the final result.
    pub fn with_preserve(&mut self) -> &mut Self {
        self.dst_preserve = true;
        self
    }

    pub fn record(mut self, writes: &mut [Write]) -> impl Op {
        assert!(self.src_textures.is_empty());
        assert_ne!(writes.len(), 0);

        if writes.len() > 1 {
            // Keeps track of the textures used while the GPU is still busy (so our caller can drop their references)
            for write in writes.iter() {
                let write_src_ptr = Texture2d::as_ptr(&write.src);
                if let Err(idx) = self.src_textures.binary_search_by(|probe| {
                    let probe = Texture2d::as_ptr(probe);
                    probe.cmp(&write_src_ptr)
                }) {
                    self.src_textures.insert(idx, Texture2d::clone(write.src));
                }
            }

            // Sort the writes by texture so that we minimize the number of descriptor sets and how often we change sets during submit
            // NOTE: Unstable sort because we don't claim to support ordering or blending of the individual writes within each batch
            writes.sort_unstable_by(|lhs, rhs| {
                let lhs = Texture2d::as_ptr(&lhs.src);
                let rhs = Texture2d::as_ptr(&rhs.src);
                lhs.cmp(&rhs)
            });
        } else {
            // We only have one write - and the above sort logic would not be called (there would be no right-hand-side!)
            self.src_textures.push(Texture2d::clone(writes[0].src));
        }

        let render_pass_mode = RenderPassMode::Color(ColorRenderPassMode {
            format: self.dst.borrow().format(),
            preserve: self.dst_preserve,
        });

        // Final setup bits
        {
            // Setup the framebuffer
            self.frame_buf.replace(Framebuffer2d::new(
                #[cfg(debug_assertions)]
                self.name.as_str(),
                Driver::clone(&self.driver),
                self.pool.render_pass(&self.driver, render_pass_mode),
                once(self.back_buf.borrow().as_default_view().as_ref()),
                self.dst.borrow().dims(),
            ));

            // Setup the graphics pipeline(s) using one descriptor set per unique source texture
            let graphics_mode = match self.mode {
                Mode::Blend((_, mode)) => GraphicsMode::Blend(mode),
                Mode::Texture => GraphicsMode::Texture,
            };
            self.graphics.replace(self.pool.graphics_sets(
                #[cfg(debug_assertions)]
                &self.name,
                &self.driver,
                graphics_mode,
                render_pass_mode,
                0,
                self.src_textures.len(),
            ));
        }

        unsafe {
            self.write_descriptor_sets();
            self.submit_begin(render_pass_mode);

            let mut set_idx = 0;
            for write in writes.iter() {
                self.submit_write(write, &mut set_idx);
            }

            self.submit_finish();
        };

        WriteOpSubmission {
            back_buf: self.back_buf,
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            dst: self.dst,
            fence: self.fence,
            frame_buf: self.frame_buf.unwrap(),
            graphics: self.graphics.unwrap(),
            src_textures: self.src_textures,
        }
    }

    unsafe fn submit_begin(&mut self, render_pass_mode: RenderPassMode) {
        let mut back_buf = self.back_buf.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let graphics = self.graphics.as_ref().unwrap();
        let dims = dst.dims();
        let rect = dims.into();
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Optional step: Fill dst into the backbuffer in order to preserve it in the output
        if self.dst_preserve {
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::TransferSrcOptimal,
                PipelineStage::TRANSFER,
                Access::TRANSFER_READ,
            );
            back_buf.set_layout(
                &mut self.cmd_buf,
                Layout::TransferDstOptimal,
                PipelineStage::TRANSFER,
                Access::TRANSFER_WRITE,
            );
            self.cmd_buf.copy_image(
                dst.as_ref(),
                Layout::TransferSrcOptimal,
                back_buf.as_ref(),
                Layout::TransferDstOptimal,
                once(ImageCopy {
                    src_subresource: SubresourceLayers {
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    src_offset: Offset::ZERO,
                    dst_subresource: SubresourceLayers {
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    dst_offset: Offset::ZERO,
                    extent: dims.as_extent_depth(1),
                }),
            );
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::ShaderReadOnlyOptimal,
                PipelineStage::FRAGMENT_SHADER,
                Access::SHADER_READ,
            );
        }

        // Step 1: Write src into the backbuffer, but blending using our shader `mode`
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            Access::COLOR_ATTACHMENT_WRITE,
        );
        // src.set_layout(
        //     &mut self.cmd_buf,
        //     Layout::ShaderReadOnlyOptimal,
        //     PipelineStage::FRAGMENT_SHADER,
        //     Access::SHADER_READ,
        // );
        self.cmd_buf.begin_render_pass(
            self.pool.render_pass(&self.driver, render_pass_mode),
            self.frame_buf.as_ref().unwrap(),
            rect,
            &[TRANSPARENT_BLACK.into()],
            SubpassContents::Inline,
        );
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[rect]);
        self.cmd_buf.set_viewports(0, &[viewport]);
        bind_graphics_descriptor_set(&mut self.cmd_buf, graphics.layout(), graphics.desc_set(0));
    }

    unsafe fn submit_write(&mut self, write: &Write, set_idx: &mut usize) {
        let graphics = self.graphics.as_ref().unwrap();
        let layout = graphics.layout();

        // If this write (writes are sorted identically to `self.src_textures` except the writes have more items) is a different
        // texture we will need to switch to the next descriptor set - this won't happen on the first write of course.
        if !Texture2d::ptr_eq(write.src, &self.src_textures[*set_idx]) {
            *set_idx += 1;
            bind_graphics_descriptor_set(&mut self.cmd_buf, layout, graphics.desc_set(*set_idx));
        }

        let offset = Vec2::zero();
        let scale = Vec2::one();
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            VertexConsts {
                offset,
                scale,
                transform: write.transform,
            }
            .as_ref(),
        );

        if let Mode::Blend((ab, _)) = self.mode {
            let ab = ab as f32 / u8::MAX as f32;
            let inv = 1.0 - ab;
            self.cmd_buf.push_graphics_constants(
                graphics.layout(),
                ShaderStageFlags::FRAGMENT,
                64,
                &[ab.to_bits(), inv.to_bits()],
            );
        }

        self.cmd_buf.draw(0..6, 0..1);
    }

    unsafe fn submit_finish(&mut self) {
        let mut device = self.driver.borrow_mut();
        let mut back_buf = self.back_buf.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let dims = dst.dims();

        // End of the previous step...
        self.cmd_buf.end_render_pass();

        // Step 2: Copy the now-composited backbuffer to the `dst` texture
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_READ,
        );
        dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            back_buf.as_ref(),
            Layout::TransferSrcOptimal,
            dst.as_ref(),
            Layout::TransferDstOptimal,
            once(ImageCopy {
                src_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                src_offset: Offset::ZERO,
                dst_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                dst_offset: Offset::ZERO,
                extent: dims.as_extent_depth(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        Device::queue_mut(&mut device).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(&self.fence),
        );
    }

    unsafe fn write_descriptor_sets(&mut self) {
        let dst = self.dst.borrow();
        let dst_view = dst.as_default_view();
        let graphics = self.graphics.as_ref().unwrap();
        let sampler = graphics.sampler(0).as_ref();

        // Each source texture requres a unique descriptor set
        for (idx, src) in self.src_textures.iter().enumerate() {
            let set = graphics.desc_set(idx);

            // A descriptor for this source texture
            let src_ref = src.borrow();
            let src_view = src_ref.as_default_view();
            let src_desc = DescriptorSetWrite {
                set,
                binding: 0,
                array_offset: 0,
                descriptors: once(Descriptor::CombinedImageSampler(
                    &**src_view,
                    Layout::ShaderReadOnlyOptimal,
                    sampler,
                )),
            };

            // Blend mode requires a descriptor for the destination texture
            if let Mode::Blend(_) = self.mode {
                let dst_desc = DescriptorSetWrite {
                    set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: once(Descriptor::CombinedImageSampler(
                        &**dst_view,
                        Layout::ShaderReadOnlyOptimal,
                        sampler,
                    )),
                };
                // TODO: Borrow in a loop? Can these be lifted?
                self.driver
                    .borrow_mut()
                    .write_descriptor_sets(vec![src_desc, dst_desc]); // TODO: Slice/Tuple?
            } else {
                self.driver
                    .borrow_mut()
                    .write_descriptor_sets(once(src_desc));
            }
        }
    }
}

pub struct WriteOpSubmission {
    back_buf: Lease<Texture2d>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    dst: Texture2d,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    graphics: Lease<Graphics>,
    src_textures: Vec<Texture2d>,
}

impl Drop for WriteOpSubmission {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for WriteOpSubmission {
    fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
