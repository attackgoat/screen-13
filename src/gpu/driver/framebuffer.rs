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
    #[cfg(debug_assertions)]
    pub fn rename(frame_buf: &mut Self, name: &str) {
        let device = frame_buf.driver.as_ref().borrow();
        let ptr = frame_buf.ptr.as_mut().unwrap();

        unsafe {
            device.set_framebuffer_name(ptr, name);
        }
    }
}

// TODO: Allow naming these in the ctor for debug purposes! Gfx supports it!

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
        let frame_buf = {
            let device = driver.as_ref().borrow();

            unsafe {
                device.create_framebuffer(
                    render_pass,
                    image_views,
                    ImageExtent {
                        width: dims.x,
                        height: dims.y,
                        depth: 1,
                    },
                )
            }
            .unwrap()
        };

        Self {
            __: Default::default(),
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
