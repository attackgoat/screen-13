use {
    super::{Data, Lease, PoolRef},
    crate::{math::{Quat,Sphere},pak::Mesh},
    std::fmt::{Debug, Error, Formatter},
};

/// An drawable collection of individually adressable meshes.
pub struct Model {
    index_buf: Lease<Data>,
    meshes: Vec<Mesh>,
    pool: PoolRef,
    vertex_buf: Lease<Data>,
}

impl Model {
    pub(crate) fn new(pool: PoolRef, meshes: Vec<Mesh>, index_buf: Lease<Data>, vertex_buf: Lease<Data>) -> Self {
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

    pub fn pose_bounds(&self, _pose: &Pose) -> Sphere {
        todo!("Get bounds w/ pose")
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}

pub struct Pose {
    pub(crate) joints: Vec<Quat>
}

impl Pose {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            joints: Vec::with_capacity(capacity)
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
