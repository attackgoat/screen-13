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
    pub color_metal: Lease<Texture2d>,
    pub normal_rough: Lease<Texture2d>,
    pub light: Lease<Texture2d>,
    pub output: Lease<Texture2d>,
    pub depth: Lease<Texture2d>,
}

impl GeometryBuffer {
    pub fn new(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        pool: &mut Pool,
        dims: Extent,
        output_fmt: Format,
    ) -> Self {
        let (geom_fmt, light_fmt, depth_fmt) = {
            let device = driver.borrow();
            let geom = Device::best_fmt(
                &device,
                &[Format::Rgba8Unorm],
                ImageFeature::COLOR_ATTACHMENT | ImageFeature::SAMPLED,
            )
            .unwrap();
            let light = Device::best_fmt(
                &device,
                &[Format::Rgba32Uint, Format::Rgba32Sfloat],
                ImageFeature::COLOR_ATTACHMENT
                    | ImageFeature::COLOR_ATTACHMENT_BLEND
                    | ImageFeature::SAMPLED,
            )
            .unwrap();
            let depth = Device::best_fmt(
                &device,
                &[Format::D24UnormS8Uint],
                ImageFeature::DEPTH_STENCIL_ATTACHMENT | ImageFeature::SAMPLED,
            )
            .unwrap();
            (geom, light, depth)
        };

        let color_metal = pool.texture(
            #[cfg(feature = "debug-names")]
            &format!("{} (Color)", name),
            driver,
            dims,
            geom_fmt,
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
        let normal_rough = pool.texture(
            #[cfg(feature = "debug-names")]
            &format!("{} (Normal)", name),
            driver,
            dims,
            geom_fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let light = pool.texture(
            #[cfg(feature = "debug-names")]
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
        let output = pool.texture(
            #[cfg(feature = "debug-names")]
            &format!("{} (Output)", name),
            driver,
            dims,
            output_fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );
        let depth = pool.texture(
            #[cfg(feature = "debug-names")]
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

        Self {
            color_metal,
            normal_rough,
            light,
            output,
            depth,
        }
    }
}
