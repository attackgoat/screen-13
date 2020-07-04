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

#[derive(Debug)]
pub struct ShaderModule {
    driver: Driver,
    shader_module: Option<<_Backend as Backend>::ShaderModule>,
}

impl ShaderModule {
    pub unsafe fn new(driver: Driver, spirv: &[u32]) -> Self {
        let shader_module = driver
            .as_ref()
            .borrow()
            .create_shader_module(spirv)
            .unwrap();

        Self {
            driver,
            shader_module: Some(shader_module),
        }
    }

    pub fn entry_point(&self) -> EntryPoint<'_, _Backend> {
        self.entry_point_specialization(Specialization::EMPTY)
    }

    pub fn entry_point_specialization<'a>(
        &'a self,
        specialization: Specialization<'a>,
    ) -> EntryPoint<'a, _Backend> {
        EntryPoint {
            entry: "main",
            module: self.shader_module.as_ref().unwrap(),
            specialization,
        }
    }
}

impl AsRef<<_Backend as Backend>::ShaderModule> for ShaderModule {
    fn as_ref(&self) -> &<_Backend as Backend>::ShaderModule {
        self.shader_module.as_ref().unwrap()
    }
}

impl Deref for ShaderModule {
    type Target = <_Backend as Backend>::ShaderModule;

    fn deref(&self) -> &Self::Target {
        self.shader_module.as_ref().unwrap()
    }
}

impl DerefMut for ShaderModule {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.shader_module.as_mut().unwrap()
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_shader_module(self.shader_module.take().unwrap());
        }
    }
}
