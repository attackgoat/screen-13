use {
    super::Device,
    gfx_hal::{
        device::Device as _,
        pass::{Attachment, Subpass, SubpassDependency, SubpassDesc},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut},
    },
};

pub struct RenderPass {
    device: Device,
    ptr: Option<<_Backend as Backend>::RenderPass>,
}

impl RenderPass {
    pub fn new<'s, IA, IS, ID>(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> Self
    where
        IA: IntoIterator,
        IA::Item: Borrow<Attachment>,
        IA::IntoIter: ExactSizeIterator,
        IS: IntoIterator,
        IS::Item: Borrow<SubpassDesc<'s>>,
        IS::IntoIter: ExactSizeIterator,
        ID: IntoIterator,
        ID::Item: Borrow<SubpassDependency>,
        ID::IntoIter: ExactSizeIterator,
    {
        let render_pass = 
            unsafe {
                let ctor = || {
                    device
                        .create_render_pass(attachments, subpasses, dependencies)
                        .unwrap()
                };

                #[cfg(feature = "debug-names")]
                let mut render_pass = ctor();

                #[cfg(not(feature = "debug-names"))]
                let render_pass = ctor();

                #[cfg(feature = "debug-names")]
                device.set_render_pass_name(&mut render_pass, name);

                render_pass
        };

        Self {
            device,
            ptr: Some(render_pass),
        }
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(feature = "debug-names")]
    pub fn set_name(render_pass: &mut Self, name: &str) {
        let device = render_pass.driver.as_ref().borrow();
        let ptr = render_pass.ptr.as_mut().unwrap();

        unsafe {
            device.set_render_pass_name(ptr, name);
        }
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
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for RenderPass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        let ptr = self.ptr.take().unwrap();

        unsafe {
            self.device.destroy_render_pass(ptr);
        }
    }
}
