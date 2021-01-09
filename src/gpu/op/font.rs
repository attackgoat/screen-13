use {
    super::{
        bitmap::{Bitmap, BitmapOp},
        Op,
    },
    crate::{
        color::{AlphaColor, TRANSPARENT_BLACK},
        gpu::{
            data::Mapping,
            def::{
                push_const::{FontPushConsts, Mat4PushConst, Vec4PushConst},
                ColorRenderPassMode, Graphics, GraphicsMode, RenderPassMode,
            },
            device,
            driver::{bind_graphics_descriptor_set, CommandPool, Fence, Framebuffer2d},
            pool::{Lease, Pool},
            queue_mut, Data, Texture2d,
        },
        math::{vec3, CoordF, Extent, Mat4},
        pak::Pak,
    },
    archery::SharedPointerKind,
    bmfont::{BMFont, CharPosition, OrdinateOrientation},
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange, Usage as BufferUsage},
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        device::Device as _,
        format::Aspects,
        image::{Access as ImageAccess, Layout, Offset, SubresourceLayers, Usage as ImageUsage},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, Rect, ShaderStageFlags, Viewport},
        queue::{CommandQueue, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        any::Any,
        f32,
        io::{Cursor, Read, Seek},
        iter::{empty, once},
        ops::Range,
        u64,
    },
};

const FONT_VERTEX_SIZE: usize = 16;
const SUBPASS_IDX: u8 = 0;

// TODO: Extend this with a DrawOp-like compiler to cache repeated frame-to-frame tesselations
// TODO: Allow one FontOp to specify a list of colors, make a rainbow-colored text example for it
/// Holds a decoded bitmap Font.
#[derive(Debug)]
pub struct Font<P>
where
    P: 'static + SharedPointerKind,
{
    def: BMFont,
    pages: Vec<Bitmap<P>>,
}

