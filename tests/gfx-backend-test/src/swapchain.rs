use {super::*, std::borrow::Borrow};

#[derive(Debug)]
pub struct SwapchainImageMock;

impl Borrow<ImageMock> for SwapchainImageMock {
    fn borrow(&self) -> &ImageMock {
        unimplemented!()
    }
}
impl Borrow<()> for SwapchainImageMock {
    fn borrow(&self) -> &() {
        unimplemented!()
    }
}
