pub mod prelude_arc {
    pub use super::*;

    use screen_13::ptr::ArcK as P;

    pub type BitmapFont = super::BitmapFont<P>;
    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
}

pub mod prelude_rc {
    pub use super::*;

    use screen_13::ptr::RcK as P;

    pub type BitmapFont = super::BitmapFont<P>;
    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
}

mod bitmap_font;
mod image_loader;
mod presenter;

pub use self::{
    bitmap_font::{BitmapFont, BitmapGlyphColor},
    image_loader::{ImageFormat, ImageLoader},
    presenter::{ComputePresenter, GraphicPresenter},
};
