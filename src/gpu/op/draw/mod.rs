mod command;
mod compiler;

/// This module houses all the dynamically created meshes used by the drawing code to fulfill user commands.
mod geom;

mod geom_buf;
mod instruction;
mod key;

pub use self::{command::Command, compiler::Compiler};

use {
    self::{
        compiler::{SunlightIter, VertexBuffers},
        geom::{LINE_STRIDE, POINT_LIGHT, RECT_LIGHT_STRIDE, SPOTLIGHT_STRIDE},
        geom_buf::GeometryBuffer,
        instruction::{
            DataComputeInstruction, DataCopyInstruction, DataTransferInstruction,VertexAttrsDescriptorsInstruction,
            DataWriteInstruction, DataWriteRefInstruction, Instruction, LightBindInstruction,
            LineDrawInstruction, MeshBindInstruction, MeshDrawInstruction,VertexAttrsBeginInstruction,
            PointLightDrawInstruction, RectLightDrawInstruction, SpotlightDrawInstruction,
        },
    },
    super::Op,
    crate::{
        camera::Camera,
        color::AlphaColor,
        gpu::{
            CalcVertexAttrsComputeMode,
            data::CopyRange,
            driver::{
                bind_compute_descriptor_set, bind_graphics_descriptor_set, CommandPool,
                ComputePipeline, Device, Driver, Fence, Framebuffer2d,
            },
            pool::{Lease, Pool},
            BitmapRef, Compute, ComputeMode, DrawRenderPassMode, Graphics, GraphicsMode,
            RenderPassMode, Texture2d, TextureRef,
        },
        pak::IndexType,
        math::{Coord, Mat4, Vec2, Vec3},
    },
    gfx_hal::{
        adapter::PhysicalDevice as _,
        buffer::{Access as BufferAccess, IndexBufferView, SubRange},
        command::{
            ClearColor, ClearDepthStencil, ClearValue, CommandBuffer as _, CommandBufferFlags,
            ImageCopy, Level, SubpassContents,
        },
        device::Device as _,
        format::Aspects,
        image::{
            Access as ImageAccess, Layout, Offset, SubresourceLayers, SubresourceRange, ViewKind,
        },
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        cmp::Ordering,
        hash::{Hash, Hasher},
        iter::{empty, once},
    },
};

#[repr(C)]
struct CalcVertexAttrsConsts {
    offset: u32,
}

impl AsRef<[u32; 1]> for CalcVertexAttrsConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 1] {
        unsafe { &*(self as *const Self as *const [u32; 1]) }
    }
}

pub struct DrawOp<'a> {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    compute_u16_vertex_attrs: Option<Lease<Compute>>,
    compute_u16_skin_vertex_attrs: Option<Lease<Compute>>,
    compute_u32_vertex_attrs: Option<Lease<Compute>>,
    compute_u32_skin_vertex_attrs: Option<Lease<Compute>>,
    driver: Driver,
    dst: Texture2d,
    dst_preserve: bool,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    geom_buf: GeometryBuffer,
    graphics_line: Option<Lease<Graphics>>,
    graphics_mesh: Option<Lease<Graphics>>,
    graphics_mesh_anim: Option<Lease<Graphics>>,
    graphics_point_light: Option<Lease<Graphics>>,
    graphics_rect_light: Option<Lease<Graphics>>,
    graphics_spotlight: Option<Lease<Graphics>>,
    graphics_sunlight: Option<Lease<Graphics>>,
    mode: DrawRenderPassMode,

    #[cfg(debug_assertions)]
    name: String,

    pool: &'a mut Pool,
}

