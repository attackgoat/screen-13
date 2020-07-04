use {
    super::Driver,
    gfx_hal::{
        device::Device,
        format::{Format, Swizzle},
        image::{SubresourceRange, ViewKind},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct ImageView {
    driver: Driver,
    image_view: Option<<_Backend as Backend>::ImageView>,
}

impl ImageView {
    pub fn new<I>(
        driver: Driver,
        image: I,
        view_kind: ViewKind,
        format: Format,
        swizzle: Swizzle,
        range: SubresourceRange,
    ) -> Self
    where
        I: Deref<Target = <_Backend as Backend>::Image>,
    {
        let image_view = unsafe {
            driver
                .borrow()
                .create_image_view(&image, view_kind, format, swizzle, range)
                .unwrap()
        };

        Self {
            driver,
            image_view: Some(image_view),
        }
    }
}

impl AsRef<<_Backend as Backend>::ImageView> for ImageView {
    fn as_ref(&self) -> &<_Backend as Backend>::ImageView {
        self.image_view.as_ref().unwrap()
    }
}

impl Deref for ImageView {
    type Target = <_Backend as Backend>::ImageView;

    fn deref(&self) -> &Self::Target {
        self.image_view.as_ref().unwrap()
    }
}

impl DerefMut for ImageView {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.image_view.as_mut().unwrap()
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .borrow()
                .destroy_image_view(self.image_view.take().unwrap());
        }
    }
}
