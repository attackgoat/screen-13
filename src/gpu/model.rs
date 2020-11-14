// TODO: Not all of these should come from super, remove from parent mod!
use {
    super::{Data, Lease, Sphere},
    std::fmt::{Debug, Error, Formatter},
};

/// A textured and renderable model.
pub struct Model {
    bounds: Sphere,
    vertex_buf: Lease<Data>,
    vertex_count: u32,
}

impl Model {
    pub fn new(bounds: Sphere, vertex_buf: Lease<Data>, vertex_count: u32) -> Self {
        Self {
            bounds,
            vertex_buf,
            vertex_count,
        }
    }

    pub fn bounds(&self) -> Sphere {
        self.bounds
    }

    pub(crate) fn is_animated(&self) -> bool {
        // TODO: This needs to be implemented in some fashion - skys the limit here what should we do? hmmmm
        false
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}
