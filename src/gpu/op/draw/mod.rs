mod command;
mod compiler;
mod graphics_buf;
mod instruction;
mod spotlight;
mod sunlight;

pub use self::{
    command::Command,
    compiler::{Compilation, Compiler},
};

use {
    self::{
        command::MeshCommand,
        compiler::Stages,
        graphics_buf::GraphicsBuffer,
        instruction::{LightInstruction, LineInstruction, MeshInstruction},
        spotlight::SpotlightCommand,
        sunlight::SunlightCommand,
    },
    super::{mat4_to_mat3_u32_array, mat4_to_u32_array, wait_for_fence, Op},
    crate::{
        camera::Camera,
        color::TRANSPARENT_BLACK,
        gpu::{
            driver::{CommandPool, Driver, Fence, Framebuffer2d, PhysicalDevice},
            pool::{Graphics, GraphicsMode, Lease, MeshType, RenderPassMode},
            Data, PoolRef, TextureRef,
        },
        math::Mat4,
    },
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange, Usage as BufferUsage},
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        format::{Aspects, Format},
        image::{
            Access as ImageAccess, Layout, Offset, SubresourceLayers, SubresourceRange, ViewKind,
        },
        pool::CommandPool as _,
        pso::{PipelineStage, Rect, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::iter::{empty, once},
};

const QUEUE_TYPE: QueueType = QueueType::Graphics;

pub struct Draw<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: TextureRef<I>,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
}

impl<I> Drop for Draw<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<I> Op for Draw<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        unsafe {
            wait_for_fence(&self.driver.borrow(), &self.fence);
        }
    }
}

#[derive(Debug)]
pub struct DrawOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    dst: TextureRef<I>,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    graphics_buf: GraphicsBuffer,
    graphics_line: Option<Lease<Graphics>>,
    graphics_mesh_dual_tex: Option<Lease<Graphics>>,
    graphics_mesh_single_tex: Option<Lease<Graphics>>,
    graphics_mesh_trans: Option<Lease<Graphics>>,
    graphics_spotlight: Option<Lease<Graphics>>,
    graphics_sunlight: Option<Lease<Graphics>>,
    line_buf: Option<(Lease<Data>, u64)>,
    #[cfg(debug_assertions)]
    name: String,
    pool: PoolRef,
}

