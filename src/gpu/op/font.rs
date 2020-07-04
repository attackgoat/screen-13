use {
    super::{mat4_to_u32_array, wait_for_fence, BitmapOp},
    crate::{
        color::{AlphaColor, BLACK, TRANSPARENT_BLACK},
        gpu::{
            driver::{
                bind_graphics_descriptor_set, CommandPool, Driver, Fence, Framebuffer2d, Image2d,
                PhysicalDevice,
            },
            op::{Bitmap, Op},
            pool::{FontVertex, Graphics, GraphicsMode, Lease, RenderPassMode},
            Data, PoolRef, TextureRef,
        },
        math::{vec3, Coord, Extent, Mat4},
        pak::Pak,
    },
    bmfont::{BMFont, CharPosition, OrdinateOrientation},
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange, Usage as BufferUsage},
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        device::Device,
        format::{Aspects, Format},
        image::{
            Access as ImageAccess, Layout, Offset, SubresourceLayers, Tiling, Usage as ImageUsage,
        },
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, Rect, ShaderStageFlags, Viewport},
        queue::{CommandQueue, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        f32,
        io::{Read, Seek},
        iter::{empty, once},
        ops::Range,
        u64,
    },
};

const FONT_VERTEX_SIZE: usize = 16;
const QUEUE_TYPE: QueueType = QueueType::Graphics;
const RENDER_PASS_MODE: RenderPassMode = RenderPassMode::ReadWrite;
const SUBPASS_IDX: u8 = 0;

/// Holds a decoded bitmap Font.
pub struct Font {
    def: BMFont,
    pages: Vec<Bitmap>,
}

impl Font {
    pub(crate) fn load<K: AsRef<str>, R: Read + Seek>(
        pool: &PoolRef,
        pak: &mut Pak<R>,
        key: K,
        format: Format,
    ) -> Self {
        let def = BMFont::new(
            // This text is raw/it does not have any locale specialization
            pak.text_raw(key.as_ref()).as_bytes(),
            OrdinateOrientation::TopToBottom,
        )
        .unwrap();
        let pages = def
            .pages()
            .iter()
            .map(|page| {
                let bitmap = pak.read_bitmap(&format!("fonts/{}", page));
                unsafe {
                    BitmapOp::new(
                        #[cfg(debug_assertions)]
                        &format!("Font {} {}", key.as_ref(), page),
                        &pool,
                        &bitmap,
                        format,
                    )
                    .record()
                }
            })
            .collect();

        Self { def, pages }
    }

    fn char_vertices(page_dims: Extent, char_pos: &CharPosition, texture_dims: Extent) -> Vec<u8> {
        let x1 = char_pos.screen_rect.x as f32 / texture_dims.x as f32;
        let y1 = char_pos.screen_rect.y as f32 / texture_dims.y as f32;
        let x2 = (char_pos.screen_rect.x + char_pos.screen_rect.width as i32) as f32
            / texture_dims.x as f32;
        let y2 = (char_pos.screen_rect.y + char_pos.screen_rect.height as i32) as f32
            / (texture_dims.y as f32);
        let u1 = char_pos.page_rect.x as f32 / page_dims.x as f32;
        let v1 = char_pos.page_rect.y as f32 / page_dims.y as f32;
        let u2 =
            (char_pos.page_rect.x + char_pos.page_rect.width as i32) as f32 / page_dims.x as f32;
        let v2 =
            (char_pos.page_rect.y + char_pos.page_rect.height as i32) as f32 / page_dims.y as f32;
        let vertices = vec![
            FontVertex {
                x: x1,
                y: y1,
                u: u1,
                v: v1,
            },
            FontVertex {
                x: x2,
                y: y1,
                u: u2,
                v: v1,
            },
            FontVertex {
                x: x2,
                y: y2,
                u: u2,
                v: v2,
            },
            FontVertex {
                x: x1,
                y: y1,
                u: u1,
                v: v1,
            },
            FontVertex {
                x: x2,
                y: y2,
                u: u2,
                v: v2,
            },
            FontVertex {
                x: x1,
                y: y2,
                u: u1,
                v: v2,
            },
        ];

        let mut res = Vec::with_capacity(96);
        for vertex in vertices {
            res.extend(&vertex.x.to_ne_bytes());
            res.extend(&vertex.y.to_ne_bytes());
            res.extend(&vertex.u.to_ne_bytes());
            res.extend(&vertex.v.to_ne_bytes());
        }

        res
    }

    pub fn measure(&self, text: &str) -> Coord {
        let mut x = 0;
        let mut y = 0;
        for char_pos in self.def.parse(text).unwrap() {
            x = char_pos.screen_rect.x + char_pos.screen_rect.width as i32 - 1;
            y = char_pos.screen_rect.height as i32;
        }

        Coord::new(x, y)
    }

