use {
    bytemuck::{bytes_of, cast_slice},
    inline_spirv::inline_spirv,
    screen_13::prelude_arc::*,
    std::io::BufReader,
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
) -> Result<(BufferBinding, u32, BufferBinding, u32), DriverError> {
    let (mut models, ..) = load_obj_buf(
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

    let indices = models
        .iter()
        .map(|model| model.mesh.indices.iter().copied())
        .flatten()
        .collect::<Vec<_>>();
    let indices_slice = cast_slice(indices.as_slice());
    let index_buf = BufferBinding::new({
        let mut buf = Buffer::create(
            device,
            BufferInfo::new_mappable(
                indices_slice.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, indices_slice);
        buf
    });
    let index_count = indices.len() as u32;

    let positions = models
        .iter()
        .map(|model| model.mesh.positions.iter().copied())
        .flatten()
        .collect::<Vec<_>>();
    let positions_slice = cast_slice(positions.as_slice());
    let vertex_buf = BufferBinding::new({
        let mut buf = Buffer::create(
            device,
            BufferInfo::new_mappable(
                positions_slice.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, positions_slice);
        buf
    });
    let vertex_count = positions.len() as u32;

    Ok((index_buf, index_count, vertex_buf, vertex_count))
}

/// Copied from http://williamlewww.com/showcase_website/vk_khr_ray_tracing_tutorial/index.html
fn main() -> anyhow::Result<()> {
    use std::slice::from_ref;

    pretty_env_logger::init();

    let event_loop = EventLoop::new().ray_tracing(true).build()?;
    let mut cache = HashPool::new(&event_loop.device);

    let (index_buf, index_count, vertex_buf, vertex_count) =
        load_scene_buffers(&event_loop.device)?;
    let ray_trace_pipeline = create_ray_trace_pipeline(&event_loop.device)?;

    let blas = AccelerationStructure::create_blas(
        &event_loop.device,
        &vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .scratch_data(vk::DeviceOrHostAddressKHR { device_address: 0 })
            .geometries(from_ref(&vk::AccelerationStructureGeometryKHR {
                flags: vk::GeometryFlagsKHR::OPAQUE,
                geometry_type: vk::GeometryTypeKHR::TRIANGLES,
                geometry: vk::AccelerationStructureGeometryDataKHR {
                    triangles: vk::AccelerationStructureGeometryTrianglesDataKHR {
                        index_type: vk::IndexType::UINT32,
                        index_data: vk::DeviceOrHostAddressConstKHR {
                            device_address: Buffer::device_address(index_buf.as_ref()),
                        },
                        vertex_format: vk::Format::R32G32B32_SFLOAT,
                        vertex_data: vk::DeviceOrHostAddressConstKHR {
                            device_address: Buffer::device_address(vertex_buf.as_ref()),
                        },
                        vertex_stride: 24,
                        max_vertex: vertex_count,
                        transform_data: vk::DeviceOrHostAddressConstKHR { device_address: 0 },
                        ..Default::default()
                    },
                },
                ..Default::default()
            })),
        from_ref(&index_count),
    )?;

    {
        let cmd_buf = cache.lease(event_loop.device.queue.family)?;
        let mut render_graph = RenderGraph::new();
        let blas = render_graph.bind_node(blas);
        render_graph.build_acceleration_structure(blas);
    }

    //   std::vector<uint32_t> materialIndexList;
    //   for (tinyobj::shape_t shape : shapes) {
    //     for (int index : shape.mesh.material_ids) {
    //       materialIndexList.push_back(index);
    //     }
    //   }

    //   VkBufferCreateInfo materialIndexBufferCreateInfo = {
    //       .sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
    //       .pNext = NULL,
    //       .flags = 0,
    //       .size = sizeof(uint32_t) * materialIndexList.size(),
    //       .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT,
    //       .sharingMode = VK_SHARING_MODE_EXCLUSIVE,
    //       .queueFamilyIndexCount = 1,
    //       .pQueueFamilyIndices = &queueFamilyIndex};

    // // =========================================================================
    //   // Material Buffer

    //   struct Material {
    //     float ambient[4] = {0, 0, 0, 0};
    //     float diffuse[4] = {0, 0, 0, 0};
    //     float specular[4] = {0, 0, 0, 0};
    //     float emission[4] = {0, 0, 0, 0};
    //   };

    //   std::vector<Material> materialList(materials.size());
    //   for (uint32_t x = 0; x < materials.size(); x++) {
    //     memcpy(materialList[x].ambient, materials[x].ambient, sizeof(float) * 3);
    //     memcpy(materialList[x].diffuse, materials[x].diffuse, sizeof(float) * 3);
    //     memcpy(materialList[x].specular, materials[x].specular, sizeof(float) * 3);
    //     memcpy(materialList[x].emission, materials[x].emission, sizeof(float) * 3);
    //   }

    //   VkBufferCreateInfo materialBufferCreateInfo = {
    //       .sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
    //       .pNext = NULL,
    //       .flags = 0,
    //       .size = sizeof(Material) * materialList.size(),
    //       .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT,
    //       .sharingMode = VK_SHARING_MODE_EXCLUSIVE,
    //       .queueFamilyIndexCount = 1,
    //       .pQueueFamilyIndices = &queueFamilyIndex};

    event_loop.run(|frame| {
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
        let uniform_buf = frame.render_graph.bind_node({
            let mut buf = cache
                .lease(BufferInfo::new_mappable(
                    70,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                ))
                .unwrap();
            let data = Buffer::mapped_slice_mut(buf.get_mut().unwrap());
            data[0..16].copy_from_slice(bytes_of(&[0f32, 0.0, 0.0, 1.0]));
            data[16..32].copy_from_slice(bytes_of(&[1f32, 0.0, 0.0, 1.0]));
            data[32..48].copy_from_slice(bytes_of(&[0f32, 1.0, 0.0, 1.0]));
            data[48..64].copy_from_slice(bytes_of(&[0f32, 0.0, 1.0, 1.0]));
            data[64..70].copy_from_slice(&0u32.to_ne_bytes());
            buf
        });

        frame
            .render_graph
            .begin_pass("basic ray tracer")
            .bind_pipeline(&ray_trace_pipeline)
            .read_descriptor(0, uniform_buf)
            .record_ray_trace(|ray_trace| {
                //ray_trace.trace_rays(frame.width, frame.height, 1);
            })
            .submit_pass()
            .copy_image(image, frame.swapchain_image);
    })?;

    Ok(())
}
