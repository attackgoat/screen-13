use {
    super::{BitmapFont, VectorFont},
    crate::{
        color::{AlphaColor, WHITE},
        math::{CoordF, Mat4},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    std::iter::{once, Once},
};

/// An expressive type which allows specification of individual text operations.
#[non_exhaustive]
pub enum Command<P, T>
where
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    /// Draws bitmapped text at the given coordinates.
    BitmapPosition(BitmapCommand<CoordF, P, T>),

    /// Draws bitmapped text using the given transformation matrix.
    BitmapTransform(BitmapCommand<Mat4, P, T>),

    /// Draws vector text of the specified size at the given coordinates.
    VectorPosition(VectorCommand<CoordF, P, T>),

    /// Draws vector text of the specified size using the given transformation matrix.
    VectorTransform(VectorCommand<Mat4, P, T>),
}

impl<P, T> Command<P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub(crate) fn bitmap_font(&self) -> Option<&Shared<BitmapFont<P>, P>> {
        match self {
            Command::BitmapPosition(cmd) => Some(&cmd.font),
            Command::BitmapTransform(cmd) => Some(&cmd.font),
            _ => None,
        }
    }

    pub(crate) fn position(&self) -> Option<CoordF> {
        match self {
            Self::BitmapPosition(cmd) => Some(cmd.layout),
            Self::VectorPosition(cmd) => Some(cmd.layout),
            _ => None,
        }
    }

    pub(crate) fn transform(&self) -> Option<Mat4> {
        match self {
            Self::BitmapTransform(cmd) => Some(cmd.layout),
            Self::VectorTransform(cmd) => Some(cmd.layout),
            _ => None,
        }
    }

    pub(crate) fn vector_font(&self) -> Option<&Shared<VectorFont, P>> {
        match self {
            Command::VectorPosition(cmd) => Some(&cmd.font),
            Command::VectorTransform(cmd) => Some(&cmd.font),
            _ => None,
        }
    }

    pub(crate) fn glyph_color(&self) -> AlphaColor {
        match self {
            Self::BitmapPosition(cmd) => cmd.glyph_color,
            Self::BitmapTransform(cmd) => cmd.glyph_color,
            Self::VectorPosition(cmd) => cmd.glyph_color,
            Self::VectorTransform(cmd) => cmd.glyph_color,
        }
    }

    pub(crate) fn outline_color(&self) -> Option<AlphaColor> {
        match self {
            Self::BitmapPosition(cmd) => cmd.outline_color,
            Self::BitmapTransform(cmd) => cmd.outline_color,
            _ => None,
        }
    }

    pub(crate) fn size(&self) -> f32 {
        match self {
            Command::BitmapPosition(_) | Command::BitmapTransform(_) => 1.0, // TODO: Offer scaled bitmap text?
            Command::VectorPosition(cmd) => cmd.size,
            Command::VectorTransform(cmd) => cmd.size,
        }
    }

    pub(crate) fn text(&self) -> &str {
        match self {
            Command::BitmapPosition(cmd) => cmd.text.as_ref(),
            Command::BitmapTransform(cmd) => cmd.text.as_ref(),
            Command::VectorPosition(cmd) => cmd.text.as_ref(),
            Command::VectorTransform(cmd) => cmd.text.as_ref(),
        }
    }
}

impl<P, T> IntoIterator for Command<P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    type Item = Command<P, T>;
    type IntoIter = Once<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        once(self)
    }
}

/// An expressive type which allows specification of individual bitmapped text operations.
///
/// If you want to set the bitmapped text outline color you will need to construct one of these
/// instances and build it into a `Text` instance. For example:
///
/// ```rust
/// # use screen_13::prelude_rc::*;
/// # let gpu = Gpu::offscreen();
/// # let awesome_font = gpu.load_bitmap_font().unwrap();
/// let cmd: Text = BitmapText::position(
///         Coord::new(0, 0),
///         &awesome_font,
///         "I have a 1px border color!",
///     )
///     .with_glygh_color(screen_13::color::WHITE)
///     .with_outline_color(screen_13::color::RED)
///     .build();
/// ```
pub struct BitmapCommand<L, P, T>
where
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    /// The font face to render.
    pub font: Shared<BitmapFont<P>, P>,

    /// The color of the font face.
    pub glyph_color: AlphaColor,

    /// The position or general transform matrix.
    pub layout: L,

    /// The outline color of the font face.
    pub outline_color: Option<AlphaColor>,

    /// The text.
    pub text: T,
}

