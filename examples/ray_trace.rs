use {
    bytemuck::cast_slice,
    inline_spirv::inline_spirv,
    screen_13::prelude_arc::*,
    std::io::BufReader,
    std::mem::size_of,
    tobj::{load_mtl_buf, load_obj_buf, LoadOptions},
};

static SHADER_RAY_GEN: &[u32] = inline_spirv!(
    r#"
    #version 460
    #extension GL_EXT_ray_tracing : require

    #define M_PI 3.1415926535897932384626433832795

    layout(location = 0) rayPayloadEXT Payload {
        vec3 rayOrigin;
        vec3 rayDirection;
        vec3 previousNormal;

        vec3 directColor;
        vec3 indirectColor;
        int rayDepth;

        int rayActive;
    } payload;

    layout(binding = 0, set = 0) uniform accelerationStructureEXT topLevelAS;
    layout(binding = 1, set = 0) uniform Camera {
        vec4 position;
        vec4 right;
        vec4 up;
        vec4 forward;

        uint frameCount;
    } camera;

    layout(binding = 4, set = 0, rgba32f) uniform image2D image;

    float random(vec2 uv, float seed) {
        return fract(sin(mod(dot(uv, vec2(12.9898, 78.233)) + 1113.1 * seed, M_PI)) *
            43758.5453);
    }

    void main() {
        vec2 uv = gl_LaunchIDEXT.xy
                + vec2(random(gl_LaunchIDEXT.xy, 0), random(gl_LaunchIDEXT.xy, 1));
        uv /= vec2(gl_LaunchSizeEXT.xy);
        uv = (uv * 2.0f - 1.0f) * vec2(1.0f, -1.0f);

        payload.rayOrigin = camera.position.xyz;
        payload.rayDirection =
            normalize(uv.x * camera.right + uv.y * camera.up + camera.forward).xyz;
        payload.previousNormal = vec3(0.0, 0.0, 0.0);

        payload.directColor = vec3(0.0, 0.0, 0.0);
        payload.indirectColor = vec3(0.0, 0.0, 0.0);
        payload.rayDepth = 0;

        payload.rayActive = 1;

        for (int x = 0; x < 16; x++) {
            traceRayEXT(topLevelAS, gl_RayFlagsOpaqueEXT, 0xFF, 0, 0, 0,
                payload.rayOrigin, 0.001, payload.rayDirection, 10000.0, 0);
        }

        vec4 color = vec4(payload.directColor + payload.indirectColor, 1.0);

        if (camera.frameCount > 0) {
            vec4 previousColor = imageLoad(image, ivec2(gl_LaunchIDEXT.xy));
            previousColor *= camera.frameCount;

            color += previousColor;
            color /= (camera.frameCount + 1);
        }

        imageStore(image, ivec2(gl_LaunchIDEXT.xy), color);
    }
    "#,
    rgen,
    vulkan1_2
)
.as_slice();

