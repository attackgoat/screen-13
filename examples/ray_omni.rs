mod profile_with_puffin;

use {
    bytemuck::{bytes_of, cast_slice, Pod, Zeroable},
    clap::Parser,
    glam::{vec3, vec4, Mat4, Vec3, Vec4},
    inline_spirv::inline_spirv,
    log::info,
    meshopt::remap::{generate_vertex_remap, remap_index_buffer, remap_vertex_buffer},
    screen_13::prelude::*,
    screen_13_window::WindowBuilder,
    std::{
        env::current_exe,
        fs::{metadata, write},
        mem::size_of,
        path::{Path, PathBuf},
        sync::Arc,
    },
    tobj::{load_obj, GPU_LOAD_OPTIONS},
};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    profile_with_puffin::init();

    let args = Args::parse();
    let window = WindowBuilder::default().debug(args.debug).build()?;
    let mut pool = LazyPool::new(&window.device);

    let depth_fmt = best_2d_optimal_format(
        &window.device,
        &[vk::Format::D32_SFLOAT, vk::Format::D16_UNORM],
        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        vk::ImageCreateFlags::empty(),
    );

    let ground_mesh = load_ground_mesh(&window.device)?;
    let model_path = download_model_from_github("happy.obj")?;
    let model_mesh = load_model_mesh(&window.device, model_path)?;
    let scene_blas = create_blas(&window.device, &[&ground_mesh, &model_mesh])?;
    let gfx_pipeline = create_pipeline(&window.device)?;

    let mut angle = 0f32;

    window.run(|frame| {
        angle += 0.016;

        let scene_tlas =
            create_tlas(frame.device, &mut pool, frame.render_graph, &scene_blas).unwrap();

        let ground_mesh_index_buf = frame.render_graph.bind_node(&ground_mesh.index_buf);
        let ground_mesh_vertex_buf = frame.render_graph.bind_node(&ground_mesh.vertex_buf);
        let model_mesh_index_buf = frame.render_graph.bind_node(&model_mesh.index_buf);
        let model_mesh_vertex_buf = frame.render_graph.bind_node(&model_mesh.vertex_buf);

        let depth_image = frame.render_graph.bind_node(
            pool.lease(ImageInfo::image_2d(
                frame.width,
                frame.height,
                depth_fmt,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            ))
            .unwrap(),
        );
        let camera_buf = frame.render_graph.bind_node({
            let mut buf = pool
                .lease(BufferInfo::host_mem(
                    size_of::<Camera>() as _,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                ))
                .unwrap();
            Buffer::copy_from_slice(
                &mut buf,
                0,
                bytes_of(&Camera {
                    projection: Mat4::perspective_rh(
                        45f32.to_radians(),
                        frame.render_aspect_ratio(),
                        0.1,
                        100.0,
                    ),
                    view: Mat4::look_at_rh(vec3(0.0, 1.2, 1.0), vec3(0.0, 0.6, 0.0), -Vec3::Y),
                    model: Mat4::IDENTITY,
                    light_position: vec4(angle.cos() * 3.0, 2.0, angle.sin() * 3.0, 0.0),
                }),
            );

            buf
        });

        frame
            .render_graph
            .begin_pass("Mesh with ray-query shadows")
            .bind_pipeline(&gfx_pipeline)
            .access_node(ground_mesh_index_buf, AccessType::IndexBuffer)
            .access_node(ground_mesh_vertex_buf, AccessType::VertexBuffer)
            .access_node(model_mesh_index_buf, AccessType::IndexBuffer)
            .access_node(model_mesh_vertex_buf, AccessType::VertexBuffer)
            .access_descriptor(0, camera_buf, AccessType::AnyShaderReadUniformBuffer)
            .access_descriptor(
                1,
                scene_tlas,
                AccessType::RayTracingShaderReadAccelerationStructure,
            )
            .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
            .clear_depth_stencil(depth_image)
            .clear_color_value(0, frame.swapchain_image, [0xff, 0xff, 0xff, 0xff])
            .store_color(0, frame.swapchain_image)
            .record_subpass(move |subpass, _| {
                subpass
                    .bind_index_buffer(model_mesh_index_buf, vk::IndexType::UINT32)
                    .bind_vertex_buffer(model_mesh_vertex_buf)
                    .draw_indexed(model_mesh.index_count, 1, 0, 0, 0);

                subpass
                    .bind_index_buffer(ground_mesh_index_buf, vk::IndexType::UINT32)
                    .bind_vertex_buffer(ground_mesh_vertex_buf)
                    .draw_indexed(ground_mesh.index_count, 1, 0, 0, 0);
            });
    })?;

    Ok(())
}

