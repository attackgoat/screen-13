use {
    super::{Device, Dim, Driver, Memory},
    crate::math::Extent,
    gfx_hal::{
        device::Device as _,
        format::Format,
        image::{Kind, Tiling, Usage, ViewCapabilities},
        memory::Properties,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        marker::PhantomData,
        ops::{Deref, DerefMut},
    },
    typenum::U2,
};

pub struct Image<D>
where
    D: Dim,
{
    __: PhantomData<D>,
    driver: Driver,
    mem: Memory, // TODO: Remove! This should not be here!
    ptr: Option<<_Backend as Backend>::Image>,
}

impl<D> Image<D>
where
    D: Dim,
{
    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(feature = "debug-names")]
    pub fn set_name(image: &mut Self, name: &str) {
        let device = image.driver.borrow();
        let ptr = image.ptr.as_mut().unwrap();

        unsafe {
            device.set_image_name(ptr, name);
        }
    }
}

/// Specialized new function for 2D images
#[allow(clippy::too_many_arguments)]
impl Image<U2> {
    pub fn new_optimal(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        dims: Extent,
        layers: u16,
        samples: u8,
        mips: u8,
        fmt: Format,
        usage: Usage,
    ) -> Self {
        let (image, mem) = unsafe {
            let kind = Kind::D2(dims.x, dims.y, layers, samples);
            let mut image = driver
                .borrow()
                .create_image(
                    kind,
                    mips,
                    fmt,
                    Tiling::Optimal,
                    usage,
                    ViewCapabilities::MUTABLE_FORMAT,
                )
                .unwrap();

            let device = driver.borrow();

            #[cfg(feature = "debug-names")]
            device.set_image_name(&mut image, name);

            let req = device.get_image_requirements(&image);
            let mem_type =
                Device::mem_ty(&device, req.type_mask, Properties::DEVICE_LOCAL).unwrap();
            let mem = Memory::new(driver, mem_type, req.size);
            device.bind_image_memory(&mem, 0, &mut image).unwrap();

            (image, mem)
        };

        Self {
            __: PhantomData,
            driver: Driver::clone(driver),
            mem,
            ptr: Some(image),
        }
    }
}

impl<D> AsMut<<_Backend as Backend>::Image> for Image<D>
where
    D: Dim,
{
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Image {
        &mut *self
    }
}

impl<D> AsRef<<_Backend as Backend>::Image> for Image<D>
where
    D: Dim,
{
    fn as_ref(&self) -> &<_Backend as Backend>::Image {
        &*self
    }
}

impl<D> Deref for Image<D>
where
    D: Dim,
{
    type Target = <_Backend as Backend>::Image;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl<D> DerefMut for Image<D>
where
    D: Dim,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl<D> Drop for Image<D>
where
    D: Dim,
{
    fn drop(&mut self) {
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_image(ptr);
        }
    }
}
