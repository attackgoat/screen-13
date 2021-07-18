pub use bmfont::CharPosition as BitmapGlyph;

use {
    crate::{
        gpu::{
            op::bitmap::{Bitmap, BitmapOp},
            pool::Pool,
            Texture2d,
        },
        math::Rect,
        pak::Pak,
    },
    archery::SharedPointerKind,
    bmfont::{BMFont, OrdinateOrientation},
    std::{
        fmt::{Debug, Error, Formatter},
        io::{Cursor, Read, Seek},
    },
};

/// Holds a decoded bitmap Font.
pub struct BitmapFont<P>
where
    P: 'static + SharedPointerKind,
{
    font: BMFont,
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
        let font = BMFont::new(
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
                    page,
                )
                .record()
            })
            .collect();

        Self { font, pages }
    }

    // TODO: Add description and example showing layout area, top/bottom explanation, etc
    /// Returns the area, in pixels, required to render the given text.
    ///
    /// **_NOTE:_** The 'start' of the render area is at the zero coordinate, however it may extend
    /// into the negative x direction due to ligatures.
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

    pub(super) fn page(&self, idx: usize) -> &Texture2d {
        &self.pages[idx]
    }

    pub(super) fn pages(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.pages.iter().map(|page| page.as_ref())
    }

    pub(super) fn parse<'a>(&'a self, text: &'a str) -> impl Iterator<Item = BitmapGlyph> + 'a {
        self.font.parse(text)
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
