use {
    super::Driver,
    gfx_hal::{
        device::Device,
        pso::{EntryPoint, Specialization},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct ShaderModule {
    driver: Driver,
    ptr: Option<<_Backend as Backend>::ShaderModule>,
}

impl ShaderModule {
    pub unsafe fn new(driver: &Driver, spirv: &[u32]) -> Self {
        let shader_module = driver
            .as_ref()
            .borrow()
            .create_shader_module(spirv)
            .unwrap();

        Self {
            driver: Driver::clone(driver),
            ptr: Some(shader_module),
        }
    }

    pub fn entry_point(module: &Self) -> EntryPoint<'_, _Backend> {
        Self::entry_point_specialization(module, Specialization::EMPTY)
    }

    pub fn entry_point_specialization<'a>(
        module: &'a Self,
        specialization: Specialization<'a>,
    ) -> EntryPoint<'a, _Backend> {
        EntryPoint {
            entry: "main",
            module,
            specialization,
        }
    }
}

impl AsMut<<_Backend as Backend>::ShaderModule> for ShaderModule {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::ShaderModule {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::ShaderModule> for ShaderModule {
    fn as_ref(&self) -> &<_Backend as Backend>::ShaderModule {
        &*self
    }
}

impl Deref for ShaderModule {
    type Target = <_Backend as Backend>::ShaderModule;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for ShaderModule {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_shader_module(ptr);
        }
    }
}
