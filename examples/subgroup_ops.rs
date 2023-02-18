use {
    bytemuck::cast_slice,
    inline_spirv::inline_spirv,
    screen_13::prelude::*,
    std::{mem::size_of, sync::Arc, time::Instant},
};

/// Advanced example demonstrating subgroup operations (arithmetic and ballot).
///
/// This code demonstrates "exclusive sum", which is a GPU-implementation of "parallel prefix scan".
///
/// Given an input buffer of unsigned integers, the result is an output buffer where each index is
/// the sum of all items prior to that index. Example:
///
/// Input:  [1, 1, 1, 1]
/// Output: [0, 1, 2, 3]
///
/// Input:  [1, 4, 0, 2]
/// Output: [0, 1, 5, 5]
///
/// This algorthim is useful in *many* places, one of which being GPU-driven rendering. In that case
/// you may have a list of meshes where each index contains the number of those meshes in the scene.
/// Exclusive sum might be used to generate an offset where you may safely record instance drawing
/// commands for a given mesh.
///
/// https://www.khronos.org/blog/vulkan-subgroup-tutorial
/// https://www.khronos.org/assets/uploads/developers/library/2018-vulkan-devday/06-subgroups.pdf
fn main() -> Result<(), DriverError> {
    pretty_env_logger::init();

    let device = Arc::new(Device::new(DriverConfig::new().build())?);

    assert!(device
        .vulkan_1_1_properties
        .subgroup_supported_operations
        .contains(vk::SubgroupFeatureFlags::ARITHMETIC));
    assert!(device
        .vulkan_1_1_properties
        .subgroup_supported_operations
        .contains(vk::SubgroupFeatureFlags::BALLOT));

    // We run a number of different workgroup sizes to be sure it works, but generally
    // Nvidia and Intel prefer a workgroup size of 32 and AMD prefers 64.
    for num_subgroups in [1, 2, 4, 8, 16, 32] {
        let workgroup_size = device.vulkan_1_1_properties.subgroup_size * num_subgroups;

        for data_len in [32, 64, 128, 256, 512, 1024, 2048, 4096, 16384] {
            // Provided data must always be at least a full workgroup
            if data_len < workgroup_size {
                continue;
            }

            exclusive_sum(&device, data_len, workgroup_size)?;
        }
    }

    Ok(())
}

fn exclusive_sum(
    device: &Arc<Device>,
    data_len: u32,
    workgroup_size: u32,
) -> Result<(), DriverError> {
    let workgroup_size = workgroup_size.max(device.vulkan_1_1_properties.subgroup_size);
    let num_subgroups = workgroup_size / device.vulkan_1_1_properties.subgroup_size;

    assert_eq!(
        data_len % device.vulkan_1_1_properties.subgroup_size,
        0,
        "Data must always be a multiple of subgroup size"
    );

    let mut render_graph = RenderGraph::new();

    let input_data = generate_input_data(data_len);
    let input_bytes = cast_slice(&input_data);
    let input_buf = render_graph.bind_node(Buffer::create_from_slice(
        &device,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        input_bytes,
    )?);

    let reduce_buf_len = (size_of::<u32>() as u32 * data_len / workgroup_size) as _;
    let reduce_buf = render_graph.bind_node(Buffer::create(
        &device,
        BufferInfo::new_mappable(
            reduce_buf_len,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        ),
    )?);

    let output_buf = render_graph.bind_node(Buffer::create(
        &device,
        BufferInfo::new_mappable(input_bytes.len() as _, vk::BufferUsageFlags::STORAGE_BUFFER),
    )?);

    // This implementation uses a hard-coded zero as the first entry in the reduction buffer, but
    // this could be avoided with minor changes to the compute shader code
    render_graph.fill_buffer_region(reduce_buf, 0, 0..size_of::<u32>() as _);

    let excl_sum_group_count = data_len / workgroup_size;
    let reduce_group_count = excl_sum_group_count - 1;

    // We only need to reduce the data if there is more than one workgroup
    if reduce_group_count > 0 {
        render_graph
            .begin_pass("reduce")
            .bind_pipeline(&create_reduce_pipeline(
                &device,
                workgroup_size,
                num_subgroups,
            ))
            .read_descriptor(0, input_buf)
            .write_descriptor(1, reduce_buf)
            .record_compute(move |compute, _| {
                compute.dispatch(reduce_group_count, 1, 1);
            });
    }

    // Run the exclusive sum algorithm
    render_graph
        .begin_pass("exclusive sum")
        .bind_pipeline(&create_exclusive_sum_pipeline(
            &device,
            workgroup_size,
            num_subgroups,
        ))
        .read_descriptor(0, reduce_buf)
        .read_descriptor(1, input_buf)
        .write_descriptor(2, output_buf)
        .record_compute(move |compute, _| {
            compute.dispatch(excl_sum_group_count, 1, 1);
        });

    let reduce_buf = render_graph.unbind_node(reduce_buf);
    let dst_buf = render_graph.unbind_node(output_buf);
    let cmd_buf = render_graph
        .resolve()
        .submit(&mut HashPool::new(&device), 0)?;

    let started = Instant::now();
    cmd_buf.wait_until_executed()?;

    println!("Waited {}μs", (Instant::now() - started).as_micros());

    assert_reduce_data(reduce_buf, &input_data, workgroup_size);
    assert_output_data(dst_buf, &input_data);

    Ok(())
}

