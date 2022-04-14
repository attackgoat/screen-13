pub mod prelude_arc {
    pub use super::*;

    use screen_13::ptr::ArcK as P;

    pub type BitmapFont = super::text::BitmapFont<P>;
    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
}

pub mod prelude_rc {
    pub use super::*;

    use screen_13::ptr::RcK as P;

    pub type BitmapFont = super::text::BitmapFont<P>;
    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
}

mod res {
    pub mod shader {
        include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
    }
}

mod image;
mod present;
mod text;

pub use self::{
    text::BitmapGlyphColor,
    image::ImageLoader,
    present::{ComputePresenter, GraphicPresenter},
};
