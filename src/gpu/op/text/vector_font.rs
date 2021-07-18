use {
    super::dyn_atlas::RasterizedGlyph,
    crate::{
        math::{CoordF, Rect, RectF},
        pak::Pak,
    },
    fontdue::{Font, FontSettings},
    std::{
        fmt::{Debug, Error, Formatter},
        io::{Read, Seek},
        ops::Deref,
    },
};

struct Parser<R>
where
    R: Iterator<Item = (char, RasterizedGlyph)>,
{
    raster: R,
    pos: CoordF,
    size: f32,
}

impl<R> Iterator for Parser<R>
where
    R: Iterator<Item = (char, RasterizedGlyph)>,
{
    type Item = (char, VectorGlyph);

    fn next(&mut self) -> Option<Self::Item> {
        let pos = &mut self.pos;
        self.raster.next().map(|(char, raster)| {
            let glyph = VectorGlyph {
                page_idx: raster.page_idx,
                page_rect: raster.page_rect,
                screen_rect: RectF::new(
                    pos.x,
                    raster.metrics.bounds.ymin,
                    raster.metrics.bounds.width,
                    raster.metrics.bounds.height,
                ),
            };

            pos.x += raster.metrics.advance_width;
            pos.y += raster.metrics.advance_height;

            (char, glyph)
        })
    }
}

// TODO: Expand to support fallback fonts like emoji
/// Holds a vector Font.
pub struct VectorFont {
    pub(super) font: Font,
}

impl VectorFont {
    pub(crate) fn load<D, S>(data: D, settings: S) -> Self
    where
        D: Deref<Target = [u8]>,
        S: Into<VectorFontSettings>,
    {
        // TODO: Use of unwrap here
        Self {
            font: Font::from_bytes(data, settings.into().into()).unwrap(),
        }
    }

    pub(crate) fn read<K, R, S>(pak: &mut Pak<R>, key: K, settings: S) -> Self
    where
        K: AsRef<str>,
        R: Read + Seek,
        S: Into<VectorFontSettings>,
    {
        let id = pak.blob_id(key).unwrap();
        let data = pak.read_blob(id);

        Self::load(data, settings)
    }

    // TODO: Add description and example showing layout area, top/bottom explanation, etc
    /// Returns the area, in pixels, required to render the given text.
    ///
    /// **_NOTE:_** The 'start' of the render area is at the zero coordinate, however it may extend
    /// into the negative x direction due to ligatures.
    pub fn measure<T>(&self, text: T, scale: f32) -> RectF
    where
        T: AsRef<str>,
    {
        let mut chars = text.as_ref().chars();
        let mut res = chars.next().map_or(RectF::ZERO, |char| {
            let bounds = self.font.metrics(char, scale).bounds;
            RectF::new(bounds.xmin, bounds.ymin, bounds.width, bounds.height)
        });

        for char in chars {
            let bounds = self.font.metrics(char, scale).bounds;
            res.dims.x += bounds.width - bounds.xmin;
            res.dims.y = res.dims.y.min(bounds.height - bounds.xmin);
        }

        res
    }

    pub(crate) fn parse<R>(&self, raster: R) -> impl Iterator<Item = (char, VectorGlyph)>
    where
        R: Iterator<Item = (char, RasterizedGlyph)>,
    {
        Parser {
            pos: CoordF::ZERO,
            raster,
            size: 0.0,
        }
    }
}

impl Debug for VectorFont {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("VectorFont")
    }
}

/// Settings for controlling specific font and layout behavior.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct VectorFontSettings {
    /// The default is 0. The index of the font to use if parsing a font collection.
    pub collection_index: u32,
    /// The default is 40. The scale in px the font geometry is optimized for. Fonts rendered at
    /// the scale defined here will be the most optimal in terms of looks and performance. Glyphs
    /// rendered smaller than this scale will look the same but perform slightly worse, while
    /// glyphs rendered larger than this will looks worse but perform slightly better.
    pub scale: f32,
}

impl Default for VectorFontSettings {
    fn default() -> Self {
        Self {
            collection_index: 0,
            scale: 40.0,
        }
    }
}

impl From<f32> for VectorFontSettings {
    fn from(scale: f32) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }
}

impl From<VectorFontSettings> for FontSettings {
    fn from(settings: VectorFontSettings) -> Self {
        Self {
            collection_index: settings.collection_index,
            scale: settings.scale,
        }
    }
}

#[derive(Clone, Copy)]
pub struct VectorGlyph {
    pub page_idx: usize,
    pub page_rect: Rect,
    pub screen_rect: RectF,
}
