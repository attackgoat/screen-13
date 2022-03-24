use {
    super::{file_key, Asset, Canonicalize, Id},
    crate::{
        into_u8_slice,
        pak::{IndexType, Mesh, ModelBuf, ModelId},
    },
    glam::{quat, vec3, Mat4, Quat, Vec3},
    gltf::{import, mesh::Mode, Node, Primitive},
    log::{info, trace, warn},
    meshopt::{
        any_as_u8_slice, generate_vertex_remap, optimize_overdraw_in_place,
        optimize_vertex_cache_in_place, optimize_vertex_fetch_in_place, quantize_unorm,
        remap_index_buffer, remap_vertex_buffer, VertexDataAdapter,
    },
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
    optimize: Option<bool>,
    overdraw_threshold: Option<OrderedFloat<f32>>,
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
            optimize: None,
            overdraw_threshold: None,
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
        let mut mesh_names = HashMap::<_, _>::default();
        for mesh in self.meshes() {
            mesh_names
                .entry(mesh.name())
                .and_modify(|_| warn!("Duplicate mesh name: {}", mesh.name()))
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
            .map(|(mesh, node)| {
                (
                    mesh.primitives()
                        .filter(|primitive| {
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
            .collect::<HashSet<_>>()
            .iter()
            .enumerate()
            .map(|(idx, material_idx)| (*material_idx, idx as u8))
            .collect::<HashMap<_, _>>();

        let mut idx_buf = vec![];
        let mut vertex_buf = vec![];
        let mut meshes = vec![];

        let mut base_idx = 0;
        for (primitives, mesh, node) in nodes {
            let dst_name = mesh_names
                .get(mesh.name().unwrap_or_default())
                .map(|name| name.map(|name| name.to_owned()))
                .unwrap_or(None);
            let skin = node.skin();
            let skin_inv_binds = skin.map(|s| {
                let joints = s.joints().map(|node| node.name().unwrap().to_owned());
                let inv_binds = s
                    .reader(|buf| bufs.get(buf.index()).map(|data| &*data.0))
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .map(|ibp| Mat4::from_cols_array_2d(&ibp));

                joints.zip(inv_binds).into_iter().collect()
            });
            let transform = Self::get_transform(&node);
            let mut index_count = 0;
            let mut vertex_count = 0;

            // Gather primitives as triangle lists, making sure the data is sized properly
            let mut mesh_primitives = HashMap::<_, Vec<_>>::default();
            for primitive in primitives {
                let data = primitive.reader(|buf| bufs.get(buf.index()).map(|data| &*data.0));

                // TODO: Convert point/line/fan/strip geometry into tri lists

                // Read indices
                let mut indices = data
                    .read_indices()
                    .map(|indices| indices.into_u32().collect::<Vec<_>>())
                    .unwrap_or_default();

                // Read positions (must have data)
                let positions = data.read_positions();
                if positions.is_none() {
                    continue;
                }
                let mut positions = positions.unwrap().collect::<Vec<_>>();

                // If we have no indices, make them - they will be optimized below
                if indices.is_empty() {
                    for idx in 0..positions.len() as u32 {
                        indices.push(idx);
                    }
                }

                // Read texture coordinates (must have same length as positions)
                let material_idx = primitive
                    .material()
                    .index()
                    .as_ref()
                    .map(|idx| material_idxs[idx])
                    .unwrap_or_default();
                let mut tex_coords = data
                    .read_tex_coords(0)
                    .map(|data| data.into_f32())
                    .map(|tex_coords| tex_coords.collect::<Vec<_>>())
                    .unwrap_or_default();
                tex_coords.resize(positions.len(), [0.0, 0.0]);

                // Read (optional) skin (must have same length as positions)
                let joints = data
                    .read_joints(0)
                    .map(|joints| {
                        let mut res = joints
                            .into_u16()
                            .map(|joints| {
                                joints[0] as u32
                                    | (joints[1] as u32) << 8
                                    | (joints[2] as u32) << 16
                                    | (joints[3] as u32) << 24
                            })
                            .collect::<Vec<_>>();
                        res.resize(positions.len(), 0);
                        res
                    })
                    .unwrap_or_default();
                let weights = data
                    .read_weights(0)
                    .map(|weights| {
                        let mut res = weights
                            .into_f32()
                            .map(|weights| {
                                (quantize_unorm(weights[0], 8)
                                    | (quantize_unorm(weights[1], 8) << 8)
                                    | (quantize_unorm(weights[2], 8) << 16)
                                    | (quantize_unorm(weights[3], 8) << 24))
                                    as u32
                            })
                            .collect::<Vec<_>>();
                        res.resize(positions.len(), 0);
                        res
                    })
                    .unwrap_or_default();
                let has_skin = joints.len() == positions.len() && weights.len() == positions.len();
                let skin = if has_skin {
                    Some((joints, weights))
                } else {
                    None
                };

                mesh_primitives
                    .entry((material_idx, skin.is_some()))
                    .or_default()
                    .push((indices, positions, tex_coords, skin));
            }

            if mesh_primitives.is_empty() {
                continue;
            }

            // Combine same-material/same-skinness primitives
            let mesh_primitives = mesh_primitives
                .into_iter()
                .map(|((material_idx, has_skin), mesh_primitives)| {
                    let (indices, positions, tex_coords, skin) = mesh_primitives
                        .into_iter()
                        .reduce(|accum, item| {
                            let (
                                mut accum_indices,
                                mut accum_positions,
                                mut accum_tex_coords,
                                mut accum_skin,
                            ) = accum;
                            let (item_indices, item_positions, item_tex_coords, item_skin) = item;

                            // Regular attributes
                            let base_idx = accum_positions.len() as u32;
                            for idx in item_indices {
                                accum_indices.push(idx + base_idx);
                            }

                            for idx in 0..item_positions.len() {
                                accum_positions.push(item_positions[idx]);
                                accum_tex_coords.push(accum_tex_coords[idx]);
                            }

                            // Skin attributes
                            if let Some((accum_joints, accum_weights)) = accum_skin.as_mut() {
                                let (item_joints, item_weights) = item_skin.as_ref().unwrap();
                                for idx in 0..item_positions.len() {
                                    accum_joints.push(item_joints[idx]);
                                    accum_weights.push(item_weights[idx]);
                                }
                            }

                            (accum_indices, accum_positions, accum_tex_coords, accum_skin)
                        })
                        .unwrap();

                    (material_idx, indices, positions, tex_coords, skin)
                })
                .collect::<Vec<_>>();

            // trace!(
            //     "Baking mesh: {} (as {})",
            //     mesh.name().unwrap_or("UNNAMED"),
            //     dst_name.as_deref().unwrap_or("UNNAMED")
            // );

            for (material_idx, mut indices, positions, tex_coords, mesh_skin) in
                mesh_primitives.into_iter()
            {
                let mut vertices = positions.into_iter().enumerate();
                let overdraw_threshold = self.overdraw_threshold();

                let (indices, mut vertices) = if let Some((joints, weights)) = mesh_skin {
                    let mut vertices = vertices
                        .map(|(idx, position)| {
                            (position, tex_coords[idx], joints[idx], weights[idx])
                        })
                        .collect::<Vec<_>>();
                    self.optimize_mesh(&mut indices, &mut vertices, 22);
                    (indices, into_u8_slice(&vertices).to_vec())
                } else {
                    let mut vertices = vertices
                        .map(|(idx, position)| (position, tex_coords[idx]))
                        .collect::<Vec<_>>();
                    self.optimize_mesh(&mut indices, &mut vertices, 20);
                    (indices, into_u8_slice(&vertices).to_vec())
                };

                let (index_ty, mut indices) = if vertices.len() <= u16::MAX as usize {
                    (
                        IndexType::U16,
                        into_u8_slice(
                            &indices
                                .into_iter()
                                .map(|idx| idx as u16)
                                .collect::<Vec<_>>(),
                        )
                        .into_iter()
                        .copied()
                        .collect(),
                    )
                } else {
                    (
                        IndexType::U32,
                        into_u8_slice(&indices).into_iter().copied().collect(),
                    )
                };

                idx_buf.append(&mut indices);
                vertex_buf.append(&mut vertices);

                meshes.push(Mesh {
                    index_count: indices.len() as _,
                    index_ty,
                    name: dst_name.clone(),
                    skin_inv_binds: skin_inv_binds.clone(),
                    transform,
                    vertex_count: vertices.len() as _,
                });
            }
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

    /// When `true` this model will be optmizied using the meshopt library.
    ///
    /// Optimization includes vertex cache, overdraw, and fetch support.
    pub fn optimize(&self) -> bool {
        self.optimize.unwrap_or(true)
    }

    fn optimize_mesh<T>(&self, indices: &mut Vec<u32>, vertices: &mut Vec<T>, vertex_stride: usize)
    where
        T: Clone + Default,
    {
        if !self.optimize() {
            return;
        }

        let (vertex_count, vertex_remap) = generate_vertex_remap(vertices, Some(indices));

        // HACK: vertex_count is unused / confusing API so I have made it usize::MAX to clearly show this
        *indices = remap_index_buffer(Some(indices), usize::MAX, &vertex_remap);
        *vertices = remap_vertex_buffer(vertices, vertex_count, &vertex_remap);

        optimize_vertex_cache_in_place(indices, vertex_count);
        optimize_overdraw_in_place(
            indices,
            &VertexDataAdapter::new(into_u8_slice(vertices), vertex_stride, 0).unwrap(),
            self.overdraw_threshold(),
        );
        optimize_vertex_fetch_in_place(indices, vertices);
    }

    /// Determines how much the optimization algorithm can compromise the vertex cache hit ratio.
    ///
    /// A value of 1.05 means that the resulting ratio should be at most 5% worse than before the
    /// optimization.
    pub fn overdraw_threshold(&self) -> f32 {
        self.overdraw_threshold.unwrap_or(OrderedFloat(1.05)).0
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
