use {
    super::{file_key, re_run_if_changed, Canonicalize},
    crate::{
        into_u8_slice,
        pak::{Detail, IndexType, Mesh, Meshlet, ModelBuf, ModelId, Primitive},
    },
    glam::{quat, vec3, Mat4, Vec3},
    gltf::{
        buffer::Data,
        import,
        mesh::{util::ReadIndices, Mode, Reader},
        Buffer, Node,
    },
    log::{info, warn},
    meshopt::{
        build_meshlets, generate_vertex_remap, optimize_overdraw_in_place,
        optimize_vertex_cache_in_place, optimize_vertex_fetch_in_place, quantize_unorm,
        remap_index_buffer, remap_vertex_buffer, simplify, unstripify, VertexDataAdapter,
    },
    ordered_float::OrderedFloat,
    serde::Deserialize,
    std::{
        collections::{HashMap, HashSet},
        io::Error,
        mem::size_of,
        path::{Path, PathBuf},
        u16,
    },
};

#[cfg(feature = "bake")]
use {super::Writer, parking_lot::Mutex, std::sync::Arc};

#[allow(dead_code)]
type Bone = (String, Mat4);
type Index = u32;
type Joint = u32;
type Material = u8;
type Normal = [f32; 3];
type Position = [f32; 3];
type Tangent = [f32; 4];
type TextureCoord = [f32; 2];
type Weight = u32;

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
    lod: Option<bool>,
    lod_min: Option<usize>,
    lod_target_error: Option<OrderedFloat<f32>>,
    #[serde(rename = "mesh")]
    meshes: Option<Vec<MeshRef>>,
    meshlet_max_triangles: Option<usize>,
    meshlet_max_vertices: Option<usize>,
    meshlets: Option<bool>,
    offset: Option<[OrderedFloat<f32>; 3]>,
    optimize: Option<bool>,
    overdraw_threshold: Option<OrderedFloat<f32>>,
    scale: Option<[OrderedFloat<f32>; 3]>,
    shadow: Option<bool>,
    src: PathBuf,
}

impl Model {
    pub fn new(src: impl AsRef<Path>) -> Self {
        Self {
            lod: None,
            lod_target_error: None,
            lod_min: None,
            meshes: None,
            meshlet_max_triangles: None,
            meshlet_max_vertices: None,
            meshlets: None,
            offset: None,
            optimize: None,
            overdraw_threshold: None,
            scale: None,
            shadow: None,
            src: src.as_ref().to_path_buf(),
        }
    }

    fn append_mesh(
        index_buf: &mut Vec<u8>,
        vertex_buf: &mut Vec<u8>,
        indices: &[u32],
        mut vertices: Vec<u8>,
    ) -> IndexType {
        let (index_ty, mut indices) = if vertices.len() <= u16::MAX as usize {
            (
                IndexType::U16,
                into_u8_slice(&indices.iter().map(|idx| *idx as u16).collect::<Vec<_>>()).to_vec(),
            )
        } else {
            (IndexType::U32, into_u8_slice(indices).to_vec())
        };
        index_buf.append(&mut indices);
        vertex_buf.append(&mut vertices);

        index_ty
    }