fn best_2d_optimal_format(
    device: &Device,
    formats: &[vk::Format],
    usage: vk::ImageUsageFlags,
    flags: vk::ImageCreateFlags,
) -> vk::Format {
    for format in formats {
        let format_props = Device::image_format_properties(
            device,
            *format,
            vk::ImageType::TYPE_2D,
            vk::ImageTiling::OPTIMAL,
            usage,
            flags,
        );

        if format_props.is_ok() {
            return *format;
        }
    }

    panic!("Unsupported format");
}

fn create_blas(
    device: &Arc<Device>,
    models: &[&Model],
) -> Result<Arc<AccelerationStructure>, DriverError> {
    let info = AccelerationStructureGeometryInfo::blas(
        models
            .iter()
            .map(|model| {
                (
                    AccelerationStructureGeometry {
                        max_primitive_count: model.index_count / 3,
                        flags: vk::GeometryFlagsKHR::OPAQUE,
                        geometry: AccelerationStructureGeometryData::triangles(
                            Buffer::device_address(&model.index_buf),
                            vk::IndexType::UINT32,
                            model.vertex_count,
                            None,
                            Buffer::device_address(&model.vertex_buf),
                            vk::Format::R32G32B32_SFLOAT,
                            24,
                        ),
                    },
                    vk::AccelerationStructureBuildRangeInfoKHR::default()
                        .primitive_count(model.index_count / 3),
                )
            })
            .collect::<Box<_>>(),
    )
    .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE);
    let size = AccelerationStructure::size_of(device, &info);

    let mut render_graph = RenderGraph::new();
    let blas = render_graph.bind_node(AccelerationStructure::create(
        device,
        AccelerationStructureInfo::blas(size.create_size),
    )?);

    let accel_struct_scratch_offset_alignment = device
        .physical_device
        .accel_struct_properties
        .as_ref()
        .unwrap()
        .min_accel_struct_scratch_offset_alignment
        as vk::DeviceSize;
    let scratch_buf = render_graph.bind_node(Buffer::create(
        device,
        BufferInfo::device_mem(
            size.build_size,
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
        )
        .to_builder()
        .alignment(accel_struct_scratch_offset_alignment),
    )?);
    let scratch_data = render_graph.node_device_address(scratch_buf);

    let mut pass = render_graph.begin_pass("Build BLAS");

    for model in models.iter().copied() {
        let index_buf = pass.bind_node(&model.index_buf);
        let vertex_buf = pass.bind_node(&model.vertex_buf);

        pass.access_node_mut(index_buf, AccessType::AccelerationStructureBuildRead);
        pass.access_node_mut(vertex_buf, AccessType::AccelerationStructureBuildRead);
    }

    pass.access_node(blas, AccessType::AccelerationStructureBuildWrite)
        .access_node(scratch_buf, AccessType::AccelerationStructureBufferWrite)
        .record_acceleration(move |accel, _| {
            accel.build_structure(&info, blas, scratch_data);
        });

    let blas = render_graph.unbind_node(blas);

    render_graph
        .resolve()
        .submit(&mut LazyPool::new(device), 0, 0)?;

    Ok(blas)
}

