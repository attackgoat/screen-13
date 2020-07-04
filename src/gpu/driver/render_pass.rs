use {
    super::Driver,
    gfx_hal::{
        device::Device,
        pass::{Attachment, Subpass, SubpassDependency, SubpassDesc},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut},
    },
};

#[derive(Debug)]
pub struct RenderPass {
    driver: Driver,
    render_pass: Option<<_Backend as Backend>::RenderPass>,
}

impl RenderPass {
    pub fn new<'s, IA, IS, ID>(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> Self
    where
        IA: IntoIterator,
        IA::Item: Borrow<Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<SubpassDesc<'s>>,
        ID: IntoIterator,
        ID::Item: Borrow<SubpassDependency>,
    {
        let render_pass = {
            let device = driver.as_ref().borrow();
            let ctor = || unsafe {
                device
                    .create_render_pass(attachments, subpasses, dependencies)
                    .unwrap()
            };

            #[cfg(debug_assertions)]
            let mut render_pass = ctor();
            #[cfg(not(debug_assertions))]
            let render_pass = ctor();

            #[cfg(debug_assertions)]
            unsafe {
                device.set_render_pass_name(&mut render_pass, name)
            }

            render_pass
        };

        Self {
            driver,
            render_pass: Some(render_pass),
        }
    }

    pub fn subpass(&self, index: u8) -> Subpass<'_, _Backend> {
        Subpass {
            index,
            main_pass: self,
        }
    }
}

impl Deref for RenderPass {
    type Target = <_Backend as Backend>::RenderPass;

    fn deref(&self) -> &Self::Target {
        self.render_pass.as_ref().unwrap()
    }
}

impl DerefMut for RenderPass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.render_pass.as_mut().unwrap()
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_render_pass(self.render_pass.take().unwrap());
        }
    }
}