fn generate_input_data(data_len: u32) -> Vec<u32> {
    let mut data = Vec::with_capacity(data_len as _);
    for i in 0..data_len {
        data.push(i);
    }

    data
}

fn assert_reduce_data(reduce_buf: Arc<Buffer>, data: &[u32], workgroup_size: u32) {
    let reduce_data: &[u32] = cast_slice(Buffer::mapped_slice(&reduce_buf));

    for workgroup_idx in 1..data.len() / workgroup_size as usize {
        let mut sum = 0;

        for idx in 0..workgroup_size as usize {
            sum += data[idx + (workgroup_idx - 1) * workgroup_size as usize];
        }

        assert_eq!(
            sum, reduce_data[workgroup_idx],
            "workgroup total at {workgroup_idx} not equal"
        );
    }
}

fn assert_output_data(output_buf: Arc<Buffer>, data: &[u32]) {
    let output_data: &[u32] = cast_slice(Buffer::mapped_slice(&output_buf));
    let mut sum = 0;

    for idx in 0..data.len() {
        assert_eq!(sum, output_data[idx], "exclusive sum at {idx} not equal");

        sum += data[idx];
    }
}

fn create_reduce_pipeline(
    device: &Arc<Device>,
    workgroup_size: u32,
    num_subgroups: u32,
) -> Arc<ComputePipeline> {
    macro_rules! compile_comp {
        ($workgroup_size:literal) => {
            inline_spirv!(
                r#"
                #version 460 core
                #extension GL_KHR_shader_subgroup_arithmetic : require

                layout(local_size_x = WORKGROUP_SIZE, local_size_y = 1, local_size_z = 1) in;
                layout(constant_id = 0) const int NUM_SUBGROUPS = 1;
                
                layout(binding = 0) restrict readonly buffer InputBuffer {
                    uint input_buf[];
                };

                layout(binding = 1) restrict writeonly buffer WorkgroupBuffer {
                    uint workgroup_buf[];
                };
                
                shared uint subgroup_buf[NUM_SUBGROUPS];
                
                void main() {
                    uint sum = subgroupAdd(input_buf[gl_GlobalInvocationID.x]);

                    if (subgroupElect()) {
                        subgroup_buf[gl_SubgroupID] = sum;
                    }

                    barrier();

                    if (gl_SubgroupID == 0 && gl_SubgroupInvocationID < NUM_SUBGROUPS) {
                        sum = subgroupAdd(subgroup_buf[gl_SubgroupInvocationID]);

                        if (subgroupElect()) {
                            workgroup_buf[gl_WorkGroupID.x + 1] = sum;
                        }
                    }
                }"#,
                comp,
                vulkan1_2,
                D WORKGROUP_SIZE = $workgroup_size,
        )};
    }

    Arc::new(
        ComputePipeline::create(
            device,
            ComputePipelineInfo::default(),
            Shader::new_compute(match workgroup_size {
                32 => compile_comp!("32").as_slice(),
                64 => compile_comp!("64").as_slice(),
                128 => compile_comp!("128").as_slice(),
                256 => compile_comp!("256").as_slice(),
                512 => compile_comp!("512").as_slice(),
                1024 => compile_comp!("1024").as_slice(),
                _ => unimplemented!(),
            })
            .specialization_info(SpecializationInfo {
                data: num_subgroups.to_ne_bytes().to_vec(),
                map_entries: vec![vk::SpecializationMapEntry {
                    constant_id: 0,
                    offset: 0,
                    size: size_of::<u32>(),
                }],
            }),
        )
        .unwrap(),
    )
}