impl<P> Font<P>
where
    P: SharedPointerKind,
{
    pub(crate) fn load<K: AsRef<str>, R: Read + Seek>(
        pool: &mut Pool<P>,
        pak: &mut Pak<R>,
        key: K,
    ) -> Self {
        let id = pak.bitmap_font_id(key).unwrap();
        let bitmap_font = pak.read_bitmap_font(id);
        let def = BMFont::new(
            Cursor::new(bitmap_font.def()),
            OrdinateOrientation::TopToBottom,
        )
        .unwrap();
        let pages = bitmap_font
            .pages()
            .map(|page| unsafe {
                BitmapOp::new(
                    #[cfg(feature = "debug-names")]
                    "Font",
                    pool,
                    &page,
                )
                .record()
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
                y: y2,
                u: u2,
                v: v2,
            },
            FontVertex {
                x: x2,
                y: y1,
                u: u2,
                v: v1,
            },
            FontVertex {
                x: x1,
                y: y1,
                u: u1,
                v: v1,
            },
            FontVertex {
                x: x1,
                y: y2,
                u: u1,
                v: v2,
            },
            FontVertex {
                x: x2,
                y: y2,
                u: u2,
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

    /// Returns the area, in pixels, required to render the given text.
    pub fn measure(&self, text: &str) -> Extent {
        let mut x = 0;
        let mut y = 0;
        for char_pos in self.def.parse(text).unwrap() {
            x = char_pos.screen_rect.x + char_pos.screen_rect.width as i32 - 1;
            y = char_pos.screen_rect.height as i32;
        }

        assert!(x >= 0);
        assert!(y >= 0);

        Extent::new(x as _, y as _)
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

// TODO: This really needs to cache data like the draw compiler does
/// A container of graphics types and the functions which allows the recording and submission of
/// bitmapped font operations.
pub struct FontOp<P>
where
    P: 'static + SharedPointerKind,
{
    back_buf: Lease<Texture2d, P>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    dst: Texture2d,
    fence: Lease<Fence, P>,
    frame_buf: Option<Framebuffer2d>,
    glyph_color: AlphaColor,
    graphics: Option<Lease<Graphics, P>>,

    #[cfg(feature = "debug-names")]
    name: String,

    outline_color: Option<AlphaColor>,
    pool: Option<Lease<Pool<P>, P>>,
    transform: Mat4,
    vertex_buf: Option<(Lease<Data, P>, u64)>,
}

impl<P> FontOp<P>
where
    P: SharedPointerKind,
{
    #[must_use]
    pub(crate) unsafe fn new<C, O>(
        #[cfg(feature = "debug-names")] name: &str,
        mut pool: Lease<Pool<P>, P>,
        dst: &Texture2d,
        pos: O,
        color: C,
    ) -> Self
    where
        C: Into<AlphaColor>,
        O: Into<CoordF>,
    {
        let (dims, fmt) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };

        let back_buf = pool.texture(
            #[cfg(feature = "debug-names")]
            name,
            dims,
            fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
            1,
            1,
            1,
        );
        let mut cmd_pool = pool.cmd_pool();
        let cmd_buf = cmd_pool.allocate_one(Level::Primary);
        let fence = pool.fence(
            #[cfg(feature = "debug-names")]
            name,
        );

        let pos = pos.into();
        let transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
            * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
            * Mat4::from_translation(vec3(pos.x / dims.x as f32, pos.y / dims.y as f32, 0.0));

        Self {
            back_buf,
            cmd_buf,
            cmd_pool,
            dst: Texture2d::clone(dst),
            fence,
            frame_buf: None,
            glyph_color: color.into(),
            graphics: None,
            #[cfg(feature = "debug-names")]
            name: name.to_owned(),
            outline_color: None,
            pool: Some(pool),
            transform,
            vertex_buf: None,
        }
    }

    /// Sets the font outline color to use.
    #[must_use]
    pub fn with_outline<C>(&mut self, color: C) -> &mut Self
    where
        C: Into<AlphaColor>,
    {
        self.outline_color = Some(color.into());
        self
    }

    /// Sets the generalized output transform to use.
    ///
    /// _NOTE:_ Overrides placement options.
    #[must_use]
    pub fn with_transform(&mut self, transform: Mat4) -> &mut Self {
        self.transform = transform;
        self
    }

    /// Submits the given font for hardware processing.
    pub fn record(&mut self, font: &Font<P>, text: &str) {
        assert!(!text.is_empty());

        let dims = self.dst.borrow().dims();
        let graphics_mode = self.mode();
        let render_pass_mode = RenderPassMode::Color(ColorRenderPassMode {
            fmt: self.dst.borrow().format(),
            preserve: true,
        });
        let pool = self.pool.as_mut().unwrap();

        // TODO: Cache these using "named" buffers? Let the client 'compile' them for reuse? Likey that more
        let tessellations = font.tessellate(text, dims);

        // Finish the remaining setup tasks
        unsafe {
            // Setup the graphics pipeline
            self.graphics.replace(pool.graphics_desc_sets(
                #[cfg(feature = "debug-names")]
                &self.name,
                render_pass_mode,
                SUBPASS_IDX,
                graphics_mode,
                1,
            ));

            // Setup the framebuffer
            self.frame_buf.replace(Framebuffer2d::new(
                #[cfg(feature = "debug-names")]
                self.name.as_str(),
                pool.render_pass(render_pass_mode),
                once(self.back_buf.borrow().as_2d_color().as_ref()),
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
                    #[cfg(feature = "debug-names")]
                    &self.name,
                    vertex_buf_len,
                    BufferUsage::VERTEX,
                ),
                vertex_buf_len,
            ));

            // Fill the vertex buffer with each tessellation in order
            {
                let (vertex_buf, _) = self.vertex_buf.as_mut().unwrap();
                let mut dst = vertex_buf.map_range_mut(0..vertex_buf_len).unwrap(); // TODO: Error handling!
                let mut dst_offset = 0;
                for (_, vertices) in &tessellations {
                    let len = vertices.len();
                    dst[dst_offset..dst_offset + len].copy_from_slice(&vertices);
                    dst_offset += len;
                }

                Mapping::flush(&mut dst).unwrap(); // TODO: Error handling!
            }

            self.submit_begin(dims, render_pass_mode);

            // Draw each page in the tessellation using those vertices and the correct font page texture index
            let mut base = 0;
            for (page_idx, vertices) in &tessellations {
                self.write_descriptors(font, *page_idx);

                self.submit_page_begin(dims);

                if self.outline_color.is_some() {
                    self.submit_page_outline();
                } else {
                    self.submit_page_normal();
                }

                // Submit the vertices for this page of the tessellation
                let len = vertices.len() as u32;
                self.submit_page_finish(base..base + len);
                base += len;
            }

            self.submit_finish(dims);
        }
    }

    fn mode(&self) -> GraphicsMode {
        if self.outline_color.is_some() {
            GraphicsMode::Font(true)
        } else {
            GraphicsMode::Font(false)
        }
    }

    unsafe fn submit_begin(&mut self, dims: Extent, render_pass_mode: RenderPassMode) {
        trace!("submit_begin");

        let graphics = self.graphics.as_ref().unwrap();
        let pool = self.pool.as_mut().unwrap();
        let render_pass = pool.render_pass(render_pass_mode);
        let (vertex_buf, vertex_buf_len) = self.vertex_buf.as_mut().unwrap();
        let mut back_buf = self.back_buf.borrow_mut();
        let mut dst = self.dst.borrow_mut();

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
        vertex_buf.write_range(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT,
            BufferAccess::VERTEX_BUFFER_READ,
            0..*vertex_buf_len,
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
                extent: dims.as_extent_depth(1),
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
            render_pass,
            self.frame_buf.as_ref().unwrap(),
            rect,
            once(&TRANSPARENT_BLACK.into()),
            SubpassContents::Inline,
        );
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_page_begin(&mut self, dims: Extent) {
        trace!("submit_page_begin");

        let graphics = self.graphics.as_ref().unwrap();
        let (vertex_buf, vertex_buf_len) = self.vertex_buf.as_mut().unwrap();
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
                vertex_buf.as_ref(),
                SubRange {
                    offset: 0,
                    size: Some(*vertex_buf_len),
                },
            )),
        );

        // Push the vertex transform
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            Mat4PushConst {
                val: self.transform,
            }
            .as_ref(),
        );
    }

    unsafe fn submit_page_normal(&mut self) {
        trace!("submit_page_normal");

        let graphics = self.graphics.as_ref().unwrap();
        let layout = graphics.layout();
        let push_constants = Vec4PushConst {
            val: self.glyph_color.to_rgba(),
        };

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::FRAGMENT,
            Mat4PushConst::BYTE_LEN,
            push_constants.as_ref(),
        );
    }

    unsafe fn submit_page_outline(&mut self) {
        trace!("submit_page_outline");

        let graphics = self.graphics.as_ref().unwrap();
        let layout = graphics.layout();
        let mut push_constants = FontPushConsts::default();
        push_constants.glyph_color = self.glyph_color.to_rgba();
        push_constants.outline_color = self.outline_color.as_ref().unwrap().to_rgba();

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::FRAGMENT,
            Mat4PushConst::BYTE_LEN,
            push_constants.as_ref(),
        );
    }

    unsafe fn submit_page_finish(&mut self, vertices: Range<u32>) {
        trace!("submit_page_finish");

        self.cmd_buf.draw(vertices, 0..1);
    }

    unsafe fn submit_finish(&mut self, dims: Extent) {
        trace!("submit_finish");

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
                extent: dims.as_extent_depth(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        queue_mut().submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(&self.fence),
        );
    }

    unsafe fn write_descriptors(&mut self, font: &Font<P>, page_idx: usize) {
        trace!("write_descriptors");

        // TODO: Fix, this should be one set per page not the same re-written
        let page = font.pages[page_idx].borrow();
        let page_view = page.as_2d_color();
        let graphics = self.graphics.as_ref().unwrap();
        device().write_descriptor_sets(once(DescriptorSetWrite {
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

impl<P> Drop for FontOp<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        unsafe {
            self.wait();
        }
    }
}

impl<P> Op<P> for FontOp<P>
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P> {
        self.pool.take().unwrap()
    }

    unsafe fn wait(&self) {
        Fence::wait(&self.fence);
    }
}

#[derive(Clone, Copy, Default)]
struct FontVertex {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
}
