pub mod prelude_arc{
    pub use super::*;

    use archery::ArcK;

    pub type Egui = super::Egui<ArcK>;
}

pub mod prelude_rc{
    pub use super::*;

    use archery::RcK;

    pub type Egui = super::Egui<RcK>;
}

use {
    std::{borrow::Cow, collections::HashMap},
    archery::{SharedPointer, SharedPointerKind},
    bytemuck::cast_slice,
    screen_13::prelude_all::*,
    screen_13::graph::Bind,
};


pub struct Egui<P> 
where P: SharedPointerKind{
    pub ctx: egui::Context,
    egui_winit: egui_winit::State,
    textures: HashMap<egui::TextureId, ImageLeaseBinding<P>>,
    cache: HashPool<P>,
    ppl: SharedPointer<GraphicPipeline<P>, P>,
    next_tex_id: u64,
    user_textures: HashMap<egui::TextureId, AnyImageNode<P>>,
}

impl<P> Egui<P> 
where P: SharedPointerKind + Send + 'static{
    pub fn new(device: &SharedPointer<Device<P>, P>, window: &Window) -> Self {
        let ppl = SharedPointer::new(
            GraphicPipeline::create(
                device,
                GraphicPipelineInfo::new()
                    .blend(BlendMode {
                        blend_enable: true,
                        src_color_blend_factor: vk::BlendFactor::ONE,
                        dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                        color_blend_op: vk::BlendOp::ADD,
                        src_alpha_blend_factor: vk::BlendFactor::ONE,
                        dst_alpha_blend_factor: vk::BlendFactor::ONE,
                        alpha_blend_op: vk::BlendOp::ADD,
                        color_write_mask: vk::ColorComponentFlags::R
                            | vk::ColorComponentFlags::G
                            | vk::ColorComponentFlags::B
                            | vk::ColorComponentFlags::A,
                    })
                    .cull_mode(vk::CullModeFlags::NONE),
                [
                    Shader::new_vertex(
                        inline_spirv::include_spirv!("shaders/vert.glsl", vert, vulkan1_2)
                            .as_slice(),
                    ),
                    Shader::new_fragment(
                        inline_spirv::include_spirv!("shaders/frag.glsl", frag, vulkan1_2)
                            .as_slice(),
                    ),
                ],
            )
            .unwrap(),
        );
        Self {
            ppl,
            ctx: egui::Context::default(),
            egui_winit: egui_winit::State::new(10000, window),
            textures: HashMap::default(),
            cache: HashPool::new(device),
            next_tex_id: 0,
            user_textures: HashMap::default(),
        }
    }

    fn bind_and_update_textures(
        &mut self,
        deltas: &egui::TexturesDelta,
        render_graph: &mut RenderGraph<P>,
    ) -> HashMap<egui::TextureId, AnyImageNode<P>> {
        let mut bound_tex = deltas
            .set
            .iter()
            .map(|(id, delta)| {
                let pixels = match &delta.image {
                    egui::ImageData::Color(image) => {
                        assert_eq!(image.width() * image.height(), image.pixels.len());
                        Cow::Borrowed(&image.pixels)
                    }
                    egui::ImageData::Font(image) => {
                        let gamma = 1.0;
                        Cow::Owned(image.srgba_pixels(gamma).collect::<Vec<_>>())
                    }
                };

                let tmp_buf = {
                    let mut buf = self
                        .cache
                        .lease(BufferInfo::new_mappable(
                            (pixels.len() * 4) as u64,
                            vk::BufferUsageFlags::TRANSFER_SRC,
                        ))
                        .unwrap();
                    Buffer::copy_from_slice(buf.get_mut().unwrap(), 0, cast_slice(&pixels));
                    render_graph.bind_node(buf)
                };

                if let Some(pos) = delta.pos {
                    let image = AnyImageNode::ImageLease(
                        self.textures
                            .remove(id)
                            .expect("Tried updating undefined texture.")
                            .bind(render_graph),
                    );

                    render_graph.copy_buffer_to_image_region(
                        tmp_buf,
                        image,
                        &vk::BufferImageCopy {
                            buffer_offset: 0,
                            buffer_row_length: 0,
                            buffer_image_height: 0,
                            image_offset: vk::Offset3D {
                                x: pos[0] as i32,
                                y: pos[1] as i32,
                                z: 0,
                            },
                            image_extent: vk::Extent3D {
                                width: delta.image.width() as u32,
                                height: delta.image.height() as u32,
                                depth: 1,
                            },
                            image_subresource: vk::ImageSubresourceLayers {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                mip_level: 0,
                                base_array_layer: 0,
                                layer_count: 1,
                            },
                        },
                    );
                    (*id, image)
                } else {
                    let image = AnyImageNode::ImageLease(
                        self.cache
                            .lease(ImageInfo::new_2d(
                                vk::Format::R8G8B8A8_UNORM,
                                delta.image.width() as u32,
                                delta.image.height() as u32,
                                vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
                            ))
                            .unwrap()
                            .bind(render_graph),
                    );

                    render_graph.copy_buffer_to_image(tmp_buf, image);
                    render_graph.unbind_node(tmp_buf);
                    (*id, image)
                }
            })
            .collect::<HashMap<_, _>>();

        // Bind the rest of the textures.
        for (id, image) in self.textures.drain() {
            bound_tex.insert(id, AnyImageNode::ImageLease(render_graph.bind_node(image)));
        }

        // Add user textures.
        for (id, node) in self.user_textures.drain() {
            bound_tex.insert(id, node);
        }

        bound_tex
    }

    fn unbind_and_free(
        &mut self,
        bound_tex: HashMap<egui::TextureId, AnyImageNode<P>>,
        render_graph: &mut RenderGraph<P>,
        deltas: &egui::TexturesDelta,
    ) {
        // Unbind textures
        for (id, tex) in bound_tex.iter() {
            if let AnyImageNode::ImageLease(tex) = tex{
                if let egui::TextureId::Managed(_) = *id{
                    self.textures.insert(*id, render_graph.unbind_node(*tex));
                }
            }
        }

        // Free textures.
        for id in deltas.free.iter() {
            self.textures.remove(id);
        }

        self.next_tex_id = 0;
    }

    fn draw_primitive(
        &mut self,
        shapes: Vec<egui::epaint::ClippedShape>,
        bound_tex: &HashMap<egui::TextureId, AnyImageNode<P>>,
        render_graph: &mut RenderGraph<P>,
        target: impl Into<AnyImageNode<P>>,
    ) {
        let target = target.into();
        let target_info = render_graph.node_info(target);
        for egui::ClippedPrimitive {
            clip_rect,
            primitive,
        } in self.ctx.tessellate(shapes)
        {
            match primitive {
                egui::epaint::Primitive::Mesh(mesh) => {
                    // Skip when there are no vertices to paint since we cannot allocate a buffer
                    // of length 0
                    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                        continue;
                    }
                    let texture = bound_tex.get(&mesh.texture_id).unwrap();

                    let idx_buf = {
                        let mut buf = self
                            .cache
                            .lease(BufferInfo::new_mappable(
                                (mesh.indices.len() * 4) as u64,
                                vk::BufferUsageFlags::INDEX_BUFFER,
                            ))
                            .unwrap();
                        Buffer::copy_from_slice(
                            buf.get_mut().unwrap(),
                            0,
                            cast_slice(&mesh.indices),
                        );
                        buf
                    };
                    let idx_buf = render_graph.bind_node(idx_buf);

                    let vert_buf = {
                        let mut buf = self
                            .cache
                            .lease(BufferInfo::new_mappable(
                                (mesh.vertices.len() * std::mem::size_of::<egui::epaint::Vertex>())
                                    as u64,
                                vk::BufferUsageFlags::VERTEX_BUFFER,
                            ))
                            .unwrap();
                        Buffer::copy_from_slice(
                            buf.get_mut().unwrap(),
                            0,
                            cast_slice(&mesh.vertices),
                        );
                        buf
                    };
                    let vert_buf = render_graph.bind_node(vert_buf);

                    #[repr(C)]
                    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
                    struct PushConstants {
                        screen_size: [f32; 2],
                    }

                    let pixels_per_point = self.ctx.pixels_per_point();

                    let push_constants = PushConstants {
                        screen_size: [
                            target_info.width as f32 / pixels_per_point,
                            target_info.height as f32 / pixels_per_point,
                        ],
                    };

                    let num_indices = mesh.indices.len() as u32;

                    let clip_x = (clip_rect.min.x as f32 * pixels_per_point) as i32;
                    let clip_y = (clip_rect.min.y as f32 * pixels_per_point) as i32;

                    let clip_width = ((clip_rect.max.x - clip_rect.min.x) as f32 * pixels_per_point) as u32;
                    let clip_height = ((clip_rect.max.y - clip_rect.min.y) as f32 * pixels_per_point) as u32;

                    render_graph
                        .begin_pass("Egui pass")
                        .bind_pipeline(&self.ppl)
                        .access_node(idx_buf, AccessType::IndexBuffer)
                        .access_node(vert_buf, AccessType::VertexBuffer)
                        .access_descriptor((0, 0), *texture, AccessType::FragmentShaderReadOther)
                        .load_color(0, target)
                        .store_color(0, target)
                        .record_subpass(move |subpass| {
                            subpass.bind_index_buffer(idx_buf, vk::IndexType::UINT32);
                            subpass.bind_vertex_buffer(vert_buf);
                            subpass.push_constants(cast_slice(&[push_constants]));
                            subpass.set_scissor(
                                clip_x,
                                clip_y,
                                clip_width,
                                clip_height,
                            );
                            subpass.draw_indexed(num_indices, 1, 0, 0, 0);
                        });
                }
                _ => panic!("Primitiv callback not yet supported."),
            }
        }
    }

    pub fn run(
        &mut self,
        window: &Window,
        events: &[Event<()>],
        target: impl Into<AnyImageNode<P>>,
        render_graph: &mut RenderGraph<P>,
        ui_fn: impl FnMut(&egui::Context),
    ) {
        // Update events and generate shapes and texture deltas.
        for event in events {
            if let Event::WindowEvent{event, ..} = event{
                self.egui_winit.on_event(&self.ctx, event);
            }
        }
        let raw_input = self.egui_winit.take_egui_input(window);
        let full_output = self.ctx.run(raw_input, ui_fn);

        self.egui_winit
            .handle_platform_output(window, &self.ctx, full_output.platform_output);

        let deltas = full_output.textures_delta;

        let bound_tex = self.bind_and_update_textures(&deltas, render_graph);

        self.draw_primitive(full_output.shapes, &bound_tex, render_graph, target);

        self.unbind_and_free(bound_tex, render_graph, &deltas);
    }

    pub fn register_texture(&mut self, tex: impl Into<AnyImageNode<P>>) -> egui::TextureId {
        let id = egui::TextureId::User(self.next_tex_id);
        self.next_tex_id += 1;
        self.user_textures.insert(id, tex.into());
        id
    }
}
