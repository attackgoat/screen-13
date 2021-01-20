use {
    super::{BitmapFont, Font, ScalableFont, DEFAULT_SIZE},
    crate::{
        color::{AlphaColor, WHITE},
        math::{CoordF, Mat4},
    },
    a_r_c_h_e_r_y::SharedPointerKind,
};

/// An expressive type which allows specification of individual text operations.
#[non_exhaustive]
pub enum Command<P, T>
where
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    /// Draws text at the given coordinates.
    Position(BitmapCommand<P, CoordF, T>),

    /// Draws text of the specified size at the given coordinates.
    SizePosition(ScalableCommand<CoordF, T>),

    /// Draws text of the specified size using the given homogenous transformation matrix.
    SizeTransform(ScalableCommand<Mat4, T>),

    /// Draws text using the given homogenous transformation matrix.
    Transform(BitmapCommand<P, Mat4, T>),
}

impl<P, T> Command<P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Draws text at the given coordinates.
    pub fn position<F, X>(font: F, pos: X, text: T) -> Self
    where
        F: Into<Font<P>>,
        X: Into<CoordF>,
    {
        match font.into() {
            Font::Bitmap(font) => Self::Position(BitmapCommand::position(font, pos, text)),
            Font::Scalable(font) => Self::SizePosition(ScalableCommand::position(font, pos, text)),
        }
    }

    /// Draws text using the given homogenous transformation matrix.
    pub fn transform<F>(font: F, transform: Mat4, text: T) -> Self
    where
        F: Into<Font<P>>,
    {
        match font.into() {
            Font::Bitmap(font) => Self::Transform(BitmapCommand::transform(font, transform, text)),
            Font::Scalable(font) => {
                Self::SizeTransform(ScalableCommand::transform(font, transform, text))
            }
        }
    }

    /// Draws text using the given glyph fill color.
    ///
    /// **_NOTE:_** This is the main font color.
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

pub struct BitmapCommand<P, L, T>
where
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    pub font: BitmapFont<P>,
    pub glyph_color: AlphaColor,
    pub layout: L,
    pub outline_color: Option<AlphaColor>,
    pub text: T,
}

impl<P, T> BitmapCommand<P, CoordF, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub fn position<X>(font: BitmapFont<P>, pos: X, text: T) -> Self
    where
        X: Into<CoordF>,
    {
        Self {
            font,
            glyph_color: WHITE.into(),
            layout: pos.into(),
            outline_color: None,
            text,
        }
    }
}

impl<P, T> BitmapCommand<P, Mat4, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub fn transform(font: BitmapFont<P>, layout: Mat4, text: T) -> Self {
        Self {
            font,
            glyph_color: WHITE.into(),
            layout,
            outline_color: None,
            text,
        }
    }
}

impl<P, L, T> BitmapCommand<P, L, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    /// Draws text using the given glyph fill color.
    ///
    /// **_NOTE:_** This is the main font color.
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

pub struct ScalableCommand<L, T>
where
    T: AsRef<str>,
{
    pub font: ScalableFont,
    pub glyph_color: AlphaColor,
    pub layout: L,
    pub size: f32,
    pub text: T,
}

impl<T> ScalableCommand<CoordF, T>
where
    T: AsRef<str>,
{
    pub fn position<X>(font: ScalableFont, pos: X, text: T) -> Self
    where
        X: Into<CoordF>,
    {
        Self {
            font,
            glyph_color: WHITE.into(),
            layout: pos.into(),
            size: DEFAULT_SIZE,
            text,
        }
    }
}

impl<T> ScalableCommand<Mat4, T>
where
    T: AsRef<str>,
{
    pub fn transform(font: ScalableFont, layout: Mat4, text: T) -> Self {
        Self {
            font,
            glyph_color: WHITE.into(),
            layout,
            size: DEFAULT_SIZE,
            text,
        }
    }
}

impl<L, T> ScalableCommand<L, T>
where
    T: AsRef<str>,
{
    /// Draws text using the given glyph fill color.
    ///
    /// **_NOTE:_** This is the main font color.
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
