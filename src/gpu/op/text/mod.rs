mod bitmap_font;
mod command;
mod compiler;
mod instruction;
mod scalable_font;

pub use self::{
    bitmap_font::BitmapFont, command::Command, compiler::Compiler, scalable_font::ScalableFont,
};

use {
    self::instruction::Instruction,
    super::Op,
    crate::{
        color::{AlphaColor, TRANSPARENT_BLACK},
        gpu::{
            def::{
                push_const::{FontPushConsts, Mat4PushConst, Vec4PushConst},
                ColorRenderPassMode, FontMode, Graphics, GraphicsMode, RenderPassMode,
            },
            device,
            driver::{bind_graphics_descriptor_set, CommandPool, Fence, Framebuffer2d},
            pool::{Lease, Pool},
            queue_mut, Data, Texture, Texture2d,
        },
        math::{Extent, Mat4},
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange},
        command::{
            CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, RenderAttachmentInfo,
            SubpassContents,
        },
        device::Device as _,
        format::Aspects,
        image::{
            Access as ImageAccess, FramebufferAttachment, Layout, Offset, SubresourceLayers,
            Usage as ImageUsage, ViewCapabilities,
        },
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, Rect, ShaderStageFlags, Viewport},
        queue::{CommandQueue, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        any::Any,
        borrow::Borrow,
        iter::{empty, once},
        ops::{Deref, Range},
        u64,
    },
};

pub const DEFAULT_SIZE: f32 = 32.0;
const FONT_VERTEX_SIZE: usize = 16;
const SUBPASS_IDX: u8 = 0;

/// Holds either a bitmapped or TrueType/Opentype font.
pub enum Font<P>
where
    P: 'static + SharedPointerKind,
{
    /// A fixed-size bitmapped font as produced by programs compatible with the `.fnt` file format.
    ///
    /// **_NOTE:_** [BMFont](https://www.angelcode.com/products/bmfont/) is supported.
    Bitmap(Shared<BitmapFont<P>, P>),

    /// A variable-size font.
    Scalable(Shared<ScalableFont, P>),
}

impl<P> From<Shared<BitmapFont<P>, P>> for Font<P>
where
    P: SharedPointerKind,
{
    fn from(val: Shared<BitmapFont<P>, P>) -> Self {
        Self::Bitmap(val)
    }
}

impl<'a, P> From<&'a Shared<BitmapFont<P>, P>> for Font<P>
where
    P: SharedPointerKind,
{
    fn from(val: &'a Shared<BitmapFont<P>, P>) -> Self {
        Self::Bitmap(Shared::clone(val))
    }
}

impl<P> From<Shared<ScalableFont, P>> for Font<P>
where
    P: SharedPointerKind,
{
    fn from(val: Shared<ScalableFont, P>) -> Self {
        Self::Scalable(val)
    }
}

impl<'a, P> From<&'a Shared<ScalableFont, P>> for Font<P>
where
    P: SharedPointerKind,
{
    fn from(val: &'a Shared<ScalableFont, P>) -> Self {
        Self::Scalable(Shared::clone(val))
    }
}

/// A container of graphics types and the functions which allows the recording and submission of
/// bitmapped font operations.
pub struct TextOp<P>
where
    P: 'static + SharedPointerKind,
{
    back_buf: Lease<Shared<Texture2d, P>, P>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    compiler: Option<Lease<Compiler<P>, P>>,
    dst: Shared<Texture2d, P>,
    fence: Lease<Fence, P>,
    frame_buf: Framebuffer2d,
    graphics_bitmap: Option<Lease<Graphics, P>>,
    graphics_bitmap_outline: Option<Lease<Graphics, P>>,
    graphics_scalable: Option<Lease<Graphics, P>>,

    #[cfg(feature = "debug-names")]
    name: String,

    pool: Option<Lease<Pool<P>, P>>,
}

