pub mod prelude_arc {
    pub use super::*;

    use screen_13::ptr::ArcK as P;

    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
    pub type ModelLoader = super::ModelLoader<P>;
}

pub mod prelude_rc {
    pub use super::*;

    use screen_13::ptr::RcK as P;

    pub type ComputePresenter = super::ComputePresenter<P>;
    pub type GraphicPresenter = super::GraphicPresenter<P>;
    pub type ImageLoader = super::ImageLoader<P>;
    pub type ModelLoader = super::ModelLoader<P>;
}

pub mod text;

mod res {
    pub mod shader {
        include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
    }
}

mod clear;
mod copy;
mod image;
mod model;
mod present;

pub use self::{
    clear::{clear_color_binding, clear_color_node},
    copy::{
        copy_buffer_binding, copy_buffer_binding_region, copy_buffer_binding_regions,
        copy_buffer_binding_to_image, copy_buffer_binding_to_image_region,
        copy_buffer_binding_to_image_regions, copy_image_binding, copy_image_binding_region,
        copy_image_binding_regions, copy_image_node, copy_image_node_region,
        copy_image_node_regions,
    },
    image::ImageLoader,
    model::ModelLoader,
    present::{ComputePresenter, GraphicPresenter},
};
