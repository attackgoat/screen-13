use {
    super::{Driver, Memory, PhysicalDevice},
    crate::math::Extent,
    gfx_hal::{
        device::Device,
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
    typenum::{U2, U3},
};

pub trait Dim {}

impl Dim for U2 {}

impl Dim for U3 {}

#[derive(Debug)]
pub struct Image<D>
where
    D: Dim,
{
    __: PhantomData<D>,
    driver: Driver,
    mem: Memory,
    ptr: Option<<_Backend as Backend>::Image>,
}

/// Specialized new function for 2D images
#[allow(clippy::too_many_arguments)]
impl Image<U2> {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        dims: Extent,
        layers: u16,
        samples: u8,
        mips: u8,
        format: Format,
        tiling: Tiling,
        usage: Usage,
    ) -> Self {
        let (image, mem) = unsafe {
            let kind = Kind::D2(dims.x, dims.y, layers, samples);
            let mut image = driver
                .borrow()
                .create_image(
                    kind,
                    mips,
                    format,
                    tiling,
                    usage,
                    ViewCapabilities::MUTABLE_FORMAT,
                )
                .unwrap();

            let device = driver.borrow();

            #[cfg(debug_assertions)]
            device.set_image_name(&mut image, name);

            let req = device.get_image_requirements(&image);
            let mem_type = device.mem_ty(req.type_mask, Properties::DEVICE_LOCAL);
            let mem = Memory::new(Driver::clone(&driver), mem_type, req.size);
            device.bind_image_memory(&mem, 0, &mut image).unwrap();

            (image, mem)
        };

        Self {
            __: Default::default(),
            driver,
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
