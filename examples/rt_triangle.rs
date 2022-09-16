use {bytemuck::cast_slice, inline_spirv::inline_spirv, screen_13::prelude::*, std::sync::Arc};

static SHADER_RAY_GEN: &[u32] = inline_spirv!(
    r#"
    #version 460
    #extension GL_EXT_ray_tracing : enable
    
    layout(binding = 0, set = 0) uniform accelerationStructureEXT topLevelAS;
    layout(binding = 1, set = 0, rgba32f) uniform image2D image;
    
    layout(location = 0) rayPayloadEXT vec3 hitValue;
    
    void main() {
        const vec2 pixelCenter = vec2(gl_LaunchIDEXT.xy) + vec2(0.5);
        const vec2 inUV = pixelCenter/vec2(gl_LaunchSizeEXT.xy);
        vec2 d = inUV * 2.0 - 1.0;
    
        vec4 origin = vec4(d.x, d.y, -1,1);
        vec4 target = vec4(d.x, d.y, 1, 1) ;
        vec4 direction = vec4(normalize(target.xyz), 0) ;
    
        float tmin = 0.001;
        float tmax = 10000.0;
    
        // hitValue = vec3(0.1);
    
        traceRayEXT(topLevelAS, gl_RayFlagsOpaqueEXT, 0xff, 0, 0, 0, origin.xyz, tmin, direction.xyz, tmax, 0);
    
        imageStore(image, ivec2(gl_LaunchIDEXT.xy), vec4(hitValue, 0.0));
    }
    "#,
    rgen,
    vulkan1_2
)
.as_slice();

static SHADER_CLOSEST_HIT: &[u32] = inline_spirv!(
    r#"
    #version 460
    #extension GL_EXT_ray_tracing : enable
    #extension GL_EXT_nonuniform_qualifier : enable
    
    layout(location = 0) rayPayloadInEXT vec3 resultColor;
    hitAttributeEXT vec2 attribs;
    
    void main() {
      const vec3 barycentricCoords = vec3(1.0f - attribs.x - attribs.y, attribs.x, attribs.y);
      resultColor = barycentricCoords;
    }
    "#,
    rchit,
    vulkan1_2
)
.as_slice();

static SHADER_MISS: &[u32] = inline_spirv!(
    r#"
    #version 460
    #extension GL_EXT_ray_tracing : enable
    
    layout(location = 0) rayPayloadInEXT vec3 hitValue;
    
    void main() {
        hitValue = vec3(0.0, 0.0, 0.2);
    }
    "#,
    rmiss,
    vulkan1_2
)
.as_slice();

fn align_up(val: u32, atom: u32) -> u32 {
    (val + atom - 1) & !(atom - 1)
}

fn create_ray_trace_pipeline(device: &Arc<Device>) -> Result<Arc<RayTracePipeline>, DriverError> {
    Ok(Arc::new(RayTracePipeline::create(
        device,
        RayTracePipelineInfo::new()
            .max_ray_recursion_depth(1)
            .build(),
        [
            Shader::new_ray_gen(SHADER_RAY_GEN),
            Shader::new_closest_hit(SHADER_CLOSEST_HIT),
            Shader::new_miss(SHADER_MISS),
        ],
        [
            RayTraceShaderGroup::new_general(0),
            RayTraceShaderGroup::new_triangles(1, None),
            RayTraceShaderGroup::new_general(2),
        ],
    )?))
}

