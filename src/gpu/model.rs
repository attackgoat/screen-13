use {
    super::{Data, Lease},
    crate::{
        math::{Quat, Sphere},
        pak::model::{Builder, Mesh},
    },
    archery::SharedPointerKind,
    gfx_hal::IndexType,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt::{Debug, Error, Formatter},
    },
};

/// Data and length
pub type DataBuffer<P> = (Lease<Data, P>, u64);

/// Data, length, and write mask (1 bit per index; all staged data is indexed)
pub type StagingBuffers<P> = (Lease<Data, P>, u64, Lease<Data, P>);

pub struct MeshIter<'a, P>
where
    P: SharedPointerKind,
{
    filter: Option<MeshFilter>,
    idx: usize,
    model: &'a Model<P>,
}

impl<'a, P> Iterator for MeshIter<'a, P>
where
    P: SharedPointerKind,
{
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

/// A reference to an individual mesh name, which may be shared by multiple meshes.
///
/// It is undefined behavior to use a MeshFilter with any Model other than the one it was received
/// from.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MeshFilter(u16);

/// A drawable collection of individually addressable meshes.
pub struct Model<P>
where
    P: SharedPointerKind,
{
    idx_buf: RefCell<Lease<Data, P>>,
    idx_buf_len: u64,
    idx_ty: IndexType,
    meshes: Vec<Mesh>,
    staging: RefCell<Option<StagingBuffers<P>>>,
    vertex_buf: RefCell<Lease<Data, P>>,
    vertex_buf_len: u64,
}

impl<P> Model<P>
where
    P: SharedPointerKind,
{
    /// Meshes must be sorted by name
    pub(crate) fn new(
        meshes: Vec<Mesh>,
        idx_ty: IndexType,
        idx_buf: DataBuffer<P>,
        vertex_buf: DataBuffer<P>,
        staging: StagingBuffers<P>,
    ) -> Self {
        let (idx_buf, idx_buf_len) = idx_buf;
        let (vertex_buf, vertex_buf_len) = vertex_buf;

        Self {
            idx_buf: RefCell::new(idx_buf),
            idx_buf_len,
            idx_ty,
            meshes,
            staging: RefCell::new(Some(staging)),
            vertex_buf: RefCell::new(vertex_buf),
            vertex_buf_len,
        }
    }

    /// Constructs a `Builder` with the given vertex count.
    pub fn mesh<N>(vertex_count: u32) -> Builder<N> {
        Builder::new(vertex_count)
    }

    /// Gets the `Sphere` which defines the rough bounding area for this model.
    pub fn bounds(&self) -> Sphere {
        todo!("Get bounds")
    }

    /// Gets a small value which allows filtered drawing of this mesh.prelude_all
    ///
    /// A returned `None` indicates the given name was not found in this model.
    ///
    /// _NOTE:_ This API may be a little dangerous because there are no lifetimes or anything
    /// telling you about the danger of getting this value from one `Model` and using it on
    /// another. Just don't do that please, or help me patch it.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # use std::iter::once;
    /// /// Draws any meshes named "Bar" within Foo
    /// fn draw_foo_bar(frame: &mut Render, foo: &Shared<Model>, mat: &Material) {
    ///     let camera = Perspective::default();
    ///     let bar = foo.filter(Some("bar"));
    ///
    ///     if let Some(bar) = bar {
    ///         frame.draw().record(&camera,
    ///             once(Draw::model((foo, bar), mat, Mat4::IDENTITY)),
    ///         );
    ///     }
    /// }
    /// ```
    pub fn filter<N: AsRef<str>>(&self, name: Option<N>) -> Option<MeshFilter> {
        let name_str = name.as_ref().map(|s| s.as_ref());
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

    pub(crate) fn idx_ty(&self) -> IndexType {
        self.idx_ty
    }

    pub(crate) fn idx_buf_ref(&self) -> (Ref<'_, Lease<Data, P>>, u64) {
        (self.idx_buf.borrow(), self.idx_buf_len)
    }

    pub(crate) fn idx_buf_mut(&self) -> (RefMut<'_, Lease<Data, P>>, u64) {
        (self.idx_buf.borrow_mut(), self.idx_buf_len)
    }

    /// Remarks: Guaranteed to be in vertex buffer order (each mesh has a unique block of vertices)
    pub(super) fn meshes(&self) -> MeshIter<'_, P> {
        MeshIter {
            filter: None,
            idx: 0,
            model: self,
        }
    }

    /// Remarks: Guaranteed to be in vertex buffer order (each mesh has a unique block of vertices)
    pub(super) fn meshes_filter(&self, filter: MeshFilter) -> MeshIter<'_, P> {
        MeshIter {
            filter: Some(filter),
            idx: 0,
            model: self,
        }
    }

    /// Remarks: Guaranteed to be in vertex buffer order (each mesh has a unique block of vertices)
    pub(super) fn meshes_filter_is(&self, filter: Option<MeshFilter>) -> MeshIter<'_, P> {
        MeshIter {
            filter,
            idx: 0,
            model: self,
        }
    }

    /// You must submit writes for our buffers if you call this.
    pub(super) fn take_pending_writes(&self) -> Option<StagingBuffers<P>> {
        self.staging.borrow_mut().take()
    }

    /// Gets the `Sphere` which defines the rough bounding area for this model, account for the
    /// given pose.
    pub fn pose_bounds(&self, _pose: &Pose) -> Sphere {
        todo!("Get bounds w/ pose")
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub fn set_name(&mut self, name: &str) {
        unsafe {
            self.idx_buf.borrow_mut().set_name(name);
            self.vertex_buf.borrow_mut().set_name(name);
        }
    }

    pub(crate) fn vertex_buf_ref(&self) -> (Ref<'_, Lease<Data, P>>, u64) {
        (self.vertex_buf.borrow(), self.vertex_buf_len)
    }

    pub(crate) fn vertex_buf_mut(&self) -> (RefMut<'_, Lease<Data, P>>, u64) {
        (self.vertex_buf.borrow_mut(), self.vertex_buf_len)
    }
}

impl<P> Debug for Model<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}

/// TODO
#[derive(Clone, Debug)]
pub struct Pose {
    joints: Vec<Quat>,
}

impl Pose {
    /// TODO
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            joints: Vec::with_capacity(capacity),
        }
    }

    /// TODO
    pub fn joint<N: AsRef<str>>(&self, _name: N) -> Quat {
        // let name = name.as_ref();
        // match self.joints.binary_search_by(|a| name.cmp(&a.0)) {
        //     Err(_) => panic!("Joint not found"),
        //     Ok(idx) => self.joints[idx].1
        // }
        todo!();
    }

    /// TODO
    pub fn set_joint(&mut self, _name: String, _val: Quat) {
        // match self.joints.binary_search_by(|a| name.cmp(&a.0)) {
        //     Err(idx) => self.joints.insert(idx, (name, val)),
        //     Ok(idx) => self.joints[idx].1 = val,
        // }
    }
}