fn create_pipeline(device: &Arc<Device>) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vert = inline_spirv!(
        r#"
        #version 460 core

        layout (location = 0) in vec3 inPos;
        layout (location = 1) in vec3 inNormal;
        
        layout (binding = 0) uniform UBO 
        {
            mat4 projection;
            mat4 view;
            mat4 model;
            vec3 lightPos;
        } ubo;
        
        layout (location = 0) out vec3 outNormal;
        layout (location = 1) out vec3 outViewVec;
        layout (location = 2) out vec3 outLightVec;
        layout (location = 3) out vec3 outWorldPos;
        
        void main() 
        {
            outNormal = inNormal;
            gl_Position = ubo.projection * ubo.view * ubo.model * vec4(inPos.xyz, 1.0);
            vec4 pos = ubo.model * vec4(inPos, 1.0);
            outWorldPos = vec3(ubo.model * vec4(inPos, 1.0));
            outNormal = mat3(ubo.model) * inNormal;
            outLightVec = normalize(ubo.lightPos - inPos);
            outViewVec = -pos.xyz;
        }
        "#,
        vert,
        vulkan1_2
    );
    let frag = inline_spirv!(
        r#"
        #version 460 core
        #extension GL_EXT_ray_tracing : enable
        #extension GL_EXT_ray_query : enable

        layout (binding = 1) uniform accelerationStructureEXT topLevelAS;

        layout (location = 0) in vec3 inNormal;
        layout (location = 1) in vec3 inViewVec;
        layout (location = 2) in vec3 inLightVec;
        layout (location = 3) in vec3 inWorldPos;

        layout (location = 0) out vec4 outFragColor;

        #define ambient 0.1

        void main() 
        {	
            vec3 N = normalize(inNormal);
            vec3 L = normalize(inLightVec);
            vec3 V = normalize(inViewVec);
            vec3 R = normalize(-reflect(L, N));
            vec3 diffuse = vec3(max(dot(N, L), ambient));

            outFragColor = vec4(diffuse, 1.0);

            rayQueryEXT rayQuery;
            rayQueryInitializeEXT(rayQuery, topLevelAS, gl_RayFlagsTerminateOnFirstHitEXT, 0xFF, inWorldPos, 0.01, L, 1000.0);

            // Traverse the acceleration structure and store information about the first intersection (if any)
            rayQueryProceedEXT(rayQuery);

            // If the intersection has hit a triangle, the fragment is shadowed
            if (rayQueryGetIntersectionTypeEXT(rayQuery, true) == gl_RayQueryCommittedIntersectionTriangleEXT ) {
                outFragColor *= 0.1;
            }
        }
        "#,
        frag,
        vulkan1_2
    );

    Ok(Arc::new(GraphicPipeline::create(
        device,
        GraphicPipelineInfo::default(),
        [
            Shader::new_vertex(vert.as_slice()),
            Shader::new_fragment(frag.as_slice()),
        ],
    )?))
}

fn create_tlas(
    device: &Arc<Device>,
    pool: &mut LazyPool,
    render_graph: &mut RenderGraph,
    blas: &Arc<AccelerationStructure>,
) -> Result<AccelerationStructureLeaseNode, DriverError> {
    let instances = [vk::AccelerationStructureInstanceKHR {
        transform: vk::TransformMatrixKHR {
            matrix: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
            ],
        },
        instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xFF),
        instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(
            0,
            vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as _,
        ),
        acceleration_structure_reference: vk::AccelerationStructureReferenceKHR {
            device_handle: AccelerationStructure::device_address(blas),
        },
    }];
    let instance_data = AccelerationStructure::instance_slice(&instances);
    let instance_buf = Arc::new({
        let mut buffer = Buffer::create(
            device,
            BufferInfo::host_mem(
                instance_data.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buffer, 0, instance_data);

        buffer
    });

    let info = AccelerationStructureGeometryInfo::tlas([(
        AccelerationStructureGeometry::opaque(
            2,
            AccelerationStructureGeometryData::instances(Buffer::device_address(&instance_buf)),
        ),
        vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(1),
    )])
    .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE);
    let size = AccelerationStructure::size_of(device, &info);
    let tlas =
        render_graph.bind_node(pool.lease(AccelerationStructureInfo::tlas(size.create_size))?);

    let accel_struct_scratch_offset_alignment = device
        .physical_device
        .accel_struct_properties
        .as_ref()
        .unwrap()
        .min_accel_struct_scratch_offset_alignment
        as vk::DeviceSize;
    let scratch_buf = render_graph.bind_node(
        pool.lease(
            BufferInfo::device_mem(
                size.build_size,
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
            )
            .to_builder()
            .alignment(accel_struct_scratch_offset_alignment),
        )?,
    );
    let scratch_data = render_graph.node_device_address(scratch_buf);
    let blas = render_graph.bind_node(blas);
    let instance_buf = render_graph.bind_node(instance_buf);

    render_graph
        .begin_pass("Build TLAS")
        .access_node(blas, AccessType::AccelerationStructureBuildRead)
        .access_node(instance_buf, AccessType::AccelerationStructureBuildRead)
        .access_node(scratch_buf, AccessType::AccelerationStructureBufferWrite)
        .access_node(tlas, AccessType::AccelerationStructureBuildWrite)
        .record_acceleration(move |accel, _| {
            accel.build_structure(&info, tlas, scratch_data);
        });

    Ok(tlas)
}

fn download_model_from_github(model_name: &str) -> anyhow::Result<PathBuf> {
    const REPO_URL: &str =
        "https://raw.githubusercontent.com/alecjacobson/common-3d-test-models/master/data/";

    let model_path = current_exe()?.parent().unwrap().join(model_name);
    let model_metadata = metadata(&model_path);

    if model_metadata.is_err() {
        info!("Downloading model from github");

        let data = reqwest::blocking::get(REPO_URL.to_owned() + model_name)?.bytes()?;
        write(&model_path, data)?;

        info!("Download complete");
    }

    Ok(model_path)
}

