use {
    super::{
        bake_bitmap,
        pak_log::LogId,
        Asset, ModelAsset, PakLog, {get_filename_key, get_path},
    },
    crate::{
        math::{vec2, vec3, Sphere, Vec2, Vec3},
        pak::{Model, ModelId, PakBuf},
    },
    gltf::Gltf,
    std::{fs::File, path::Path},
};

pub fn bake_model<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    model_asset: &ModelAsset,
    mut pak: &mut PakBuf,
    mut log: &mut PakLog,
) -> ModelId {
    let dir = asset_filename.as_ref().parent().unwrap();
    let src = get_path(&dir, model_asset.src());

    // Early out if we've already baked this asset
    // Also - the loggable asset won't have any texture so we can
    // reuse our vertices
    let proto = Asset::Model(ModelAsset {
        bitmaps: Default::default(),
        offset: model_asset.offset,
        scale: model_asset.scale,
        src: src.clone(),
    });
    if let Some(LogId::Model(id)) = log.get(&proto) {
        return id;
    }

    let key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    // Get the fs objects for this asset
    // SRC now ! let src_filename = get_path(&dir, model_asset.src());
    let bitmaps = model_asset
        .bitmaps()
        .iter()
        .map(|bitmap| {
            let bitmap_filename = get_path(&dir, &bitmap);

            // Bake the bitmap
            match Asset::read(&bitmap_filename) {
                Asset::Bitmap(bitmap) => {
                    bake_bitmap(&project_dir, &bitmap_filename, &bitmap, &mut pak, &mut log)
                }
                _ => panic!(),
            }
        })
        .collect();

    let data = Gltf::open(src).unwrap();

    data.meshes()

    // Bake the vertices
    //let vertices = parse_vertices(&src_filename, model_asset.scale);
    //let len = SingleTexture::BYTES * vertices.len();
    let len = 0;
    let mut vertex_buf = Vec::with_capacity(len);
    /*unsafe {
        vertex_buf.set_len(len);
    }
    let mut center = Vec3::zero();
    let mut radius = 0f32;
    for (idx, vertex) in vertices.iter().enumerate() {
        let start = idx * SingleTexture::BYTES;
        let end = start + SingleTexture::BYTES;
        vertex.write(&mut vertex_buf[start..end]);
        center += vertex.pos;
    }

    // Find the bounding sphere parts:
    // - Center is the average of all vertex positions
    // - Rradius is the max vertex distance from center
    center /= vertices.len() as f32;
    for vertex in &vertices {
        radius = radius.max((vertex.pos - center).length());
    }*/

    let center = Vec3::zero();
    let radius = 0.0;

    // Pak and log this asset
    let bounds = Sphere::new(center, radius);
    let model = Model::new(bitmaps, bounds, vertex_buf);
    let model_id = pak.push_model(key, model);
    log.add(&proto, model_id);

    model_id
}
/*
fn compute_tri_normal(tri: [Vec3; 3]) -> Vec3 {
    let v0 = vec3(tri[0].x(), tri[0].y(), tri[0].z());
    let v1 = vec3(tri[1].x(), tri[1].y(), tri[1].z());
    let v2 = vec3(tri[2].x(), tri[2].y(), tri[2].z());
    let u = v0 - v1;
    let v = v0 - v2;

    u.cross(v).normalize()
}

// TODO: Make this a bit more generic so we can accept dual-tex and animated vertices too
fn parse_vertices<P: AsRef<Path>>(path: P, scale: Vec3) -> Vec<SingleTexture> {
    let fbx = Fbx::read(path);

    /*debug!("Found {} vertices", fbx_vertices.len());
    debug!("Found {} poly_indices", fbx_poly_indices.len());
    debug!("Found {} uv_coords", fbx_uv_coords.len());
    debug!("Found {} uv_indices", fbx_uv_indices.len());*/

    let tri_count = fbx.tri_count();
    let mut vertices = Vec::with_capacity(tri_count * 3);

    let make_vertex = |tri_idx, uv_idx| {
        // Position
        let px = fbx.vertices[3 * tri_idx] * scale.x();
        let py = fbx.vertices[3 * tri_idx + 1] * scale.y();
        let pz = fbx.vertices[3 * tri_idx + 2] * scale.z();

        // Texture coords
        let u = fbx.uv_coords[2 * uv_idx];
        let v = fbx.uv_coords[2 * uv_idx + 1];

        SingleTexture {
            pos: vec3(px, py, pz),
            normal: vec3(0.0, 0.0, 0.0),
            tex_coord: vec2(u, v),
        }
    };

    for index in 0..tri_count {
        let a = fbx.poly_indices[3 * index] as usize;
        let b = fbx.poly_indices[3 * index + 1] as usize;
        let c = (-fbx.poly_indices[3 * index + 2] - 1) as usize;

        assert!(3 * a < fbx.vertices.len() - 2);
        assert!(3 * b < fbx.vertices.len() - 2);
        assert!(3 * c < fbx.vertices.len() - 2);

        let vert_x = fbx.uv_indices[3 * index] as usize;
        let vert_y = fbx.uv_indices[3 * index + 1] as usize;
        let vert_z = fbx.uv_indices[3 * index + 2] as usize;

        let mut a = make_vertex(a, vert_x);
        let mut b = make_vertex(b, vert_y);
        let mut c = make_vertex(c, vert_z);

        // Calculate normal
        let normal = compute_tri_normal([a.pos, b.pos, c.pos]);
        a.normal = normal;
        b.normal = normal;
        c.normal = normal;

        vertices.push(a);
        vertices.push(b);
        vertices.push(c);
    }

    assert_ne!(vertices.len(), 0);

    vertices
}

