use {
    super::{file_key, Asset, Canonicalize, Handle, ModelBuf, ModelHandle},
    glam::{quat, vec3, Mat4, Quat, Vec3},
    gltf::{import, mesh::Mode, Node, Primitive},
    log::{info, warn},
    ordered_float::OrderedFloat,
    serde::Deserialize,
    std::{
        collections::{HashMap, HashSet},
        path::{Path, PathBuf},
        u16,
    },
};

#[cfg(feature = "bake")]
use super::Writer;

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
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Mesh {
    name: String,
    rename: Option<String>,
}

impl Mesh {
    /// The artist-provided name of a mesh within the model.
    #[allow(unused)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Allows the artist-provided name to be different when referenced by a program.
    #[allow(unused)]
    pub fn rename(&self) -> Option<&str> {
        let rename = self.rename.as_deref();
        if let Some("") = rename {
            None
        } else {
            rename
        }
    }
}

/// Holds a description of `.glb` or `.gltf` 3D models.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Model {
    offset: Option<[OrderedFloat<f32>; 3]>,
    scale: Option<[OrderedFloat<f32>; 3]>,
    src: PathBuf,

    #[serde(rename = "mesh")]
    meshes: Option<Vec<Mesh>>,
}

impl Model {
    #[allow(unused)]
    pub(crate) fn new<P>(src: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            meshes: None,
            offset: None,
            scale: None,
            src: src.as_ref().to_owned(),
        }
    }

    /// Reads and processes 3D model source files into an existing `.pak` file buffer.
    #[allow(unused)]
    #[cfg(feature = "bake")]
    pub(super) fn bake(
        &self,
        context: &mut HashMap<Asset, Handle>,
        pak: &mut Writer,
        project_dir: impl AsRef<Path>,
        src: Option<impl AsRef<Path>>,
    ) -> ModelHandle {
        // Early-out if we have this asset in our context
        let context_key = self.clone().into();
        if let Some(id) = context.get(&context_key) {
            return id.as_model().unwrap();
        }

        // If a source is given it will be available as a key inside the .pak (sources are not
        // given if the asset is specified inline - those are only available in the .pak via ID)
        let key = src.as_ref().map(|src| file_key(&project_dir, &src));
        if let Some(key) = &key {
            // This model will be accessible using this key
            info!("Baking model: {}", key);
        } else {
            // This model will only be accessible using the ID
            info!(
                "Baking model: {} (inline)",
                file_key(&project_dir, self.src())
            );
        }

        // Pak this asset and add it to the context
        let buf = self.bake2();
        let id = pak.push_model(buf, key);
        context.insert(context_key, id.into());
        id
    }

    #[allow(unused)]
    fn bake2(&self) -> ModelBuf {
        let mut mesh_names: HashMap<&str, Option<&str>> = Default::default();
        for mesh in self.meshes() {
            mesh_names
                .entry(mesh.name())
                .or_insert_with(|| mesh.rename());
        }

        let (doc, bufs, _) = import(self.src()).unwrap();
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
            .collect::<Vec<_>>();

        // Uncomment to see what meshes this model contains
        // trace!(
        //     "Found {}",
        //     doc.nodes()
        //         .filter(|node| node.mesh().is_some())
        //         .map(|node| node.mesh().unwrap().name().unwrap_or("UNNAMED"))
        //         .collect::<Vec<_>>()
        //         .join(", ")
        // );

        let mut idx_buf = vec![];
        let mut idx_write = vec![];
        let mut vertex_buf = vec![];
        let meshes = vec![];

        // The whole model will use either 16 or 32 bit indices
        let tiny_idx = nodes.iter().all(|(mesh, _)| {
            let max_idx = mesh
                .primitives()
                .map(|primitive| {
                    // We need to find out how many positions there are for this primitive
                    let data = primitive.reader(|buf| bufs.get(buf.index()).map(|data| &*data.0));
                    data.read_indices()
                        .map(|indices| indices.into_u32().max())
                        .unwrap_or_default()
                })
                .max()
                .flatten()
                .unwrap_or_default();
            max_idx <= u16::MAX as _
        });
        let idx_ty = tiny_idx;

        let _base_idx = 0;
        for (mesh, node) in nodes {
            if meshes.len() == u16::MAX as usize {
                warn!(
                    "Maximum number of meshes supported per model have been loaded, others have been \
                skipped"
                );
                break;
            }

            let _dst_name = mesh_names
                .get(mesh.name().unwrap_or_default())
                .map(|name| name.map(|name| name.to_owned()))
                .unwrap_or(None);

            // trace!(
            //     "Baking mesh: {} (as {})",
            //     mesh.name().unwrap_or("UNNAMED"),
            //     dst_name.as_deref().unwrap_or("UNNAMED")
            // );

            let _skin = node.skin();
            let _transform = Self::get_transform(&node);
            let mut all_positions = vec![];
            let mut _idx_count = 0;
            let mut _vertex_count = 0;
            let _vertex_offset = vertex_buf.len() as u32;

            for primitive in mesh.primitives() {
                match TriangleMode::classify(&primitive) {
                    Some(TriangleMode::List) => (),
                    _ => continue,
                }

                let data = primitive.reader(|buf| bufs.get(buf.index()).map(|data| &*data.0));

                // Read indices (must have sets of three positions)
                let mut indices = data
                    .read_indices()
                    .map(|indices| indices.into_u32().collect::<Vec<_>>())
                    .unwrap_or_default();
                if indices.is_empty() || indices.len() % 3 != 0 {
                    continue;
                }

                // Read positions (must have data)
                let positions = data.read_positions();
                if positions.is_none() {
                    continue;
                }
                let positions = positions.unwrap().collect::<Vec<_>>();

                // Read texture coordinates (must have same length as positions)
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

                all_positions.extend_from_slice(&positions);
                _idx_count += indices.len() as u32;
                _vertex_count += positions.len();

                // For each index, store a true if it has not yet appeared in the list
                let mut seen = HashSet::new();
                for idx in &indices {
                    idx_write.push(!seen.contains(&idx));
                    seen.insert(idx);
                }

                match idx_ty {
                    true => indices
                        .iter()
                        .for_each(|idx| idx_buf.extend_from_slice(&(*idx as u16).to_ne_bytes())),
                    false => indices
                        .iter()
                        .for_each(|idx| idx_buf.extend_from_slice(&idx.to_ne_bytes())),
                }

                for idx in 0..positions.len() {
                    vertex_buf.extend_from_slice(&positions[idx][0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&positions[idx][1].to_ne_bytes());
                    vertex_buf.extend_from_slice(&positions[idx][2].to_ne_bytes());

                    vertex_buf.extend_from_slice(&tex_coords[idx][0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&tex_coords[idx][1].to_ne_bytes());

                    if has_skin {
                        vertex_buf.extend_from_slice(&joints[idx][0].to_ne_bytes());
                        vertex_buf.extend_from_slice(&joints[idx][1].to_ne_bytes());
                        vertex_buf.extend_from_slice(&joints[idx][2].to_ne_bytes());
                        vertex_buf.extend_from_slice(&joints[idx][3].to_ne_bytes());
                        vertex_buf.extend_from_slice(&weights[idx][0].to_ne_bytes());
                        vertex_buf.extend_from_slice(&weights[idx][1].to_ne_bytes());
                        vertex_buf.extend_from_slice(&weights[idx][2].to_ne_bytes());
                        vertex_buf.extend_from_slice(&weights[idx][3].to_ne_bytes());
                    }
                }
            }

            todo!();
            // meshes.push(Mesh::new(
            //     dst_name,
            //     base_idx..idx_count,
            //     vertex_count as _,
            //     vertex_offset,
            //     Sphere::from_point_cloud(all_positions.iter().map(|position| (*position).into())),
            //     transform,
            //     skin.map(|s| {
            //         let joints = s.joints().map(|node| node.name().unwrap().to_owned());
            //         let inv_binds = s
            //             .reader(|buf| bufs.get(buf.index()).map(|data| &*data.0))
            //             .read_inverse_bind_matrices()
            //             .unwrap()
            //             .map(|ibp| Mat4::from_cols_array_2d(&ibp));

            //         joints.zip(inv_binds).into_iter().collect()
            //     }),
            // ));
            //base_idx += idx_count;
        }

        // The write mask is a compression structure. It is used to allow the compute shaders which
        // calculate extra vertex attributes (normal and tangent) to run in a lock-free manner. This
        // *could* be done at runtime but the model loading code would have to iterate through the
        // indices - this extra storage space (basically 1/32 index count in uncompressed bytes)
        // prevents that.
        let mask_len = (idx_write.len() + 31) >> 5;

        // The index-write vector requires padding space because of each stride of the loop below
        idx_write.resize(mask_len << 5, false);

        // Turn the vec of bools into a vec of u32s where each bit is one of the bools
        let mut write_mask = vec![];
        for idx in 0..mask_len {
            let idx = idx << 5;
            let mask = idx_write[idx] as u32
                | (idx_write[idx + 1] as u32) << 1
                | (idx_write[idx + 2] as u32) << 2
                | (idx_write[idx + 3] as u32) << 3
                | (idx_write[idx + 4] as u32) << 4
                | (idx_write[idx + 5] as u32) << 5
                | (idx_write[idx + 6] as u32) << 6
                | (idx_write[idx + 7] as u32) << 7
                | (idx_write[idx + 8] as u32) << 8
                | (idx_write[idx + 9] as u32) << 9
                | (idx_write[idx + 10] as u32) << 10
                | (idx_write[idx + 11] as u32) << 11
                | (idx_write[idx + 12] as u32) << 12
                | (idx_write[idx + 13] as u32) << 13
                | (idx_write[idx + 14] as u32) << 14
                | (idx_write[idx + 15] as u32) << 15
                | (idx_write[idx + 16] as u32) << 16
                | (idx_write[idx + 17] as u32) << 17
                | (idx_write[idx + 18] as u32) << 18
                | (idx_write[idx + 19] as u32) << 19
                | (idx_write[idx + 20] as u32) << 20
                | (idx_write[idx + 21] as u32) << 21
                | (idx_write[idx + 22] as u32) << 22
                | (idx_write[idx + 23] as u32) << 23
                | (idx_write[idx + 24] as u32) << 24
                | (idx_write[idx + 25] as u32) << 25
                | (idx_write[idx + 26] as u32) << 26
                | (idx_write[idx + 27] as u32) << 27
                | (idx_write[idx + 28] as u32) << 28
                | (idx_write[idx + 29] as u32) << 29
                | (idx_write[idx + 30] as u32) << 30
                | (idx_write[idx + 31] as u32) << 31;
            write_mask.extend_from_slice(&mask.to_ne_bytes());
        }

        ModelBuf::new(meshes, idx_ty, idx_buf, vertex_buf, write_mask)
    }

    #[allow(unused)]
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
    #[allow(unused)]
    pub fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.meshes.iter().flatten()
    }

    #[allow(unused)]
    fn node_stride(node: &Node) -> usize {
        if node.skin().is_some() {
            88
        } else {
            64
        }
    }

    /// Translation of the model origin.
    #[allow(unused)]
    pub fn offset(&self) -> Vec3 {
        self.offset
            .map(|offset| vec3(offset[0].0, offset[1].0, offset[2].0))
            .unwrap_or(Vec3::ZERO)
    }

    /// Scaling of the model.
    #[allow(unused)]
    pub fn scale(&self) -> Vec3 {
        self.scale
            .map(|scale| vec3(scale[0].0, scale[1].0, scale[2].0))
            .unwrap_or(Vec3::ONE)
    }

    /// The model file source.
    #[allow(unused)]
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

impl Canonicalize for Model {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}