fn create_exclusive_sum_pipeline(
    device: &Arc<Device>,
    workgroup_size: u32,
    num_subgroups: u32,
) -> Arc<ComputePipeline> {
    macro_rules! compile_comp {
        ($workgroup_size:literal) => {
            inline_spirv!(
                r#"
                #version 460 core
                #extension GL_KHR_shader_subgroup_arithmetic : require
                #extension GL_KHR_shader_subgroup_ballot : require

                layout(local_size_x = WORKGROUP_SIZE, local_size_y = 1, local_size_z = 1) in;
                layout(constant_id = 0) const int NUM_SUBGROUPS = 1;
                
                layout(binding = 0) restrict readonly buffer WorkgroupBuffer {
                    uint workgroup_buf[];
                };

                layout(binding = 1) restrict readonly buffer InputBuffer {
                    uint input_buf[];
                };

                layout(binding = 2) restrict writeonly buffer OutputBuffer {
                    uint output_buf[];
                };
                
                shared uint subgroup_buf[NUM_SUBGROUPS];
                
                void main() {
                    uint subgroup_value = input_buf[gl_GlobalInvocationID.x];
                    uint subgroup_sum = subgroupExclusiveAdd(subgroup_value);

                    if (gl_SubgroupInvocationID == gl_SubgroupSize - 1) {
                        subgroup_buf[gl_SubgroupID] = subgroup_sum + subgroup_value;
                    }

                    barrier();

                    uint workgroup_sum = 0;

                    if (subgroupElect()) {
                        for (uint subgroup_id = 0; subgroup_id < gl_SubgroupID; subgroup_id++) {
                            workgroup_sum += subgroup_buf[subgroup_id];
                        }

                        for (uint workgroup_id = 1; workgroup_id <= gl_WorkGroupID.x; workgroup_id++) {
                            workgroup_sum += workgroup_buf[workgroup_id];
                        }
                    }

                    workgroup_sum = subgroupBroadcastFirst(workgroup_sum);
                    output_buf[gl_GlobalInvocationID.x] = subgroup_sum + workgroup_sum;
                }"#,
                comp,
                vulkan1_2,
                D WORKGROUP_SIZE = $workgroup_size,
        )};
    }

    Arc::new(
        ComputePipeline::create(
            device,
            ComputePipelineInfo::default(),
            Shader::new_compute(match workgroup_size {
                32 => compile_comp!("32").as_slice(),
                64 => compile_comp!("64").as_slice(),
                128 => compile_comp!("128").as_slice(),
                256 => compile_comp!("256").as_slice(),
                512 => compile_comp!("512").as_slice(),
                1024 => compile_comp!("1024").as_slice(),
                _ => unimplemented!(),
            })
            .specialization_info(SpecializationInfo {
                data: num_subgroups.to_ne_bytes().to_vec(),
                map_entries: vec![vk::SpecializationMapEntry {
                    constant_id: 0,
                    offset: 0,
                    size: size_of::<u32>(),
                }],
            }),
        )
        .unwrap(),
    )
}
