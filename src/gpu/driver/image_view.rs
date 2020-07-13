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
    ptr: Option<<_Backend as Backend>::ImageView>,
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
        let image_view = {
            let device = driver.borrow();

            unsafe { device.create_image_view(&image, view_kind, format, swizzle, range) }.unwrap()
        };

        Self {
            driver,
            ptr: Some(image_view),
        }
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
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for ImageView {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_image_view(ptr);
        }
    }
}
