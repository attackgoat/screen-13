use {
    super::{
        pak_log::LogId,
        Asset, Model as ModelAsset, PakLog, {get_filename_key, get_path},
    },
    crate::{
        math::{quat, vec3, Mat4, Quat, Sphere, Vec3},
        pak::{Batch, Mesh, Model, ModelId, PakBuf, TriangleMode},
    },
    gltf::{
        import,
        mesh::{Mode, Semantic},
        Node, Primitive,
    },
    std::{collections::HashMap, path::Path, u16, u8},
};

pub fn bake_model<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    asset: &ModelAsset,
    pak: &mut PakBuf,
    log: &mut PakLog,
) -> ModelId {
    let dir = asset_filename.as_ref().parent().unwrap();
    let src = get_path(&dir, asset.src());

    // Early out if we've already baked this asset
    let proto = Asset::Model(ModelAsset::new(&src, asset.offset(), asset.scale()));
    if let Some(LogId::Model(id)) = log.get(&proto) {
        return id;
    }

    let key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    let mut mesh_names: HashMap<&str, Option<&str>> = HashMap::default();
    for mesh in asset.meshes() {
        mesh_names
            .entry(mesh.src_name())
            .or_insert_with(|| mesh.dst_name());
    }

    let (doc, bufs, _) = import(src).unwrap();
    let nodes = doc
        .nodes()
        .filter(|node| node.mesh().is_some())
        .map(|node| (node.mesh().unwrap(), node))
        .filter(|(mesh, _)| mesh.name().is_some())
        .map(|(mesh, node)| (mesh.name().unwrap(), mesh, node))
        .filter(|(name, _, _)| mesh_names.contains_key(name))
        .collect::<Vec<_>>();
    let index_count = nodes
        .iter()
        .map(|(_, mesh, _)| {
            mesh.primitives()
                .filter(|primitive| tri_mode(primitive).is_some())
                .map(|primitive| primitive.indices().unwrap().count())
                .sum::<usize>()
        })
        .sum::<usize>();
    let vertex_count = nodes
        .iter()
        .map(|(_, mesh, _)| {
            mesh.primitives()
                .filter(|primitive| tri_mode(primitive).is_some())
                .map(|primitive| primitive.get(&Semantic::Positions).unwrap().count())
                .sum::<usize>()
        })
        .sum::<usize>();
    let vertex_buf_len = nodes
        .iter()
        .map(|(_, mesh, node)| {
            let stride = node_stride(&node);
            mesh.primitives()
                .filter(|primitive| tri_mode(primitive).is_some())
                .map(|primitive| stride * primitive.get(&Semantic::Positions).unwrap().count())
                .sum::<usize>()
        })
        .sum::<usize>();
    let (index_buf_len, index_mode) = if index_count <= u8::MAX as usize {
        (index_count, IndexMode::U8)
    } else if vertex_count <= u16::MAX as usize {
        (index_count << 1, IndexMode::U16)
    } else {
        (index_count << 2, IndexMode::U32)
    };
    let mut index_buf = Vec::with_capacity(index_buf_len);
    let mut vertex_buf = Vec::with_capacity(vertex_buf_len);
    let mut index_count = 0;

    let mut meshes = vec![];

    for (name, mesh, node) in nodes {
        let dst_name = mesh_names[name];
        let skin = node.skin();
        let (translation, rotation, scale) = node.transform().decomposed();
        let rotation = quat(rotation[0], rotation[1], rotation[2], rotation[3]);
        let scale = vec3(scale[0], scale[1], scale[2]);
        let translation = vec3(translation[0], translation[1], translation[2]);
        let transform = if scale != Vec3::one()
            || rotation != Quat::identity()
            || translation != Vec3::zero()
        {
            Some(Mat4::from_scale_rotation_translation(
                scale,
                rotation,
                translation,
            ))
        } else {
            None
        };
        let mut batches = vec![];
        let mut all_positions = vec![];

        for (mode, primitive) in mesh
            .primitives()
            .map(|primitive| (tri_mode(&primitive), primitive))
            .filter(|(mode, _)| mode.is_some())
            .map(|(mode, primitive)| (mode.unwrap(), primitive))
        {
            let data = primitive.reader(|buf| bufs.get(buf.index()).map(|data| &*data.0));
            let indices = data.read_indices().unwrap().into_u32().collect::<Vec<_>>();
            let positions = data.read_positions().unwrap().collect::<Vec<_>>();
            let normals = data.read_normals().unwrap().collect::<Vec<_>>();
            let tex_coords = data
                .read_tex_coords(0)
                .unwrap()
                .into_f32()
                .collect::<Vec<_>>();

            all_positions.extend_from_slice(&positions);

            let index_end = index_count + indices.len() as u32;
            batches.push(Batch::new(index_count..index_end, mode));
            index_count = index_end;

            match index_mode {
                IndexMode::U8 => indices.iter().for_each(|idx| index_buf.push(*idx as u8)),
                IndexMode::U16 => indices
                    .iter()
                    .for_each(|idx| index_buf.extend_from_slice(&(*idx as u16).to_ne_bytes())),
                IndexMode::U32 => indices
                    .iter()
                    .for_each(|idx| index_buf.extend_from_slice(&idx.to_ne_bytes())),
            }

            if skin.is_some() {
                let joints = data.read_joints(0).unwrap().into_u16().collect::<Vec<_>>();
                let weights = data.read_weights(0).unwrap().into_f32().collect::<Vec<_>>();

                for idx in 0..positions.len() {
                    let position = positions[idx];
                    vertex_buf.extend_from_slice(&position[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&position[1].to_ne_bytes());
                    vertex_buf.extend_from_slice(&position[2].to_ne_bytes());

                    let normal = normals[idx];
                    vertex_buf.extend_from_slice(&normal[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&normal[1].to_ne_bytes());
                    vertex_buf.extend_from_slice(&normal[2].to_ne_bytes());

                    let tex_coord = tex_coords[idx];
                    vertex_buf.extend_from_slice(&tex_coord[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&tex_coord[1].to_ne_bytes());

                    let joint = joints[idx];
                    vertex_buf.extend_from_slice(&joint[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&joint[1].to_ne_bytes());
                    vertex_buf.extend_from_slice(&joint[2].to_ne_bytes());
                    vertex_buf.extend_from_slice(&joint[3].to_ne_bytes());

                    let weight = weights[idx];
                    vertex_buf.extend_from_slice(&weight[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&weight[1].to_ne_bytes());
                    vertex_buf.extend_from_slice(&weight[2].to_ne_bytes());
                    vertex_buf.extend_from_slice(&weight[3].to_ne_bytes());
                }
            } else {
                for idx in 0..positions.len() {
                    let position = positions[idx];
                    vertex_buf.extend_from_slice(&position[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&position[1].to_ne_bytes());
                    vertex_buf.extend_from_slice(&position[2].to_ne_bytes());

                    let normal = normals[idx];
                    vertex_buf.extend_from_slice(&normal[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&normal[1].to_ne_bytes());
                    vertex_buf.extend_from_slice(&normal[2].to_ne_bytes());

                    let tex_coord = tex_coords[idx];
                    vertex_buf.extend_from_slice(&tex_coord[0].to_ne_bytes());
                    vertex_buf.extend_from_slice(&tex_coord[1].to_ne_bytes());
                }
            }
        }

        meshes.push(Mesh::new(
            batches,
            Sphere::from_point_cloud(
                all_positions
                    .iter()
                    .map(|position| vec3(position[0], position[1], position[2])),
            ),
            dst_name.map_or(None, |name| Some(name.to_owned())),
            transform,
            skin.map(|s| {
                let joints = s.joints().map(|node| node.name().unwrap().to_owned());
                let inv_binds = s
                    .reader(|buf| bufs.get(buf.index()).map(|data| &*data.0))
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .map(|ibp| Mat4::from_cols_array_2d(&ibp));

                joints.zip(inv_binds).into_iter().collect()
            }),
        ));
    }

    // Pak and log this asset
    let model = Model::new(meshes, index_buf, vertex_buf);
    let model_id = pak.push_model(key, model);
    log.add(&proto, model_id);

    model_id
}

fn node_stride(node: &Node) -> usize {
    if node.skin().is_some() {
        56
    } else {
        32
    }
}

fn tri_mode(primitive: &Primitive) -> Option<TriangleMode> {
    match primitive.mode() {
        Mode::TriangleFan => Some(TriangleMode::Fan),
        Mode::Triangles => Some(TriangleMode::List),
        Mode::TriangleStrip => Some(TriangleMode::Strip),
        _ => None,
    }
}

enum IndexMode {
    U8,
    U16,
    U32,
}