/// Adapted from http://williamlewww.com/showcase_website/vk_khr_ray_tracing_tutorial/index.html
fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let event_loop = EventLoop::new().ray_tracing(true).build()?;
    let mut cache = HashPool::new(&event_loop.device);

    // ------------------------------------------------------------------------------------------ //
    // Setup the ray tracing pipeline
    // ------------------------------------------------------------------------------------------ //

    let &PhysicalDeviceRayTracePipelineProperties {
        shader_group_base_alignment,
        shader_group_handle_alignment,
        shader_group_handle_size,
        ..
    } = event_loop
        .device
        .ray_tracing_pipeline_properties
        .as_ref()
        .unwrap();
    let ray_trace_pipeline = create_ray_trace_pipeline(&event_loop.device)?;

    // ------------------------------------------------------------------------------------------ //
    // Setup a shader binding table
    // ------------------------------------------------------------------------------------------ //

    let sbt_handle_size = align_up(shader_group_handle_size, shader_group_handle_alignment);
    let sbt_rgen_size = align_up(sbt_handle_size, shader_group_base_alignment);
    let sbt_hit_size = align_up(sbt_handle_size, shader_group_base_alignment);
    let sbt_miss_size = align_up(2 * sbt_handle_size, shader_group_base_alignment);
    let sbt_buf = Arc::new({
        let mut buf = Buffer::create(
            &event_loop.device,
            BufferInfo::new_mappable(
                (sbt_rgen_size + sbt_hit_size + sbt_miss_size) as _,
                vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            ),
        )
        .unwrap();

        let mut data = Buffer::mapped_slice_mut(&mut buf);
        data.fill(0);

        let rgen_handle = ray_trace_pipeline.group_handle(0)?;
        data[0..rgen_handle.len()].copy_from_slice(rgen_handle);
        data = &mut data[sbt_rgen_size as _..];

        // If hit/miss had different strides we would need to iterate each here
        for idx in 1..3 {
            let handle = ray_trace_pipeline.group_handle(idx)?;
            data[0..handle.len()].copy_from_slice(handle);
            data = &mut data[sbt_handle_size as _..];
        }

        buf
    });
    let sbt_address = Buffer::device_address(&sbt_buf);
    let sbt_rgen = vk::StridedDeviceAddressRegionKHR {
        device_address: sbt_address,
        stride: sbt_rgen_size as _,
        size: sbt_rgen_size as _,
    };
    let sbt_hit = vk::StridedDeviceAddressRegionKHR {
        device_address: sbt_rgen.device_address + sbt_rgen_size as vk::DeviceAddress,
        stride: sbt_handle_size as _,
        size: sbt_hit_size as _,
    };
    let sbt_miss = vk::StridedDeviceAddressRegionKHR {
        device_address: sbt_hit.device_address + sbt_hit_size as vk::DeviceAddress,
        stride: sbt_handle_size as _,
        size: sbt_miss_size as _,
    };
    let sbt_callable = vk::StridedDeviceAddressRegionKHR::default();

    // ------------------------------------------------------------------------------------------ //
    // Generate the geometry and load it into buffers
    // ------------------------------------------------------------------------------------------ //
    let triangle_count = 1;
    let vertex_count = triangle_count * 3;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    #[allow(dead_code)]
    struct Vertex {
        pos: [f32; 3],
    }

    unsafe impl bytemuck::Pod for Vertex {}
    unsafe impl bytemuck::Zeroable for Vertex {}

    const VERTICES: [Vertex; 3] = [
        Vertex {
            pos: [-1.0, 1.0, 0.0],
        },
        Vertex {
            pos: [1.0, 1.0, 0.0],
        },
        Vertex {
            pos: [0.0, -1.0, 0.0],
        },
    ];

    const INDICES: [u32; 3] = [0, 1, 2];

    let index_buf = {
        let data = cast_slice(&INDICES);
        let mut buf = Buffer::create(
            &event_loop.device,
            BufferInfo::new_mappable(
                data.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, data);
        Arc::new(buf)
    };

    let vertex_buf = {
        let data = cast_slice(&VERTICES);
        let mut buf = Buffer::create(
            &event_loop.device,
            BufferInfo::new_mappable(
                data.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ),
        )?;
        Buffer::copy_from_slice(&mut buf, 0, data);
        Arc::new(buf)
    };

    // ------------------------------------------------------------------------------------------ //
    // Create the bottom level acceleration structure
    // ------------------------------------------------------------------------------------------ //

    let blas_geometry_info = AccelerationStructureGeometryInfo {
        ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
        flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
        geometries: vec![AccelerationStructureGeometry {
            max_primitive_count: triangle_count,
            flags: vk::GeometryFlagsKHR::OPAQUE,
            geometry: AccelerationStructureGeometryData::Triangles {
                index_data: DeviceOrHostAddress::DeviceAddress(Buffer::device_address(&index_buf)),
                index_type: vk::IndexType::UINT32,
                max_vertex: vertex_count,
                transform_data: None,
                vertex_data: DeviceOrHostAddress::DeviceAddress(Buffer::device_address(
                    &vertex_buf,
                )),
                vertex_format: vk::Format::R32G32B32_SFLOAT,
                vertex_stride: 12,
            },
        }],
    };
    let blas_size = AccelerationStructure::size_of(&event_loop.device, &blas_geometry_info);
    let blas = Arc::new(AccelerationStructure::create(
        &event_loop.device,
        AccelerationStructureInfo {
            ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
            size: blas_size.create_size,
        },
    )?);
    let blas_device_address = AccelerationStructure::device_address(&blas);

    // ------------------------------------------------------------------------------------------ //
    // Create an instance buffer, which is just one instance for the single BLAS
    // ------------------------------------------------------------------------------------------ //

    let instance = AccelerationStructure::instance_slice(vk::AccelerationStructureInstanceKHR {
        transform: vk::TransformMatrixKHR {
            matrix: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
            ],
        },
        instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xff),
        instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(
            0,
            vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as _,
        ),
        acceleration_structure_reference: vk::AccelerationStructureReferenceKHR {
            device_handle: blas_device_address,
        },
    });
    let instance_buf = Arc::new({
        let mut buffer = Buffer::create(
            &event_loop.device,
            BufferInfo::new_mappable(
                instance.len() as _,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            ),
        )?;
        Buffer::copy_from_slice(&mut buffer, 0, instance);

        buffer
    });

    // ------------------------------------------------------------------------------------------ //
    // Create the top level acceleration structure
    // ------------------------------------------------------------------------------------------ //

    let tlas_geometry_info = AccelerationStructureGeometryInfo {
        ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
        flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
        geometries: vec![AccelerationStructureGeometry {
            max_primitive_count: 1,
            flags: vk::GeometryFlagsKHR::OPAQUE,
            geometry: AccelerationStructureGeometryData::Instances {
                array_of_pointers: false,
                data: DeviceOrHostAddress::DeviceAddress(Buffer::device_address(&instance_buf)),
            },
        }],
    };
    let tlas_size = AccelerationStructure::size_of(&event_loop.device, &tlas_geometry_info);
    let tlas = Arc::new(AccelerationStructure::create(
        &event_loop.device,
        AccelerationStructureInfo {
            ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
            size: tlas_size.create_size,
        },
    )?);

    // ------------------------------------------------------------------------------------------ //
    // Build the BLAS and TLAS; note that we don't drop the cache and so there is no CPU stall
    // ------------------------------------------------------------------------------------------ //

    {
        let mut render_graph = RenderGraph::new();
        let index_node = render_graph.bind_node(&index_buf);
        let vertex_node = render_graph.bind_node(&vertex_buf);
        let blas_node = render_graph.bind_node(&blas);

        {
            let scratch_buf = render_graph.bind_node(Buffer::create(
                &event_loop.device,
                BufferInfo::new(
                    blas_size.build_size,
                    vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                        | vk::BufferUsageFlags::STORAGE_BUFFER,
                ),
            )?);

            render_graph
                .begin_pass("Build BLAS")
                .read_node(index_node)
                .read_node(vertex_node)
                .write_node(blas_node)
                .write_node(scratch_buf)
                .record_acceleration(move |accel| {
                    accel.build_structure(
                        blas_node,
                        scratch_buf,
                        &blas_geometry_info,
                        &[vk::AccelerationStructureBuildRangeInfoKHR {
                            first_vertex: 0,
                            primitive_count: triangle_count,
                            primitive_offset: 0,
                            transform_offset: 0,
                        }],
                    )
                });
        }

        {
            let scratch_buf = render_graph.bind_node(Buffer::create(
                &event_loop.device,
                BufferInfo::new(
                    tlas_size.build_size,
                    vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                        | vk::BufferUsageFlags::STORAGE_BUFFER,
                ),
            )?);
            let instance_node = render_graph.bind_node(&instance_buf);
            let tlas_node = render_graph.bind_node(&tlas);

            render_graph
                .begin_pass("Build TLAS")
                .read_node(blas_node)
                .read_node(instance_node)
                .write_node(scratch_buf)
                .write_node(tlas_node)
                .record_acceleration(move |accel| {
                    accel.build_structure(
                        tlas_node,
                        scratch_buf,
                        &tlas_geometry_info,
                        &[vk::AccelerationStructureBuildRangeInfoKHR {
                            first_vertex: 0,
                            primitive_count: 1,
                            primitive_offset: 0,
                            transform_offset: 0,
                        }],
                    );
                });
        }

        render_graph
            .resolve()
            .submit(&event_loop.device.queue, &mut cache)?;
    }

    // ------------------------------------------------------------------------------------------ //
    // Setup some state variables to hold between frames
    // ------------------------------------------------------------------------------------------ //

    let mut image = None;

    // The event loop consists of:
    // - Trace the image
    // - Copy image to the swapchain
    event_loop.run(|frame| {
        image = Some(Arc::new(
            cache
                .lease(ImageInfo::new_2d(
                    frame.render_graph.node_info(frame.swapchain_image).fmt,
                    frame.width,
                    frame.height,
                    vk::ImageUsageFlags::STORAGE
                        | vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                ))
                .unwrap(),
        ));

        let image_node = frame.render_graph.bind_node(image.as_ref().unwrap());

        let blas_node = frame.render_graph.bind_node(&blas);
        let tlas_node = frame.render_graph.bind_node(&tlas);
        let sbt_node = frame.render_graph.bind_node(&sbt_buf);

        frame
            .render_graph
            .begin_pass("basic ray tracer")
            .bind_pipeline(&ray_trace_pipeline)
            .access_node(
                blas_node,
                AccessType::RayTracingShaderReadAccelerationStructure,
            )
            .access_node(sbt_node, AccessType::RayTracingShaderReadOther)
            .access_descriptor(
                0,
                tlas_node,
                AccessType::RayTracingShaderReadAccelerationStructure,
            )
            .write_descriptor(1, image_node)
            .record_ray_trace(move |ray_trace| {
                ray_trace.trace_rays(
                    &sbt_rgen,
                    &sbt_miss,
                    &sbt_hit,
                    &sbt_callable,
                    frame.width,
                    frame.height,
                    1,
                );
            })
            .submit_pass()
            .copy_image(image_node, frame.swapchain_image);
    })?;

    Ok(())
}
