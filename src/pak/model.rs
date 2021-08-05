use {
    super::IndexType,
    crate::math::{Mat4, Sphere},
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        fmt::{Debug, Error, Formatter},
        iter::FromIterator,
        ops::Range,
    },
};

// TODO: Probably make a bunch of these fields public
#[derive(Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    base_vertex: Option<u32>,
    pub(crate) bounds: Sphere,
    pub(crate) indices: Range<u32>,
    name: Option<String>,
    skin_inv_binds: Option<HashMap<String, Mat4>>,
    transform: Option<Mat4>,
    vertex_count: u32,
    vertex_offset: u32,
}

impl Mesh {
    pub(crate) fn new_indexed<N: Into<Option<String>>>(
        name: N,
        indices: Range<u32>,
        vertex_count: u32,
        vertex_offset: u32,
        bounds: Sphere,
        transform: Option<Mat4>,
        skin_inv_binds: Option<HashMap<String, Mat4>>,
    ) -> Self {
        Self {
            base_vertex: None,
            bounds,
            indices,
            name: name.into(),
            skin_inv_binds,
            transform,
            vertex_count,
            vertex_offset,
        }
    }

    // The number of (same sized) vertices that appear before this one in the vertex buffer, by simple
    // division of the position and stride of the vertices of this mesh.
    pub fn base_vertex(&self) -> u32 {
        self.base_vertex.unwrap()
    }

    pub fn is_animated(&self) -> bool {
        self.skin_inv_binds.is_some()
    }

    pub fn mesh_with_vertex_count<N>(vertex_count: u32) -> Builder<N> {
        Builder::new(vertex_count)
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn set_base_vertex(&mut self, val: u32) {
        self.base_vertex = Some(val);
    }

    pub(crate) fn set_vertex_offset(&mut self, val: u32) {
        self.vertex_offset = val;
    }

    pub(crate) fn skin_inv_binds(&self) -> impl Iterator<Item = &Mat4> {
        self.skin_inv_binds.as_ref().unwrap().values()
    }

    pub fn transform(&self) -> Option<Mat4> {
        self.transform
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Offset in the vertex buffer, in bytes, where our first vertex begins
    pub fn vertex_offset(&self) -> u32 {
        self.vertex_offset
    }
}

impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Mesh")
    }
}

impl From<u32> for Mesh {
    fn from(vertex_count: u32) -> Self {
        Self::mesh_with_vertex_count::<String>(vertex_count).into()
    }
}

impl<N> From<Builder<N>> for Mesh
where
    N: AsRef<str>,
{
    fn from(builder: Builder<N>) -> Self {
        builder.build()
    }
}

#[derive(Debug)]
pub struct Builder<N> {
    name: Option<N>,
    bounds: Option<Sphere>,
    indices: Option<Range<u32>>,
    skin_inv_binds: Option<HashMap<String, Mat4>>,
    transform: Option<Mat4>,
    vertex_count: u32,
}

impl<N> Builder<N> {
    pub fn new(vertex_count: u32) -> Self {
        Self {
            vertex_count,
            ..Self::default()
        }
    }

    pub fn with_bounds(self, bounds: Sphere) -> Self {
        self.with_bounds_is(Some(bounds))
    }

    pub fn with_bounds_is(mut self, bounds: Option<Sphere>) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn with_indices(self, indices: Range<u32>) -> Self {
        self.with_indices_is(Some(indices))
    }

    pub fn with_indices_is(mut self, indices: Option<Range<u32>>) -> Self {
        self.indices = indices;
        self
    }

    pub fn with_skin<S: IntoIterator<Item = (String, Mat4)>>(self, skin: S) -> Self {
        self.with_skin_is(Some(skin))
    }

    pub fn with_skin_is<S: IntoIterator<Item = (String, Mat4)>>(mut self, skin: Option<S>) -> Self {
        self.skin_inv_binds = skin.map(HashMap::from_iter);
        self
    }

    pub fn with_transform(self, transform: Mat4) -> Self {
        self.with_transform_is(Some(transform))
    }

    pub fn with_transform_is(mut self, transform: Option<Mat4>) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_vertex_count(mut self, vertex_count: u32) -> Self {
        self.vertex_count = vertex_count;
        self
    }
}

impl<N> Builder<N>
where
    N: AsRef<str>,
{
    pub fn with_name(self, name: N) -> Self {
        self.with_name_is(Some(name))
    }

    pub fn with_name_is(mut self, name: Option<N>) -> Self {
        self.name = name;
        self
    }

    pub fn build(self) -> Mesh {
        Mesh {
            base_vertex: None,
            bounds: Default::default(),
            indices: self.indices.unwrap_or_default(),
            name: self.name.map(|name| name.as_ref().to_owned()),
            skin_inv_binds: self.skin_inv_binds,
            transform: self.transform,
            vertex_count: self.vertex_count,
            vertex_offset: 0,
        }
    }
}

impl<N> Default for Builder<N> {
    fn default() -> Self {
        Self {
            name: None,
            bounds: None,
            indices: None,
            skin_inv_binds: None,
            transform: None,
            vertex_count: 0,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct ModelBuf {
    idx_ty: IndexType,

    #[serde(with = "serde_bytes")]
    indices: Vec<u8>,

    meshes: Vec<Mesh>,

    #[serde(with = "serde_bytes")]
    vertices: Vec<u8>,

    #[serde(with = "serde_bytes")]
    write_mask: Vec<u8>,
}

impl ModelBuf {
    pub(crate) fn new(
        mut meshes: Vec<Mesh>,
        idx_ty: IndexType,
        indices: Vec<u8>,
        vertices: Vec<u8>,
        write_mask: Vec<u8>,
    ) -> Self {
        assert_ne!(meshes.len(), 0);
        assert_ne!(indices.len(), 0);
        assert_ne!(vertices.len(), 0);
        assert_ne!(write_mask.len(), 0);

        // Filtering relies on meshes being sorted by name
        meshes.sort_unstable_by(|lhs, rhs| lhs.name().cmp(&rhs.name()));

        Self {
            idx_ty,
            indices,
            meshes,
            vertices,
            write_mask,
        }
    }

    pub(crate) fn idx_ty(&self) -> IndexType {
        self.idx_ty
    }

    pub(crate) fn indices(&self) -> &[u8] {
        &self.indices
    }

    pub(crate) fn take_meshes(self) -> Vec<Mesh> {
        self.meshes
    }

    pub(crate) fn vertices(&self) -> &[u8] {
        &self.vertices
    }

    pub(crate) fn write_mask(&self) -> &[u8] {
        &self.write_mask
    }
}

impl Debug for ModelBuf {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("ModelBuf")
    }
}
