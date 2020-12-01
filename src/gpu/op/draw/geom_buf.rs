use {
    crate::{
        gpu::{
            pool::{Lease, Pool},
            Texture2d,
        },
        math::Extent,
    },
    gfx_hal::{
        format::Format,
        image::{Layout, Tiling, Usage as ImageUsage},
    },
};

pub struct GeometryBuffer {
    pub albedo_metal: Lease<Texture2d>,
    pub depth: Lease<Texture2d>,
    pub light: Lease<Texture2d>,
    pub normal: Lease<Texture2d>,
}

impl GeometryBuffer {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &mut Pool,
        dims: Extent,
        color_format: Format,
    ) -> Self {
        let albedo_metal = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Albedo/Metal buf)", name),
            dims,
            Tiling::Optimal,
            color_format,
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
            &format!("{} (Depth buf)", name),
            dims,
            Tiling::Optimal,
            Format::D32Sfloat,// TODO: We're just using this format but it's not guaranteed: VK_FORMAT_FEATURE_DEPTH_STENCIL_ATTACHMENT_BIT feature must be supported for at least one of VK_FORMAT_X8_D24_UNORM_PACK32 and VK_FORMAT_D32_SFLOAT, and must be supported for at least one of VK_FORMAT_D24_UNORM_S8_UINT and VK_FORMAT_D32_SFLOAT_S8_UINT.
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
            &format!("{} (Light buf)", name),
            dims,
            Tiling::Optimal,
            Format::R32Uint,
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
            Format::Rgb32Sfloat,// Also need to check this format before use!!!!
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );

        Self {
            albedo_metal,
            depth,
            light,
            normal,
        }
    }
}
