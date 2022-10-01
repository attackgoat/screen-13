pub mod prelude {
    pub use super::{imgui, Condition, ImGui, Ui};
}

pub use imgui::{self, Condition, Ui};

use {
    bytemuck::cast_slice,
    imgui::{Context, DrawCmd, DrawCmdParams},
    imgui_winit_support::{HiDpiMode, WinitPlatform},
    inline_spirv::include_spirv,
    screen_13::prelude::*,
    std::{sync::Arc, time::Duration},
};

#[derive(Debug)]
pub struct ImGui {
    context: Context,
    font_atlas_image: Option<Arc<Lease<Image>>>,
    pipeline: Arc<GraphicPipeline>,
    platform: WinitPlatform,
    pool: HashPool,
}

impl ImGui {
    pub fn new(device: &Arc<Device>) -> Self {
        let mut context = Context::create();
        let platform = WinitPlatform::init(&mut context);
        let pool = HashPool::new(device);
        let pipeline = Arc::new(
            GraphicPipeline::create(
                device,
                GraphicPipelineInfo::new()
                    .blend(BlendMode::PRE_MULTIPLIED_ALPHA)
                    .cull_mode(vk::CullModeFlags::NONE),
                [
                    Shader::new_vertex(include_spirv!("res/shader/imgui.vert", vert).as_slice()),
                    Shader::new_fragment(include_spirv!("res/shader/imgui.frag", frag).as_slice()),
                ],
            )
            .unwrap(),
        );

        Self {
            context,
            font_atlas_image: None,
            pipeline,
            platform,
            pool,
        }
    }

