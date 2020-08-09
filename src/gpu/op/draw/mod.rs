mod command;
mod compiler;

/// This module houses all the dynamically created meshes used by the drawing code to fulfill user commands.
mod geom;

mod graphics_buf;
mod instruction;
mod key;

pub use self::{command::Command, compiler::Compiler};

use {
    self::{compiler::Compilation, graphics_buf::GraphicsBuffer, instruction::MeshInstruction},
    super::{wait_for_fence, Op},
    crate::{
        camera::Camera,
        color::{AlphaColor, Color, TRANSPARENT_BLACK},
        gpu::{
            data::CopyRange,
            driver::{CommandPool, Device, Driver, Fence, Framebuffer2d, PhysicalDevice},
            pool::{Graphics, GraphicsMode, Lease, MeshType, RenderPassMode},
            Data, Mesh, PoolRef, Texture2d, TextureRef,
        },
        math::{Cone, Coord, CoordF, Extent, Mat4, Sphere, Vec2, Vec3},
    },
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange, Usage as BufferUsage},
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        format::{Aspects, Format},
        image::{
            Access as ImageAccess, Layout, Offset, SubresourceLayers, SubresourceRange, ViewKind,
        },
        pool::CommandPool as _,
        pso::{PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        iter::{empty, once},
        ops::Range,
    },
};

// TODO: Remove!
const _0: BufferAccess = BufferAccess::MEMORY_WRITE;
const _1: Extent = Extent::ZERO;
const _2: SubRange = SubRange::WHOLE;

const QUEUE_TYPE: QueueType = QueueType::Graphics;

struct CopyInstruction<'a> {
    data: &'a Data,
    ranges: &'a [CopyRange],
}

pub struct DrawOp {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    compiler: Lease<Compiler>,
    dst: Texture2d,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    graphics_buf: GraphicsBuffer,
    graphics_line: Option<Lease<Graphics>>,
    graphics_mesh_animated: Option<Lease<Graphics>>,
    graphics_mesh_dual_tex: Option<Lease<Graphics>>,
    graphics_mesh_single_tex: Option<Lease<Graphics>>,
    graphics_mesh_transparent: Option<Lease<Graphics>>,
    graphics_spotlight: Option<Lease<Graphics>>,
    graphics_sunlight: Option<Lease<Graphics>>,
    line_buf: Option<(Lease<Data>, u64)>, // TODO: Remove the size tuple item
    #[cfg(debug_assertions)]
    name: String,
    pool: PoolRef,
}

