use {
    super::glyph::BitmapGlyph,
    crate::ImageLoader,
    bmfont::{BMFont, OrdinateOrientation},
    screen_13::prelude_all::*,
    std::io::Cursor,
};

/// Holds a decoded bitmap Font.
#[derive(Debug)]
pub struct BitmapFont<P>
where
    P: SharedPointerKind,
{
    font: BMFont,
    pages: Vec<ImageBinding<P>>,
}

impl<P> BitmapFont<P>
where
    P: SharedPointerKind,
{
    pub fn load(
        bitmap_font: BitmapFontBuf,
        image_loader: &mut ImageLoader<P>,
    ) -> anyhow::Result<Self>
    where
        P: 'static,
    {
        let font = BMFont::new(
            Cursor::new(bitmap_font.def()),
            OrdinateOrientation::TopToBottom,
        )?;
        let pages = bitmap_font
            .pages()
            .map(|page_buf| image_loader.decode_linear(page_buf))
            .collect::<Result<_, _>>()?;

        Ok(Self { font, pages })
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

    pub(super) fn page(&self, idx: usize) -> &ImageBinding<P> {
        &self.pages[idx]
    }

    // pub(super) fn pages(&self) -> impl ExactSizeIterator<Item = &Image<P>> {
    //     self.pages.iter().map(|page| page.as_ref())
    // }

    pub(super) fn parse<'a>(&'a self, text: &'a str) -> impl Iterator<Item = BitmapGlyph> + 'a {
        self.font.parse(text)
    }
}

#[derive(Debug)]
pub struct BitmapFontRenderer<P>
where
    P: SharedPointerKind,
{
    device: Shared<Device<P>, P>,
}

impl<P> BitmapFontRenderer<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self {
            device: Shared::clone(device),
        })
    }

    pub fn render(
        &self,
        graph: &mut RenderGraph<P>,
        image: impl Into<AnyImageNode<P>>,
        font: &BitmapFont<P>,
        position: IVec2,
        text: impl AsRef<str>,
    ) where
        P: 'static,
    {
    }
}
