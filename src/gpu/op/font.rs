use {
    super::BitmapOp,
    crate::{
        color::{AlphaColor, TRANSPARENT_BLACK},
        gpu::{
            data::Mapping,
            driver::{
                bind_graphics_descriptor_set, CommandPool, Device, Driver, Fence, Framebuffer2d,
                PhysicalDevice,
            },
            op::{Bitmap, Op},
            pool::{
                ColorRenderPassMode, FontVertex, Graphics, GraphicsMode, Lease, Pool,
                RenderPassMode,
            },
            Data, Texture2d,
        },
        math::{vec3, CoordF, Extent, Mat4},
        pak::Pak,
    },
    bmfont::{BMFont, CharPosition, OrdinateOrientation},
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange, Usage as BufferUsage},
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        device::Device as _,
        format::Aspects,
        image::{
            Access as ImageAccess, Layout, Offset, SubresourceLayers, Tiling, Usage as ImageUsage,
        },
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, Rect, ShaderStageFlags, Viewport},
        queue::{CommandQueue, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
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
pub struct Font {
    def: BMFont,
    pages: Vec<Bitmap>,
}

impl Font {
    pub(crate) fn load<K: AsRef<str>, R: Read + Seek>(
        driver: &Driver,
        pool: &mut Pool,
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
                    #[cfg(debug_assertions)]
                    "Font",
                    driver,
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
pub struct FontOp<'a> {
    back_buf: Lease<Texture2d>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: Texture2d,
    fence: Lease<Fence>,
    frame_buf: Option<Framebuffer2d>,
    glyph_color: AlphaColor,
    graphics: Option<Lease<Graphics>>,

    #[cfg(debug_assertions)]
    name: String,

    outline_color: Option<AlphaColor>,
    pool: &'a mut Pool,
    transform: Mat4,
    vertex_buf: Option<(Lease<Data>, u64)>,
}

impl<'a> FontOp<'a> {
    pub fn new<C, P>(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        pool: &'a mut Pool,
        dst: Texture2d,
        pos: P,
        color: C,
    ) -> Self
    where
        C: Into<AlphaColor>,
        P: Into<CoordF>,
    {
        let (dims, fmt) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };

        let back_buf = pool.texture(
            #[cfg(debug_assertions)]
            name,
            &driver,
            dims,
            Tiling::Optimal,
            &[fmt],
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );
        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(&driver, family);
        let cmd_buf = unsafe { cmd_pool.allocate_one(Level::Primary) };
        let fence = pool.fence(
            #[cfg(debug_assertions)]
            name,
            &driver,
        );

        let pos = pos.into();
        let transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
            * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
            * Mat4::from_translation(vec3(pos.x / dims.x as f32, pos.y / dims.y as f32, 0.0));

        Self {
            back_buf,
            cmd_buf,
            cmd_pool,
            driver,
            dst,
            fence,
            frame_buf: None,
            glyph_color: color.into(),
            graphics: None,
            #[cfg(debug_assertions)]
            name: name.to_owned(),
            outline_color: None,
            pool,
            transform,
            vertex_buf: None,
        }
    }

    pub fn with_outline_color<C>(&mut self, color: C) -> &mut Self
    where
        C: Into<AlphaColor>,
    {
        self.outline_color = Some(color.into());
        self
    }

    pub fn with_transform(&mut self, transform: Mat4) -> &mut Self {
        self.transform = transform;
        self
    }

    pub fn record(mut self, font: &Font, text: &str) -> impl Op {
        assert!(!text.is_empty());

        let dims = self.dst.borrow().dims();

        // TODO: Cache these using "named" buffers? Let the client 'compile' them for reuse? Likey that more
        let tessellations = font.tessellate(text, dims);

        let render_pass_mode = RenderPassMode::Color(ColorRenderPassMode {
            format: self.dst.borrow().format(),
            preserve: false,
        });

        // Finish the remaining setup tasks
        {
            let driver = Driver::clone(&self.driver);

            // Setup the graphics pipeline
            self.graphics.replace(self.pool.graphics(
                #[cfg(debug_assertions)]
                &self.name,
                &self.driver,
                self.mode(),
                render_pass_mode,
                SUBPASS_IDX,
            ));

            // Setup the framebuffer
            self.frame_buf.replace(Framebuffer2d::new(
                #[cfg(debug_assertions)]
                self.name.as_str(),
                driver,
                self.pool.render_pass(&self.driver, render_pass_mode),
                once(self.back_buf.borrow().as_default_view().as_ref()),
                dims,
            ));

            // Setup the vetex buffers
            let vertex_buf_len = FONT_VERTEX_SIZE as u64
                * tessellations
                    .iter()
                    .map(|(_, vertices)| vertices.len())
                    .sum::<usize>() as u64;
            self.vertex_buf.replace((
                self.pool.data_usage(
                    #[cfg(debug_assertions)]
                    &self.name,
                    &self.driver,
                    vertex_buf_len,
                    BufferUsage::VERTEX,
                ),
                vertex_buf_len,
            ));

            // Fill the vertex buffer with each tessellation in order
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

        unsafe {
            self.submit_begin(dims, render_pass_mode);

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

        FontOpSubmission {
            back_buf: self.back_buf,
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            dst: self.dst,
            fence: self.fence,
            frame_buf: self.frame_buf.unwrap(),
            graphics: self.graphics.unwrap(),
            vertex_buf: self.vertex_buf.unwrap().0,
        }
    }

    fn mode(&self) -> GraphicsMode {
        if self.outline_color.is_some() {
            GraphicsMode::FontOutline
        } else {
            GraphicsMode::Font
        }
    }

    unsafe fn submit_begin(&mut self, dims: Extent, render_pass_mode: RenderPassMode) {
        let graphics = self.graphics.as_ref().unwrap();
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
            self.pool.render_pass(&self.driver, render_pass_mode),
            self.frame_buf.as_ref().unwrap(),
            rect,
            once(&TRANSPARENT_BLACK.into()),
            SubpassContents::Inline,
        );
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_page(&mut self, dims: Extent, vertices: Range<u32>) {
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
            VertexConsts {
                transform: self.transform,
            }
            .as_ref(),
        );

        // Push the glyph (and optional outline color) constants
        // TODO: Maybe slice extend instead
        let mut push_constants = vec![];
        push_constants.extend(&self.glyph_color.to_unorm_bits());
        if let Some(outline_color) = self.outline_color {
            push_constants.extend(&outline_color.to_unorm_bits())
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
        let mut device = self.driver.borrow_mut();
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
        Device::queue_mut(&mut device).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(&self.fence),
        );
    }

    unsafe fn write_descriptor_sets(&mut self, font: &Font, page_idx: usize) {
        // TODO: Fix, this should be one set per page not the same re-written
        let page = font.pages[page_idx].borrow();
        let page_view = page.as_default_view();
        let graphics = self.graphics.as_ref().unwrap();
        self.driver
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

pub struct FontOpSubmission {
    back_buf: Lease<Texture2d>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    dst: Texture2d,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    graphics: Lease<Graphics>,
    vertex_buf: Lease<Data>,
}

impl Drop for FontOpSubmission {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for FontOpSubmission {
    fn wait(&self) {
        Fence::wait(&self.fence);
    }
}

#[repr(C)]
struct VertexConsts {
    transform: Mat4,
}

impl AsRef<[u32; 16]> for VertexConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 16] {
        unsafe { &*(self as *const _ as *const _) }
    }
}