impl DrawOp {
    /// # Safety
    /// None
    pub fn new(#[cfg(debug_assertions)] name: &str, pool: &PoolRef, dst: &Texture2d) -> Self {
        let mut pool_ref = pool.borrow_mut();
        let driver = Driver::clone(pool_ref.driver());

        // Allocate the command buffer
        let family = Device::queue_family(&driver.borrow(), QUEUE_TYPE);
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
            compiler: pool_ref.compiler(),
            dst: TextureRef::clone(dst),
            fence: pool_ref.fence(),
            frame_buf,
            graphics_buf,
            graphics_line: None,
            graphics_mesh_animated: None,
            graphics_mesh_dual_tex: None,
            graphics_mesh_single_tex: None,
            graphics_mesh_transparent: None,
            graphics_spotlight: None,
            graphics_sunlight: None,
            line_buf: None,
            #[cfg(debug_assertions)]
            name: name.to_owned(),
            pool: PoolRef::clone(pool),
        }
    }

    // /// Sets up the draw op for rendering using the given compilation results by initializing the
    // /// required graphics pipeline instances.
    // fn with_compilation(&mut self, compilation: &Compilation) {
    //     let mut pool = self.pool.borrow_mut();

    //     // We lazy-load the number of required mesh descriptor sets
    //     let mut mesh_sets = None;
    //     let mut set_mesh_sets = || {
    //         if mesh_sets.is_none() {
    //             mesh_sets = Some(compilation.mesh_sets_required());
    //         };
    //     };

    //     // Setup the graphics pipelines
    //     let stages = compilation.stages_required();
    //     if stages.contains(Stages::MESH_SINGLE_TEX) {
    //         set_mesh_sets();
    //         self.graphics_mesh_single_tex = Some(pool.graphics_sets(
    //             #[cfg(debug_assertions)]
    //             &format!("{} (Mesh/SingleTex)", &self.name),
    //             GraphicsMode::Mesh(MeshType::SingleTexture),
    //             RenderPassMode::Draw,
    //             0,
    //             mesh_sets.as_ref().unwrap().single_tex,
    //         ));
    //     }

    //     if stages.contains(Stages::MESH_DUAL_TEX) {
    //         self.graphics_mesh_dual_tex = Some(pool.graphics_sets(
    //             #[cfg(debug_assertions)]
    //             &format!("{} (Mesh/DualTex)", &self.name),
    //             GraphicsMode::Mesh(MeshType::DualTexture),
    //             RenderPassMode::Draw,
    //             0,
    //             mesh_sets.as_ref().unwrap().dual_tex,
    //         ));
    //     }

    //     if stages.contains(Stages::MESH_TRANSPARENT) {
    //         self.graphics_mesh_transparent = Some(pool.graphics_sets(
    //             #[cfg(debug_assertions)]
    //             &format!("{} (Mesh/Trans)", &self.name),
    //             GraphicsMode::Mesh(MeshType::Transparent),
    //             RenderPassMode::Draw,
    //             2,
    //             mesh_sets.as_ref().unwrap().trans,
    //         ));
    //     }

    //     if stages.contains(Stages::LINE) {
    //         self.graphics_line = Some(pool.graphics(
    //             #[cfg(debug_assertions)]
    //             &format!("{} (Line)", &self.name),
    //             GraphicsMode::Line,
    //             RenderPassMode::Draw,
    //             0,
    //         ));
    //         // // // // self.line_buf = Some((
    //         // // // //     pool.data_usage(
    //         // // // //         #[cfg(debug_assertions)]
    //         // // // //         &format!("{} (Line Buf)", &self.name),
    //         // // // //         compilation.line_buf().len() as _,
    //         // // // //         BufferUsage::STORAGE,
    //         // // // //     ),
    //         // // // //     0,
    //         // // // // ));
    //     }

    //     if stages.contains(Stages::SPOTLIGHT) {
    //         self.graphics_spotlight = Some(pool.graphics(
    //             #[cfg(debug_assertions)]
    //             &format!("{} (Spotlight)", &self.name),
    //             GraphicsMode::Spotlight,
    //             RenderPassMode::Draw,
    //             0,
    //         ));
    //     }

    //     if stages.contains(Stages::SUNLIGHT) {
    //         self.graphics_sunlight = Some(pool.graphics(
    //             #[cfg(debug_assertions)]
    //             &format!("{} (Sunlight)", &self.name),
    //             GraphicsMode::Sunlight,
    //             RenderPassMode::Draw,
    //             0,
    //         ));
    //     }
    // }

    // TODO: Use new method of unsafe as_ref pointer cast
    fn mesh_vertex_push_consts(_world_view_proj: Mat4, _world: Mat4) -> Vec<u32> {
        // let res = Vec::with_capacity(100);
        // // res.extend(&mat4_bits(world_view_proj));
        // // res.extend(&mat4_to_mat3_u32_array(world));
        // res
        todo!();
    }

    // TODO: Returns concrete type instead of impl Op because https://github.com/rust-lang/rust/issues/42940
    pub fn record<'c>(
        mut self,
        camera: &impl Camera,
        cmds: &'c mut [Command<'c>],
    ) -> DrawOpSubmission {
        // HACK: Hiding these warnings for now in the most I-will-remember-to-remove-later way
        let _ = ShaderStageFlags::empty();
        let _ = BufferUsage::STORAGE;
        let _ = Vec2::zero();

        let dims: Coord = self.dst.borrow().dims().into();
        let viewport = Viewport {
            rect: dims.as_rect_at(Coord::ZERO),
            depth: 0.0..1.0,
        };

        // Use a compiler to figure out rendering instructions without allocating
        // memory per rendering command. The compiler caches code between frames.
        let mut compiler = self.pool.borrow_mut().compiler();
        let mut instrs = compiler.compile(
            #[cfg(debug_assertions)]
            &self.name,
            &self.pool,
            camera,
            cmds,
        );

        unsafe {
            // NOTE: There will always be at least one instruction (Stop)
            let mut _instr = instrs.next().unwrap();

            self.submit_begin(&viewport);

            // // Step 1: Opaque meshes (single and dual texture)
            // if instr.is_mesh() {
            //     self.submit_mesh_begin();

            //     loop {
            //         // This mesh...
            //         let mesh = instr.as_mesh().unwrap();
            //         self.submit_mesh(mesh);

            //         // Next mesh...
            //         instr = instrs.next().unwrap();
            //         if !instr.is_mesh() {
            //             break;
            //         }
            //     }
            // }

            // // Step 4: Light
            // if instr.is_light() {
            //     self.submit_light_begin();

            //     loop {
            //         // This light...
            //         let light = instr.as_light().unwrap();
            //         self.submit_light(light);

            //         // Next light...
            //         instr = instrs.next().unwrap();
            //         if !instr.is_light() {
            //             break;
            //         }
            //     }
            // }

            // // Step 5: Transparent meshes
            // if instr.is_mesh() {
            //     self.submit_mesh_begin();

            //     loop {
            //         // This mesh...
            //         let mesh = instr.as_mesh().unwrap();
            //         self.submit_mesh(mesh);

            //         // Next mesh...
            //         instr = instrs.next().unwrap();
            //         if !instr.is_mesh() {
            //             break;
            //         }
            //     }
            // }

            // // Step 2: Lines
            // if instr.is_line() {
            //     self.submit_line_begin(&viewport, instrs.view_proj());

            //     loop {
            //         // This line...
            //         let line = instr.as_line().unwrap();
            //         self.submit_line(line);

            //         // Next line...
            //         instr = instrs.next().unwrap();
            //         if !instr.is_line() {
            //             break;
            //         }
            //     }
            // }

            self.submit_finish();
        };

        let line_buf = if let Some((line_buf, _)) = self.line_buf {
            Some(line_buf)
        } else {
            None
        };

        DrawOpSubmission {
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            compiler: self.compiler,
            dst: self.dst,
            fence: self.fence,
            frame_buf: self.frame_buf,
            graphics_buf: self.graphics_buf,
            graphics_line: self.graphics_line,
            graphics_mesh_animated: self.graphics_mesh_animated,
            graphics_mesh_dual_tex: self.graphics_mesh_dual_tex,
            graphics_mesh_single_tex: self.graphics_mesh_single_tex,
            graphics_mesh_transparent: self.graphics_mesh_transparent,
            graphics_spotlight: self.graphics_spotlight,
            graphics_sunlight: self.graphics_sunlight,
            line_buf,
            pool: self.pool,
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
                extent: dims.as_extent_with_depth(1),
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

    //unsafe fn submit_light(&mut self, _instr: &LightInstruction) {
    //   let _ = ShaderStageFlags::VERTEX;

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
    //}

    unsafe fn submit_line_begin(&mut self, viewport: &Viewport, _view_proj: Mat4) {
        let graphics = self.graphics_line.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
        // self.cmd_buf.push_graphics_constants(
        //     graphics.layout(),
        //     ShaderStageFlags::VERTEX,
        //     0,
        //     &mat4_bits(view_proj),
        // );
    }

    //unsafe fn submit_line_width<'i>(&mut self, _instr: &LineInstruction<'i>) {}

    //unsafe fn submit_line<'i>(&mut self, _instr: &LineInstruction<'i>) {
    // let len: u64 = instr.data.len() as _;
    // let vertices = instr.vertices();
    // let (ref mut buf, ref mut buf_len) = self.line_buf.as_mut().unwrap();
    // let range = *buf_len..*buf_len + len;

    // // Copy this line data into the buffer
    // buf.map_range_mut(range.clone()).copy_from_slice(instr.data); // TOOD: flush when done!
    // buf.copy_cpu_range(
    //     &mut self.cmd_buf,
    //     PipelineStage::VERTEX_INPUT,
    //     BufferAccess::VERTEX_BUFFER_READ,
    //     range,
    // );

    // self.cmd_buf.set_line_width(instr.width);
    // self.cmd_buf.bind_vertex_buffers(
    //     0,
    //     once((
    //         &*buf.as_ref(),
    //         SubRange {
    //             offset: *buf_len,
    //             size: Some(len),
    //         },
    //     )),
    // );
    // self.cmd_buf.draw(0..vertices, 0..1);

    // // Advance the buf len value
    // *buf_len += len;
    //}

    unsafe fn submit_mesh_begin(&mut self) {
        // let mesh = self.mesh.as_ref().unwrap();

        // self.cmd_buf.bind_graphics_pipeline(mesh.pipeline());
        // self.cmd_buf.set_scissors(0, &[self.rect()]);
        // self.cmd_buf.set_viewports(0, &[self.viewport()]);
    }

    unsafe fn submit_mesh_descriptor_set(&mut self, _set: usize) {
        // let mesh = self.mesh.as_ref().unwrap();

        // bind_graphics_descriptor_set(&mut self.cmd_buf, mesh.layout(), mesh.desc_set(set));
    }

    unsafe fn submit_mesh(&mut self, _instr: &MeshInstruction<'_>) {
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

    unsafe fn submit_transparency_descriptor_set(&mut self, _set: usize) {
        // let transparency = self.transparency.as_ref().unwrap();

        // bind_graphics_descriptor_set(
        //     &mut self.cmd_buf,
        //     transparency.layout(),
        //     transparency.desc_set(set),
        // );
    }

    unsafe fn submit_transparency(&mut self, _model_view_proj: Mat4, _cmd: MeshCommand<'_>) {
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
        let mut device = driver.borrow_mut();
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
                extent: dims.as_extent_with_depth(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        Device::queue_mut(&mut device, QUEUE_TYPE).submit(
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

pub struct DrawOpSubmission {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    compiler: Lease<Compiler>,
    dst: Texture2d,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    graphics_buf: GraphicsBuffer,
    graphics_line: Option<Lease<Graphics>>,
    graphics_mesh_animated: Option<Lease<Graphics>>,
    graphics_mesh_dual_tex: Option<Lease<Graphics>>,
    graphics_mesh_single_tex: Option<Lease<Graphics>>,
    graphics_mesh_transparent: Option<Lease<Graphics>>,
    graphics_spotlight: Option<Lease<Graphics>>,
    graphics_sunlight: Option<Lease<Graphics>>,
    line_buf: Option<Lease<Data>>,
    pool: PoolRef,
}

impl Drop for DrawOpSubmission {
    fn drop(&mut self) {
        self.wait();

        // Causes the compiler to drop internal caches which store texture refs; they were being held
        // alive there so that they could not be dropped until we finished GPU execution
        self.compiler.reset();
    }
}

impl Op for DrawOpSubmission {
    fn wait(&self) {
        let pool = self.pool.borrow();
        let device = pool.driver().borrow();

        unsafe {
            wait_for_fence(&device, &self.fence);
        }
    }
}

#[derive(Clone, Debug)]
pub struct LineCommand {
    vertices: [LineVertex; 2],
    width: f32,
}

#[derive(Clone, Debug)]
struct LineVertex {
    color: AlphaColor,
    pos: Vec3,
}

#[derive(Clone, Copy)]
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

// TODO: cast_shadows, receive_shadows, ambient?
#[derive(Clone)]
pub struct MeshCommand<'m> {
    camera_z: f32,
    material: Material,
    mesh: &'m Mesh,
    transform: Mat4,
}

pub struct MeshDrawInstruction<'i> {
    material: u32,
    mesh: &'i Mesh,
    transform: &'i [u8],
}

#[derive(Clone, Debug)]
pub struct PointLightCommand {
    core: Sphere,  // full-bright center and radius
    color: Color,  // `core` and penumbra-to-transparent color
    penumbra: f32, // distance after `core` which fades from `color` to transparent
    power: f32, // sRGB power value, normalized to current gamma so 1.0 == a user setting of 1.2 and 2.0 == 2.4
}

impl PointLightCommand {
    /// Returns a tightly fitting sphere around the lit area of this point light, including the penumbra
    pub(self) fn bounds(&self) -> Sphere {
        self.core + self.penumbra
    }
}

#[derive(Clone, Debug)]
pub struct RectLightCommand {
    color: Color, // full-bright and penumbra-to-transparent color
    dims: CoordF,
    radius: f32, // size of the penumbra area beyond the box formed by `pos` and `range` which fades from `color` to transparent
    pos: Vec3,   // top-left corner when viewed from above
    power: f32, // sRGB power value, normalized to current gamma so 1.0 == a user setting of 1.2 and 2.0 == 2.4
    range: f32, // distance from `pos` to the bottom of the rectangular light
}

impl RectLightCommand {
    /// Returns a tightly fitting sphere around the lit area of this rectangular light, including the penumbra
    pub(self) fn bounds(&self) -> Sphere {
        todo!();
    }
}

#[derive(Clone, Debug)]
pub struct SunlightCommand {
    color: Color, // uniform color for any area exposed to the sunlight
    normal: Vec3, // direction which the sunlight shines
    power: f32, // sRGB power value, normalized to current gamma so 1.0 == a user setting of 1.2 and 2.0 == 2.4
}

#[derive(Clone, Debug)]
pub struct SpotlightCommand {
    color: Color,         // `cone` and penumbra-to-transparent color
    cone_radius: f32, // radius of the spotlight cone from the center to the edge of the full-bright area
    normal: Vec3,     // direction from `pos` which the spotlight shines
    penumbra_radius: f32, // Additional radius beyond `cone_radius` which fades from `color` to transparent
    pos: Vec3,            // position of the pointy end
    power: f32, // sRGB power value, normalized to current gamma so 1.0 == a user setting of 1.2 and 2.0 == 2.4
    range: Range<f32>, // lit distance from `pos` and to the bottom of the spotlight (does not account for the lens-shaped end)
    top_radius: f32,
}

impl SpotlightCommand {
    /// Returns a tightly fitting cone around the lit area of this spotlight, including the penumbra and
    /// lens-shaped base.
    pub(self) fn bounds(&self) -> Cone {
        Cone::new(
            self.pos,
            self.normal,
            self.range.end,
            self.cone_radius + self.penumbra_radius,
        )
    }
}