impl<'a> DrawOp<'a> {
    /// # Safety
    /// None
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        pool: &'a mut Pool,
        dst: &Texture2d,
    ) -> Self {
        // Allocate the command buffer
        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(&driver, family);

        // The geometry buffer will share size and output format with the destination texture
        let (dims, fmt) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };
        let geom_buf = GeometryBuffer::new(
            #[cfg(debug_assertions)]
            name,
            &driver,
            pool,
            dims,
            fmt,
        );

        let (frame_buf, mode) = {
            let color_metal = geom_buf.color_metal.borrow();
            let depth = geom_buf.depth.borrow();
            let light = geom_buf.light.borrow();
            let normal_rough = geom_buf.normal_rough.borrow();
            let output = geom_buf.output.borrow();

            let mode = DrawRenderPassMode {
                depth: depth.format(),
                geom_buf: color_metal.format(),
                light: light.format(),
                output: output.format(),
            };

            // Setup the framebuffer
            let frame_buf = Framebuffer2d::new(
                #[cfg(debug_assertions)]
                &name,
                Driver::clone(&driver),
                pool.render_pass(&driver, RenderPassMode::Draw(mode)),
                vec![
                    color_metal.as_default_view().as_ref(),
                    normal_rough.as_default_view().as_ref(),
                    light.as_default_view().as_ref(),
                    output.as_default_view().as_ref(),
                    depth
                        .as_view(
                            ViewKind::D2,
                            mode.depth,
                            Default::default(),
                            SubresourceRange {
                                aspects: Aspects::DEPTH,
                                ..Default::default()
                            },
                        )
                        .as_ref(),
                ],
                dims,
            );

            (frame_buf, mode)
        };
        let fence = pool.fence(
            #[cfg(debug_assertions)]
            name,
            &driver,
        );

        Self {
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            compute_u16_vertex_attrs: None,
            compute_u16_skin_vertex_attrs: None,
            compute_u32_vertex_attrs: None,
            compute_u32_skin_vertex_attrs: None,
            driver,
            dst: TextureRef::clone(dst),
            dst_preserve: false,
            fence,
            frame_buf,
            geom_buf,
            graphics_line: None,
            graphics_mesh: None,
            graphics_mesh_anim: None,
            graphics_point_light: None,
            graphics_rect_light: None,
            graphics_spotlight: None,
            graphics_sunlight: None,
            mode,

            #[cfg(debug_assertions)]
            name: name.to_owned(),

            pool,
        }
    }

    /// Preserves the contents of the destination texture. Without calling this function the existing
    /// contents of the destination texture will not be composited into the final result.
    pub fn with_preserve(&mut self) -> &mut Self {
        self.dst_preserve = true;
        self
    }

    // TODO: Returns concrete type instead of impl Op because https://github.com/rust-lang/rust/issues/42940
    pub fn record(mut self, camera: &impl Camera, cmds: &mut [Command]) -> DrawOpSubmission {
        // Use a compiler to figure out rendering instructions without allocating
        // memory per rendering command. The compiler caches code between frames.
        let mut compiler = self.pool.compiler();
        {
            let mut instrs = compiler.compile(
                #[cfg(debug_assertions)]
                &self.name,
                &self.driver,
                &mut self.pool,
                camera,
                cmds,
            );

            // Setup compute and graphics pipelines and with their descriptor sets
            {
                // Material descriptors for PBR rendering (Color+Normal+Metal/Rough)
                let descriptors = instrs.materials();
                let desc_sets = descriptors.len();
                if desc_sets > 0 {
                    let graphics = self.pool.graphics_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        GraphicsMode::DrawMesh,
                        RenderPassMode::Draw(self.mode),
                        0,
                        desc_sets,
                    );
                    let device = self.driver.borrow();

                    unsafe {
                        Self::write_material_descriptors(&device, &graphics, descriptors);
                    }

                    self.graphics_mesh = Some(graphics);
                }

                if instrs.contains_point_light() {
                    self.graphics_point_light = Some(self.pool.graphics_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        GraphicsMode::DrawPointLight,
                        RenderPassMode::Draw(self.mode),
                        1,
                        0,
                    ));
                }

                if instrs.contains_rect_light() {
                    self.graphics_rect_light = Some(self.pool.graphics_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        GraphicsMode::DrawRectLight,
                        RenderPassMode::Draw(self.mode),
                        1,
                        0,
                    ));
                }

                if instrs.contains_spotlight() {
                    self.graphics_spotlight = Some(self.pool.graphics_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        GraphicsMode::DrawSpotlight,
                        RenderPassMode::Draw(self.mode),
                        1,
                        0,
                    ));
                }

                if instrs.contains_sunlight() {
                    self.graphics_sunlight = Some(self.pool.graphics_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        GraphicsMode::DrawSunlight,
                        RenderPassMode::Draw(self.mode),
                        1,
                        0,
                    ));
                }

                // Buffer descriptors for calculation of u16-indexed vertex attributes
                let descriptors = instrs.u16_vertex_bufs();
                let desc_sets = descriptors.len();
                if desc_sets > 0 {
                    let compute = self.pool.compute_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        ComputeMode::CalcVertexAttrs(CalcVertexAttrsComputeMode {
                            idx_ty: IndexType::U16,
                            skin: false,
                        }),
                        desc_sets,
                    );
                    let device = self.driver.borrow();

                    unsafe {
                        Self::write_vertex_descriptors(&device, &compute, descriptors);
                    }

                    self.compute_u16_vertex_attrs = Some(compute);
                }

                // Buffer descriptors for calculation of u16-indexed skinned vertex attributes
                let descriptors = instrs.u16_skin_vertex_bufs();
                let desc_sets = descriptors.len();
                if desc_sets > 0 {
                    let compute = self.pool.compute_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        ComputeMode::CalcVertexAttrs(CalcVertexAttrsComputeMode {
                            idx_ty: IndexType::U16,
                            skin: true,
                        }),
                        desc_sets,
                    );
                    let device = self.driver.borrow();

                    unsafe {
                        Self::write_vertex_descriptors(&device, &compute, descriptors);
                    }

                    self.compute_u16_skin_vertex_attrs = Some(compute);
                }

                // Buffer descriptors for calculation of u32-indexed vertex attributes
                let descriptors = instrs.u32_vertex_bufs();
                let desc_sets = descriptors.len();
                if desc_sets > 0 {
                    let compute = self.pool.compute_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        ComputeMode::CalcVertexAttrs(CalcVertexAttrsComputeMode {
                            idx_ty: IndexType::U32,
                            skin: false,
                        }),
                        desc_sets,
                    );
                    let device = self.driver.borrow();

                    unsafe {
                        Self::write_vertex_descriptors(&device, &compute, descriptors);
                    }

                    self.compute_u32_vertex_attrs = Some(compute);
                }

                // Buffer descriptors for calculation of u32-indexed skinned vertex attributes
                let descriptors = instrs.u32_vertex_bufs();
                let desc_sets = descriptors.len();
                if desc_sets > 0 {
                    let compute = self.pool.compute_desc_sets(
                        #[cfg(debug_assertions)]
                        &self.name,
                        &self.driver,
                        ComputeMode::CalcVertexAttrs(CalcVertexAttrsComputeMode {
                            idx_ty: IndexType::U32,
                            skin: true,
                        }),
                        desc_sets,
                    );
                    let device = self.driver.borrow();

                    unsafe {
                        Self::write_vertex_descriptors(&device, &compute, descriptors);
                    }

                    self.compute_u32_skin_vertex_attrs = Some(compute);
                }
            }

            if !instrs.is_empty() {
                let view_proj = camera.projection() * camera.view();
                let dims: Coord = self.dst.borrow().dims().into();
                let viewport = Viewport {
                    rect: dims.as_rect_at(Coord::ZERO),
                    depth: 0.0..1.0,
                };

                unsafe {
                    self.submit_begin(&viewport);

                    while let Some(instr) = instrs.next() {
                        match instr {
                            Instruction::DataTransfer(instr) => self.submit_data_transfer(instr),
                            Instruction::IndexWriteRef(instr) => self.submit_index_write_ref(instr),
                            Instruction::LightBegin => self.submit_light_begin(),
                            Instruction::LightBind(instr) => self.submit_light_bind(instr),
                            Instruction::LineDraw(instr) => {
                                self.submit_lines(instr, &viewport, view_proj)
                            }
                            Instruction::MeshBegin => self.submit_mesh_begin(&viewport),
                            Instruction::MeshBind(instr) => self.submit_mesh_bind(instr),
                            Instruction::MeshDescriptors(set) => self.submit_mesh_descriptors(set),
                            Instruction::MeshDraw(instr) => self.submit_mesh(instr, view_proj),
                            Instruction::PointLightDraw(instr) => {
                                self.submit_point_lights(instr, &viewport, view_proj)
                            }
                            Instruction::RectLightBegin => self.submit_rect_light_begin(&viewport),
                            Instruction::RectLightDraw(instr) => {
                                self.submit_rect_light(instr, view_proj)
                            }
                            Instruction::SpotlightBegin => self.submit_spotlight_begin(&viewport),
                            Instruction::SpotlightDraw(instr) => {
                                self.submit_spotlight(instr, view_proj)
                            }
                            Instruction::SunlightBegin => self.submit_sunlight_begin(&viewport),
                            Instruction::SunlightDraw(instr) => self.submit_sunlights(instr),
                            Instruction::VertexAttrsBegin(instr) => self.submit_vertex_attrs_begin(instr),
                            Instruction::VertexAttrsCalc(instr) => {
                                self.submit_vertex_attrs_calc(instr)
                            }
                            Instruction::VertexAttrsDescriptors(instr) => {
                                self.submit_vertex_attrs_descriptors(instr)
                            }
                            Instruction::VertexCopy(instr) => self.submit_vertex_copies(instr),
                            Instruction::VertexWrite(instr) => self.submit_vertex_write(instr),
                            Instruction::VertexWriteRef(instr) => {
                                self.submit_vertex_write_ref(instr)
                            }
                        }
                    }

                    self.submit_finish();
                }
            }
        }

        DrawOpSubmission {
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            compiler,
            compute_u16_vertex_attrs: self.compute_u16_vertex_attrs,
            compute_u32_vertex_attrs: self.compute_u32_vertex_attrs,
            dst: self.dst,
            fence: self.fence,
            frame_buf: self.frame_buf,
            geom_buf: self.geom_buf,
            graphics_line: self.graphics_line,
            graphics_mesh: self.graphics_mesh,
            graphics_mesh_anim: self.graphics_mesh_anim,
            graphics_point_light: self.graphics_point_light,
            graphics_spotlight: self.graphics_spotlight,
            graphics_sunlight: self.graphics_sunlight,
        }
    }

    unsafe fn submit_begin(&mut self, viewport: &Viewport) {
        trace!("submit_begin");

        let mut dst = self.dst.borrow_mut();
        let mut color_metal = self.geom_buf.color_metal.borrow_mut();
        let mut normal_rough = self.geom_buf.normal_rough.borrow_mut();
        let mut light = self.geom_buf.light.borrow_mut();
        let mut output = self.geom_buf.output.borrow_mut();
        let mut depth = self.geom_buf.depth.borrow_mut();
        let dims = dst.dims();
        let depth_clear = ClearValue {
            depth_stencil: ClearDepthStencil {
                depth: 1.0,
                stencil: 0,
            },
        };
        let light_clear = ClearValue {
            color: ClearColor {
                float32: [0.0, 0.0, 0.0, 0.0],
            }, // f32::NAN?
        };

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Optional Step 1: Copy dst into the color render target
        if self.dst_preserve {
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::TransferSrcOptimal,
                PipelineStage::TRANSFER,
                ImageAccess::TRANSFER_READ,
            );
            color_metal.set_layout(
                &mut self.cmd_buf,
                Layout::TransferDstOptimal,
                PipelineStage::TRANSFER,
                ImageAccess::TRANSFER_WRITE,
            );
            self.cmd_buf.copy_image(
                dst.as_ref(),
                Layout::TransferSrcOptimal,
                color_metal.as_ref(),
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
        }

        // Prepare the render pass for mesh rendering
        color_metal.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        normal_rough.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        light.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        output.set_layout(
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
            self.pool
                .render_pass(&self.driver, RenderPassMode::Draw(self.mode)),
            self.frame_buf.as_ref(),
            viewport.rect,
            &[depth_clear, light_clear],
            SubpassContents::Inline,
        );
    }

    unsafe fn submit_data_transfer(&mut self, instr: DataTransferInstruction) {
        trace!("submit_data_transfer");

        instr.src.transfer_range(
            &mut self.cmd_buf,
            instr.dst,
            CopyRange {
                src: instr.src_range,
                dst: 0,
            },
        );
    }

    unsafe fn submit_index_write_ref(&mut self, mut instr: DataWriteRefInstruction) {
        trace!("submit_index_write_ref");

        instr.buf.write_range(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT, // TODO: Should be DRAW_INDIRECT?
            BufferAccess::INDEX_BUFFER_READ,
            instr.range,
        );
    }

    unsafe fn submit_lines(
        &mut self,
        instr: LineDrawInstruction,
        viewport: &Viewport,
        transform: Mat4,
    ) {
        trace!("submit_lines");

        let render_pass_mode = RenderPassMode::Draw(self.mode);
        let graphics = self.pool.graphics(
            #[cfg(debug_assertions)]
            &format!("{} line", &self.name),
            &self.driver,
            GraphicsMode::DrawLine,
            render_pass_mode,
            0,
        );

        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            LineVertexConsts { transform }.as_ref(),
        );
        self.cmd_buf.bind_vertex_buffers(
            0,
            Some((
                instr.buf.as_ref(),
                SubRange {
                    offset: 0,
                    size: Some((instr.line_count * LINE_STRIDE as u32) as _),
                },
            )),
        );
        self.cmd_buf.draw(0..instr.line_count, 0..1);

        self.graphics_line = Some(graphics);
    }

    unsafe fn submit_light_begin(&mut self) {
        trace!("submit_light_begin");

        self.cmd_buf.next_subpass(SubpassContents::Inline);
    }

    unsafe fn submit_light_bind(&mut self, instr: LightBindInstruction) {
        trace!("submit_light_bind");

        self.cmd_buf.bind_vertex_buffers(
            0,
            once((
                instr.buf.as_ref(),
                SubRange {
                    offset: 0,
                    size: Some(instr.buf_len),
                },
            )),
        );
    }

    unsafe fn submit_mesh_begin(&mut self, viewport: &Viewport) {
        trace!("submit_mesh_begin");

        let graphics = self.graphics_mesh.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
    }

    unsafe fn submit_mesh_bind(&mut self, instr: MeshBindInstruction<'_>) {
        trace!("submit_mesh_bind");

        self.cmd_buf.bind_index_buffer(IndexBufferView {
            buffer: instr.idx_buf.as_ref(),
            index_type: instr.idx_ty.into(),
            range: SubRange {
                offset: 0,
                size: Some(instr.idx_buf_len),
            },
        });
        self.cmd_buf.bind_vertex_buffers(
            0,
            once((
                instr.vertex_buf.as_ref(),
                SubRange {
                    offset: 0,
                    size: Some(instr.vertex_buf_len),
                },
            )),
        );
    }

    unsafe fn submit_mesh_descriptors(&mut self, desc_set: usize) {
        trace!("submit_mesh_descriptors");

        let graphics = self.graphics_mesh.as_ref().unwrap();
        let desc_set = graphics.desc_set(desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
    }

    unsafe fn submit_mesh(&mut self, instr: MeshDrawInstruction, view_proj: Mat4) {
        trace!("submit_mesh");

        let graphics = self.graphics_mesh.as_ref().unwrap();
        let layout = graphics.layout();
        let world_view_proj = view_proj * instr.transform;

        for mesh in instr.meshes.filter(|mesh| !mesh.is_animated()) {
            let world_view_proj = if let Some(transform) = mesh.transform() {
                world_view_proj * transform
            } else {
                world_view_proj
            };

            self.cmd_buf.push_graphics_constants(
                layout,
                ShaderStageFlags::VERTEX,
                0,
                Mat4Const(world_view_proj).as_ref(),
            );

            self.cmd_buf.draw_indexed(mesh.indices(), 0, 0..1);
        }
    }

    unsafe fn submit_point_lights(
        &mut self,
        instr: PointLightDrawInstruction,
        viewport: &Viewport,
        view_proj: Mat4,
    ) {
        trace!("submit_point_lights");

        const POINT_LIGHT_DRAW_COUNT: u32 = POINT_LIGHT.len() as u32 / 12;

        let graphics = self.graphics_point_light.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[viewport.rect]); // TODO: Not sure this is needed!
        self.cmd_buf.set_viewports(0, &[viewport.clone()]); // TODO: Not sure this is needed!
        self.cmd_buf.bind_vertex_buffers(
            0,
            once((
                instr.buf.as_ref(),
                SubRange {
                    offset: 0,
                    size: Some(POINT_LIGHT.len() as _),
                },
            )),
        );

        for light in instr.lights {
            let world_view_proj = view_proj * Mat4::from_translation(light.center);

            self.cmd_buf.push_graphics_constants(
                graphics.layout(),
                ShaderStageFlags::VERTEX,
                0,
                Mat4Const(world_view_proj).as_ref(),
            );
            self.cmd_buf.push_graphics_constants(
                graphics.layout(),
                ShaderStageFlags::VERTEX,
                0,
                PointLightConsts {
                    intensity: light.color.to_rgb() * light.lumens,
                    radius: light.radius,
                }
                .as_ref(),
            );
            self.cmd_buf.draw(0..POINT_LIGHT_DRAW_COUNT, 0..1);
        }
    }

    unsafe fn submit_rect_light_begin(&mut self, viewport: &Viewport) {
        trace!("submit_rect_light_begin");

        let graphics = self.graphics_rect_light.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
    }

    unsafe fn submit_rect_light(&mut self, instr: RectLightDrawInstruction, view_proj: Mat4) {
        trace!("submit_rect_light");

        const RECT_LIGHT_DRAW_COUNT: u32 = RECT_LIGHT_STRIDE as u32 / 12;

        let graphics = self.graphics_rect_light.as_ref().unwrap();

        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::FRAGMENT,
            0,
            RectLightConsts {
                dims: instr.light.dims.into(),
                intensity: instr.light.color.to_rgb() * instr.light.lumens,
                normal: instr.light.normal,
                position: instr.light.position,
                radius: instr.light.radius,
                range: instr.light.range,
                view_proj,
            }
            .as_ref(),
        );

        self.cmd_buf
            .draw(instr.offset..instr.offset + RECT_LIGHT_DRAW_COUNT, 0..1);
    }

    unsafe fn submit_spotlight_begin(&mut self, viewport: &Viewport) {
        trace!("submit_spotlight_begin");

        let graphics = self.graphics_spotlight.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
    }

    unsafe fn submit_spotlight(&mut self, instr: SpotlightDrawInstruction, view_proj: Mat4) {
        trace!("submit_spotlight");

        const SPOTLIGHT_DRAW_COUNT: u32 = SPOTLIGHT_STRIDE as u32 / 12;

        let graphics = self.graphics_spotlight.as_ref().unwrap();

        /*let up = Vec3::unit_z();
        let light_view = Mat4::look_at_rh(e.position, e.position + e.normal, up);
        let light_space =
            Mat4::perspective_rh_gl(2.0 * e.cutoff_outer, 1.0, 1.0, 35.0) * light_view;
        let cutoff_inner = e.cutoff_inner.cos();
        let cutoff_outer = e.cutoff_outer.cos();
        draw_commands.push(
            SpotlightCommand {
                anormal: -e.normal,
                cutoff_inner,
                cutoff_outer,
                diffuse: e.diffuse,
                position: e.position,
                power: e.power,
                light_space,
            }
            .into(),
        );*/

        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::FRAGMENT,
            0,
            Mat4Const(view_proj).as_ref(),
        );

        self.cmd_buf
            .draw(instr.offset..instr.offset + SPOTLIGHT_DRAW_COUNT, 0..1);
    }

    unsafe fn submit_sunlight_begin(&mut self, viewport: &Viewport) {
        trace!("submit_sunlight_begin");

        let graphics = self.graphics_sunlight.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[viewport.rect]);
        self.cmd_buf.set_viewports(0, &[viewport.clone()]);
    }

    unsafe fn submit_sunlights(&mut self, lights: SunlightIter) {
        let graphics = self.graphics_spotlight.as_ref().unwrap();

        /*let view_inv = camera.view_inv();

        // TODO: Calculate this with object AABBs once those are ready (any AABB inside both the camera and shadow projections)
        // Calculate the world-space coords of the eight points that make up our camera frustum
        // and calculate the min/max/mid coordinates of them
        let camera_world = [
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, -1.0, -1.0)), 1.0))
                .truncate(),
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, -1.0, 1.0)), 1.0))
                .truncate(),
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, 1.0, -1.0)), 1.0))
                .truncate(),
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, 1.0, 1.0)), 1.0))
                .truncate(),
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, -1.0, -1.0)), 1.0))
                .truncate(),
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, -1.0, 1.0)), 1.0))
                .truncate(),
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, 1.0, -1.0)), 1.0))
                .truncate(),
            (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, 1.0, 1.0)), 1.0))
                .truncate(),
        ];
        let (mut min_x, mut min_y, mut min_z, mut max_x, mut max_y, mut max_z) = {
            let p0 = camera_world[0];
            (p0.x(), p0.y(), p0.z(), p0.x(), p0.y(), p0.z())
        };
        for pi in &camera_world {
            min_x = pi.x().min(min_x);
            min_y = pi.y().min(min_y);
            min_z = pi.z().min(min_z);
            max_x = pi.x().max(max_x);
            max_y = pi.y().max(max_y);
            max_z = pi.z().max(max_z);
        }
        let mid_x = (max_x + min_x) / 2.0;
        let mid_y = (max_y + min_y) / 2.0;
        let mid_z = (max_z + min_z) / 2.0;
        let position = vec3(mid_x, mid_y, mid_z);
        let target = position + e.normal;
        let n_dot_x = e.normal.dot(Vec3::unit_x()).abs();
        let n_dot_y = e.normal.dot(Vec3::unit_y()).abs();
        let up = if n_dot_x < n_dot_y {
            Vec3::unit_x()
        } else {
            Vec3::unit_y()
        };
        let light_view = Mat4::look_at_rh(position, target, up);
        let light_world = [
            (light_view * vec4_from_vec3(camera_world[0], 1.0)).truncate(),
            (light_view * vec4_from_vec3(camera_world[1], 1.0)).truncate(),
            (light_view * vec4_from_vec3(camera_world[2], 1.0)).truncate(),
            (light_view * vec4_from_vec3(camera_world[3], 1.0)).truncate(),
            (light_view * vec4_from_vec3(camera_world[4], 1.0)).truncate(),
            (light_view * vec4_from_vec3(camera_world[5], 1.0)).truncate(),
            (light_view * vec4_from_vec3(camera_world[6], 1.0)).truncate(),
            (light_view * vec4_from_vec3(camera_world[7], 1.0)).truncate(),
        ];
        let (mut min_x, mut min_y, mut min_z, mut max_x, mut max_y, mut max_z) = {
            let p0 = light_world[0];
            (p0.x(), p0.y(), p0.z(), p0.x(), p0.y(), p0.z())
        };
        for pi in &light_world {
            min_x = pi.x().min(min_x);
            min_y = pi.y().min(min_y);
            min_z = pi.z().min(min_z);
            max_x = pi.x().max(max_x);
            max_y = pi.y().max(max_y);
            max_z = pi.z().max(max_z);
        }
        let light_space =
            Mat4::orthographic_rh(min_x, max_x, min_y, max_y, min_z, max_z) * light_view;

        Self {
            normal_inv: -e.normal,
            diffuse: e.diffuse,
            power: e.power,
            light_space,
        }*/

        for light in lights {
            self.cmd_buf.push_graphics_constants(
                graphics.layout(),
                ShaderStageFlags::FRAGMENT,
                0,
                SunlightConsts {
                    intensity: light.color.to_rgb() * light.lumens,
                    normal: light.normal,
                }
                .as_ref(),
            );

            self.cmd_buf.draw(0..6, 0..1);
        }
    }

    unsafe fn submit_vertex_attrs_begin(&mut self, instr: VertexAttrsBeginInstruction) {
        trace!("submit_vertex_attrs_begin");

        let compute = match instr.idx_ty {
            IndexType::U16 => if instr.skin { self.compute_u16_skin_vertex_attrs.as_ref() } else {self.compute_u16_vertex_attrs.as_ref()},
            IndexType::U32 => if instr.skin { self.compute_u32_skin_vertex_attrs.as_ref() } else {self.compute_u32_vertex_attrs.as_ref()},
        }.unwrap();
        let pipeline = compute.pipeline();

        self.cmd_buf.bind_compute_pipeline(pipeline);
    }

    unsafe fn submit_vertex_attrs_descriptors(&mut self, instr: VertexAttrsDescriptorsInstruction) {
        trace!("submit_vertex_attrs_descriptors");

        let compute = match instr.idx_ty {
            IndexType::U16 => if instr.skin { self.compute_u16_skin_vertex_attrs.as_ref() } else {self.compute_u16_vertex_attrs.as_ref()},
            IndexType::U32 => if instr.skin { self.compute_u32_skin_vertex_attrs.as_ref() } else {self.compute_u32_vertex_attrs.as_ref()},
        }.unwrap();
        let desc_set = compute.desc_set(instr.desc_set);
        let pipeline = compute.pipeline();
        let layout = ComputePipeline::layout(&pipeline);

        bind_compute_descriptor_set(&mut self.cmd_buf, layout, desc_set);
    }

    unsafe fn submit_vertex_attrs_calc(&mut self, instr: DataComputeInstruction) {
        trace!("submit_vertex_attrs_calc");

        let device = self.driver.borrow();
        let limit = Device::gpu(&device).limits().max_compute_work_group_size[0];

        let compute = match instr.idx_ty {
            IndexType::U16 => if instr.skin { self.compute_u16_skin_vertex_attrs.as_ref() } else {self.compute_u16_vertex_attrs.as_ref()},
            IndexType::U32 => if instr.skin { self.compute_u32_skin_vertex_attrs.as_ref() } else {self.compute_u32_vertex_attrs.as_ref()},
        }.unwrap();
        let pipeline = compute.pipeline();
        let layout = ComputePipeline::layout(&pipeline);

        // We may be limited by the count of dispatches we issue; so use a loop
        // to dispatch as many times as needed
        self.cmd_buf.push_compute_constants(
            layout,
            0,
            CalcVertexAttrsConsts {
                offset: instr.offset,
            }
            .as_ref(),
        );
        self.cmd_buf.dispatch([instr.dispatch, 1, 1]);
    }

    unsafe fn submit_vertex_copies(&mut self, instr: DataCopyInstruction) {
        trace!("submit_vertex_copies");

        instr.buf.copy_ranges(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT,
            BufferAccess::VERTEX_BUFFER_READ,
            instr.ranges,
        );
    }

    unsafe fn submit_vertex_write(&mut self, instr: DataWriteInstruction) {
        trace!("submit_vertex_write");

        instr.buf.write_range(
            &mut self.cmd_buf,
            PipelineStage::VERTEX_INPUT,
            BufferAccess::VERTEX_BUFFER_READ,
            instr.range,
        );
    }

    unsafe fn submit_vertex_write_ref(&mut self, mut instr: DataWriteRefInstruction) {
        trace!("submit_vertex_write_ref");

        instr.buf.write_range(
            &mut self.cmd_buf,
            PipelineStage::COMPUTE_SHADER,
            BufferAccess::SHADER_READ,
            instr.range,
        );
    }

    unsafe fn submit_finish(&mut self) {
        trace!("submit_finish");

        let mut device = self.driver.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let mut output = self.geom_buf.output.borrow_mut();
        let dims = dst.dims();

        // Step 6: Copy the output graphics buffer into dst
        self.cmd_buf.end_render_pass();
        output.set_layout(
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
            output.as_ref(),
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
            Some(self.fence.as_ref()),
        );
    }

    unsafe fn write_material_descriptors<'m>(
        device: &Device,
        graphics: &Graphics,
        materials: impl ExactSizeIterator<Item = &'m Material>,
    ) {
        for (idx, material) in materials.enumerate() {
            let set = graphics.desc_set(idx);
            device.write_descriptor_sets(vec![
                DescriptorSetWrite {
                    set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: once(Descriptor::CombinedImageSampler(
                        material.color.borrow().as_default_view().as_ref(),
                        Layout::ShaderReadOnlyOptimal,
                        graphics.sampler(0).as_ref(),
                    )),
                },
                DescriptorSetWrite {
                    set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: once(Descriptor::CombinedImageSampler(
                        material.metal_rough.borrow().as_default_view().as_ref(),
                        Layout::ShaderReadOnlyOptimal,
                        graphics.sampler(1).as_ref(),
                    )),
                },
                DescriptorSetWrite {
                    set,
                    binding: 2,
                    array_offset: 0,
                    descriptors: once(Descriptor::CombinedImageSampler(
                        material.normal.borrow().as_default_view().as_ref(),
                        Layout::ShaderReadOnlyOptimal,
                        graphics.sampler(2).as_ref(),
                    )),
                },
            ]);
        }
    }

    unsafe fn write_vertex_descriptors<'v>(
        device: &Device,
        compute: &Compute,
        vertex_bufs: impl ExactSizeIterator<Item = VertexBuffers<'v>>,
    ) {
        for (idx, vertex_buf) in vertex_bufs.enumerate() {
            let set = compute.desc_set(idx);
            device.write_descriptor_sets(vec![
                DescriptorSetWrite {
                    set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: once(Descriptor::Buffer(
                        vertex_buf.idx.as_ref(),
                        SubRange {
                            offset: 0,
                            size: Some(vertex_buf.idx_len),
                        },
                    )),
                },
                DescriptorSetWrite {
                    set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: once(Descriptor::Buffer(
                        vertex_buf.src.as_ref(),
                        SubRange {
                            offset: 0,
                            size: Some(vertex_buf.src_len),
                        },
                    )),
                },
                DescriptorSetWrite {
                    set,
                    binding: 2,
                    array_offset: 0,
                    descriptors: once(Descriptor::Buffer(
                        vertex_buf.dst.as_ref(),
                        SubRange {
                            offset: 0,
                            size: Some(vertex_buf.dst_len),
                        },
                    )),
                },
                DescriptorSetWrite {
                    set,
                    binding: 3,
                    array_offset: 0,
                    descriptors: once(Descriptor::Buffer(
                        vertex_buf.write_mask.as_ref(),
                        SubRange {
                            offset: 0,
                            size: Some(vertex_buf.write_mask_len),
                        },
                    )),
                },
            ]);
        }
    }
}