static SHADER_CLOSEST_HIT: &[u32] = inline_spirv!(
    r#"
    #version 460
    #extension GL_EXT_ray_tracing : require
    #extension GL_EXT_nonuniform_qualifier : enable

    #define M_PI 3.1415926535897932384626433832795

    struct Material {
        vec3 ambient;
        vec3 diffuse;
        vec3 specular;
        vec3 emission;
    };

    hitAttributeEXT vec2 hitCoordinate;

    layout(location = 0) rayPayloadInEXT Payload {
        vec3 rayOrigin;
        vec3 rayDirection;
        vec3 previousNormal;

        vec3 directColor;
        vec3 indirectColor;
        int rayDepth;

        int rayActive;
    } payload;

    layout(location = 1) rayPayloadEXT bool isShadow;

    layout(binding = 0, set = 0) uniform accelerationStructureEXT topLevelAS;
    layout(binding = 1, set = 0) uniform Camera {
        vec4 position;
        vec4 right;
        vec4 up;
        vec4 forward;

        uint frameCount;
    } camera;

    layout(binding = 2, set = 0) buffer IndexBuffer {
        uint data[];
    } indexBuffer;
    layout(binding = 3, set = 0) buffer VertexBuffer {
        float data[];
    } vertexBuffer;

    layout(binding = 0, set = 1) buffer MaterialIndexBuffer {
        uint data[];
    } materialIndexBuffer;
    layout(binding = 1, set = 1) buffer MaterialBuffer {
        Material data[];
    } materialBuffer;

    float random(vec2 uv, float seed) {
        return fract(sin(mod(dot(uv, vec2(12.9898, 78.233)) + 1113.1 * seed, M_PI)) *
            43758.5453);
    }

    vec3 uniformSampleHemisphere(vec2 uv) {
        float z = uv.x;
        float r = sqrt(max(0, 1.0 - z * z));
        float phi = 2.0 * M_PI * uv.y;

        return vec3(r * cos(phi), z, r * sin(phi));
    }

    vec3 alignHemisphereWithCoordinateSystem(vec3 hemisphere, vec3 up) {
        vec3 right = normalize(cross(up, vec3(0.0072f, 1.0f, 0.0034f)));
        vec3 forward = cross(right, up);

        return hemisphere.x * right + hemisphere.y * up + hemisphere.z * forward;
    }

    void main() {
        if (payload.rayActive == 0) {
            return;
        }

        ivec3 indices = ivec3(indexBuffer.data[3 * gl_PrimitiveID + 0],
                              indexBuffer.data[3 * gl_PrimitiveID + 1],
                              indexBuffer.data[3 * gl_PrimitiveID + 2]);

        vec3 barycentric = vec3(1.0 - hitCoordinate.x - hitCoordinate.y,
                                hitCoordinate.x,
                                hitCoordinate.y);

        vec3 vertexA = vec3(vertexBuffer.data[3 * indices.x + 0],
                            vertexBuffer.data[3 * indices.x + 1],
                            vertexBuffer.data[3 * indices.x + 2]);
        vec3 vertexB = vec3(vertexBuffer.data[3 * indices.y + 0],
                            vertexBuffer.data[3 * indices.y + 1],
                            vertexBuffer.data[3 * indices.y + 2]);
        vec3 vertexC = vec3(vertexBuffer.data[3 * indices.z + 0],
                            vertexBuffer.data[3 * indices.z + 1],
                            vertexBuffer.data[3 * indices.z + 2]);

        vec3 position = vertexA * barycentric.x
                      + vertexB * barycentric.y
                      + vertexC * barycentric.z;
        vec3 geometricNormal = normalize(cross(vertexB - vertexA, vertexC - vertexA));

        vec3 surfaceColor =
            materialBuffer.data[materialIndexBuffer.data[gl_PrimitiveID]].diffuse;

        // 40 & 41 == light
        if (gl_PrimitiveID == 40 || gl_PrimitiveID == 41) {
            if (payload.rayDepth == 0) {
                payload.directColor =
                    materialBuffer.data[materialIndexBuffer.data[gl_PrimitiveID]].emission;
            } else {
                payload.indirectColor += (1.0 / payload.rayDepth)
                    * materialBuffer.data[materialIndexBuffer.data[gl_PrimitiveID]].emission
                    * dot(payload.previousNormal, payload.rayDirection);
            }
        } else {
            int randomIndex =
                int(random(gl_LaunchIDEXT.xy, camera.frameCount) * 2 + 40);
            vec3 lightColor = vec3(0.6, 0.6, 0.6);

            ivec3 lightIndices = ivec3(indexBuffer.data[3 * randomIndex + 0],
                                       indexBuffer.data[3 * randomIndex + 1],
                                       indexBuffer.data[3 * randomIndex + 2]);

            vec3 lightVertexA = vec3(vertexBuffer.data[3 * lightIndices.x + 0],
                                     vertexBuffer.data[3 * lightIndices.x + 1],
                                     vertexBuffer.data[3 * lightIndices.x + 2]);
            vec3 lightVertexB = vec3(vertexBuffer.data[3 * lightIndices.y + 0],
                                     vertexBuffer.data[3 * lightIndices.y + 1],
                                     vertexBuffer.data[3 * lightIndices.y + 2]);
            vec3 lightVertexC = vec3(vertexBuffer.data[3 * lightIndices.z + 0],
                                     vertexBuffer.data[3 * lightIndices.z + 1],
                                     vertexBuffer.data[3 * lightIndices.z + 2]);

            vec2 uv = vec2(random(gl_LaunchIDEXT.xy, camera.frameCount),
                           random(gl_LaunchIDEXT.xy, camera.frameCount + 1));
            if (uv.x + uv.y > 1.0f) {
                uv.x = 1.0f - uv.x;
                uv.y = 1.0f - uv.y;
            }

            vec3 lightBarycentric = vec3(1.0 - uv.x - uv.y, uv.x, uv.y);
            vec3 lightPosition = lightVertexA * lightBarycentric.x
                               + lightVertexB * lightBarycentric.y
                               + lightVertexC * lightBarycentric.z;

            vec3 positionToLightDirection = normalize(lightPosition - position);

            vec3 shadowRayOrigin = position;
            vec3 shadowRayDirection = positionToLightDirection;
            float shadowRayDistance = length(lightPosition - position) - 0.001f;

            uint shadowRayFlags = gl_RayFlagsTerminateOnFirstHitEXT
                                | gl_RayFlagsOpaqueEXT
                                | gl_RayFlagsSkipClosestHitShaderEXT;

            isShadow = true;
            traceRayEXT(topLevelAS, shadowRayFlags, 0xFF, 0, 0, 1, shadowRayOrigin,
                        0.001, shadowRayDirection, shadowRayDistance, 1);

            if (!isShadow) {
                if (payload.rayDepth == 0) {
                    payload.directColor = surfaceColor * lightColor
                                        * dot(geometricNormal, positionToLightDirection);
                } else {
                    payload.indirectColor +=
                        (1.0 / payload.rayDepth) * surfaceColor * lightColor *
                        dot(payload.previousNormal, payload.rayDirection) *
                        dot(geometricNormal, positionToLightDirection);
                }
            } else {
                if (payload.rayDepth == 0) {
                    payload.directColor = vec3(0.0, 0.0, 0.0);
                } else {
                    payload.rayActive = 0;
                }
            }
        }

        vec3 hemisphere = uniformSampleHemisphere(vec2(
            random(gl_LaunchIDEXT.xy, camera.frameCount),
            random(gl_LaunchIDEXT.xy, camera.frameCount + 1)
        ));
        vec3 alignedHemisphere =
            alignHemisphereWithCoordinateSystem(hemisphere, geometricNormal);

        payload.rayOrigin = position;
        payload.rayDirection = alignedHemisphere;
        payload.previousNormal = geometricNormal;

        payload.rayDepth += 1;
    }
    "#,
    rchit,
    vulkan1_2
)
.as_slice();