impl<P, T> BitmapCommand<CoordF, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Constructs a renderable command from the given instance.
    pub fn build(self) -> Command<P, T> {
        Command::BitmapPosition(self)
    }

    /// Renders bitmapped text at the given position.
    pub fn position<X>(pos: X, font: &Shared<BitmapFont<P>, P>, text: T) -> Self
    where
        X: Into<CoordF>,
    {
        Self {
            font: Shared::clone(font),
            glyph_color: WHITE.into(),
            layout: pos.into(),
            outline_color: None,
            text,
        }
    }
}

impl<P, T> BitmapCommand<Mat4, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Constructs a renderable command from the given instance.
    pub fn build(self) -> Command<P, T> {
        Command::BitmapTransform(self)
    }

    /// Renders bitmapped text using the the given transform matrix.
    pub fn transform(layout: Mat4, font: &Shared<BitmapFont<P>, P>, text: T) -> Self {
        Self {
            font: Shared::clone(font),
            glyph_color: WHITE.into(),
            layout,
            outline_color: None,
            text,
        }
    }
}

impl<L, P, T> BitmapCommand<L, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Draws text using the given glyph fill color.
    ///
    /// **_NOTE:_** This is the primary font color.
    pub fn with_glygh_color<C>(mut self, color: C) -> Self
    where
        C: Into<AlphaColor>,
    {
        self.glyph_color = color.into();
        self
    }

    /// Draws text using the given glyph outline color.
    ///
    /// **_NOTE:_** This is the secondary font color.
    pub fn with_outline_color<C>(self, color: C) -> Self
    where
        C: Into<AlphaColor>,
    {
        self.with_outline_color_is(Some(color))
    }

    /// Draws text using the given glyph outline color, if set.
    ///
    /// **_NOTE:_** This is the secondary font color.
    pub fn with_outline_color_is<C>(mut self, color: Option<C>) -> Self
    where
        C: Into<AlphaColor>,
    {
        self.outline_color = color.map(|color| color.into());
        self
    }
}

/// An expressive type which allows specification of vector text operations.
///
/// In order to set the glyph height you will need to construct one of these instances and build it
/// into a `Text` instance. For example:
///
/// ```rust
/// # use fontdue::Font;
/// # use screen_13::prelude_all::*;
/// # let gpu = Gpu::offscreen();
/// # let awesome_font = gpu.load_outline_font().unwrap();
/// let cmd: Text = VectorText::position(
///         Coord::new(0, 0),
///         &awesome_font,
///         "My letters are 48px tall",
///     )
///     .with_size(48)
///     .build();
/// ```
pub struct VectorCommand<L, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// The font face to render.
    pub font: Shared<VectorFont, P>,

    /// The color of the font face.
    pub glyph_color: AlphaColor,

    /// The position or general transform matrix.
    pub layout: L,

    /// The glyph height, in pixels, to render.
    pub size: f32,

    /// The text.
    pub text: T,
}

impl<P, T> VectorCommand<CoordF, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Constructs a renderable command from the given instance.
    pub fn build(self) -> Command<P, T> {
        Command::VectorPosition(self)
    }

    /// Renders vector text at the given position.
    pub fn position<X>(pos: X, font: &Shared<VectorFont, P>, text: T) -> Self
    where
        X: Into<CoordF>,
    {
        Self {
            font: Shared::clone(font),
            glyph_color: WHITE.into(),
            layout: pos.into(),
            size: 40.0,
            text,
        }
    }
}

impl<P, T> VectorCommand<Mat4, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Constructs a renderable command from the given instance.
    pub fn build(self) -> Command<P, T> {
        Command::VectorTransform(self)
    }

    /// Renders outlined vector text using the the given transform matrix.
    pub fn transform(layout: Mat4, font: &Shared<VectorFont, P>, text: T) -> Self {
        Self {
            font: Shared::clone(font),
            glyph_color: WHITE.into(),
            layout,
            size: 40.0,
            text,
        }
    }
}

impl<L, P, T> VectorCommand<L, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Draws text using the given glyph fill color.
    ///
    /// **_NOTE:_** This is the primary font color.
    pub fn with_glygh_color<C>(mut self, color: C) -> Self
    where
        C: Into<AlphaColor>,
    {
        self.glyph_color = color.into();
        self
    }

    /// Draws text using the given size, in pixels.
    pub fn with_size<S>(mut self, size: S) -> Self
    where
        S: Into<f32>,
    {
        self.size = size.into();
        self
    }
}
