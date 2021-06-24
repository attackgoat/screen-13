use {
    super::{BitmapFont, Font, ScalableFont, DEFAULT_SIZE},
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
    /// Draws text at the given coordinates.
    Position(BitmapCommand<CoordF, P, T>),

    /// Draws text of the specified size at the given coordinates.
    SizePosition(ScalableCommand<CoordF, P, T>),

    /// Draws text of the specified size using the given homogenous transformation matrix.
    SizeTransform(ScalableCommand<Mat4, P, T>),

    /// Draws text using the given homogenous transformation matrix.
    Transform(BitmapCommand<Mat4, P, T>),
}

impl<P, T> Command<P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Draws text at the given coordinates.
    pub fn position<'f, F, X>(pos: X, font: F, text: T) -> Self
    where
        F: Into<Font<'f, P>>,
        X: Into<CoordF>,
    {
        match font.into() {
            Font::Bitmap(font) => Self::Position(BitmapCommand::position(pos, font, text)),
            Font::Scalable(font) => Self::SizePosition(ScalableCommand::position(pos, font, text)),
        }
    }

    /// Draws text using the given homogenous transformation matrix.
    pub fn transform<'f, F>(transform: Mat4, font: F, text: T) -> Self
    where
        F: Into<Font<'f, P>>,
    {
        match font.into() {
            Font::Bitmap(font) => Self::Transform(BitmapCommand::transform(transform, font, text)),
            Font::Scalable(font) => {
                Self::SizeTransform(ScalableCommand::transform(transform, font, text))
            }
        }
    }

    pub(crate) fn as_position(&self) -> Option<CoordF> {
        match self {
            Self::SizePosition(cmd) => Some(cmd.layout),
            Self::Position(cmd) => Some(cmd.layout),
            _ => None,
        }
    }

    pub(crate) fn as_transform(&self) -> Option<Mat4> {
        match self {
            Self::SizeTransform(cmd) => Some(cmd.layout),
            Self::Transform(cmd) => Some(cmd.layout),
            _ => None,
        }
    }

    pub(crate) fn font(&self) -> Font<'_, P> {
        match self {
            Command::Position(cmd) => (&cmd.font).into(),
            Command::SizePosition(cmd) => (&cmd.font).into(),
            Command::SizeTransform(cmd) => (&cmd.font).into(),
            Command::Transform(cmd) => (&cmd.font).into(),
        }
    }

    pub(crate) fn glyph_color(&self) -> AlphaColor {
        match self {
            Self::Position(cmd) => cmd.glyph_color,
            Self::SizePosition(cmd) => cmd.glyph_color,
            Self::SizeTransform(cmd) => cmd.glyph_color,
            Self::Transform(cmd) => cmd.glyph_color,
        }
    }

    pub(crate) fn is_position(&self) -> bool {
        self.as_position().is_some()
    }

    pub(crate) fn is_transform(&self) -> bool {
        self.as_transform().is_some()
    }

    pub(crate) fn outline_color(&self) -> Option<AlphaColor> {
        match self {
            Self::Position(cmd) => cmd.outline_color,
            Self::Transform(cmd) => cmd.outline_color,
            _ => None,
        }
    }

    pub(crate) fn text(&self) -> &str {
        match self {
            Command::Position(cmd) => cmd.text.as_ref(),
            Command::SizePosition(cmd) => cmd.text.as_ref(),
            Command::SizeTransform(cmd) => cmd.text.as_ref(),
            Command::Transform(cmd) => cmd.text.as_ref(),
        }
    }

    /// Draws text using the given glyph fill color.
    ///
    /// **_NOTE:_** This is the primary font color.
    pub fn with_glygh_color<C>(self, color: C) -> Self
    where
        C: Into<AlphaColor>,
    {
        match self {
            Self::Position(cmd) => Self::Position(cmd.with_glygh_color(color)),
            Self::SizePosition(cmd) => Self::SizePosition(cmd.with_glygh_color(color)),
            Self::SizeTransform(cmd) => Self::SizeTransform(cmd.with_glygh_color(color)),
            Self::Transform(cmd) => Self::Transform(cmd.with_glygh_color(color)),
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
        Command::Position(self)
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
        Command::Transform(self)
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

/// An expressive type which allows specification of scalable text operations.
///
/// In order to set the glyph height you will need to construct one of these instances and build it
/// into a `Text` instance. For example:
///
/// ```rust
/// # use fontdue::Font;
/// # use screen_13::prelude_all::*;
/// # let gpu = Gpu::offscreen();
/// # let awesome_font = gpu.load_scalable_font().unwrap();
/// let cmd: Text = ScalableText::position(
///         Coord::new(0, 0),
///         &awesome_font,
///         "My letters are 48px tall",
///     )
///     .with_size(48)
///     .build();
/// ```
pub struct ScalableCommand<L, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// The font face to render.
    pub font: Shared<ScalableFont, P>,

    /// The color of the font face.
    pub glyph_color: AlphaColor,

    /// The position or general transform matrix.
    pub layout: L,

    /// The glyph height, in pixels, to render.
    pub size: f32,

    /// The text.
    pub text: T,
}

impl<P, T> ScalableCommand<CoordF, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Constructs a renderable command from the given instance.
    pub fn build(self) -> Command<P, T> {
        Command::SizePosition(self)
    }

    /// Renders scalable text at the given position.
    pub fn position<X>(pos: X, font: &Shared<ScalableFont, P>, text: T) -> Self
    where
        X: Into<CoordF>,
    {
        Self {
            font: Shared::clone(font),
            glyph_color: WHITE.into(),
            layout: pos.into(),
            size: DEFAULT_SIZE,
            text,
        }
    }
}

impl<P, T> ScalableCommand<Mat4, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Constructs a renderable command from the given instance.
    pub fn build(self) -> Command<P, T> {
        Command::SizeTransform(self)
    }

    /// Renders scalable text using the the given transform matrix.
    pub fn transform(layout: Mat4, font: &Shared<ScalableFont, P>, text: T) -> Self {
        Self {
            font: Shared::clone(font),
            glyph_color: WHITE.into(),
            layout,
            size: DEFAULT_SIZE,
            text,
        }
    }
}

impl<L, P, T> ScalableCommand<L, P, T>
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