static SHADER_MISS: &[u32] = inline_spirv!(
    r#"
    #version 460
    #extension GL_EXT_ray_tracing : require

    layout(location = 0) rayPayloadInEXT Payload {
        vec3 rayOrigin;
        vec3 rayDirection;
        vec3 previousNormal;

        vec3 directColor;
        vec3 indirectColor;
        int rayDepth;

        int rayActive;
    } payload;

    void main() {
        payload.rayActive = 0;
    }
    "#,
    rmiss,
    vulkan1_2
)
.as_slice();

static SHADER_SHADOW_MISS: &[u32] = inline_spirv!(
    r#"
    #version 460
    #extension GL_EXT_ray_tracing : require

    layout(location = 1) rayPayloadInEXT bool isShadow;

    void main() {
        isShadow = false;
    }
    "#,
    rmiss,
    vulkan1_2
)
.as_slice();

fn create_ray_trace_pipeline(
    device: &Shared<Device>,
) -> Result<Shared<RayTracePipeline>, DriverError> {
    Ok(Shared::new(RayTracePipeline::create(
        device,
        RayTracePipelineInfo::default(),
        [
            Shader::new_ray_gen(SHADER_RAY_GEN),
            Shader::new_closest_hit(SHADER_CLOSEST_HIT),
            Shader::new_miss(SHADER_MISS),
            Shader::new_miss(SHADER_SHADOW_MISS),
        ],
        [
            RayTraceShaderGroup::new_triangles(None, None, None, 0),
            RayTraceShaderGroup::new_general(1, None, None, None),
            RayTraceShaderGroup::new_general(2, None, None, None),
            RayTraceShaderGroup::new_general(3, None, None, None),
        ],
    )?))
}

fn load_scene_buffers(
    device: &Shared<Device>,
) -> Result<
    (
        Option<BufferBinding>,
        u32,
        Option<BufferBinding>,
        u32,
        Option<BufferBinding>,
        Option<BufferBinding>,
    ),
    DriverError,
