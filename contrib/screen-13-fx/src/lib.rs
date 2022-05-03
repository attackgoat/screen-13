pub mod prelude_arc {
    pub use super::*;

    use archery::ArcK as P;

    pub type BitmapFont = super::BitmapFont<P>;
    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
    pub type Transition = super::Transition<P>;
    pub type TransitionPipeline = super::TransitionPipeline<P>;
}

pub mod prelude_rc {
    pub use super::*;

    use archery::RcK as P;

    pub type BitmapFont = super::BitmapFont<P>;
    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
    pub type Transition = super::Transition<P>;
    pub type TransitionPipeline = super::TransitionPipeline<P>;
}

mod bitmap_font;
mod image_loader;
mod presenter;
mod transition;

pub use self::{
    bitmap_font::{BitmapFont, BitmapGlyphColor},
    image_loader::{ImageFormat, ImageLoader},
    presenter::{ComputePresenter, GraphicPresenter},
    transition::{Transition, TransitionPipeline},
};
