use {
    crate::gpu::device,
    gfx_hal::{
        device::Device as _,
        format::{Format, Swizzle},
        image::{SubresourceRange, ViewKind},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct ImageView(Option<<_Backend as Backend>::ImageView>);

impl ImageView {
    pub unsafe fn new<I>(
        image: I,
        view_kind: ViewKind,
        format: Format,
        swizzle: Swizzle,
        range: SubresourceRange,
    ) -> Self
    where
        I: Deref<Target = <_Backend as Backend>::Image>,
    {
        let ptr = device()
            .create_image_view(&image, view_kind, format, swizzle, range)
            .unwrap();

        Self(Some(ptr))
    }
}

impl AsMut<<_Backend as Backend>::ImageView> for ImageView {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::ImageView {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::ImageView> for ImageView {
    fn as_ref(&self) -> &<_Backend as Backend>::ImageView {
        &*self
    }
}

impl Deref for ImageView {
    type Target = <_Backend as Backend>::ImageView;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for ImageView {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_image_view(ptr);
        }
    }
}
