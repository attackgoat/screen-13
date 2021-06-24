use {
    crate::{
        gpu::{
            op::bitmap::{Bitmap, BitmapOp},
            pool::Pool,
            Texture2d,
        },
        math::{CoordF, Rect},
        pak::Pak,
    },
    archery::SharedPointerKind,
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
    font: BMFont,
    page: Bitmap<P>,
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
        let font = BMFont::new(
            Cursor::new(bitmap_font.def()),
            OrdinateOrientation::TopToBottom,
        )
        .unwrap();
        let page = unsafe {
            BitmapOp::new(
                #[cfg(feature = "debug-names")]
                "Font",
                pool,
                bitmap_font.page(),
            )
            .record()
        };

        Self { font, page }
    }

    /// Returns the area, in pixels, required to render the given text.
    ///
    /// **_NOTE:_** The 'start' of the render area is at the zero coordinate, however it may extend
    /// into the negative x direction due to ligatures and right-to-left fonts.
    pub fn measure(&self, text: &str) -> Rect {
        let parse = self.font.parse(text);

        // // TODO: Let them know about the missing/unsupported characters?
        // if parse.is_err() {
        //     return Rect::ZERO;
        // }

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

        Rect::new(min_x, 0, (max_x - min_x) as _, max_y as _)
    }

    pub(super) fn page(&self) -> &Texture2d {
        &self.page
    }

    pub(super) fn parse<'a>(&'a self, text: &'a str) -> impl Iterator<Item = CharPosition> + 'a {
        self.font.parse(text)
    }

    pub(super) fn tessellate(&self, char: &CharPosition) -> [u8; 96] {
        let x1 = char.screen_rect.x as f32;
        let y1 = char.screen_rect.y as f32;
        let x2 = (char.screen_rect.x + char.screen_rect.width as i32) as f32;
        let y2 = (char.screen_rect.y + char.screen_rect.height as i32) as f32;

        // BMFont coordinates are based on square pages, but we tile additional pages onto the
        // bottom of our single page to avoid requiring page switching.
        let page_dims: CoordF = self.page.dims().into();
        let page_offset = char.page_index as f32 * page_dims.x / page_dims.y;

        let u1 = char.page_rect.x as f32 / page_dims.y;
        let v1 = page_offset + char.page_rect.y as f32 / page_dims.y;
        let u2 = (char.page_rect.x + char.page_rect.width as i32) as f32 / page_dims.y;
        let v2 =
            page_offset + (char.page_rect.y + char.page_rect.height as i32) as f32 / page_dims.y;

        let x1 = x1.to_ne_bytes();
        let x2 = x2.to_ne_bytes();
        let y1 = y1.to_ne_bytes();
        let y2 = y2.to_ne_bytes();
        let u1 = u1.to_ne_bytes();
        let u2 = u2.to_ne_bytes();
        let v1 = v1.to_ne_bytes();
        let v2 = v2.to_ne_bytes();

        let mut res: [u8; 96] = [0; 96];

        // Top left (first triangle)
        res[0..4].copy_from_slice(&x1);
        res[4..8].copy_from_slice(&y1);
        res[8..12].copy_from_slice(&u1);
        res[12..16].copy_from_slice(&v1);

        // Bottom right
        res[16..20].copy_from_slice(&x2);
        res[20..24].copy_from_slice(&y2);
        res[24..28].copy_from_slice(&u2);
        res[28..32].copy_from_slice(&v2);

        // Top right
        res[32..36].copy_from_slice(&x2);
        res[36..40].copy_from_slice(&y1);
        res[40..44].copy_from_slice(&u2);
        res[44..48].copy_from_slice(&v1);

        // Top left (second triangle)
        res[48..52].copy_from_slice(&x1);
        res[52..56].copy_from_slice(&y1);
        res[56..60].copy_from_slice(&u1);
        res[60..64].copy_from_slice(&v1);

        // Bottom left
        res[64..68].copy_from_slice(&x1);
        res[68..72].copy_from_slice(&y2);
        res[72..76].copy_from_slice(&u1);
        res[76..80].copy_from_slice(&v2);

        // Bottom right
        res[80..84].copy_from_slice(&x2);
        res[84..88].copy_from_slice(&y2);
        res[88..92].copy_from_slice(&u2);
        res[92..96].copy_from_slice(&v2);

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