    fn append_meshlets(index_buf: &mut Vec<u8>, meshlets: &[([[u8; 3]; 126], u32)]) {
        for (indices, _) in meshlets {
            index_buf.extend_from_slice(into_u8_slice(indices))
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

    fn build_meshlets(&self, indices: &[u32], vertex_count: usize) -> Vec<([[u8; 3]; 126], u32)> {
        if !self.meshlets.unwrap_or_default() {
            let triangle_count = indices.len() as u32 / 3;
            let indices = [[0; 3]; 126];

            return vec![(indices, triangle_count)];
        }

        let max_vertices = self.meshlet_max_vertices.unwrap_or(64);
        let max_triangles = self.meshlet_max_triangles.unwrap_or(126);
        let res = build_meshlets(indices, vertex_count, max_vertices, max_triangles);

        assert!(!res.is_empty(), "Invalid meshlets");

        res.iter()
            .map(|meshlet| (meshlet.indices, meshlet.triangle_count as _))
            .collect()
    }

    fn calculate_lods(
        &self,
        indices: &[u32],
        vertices: &[u8],
        vertex_stride: usize,
    ) -> Vec<Vec<u32>> {
        let lod_target_error = self.lod_target_error.unwrap_or(OrderedFloat(0.05)).0;
        let lod_threshold = 1.0 + lod_target_error;
        let lod_min = self.lod_min.unwrap_or(64);
        let mut lods = vec![];
        let mut triangle_count = indices.len() / 3;
        if self.lod.unwrap_or_default() {
            while triangle_count > lod_min {
                let target_count = triangle_count >> 1;
                let lod_indices = simplify(
                    indices,
                    &Self::vertex_data_adapter(vertices, vertex_stride),
                    target_count,
                    lod_target_error,
                );

                let lod_triangle_count = lod_indices.len() / 3;
                if lod_triangle_count >= triangle_count
                    || lod_triangle_count as f32 / target_count as f32 > lod_threshold
                {
                    break;
                }

                lods.push(lod_indices);
                triangle_count = lod_triangle_count;
            }
        }

        lods
    }

    fn convert_triangle_fan_to_list(indices: &mut Vec<Index>) {
        if indices.is_empty() {
            return;
        }

        indices.reserve_exact((indices.len() - 1) >> 1);
        let mut idx = 3;
        while idx < indices.len() {
            indices.insert(idx, 0);
            idx += 3;
        }
    }

    fn convert_triangle_strip_to_list(indices: &mut Vec<Index>, restart_index: u32) {
        *indices = unstripify(indices, restart_index).expect("Unable to unstripify index buffer");
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

    fn optimize_mesh(
        &self,
        vertices: &VertexData,
        shadow: bool,
    ) -> (Vec<Index>, Vec<u8>, usize, usize) {
        if !shadow {
            let positions = vertices.positions.iter().copied().enumerate();
            if let Some((joints, weights)) = &vertices.skin {
                self.optimize_vertices(
                    &vertices.indices,
                    &positions
                        .map(|(idx, position)| {
                            (
                                position,
                                vertices.tex_coords[idx],
                                vertices.normals[idx],
                                vertices.tangents[idx],
                                joints[idx],
                                weights[idx],
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                self.optimize_vertices(
                    &vertices.indices,
                    &positions
                        .map(|(idx, position)| {
                            (
                                position,
                                vertices.tex_coords[idx],
                                vertices.normals[idx],
                                vertices.tangents[idx],
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            }
        } else if let Some((joints, weights)) = &vertices.skin {
            self.optimize_vertices(
                &vertices.indices,
                &vertices
                    .positions
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(idx, position)| (position, joints[idx], weights[idx]))
                    .collect::<Vec<_>>(),
            )
        } else {
            self.optimize_vertices(&vertices.indices, &vertices.positions)
        }
    }

    /// At the very least this function will re-index the vertices, and optionally may
    /// perform full meshopt optimization.
    fn optimize_vertices<T>(
        &self,
        index_buf: &[u32],
        vertex_buf: &[T],
    ) -> (Vec<u32>, Vec<u8>, usize, usize)
    where
        T: Clone + Default,
    {
        let vertex_stride = size_of::<T>();

        // Generate an index buffer from a naively indexed vertex buffer or reindex an existing one
        let (vertex_count, vertex_remap) = generate_vertex_remap(vertex_buf, Some(index_buf));
        let mut index_buf = remap_index_buffer(Some(index_buf), vertex_count, &vertex_remap);
        let mut vertex_buf = remap_vertex_buffer(vertex_buf, vertex_count, &vertex_remap);

        // Run the suggested routines from meshopt: https://github.com/gwihlidal/meshopt-rs#pipeline
        if self.optimize() {
            optimize_vertex_cache_in_place(&index_buf, vertex_count);
            optimize_overdraw_in_place(
                &index_buf,
                &Self::vertex_data_adapter(&vertex_buf, vertex_stride),
                self.overdraw_threshold(),
            );
            optimize_vertex_fetch_in_place(&mut index_buf, &mut vertex_buf);
        }

        // Return the vertices as opaque bytes
        let vertex_buf = into_u8_slice(&vertex_buf).to_vec();

        (index_buf, vertex_buf, vertex_count, vertex_stride)
    }

    /// Determines how much the optimization algorithm can compromise the vertex cache hit ratio.
    ///
    /// A value of 1.05 means that the resulting ratio should be at most 5% worse than before the
    /// optimization.
    pub fn overdraw_threshold(&self) -> f32 {
        self.overdraw_threshold.unwrap_or(OrderedFloat(1.05)).0
    }

    fn read_bones(node: &Node, bufs: &[Data]) -> HashMap<String, Mat4> {
        node.skin()
            .map(|skin| {
                let joints = skin
                    .joints()
                    .map(|node| node.name().unwrap_or_default().to_owned());
                let inv_binds = skin
                    .reader(|buf| bufs.get(buf.index()).map(|data| data.0.as_slice()))
                    .read_inverse_bind_matrices()
                    .map(|ibp| {
                        ibp.map(|ibp| Mat4::from_cols_array_2d(&ibp))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                joints.zip(inv_binds).into_iter().collect()
            })
            .unwrap_or_default()
    }

    fn read_vertices<'a, 's, F>(data: Reader<'a, 's, F>) -> (u32, VertexData)
    where
        F: Clone + Fn(Buffer<'a>) -> Option<&'s [u8]>,
    {
        let positions = data
            .read_positions()
            .map(|positions| positions.collect::<Vec<_>>())
            .unwrap_or_default();

        let (restart_index, indices) = data
            .read_indices()
            .map(|indices| {
                (
                    match indices {
                        ReadIndices::U8(_) => u8::MAX as u32,
                        ReadIndices::U16(_) => u16::MAX as u32,
                        ReadIndices::U32(_) => u32::MAX,
                    },
                    indices.into_u32().collect::<Vec<_>>(),
                )
            })
            .unwrap_or_else(|| (u32::MAX, (0..positions.len() as u32).collect()));

        let mut tex_coords = data
            .read_tex_coords(0)
            .map(|data| data.into_f32())
            .map(|tex_coords| tex_coords.collect::<Vec<_>>())
            .unwrap_or_default();
        tex_coords.resize(positions.len(), Default::default());

        let mut normals = data
            .read_normals()
            .map(|normals| normals.collect::<Vec<_>>())
            .unwrap_or_default();
        normals.resize(positions.len(), Default::default());

        let mut tangents = data
            .read_tangents()
            .map(|tangents| tangents.collect::<Vec<_>>())
            .unwrap_or_default();
        tangents.resize(positions.len(), Default::default());
        let tangents = tangents.into_iter().collect();

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
                            | (quantize_unorm(weights[3], 8) << 24)) as u32
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

        (
            restart_index,
            VertexData {
                indices,
                normals,
                positions,
                skin,
                tangents,
                tex_coords,
            },
        )
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

    fn to_model_buf(&self) -> ModelBuf {
        let build_meshlets = self.meshlets.unwrap_or_default();

        // Gather a map of the importable mesh names and the renamed name they should get
        let mut mesh_names = HashMap::<_, _>::default();
        if let Some(meshes) = &self.meshes {
            for mesh in meshes {
                mesh_names
                    .entry(mesh.name())
                    .and_modify(|_| warn!("Duplicate mesh name: {}", mesh.name()))
                    .or_insert_with(|| mesh.rename());
            }
        }

        // Watch the GLTF file for changes, only if we're in a cargo build
        let src = self.src();
        re_run_if_changed(&src);

        // Just in case there is a GLTF bin file; also watch it for changes
        let mut src_bin = src.to_path_buf();
        src_bin.set_extension("bin");
        re_run_if_changed(src_bin);

        // Load the mesh nodes from this GLTF file
        let (doc, bufs, _) = import(self.src()).unwrap();
        let doc_meshes = doc
            .nodes()
            .filter_map(|node| {
                node.mesh()
                    .filter(|mesh| {
                        // If the model asset contains no mesh array then we bake all meshes
                        // If the model asset does contain a mesh array then we only bake what is specified
                        mesh_names.is_empty()
                            || mesh
                                .name()
                                .map(|name| mesh_names.contains_key(name))
                                .unwrap_or_default()
                    })
                    .map(|mesh| {
                        (
                            mesh.primitives()
                                .filter_map(|primitive| match primitive.mode() {
                                    Mode::TriangleFan | Mode::TriangleStrip | Mode::Triangles => {
                                        // Read material and vertex data
                                        let material =
                                            primitive.material().index().unwrap_or_default();
                                        let (restart_index, mut vertices) =
                                            Self::read_vertices(primitive.reader(|buf| {
                                                bufs.get(buf.index()).map(|data| data.0.as_slice())
                                            }));

                                        // Convert unsupported modes (meshopt requires triangle lists)
                                        match primitive.mode() {
                                            Mode::TriangleFan => {
                                                Self::convert_triangle_fan_to_list(
                                                    &mut vertices.indices,
                                                )
                                            }
                                            Mode::TriangleStrip => {
                                                Self::convert_triangle_strip_to_list(
                                                    &mut vertices.indices,
                                                    restart_index,
                                                )
                                            }
                                            _ => (),
                                        }

                                        Some((material, vertices))
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>(),
                            mesh,
                            node,
                        )
                    })
                    .filter(|(primitives, ..)| !primitives.is_empty())
            })
            .collect::<Vec<_>>();

        // Figure out which unique materials are used on these target mesh primitives and convert
        // those to a map of "Mesh Local" material index from "Gltf File" material index
        // This makes the final materials used index as 0, 1, 2, etc
        let materials = doc_meshes
            .iter()
            .flat_map(|(primitives, ..)| primitives)
            .map(|(material, ..)| *material)
            .collect::<HashSet<_>>()
            .into_iter()
            .enumerate()
            .map(|(idx, material)| (material, idx as Material))
            .collect::<HashMap<_, _>>();

        // Build the list of meshes from this document into index and vertex buffers, and mesh structs
        let shadow = self.shadow.unwrap_or_default();
        let mut meshes = vec![];
        let mut index_buf = vec![];
        let mut vertex_buf = vec![];
        for (mesh_primitives, mesh, node) in doc_meshes {
            let name = mesh_names
                .get(mesh.name().unwrap_or_default())
                .map(|name| name.map(|name| name.to_owned()))
                .unwrap_or(None);
            let bones = Self::read_bones(&node, &bufs);
            let transform = self.transform(&node);

            let mut primitives = vec![];
            for (material, vertices) in mesh_primitives {
                let mut levels = vec![];
                let mut shadows = vec![];
                let material = materials[&material];

                // Optimize and append the main mesh
                let (indices, vertices_optimal, vertex_count, vertex_stride) =
                    self.optimize_mesh(&vertices, false);

                // Store optional shadow mesh (vertices are just positions)
                if shadow {
                    let (indices_shadow, vertices_shadow, vertex_count_shadow, _) =
                        self.optimize_mesh(&vertices, true);

                    // Either store the shadow mesh as-is OR store meshlets of it
                    let meshlets = self.build_meshlets(&indices_shadow, vertex_count_shadow);
                    let index_ty = if build_meshlets {
                        Self::append_meshlets(&mut index_buf, &meshlets);
                        Self::append_mesh(&mut index_buf, &mut vertex_buf, &[], vertices_shadow)
                    } else {
                        Self::append_mesh(
                            &mut index_buf,
                            &mut vertex_buf,
                            &indices_shadow,
                            vertices_shadow,
                        )
                    };

                    shadows.push(Detail {
                        index_ty,
                        meshlets: if build_meshlets {
                            meshlets
                                .iter()
                                .map(|(_, triangle_count)| Meshlet {
                                    triangle_count: *triangle_count,
                                })
                                .collect()
                        } else {
                            vec![Meshlet {
                                triangle_count: indices_shadow.len() as u32 / 3,
                            }]
                        },
                        vertex_count: vertex_count_shadow as _,
                    });
                }

                // Optionally calculate levels of detail: when disabled this returns empty
                let lods = self.calculate_lods(&indices, &vertices_optimal, vertex_stride);

                // Either store the mesh as-is OR store meshlets of the mesh
                let meshlets = self.build_meshlets(&indices, vertex_count);
                let index_ty = if build_meshlets {
                    Self::append_meshlets(&mut index_buf, &meshlets);
                    Self::append_mesh(&mut index_buf, &mut vertex_buf, &[], vertices_optimal)
                } else {
                    Self::append_mesh(&mut index_buf, &mut vertex_buf, &indices, vertices_optimal)
                };

                levels.push(Detail {
                    index_ty,
                    meshlets: if build_meshlets {
                        meshlets
                            .iter()
                            .map(|(_, triangle_count)| Meshlet {
                                triangle_count: *triangle_count,
                            })
                            .collect()
                    } else {
                        vec![Meshlet {
                            triangle_count: indices.len() as u32 / 3,
                        }]
                    },
                    vertex_count: vertices.positions.len() as _,
                });

                // Optimize and append the levels of detail
                for _i_think_this_is_broken_indices in lods {
                    let (indices, vertices_optimal, vertex_count, _) =
                        self.optimize_mesh(&vertices, false);

                    // Store optional shadow mesh (vertices are just positions)
                    if shadow {
                        let (indices_shadow, vertices_shadow, vertex_count_shadow, _) =
                            self.optimize_mesh(&vertices, true);

                        // Either store the shadow mesh as-is OR store meshlets of it
                        let meshlets = self.build_meshlets(&indices_shadow, vertex_count_shadow);
                        let index_ty = if build_meshlets {
                            Self::append_meshlets(&mut index_buf, &meshlets);
                            Self::append_mesh(&mut index_buf, &mut vertex_buf, &[], vertices_shadow)
                        } else {
                            Self::append_mesh(
                                &mut index_buf,
                                &mut vertex_buf,
                                &indices_shadow,
                                vertices_shadow,
                            )
                        };

                        shadows.push(Detail {
                            index_ty,
                            meshlets: if build_meshlets {
                                meshlets
                                    .iter()
                                    .map(|(_, triangle_count)| Meshlet {
                                        triangle_count: *triangle_count,
                                    })
                                    .collect()
                            } else {
                                vec![Meshlet {
                                    triangle_count: indices_shadow.len() as u32 / 3,
                                }]
                            },
                            vertex_count: vertex_count_shadow as _,
                        });
                    }

                    // Either store the mesh as-is OR store meshlets of the mesh
                    let meshlets = self.build_meshlets(&indices, vertex_count);
                    let index_ty = if build_meshlets {
                        Self::append_meshlets(&mut index_buf, &meshlets);
                        Self::append_mesh(&mut index_buf, &mut vertex_buf, &[], vertices_optimal)
                    } else {
                        Self::append_mesh(
                            &mut index_buf,
                            &mut vertex_buf,
                            &indices,
                            vertices_optimal,
                        )
                    };

                    levels.push(Detail {
                        index_ty,
                        meshlets: if build_meshlets {
                            meshlets
                                .iter()
                                .map(|(_, triangle_count)| Meshlet {
                                    triangle_count: *triangle_count,
                                })
                                .collect()
                        } else {
                            vec![Meshlet {
                                triangle_count: indices.len() as u32 / 3,
                            }]
                        },
                        vertex_count: vertices.positions.len() as _,
                    });
                }

                primitives.push(Primitive {
                    material,
                    levels,
                    shadows,
                });
            }
            meshes.push(Mesh {
                bones,
                name,
                primitives,
                transform,
            });
        }

        ModelBuf::new(meshes, index_buf, vertex_buf)
    }

    fn transform(&self, node: &Node) -> Mat4 {
        let (translation, rotation, scale) = node.transform().decomposed();
        let rotation = quat(rotation[0], rotation[1], rotation[2], rotation[3]);
        let scale = vec3(scale[0], scale[1], scale[2]) * self.scale();
        let translation = vec3(translation[0], translation[1], translation[2]) + self.offset();

        Mat4::from_scale_rotation_translation(scale, rotation, translation)
    }

    fn vertex_data_adapter<T>(vertex_buf: &[T], vertex_stride: usize) -> VertexDataAdapter {
        VertexDataAdapter::new(into_u8_slice(vertex_buf), vertex_stride, 0).unwrap()
    }
}

impl Canonicalize for Model {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}

struct VertexData {
    indices: Vec<Index>,
    normals: Vec<Normal>,
    positions: Vec<Position>,
    skin: Option<(Vec<Joint>, Vec<Weight>)>,
    tangents: Vec<Tangent>,
    tex_coords: Vec<TextureCoord>,
}
