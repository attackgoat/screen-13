use super::*;

#[derive(Debug)]
pub struct ImageMock {
    kind: Kind,
}

impl ImageMock {
    pub fn new(kind: Kind) -> Self {
        ImageMock { kind }
    }

    pub fn get_requirements(&self) -> Requirements {
        let size = match self.kind {
            Kind::D2(width, height, layers, samples) => {
                assert_eq!(layers, 1, "Multi-layer images are not supported");
                assert_eq!(samples, 1, "Multisampled images are not supported");
                u64::from(width) * u64::from(height)
            }
            _ => unimplemented!("Unsupported image kind"),
        };

        Requirements {
            size,
            alignment: 1,
            type_mask: !0,
        }
    }
}
