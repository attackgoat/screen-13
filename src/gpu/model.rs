// TODO: Not all of these should come from super, remove from parent mod!
use {
    super::{Data, Lease},
    crate::pak::Mesh,
    std::fmt::{Debug, Error, Formatter},
};

/// An drawable collection of individually adressable meshes.
pub struct Model {
    index_buf: Lease<Data>,
    meshes: Vec<Mesh>,
    vertex_buf: Lease<Data>,
}

impl Model {
    pub(crate) fn new(meshes: Vec<Mesh>, index_buf: Lease<Data>, vertex_buf: Lease<Data>) -> Self {
        Self {
            index_buf,
            meshes,
            vertex_buf,
        }
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}