impl<P> TextOp<P>
where
    P: SharedPointerKind,
{
    #[must_use]
    pub(crate) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        mut pool: Lease<Pool<P>, P>,
        dst: &Shared<Texture2d, P>,
    ) -> Self {
        let dims = dst.dims();
        let fmt = dst.format();

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

        // Setup the framebuffer
        let frame_buf = Framebuffer2d::new(
            #[cfg(feature = "debug-names")]
            self.name.as_str(),
            pool.render_pass(RenderPassMode::Color(ColorRenderPassMode {
                fmt,
                preserve: true,
            })),
            once(FramebufferAttachment {
                format: fmt,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                view_caps: ViewCapabilities::MUTABLE_FORMAT,
            }),
            dims,
        );

        Self {
            back_buf,
            cmd_buf,
            cmd_pool,
            compiler: None,
            dst: Shared::clone(dst),
            fence,
            frame_buf,
            graphics_bitmap: None,
            graphics_bitmap_outline: None,
            graphics_scalable: None,

            #[cfg(feature = "debug-names")]
            name: name.to_owned(),

            pool: Some(pool),
        }
    }

    /// Submits the given commands for hardware processing.
    pub fn record<'c, C, T>(&mut self, cmds: &'c [C])
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        unsafe {
            let pool = self.pool.as_mut().unwrap();
            let mut compiler = pool.text_compiler();
            {
                let dims = self.dst.dims();
                let mut instrs = compiler.compile(
                    #[cfg(feature = "debug-names")]
                    &self.name,
                    cmds,
                    dims,
                );
                let fmt = self.dst.format();
                let render_pass_mode = RenderPassMode::Color(ColorRenderPassMode {
                    fmt,
                    preserve: true,
                });

                // Texture descriptors for one-color bitmap fonts
                {
                    let descriptors = instrs.bitmap_descriptors();
                    let desc_sets = descriptors.len();
                    if desc_sets > 0 {
                        let mut graphics = pool.graphics_desc_sets(
                            #[cfg(feature = "debug-names")]
                            &self.name,
                            render_pass_mode,
                            SUBPASS_IDX,
                            GraphicsMode::Font(FontMode::Bitmap(false)),
                            desc_sets,
                        );
                        Self::write_texture_descriptors(&mut graphics, descriptors);
                        self.graphics_bitmap = Some(graphics);
                    }
                }

                // Texture descriptors for two-color bitmap fonts
                {
                    let descriptors = instrs.bitmap_outline_descriptors();
                    let desc_sets = descriptors.len();
                    if desc_sets > 0 {
                        let mut graphics = pool.graphics_desc_sets(
                            #[cfg(feature = "debug-names")]
                            &self.name,
                            render_pass_mode,
                            SUBPASS_IDX,
                            GraphicsMode::Font(FontMode::Bitmap(true)),
                            desc_sets,
                        );
                        Self::write_texture_descriptors(&mut graphics, descriptors);
                        self.graphics_bitmap_outline = Some(graphics);
                    }
                }

                // TODO: Replace this!
                // let pos = pos.into();
                // let transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
                //     * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
                //     * Mat4::from_translation(vec3(pos.x / dims.x as f32, pos.y / dims.y as f32, 0.0));

                self.submit_begin(dims, render_pass_mode);

                while let Some(instr) = instrs.next() {
                    match instr {
                        Instruction::BitmapDescriptor(desc_set) => {
                            self.submit_bitmap_descriptor(desc_set)
                        }
                        Instruction::BitmapOutlineDescriptor(desc_set) => {
                            self.submit_bitmap_outline_descriptor(desc_set)
                        }
                        Instruction::ScalableDescriptor(desc_set) => {
                            self.submit_scalable_descriptor(desc_set)
                        }
                        _ => todo!(),
                    }
                }

                self.submit_finish(dims);
            }

            self.compiler = Some(compiler);
        }
        /*

        let (dims, render_pass_mode, tessellations) = {

            let pool = self.pool.as_mut().unwrap();

            // TODO: Cache these using "named" buffers? Let the client 'compile' them for reuse? Likey that more
            let tessellations = font.tessellate(text, dims);

            // Finish the remaining setup tasks
            unsafe {




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
            }

            (dims, render_pass_mode, tessellations)
        };

        unsafe {
            // Draw each page in the tessellation using those vertices and the correct font page texture index
            let mut base = 0;
            for (page_idx, vertices) in &tessellations {
                self.submit_page_begin(dims, *page_idx);

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

        }
        */
    }

    unsafe fn submit_begin(&mut self, dims: Extent, render_pass_mode: RenderPassMode) {
        trace!("submit_begin");

        // let graphics = self.graphics.as_ref().unwrap();
        // let pool = self.pool.as_mut().unwrap();
        // let render_pass = pool.render_pass(render_pass_mode);
        // let (vertex_buf, vertex_buf_len) = self.vertex_buf.as_mut().unwrap();

        // // TODO: Limit this rect to just where we're drawing text
        // let rect = Rect {
        //     x: 0,
        //     y: 0,
        //     w: dims.x as _,
        //     h: dims.y as _,
        // };

        // // Begin
        // self.cmd_buf
        //     .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // // Step 1: Copy the cpu-local vertex buffer to the gpu
        // vertex_buf.write_range(
        //     &mut self.cmd_buf,
        //     PipelineStage::VERTEX_INPUT,
        //     BufferAccess::VERTEX_BUFFER_READ,
        //     0..*vertex_buf_len,
        // );

        // // Step 2: Copy dst into the backbuffer
        // self.dst.set_layout(
        //     &mut self.cmd_buf,
        //     Layout::TransferSrcOptimal,
        //     PipelineStage::TRANSFER,
        //     ImageAccess::TRANSFER_READ,
        // );
        // self.back_buf.set_layout(
        //     &mut self.cmd_buf,
        //     Layout::TransferDstOptimal,
        //     PipelineStage::TRANSFER,
        //     ImageAccess::TRANSFER_WRITE,
        // );
        // self.cmd_buf.copy_image(
        //     self.dst.as_ref(),
        //     Layout::TransferSrcOptimal,
        //     self.back_buf.as_ref(),
        //     Layout::TransferDstOptimal,
        //     once(ImageCopy {
        //         src_subresource: SubresourceLayers {
        //             aspects: Aspects::COLOR,
        //             level: 0,
        //             layers: 0..1,
        //         },
        //         src_offset: Offset::ZERO,
        //         dst_subresource: SubresourceLayers {
        //             aspects: Aspects::COLOR,
        //             level: 0,
        //             layers: 0..1,
        //         },
        //         dst_offset: Offset::ZERO,
        //         extent: dims.as_extent_depth(1),
        //     }),
        // );

        // // Step 3: Draw the font vertices into the backbuffer
        // self.back_buf.set_layout(
        //     &mut self.cmd_buf,
        //     Layout::ColorAttachmentOptimal,
        //     PipelineStage::COLOR_ATTACHMENT_OUTPUT,
        //     ImageAccess::COLOR_ATTACHMENT_WRITE,
        // );
        // self.cmd_buf.begin_render_pass(
        //     render_pass,
        //     self.frame_buf.as_ref().unwrap(),
        //     rect,
        //     once(RenderAttachmentInfo {
        //         image_view: self.back_buf.as_2d_color().as_ref(),
        //         clear_value: TRANSPARENT_BLACK.into(),
        //     }),
        //     SubpassContents::Inline,
        // );
        // self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_bitmap_descriptor(&mut self, desc_set: usize) {
        trace!("submit_bitmap_descriptor");

        let graphics = self.graphics_bitmap.as_ref().unwrap();
        let desc_set = graphics.desc_set(desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
    }

    unsafe fn submit_bitmap_outline_descriptor(&mut self, desc_set: usize) {
        trace!("submit_bitmap_outline_descriptor");

        let graphics = self.graphics_bitmap_outline.as_ref().unwrap();
        let desc_set = graphics.desc_set(desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
    }

    unsafe fn submit_page_begin(&mut self, dims: Extent, page_idx: usize) {
        trace!("submit_page_begin");

        // let graphics = self.graphics.as_ref().unwrap();
        // let (vertex_buf, vertex_buf_len) = self.vertex_buf.as_mut().unwrap();
        // let rect = Rect {
        //     x: 0,
        //     y: 0,
        //     w: dims.x as _,
        //     h: dims.y as _,
        // };
        // let viewport = Viewport {
        //     rect,
        //     depth: 0.0..1.0,
        // };

        // bind_graphics_descriptor_set(
        //     &mut self.cmd_buf,
        //     graphics.layout(),
        //     graphics.desc_set(page_idx),
        // );
        // self.cmd_buf.set_scissors(0, &[rect]);
        // self.cmd_buf.set_viewports(0, &[viewport]);
        // self.cmd_buf.bind_vertex_buffers(
        //     0,
        //     Some((
        //         vertex_buf.as_ref(),
        //         SubRange {
        //             offset: 0,
        //             size: Some(*vertex_buf_len),
        //         },
        //     )),
        // );

        // // Push the vertex transform
        // self.cmd_buf.push_graphics_constants(
        //     graphics.layout(),
        //     ShaderStageFlags::VERTEX,
        //     0,
        //     Mat4PushConst {
        //         val: self.transform,
        //     }
        //     .as_ref(),
        // );
    }

    unsafe fn submit_page_normal(&mut self) {
        trace!("submit_page_normal");

        // let graphics = self.graphics.as_ref().unwrap();
        // let layout = graphics.layout();
        // let push_constants = Vec4PushConst {
        //     val: self.glyph_color.to_rgba(),
        // };

        // self.cmd_buf.push_graphics_constants(
        //     layout,
        //     ShaderStageFlags::FRAGMENT,
        //     Mat4PushConst::BYTE_LEN,
        //     push_constants.as_ref(),
        // );
    }

    unsafe fn submit_page_outline(&mut self) {
        trace!("submit_page_outline");

        // let graphics = self.graphics.as_ref().unwrap();
        // let layout = graphics.layout();
        // let push_constants = FontPushConsts {
        //     glyph_color: self.glyph_color.to_rgba(),
        //     outline_color: self.outline_color.as_ref().unwrap().to_rgba(),
        // };

        // self.cmd_buf.push_graphics_constants(
        //     layout,
        //     ShaderStageFlags::FRAGMENT,
        //     Mat4PushConst::BYTE_LEN,
        //     push_constants.as_ref(),
        // );
    }

    unsafe fn submit_page_finish(&mut self, vertices: Range<u32>) {
        trace!("submit_page_finish");

        self.cmd_buf.draw(vertices, 0..1);
    }

    unsafe fn submit_scalable_descriptor(&mut self, desc_set: usize) {
        trace!("submit_scalable_descriptor");

        let graphics = self.graphics_scalable.as_ref().unwrap();
        let desc_set = graphics.desc_set(desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
    }

    unsafe fn submit_finish(&mut self, dims: Extent) {
        trace!("submit_finish");

        // Continue where submit_page left off
        self.cmd_buf.end_render_pass();

        // Step 3: Copy the backbuffer into dst
        self.back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        self.dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            self.back_buf.as_ref(),
            Layout::TransferSrcOptimal,
            self.dst.as_ref(),
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
            Some(&mut self.fence),
        );
    }

    unsafe fn write_texture_descriptors<I, T>(graphics: &mut Graphics, textures: I)
    where
        I: Iterator<Item = T>,
        T: Deref<Target = Texture2d>,
    {
        for (idx, tex) in textures.enumerate() {
            let (set, samplers) = graphics.desc_set_mut_with_samplers(idx);

            device().write_descriptor_set(DescriptorSetWrite {
                set,
                binding: 0,
                array_offset: 0,
                descriptors: Some(Descriptor::CombinedImageSampler(
                    tex.as_2d_color().as_ref(),
                    Layout::General,
                    samplers[0].as_ref(),
                )),
            });
        }
    }
}

impl<P> Drop for TextOp<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        unsafe {
            self.wait();
        }
    }
}

impl<P> Op<P> for TextOp<P>
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    unsafe fn is_complete(&self) -> bool {
        Fence::status(&self.fence)
    }

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P> {
        self.pool.take().unwrap()
    }

    unsafe fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