    fn tessellate(&self, text: &str, texture_dims: Extent) -> Vec<(usize, Vec<u8>)> {
        let mut tess_pages: Vec<Option<Vec<u8>>> = vec![];
        tess_pages.resize_with(self.pages.len(), Default::default);

        for char_pos in self.def.parse(text).unwrap() {
            let page_idx = char_pos.page_index as usize;
            let font_texture = &self.pages[page_idx];

            if tess_pages[page_idx].is_none() {
                tess_pages[page_idx] = Some(vec![]);
            }

            tess_pages[page_idx]
                .as_mut()
                .unwrap()
                .extend(&Self::char_vertices(
                    font_texture.borrow().dims(),
                    &char_pos,
                    texture_dims,
                ));
        }

        let mut res = vec![];
        for (idx, tess_page) in tess_pages.into_iter().enumerate() {
            if let Some(tess_page) = tess_page {
                res.push((idx, tess_page));
            }
        }

        res
    }
}

#[derive(Debug)]
pub struct FontOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    back_buf: Lease<TextureRef<Image2d>>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    dst: TextureRef<I>,
    fence: Lease<Fence>,
    frame_buf: Option<Framebuffer2d>,
    glyph_color: AlphaColor,
    graphics: Option<Lease<Graphics>>,
    #[cfg(debug_assertions)]
    name: String,
    outline_color: Option<AlphaColor>,
    pool: PoolRef,
    transform: Mat4,
    vertex_buf: Option<(Lease<Data>, u64)>,
}