pub struct DrawOpSubmission {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    compiler: Lease<Compiler>,
    compute_u16_vertex_attrs: Option<Lease<Compute>>,
    compute_u32_vertex_attrs: Option<Lease<Compute>>,
    dst: Texture2d,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    geom_buf: GeometryBuffer,
    graphics_line: Option<Lease<Graphics>>,
    graphics_mesh: Option<Lease<Graphics>>,
    graphics_mesh_anim: Option<Lease<Graphics>>,
    graphics_point_light: Option<Lease<Graphics>>,
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

#[derive(Clone, Debug)]
pub struct Material {
    pub color: BitmapRef,
    pub metal_rough: BitmapRef,
    pub normal: BitmapRef,
}

impl Eq for Material {}

impl Hash for Material {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.color.as_ptr().hash(state);
        self.metal_rough.as_ptr().hash(state);
        self.normal.as_ptr().hash(state);
    }
}

impl Ord for Material {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut res = BitmapRef::as_ptr(&self.color).cmp(&BitmapRef::as_ptr(&other.color));
        if res != Ordering::Less {
            return res;
        }

        res = BitmapRef::as_ptr(&self.metal_rough).cmp(&BitmapRef::as_ptr(&other.metal_rough));
        if res != Ordering::Less {
            return res;
        }

