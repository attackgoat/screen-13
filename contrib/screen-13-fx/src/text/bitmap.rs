use {
    super::glyph::Glyph,
    crate::ImageLoader,
    anyhow::Context,
    bmfont::{BMFont, OrdinateOrientation},
    screen_13::prelude_all::*,
    std::{cell::RefCell, io::Cursor},
};

type Color = [u8; 4];

fn color_to_unorm(color: Color) -> [f32; 4] {
    [
        color[0] as f32 / u8::MAX as f32,
        color[1] as f32 / u8::MAX as f32,
        color[2] as f32 / u8::MAX as f32,
        color[3] as f32 / u8::MAX as f32,
    ]
}

/// Holds a decoded bitmap Font.
#[derive(Debug)]
pub struct BitmapFont<P>
where
    P: SharedPointerKind,
{
    font: BMFont,
    pages: RefCell<Vec<ImageBinding<P>>>,
    pipeline: Shared<GraphicPipeline<P>, P>,
    pool: RefCell<HashPool<P>>,
}

impl<P> BitmapFont<P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub fn load(
        bitmap_font: BitmapFontBuf,
        image_loader: &mut ImageLoader<P>,
    ) -> anyhow::Result<Self> {
        let font = BMFont::new(
            Cursor::new(bitmap_font.def()),
            OrdinateOrientation::TopToBottom,
        )?;
        let pages = RefCell::new(
            bitmap_font
                .pages()
                .map(|page_buf| image_loader.decode_linear(page_buf))
                .collect::<Result<_, _>>()?,
        );
        let num_pages = bitmap_font.pages().len() as i32;
        let pipeline = Shared::new(
            GraphicPipeline::create(
                &image_loader.device,
                GraphicPipelineInfo::new()
                    .blend(BlendMode::Alpha)
                    .extra_descriptors(
                        [(
                            DescriptorBinding(0, 0),
                            DescriptorInfo::CombinedImageSampler(
                                num_pages as u32,
                                vk::Sampler::null(),
                            ),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                [
                    Shader::new_vertex(crate::res::shader::GRAPHIC_FONT_VERT),
                    Shader::new_fragment(crate::res::shader::GRAPHIC_FONT_BITMAP_FRAG)
                        .specialization_info(SpecializationInfo::new(
                            [vk::SpecializationMapEntry {
                                constant_id: 0,
                                offset: 0,
                                size: 4,
                            }],
                            num_pages.to_ne_bytes(),
                        )),
                ],
            )
            .context("Unable to create bitmap font pipeline")?,
        );
        let pool = RefCell::new(HashPool::new(&image_loader.device));

        Ok(Self {
            font,
            pages,
            pipeline,
            pool,
        })
    }

    // TODO: Add description and example showing layout area, top/bottom explanation, etc
    /// Returns the position and area, in pixels, required to render the given text.
    ///
    /// **_NOTE:_** The 'start' of the render area is at the zero coordinate, however it may extend
    /// into the negative x direction due to ligatures.
    pub fn measure(&self, text: &str) -> (IVec2, UVec2) {
        let parse = self.font.parse(text);

        // TODO: Use if we enable parsing errors on bmfont library
        // if parse.is_err() {
        //     return (IVec2::ZERO, UVec2::ZERO);
        // }
        // let parse = parse.unwrap();

        let mut min_x = 0;
        let mut max_x = 0;
        let mut max_y = 0;
        for char in parse {
            if char.screen_rect.x < min_x {
                min_x = char.screen_rect.x;
            }

            let screen_x = char.screen_rect.max_x();
            if screen_x > max_x {
                max_x = screen_x;
            }

            let screen_y = char.screen_rect.max_y();
            if screen_y > max_y {
                max_y = screen_y;
            }
        }

        let position = ivec2(min_x, 0);
        let size = uvec2((max_x - min_x) as _, max_y as _);

        (position, size)
    }

    pub fn print(
        &self,
        graph: &mut RenderGraph<P>,
        image: impl Into<AnyImageNode<P>>,
        position: Vec2,
        color: impl Into<BitmapGlyphColor>,
        text: impl AsRef<str>,
    ) {
        self.print_scale(graph, image, position, color, text, 1.0);
    }

    pub fn print_scale(
        &self,
        graph: &mut RenderGraph<P>,
        image: impl Into<AnyImageNode<P>>,
        position: Vec2,
        color: impl Into<BitmapGlyphColor>,
        text: impl AsRef<str>,
        scale: f32,
    ) {
        let color = color.into();
        let image = image.into();
        let text = text.as_ref();
        let image_info = graph.node_info(image);
        let transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
            * Mat4::from_scale(vec3(2.0 * scale, 2.0 * scale, 1.0))
            * Mat4::from_translation(vec3(
                position.x / image_info.extent.x as f32,
                position.y / image_info.extent.y as f32,
                0.0,
            ));

        let vertex_buf_len = text.chars().count() as u64 * 120;
        let mut vertex_buf_binding = self
            .pool
            .borrow_mut()
            .lease(BufferInfo {
                size: vertex_buf_len,
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                can_map: true,
            })
            .unwrap();
        let vertex_buf = &mut Buffer::mapped_slice_mut(vertex_buf_binding.get_mut().unwrap())
            [0..vertex_buf_len as usize];

        let mut vertex_count = 0;
        let mut offset = 0;
        for (data, char) in self.font.parse(text).map(|char| (char.tessellate(), char)) {
            vertex_buf[offset..offset + 16].copy_from_slice(&data[0]);
            vertex_buf[offset + 20..offset + 36].copy_from_slice(&data[1]);
            vertex_buf[offset + 40..offset + 56].copy_from_slice(&data[2]);
            vertex_buf[offset + 60..offset + 76].copy_from_slice(&data[3]);
            vertex_buf[offset + 80..offset + 96].copy_from_slice(&data[4]);
            vertex_buf[offset + 100..offset + 116].copy_from_slice(&data[5]);

            let page_idx = char.page_index as i32;
            let page_idx = page_idx.to_ne_bytes();
            vertex_buf[offset + 16..offset + 20].copy_from_slice(&page_idx);
            vertex_buf[offset + 36..offset + 40].copy_from_slice(&page_idx);
            vertex_buf[offset + 56..offset + 60].copy_from_slice(&page_idx);
            vertex_buf[offset + 76..offset + 80].copy_from_slice(&page_idx);
            vertex_buf[offset + 96..offset + 100].copy_from_slice(&page_idx);
            vertex_buf[offset + 116..offset + 120].copy_from_slice(&page_idx);

            vertex_count += 6;
            offset += 120;
        }

        let vertex_buf_node = graph.bind_node(vertex_buf_binding);

        let mut pages = self.pages.borrow_mut();
        let mut page_nodes: Vec<ImageNode<P>> = Vec::with_capacity(pages.len());
        for page in pages.drain(..) {
            page_nodes.push(graph.bind_node(page));
        }

        let mut pass = graph
            .record_pass("text")
            .access_node(vertex_buf_node, AccessType::VertexBuffer)
            .bind_pipeline(&self.pipeline)
            .load_color(0, image)
            .store_color(0, image);

        for (idx, page_node) in page_nodes.iter().enumerate() {
            pass = pass.read_descriptor((0, [idx as _]), *page_node);
        }

        pass.push_constants((
            transform,
            1.0 / image_info.extent.xy().as_vec2(),
            0u32, // Padding
            0u32, // Padding
            color_to_unorm(color.solid()),
            color_to_unorm(color.outline()),
        ))
        .draw(move |device, cmd_buf, bindings| unsafe {
            use std::slice::from_ref;

            device.cmd_bind_vertex_buffers(
                cmd_buf,
                0,
                from_ref(&bindings[vertex_buf_node]),
                from_ref(&0),
            );
            device.cmd_draw(cmd_buf, vertex_count, 1, 0, 0);
        });

        for page_node in page_nodes {
            pages.push(graph.unbind_node(page_node));
        }
    }
}

pub enum BitmapGlyphColor {
    Outline(Color),
    Solid(Color),
    SolidOutline(Color, Color),
}

impl BitmapGlyphColor {
    const TRANSARENT: Color = [0, 0, 0, u8::MAX];

    fn outline(&self) -> Color {
        match self {
            Self::Outline(color) => *color,
            _ => Self::TRANSARENT,
        }
    }

    fn solid(&self) -> Color {
        match self {
            Self::Outline(_) => Self::TRANSARENT,
            Self::Solid(color) => *color,
            Self::SolidOutline(color, _) => *color,
        }
    }
}

impl From<[f32; 3]> for BitmapGlyphColor {
    fn from(color: [f32; 3]) -> Self {
        Self::Solid([
            (color[0].clamp(0.0, 1.0) * u8::MAX as f32) as _,
            (color[1].clamp(0.0, 1.0) * u8::MAX as f32) as _,
            (color[2].clamp(0.0, 1.0) * u8::MAX as f32) as _,
            u8::MAX,
        ])
    }
}

impl From<[f32; 4]> for BitmapGlyphColor {
    fn from(color: [f32; 4]) -> Self {
        Self::Solid([
            (color[0].clamp(0.0, 1.0) * u8::MAX as f32) as _,
            (color[1].clamp(0.0, 1.0) * u8::MAX as f32) as _,
            (color[2].clamp(0.0, 1.0) * u8::MAX as f32) as _,
            (color[3].clamp(0.0, 1.0) * u8::MAX as f32) as _,
        ])
    }
}

impl From<[u8; 3]> for BitmapGlyphColor {
    fn from(color: [u8; 3]) -> Self {
        Self::Solid([color[0], color[1], color[2], u8::MAX])
    }
}

impl From<[u8; 4]> for BitmapGlyphColor {
    fn from(color: [u8; 4]) -> Self {
        Self::Solid(color)
    }
}