#[derive(Default)]
struct Fbx {
    vertices: Vec<f32>,
    poly_indices: Vec<isize>,
    uv_coords: Vec<f32>,
    uv_indices: Vec<isize>,
}

impl Fbx {
    fn read<P: AsRef<Path>>(path: P) -> Self {
        let mut file = File::open(path.as_ref()).unwrap();
        let parser = EventReader::new(&mut file);

        let mut res = Self::default();

        for e in parser {
            match e {
                Ok(FbxEvent::StartNode {
                    ref name,
                    ref properties,
                }) => {
                    if "Vertices" == name {
                        if let OwnedProperty::VecF64(ref vertices) = properties[0] {
                            for vertex in vertices {
                                res.vertices.push(*vertex as _);
                            }
                        }
                    } else if "PolygonVertexIndex" == name {
                        if let OwnedProperty::VecI32(ref poly_indices) = properties[0] {
                            for poly_index in poly_indices {
                                res.poly_indices.push(*poly_index as isize);
                            }
                        }
                    } else if "UV" == name {
                        if let OwnedProperty::VecF64(ref uv_coords) = properties[0] {
                            for uv_coord in uv_coords {
                                res.uv_coords.push(*uv_coord as f32);
                            }
                        }
                    } else if "UVIndex" == name {
                        if let OwnedProperty::VecI32(ref uv_indices) = properties[0] {
                            for uv_index in uv_indices {
                                res.uv_indices.push(*uv_index as isize);
                            }
                        }
                    }
                }
                Err(e) => {
                    panic!("Error parsing fbx: {}", e);
                }
                _ => {
                    continue;
                }
            }
        }

        // Model must be triangulated for us with 1 uv set
        assert!(0 == res.poly_indices.len() % 3);
        assert!(res.poly_indices.len() == res.uv_indices.len());

        res
    }

    fn tri_count(&self) -> usize {
        self.poly_indices.len() / 3
    }
}

struct SingleTexture {
    pos: Vec3,
    normal: Vec3,
    tex_coord: Vec2,
}

impl Vertex for SingleTexture {
    const BYTES: usize = 32;

    fn write(&self, buf: &mut [u8]) {
        buf[0..4].copy_from_slice(&self.pos.x().to_ne_bytes());
        buf[4..8].copy_from_slice(&self.pos.y().to_ne_bytes());
        buf[8..12].copy_from_slice(&self.pos.z().to_ne_bytes());
        buf[12..16].copy_from_slice(&self.normal.x().to_ne_bytes());
        buf[16..20].copy_from_slice(&self.normal.y().to_ne_bytes());
        buf[20..24].copy_from_slice(&self.normal.z().to_ne_bytes());
        buf[24..28].copy_from_slice(&self.tex_coord.x().to_ne_bytes());
        buf[28..32].copy_from_slice(&self.tex_coord.y().to_ne_bytes());
    }
}

trait Vertex {
    const BYTES: usize; // TODO: name should be Size-ish not bytes-ish

    fn write(&self, buf: &mut [u8]);
}
*/