        BitmapRef::as_ptr(&self.normal).cmp(&BitmapRef::as_ptr(&other.normal))
    }
}

impl PartialEq for Material {
    fn eq(&self, other: &Self) -> bool {
        BitmapRef::ptr_eq(&self.color, &other.color)
            && BitmapRef::ptr_eq(&self.normal, &other.normal)
            && BitmapRef::ptr_eq(&self.metal_rough, &other.metal_rough)
    }
}

impl PartialOrd for Material {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[repr(C)]
struct Mat4Const(Mat4);

impl AsRef<[u32; 16]> for Mat4Const {
    #[inline]
    fn as_ref(&self) -> &[u32; 16] {
        unsafe { &*(self as *const Self as *const [u32; 16]) }
    }
}

#[repr(C)]
struct PointLightConsts {
    intensity: Vec3,
    radius: f32,
}

impl AsRef<[u32; 4]> for PointLightConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 4] {
        unsafe { &*(self as *const Self as *const [u32; 4]) }
    }
}

#[repr(C)]
struct RectLightConsts {
    dims: Vec2,
    intensity: Vec3,
    normal: Vec3,
    position: Vec3,
    radius: f32,
    range: f32,
    view_proj: Mat4,
}

impl AsRef<[u32; 6]> for RectLightConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 6] {
        unsafe { &*(self as *const Self as *const [u32; 6]) }
    }
}

#[repr(C)]
struct SunlightConsts {
    intensity: Vec3,
    normal: Vec3,
}

impl AsRef<[u32; 6]> for SunlightConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 6] {
        unsafe { &*(self as *const Self as *const [u32; 6]) }
    }
}

#[repr(C)]
struct SpotlightConsts {
    intensity: Vec3,
    normal: Vec3,
}

impl AsRef<[u32; 6]> for SpotlightConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 6] {
        unsafe { &*(self as *const Self as *const [u32; 6]) }
    }
}
