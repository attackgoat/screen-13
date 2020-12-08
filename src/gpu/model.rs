use {
    super::{Data, Lease},
    crate::{
        math::{Quat, Sphere},
        pak::{model::Mesh, IndexType},
    },
    std::fmt::{Debug, Error, Formatter},
};

// TODO: Could not force the lifetime to work without an explicit function which means I'm missing something really basic
#[inline]
fn deref_str<S: AsRef<str>>(s: &Option<S>) -> Option<&str> {
    if let Some(s) = s {
        Some(s.as_ref())
    } else {
        None
    }
}

pub struct MeshIter<'a> {
    filter: Option<MeshFilter>,
    idx: usize,
    model: &'a Model,
}

impl<'a> Iterator for MeshIter<'a> {
    type Item = &'a Mesh;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(filter) = self.filter {
            if let Some(mesh) = self.model.meshes.get(filter.0 as usize + self.idx) {
                if mesh.name() == self.model.meshes[self.idx].name() {
                    self.idx += 1;
                    return Some(mesh);
                }
            }

            None
        } else if let Some(mesh) = self.model.meshes.get(self.idx) {
            self.idx += 1;
            Some(mesh)
        } else {
            None
        }
    }
}

/// A reference to an individual mesh name, which may be shared by multiple meshes. Only useful with the Model it was received from.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct MeshFilter(u16);

/// A drawable collection of individually adressable meshes.
pub struct Model {
    index_buf: Lease<Data>,
    index_ty: IndexType,
    meshes: Vec<Mesh>,
    vertex_buf: Lease<Data>,
}

impl Model {
    /// Meshes must be sorted by name
    pub(crate) fn new(
        meshes: Vec<Mesh>,
        index_ty: IndexType,
        index_buf: Lease<Data>,
        vertex_buf: Lease<Data>,
    ) -> Self {
        Self {
            index_buf,
            index_ty,
            meshes,
            vertex_buf,
        }
    }

    pub fn bounds(&self) -> Sphere {
        todo!("Get bounds")
    }

    pub(crate) fn buffers(&self) -> (IndexType, &Data, &Data) {
        (self.index_ty, &self.index_buf, &self.vertex_buf)
    }

    pub fn filter<N: AsRef<str>>(&self, name: Option<N>) -> Option<MeshFilter> {
        let name_str = deref_str(&name);
        match self
            .meshes
            .binary_search_by(|probe| probe.name().cmp(&name_str))
        {
            Err(_) => None,
            Ok(mut idx) => {
                // Rewind to the start of this same-named group
                while idx > 0 {
                    let next_idx = idx - 1;
                    if self.meshes[next_idx].name() == name_str {
                        idx = next_idx;
                    } else {
                        break;
                    }
                }

                Some(MeshFilter(idx as _))
            }
        }
    }

    pub(super) fn meshes(&self, filter: Option<MeshFilter>) -> MeshIter {
        MeshIter {
            filter,
            idx: 0,
            model: self,
        }
    }

    pub fn pose_bounds(&self, _pose: &Pose) -> Sphere {
        todo!("Get bounds w/ pose")
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(debug_assertions)]
    pub fn set_name(&mut self, name: &str) {
        self.index_buf.set_name(name);
        self.vertex_buf.set_name(name);
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}

#[derive(Clone)]
pub struct Pose {
    joints: Vec<Quat>,
}

impl Pose {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            joints: Vec::with_capacity(capacity),
        }
    }

    pub fn joint<N: AsRef<str>>(&self, _name: N) -> Quat {
        // let name = name.as_ref();
        // match self.joints.binary_search_by(|a| name.cmp(&a.0)) {
        //     Err(_) => panic!("Joint not found"),
        //     Ok(idx) => self.joints[idx].1
        // }
        todo!();
    }

    pub fn set_joint(&mut self, _name: String, _val: Quat) {
        // match self.joints.binary_search_by(|a| name.cmp(&a.0)) {
        //     Err(idx) => self.joints.insert(idx, (name, val)),
        //     Ok(idx) => self.joints[idx].1 = val,
        // }
    }
}