impl<I> FontOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    pub fn new(#[cfg(debug_assertions)] name: &str, pool: &PoolRef, dst: &TextureRef<I>) -> Self {
        let (dims, format) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };

        let mut pool_ref = pool.borrow_mut();
        let back_buf = pool_ref.texture(
            #[cfg(debug_assertions)]
            name,
            dims,
            Tiling::Optimal,
            format,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );
        let family = pool_ref.driver().borrow().get_queue_family(QUEUE_TYPE);
        let mut cmd_pool = pool_ref.cmd_pool(family);
        let cmd_buf = unsafe { cmd_pool.allocate_one(Level::Primary) };
        let fence = pool_ref.fence();

        Self {
            #[cfg(debug_assertions)]
            name: name.to_owned(),
            back_buf,
            cmd_buf,
            cmd_pool,
            dst: TextureRef::clone(dst),
            fence,
            frame_buf: None,
            glyph_color: BLACK.into(),
            graphics: None,
            outline_color: None,
            pool: PoolRef::clone(&pool),
            transform: Mat4::identity(),
            vertex_buf: None,
        }
    }

    pub fn with_pos(mut self, pos: Coord) -> Self {
        let dims = self.dst.borrow().dims();
        self.transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
            * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
            * Mat4::from_translation(vec3(
                pos.x as f32 / dims.x as f32,
                pos.y as f32 / dims.y as f32,
                0.0,
            ));
        self
    }

    pub fn with_glyph_color<C>(mut self, color: C) -> Self
    where
        C: Into<AlphaColor>,
    {
        self.glyph_color = color.into();
        self
    }

    pub fn with_outline_color<C>(mut self, color: C) -> Self
    where
        C: Into<AlphaColor>,
    {
        self.outline_color = Some(color.into());
        self
    }

    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    pub fn record(mut self, font: &Font, text: &str) -> impl Op {
        assert!(!text.is_empty());

        let dims = self.dst.borrow().dims();

        // TODO: Cache these using "named" buffers? Let the client 'compile' them for reuse? Likey that more
        let tessellations = font.tessellate(text, dims);

        // Finish the remaining setup tasks
        {
            let mut pool = self.pool.borrow_mut();
            let driver = Driver::clone(pool.driver()); // TODO: Yuck

            // Setup the graphics pipeline
            self.graphics.replace(pool.graphics(
                #[cfg(debug_assertions)]
                &self.name,
                self.mode(),
                RENDER_PASS_MODE,
                SUBPASS_IDX,
            ));

            // Setup the framebuffer
            self.frame_buf.replace(Framebuffer2d::new(
                driver,
                pool.render_pass(RENDER_PASS_MODE),
                once(self.back_buf.borrow().as_default_2d_view().as_ref()),
                dims,
            ));

            // Setup the vetex buffers
            let vertex_buf_len = FONT_VERTEX_SIZE as u64
                * tessellations
                    .iter()
                    .map(|(_, vertices)| vertices.len())
                    .sum::<usize>() as u64;
            self.vertex_buf.replace((
                pool.data_usage(
                    #[cfg(debug_assertions)]
                    &self.name,
                    vertex_buf_len,
                    BufferUsage::VERTEX,
                ),
                vertex_buf_len,
            ));

            // Fill the vertex buffer with each tessellation in order
            let (vertex_buf, _) = self.vertex_buf.as_ref().unwrap();
            let mut dst = unsafe { vertex_buf.map_range_mut(0..vertex_buf_len) };
            let mut dst_offset = 0;
            for (_, vertices) in &tessellations {
                let len = vertices.len();
                dst[dst_offset..dst_offset + len].copy_from_slice(&vertices);
                dst_offset += len;
            }
        }

        unsafe {
            self.submit_begin(dims);

            // Draw each page in the tessellation using those vertices and the correct font page texture index
            let mut base = 0;
            for (page_idx, vertices) in &tessellations {
                self.write_descriptor_sets(font, *page_idx);

                // Submit the vertices for this page of the tessellation
                let len = vertices.len() as u32;
                self.submit_page(dims, base..base + len);
                base += len;
            }

            self.submit_finish(dims);
        };

        FontSubmission { op: self }
    }

    fn mode(&self) -> GraphicsMode {
        if self.outline_color.is_some() {
            GraphicsMode::FontOutline
        } else {
            GraphicsMode::Font
        }
    }

    unsafe fn submit_begin(&mut self, dims: Extent) {
        let graphics = self.graphics.as_ref().unwrap();
        let (vertex_buf, vertex_buf_len) = self.vertex_buf.as_ref().unwrap();
        let mut back_buf = self.back_buf.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let mut pool = self.pool.borrow_mut();

        // TODO: Limit this rect to just where we're drawing text
        let rect = Rect {
            x: 0,
            y: 0,
            w: dims.x as _,
            h: dims.y as _,
        };

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy the cpu-local vertex buffer to the gpu
        vertex_buf.copy_cpu(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT,
            BufferAccess::VERTEX_BUFFER_READ,
            *vertex_buf_len,
        );

        // Step 2: Copy dst into the backbuffer
        dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
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
                extent: dims.as_extent(1),
            }),
        );

        // Step 3: Draw the font vertices into the backbuffer
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        self.cmd_buf.begin_render_pass(
            pool.render_pass(RENDER_PASS_MODE),
            self.frame_buf.as_ref().unwrap(),
            rect,
            once(&TRANSPARENT_BLACK.into()),
            SubpassContents::Inline,
        );
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_page(&mut self, dims: Extent, vertices: Range<u32>) {
        let graphics = self.graphics.as_ref().unwrap();
        let (vertex_buf, _) = self.vertex_buf.as_mut().unwrap();
        let rect = Rect {
            x: 0,
            y: 0,
            w: dims.x as _,
            h: dims.y as _,
        };
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        bind_graphics_descriptor_set(&mut self.cmd_buf, graphics.layout(), graphics.desc_set(0));
        self.cmd_buf.set_scissors(0, &[rect]);
        self.cmd_buf.set_viewports(0, &[viewport]);
        self.cmd_buf.bind_vertex_buffers(
            0,
            Some((
                vertex_buf.as_ref().as_ref(),
                SubRange {
                    offset: 0,
                    size: None,
                },
            )),
        );

        // Push the vertex transform
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            &mat4_to_u32_array(self.transform),
        );

        // Push the glyph (and optional outline color) constants
        let mut push_constants = vec![];
        push_constants.extend(&self.glyph_color.to_rgba_unorm_u32_array());
        if let Some(outline_color) = self.outline_color {
            push_constants.extend(&outline_color.to_rgba_unorm_u32_array())
        }
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::FRAGMENT,
            64,
            push_constants.as_slice(),
        );

        self.cmd_buf.draw(vertices, 0..1);
    }

    unsafe fn submit_finish(&mut self, dims: Extent) {
        let mut dst = self.dst.borrow_mut();
        let mut back_buf = self.back_buf.borrow_mut();

        // Continue where submit_page left off
        self.cmd_buf.end_render_pass();

        // Step 3: Copy the backbuffer into dst
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
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
                extent: dims.as_extent(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        self.pool
            .borrow()
            .driver()
            .borrow_mut()
            .get_queue_mut(QUEUE_TYPE)
            .submit(
                Submission {
                    command_buffers: once(&self.cmd_buf),
                    wait_semaphores: empty(),
                    signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
                },
                Some(self.fence.as_ref()),
            );
    }

    unsafe fn write_descriptor_sets(&mut self, font: &Font, page_idx: usize) {
        let page = font.pages[page_idx].borrow();
        let page_view = page.as_default_2d_view();
        let graphics = self.graphics.as_ref().unwrap();
        self.pool
            .borrow()
            .driver()
            .borrow_mut()
            .write_descriptor_sets(once(DescriptorSetWrite {
                set: graphics.desc_set(0),
                binding: 0,
                array_offset: 0,
                descriptors: Some(Descriptor::CombinedImageSampler(
                    page_view.as_ref(),
                    Layout::General,
                    graphics.sampler(0).as_ref(),
                )),
            }));
    }
}

#[derive(Debug)]
struct FontSubmission<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    op: FontOp<I>,
}

impl<I> Drop for FontSubmission<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<I> Op for FontSubmission<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        unsafe {
            wait_for_fence(&self.op.pool.borrow().driver().borrow(), &self.op.fence);
        }
    }
}
