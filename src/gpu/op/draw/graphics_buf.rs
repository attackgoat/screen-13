use {
    crate::{
        gpu::{
            driver::Image2d,
            pool::{Lease, Pool},
            TextureRef,
        },
        math::Extent,
    },
    gfx_hal::{
        format::Format,
        image::{Layout, Tiling, Usage as ImageUsage},
    },
};

#[derive(Debug)]
pub struct GraphicsBuffer {
    color: Lease<TextureRef<Image2d>>,
    depth: Lease<TextureRef<Image2d>>,
    material: Lease<TextureRef<Image2d>>,
    normal: Lease<TextureRef<Image2d>>,
    position: Lease<TextureRef<Image2d>>,
}

impl GraphicsBuffer {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &mut Pool,
        dims: Extent,
        format: Format,
    ) -> Self {
        let color = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Color)", name),
            dims,
            Tiling::Optimal,
            format,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::SAMPLED
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );
        let position = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Position)", name),
            dims,
            Tiling::Optimal,
            Format::Rgba16Sfloat,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let normal = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Normal)", name),
            dims,
            Tiling::Optimal,
            Format::Rgba16Sfloat,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let material = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Material)", name),
            dims,
            Tiling::Optimal,
            format,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let depth = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Depth)", name),
            dims,
            Tiling::Optimal,
            Format::D32Sfloat,
            Layout::Undefined,
            ImageUsage::DEPTH_STENCIL_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );

        Self {
            color,
            depth,
            material,
            normal,
            position,
        }
    }

    pub fn color(&self) -> &TextureRef<Image2d> {
        &self.color
    }

    pub fn depth(&self) -> &TextureRef<Image2d> {
        &self.depth
    }

    pub fn material(&self) -> &TextureRef<Image2d> {
        &self.material
    }

    pub fn normal(&self) -> &TextureRef<Image2d> {
        &self.normal
    }

    pub fn position(&self) -> &TextureRef<Image2d> {
        &self.position
    }
}
