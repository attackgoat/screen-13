use {
    crate::{
        gpu::{
            driver::Device,
            pool::{Lease, Pool},
            Driver, Texture2d,
        },
        math::Extent,
    },
    gfx_hal::{
        format::{Format, ImageFeature},
        image::{Layout, Usage as ImageUsage},
    },
};

pub struct GeometryBuffer {
    pub albedo: Lease<Texture2d>,
    pub depth: Lease<Texture2d>,
    pub light: Lease<Texture2d>,
    pub material: Lease<Texture2d>,
    pub normal: Lease<Texture2d>,
    pub output: Lease<Texture2d>,
}

impl GeometryBuffer {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        pool: &mut Pool,
        dims: Extent,
        albedo_fmt: Format,
    ) -> Self {
        let albedo = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Albedo)", name),
            driver,
            dims,
            albedo_fmt,
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

        let (depth_fmt, light_fmt, material_fmt, normal_fmt) = {
            let device = driver.borrow();
            let depth_fmt = Device::best_fmt(
                &device,
                &[Format::D24UnormS8Uint],
                ImageFeature::DEPTH_STENCIL_ATTACHMENT | ImageFeature::SAMPLED,
            )
            .unwrap();
            let light_fmt = Device::best_fmt(
                &device,
                &[Format::R32Uint],
                ImageFeature::COLOR_ATTACHMENT
                    | ImageFeature::COLOR_ATTACHMENT_BLEND
                    | ImageFeature::SAMPLED,
            )
            .unwrap();
            let material_fmt = Device::best_fmt(
                &device,
                &[Format::Rg8Unorm],
                ImageFeature::COLOR_ATTACHMENT | ImageFeature::SAMPLED,
            )
            .unwrap();
            let normal_fmt = Device::best_fmt(
                &device,
                &[Format::Rgb32Sfloat],
                ImageFeature::COLOR_ATTACHMENT | ImageFeature::SAMPLED,
            )
            .unwrap();
            (depth_fmt, light_fmt, material_fmt, normal_fmt)
        };
        let depth = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Depth)", name),
            driver,
            dims,
            depth_fmt,
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
            driver,
            dims,
            light_fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let material = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Material)", name),
            driver,
            dims,
            material_fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let normal = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Normal)", name),
            driver,
            dims,
            normal_fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let output = pool.texture(
            #[cfg(debug_assertions)]
            &format!("{} (Output)", name),
            driver,
            dims,
            albedo_fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
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
            output,
        }
    }
}
