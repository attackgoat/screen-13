use {
    crate::{
        gpu::{
            op::bitmap::{Bitmap, BitmapOp},
            pool::Pool,
        },
        math::Extent,
        pak::Pak,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    bmfont::{BMFont, CharPosition, OrdinateOrientation},
    std::{
        f32,
        fmt::{Debug, Error, Formatter},
        io::{Cursor, Read, Seek},
    },
};

// TODO: Add automatic character-rejection/skipping by adding support for the "auto-cull" feature?
/// Holds a decoded bitmap Font.
pub struct BitmapFont<P>
where
    P: 'static + SharedPointerKind,
{
    def: BMFont,
    pages: Vec<Bitmap<P>>,
}

impl<P> BitmapFont<P>
where
    P: SharedPointerKind,
{
    pub(crate) fn read<K: AsRef<str>, R: Read + Seek>(
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
            Vertex {
                x: x1,
                y: y1,
                u: u1,
                v: v1,
            },
            Vertex {
                x: x2,
                y: y2,
                u: u2,
                v: v2,
            },
            Vertex {
                x: x2,
                y: y1,
                u: u2,
                v: v1,
            },
            Vertex {
                x: x1,
                y: y1,
                u: u1,
                v: v1,
            },
            Vertex {
                x: x1,
                y: y2,
                u: u1,
                v: v2,
            },
            Vertex {
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
                    font_texture.dims(),
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

impl<P> Debug for BitmapFont<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("BitmapFont")
    }
}

#[derive(Clone, Copy, Default)]
struct Vertex {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
}