fn load_ground_mesh(device: &Arc<Device>) -> Result<Model, DriverError> {
    let extent = 100f32;
    let v0 = [-extent, 0.0, -extent];
    let v1 = [extent, 0.0, -extent];
    let v2 = [-extent, 0.0, extent];
    let v3 = [extent, 0.0, extent];
    let up = [0f32, 1.0, 0.0];

    let index_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::INDEX_BUFFER
            | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        cast_slice(&[0u32, 1, 2, 1, 3, 2]),
    )?);
    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER
            | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        cast_slice(&[v0, up, v1, up, v2, up, v3, up]),
    )?);

    Ok(Model {
        index_buf,
        index_count: 6,
        vertex_buf,
        vertex_count: 4,
    })
}

fn load_model<T>(
    device: &Arc<Device>,
    path: impl AsRef<Path>,
    face_fn: fn(a: Vec3, b: Vec3, c: Vec3) -> [T; 3],
) -> anyhow::Result<Model>
where
    T: Default + Pod,
{
    let (models, _) = load_obj(path.as_ref(), &GPU_LOAD_OPTIONS)?;
    let mut vertices =
        Vec::with_capacity(models.iter().map(|model| model.mesh.indices.len()).sum());

    // Calculate AABB
    let mut min = Vec3::ZERO;
    let mut max = Vec3::ZERO;
    for model in &models {
        for n in 0..model.mesh.positions.len() / 3 {
            let idx = 3 * n;
            let position = Vec3::from_slice(&model.mesh.positions[idx..idx + 3]);

            min = min.min(position);
            max = max.max(position);
        }
    }

    // Calculate a uniform scale which fits the model to a unit cube
    let scale = Vec3::splat(1.0 / (max - min).max_element());

    // Load the triangles using the face_fn closure to form vertices
    for model in models {
        for n in 0..model.mesh.indices.len() / 3 {
            let idx = 3 * n;
            let a_idx = 3 * model.mesh.indices[idx] as usize;
            let b_idx = 3 * model.mesh.indices[idx + 1] as usize;
            let c_idx = 3 * model.mesh.indices[idx + 2] as usize;
            let a = Vec3::from_slice(&model.mesh.positions[a_idx..a_idx + 3]) * scale;
            let b = Vec3::from_slice(&model.mesh.positions[b_idx..b_idx + 3]) * scale;
            let c = Vec3::from_slice(&model.mesh.positions[c_idx..c_idx + 3]) * scale;
            let face = face_fn(a, b, c);

            vertices.push(face[0]);
            vertices.push(face[1]);
            vertices.push(face[2]);
        }
    }

    // Re-index and de-dupe the model vertices using meshopt
    let indices = (0u32..vertices.len() as u32).collect::<Vec<_>>();
    let (vertex_count, remap) = generate_vertex_remap(&vertices, Some(&indices));
    let indices = remap_index_buffer(Some(&indices), vertex_count, &remap);
    let vertices = remap_vertex_buffer(&vertices, vertex_count, &remap);

    let index_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::INDEX_BUFFER
            | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        cast_slice(&indices),
    )?);
    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER
            | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        cast_slice(&vertices),
    )?);

    Ok(Model {
        index_buf,
        index_count: indices.len() as _,
        vertex_buf,
        vertex_count: vertices.len() as _,
    })
}

/// Loads an .obj model as indexed position and normal vertices
fn load_model_mesh(device: &Arc<Device>, path: impl AsRef<Path>) -> anyhow::Result<Model> {
    #[repr(C)]
    #[derive(Clone, Copy, Default, Pod, Zeroable)]
    struct Vertex {
        position: Vec3,
        normal: Vec3,
    }

    load_model(device, path, |a, b, c| {
        let u = b - a;
        let v = c - a;
        let normal = vec3(
            u.y * v.z - u.z * v.y,
            u.z * v.x - u.x * v.z,
            u.x * v.y - u.y * v.x,
        )
        .normalize();

        // Make faces CCW
        [
            Vertex {
                position: a,
                normal,
            },
            Vertex {
                position: c,
                normal,
            },
            Vertex {
                position: b,
                normal,
            },
        ]
    })
}

#[derive(Parser)]
struct Args {
    /// Enable Vulkan SDK validation layers
    #[arg(long)]
    debug: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Camera {
    projection: Mat4,
    view: Mat4,
    model: Mat4,
    light_position: Vec4,
}

struct Model {
    index_buf: Arc<Buffer>,
    index_count: u32,
    vertex_buf: Arc<Buffer>,
    vertex_count: u32,
}
