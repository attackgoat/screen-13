use {
    super::{Dim, RenderPass},
    crate::{gpu::device, math::Extent},
    gfx_hal::{device::Device as _, image::FramebufferAttachment, Backend},
    gfx_impl::Backend as _Backend,
    std::{
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
    ptr: Option<<_Backend as Backend>::Framebuffer>,
}

impl<D> Framebuffer<D>
where
    D: Dim,
{
    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(frame_buf: &mut Self, name: &str) {
        let ptr = frame_buf.ptr.as_mut().unwrap();
        device().set_framebuffer_name(ptr, name);
    }
}

// TODO: Allow naming these in the ctor for debug purposes! Gfx supports it!

impl Framebuffer<U2> {
    /// Specialized new function for 2D framebuffers
    pub unsafe fn new<I>(
        #[cfg(feature = "debug-names")] name: &str,
        render_pass: &RenderPass,
        attachments: I,
        dims: Extent,
    ) -> Self
    where
        I: Iterator<Item = FramebufferAttachment>,
    {
        let ctor = || {
            device()
                .create_framebuffer(render_pass, attachments, dims.as_extent_depth(1))
                .unwrap()
        };

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_framebuffer_name(&mut ptr, name);

        Self {
            __: PhantomData,
            ptr: Some(ptr),
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
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device().destroy_framebuffer(ptr);
        }
    }
}
