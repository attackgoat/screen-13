use {
    super::RenderReturn,
    crate::{color::Color, math::Extent, DynScreen, Gpu, Input, Render, Screen},
    a_r_c_h_e_r_y::SharedPointerKind,
};

#[cfg(feature = "multi-monitor")]
use crate::math::Area;

/// Displays a solid color forever.
pub struct Solid {
    color: Color,
}

impl Solid {
    /// Constructs a new `Solid` from the given color.
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Solid {
    fn frame<P>(&self, gpu: &Gpu<P>) -> Render<P>
    where
        P: 'static + SharedPointerKind,
    {
        let mut frame = gpu.render(
            #[cfg(feature = "debug-names")]
            &format!("Solid {}", self.color.to_hex()),
            Extent::new(8, 8),
        );
        frame
            .clear(
                #[cfg(feature = "debug-names")]
                "Solid",
            )
            .with(self.color)
            .record();

        frame
    }
}

impl<P> Screen<P> for Solid
where
    P: 'static + SharedPointerKind,
{
    fn render(
        &self,
        gpu: &Gpu<P>,
        #[cfg(not(feature = "multi-monitor"))] _: Extent,
        #[cfg(feature = "multi-monitor")] viewports: &[Area],
    ) -> RenderReturn<P> {
        #[cfg(not(feature = "multi-monitor"))]
        {
            self.frame(gpu)
        }

        #[cfg(feature = "multi-monitor")]
        {
            viewports.map(|_| self.frame(gpu)).collect()
        }
    }

    fn update(self: Box<Self>, _: &Gpu<P>, _: &Input) -> DynScreen<P> {
        self
    }
}
