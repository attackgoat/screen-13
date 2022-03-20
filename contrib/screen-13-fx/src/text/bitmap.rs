use screen_13::prelude_all::*;

#[derive(Debug)]
pub struct BitmapRenderer<P>
where
    P: SharedPointerKind, {
    device: Shared<Device<P>, P>,
}

impl<P> BitmapRenderer<P>
where
    P: SharedPointerKind, {
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self {
            device: Shared::clone(device),
        })
    }

    pub fn text(
        &self,
        graph: &mut RenderGraph<P>,
        image: impl Into<AnyImageNode<P>>,
        font: (),
        str: impl AsRef<str>,
    ) where
        P: 'static,
    {
        
    }
}