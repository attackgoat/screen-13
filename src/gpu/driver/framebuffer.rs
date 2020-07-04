use {
    super::{Dim, Driver, RenderPass},
    crate::math::Extent,
    gfx_hal::{device::Device, image::Extent as ImageExtent, Backend},
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        marker::PhantomData,
        ops::{Deref, DerefMut},
    },
    typenum::U2,
};

#[derive(Debug)]
pub struct Framebuffer<D>
where
    D: Dim,
{
    __: PhantomData<D>,
    driver: Driver,
    frame_buf: Option<<_Backend as Backend>::Framebuffer>,
}

#[doc = "Specialized new function for 2D framebuffers"]
impl Framebuffer<U2> {
    pub fn new<I, II>(
        driver: Driver,
        render_pass: &RenderPass,
        image_views: I,
        dims: Extent,
    ) -> Self
    where
        I: IntoIterator<Item = II>,
        II: Borrow<<_Backend as Backend>::ImageView>,
    {
        let frame_buf = unsafe {
            driver
                .as_ref()
                .borrow()
                .create_framebuffer(
                    render_pass,
                    image_views,
                    ImageExtent {
                        width: dims.x,
                        height: dims.y,
                        depth: 1,
                    },
                )
                .unwrap()
        };

        Self {
            __: PhantomData,
            driver,
            frame_buf: Some(frame_buf),
        }
    }
}

impl<D> AsRef<<_Backend as Backend>::Framebuffer> for Framebuffer<D>
where
    D: Dim,
{
    fn as_ref(&self) -> &<_Backend as Backend>::Framebuffer {
        self.frame_buf.as_ref().unwrap()
    }
}

impl<D> Deref for Framebuffer<D>
where
    D: Dim,
{
    type Target = <_Backend as Backend>::Framebuffer;

    fn deref(&self) -> &Self::Target {
        self.frame_buf.as_ref().unwrap()
    }
}

impl<D> DerefMut for Framebuffer<D>
where
    D: Dim,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.frame_buf.as_mut().unwrap()
    }
}

impl<D> Drop for Framebuffer<D>
where
    D: Dim,
{
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_framebuffer(self.frame_buf.take().unwrap());
        }
    }
}
