use {
    crate::{
        gpu::{
            pool::{DrawRenderPassMode, Lease, Pool},
            Texture2d,
        },
        math::Extent,
    },
    gfx_hal::image::{Layout, Tiling, Usage as ImageUsage},
};

pub struct GeometryBuffer {
    pub albedo: Lease<Texture2d>,
    pub depth: Lease<Texture2d>,
    pub light: Lease<Texture2d>,
    pub material: Lease<Texture2d>,
    pub normal: Lease<Texture2d>,
}

impl GeometryBuffer {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &mut Pool,
        dims: Extent,
        mode: DrawRenderPassMode,
    ) -> Self {
        let albedo = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Albedo)", name),
            dims,
            Tiling::Optimal,
            mode.albedo,
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
        let depth = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Depth)", name),
            dims,
            Tiling::Optimal,
            mode.depth,
            Layout::Undefined,
            ImageUsage::DEPTH_STENCIL_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let light = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Light)", name),
            dims,
            Tiling::Optimal,
            mode.light,
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
            mode.material,
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
            mode.normal,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );

        Self {
            albedo,
            depth,
            light,
            material,
            normal,
        }
    }
}