> {
    use std::slice::from_raw_parts;

    let (mut models, materials, ..) = load_obj_buf(
        &mut BufReader::new(include_bytes!("res/cube_scene.obj").as_slice()),
        &LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |_| {
            load_mtl_buf(&mut BufReader::new(
                include_bytes!("res/cube_scene.mtl").as_slice(),
            ))
        },
    )
    .map_err(|err| {
        warn!("{err}");

        DriverError::InvalidData
    })?;
    let materials = materials.map_err(|err| {
        warn!("{err}");

        DriverError::InvalidData
    })?;

    let indices = models
        .iter()
        .map(|model| model.mesh.indices.iter().copied())
        .flatten()
        .collect::<Vec<_>>();
    let index_buf = BufferBinding::new({
        let data = cast_slice(indices.as_slice());
        let mut buf = Buffer::create(
            device,
            BufferInfo::new_mappable(
                data.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, data);
        buf
    });
    let index_count = indices.len() as u32;

    let positions = models
        .iter()
        .map(|model| model.mesh.positions.iter().copied())
        .flatten()
        .collect::<Vec<_>>();
    let vertex_buf = BufferBinding::new({
        let data = cast_slice(positions.as_slice());
        let mut buf = Buffer::create(
            device,
            BufferInfo::new_mappable(
                data.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, data);
        buf
    });
    let vertex_count = positions.len() as u32;

    let material_id_buf = BufferBinding::new({
        let material_ids = models
            .iter()
            .map(|model| model.mesh.material_id.unwrap_or_default())
            .collect::<Vec<_>>();
        let data = cast_slice(material_ids.as_slice());
        let mut buf = Buffer::create(
            device,
            BufferInfo::new_mappable(data.len() as _, vk::BufferUsageFlags::STORAGE_BUFFER),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, data);
        buf
    });

    let material_buf = BufferBinding::new({
        #[repr(C)]
        struct Material {
            ambient: [f32; 4],
            diffuse: [f32; 4],
            specular: [f32; 4],
            emission: [f32; 4],
        }

        let materials = models
            .iter()
            .map(|model| {
                let material = &materials[model.mesh.material_id.unwrap_or_default()];

                Material {
                    ambient: [
                        material.ambient[0],
                        material.ambient[1],
                        material.ambient[2],
                        0.0,
                    ],
                    diffuse: [
                        material.diffuse[0],
                        material.diffuse[1],
                        material.diffuse[2],
                        0.0,
                    ],
                    specular: [
                        material.specular[0],
                        material.specular[1],
                        material.specular[2],
                        0.0,
                    ],
                    emission: [1.0, 0.0, 0.0, 0.0],
                }
            })
            .collect::<Vec<_>>();
        let data = unsafe { from_raw_parts(materials.as_ptr() as *const _, size_of::<Material>()) };
        let mut buf = Buffer::create(
            device,
            BufferInfo::new_mappable(
                (size_of::<Material>() * materials.len()) as _,
                vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, data);
        buf
    });

    Ok((
        Some(index_buf),
        index_count,
        Some(vertex_buf),
        vertex_count,
        Some(material_id_buf),
        Some(material_buf),
    ))
}

/// Copied from http://williamlewww.com/showcase_website/vk_khr_ray_tracing_tutorial/index.html
fn main() -> anyhow::Result<()> {
    use std::slice::{from_raw_parts, from_ref};

    pretty_env_logger::init();

    let event_loop = EventLoop::new().ray_tracing(true).build()?;
    let mut cache = HashPool::new(&event_loop.device);

    let (
        mut index_buf,
        index_count,
        mut vertex_buf,
        vertex_count,
        mut material_id_buf,
        mut material_buf,
    ) = load_scene_buffers(&event_loop.device)?;

    let &PhysicalDeviceRayTracePipelineProperties {
        shader_group_base_alignment,
        shader_group_handle_size,
        ..
    } = event_loop
        .device
        .ray_tracing_pipeline_properties
        .as_ref()
        .unwrap();
    let ray_trace_pipeline = create_ray_trace_pipeline(&event_loop.device)?;
    let mut sbt_buf = Some(BufferBinding::new({
        let mut buf = Buffer::create(
            &event_loop.device,
            BufferInfo::new_mappable(
                4 * shader_group_handle_size as vk::DeviceSize,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
            ),
        )
        .unwrap();

        let shader_handle_buf = unsafe {
            event_loop
                .device
                .ray_tracing_pipeline_ext
                .as_ref()
                .unwrap()
                .get_ray_tracing_shader_group_handles(
                    **ray_trace_pipeline,
                    0,
                    4,
                    buf.info.size as _,
                )
        }?;

        let data = Buffer::mapped_slice_mut(&mut buf);
        for idx in 0..4 {
            let data_start = idx * shader_group_handle_size as usize;
            let data_end = data_start + shader_group_handle_size as usize;
            let buf_start = idx * shader_group_base_alignment as usize;
            let buf_end = buf_start + shader_group_handle_size as usize;
            data[data_start..data_end].copy_from_slice(&shader_handle_buf[buf_start..buf_end]);
        }

        buf
    }));
    let sbt_address = Buffer::device_address(sbt_buf.as_ref().unwrap().as_ref());
    let sbt_size = 4 * shader_group_base_alignment as vk::DeviceSize;
    let sbt_rchit = vk::StridedDeviceAddressRegionKHR {
        device_address: sbt_address,
        stride: shader_group_base_alignment as _,
        size: sbt_size,
    };
    let sbt_rgen = vk::StridedDeviceAddressRegionKHR {
        device_address: sbt_address + shader_group_base_alignment as vk::DeviceAddress,
        stride: sbt_size,
        size: sbt_size,
    };
    let sbt_rmiss = vk::StridedDeviceAddressRegionKHR {
        device_address: sbt_address + 2 * shader_group_base_alignment as vk::DeviceAddress,
        stride: shader_group_base_alignment as _,
        size: 2 * sbt_size,
    };
    let sbt_callable = vk::StridedDeviceAddressRegionKHR::default();

    let blas_geometry_info = AccelerationStructureGeometryInfo {
        ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
        flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
        geometries: vec![AccelerationStructureGeometry {
            count: 1,
            flags: vk::GeometryFlagsKHR::OPAQUE,
            geometry: AccelerationStructureGeometryData::Triangles {
                index_type: vk::IndexType::UINT32,
                max_vertex: vertex_count,
                transform_data: Default::default(),
                vertex_format: vk::Format::R32G32B32_SFLOAT,
                vertex_stride: 24,
            },
        }],
    };
    let blas_size = AccelerationStructure::size_of(&event_loop.device, &blas_geometry_info);
    let mut blas = Some(AccelerationStructureBinding::new(
        AccelerationStructure::create(
            &event_loop.device,
            AccelerationStructureInfo {
                ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
                size: blas_size.create_size,
            },
        )?,
    ));

    let tlas_geometry_info = AccelerationStructureGeometryInfo {
        ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
        flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
        geometries: vec![AccelerationStructureGeometry {
            count: 1,
            flags: vk::GeometryFlagsKHR::OPAQUE,
            geometry: AccelerationStructureGeometryData::Instances {
                array_of_pointers: false,
            },
        }],
    };
    let tlas_size = AccelerationStructure::size_of(&event_loop.device, &tlas_geometry_info);
    let mut tlas = Some(AccelerationStructureBinding::new({
        let mut accel_struct = AccelerationStructure::create(
            &event_loop.device,
            AccelerationStructureInfo {
                ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
                size: tlas_size.create_size,
            },
        )?;
        let device_handle = AccelerationStructure::device_address(&accel_struct);
        Buffer::copy_from_slice(&mut accel_struct.buffer, 0, unsafe {
            from_raw_parts(
                &vk::AccelerationStructureInstanceKHR {
                    transform: vk::TransformMatrixKHR { matrix: [0.0; 12] },
                    instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xff),
                    instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(
                        0,
                        vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as _,
                    ),
                    acceleration_structure_reference: vk::AccelerationStructureReferenceKHR {
                        device_handle,
                    },
                } as *const _ as *const u8,
                size_of::<vk::AccelerationStructureInstanceKHR>(),
            )
        });

        accel_struct
    }));

    let mut render_graph = RenderGraph::new();

    {
        let scratch_buf = render_graph.bind_node(Buffer::create(
            &event_loop.device,
            BufferInfo::new(
                blas_size.build_size,
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?);
        let blas_node = render_graph.bind_node(blas.take().unwrap());
        render_graph.build_acceleration_structure(
            blas_node,
            scratch_buf,
            blas_geometry_info,
            [vk::AccelerationStructureBuildRangeInfoKHR {
                first_vertex: 0,
                primitive_count: index_count / 3,
                primitive_offset: 0,
                transform_offset: 0,
            }],
        );
        blas = Some(render_graph.unbind_node(blas_node));
    }

    {
        let scratch_buf = render_graph.bind_node(Buffer::create(
            &event_loop.device,
            BufferInfo::new(
                tlas_size.build_size,
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?);
        let tlas_node = render_graph.bind_node(tlas.take().unwrap());
        render_graph.build_acceleration_structure(
            tlas_node,
            scratch_buf,
            tlas_geometry_info,
            [vk::AccelerationStructureBuildRangeInfoKHR {
                first_vertex: 0,
                primitive_count: 1,
                primitive_offset: 0,
                transform_offset: 0,
            }],
        );
        tlas = Some(render_graph.unbind_node(tlas_node));
    }

    render_graph.resolve().submit(&mut cache)?;

    event_loop.run(|frame| {
        let camera_buf = frame.render_graph.bind_node({
            #[repr(C)]
            struct Camera {
                position: [f32; 4],
                right: [f32; 4],
                up: [f32; 4],
                forward: [f32; 4],
                frame_count: u32,
            }

            let mut buf = cache
                .lease(BufferInfo::new_mappable(
                    70,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                ))
                .unwrap();
            Buffer::copy_from_slice(buf.get_mut().unwrap(), 0, unsafe {
                from_raw_parts(
                    &Camera {
                        position: [0f32, 0.0, 0.0, 1.0],
                        right: [1f32, 0.0, 0.0, 1.0],
                        up: [0f32, 1.0, 0.0, 1.0],
                        forward: [0f32, 0.0, 1.0, 1.0],
                        frame_count: 0,
                    } as *const _ as *const u8,
                    size_of::<Camera>(),
                )
            });

            buf
        });
        let image = frame.render_graph.bind_node(
            cache
                .lease(ImageInfo::new_2d(
                    vk::Format::R8G8B8A8_UNORM,
                    frame.width,
                    frame.height,
                    vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC,
                ))
                .unwrap(),
        );
        let blas_node = frame.render_graph.bind_node(blas.take().unwrap());
        let tlas_node = frame.render_graph.bind_node(tlas.take().unwrap());
        let index_buf_node = frame.render_graph.bind_node(index_buf.take().unwrap());
        let vertex_buf_node = frame.render_graph.bind_node(vertex_buf.take().unwrap());
        let material_id_buf_node = frame
            .render_graph
            .bind_node(material_id_buf.take().unwrap());
        let material_buf_node = frame.render_graph.bind_node(material_buf.take().unwrap());

        frame
            .render_graph
            .begin_pass("basic ray tracer")
            .bind_pipeline(&ray_trace_pipeline)
            .access_node(blas_node, AccessType::AnyShaderReadOther)
            .access_descriptor(
                0,
                tlas_node,
                AccessType::RayTracingShaderReadAccelerationStructure,
            )
            .access_descriptor(1, camera_buf, AccessType::RayTracingShaderReadOther)
            .access_descriptor(2, index_buf_node, AccessType::RayTracingShaderReadOther)
            .access_descriptor(3, vertex_buf_node, AccessType::RayTracingShaderReadOther)
            .write_descriptor(4, image)
            .access_descriptor(
                5,
                material_id_buf_node,
                AccessType::RayTracingShaderReadOther,
            )
            .access_descriptor(6, material_buf_node, AccessType::RayTracingShaderReadOther)
            .record_ray_trace(move |ray_trace| {
                ray_trace.trace_rays(
                    &sbt_rgen,
                    &sbt_rmiss,
                    &sbt_rchit,
                    &sbt_callable,
                    frame.width,
                    frame.height,
                    1,
                );
            })
            .submit_pass()
            .copy_image(image, frame.swapchain_image);

        blas = Some(frame.render_graph.unbind_node(blas_node));
        tlas = Some(frame.render_graph.unbind_node(tlas_node));
        index_buf = Some(frame.render_graph.unbind_node(index_buf_node));
        vertex_buf = Some(frame.render_graph.unbind_node(vertex_buf_node));
        material_id_buf = Some(frame.render_graph.unbind_node(material_id_buf_node));
        material_buf = Some(frame.render_graph.unbind_node(material_buf_node));
    })?;

    Ok(())
}