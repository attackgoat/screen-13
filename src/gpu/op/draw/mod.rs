mod command;
mod compiler;

/// This module houses all the dynamically created meshes used by the drawing code to fulfill user commands.
mod geom;

mod graphics_buf;
mod instruction;
mod key;

pub use self::{command::Command, compiler::Compiler};

use {
    self::{
        geom::LINE_STRIDE,
        graphics_buf::GraphicsBuffer,
        instruction::{Instruction, MeshInstruction},
    },
    super::Op,
    crate::{
        camera::Camera,
        color::{AlphaColor, Color, TRANSPARENT_BLACK},
        gpu::{
            data::CopyRange,
            driver::{CommandPool, Device, Driver, Fence, Framebuffer2d, PhysicalDevice},
            pool::{Graphics, GraphicsMode, Lease, RenderPassMode},
            Data, Mesh, PoolRef, Texture2d, TextureRef,
        },
        math::{Cone, Coord, CoordF, Extent, Mat4, Sphere, Vec3},
    },
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange},
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        format::{Aspects, Format},
        image::{
            Access as ImageAccess, Layout, Offset, SubresourceLayers, SubresourceRange, ViewKind,
        },
        pool::CommandPool as _,
        pso::{PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, Submission},
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

pub struct DrawOp {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    compiler: Lease<Compiler>,
    dst: Texture2d,
    dst_preserve: bool,
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
        let family = Device::queue_family(&driver.borrow());
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
                graphics_buf.color().borrow().as_default_view().as_ref(),
                graphics_buf.position().borrow().as_default_view().as_ref(),
                graphics_buf.normal().borrow().as_default_view().as_ref(),
                graphics_buf.material().borrow().as_default_view().as_ref(),
                graphics_buf
                    .depth()
                    .borrow()
                    .as_view(
                        ViewKind::D2,
                        Format::D32Sfloat,
                        Default::default(),
                        SubresourceRange {
                            aspects: Aspects::DEPTH,
                            ..Default::default()
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
            dst_preserve: false,
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
            #[cfg(debug_assertions)]
            name: name.to_owned(),
            pool: PoolRef::clone(pool),
        }
    }

    /// Preserves the contents of the destination texture. Without calling this function the existing
    /// contents of the destination texture will not be composited into the final result.
    pub fn with_preserve(&mut self) -> &mut Self {
        self.dst_preserve = true;
        self
    }

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
        let dims: Coord = self.dst.borrow().dims().into();
        let viewport = Viewport {
            rect: dims.as_rect_at(Coord::ZERO),
            depth: 0.0..1.0,
        };
        let view_projection = camera.view() * camera.projection();

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
            self.submit_begin(&viewport);

            while let Some(instr) = instrs.next() {
                match instr {
                    Instruction::CopyVertices((buf, ranges)) => {
                        self.submit_vertex_copies(buf, ranges);
                    }
                    Instruction::DrawLines((buf, count)) => {
                        self.submit_draw_lines(buf, count, &viewport, &view_projection);
                    }
                    Instruction::TransferData((src, dst)) => {
                        self.submit_data_transfer(src, dst);
                    }
                    Instruction::WriteVertces((buf, range)) => {
                        self.submit_vertex_write(buf, range);
                    }
                    _ => panic!(),
                }
            }

            self.submit_finish();

            debug!("Done drawing");
        };

        DrawOpSubmission {
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            compiler: self.compiler,
            driver: Driver::clone(self.pool.borrow().driver()),
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

    unsafe fn submit_bind_geom_buffers(&mut self, vertex_buf: &<_Backend as Backend>::Buffer) {
        self.cmd_buf
            .bind_vertex_buffers(0, once((vertex_buf, SubRange::WHOLE)));
    }

    unsafe fn submit_data_transfer(&mut self, src: &mut Data, dst: &mut Data) {
        src.transfer_range(
            &mut self.cmd_buf,
            dst,
            CopyRange {
                dst: 0,
                src: 0..src.capacity(),
            },
        );
    }

    unsafe fn submit_draw_lines(
        &mut self,
        buf: &mut Data,
        count: u32,
        viewport: &Viewport,
        transform: &Mat4,
    ) {
        debug!("Drawing {} lines", count);

        self.graphics_line = Some(self.pool.borrow_mut().graphics(
            #[cfg(debug_assertions)]
            &format!("{} line", &self.name),
            GraphicsMode::DrawLine,
            RenderPassMode::Draw,
            0,
        ));
        let graphics = self.graphics_line.as_ref().unwrap();

        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            LineVertexConsts {
                transform: transform.clone(),
            }
            .as_ref(),
        );
        self.cmd_buf.bind_vertex_buffers(
            0,
            Some((
                buf.as_ref(),
                SubRange {
                    offset: 0,
                    size: Some((count * LINE_STRIDE as u32) as _),
                },
            )),
        );
        self.cmd_buf.draw(0..count, 0..1);
    }

    unsafe fn submit_vertex_copies(&mut self, buf: &mut Data, ranges: &[CopyRange]) {
        buf.copy_ranges(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT,
            BufferAccess::VERTEX_BUFFER_READ,
            ranges,
        );
    }

    unsafe fn submit_vertex_write(&mut self, buf: &mut Data, range: Range<u64>) {
        debug!("Submitting vertex write");
        buf.write_range(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT,
            BufferAccess::VERTEX_BUFFER_READ,
            range,
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
        Device::queue_mut(&mut device).submit(
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
    driver: Driver,
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
        Fence::wait(&self.fence);
    }
}

struct LineInstruction(u32);

#[derive(Clone, Debug)]
pub struct LineCommand([LineVertex; 2]);

#[derive(Clone, Debug)]
struct LineVertex {
    color: AlphaColor,
    pos: Vec3,
}

#[repr(C)]
struct LineVertexConsts {
    transform: Mat4,
}

impl AsRef<[u32; 16]> for LineVertexConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 16] {
        unsafe { &*(self as *const _ as *const _) }
    }
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
    cull_group: usize,
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
