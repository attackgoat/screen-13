use {
    super::{BitmapFont, Font, ScalableFont, DEFAULT_SIZE},
    crate::{
        color::{AlphaColor, WHITE},
        math::{vec3, CoordF, Extent, Mat4},
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
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
            Font::Bitmap(font) => Self::Position(BitmapCommand::position(pos, font.clone(), text)),
            Font::Scalable(font) => Self::SizePosition(ScalableCommand::position(pos, font.clone(), text)),
        }
    }

    /// Draws text using the given homogenous transformation matrix.
    pub fn transform<'f, F>(transform: Mat4, font: F, text: T) -> Self
    where
        F: Into<Font<'f, P>>,
    {
        match font.into() {
            Font::Bitmap(font) => Self::Transform(BitmapCommand::transform(transform, font.clone(), text)),
            Font::Scalable(font) => {
                Self::SizeTransform(ScalableCommand::transform(transform, font.clone(), text))
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

    pub(crate) fn is_position(&self) -> bool {
        self.as_position().is_some()
    }

    pub(crate) fn is_transform(&self) -> bool {
        self.as_transform().is_some()
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

pub struct BitmapCommand<L, P, T>
where
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    pub font: Shared<BitmapFont<P>, P>,
    pub glyph_color: AlphaColor,
    pub layout: L,
    pub outline_color: Option<AlphaColor>,
    pub text: T,
}

impl<P, T> BitmapCommand<CoordF, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub fn position<X>(pos: X, font: Shared<BitmapFont<P>, P>, text: T) -> Self
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

impl<P, T> BitmapCommand<Mat4, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub fn transform(layout: Mat4, font: Shared<BitmapFont<P>, P>, text: T) -> Self {
        Self {
            font,
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

pub struct ScalableCommand<L, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub font: Shared<ScalableFont, P>,
    pub glyph_color: AlphaColor,
    pub layout: L,
    pub size: f32,
    pub text: T,
}

impl<P, T> ScalableCommand<CoordF, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub fn position<X>(pos: X, font: Shared<ScalableFont, P>, text: T) -> Self
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

impl<P, T> ScalableCommand<Mat4, P, T>
where
    P: SharedPointerKind,
    T: AsRef<str>,
{
    pub fn transform(layout: Mat4, font: Shared<ScalableFont, P>, text: T) -> Self {
        Self {
            font,
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
