use {
    super::{file_key, Asset, Canonicalize, Id},
    crate::pak::{IndexType, Mesh, ModelBuf, ModelId},
    glam::{quat, vec3, Mat4, Quat, Vec3},
    gltf::{import, mesh::Mode, Node, Primitive},
    log::{info, trace, warn},
    ordered_float::OrderedFloat,
    serde::Deserialize,
    std::{
        collections::{BTreeMap, BTreeSet, HashMap, HashSet},
        io::Error,
        path::{Path, PathBuf},
        u16,
    },
};

#[cfg(feature = "bake")]
use {super::Writer, parking_lot::Mutex, std::sync::Arc};

#[derive(PartialEq)]
enum TriangleMode {
    #[allow(unused)]
    Fan,
    #[allow(unused)]
    List,
    #[allow(unused)]
    Strip,
}

impl TriangleMode {
    #[allow(unused)]
    fn classify(primitive: &Primitive) -> Option<TriangleMode> {
        match primitive.mode() {
            Mode::TriangleFan => Some(TriangleMode::Fan),
            Mode::Triangles => Some(TriangleMode::List),
            Mode::TriangleStrip => Some(TriangleMode::Strip),
            _ => None,
        }
    }
}

/// Holds a description of individual meshes within a `.glb` or `.gltf` 3D model.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct MeshRef {
    name: String,
    rename: Option<String>,
}

impl MeshRef {
    /// The artist-provided name of a mesh within the model.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Allows the artist-provided name to be different when referenced by a program.
    pub fn rename(&self) -> Option<&str> {
        let rename = self.rename.as_deref();
        if matches!(rename, Some(rename) if rename.trim().is_empty()) {
            None
        } else {
            rename
        }
    }
}

/// Holds a description of `.glb` or `.gltf` 3D models.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Model {
    offset: Option<[OrderedFloat<f32>; 3]>,
    scale: Option<[OrderedFloat<f32>; 3]>,
    src: PathBuf,

    #[serde(rename = "mesh")]
    meshes: Option<Vec<MeshRef>>,
}

impl Model {
    pub fn new(src: impl AsRef<Path>) -> Self {
        Self {
            meshes: None,
            offset: None,
            scale: None,
            src: src.as_ref().to_path_buf(),
        }
    }

    /// Reads and processes 3D model source files into an existing `.pak` file buffer.
    #[cfg(feature = "bake")]
    pub fn bake(
        &self,
        writer: &Arc<Mutex<Writer>>,
        project_dir: impl AsRef<Path>,
        src: Option<impl AsRef<Path>>,
    ) -> Result<ModelId, Error> {
        // Early-out if we have already baked this model
        let asset = self.clone().into();
        if let Some(id) = writer.lock().ctx.get(&asset) {
            return Ok(id.as_model().unwrap());
        }

        // If a source is given it will be available as a key inside the .pak (sources are not
        // given if the asset is specified inline - those are only available in the .pak via ID)
        let key = src.as_ref().map(|src| file_key(&project_dir, &src));
        if let Some(key) = &key {
            // This model will be accessible using this key
            info!("Baking model: {}", key);
        } else {
            // This model will only be accessible using the handle
            info!(
                "Baking model: {} (inline)",
                file_key(&project_dir, self.src())
            );
        }

        let model = self.to_model_buf();

        // Check again to see if we are the first one to finish this
        let mut writer = writer.lock();
        if let Some(id) = writer.ctx.get(&asset) {
            return Ok(id.as_model().unwrap());
        }

        Ok(writer.push_model(model, key))
    }