    // TODO: This produces an image which is RGBA8 UNORM and has STORAGE set. *We* don't need storage here and should instead ask the user what settings to give the output image.....
    pub fn draw(
        &mut self,
        dt: f32,
        events: &[Event<'_, ()>],
        window: &Window,
        render_graph: &mut RenderGraph,
        ui_func: impl FnOnce(&mut Ui),
    ) -> ImageLeaseNode {
        let hidpi = self.platform.hidpi_factor();

        self.platform
            .attach_window(self.context.io_mut(), window, HiDpiMode::Default);

        if self.font_atlas_image.is_none() || self.platform.hidpi_factor() != hidpi {
            self.lease_font_atlas_image(render_graph);
        }

        let io = self.context.io_mut();
        io.update_delta_time(Duration::from_secs_f32(dt));

        for event in events {
            self.platform.handle_event(io, window, event);
        }

        self.platform
            .prepare_frame(io, window)
            .expect("Unable to prepare ImGui frame");

        // Let the caller draw the GUI
        let mut ui = self.context.frame();

        ui_func(&mut ui);

        self.platform.prepare_render(&ui, window);
        let draw_data = self.context.render();

        let image = render_graph.bind_node(
            self.pool
                .lease(ImageInfo::new_2d(
                    vk::Format::R8G8B8A8_UNORM,
                    window.inner_size().width,
                    window.inner_size().height,
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::STORAGE
                        | vk::ImageUsageFlags::TRANSFER_SRC, // TODO: Make TRANSFER_SRC an "extra flags"
                ))
                .unwrap(),
        );
        let font_atlas_image = render_graph.bind_node(self.font_atlas_image.as_ref().unwrap());
        let display_pos = draw_data.display_pos;
        let framebuffer_scale = draw_data.framebuffer_scale;

        for draw_list in draw_data.draw_lists() {
            let indices = cast_slice(draw_list.idx_buffer());
            let mut index_buf = self
                .pool
                .lease(BufferInfo {
                    size: indices.len() as _,
                    usage: vk::BufferUsageFlags::INDEX_BUFFER,
                    can_map: true,
                })
                .unwrap();

            {
                Buffer::mapped_slice_mut(&mut index_buf)[0..indices.len()].copy_from_slice(indices);
            }

            let index_buf = render_graph.bind_node(index_buf);

            let vertices = draw_list.vtx_buffer();
            let vertex_buf_len = vertices.len() * 20;
            let mut vertex_buf = self
                .pool
                .lease(BufferInfo {
                    size: vertex_buf_len as _,
                    usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                    can_map: true,
                })
                .unwrap();

            {
                let vertex_buf = Buffer::mapped_slice_mut(&mut vertex_buf);
                for (idx, vertex) in vertices.iter().enumerate() {
                    let offset = idx * 20;
                    vertex_buf[offset..offset + 8].copy_from_slice(cast_slice(&vertex.pos));
                    vertex_buf[offset + 8..offset + 16].copy_from_slice(cast_slice(&vertex.uv));
                    vertex_buf[offset + 16..offset + 20].copy_from_slice(&vertex.col);
                }
            }

            let vertex_buf = render_graph.bind_node(vertex_buf);

            let draw_cmds = draw_list
                .commands()
                .map(|draw_cmd| match draw_cmd {
                    DrawCmd::Elements {
                        count,
                        cmd_params:
                            DrawCmdParams {
                                clip_rect,
                                idx_offset,
                                vtx_offset,
                                ..
                            },
                    } => (count, clip_rect, idx_offset, vtx_offset),
                    _ => unimplemented!(),
                })
                .collect::<Vec<_>>();

            let window_width =
                self.platform.hidpi_factor() as f32 / window.inner_size().width as f32;
            let window_height =
                self.platform.hidpi_factor() as f32 / window.inner_size().height as f32;

            render_graph
                .begin_pass("imgui")
                .bind_pipeline(&self.pipeline)
                .access_node(index_buf, AccessType::IndexBuffer)
                .access_node(vertex_buf, AccessType::IndexBuffer)
                .read_descriptor(0, font_atlas_image)
                .clear_color(0, image)
                .store_color(0, image)
                .record_subpass(move |subpass| {
                    subpass
                        .push_constants_offset(0, &window_width.to_ne_bytes())
                        .push_constants_offset(4, &window_height.to_ne_bytes())
                        .bind_index_buffer(index_buf, vk::IndexType::UINT16)
                        .bind_vertex_buffer(vertex_buf);

                    for (index_count, clip_rect, first_index, vertex_offset) in draw_cmds {
                        let clip_rect = [
                            (clip_rect[0] - display_pos[0]) * framebuffer_scale[0],
                            (clip_rect[1] - display_pos[1]) * framebuffer_scale[1],
                            (clip_rect[2] - display_pos[0]) * framebuffer_scale[0],
                            (clip_rect[3] - display_pos[1]) * framebuffer_scale[1],
                        ];
                        let x = clip_rect[0].floor() as i32;
                        let y = clip_rect[1].floor() as i32;
                        let width = (clip_rect[2] - clip_rect[0]).ceil() as u32;
                        let height = (clip_rect[3] - clip_rect[1]).ceil() as u32;
                        subpass.set_scissor(x, y, width, height);
                        subpass.draw_indexed(
                            index_count as _,
                            1,
                            first_index as _,
                            vertex_offset as _,
                            0,
                        );
                    }
                });
        }

        image
    }

    pub fn draw_frame(
        &mut self,
        frame: &mut FrameContext<'_>,
        ui_func: impl FnOnce(&mut Ui),
    ) -> ImageLeaseNode {
        self.draw(
            frame.dt,
            frame.events,
            frame.window,
            frame.render_graph,
            ui_func,
        )
    }

    fn lease_font_atlas_image(&mut self, render_graph: &mut RenderGraph) {
        use imgui::{FontConfig, FontGlyphRanges, FontSource};

        let hidpi_factor = self.platform.hidpi_factor();
        self.context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        let font_size = (14.0 * hidpi_factor) as f32;
        let fonts = self.context.fonts();
        fonts.clear_fonts();
        fonts.add_font(&[
            FontSource::TtfData {
                data: include_bytes!("../res/font/roboto/roboto-regular.ttf"),
                size_pixels: font_size,
                config: Some(FontConfig {
                    rasterizer_multiply: 2.0,
                    glyph_ranges: FontGlyphRanges::japanese(),
                    ..FontConfig::default()
                }),
            },
            FontSource::TtfData {
                data: include_bytes!("../res/font/mplus-1p/mplus-1p-regular.ttf"),
                size_pixels: font_size,
                config: Some(FontConfig {
                    oversample_h: 2,
                    oversample_v: 2,
                    // Range of glyphs to rasterize
                    glyph_ranges: FontGlyphRanges::japanese(),
                    ..FontConfig::default()
                }),
            },
        ]);

        let texture = fonts.build_rgba32_texture(); // TODO: Fix fb channel writes and use alpha8!
        let temp_buf_len = texture.data.len();
        let mut temp_buf = self
            .pool
            .lease(BufferInfo {
                size: temp_buf_len as _,
                usage: vk::BufferUsageFlags::TRANSFER_SRC,
                can_map: true,
            })
            .unwrap();

        {
            let temp_buf = Buffer::mapped_slice_mut(&mut temp_buf);
            temp_buf[0..temp_buf_len].copy_from_slice(texture.data);
        }

        let temp_buf = render_graph.bind_node(temp_buf);
        let image = render_graph.bind_node(
            self.pool
                .lease(ImageInfo::new_2d(
                    vk::Format::R8G8B8A8_UNORM,
                    texture.width,
                    texture.height,
                    vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::STORAGE
                        | vk::ImageUsageFlags::TRANSFER_DST,
                ))
                .unwrap(),
        );

        render_graph.copy_buffer_to_image(temp_buf, image);

        self.font_atlas_image = Some(render_graph.unbind_node(image));
    }
}
