use {
    crate::gpu::device,
    gfx_hal::{
        device::Device as _,
        pso::{EntryPoint, Specialization},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct ShaderModule(Option<<_Backend as Backend>::ShaderModule>);

impl ShaderModule {
    pub unsafe fn new(spirv: &[u32]) -> Self {
        let ptr = device().create_shader_module(spirv).unwrap();

        Self(Some(ptr))
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
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for ShaderModule {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_shader_module(ptr);
        }
    }
}
