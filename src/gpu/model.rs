use {
    super::{Data, Lease, PoolRef},
    crate::{
        math::{Quat, Sphere},
        pak::model::Mesh as PakMesh,
    },
    std::fmt::{Debug, Error, Formatter},
};

// TODO: Could not force the lifetime to work without an explicit function which means I'm missing something really basic
#[inline]
fn deref_str<'a, S: AsRef<str>>(s: &'a Option<S>) -> Option<&'a str> {
    if let Some(s) = s {
        Some(s.as_ref())
    } else {
        None
    }
}

/// A reference to an individual mesh. Only useful with the Model it was received from.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Mesh(u16);

/// A drawable collection of individually adressable meshes.
pub struct Model {
    index_buf: Lease<Data>,
    meshes: Vec<PakMesh>,
    pool: PoolRef,
    vertex_buf: Lease<Data>,
}

impl Model {
    /// Meshes must be sorted by name
    pub(crate) fn new(
        pool: PoolRef,
        meshes: Vec<PakMesh>,
        index_buf: Lease<Data>,
        vertex_buf: Lease<Data>,
    ) -> Self {
        Self {
            index_buf,
            meshes,
            pool,
            vertex_buf,
        }
    }

    pub fn bounds(&self) -> Sphere {
        todo!("Get bounds")
    }

    pub fn mesh<N: AsRef<str>>(&self, name: Option<N>) -> Option<Mesh> {
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

                Some(Mesh(idx as _))
            }
        }
    }

    pub fn pose_bounds(&self, _pose: &Pose) -> Sphere {
        todo!("Get bounds w/ pose")
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
