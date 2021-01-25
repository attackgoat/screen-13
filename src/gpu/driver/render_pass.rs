use {
    crate::gpu::device,
    gfx_hal::{
        device::Device as _,
        pass::{Attachment, Subpass, SubpassDependency, SubpassDesc},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        iter::empty,
        ops::{Deref, DerefMut},
    },
};

pub struct RenderPass(Option<<_Backend as Backend>::RenderPass>);

impl RenderPass {
    pub unsafe fn new<'s, Ia, Is>(
        #[cfg(feature = "debug-names")] name: &str,
        attachments: Ia,
        subpasses: Is,
    ) -> Self
    where
        Ia: IntoIterator<Item = Attachment>,
        Ia::IntoIter: ExactSizeIterator,
        Is: IntoIterator<Item = SubpassDesc<'s>>,
        Is::IntoIter: ExactSizeIterator,
    {
        Self::new_dependencies(
            #[cfg(feature = "debug-names")]
            name,
            attachments,
            subpasses,
            empty(),
        )
    }

    pub unsafe fn new_dependencies<'s, Ia, Is, Id>(
        #[cfg(feature = "debug-names")] name: &str,
        attachments: Ia,
        subpasses: Is,
        dependencies: Id,
    ) -> Self
    where
        Ia: IntoIterator<Item = Attachment>,
        Ia::IntoIter: ExactSizeIterator,
        Is: IntoIterator<Item = SubpassDesc<'s>>,
        Is::IntoIter: ExactSizeIterator,
        Id: IntoIterator<Item = SubpassDependency>,
        Id::IntoIter: ExactSizeIterator,
    {
        let ctor = || {
            device()
                .create_render_pass(attachments, subpasses, dependencies)
                .unwrap()
        };

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_render_pass_name(&mut ptr, name);

        Self(Some(ptr))
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(render_pass: &mut Self, name: &str) {
        let ptr = render_pass.0.as_mut().unwrap();
        device().set_render_pass_name(ptr, name);
    }

    pub fn subpass(main_pass: &Self, index: u8) -> Subpass<'_, _Backend> {
        Subpass { index, main_pass }
    }
}

impl AsMut<<_Backend as Backend>::RenderPass> for RenderPass {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::RenderPass {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::RenderPass> for RenderPass {
    fn as_ref(&self) -> &<_Backend as Backend>::RenderPass {
        &*self
    }
}

impl Deref for RenderPass {
    type Target = <_Backend as Backend>::RenderPass;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for RenderPass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_render_pass(ptr);
        }
    }
}