    pub fn to_model_buf(&self) -> ModelBuf {
        let mut mesh_names: HashMap<&str, Option<&str>> = Default::default();
        for mesh in self.meshes() {
            mesh_names
                .entry(mesh.name())
                .or_insert_with(|| mesh.rename());
        }

        //trace!("Named meshes: {}", mesh_names.len());

        let (doc, bufs, _) = import(self.src()).unwrap();

        //trace!("Buffers: {}", bufs.len());

        let nodes = doc
            .nodes()
            .filter(|node| node.mesh().is_some())
            .map(|node| (node.mesh().unwrap(), node))
            .filter(|(mesh, _)| {
                // If the model asset contains no mesh array then we bake all meshes
                if mesh_names.is_empty() {
                    return true;
                }

                // If the model asset does contain a mesh array then we only bake what is specified
                if let Some(name) = mesh.name() {
                    return mesh_names.contains_key(name);
                }

                false
            })
            .map(|(mesh, node)| {
                (
                    mesh.primitives()
                        .filter(|primitive| {
                            // TODO: Support the other modes; for now export triangulated gltfs
                            matches!(TriangleMode::classify(primitive), Some(TriangleMode::List))
                        })
                        .collect::<Vec<_>>(),
                    mesh,
                    node,
                )
            })
            .collect::<Vec<_>>();

        // trace!(
        //     "Found {}",
        //     nodes
        //         .iter()
        //         .map(|(_, mesh, _)| mesh.name().unwrap_or("UNNAMED"))
        //         .collect::<Vec<_>>()
        //         .join(", ")
        // );

        // Figure out which unique materials are used on these target mesh primitives and convert
        // those to a map of "Mesh Local" material index from "Gltf File" material index
        // This makes the final materials used index as 0, 1, 2, etc
        let material_idxs = nodes
            .iter()
            .flat_map(|(primitives, ..)| primitives)
            .filter_map(|primitive| primitive.material().index())
            .collect::<BTreeSet<_>>()
            .iter()
            .enumerate()
            .map(|(idx, material_idx)| (*material_idx, idx as u8))
            .collect::<BTreeMap<_, _>>();

        let mut idx_buf = vec![];
        let mut vertex_buf = vec![];
        let mut meshes = vec![];

        let mut base_idx = 0;
        for (primitives, mesh, node) in nodes {
            let dst_name = mesh_names
                .get(mesh.name().unwrap_or_default())
                .map(|name| name.map(|name| name.to_owned()))
                .unwrap_or(None);

            // trace!(
            //     "Baking mesh: {} (as {})",
            //     mesh.name().unwrap_or("UNNAMED"),
            //     dst_name.as_deref().unwrap_or("UNNAMED")
            // );

            // The mesh will use either 16 or 32 bit indices
            let max_idx = primitives
                .iter()
                .map(|primitive| {
                    primitive
                        .reader(|buf| bufs.get(buf.index()).map(|data| &*data.0))
                        .read_indices()
                        .map(|indices| indices.into_u32().max().unwrap_or_default())
                        .unwrap_or_default()
                })
                .max()
                .unwrap_or_default();
            let index_ty = if max_idx <= u16::MAX as _ {
                IndexType::U16
            } else {
                IndexType::U32
            };

            let skin = node.skin();
            let transform = Self::get_transform(&node);
            let mut index_count = 0;
            let mut vertex_count = 0;

            for primitive in primitives {
                let data = primitive.reader(|buf| bufs.get(buf.index()).map(|data| &*data.0));

                // Read indices (must have sets of three positions)
                let mut indices = data
                    .read_indices()
                    .map(|indices| indices.into_u32().collect::<Vec<_>>())
                    .unwrap_or_default();
                if indices.is_empty() || indices.len() % 3 != 0 {
                    unimplemented!("Unindexed models are not yet supported");
                }

                // Read positions (must have data)
                let positions = data.read_positions();
                if positions.is_none() {
                    panic!("Malformed primitive");
                }
                let positions = positions.unwrap().collect::<Vec<_>>();

                // Read texture coordinates (must have same length as positions)
                let material_idx = primitive
                    .material()
                    .index()
                    .map(|idx| material_idxs[&idx])
                    .unwrap_or_default();
                let mut tex_coords = data
                    .read_tex_coords(0)
                    .map(|data| data.into_f32())
                    .map(|tex_coords| tex_coords.collect::<Vec<_>>())
                    .unwrap_or_default();
                tex_coords.resize(positions.len(), [0.0, 0.0]);

                // Read (optional) skin
                let joints = data
                    .read_joints(0)
                    .map(|joints| joints.into_u16().collect::<Vec<_>>())
                    .unwrap_or_default();
                let weights = data
                    .read_weights(0)
                    .map(|weights| weights.into_f32().collect::<Vec<_>>())
                    .unwrap_or_default();
                let has_skin = joints.len() == positions.len() && weights.len() == joints.len();

                // Flip triangle front faces from CW to CCW
                for tri_idx in 0..indices.len() / 3 {
                    let a_idx = tri_idx * 3;
                    let c_idx = a_idx + 2;
                    indices.swap(a_idx, c_idx);
                }

                index_count += indices.len();
                vertex_count += positions.len();

                match index_ty {
                    IndexType::U16 => indices
                        .iter()
                        .for_each(|idx| idx_buf.extend_from_slice(&(*idx as u16).to_ne_bytes())),
                    IndexType::U32 => indices
                        .iter()
                        .for_each(|idx| idx_buf.extend_from_slice(&idx.to_ne_bytes())),
                }

                // trace!("{} vertices", positions.len());

                for idx in 0..positions.len() {
                    for dim_idx in 0..3 {
                        vertex_buf.extend_from_slice(&positions[idx][dim_idx].to_ne_bytes());
                    }

                    vertex_buf.push(material_idx);

                    for dim_idx in 0..2 {
                        vertex_buf.extend_from_slice(&tex_coords[idx][dim_idx].to_ne_bytes());
                    }

                    if has_skin {
                        for bone_idx in 0..4 {
                            vertex_buf.extend_from_slice(&joints[idx][bone_idx].to_ne_bytes());
                            vertex_buf.extend_from_slice(&weights[idx][bone_idx].to_ne_bytes());
                        }
                    }
                }
            }

            let skin_inv_binds = skin.map(|s| {
                let joints = s.joints().map(|node| node.name().unwrap().to_owned());
                let inv_binds = s
                    .reader(|buf| bufs.get(buf.index()).map(|data| &*data.0))
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .map(|ibp| Mat4::from_cols_array_2d(&ibp));

                joints.zip(inv_binds).into_iter().collect()
            });

            meshes.push(Mesh {
                index_count: index_count as _,
                index_ty,
                name: dst_name,
                skin_inv_binds,
                transform,
                vertex_count: vertex_count as _,
            });
        }

        ModelBuf::new(meshes, idx_buf, vertex_buf)
    }