impl<I> DrawOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    /// # Safety
    /// None
    pub fn new(#[cfg(debug_assertions)] name: &str, pool: &PoolRef, dst: &TextureRef<I>) -> Self {
        let mut pool_ref = pool.borrow_mut();
        let driver = Driver::clone(pool_ref.driver());

        // Allocate the command buffer
        let family = driver.borrow().get_queue_family(QUEUE_TYPE);
        let mut cmd_pool = pool_ref.cmd_pool(family);

        let (dims, format) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };

        // Setup the framebuffer
        let graphics_buf = GraphicsBuffer::new(
            #[cfg(debug_assertions)]
            name,
            &mut pool_ref,
            dims,
            format,
        );
        let frame_buf = Framebuffer2d::new(
            Driver::clone(&driver),
            pool_ref.render_pass(RenderPassMode::Draw),
            vec![
                graphics_buf.color().borrow().as_default_2d_view().as_ref(),
                graphics_buf
                    .position()
                    .borrow()
                    .as_default_2d_view()
                    .as_ref(),
                graphics_buf.normal().borrow().as_default_2d_view().as_ref(),
                graphics_buf
                    .material()
                    .borrow()
                    .as_default_2d_view()
                    .as_ref(),
                graphics_buf
                    .depth()
                    .borrow()
                    .as_view(
                        ViewKind::D2,
                        Format::D32Sfloat,
                        Default::default(),
                        SubresourceRange {
                            aspects: Aspects::DEPTH,
                            levels: 0..1,
                            layers: 0..1,
                        },
                    )
                    .as_ref(),
            ]
            .drain(..),
            dims,
        );

        Self {
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            dst: TextureRef::clone(dst),
            fence: pool_ref.fence(),
            frame_buf,
            graphics_buf,
            graphics_line: None,
            graphics_mesh_dual_tex: None,
            graphics_mesh_single_tex: None,
            graphics_mesh_trans: None,
            graphics_spotlight: None,
            graphics_sunlight: None,
            line_buf: None,
            #[cfg(debug_assertions)]
            name: name.to_owned(),
            pool: PoolRef::clone(pool),
        }
    }

    /// Sets up the draw op for rendering using the given compilation results by initializing the
    /// required graphics pipeline instances.
    fn with_compilation(&mut self, compilation: &Compilation<'_, '_>) {
        let mut pool = self.pool.borrow_mut();

        // Setup the mesh graphics pipelines
        let sets = compilation.mesh_sets_required();
        if sets.single_tex > 0 {
            self.graphics_mesh_single_tex = Some(pool.graphics_sets(
                #[cfg(debug_assertions)]
                &format!("{} (Mesh/SingleTex)", &self.name),
                GraphicsMode::Mesh(MeshType::SingleTexture),
                RenderPassMode::Draw,
                0,
                sets.single_tex,
            ));
        }
        if sets.dual_tex > 0 {
            self.graphics_mesh_dual_tex = Some(pool.graphics_sets(
                #[cfg(debug_assertions)]
                &format!("{} (Mesh/DualTex)", &self.name),
                GraphicsMode::Mesh(MeshType::DualTexture),
                RenderPassMode::Draw,
                0,
                sets.dual_tex,
            ));
        }
        if sets.trans > 0 {
            self.graphics_mesh_trans = Some(pool.graphics_sets(
                #[cfg(debug_assertions)]
                &format!("{} (Mesh/Trans)", &self.name),
                GraphicsMode::Mesh(MeshType::Transparent),
                RenderPassMode::Draw,
                2,
                sets.trans,
            ));
        }

        // Setup other graphics pipelines
        let stages = compilation.stages_required();
        if stages.contains(Stages::LINE) {
            self.graphics_line = Some(pool.graphics(
                #[cfg(debug_assertions)]
                &format!("{} (Line)", &self.name),
                GraphicsMode::Line,
                RenderPassMode::Draw,
                0,
            ));
            self.line_buf = Some((
                pool.data_usage(
                    #[cfg(debug_assertions)]
                    &format!("{} (Line Buf)", &self.name),
                    compilation.line_buf_len() as _,
                    BufferUsage::STORAGE,
                ),
                0,
            ));
        }
        if stages.contains(Stages::SPOTLIGHT) {
            self.graphics_spotlight = Some(pool.graphics(
                #[cfg(debug_assertions)]
                &format!("{} (Spotlight)", &self.name),
                GraphicsMode::Spotlight,
                RenderPassMode::Draw,
                0,
            ));
        }
        if stages.contains(Stages::SUNLIGHT) {
            self.graphics_sunlight = Some(pool.graphics(
                #[cfg(debug_assertions)]
                &format!("{} (Sunlight)", &self.name),
                GraphicsMode::Sunlight,
                RenderPassMode::Draw,
                0,
            ));
        }
    }

    // TODO: Return slice!
    fn mesh_vertex_push_consts(world_view_proj: Mat4, world: Mat4) -> Vec<u32> {
        let mut res = Vec::with_capacity(100);
        res.extend(&mat4_to_u32_array(world_view_proj));
        res.extend(&mat4_to_mat3_u32_array(world));
        res
    }

    // TODO: Specialize this function for cases where we don't do any 3D work and so we don't need the full g-buffer
    pub fn record<'c>(mut self, camera: &impl Camera, cmds: &mut [Command<'c>]) -> Draw<I> {
        let dims = self.dst.borrow().dims();
        let viewport = Viewport {
            rect: Rect {
                x: 0,
                y: 0,
                w: dims.x as _,
                h: dims.y as _,
            },
            depth: 0.0..1.0,
        };

        // Use a compiler to figure out rendering instructions without allocating
        // memory per rendering command. The compiler caches code between frames.
        let (mut compiler, driver) = {
            let mut pool = self.pool.borrow_mut();
            let driver = Driver::clone(pool.driver());
            let compiler = pool.compiler();

            (compiler, driver)
        };
        let mut instrs = compiler.compile(camera, cmds);

        // Setup our graphics pipelines for these compiled instructions
        self.with_compilation(&instrs);

        unsafe {
            let mut instr = instrs.next().unwrap();

            self.submit_begin(&viewport);

            // Step 1: Lines
            if instr.is_line() {
                self.submit_line_begin(&viewport, instrs.view_proj());

                loop {
                    // This line...
                    let line = instr.as_line().unwrap();
                    self.submit_line(line);

                    // Next line...
                    instr = instrs.next().unwrap();
                    if !instr.is_line() {
                        break;
                    }
                }
            }

            // Step 2: Meshes (...the opaque ones...)
            if instr.is_mesh() {
                self.submit_mesh_begin();

                loop {
                    // This mesh...
                    let mesh = instr.as_mesh().unwrap();
                    self.submit_mesh(mesh);

                    // Next mesh...
                    instr = instrs.next().unwrap();
                    if !instr.is_mesh() {
                        break;
                    }
                }
            }

            // Step 3: Light
            if instr.is_light() {
                self.submit_light_begin();

                loop {
                    // This light...
                    let light = instr.as_light().unwrap();
                    self.submit_light(light);

                    // Next light...
                    instr = instrs.next().unwrap();
                    if !instr.is_light() {
                        break;
                    }
                }
            }

            // Step 4: Meshes (...the transparent ones...)
            if instr.is_mesh() {
                self.submit_mesh_begin();

                loop {
                    // This mesh...
                    let mesh = instr.as_mesh().unwrap();
                    self.submit_mesh(mesh);

                    // Next mesh...
                    instr = instrs.next().unwrap();
                    if !instr.is_mesh() {
                        break;
                    }
                }
            }

            self.submit_finish();
        };

        Draw {
            cmd_pool: self.cmd_pool,
            driver,
            dst: self.dst,
            fence: self.fence,
            frame_buf: self.frame_buf,
        }
    }

    unsafe fn submit_begin(&mut self, viewport: &Viewport) {
        let mut pool = self.pool.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let mut color = self.graphics_buf.color().borrow_mut();
        let mut position = self.graphics_buf.position().borrow_mut();
        let mut normal = self.graphics_buf.normal().borrow_mut();
        let mut material = self.graphics_buf.material().borrow_mut();
        let mut depth = self.graphics_buf.depth().borrow_mut();
        let dims = dst.dims();

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy dst into the color graphics buffer
        dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        color.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            dst.as_ref(),
            Layout::TransferSrcOptimal,
            color.as_ref(),
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

        // Prepare the render pass for mesh rendering
        color.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        position.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        normal.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        material.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        depth.set_layout(
            &mut self.cmd_buf,
            Layout::DepthStencilAttachmentOptimal,
            PipelineStage::LATE_FRAGMENT_TESTS, // TODO: VK_PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT or VK_PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT
            ImageAccess::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );
        self.cmd_buf.begin_render_pass(
            pool.render_pass(RenderPassMode::Draw),
            self.frame_buf.as_ref(),
            viewport.rect,
            vec![&TRANSPARENT_BLACK.into()].drain(..),
            SubpassContents::Inline,
        );
    }

    unsafe fn submit_light_begin(&mut self) {}

    unsafe fn submit_light(&mut self, instr: &LightInstruction) {
        // Step 3: Render sunlight
        // self.cmd_buf.next_subpass(SubpassContents::Inline);
        // if self.cmds[idx].is_sunlight() {
        //     let sunlight = self.sunlight.as_ref().unwrap();

        //     self.cmd_buf.bind_graphics_pipeline(sunlight.pipeline());
        //     bind_graphics_descriptor_set(
        //         &mut self.cmd_buf,
        //         sunlight.layout(),
        //         sunlight.desc_set(0),
        //     );
        //     self.cmd_buf.set_scissors(0, &[self.rect()]);
        //     self.cmd_buf.set_viewports(0, &[self.viewport()]);
        //     loop {
        //         let _ = self.cmds.pop_front();
        //         // self.cmd_buf.push_graphics_constants(
        //         //     self.sunlight.layout(),
        //         //     ShaderStageFlags::VERTEX,
        //         //     0,
        //         //     &mat4_to_u32_array(cmd.world * self.view_proj),
        //         // );
        //         self.cmd_buf.draw(0..6, 0..1);

        //         if !self.cmds[0].is_sunlight() {
        //             break;
        //         }
        //     }
        // }

        // // Step 4: Render spotlights
        // if self.cmds[0].is_spotlight() {
        //     let spotlight = self.spotlight.as_ref().unwrap();

        //     self.cmd_buf.bind_graphics_pipeline(spotlight.pipeline());
        //     bind_graphics_descriptor_set(
        //         &mut self.cmd_buf,
        //         spotlight.layout(),
        //         spotlight.desc_set(0),
        //     );
        //     self.cmd_buf.set_scissors(0, &[self.rect()]);
        //     self.cmd_buf.set_viewports(0, &[self.viewport()]);
        //     loop {
        //         let _ = self.cmds.pop_front();
        //         // self.cmd_buf.push_graphics_constants(
        //         //     self.sunlight.layout(),
        //         //     ShaderStageFlags::VERTEX,
        //         //     0,
        //         //     &mat4_to_u32_array(cmd.world * self.view_proj),
        //         // );
        //         self.cmd_buf.draw(0..6, 0..1);

        //         if !self.cmds[0].is_spotlight() {
        //             break;
        //         }
        //     }
        // }

        // self.cmd_buf.next_subpass(SubpassContents::Inline);
        // idx
    }

    unsafe fn submit_line_begin(&mut self, viewport: &Viewport, view_proj: Mat4) {
        let graphics = self.graphics_line.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            &mat4_to_u32_array(view_proj),
        );
    }

    unsafe fn submit_line<'i>(&mut self, instr: &LineInstruction<'i>) {
        let len: u64 = instr.data.len() as _;
        let vertices = instr.vertices();
        let (ref mut buf, ref mut buf_len) = self.line_buf.as_mut().unwrap();
        let range = *buf_len..*buf_len + len;

        // Copy this line data into the buffer
        buf.map_range_mut(range.clone()).copy_from_slice(instr.data);
        buf.copy_cpu_range(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT,
            BufferAccess::VERTEX_BUFFER_READ,
            range,
        );

        self.cmd_buf.set_line_width(instr.width);
        self.cmd_buf.bind_vertex_buffers(
            0,
            once((
                buf.as_ref().as_ref(),
                SubRange {
                    offset: *buf_len,
                    size: Some(len),
                },
            )),
        );
        self.cmd_buf.draw(0..vertices, 0..1);

        // Advance the buf len value
        *buf_len += len;
    }

    unsafe fn submit_mesh_begin(&mut self) {
        // let mesh = self.mesh.as_ref().unwrap();

        // self.cmd_buf.bind_graphics_pipeline(mesh.pipeline());
        // self.cmd_buf.set_scissors(0, &[self.rect()]);
        // self.cmd_buf.set_viewports(0, &[self.viewport()]);
    }

    unsafe fn submit_mesh_descriptor_set(&mut self, set: usize) {
        // let mesh = self.mesh.as_ref().unwrap();

        // bind_graphics_descriptor_set(&mut self.cmd_buf, mesh.layout(), mesh.desc_set(set));
    }

    unsafe fn submit_mesh(&mut self, instr: &MeshInstruction<'_>) {
        // let mesh = self.mesh.as_ref().unwrap();

        // self.cmd_buf.bind_vertex_buffers(
        //     0,
        //     Some((
        //         cmd.mesh.vertex_buf.as_ref(),
        //         SubRange {
        //             offset: 0,
        //             size: None,
        //         },
        //     )),
        // );
        // self.cmd_buf.push_graphics_constants(
        //     mesh.layout(),
        //     ShaderStageFlags::VERTEX,
        //     0,
        //     Self::mesh_vertex_push_consts(model_view_proj, cmd.model).as_slice(),
        // );
        // self.cmd_buf.push_graphics_constants(
        //     mesh.layout(),
        //     ShaderStageFlags::FRAGMENT,
        //     100,
        //     &[cmd.material],
        // );
        // self.cmd_buf.draw(0..cmd.mesh.vertex_count, 0..1);
    }

    unsafe fn submit_transparency_begin(&mut self) {
        // let transparency = self.transparency.as_ref().unwrap();

        // self.cmd_buf.bind_graphics_pipeline(transparency.pipeline());
        // self.cmd_buf.set_scissors(0, &[self.rect()]);
        // self.cmd_buf.set_viewports(0, &[self.viewport()]);
    }

    unsafe fn submit_transparency_descriptor_set(&mut self, set: usize) {
        // let transparency = self.transparency.as_ref().unwrap();

        // bind_graphics_descriptor_set(
        //     &mut self.cmd_buf,
        //     transparency.layout(),
        //     transparency.desc_set(set),
        // );
    }

    unsafe fn submit_transparency(&mut self, model_view_proj: Mat4, cmd: MeshCommand<'_>) {
        // let transparency = self.transparency.as_ref().unwrap();

        // self.cmd_buf.bind_vertex_buffers(
        //     0,
        //     Some((
        //         cmd.mesh.vertex_buf.as_ref(),
        //         SubRange {
        //             offset: 0,
        //             size: None,
        //         },
        //     )),
        // );
        // self.cmd_buf.push_graphics_constants(
        //     transparency.layout(),
        //     ShaderStageFlags::VERTEX,
        //     0,
        //     Self::mesh_vertex_push_consts(model_view_proj, cmd.model).as_slice(),
        // );
        // self.cmd_buf.push_graphics_constants(
        //     transparency.layout(),
        //     ShaderStageFlags::FRAGMENT,
        //     100,
        //     &[cmd.material],
        // );
        // self.cmd_buf.draw(0..cmd.mesh.vertex_count, 0..1);
    }

    unsafe fn submit_finish(&mut self) {
        let pool = self.pool.borrow();
        let driver = pool.driver();
        let mut dst = self.dst.borrow_mut();
        let mut material = self.graphics_buf.material().borrow_mut();
        let dims = dst.dims();

        // Step 6: Copy the color graphics buffer into dst
        self.cmd_buf.end_render_pass();
        material.set_layout(
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
            material.as_ref(),
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
        driver.borrow_mut().get_queue_mut(QUEUE_TYPE).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(self.fence.as_ref()),
        );
    }

    unsafe fn write_mesh_dual_tex_descriptors(&mut self) {
        // let mut cmds = self.cmds.iter();
        // let graphics = self.mesh.as_ref().unwrap();
        // let mut set = 0;
        // TODO: let mut diffuse_id = None;

        // TODO: while let Some(cmd) = cmds.next().unwrap().as_mesh() {
        //     if let Some(id) = diffuse_id {
        //         if id == cmd.mesh.diffuse_id {
        //             continue;
        //         }
        //     }

        //     let diffuse = cmd.mesh.diffuse.borrow();
        //     let diffuse_view = diffuse.as_default_2d_view();
        //     self.pool
        //         .borrow()
        //         .driver()
        //         .borrow()
        //         .write_descriptor_sets(once(DescriptorSetWrite {
        //             set: graphics.desc_set(set),
        //             binding: 0,
        //             array_offset: 0,
        //             descriptors: once(Descriptor::CombinedImageSampler(
        //                 diffuse_view.as_ref(),
        //                 Layout::ShaderReadOnlyOptimal,
        //                 graphics.sampler(0).as_ref(),
        //             )),
        //         }));

        //     set += 1;
        //     diffuse_id = Some(cmd.mesh.diffuse_id);
        // }
    }

    unsafe fn write_mesh_single_tex_descriptors(&mut self) {
        //let mut cmds = self.cmds.iter();
        // let graphics = self.mesh.as_ref().unwrap();
        // let mut set = 0;
        // TODO: let mut diffuse_id = None;

        // TODO: while let Some(cmd) = cmds.next().unwrap().as_mesh() {
        //     if let Some(id) = diffuse_id {
        //         if id == cmd.mesh.diffuse_id {
        //             continue;
        //         }
        //     }

        //     let diffuse = cmd.mesh.diffuse.borrow();
        //     let diffuse_view = diffuse.as_default_2d_view();
        //     self.pool
        //         .borrow()
        //         .driver()
        //         .borrow()
        //         .write_descriptor_sets(once(DescriptorSetWrite {
        //             set: graphics.desc_set(set),
        //             binding: 0,
        //             array_offset: 0,
        //             descriptors: once(Descriptor::CombinedImageSampler(
        //                 diffuse_view.as_ref(),
        //                 Layout::ShaderReadOnlyOptimal,
        //                 graphics.sampler(0).as_ref(),
        //             )),
        //         }));

        //     set += 1;
        //     diffuse_id = Some(cmd.mesh.diffuse_id);
        // }
    }

    unsafe fn write_mesh_trans_descriptors(&mut self) {
        // let mut cmds = self
        //     .cmds
        //     .iter()
        //     .skip_while(|cmd| !cmd.is_transparency() && !cmd.is_stop());
        // // let graphics = self.transparency.as_ref().unwrap();
        // // let mut set = 0;
        // // TODO: let mut diffuse_id = None;

        // while let Some(cmd) = cmds.next().unwrap().as_mesh() {
        //     // TODO: if let Some(id) = diffuse_id {
        //     //     if id == cmd.mesh.diffuse_id {
        //     //         continue;
        //     //     }
        //     // }

        //     // let color = self.g_buf.color.borrow();
        //     // let depth = self.g_buf.depth.borrow();
        //     // let diffuse = cmd.mesh.diffuse.borrow();
        //     // let color_view = color.as_default_2d_view();
        //     // let diffuse_view = diffuse.as_default_2d_view();
        //     // let depth_view = depth.as_view(
        //     //     ViewKind::D2,
        //     //     Format::D32Sfloat,
        //     //     Default::default(),
        //     //     SubresourceRange {
        //     //         aspects: Aspects::DEPTH,
        //     //         levels: 0..1,
        //     //         layers: 0..1,
        //     //     },
        //     // );
        //     // self.pool
        //     //     .borrow()
        //     //     .driver()
        //     //     .borrow()
        //     //     .write_descriptor_sets(once(DescriptorSetWrite {
        //     //         set: graphics.desc_set(set),
        //     //         binding: 0,
        //     //         array_offset: 0,
        //     //         descriptors: &[
        //     //             Descriptor::CombinedImageSampler(
        //     //                 color_view.as_ref(),
        //     //                 Layout::ShaderReadOnlyOptimal,
        //     //                 graphics.sampler(0).as_ref(),
        //     //             ),
        //     //             Descriptor::CombinedImageSampler(
        //     //                 depth_view.as_ref(),
        //     //                 Layout::ShaderReadOnlyOptimal,
        //     //                 graphics.sampler(0).as_ref(),
        //     //             ),
        //     //         ],
        //     //     }));
        //     // self.pool
        //     //     .borrow()
        //     //     .driver()
        //     //     .borrow()
        //     //     .write_descriptor_sets(once(DescriptorSetWrite {
        //     //         set: graphics.desc_set(set),
        //     //         binding: 1,
        //     //         array_offset: 0,
        //     //         descriptors: once(Descriptor::CombinedImageSampler(
        //     //             diffuse_view.as_ref(),
        //     //             Layout::ShaderReadOnlyOptimal,
        //     //             graphics.sampler(0).as_ref(),
        //     //         )),
        //     //     }));

        //     // set += 1;
        //     // diffuse_id = Some(cmd.mesh.diffuse_id);
        // }
    }
}

#[derive(Clone, Debug)]
pub enum Material {
    Standard,
    Shiny,
    Dull,
    Plastic,
    Metal,
    MetalRusty,
    Water,
    Foliage,
    FoliageDense,
    FoliageGrass,
    Fire,
    Glass,
    GlassDirty,
    Lava,
    Neon,
}

impl Default for Material {
    fn default() -> Self {
        Self::Standard
    }
}
