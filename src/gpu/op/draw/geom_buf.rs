use {
    crate::{
        gpu::{
            pool::{Lease, Pool},
            Texture2d,
        },
        math::Extent,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    gfx_hal::{
        format::{Format, ImageFeature},
        image::{Layout, Usage as ImageUsage},
    },
};

pub struct GeometryBuffer<P>
where
    P: SharedPointerKind,
{
    pub color_metal: Lease<Texture2d, P>,
    pub normal_rough: Lease<Texture2d, P>,
    pub light: Lease<Texture2d, P>,
    pub output: Lease<Texture2d, P>,
    pub depth: Lease<Texture2d, P>,
}

impl<P> GeometryBuffer<P>
where
    P: SharedPointerKind,
{
    pub unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        dims: Extent,
        output_fmt: Format,
    ) -> Self {
        let (geom_fmt, light_fmt, depth_fmt) = {
            let geom = pool
                .best_fmt(
                    &[Format::Rgba8Unorm],
                    ImageFeature::COLOR_ATTACHMENT | ImageFeature::SAMPLED,
                )
                .unwrap();
            let light = pool
                .best_fmt(
                    &[Format::Rgba32Uint, Format::Rgba32Sfloat],
                    ImageFeature::COLOR_ATTACHMENT
                        | ImageFeature::COLOR_ATTACHMENT_BLEND
                        | ImageFeature::SAMPLED,
                )
                .unwrap();
            let depth = pool
                .best_fmt(
                    &[Format::D24UnormS8Uint],
                    ImageFeature::DEPTH_STENCIL_ATTACHMENT | ImageFeature::SAMPLED,
                )
                .unwrap();
            (geom, light, depth)
        };

        let color_metal = pool.texture(
            #[cfg(feature = "debug-names")]
            &format!("{} (Color)", name),
            dims,
            geom_fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT | ImageUsage::SAMPLED,
            1,
            1,
            1,
        );
        let normal_rough = pool.texture(
            #[cfg(feature = "debug-names")]
            &format!("{} (Normal)", name),
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