    fn get_transform(node: &Node) -> Option<Mat4> {
        let (translation, rotation, scale) = node.transform().decomposed();
        let rotation = quat(rotation[0], rotation[1], rotation[2], rotation[3]);
        let scale = vec3(scale[0], scale[1], scale[2]);
        let translation = vec3(translation[0], translation[1], translation[2]);
        if scale != Vec3::ONE || rotation != Quat::IDENTITY || translation != Vec3::ZERO {
            Some(Mat4::from_scale_rotation_translation(
                scale,
                rotation,
                translation,
            ))
        } else {
            None
        }
    }

    /// The list of meshes within a model.
    pub fn meshes(&self) -> impl Iterator<Item = &MeshRef> {
        self.meshes.iter().flatten()
    }

    fn node_stride(node: &Node) -> usize {
        if node.skin().is_some() {
            88
        } else {
            64
        }
    }

    /// Translation of the model origin.
    pub fn offset(&self) -> Vec3 {
        self.offset
            .map(|offset| vec3(offset[0].0, offset[1].0, offset[2].0))
            .unwrap_or(Vec3::ZERO)
    }

    /// Scaling of the model.
    pub fn scale(&self) -> Vec3 {
        self.scale
            .map(|scale| vec3(scale[0].0, scale[1].0, scale[2].0))
            .unwrap_or(Vec3::ONE)
    }

    /// The model file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

impl Canonicalize for Model {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}
