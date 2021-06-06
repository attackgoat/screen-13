use {
    crate::{
        gpu::{
            pool::{Lease, Pool},
            Texture2d,
        },
        math::Extent,
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::{
        format::{Format, ImageFeature},
        image::{Layout, Usage as ImageUsage},
    },
};

type BackBuffer<P> = Lease<Shared<Texture2d, P>, P>;

pub struct GeometryBuffer<P>([BackBuffer<P>; 5])
where
    P: SharedPointerKind;

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

        Self([color_metal, normal_rough, light, output, depth])
    }

    pub fn color_metal(&self) -> &Texture2d {
        &self.0[0]
    }

    pub fn depth(&self) -> &Texture2d {
        &self.0[4]
    }

    pub fn light(&self) -> &Texture2d {
        &self.0[2]
    }

    pub fn normal_rough(&self) -> &Texture2d {
        &self.0[1]
    }

    pub fn output(&self) -> &Texture2d {
        &self.0[3]
    }
}
