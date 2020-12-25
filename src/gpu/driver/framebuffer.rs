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

pub struct Framebuffer<D>
where
    D: Dim,
{
    __: PhantomData<D>,
    driver: Driver,
    ptr: Option<<_Backend as Backend>::Framebuffer>,
}

impl<D> Framebuffer<D>
where
    D: Dim,
{
    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(feature = "debug-names")]
    pub fn set_name(frame_buf: &mut Self, name: &str) {
        let device = frame_buf.driver.as_ref().borrow();
        let ptr = frame_buf.ptr.as_mut().unwrap();

        unsafe {
            device.set_framebuffer_name(ptr, name);
        }
    }
}

// TODO: Allow naming these in the ctor for debug purposes! Gfx supports it!

impl Framebuffer<U2> {
    /// Specialized new function for 2D framebuffers
    pub fn new<I>(
        #[cfg(feature = "debug-names")] name: &str,
        driver: Driver,
        render_pass: &RenderPass,
        image_views: I,
        dims: Extent,
    ) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<<_Backend as Backend>::ImageView>,
    {
        let frame_buf = {
            let device = driver.as_ref().borrow();

            unsafe {
                let ctor = || {
                    device
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

                #[cfg(feature = "debug-names")]
                let mut frame_buf = ctor();

                #[cfg(not(feature = "debug-names"))]
                let frame_buf = ctor();

                #[cfg(feature = "debug-names")]
                device.set_framebuffer_name(&mut frame_buf, name);

                frame_buf
            }
        };

        Self {
            __: PhantomData,
            driver,
            ptr: Some(frame_buf),
        }
    }
}

impl<D> AsMut<<_Backend as Backend>::Framebuffer> for Framebuffer<D>
where
    D: Dim,
{
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Framebuffer {
        &mut *self
    }
}

impl<D> AsRef<<_Backend as Backend>::Framebuffer> for Framebuffer<D>
where
    D: Dim,
{
    fn as_ref(&self) -> &<_Backend as Backend>::Framebuffer {
        &*self
    }
}

impl<D> Deref for Framebuffer<D>
where
    D: Dim,
{
    type Target = <_Backend as Backend>::Framebuffer;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl<D> DerefMut for Framebuffer<D>
where
    D: Dim,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl<D> Drop for Framebuffer<D>
where
    D: Dim,
{
    fn drop(&mut self) {
        let device = self.driver.as_ref().borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_framebuffer(ptr);
        }
    }
}
