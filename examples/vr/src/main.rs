mod driver;

use {
    self::driver::{Instance, Swapchain},
    bytemuck::{bytes_of, cast_slice, Pod, Zeroable},
    glam::{vec3, vec4, Mat3, Mat4, Quat, Vec2, Vec3},
    meshopt::{generate_vertex_remap, remap_index_buffer, remap_vertex_buffer},
    openxr::{self as xr, EnvironmentBlendMode, ViewConfigurationType},
    screen_13::{
        driver::{
            ash::vk::{self},
            buffer::{Buffer, BufferInfo},
            device::Device,
            graphic::{DepthStencilMode, GraphicPipelineInfo},
            image::{Image, ImageInfo},
            AccessType,
        },
        graph::RenderGraph,
        pool::{lazy::LazyPool, Pool as _},
        prelude::{debug, error, trace},
    },
    screen_13_hot::{graphic::HotGraphicPipeline, shader::HotShader},
    std::{
        fs::{metadata, File},
        io::BufReader,
        path::{Path, PathBuf},
        ptr::copy_nonoverlapping,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread::sleep,
        time::Duration,
    },
    tobj::{load_obj, GPU_LOAD_OPTIONS},
};

// Sets bits with index 0 and 1 for stereoscopic rendering
const VIEW_MASK: u32 = !(!0 << 2);

fn main() -> anyhow::Result<()> {
    // Run with RUST_LOG=trace to see detailed event logging
    pretty_env_logger::init();

    // Set a CTRL+C handler so that we can exit VR gracefully
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .unwrap_or_default();

    trace!("Starting");

    // Initialize OpenXR and Vulkan
    let mut instance = Instance::new().unwrap();
    let device = Instance::device(&instance);
    let queue_family_index =
        device_queue_family_index(device, vk::QueueFlags::GRAPHICS | vk::QueueFlags::TRANSFER)
            .unwrap();

    // Start a VR session
    let (session, mut frame_wait, mut frame_stream) =
        Instance::create_session(&instance, queue_family_index, 0).unwrap();
    let action_set = instance
        .create_action_set("input", "input pose information", 0)
        .unwrap();
    let right_action = action_set
        .create_action::<xr::Posef>("right_hand", "Right Hand Controller", &[])
        .unwrap();
    let left_action = action_set
        .create_action::<xr::Posef>("left_hand", "Left Hand Controller", &[])
        .unwrap();
    instance
        .suggest_interaction_profile_bindings(
            instance
                .string_to_path("/interaction_profiles/khr/simple_controller")
                .unwrap(),
            &[
                xr::Binding::new(
                    &right_action,
                    instance
                        .string_to_path("/user/hand/right/input/grip/pose")
                        .unwrap(),
                ),
                xr::Binding::new(
                    &left_action,
                    instance
                        .string_to_path("/user/hand/left/input/grip/pose")
                        .unwrap(),
                ),
            ],
        )
        .unwrap();
    session.attach_action_sets(&[&action_set]).unwrap();

    let right_space = right_action
        .create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)
        .unwrap();
    let left_space = left_action
        .create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)
        .unwrap();
    let stage = session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)
        .unwrap();

    let mut swapchain = Swapchain::new(&instance, &session);
    let resolution = Swapchain::resolution(&swapchain);
    let rect = xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: resolution.width as _,
            height: resolution.height as _,
        },
    };

    let mut pool = LazyPool::new(device);
    let mut graphs = Vec::with_capacity(Swapchain::images(&swapchain).len());
    for _ in 0..graphs.capacity() {
        graphs.push(None);
    }

    let res_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("res");

    let mut hands_pipeline = HotGraphicPipeline::create(
        device,
        GraphicPipelineInfo::new(),
        [
            HotShader::new_vertex(res_dir.join("model.vert")),
            HotShader::new_fragment(res_dir.join("hands.frag")),
        ],
    )?;
    let mut mammoth_pipeline = HotGraphicPipeline::create(
        device,
        GraphicPipelineInfo::new(),
        [
            HotShader::new_vertex(res_dir.join("model.vert")),
            HotShader::new_fragment(res_dir.join("mammoth.frag")),
        ],
    )?;

    let example_assets_dir = res_dir.join("example-assets");
    let lincoln_hands_dir = example_assets_dir.join("lincoln-hands");
    let woolly_mammoth_dir = example_assets_dir.join("woolly-mammoth");

    if metadata(&lincoln_hands_dir).is_err() {
        panic!("Asset submodule missing! You must first initialize the submodules and then update them using:\ngit submodule init\ngit submodule update");
    }

    // Load a model and textures for the left hand
    let lincoln_hand_left = load_model(
        device,
        lincoln_hands_dir.join("npg_71_6_left-hires_unwrapped-150k-unwrapped.obj"),
    )?;
    let lincoln_hand_left_diffuse = load_texture(
        device,
        lincoln_hands_dir.join("npg_71_6_left-hires_unwrapped-150k-4096-diffuse.jpg"),
        vk::Format::R8G8B8A8_SRGB,
    )?;
    let lincoln_hand_left_normal = load_texture(
        device,
        lincoln_hands_dir.join("npg_71_6_left-hires_unwrapped-150k-4096-normals.jpg"),
        vk::Format::R8G8B8A8_UNORM,
    )?;
    let lincoln_hand_left_occlusion = load_texture(
        device,
        lincoln_hands_dir.join("npg_71_6_left-hires_unwrapped-150k-4096-occlusion.jpg"),
        vk::Format::R8G8B8A8_UNORM,
    )?;

    // Load a model and textures for the right hand
    let lincoln_hand_right = load_model(
        device,
        lincoln_hands_dir.join("npg_71_6_right-hires_unwrapped-150k-unwrapped.obj"),
    )?;
    let lincoln_hand_right_diffuse = load_texture(
        device,
        lincoln_hands_dir.join("npg_71_6_right-hires_unwrapped-150k-4096-diffuse.jpg"),
        vk::Format::R8G8B8A8_SRGB,
    )?;
    let lincoln_hand_right_normal = load_texture(
        device,
        lincoln_hands_dir.join("npg_71_6_right-hires_unwrapped-150k-4096-normals.jpg"),
        vk::Format::R8G8B8A8_UNORM,
    )?;
    let lincoln_hand_right_occlusion = load_texture(
        device,
        lincoln_hands_dir.join("npg_71_6_right-hires_unwrapped-150k-4096-occlusion.jpg"),
        vk::Format::R8G8B8A8_UNORM,
    )?;

    // Load a model and textures for the woolly mammoth exhibit
    let woolly_mammoth =
        load_model(device, woolly_mammoth_dir.join("woolly-mammoth-150k.obj")).unwrap();
    let woolly_mammoth_normal = load_texture(
        device,
        woolly_mammoth_dir.join("woolly-mammoth-100k-4096-normals.jpg"),
        vk::Format::R8G8B8A8_UNORM,
    )?;
    let woolly_mammoth_occlusion = load_texture(
        device,
        woolly_mammoth_dir.join("woolly-mammoth-100k-4096-occlusion.jpg"),
        vk::Format::R8G8B8A8_UNORM,
    )?;

    let mut session_running = false;
    'main_loop: loop {
        if !running.load(Ordering::Relaxed) {
            println!("requesting exit");
            // The OpenXR runtime may want to perform a smooth transition between scenes, so we
            // can't necessarily exit instantly. Instead, we must notify the runtime of our
            // intent and wait for it to tell us when we're actually done.
            match session.request_exit() {
                Ok(()) => {}
                Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => break,
                Err(e) => panic!("{}", e),
            }
        }

        while let Some(event) = Instance::poll_event(&mut instance).unwrap() {
            use xr::Event::*;
            match event {
                SessionStateChanged(e) => {
                    debug!("entered state {:?}", e.state());
                    match e.state() {
                        xr::SessionState::READY => {
                            session
                                .begin(ViewConfigurationType::PRIMARY_STEREO)
                                .unwrap();
                            session_running = true;
                        }
                        xr::SessionState::STOPPING => {
                            session.end().unwrap();
                            session_running = false;
                        }
                        xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                            break 'main_loop;
                        }
                        _ => {}
                    }
                }
                InstanceLossPending(_) => {
                    break 'main_loop;
                }
                EventsLost(e) => {
                    error!("lost {} events", e.lost_event_count());
                }
                _ => {}
            }
        }

        if !session_running {
            sleep(Duration::from_millis(100));
            continue;
        }

        let xr_frame_state = frame_wait.wait().unwrap();
        frame_stream.begin().unwrap();

        if !xr_frame_state.should_render {
            frame_stream
                .end(
                    xr_frame_state.predicted_display_time,
                    EnvironmentBlendMode::OPAQUE,
                    &[],
                )
                .unwrap();
            continue;
        }

        let mut render_graph = RenderGraph::new();
        let depth_image = render_graph.bind_node(
            pool.lease(ImageInfo::new_2d_array(
                vk::Format::D32_SFLOAT,
                resolution.width,
                resolution.height,
                2,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            ))
            .unwrap(),
        );

        let swapchain_image_index = swapchain.acquire_image().unwrap();
        let swapchain_image = Swapchain::image(&swapchain, swapchain_image_index as _);
        let swapchain_image = render_graph.bind_node(swapchain_image);

        // Get the XR views and copy them into a leased uniform buffer
        let (_, views) = session.locate_views(
            ViewConfigurationType::PRIMARY_STEREO,
            xr_frame_state.predicted_display_time,
            &stage,
        )?;
        let camera_buf = {
            let cameras = [CameraBuffer::new(views[0]), CameraBuffer::new(views[1])];
            let data = cast_slice(&cameras);
            let mut buf = pool.lease(BufferInfo::new_mappable(
                data.len() as _,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
            ))?;
            Buffer::copy_from_slice(&mut buf, 0, data);
            render_graph.bind_node(buf)
        };

        session.sync_actions(&[(&action_set).into()]).unwrap();

        let left_hand_location = left_action
            .is_active(&session, xr::Path::NULL)
            .ok()
            .and_then(|active| {
                active.then(|| {
                    left_space
                        .locate(&stage, xr_frame_state.predicted_display_time)
                        .ok()
                })
            })
            .flatten();
        let right_hand_location = right_action
            .is_active(&session, xr::Path::NULL)
            .ok()
            .and_then(|active| {
                active.then(|| {
                    right_space
                        .locate(&stage, xr_frame_state.predicted_display_time)
                        .ok()
                })
            })
            .flatten();
        let light_position = right_hand_location
            .map(|location| pose_position(location.pose))
            .unwrap_or(vec3(0.0, 10.0, 0.0));

        let light_buf = {
            let light = LightBuffer::new(light_position);
            let data = bytes_of(&light);
            let mut buf = pool.lease(BufferInfo::new_mappable(
                data.len() as _,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
            ))?;
            Buffer::copy_from_slice(&mut buf, 0, data);
            render_graph.bind_node(buf)
        };

        render_graph.clear_color_image_value(swapchain_image, [0x00, 0x00, 0x00, 0xff]);

        if let Some(location) = left_hand_location {
            let index_buf = render_graph.bind_node(&lincoln_hand_left.index_buf);
            let vertex_buf = render_graph.bind_node(&lincoln_hand_left.vertex_buf);
            let diffuse_texture = render_graph.bind_node(&lincoln_hand_left_diffuse);
            let normal_texture = render_graph.bind_node(&lincoln_hand_left_normal);
            let occlusion_texture = render_graph.bind_node(&lincoln_hand_left_occlusion);
            let model_transform = pose_transform(location.pose);
            let push_consts = PushConstants::new(model_transform);

            render_graph
                .begin_pass("Left hand")
                .bind_pipeline(hands_pipeline.hot())
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .set_multiview(VIEW_MASK, VIEW_MASK)
                .store_color(0, swapchain_image)
                .clear_depth_stencil(depth_image)
                .access_node(index_buf, AccessType::IndexBuffer)
                .access_node(vertex_buf, AccessType::VertexBuffer)
                .access_descriptor(0, camera_buf, AccessType::VertexShaderReadUniformBuffer)
                .access_descriptor(1, light_buf, AccessType::VertexShaderReadUniformBuffer)
                .access_descriptor(
                    2,
                    diffuse_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .access_descriptor(
                    3,
                    normal_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .access_descriptor(
                    4,
                    occlusion_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(vertex_buf)
                        .push_constants(bytes_of(&push_consts))
                        .draw_indexed(lincoln_hand_left.index_count, 1, 0, 0, 0);
                });
        }

        if let Some(location) = right_hand_location {
            let index_buf = render_graph.bind_node(&lincoln_hand_right.index_buf);
            let vertex_buf = render_graph.bind_node(&lincoln_hand_right.vertex_buf);
            let diffuse_texture = render_graph.bind_node(&lincoln_hand_right_diffuse);
            let normal_texture = render_graph.bind_node(&lincoln_hand_right_normal);
            let occlusion_texture = render_graph.bind_node(&lincoln_hand_right_occlusion);
            let model_transform = pose_transform(location.pose);
            let push_consts = PushConstants::new(model_transform);

            render_graph
                .begin_pass("Right hand")
                .bind_pipeline(hands_pipeline.hot())
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .set_multiview(VIEW_MASK, VIEW_MASK)
                .store_color(0, swapchain_image)
                .clear_depth_stencil(depth_image)
                .access_node(index_buf, AccessType::IndexBuffer)
                .access_node(vertex_buf, AccessType::VertexBuffer)
                .access_descriptor(0, camera_buf, AccessType::VertexShaderReadUniformBuffer)
                .access_descriptor(1, light_buf, AccessType::VertexShaderReadUniformBuffer)
                .access_descriptor(
                    2,
                    diffuse_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .access_descriptor(
                    3,
                    normal_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .access_descriptor(
                    4,
                    occlusion_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(vertex_buf)
                        .push_constants(bytes_of(&push_consts))
                        .draw_indexed(lincoln_hand_right.index_count, 1, 0, 0, 0);
                });
        }

        {
            let index_buf = render_graph.bind_node(&woolly_mammoth.index_buf);
            let vertex_buf = render_graph.bind_node(&woolly_mammoth.vertex_buf);
            let normal_texture = render_graph.bind_node(&woolly_mammoth_normal);
            let occlusion_texture = render_graph.bind_node(&woolly_mammoth_occlusion);
            let push_consts = PushConstants::new(Mat4::IDENTITY);

            render_graph
                .begin_pass("Woolly Mammoth")
                .bind_pipeline(mammoth_pipeline.hot())
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .set_multiview(VIEW_MASK, VIEW_MASK)
                .store_color(0, swapchain_image)
                .clear_depth_stencil(depth_image)
                .access_node(index_buf, AccessType::IndexBuffer)
                .access_node(vertex_buf, AccessType::VertexBuffer)
                .access_descriptor(0, camera_buf, AccessType::VertexShaderReadUniformBuffer)
                .access_descriptor(1, light_buf, AccessType::VertexShaderReadUniformBuffer)
                .access_descriptor(
                    2,
                    normal_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .access_descriptor(
                    3,
                    occlusion_texture,
                    AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer,
                )
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(vertex_buf)
                        .push_constants(bytes_of(&push_consts))
                        .draw_indexed(lincoln_hand_right.index_count, 1, 0, 0, 0);
                });
        }

        // Wait on the acquired swapchain image to be ready, submit rendering commands, and release
        // the image - afterwards we keep the submitted command buffer around (including all
        // in-flight resources) so that nothing is dropped until that image is actually done.
        swapchain.wait_image(xr::Duration::INFINITE).unwrap();
        let cmd_buf = render_graph
            .resolve()
            .submit(&mut pool, queue_family_index as _, 0)
            .unwrap();
        swapchain.release_image().unwrap();
        graphs[swapchain_image_index as usize] = Some(cmd_buf);

        frame_stream.end(
            xr_frame_state.predicted_display_time,
            EnvironmentBlendMode::OPAQUE,
            &[
                &xr::CompositionLayerProjection::new().space(&stage).views(&[
                    xr::CompositionLayerProjectionView::new()
                        .pose(views[0].pose)
                        .fov(views[0].fov)
                        .sub_image(
                            xr::SwapchainSubImage::new()
                                .swapchain(&swapchain)
                                .image_array_index(0)
                                .image_rect(rect),
                        ),
                    xr::CompositionLayerProjectionView::new()
                        .pose(views[1].pose)
                        .fov(views[1].fov)
                        .sub_image(
                            xr::SwapchainSubImage::new()
                                .swapchain(&swapchain)
                                .image_array_index(1)
                                .image_rect(rect),
                        ),
                ]),
            ],
        )?;
    }

    trace!("OK");

    Ok(())
}

fn arbitrary_perspective_rh(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    debug_assert!(left <= right);
    debug_assert!(bottom <= top);
    debug_assert!(near <= far);

    let (left, right, bottom, top) = (
        left.tan() * near,
        right.tan() * near,
        bottom.tan() * near,
        top.tan() * near,
    );
    Mat4::from_cols(
        vec4((2.0 * near) / (right - left), 0.0, 0.0, 0.0),
        vec4(0.0, (2.0 * near) / (top - bottom), 0.0, 0.0),
        vec4(
            (right + left) / (right - left),
            (top + bottom) / (top - bottom),
            -(far + near) / (far - near),
            -1.0,
        ),
        vec4(0.0, 0.0, (-2.0 * far * near) / (far - near), 0.0),
    )
}

/// Helper to pick a queue family for submitting device commands.
fn device_queue_family_index(device: &Device, flags: vk::QueueFlags) -> Option<u32> {
    device
        .physical_device
        .queue_families
        .iter()
        .enumerate()
        .find(|(_, properties)| properties.queue_flags.contains(flags))
        .map(|(index, _)| index as u32)
}

/// Loads a .obj model from disk, reading position, normal and UV data.
///
/// Tangent (and bitangent) data is calculated and the whole thing is re-indexed using meshopt.
fn load_model(device: &Arc<Device>, path: impl AsRef<Path>) -> anyhow::Result<Model> {
    trace!("Loading model {}", path.as_ref().display());

    let (mut models, _) = load_obj(path.as_ref(), &GPU_LOAD_OPTIONS)?;
    let model = models.pop().unwrap();
    let tri_count = model.mesh.indices.len() / 3;
    let mut vertices = Vec::with_capacity(tri_count * 3);

    for tri_idx in 0..tri_count {
        let base_idx = 3 * tri_idx;

        let a_idx = 3 * model.mesh.indices[base_idx] as usize;
        let b_idx = 3 * model.mesh.indices[base_idx + 1] as usize;
        let c_idx = 3 * model.mesh.indices[base_idx + 2] as usize;
        let a_position = Vec3::from_slice(&model.mesh.positions[a_idx..a_idx + 3]);
        let b_position = Vec3::from_slice(&model.mesh.positions[b_idx..b_idx + 3]);
        let c_position = Vec3::from_slice(&model.mesh.positions[c_idx..c_idx + 3]);
        let a_normal = Vec3::from_slice(&model.mesh.normals[a_idx..a_idx + 3]);
        let b_normal = Vec3::from_slice(&model.mesh.normals[b_idx..b_idx + 3]);
        let c_normal = Vec3::from_slice(&model.mesh.normals[c_idx..c_idx + 3]);

        let a_idx = 2 * model.mesh.indices[base_idx] as usize;
        let b_idx = 2 * model.mesh.indices[base_idx + 1] as usize;
        let c_idx = 2 * model.mesh.indices[base_idx + 2] as usize;
        let a_texcoord = Vec2::from_slice(&model.mesh.texcoords[a_idx..a_idx + 2]);
        let b_texcoord = Vec2::from_slice(&model.mesh.texcoords[b_idx..b_idx + 2]);
        let c_texcoord = Vec2::from_slice(&model.mesh.texcoords[c_idx..c_idx + 2]);

        vertices.push([
            -a_position.x,
            a_position.y,
            a_position.z,
            0.0,
            0.0,
            0.0,
            0.0,
            -a_normal.x,
            a_normal.y,
            a_normal.z,
            a_texcoord.x,
            a_texcoord.y,
        ]);
        vertices.push([
            -b_position.x,
            b_position.y,
            b_position.z,
            0.0,
            0.0,
            0.0,
            0.0,
            -b_normal.x,
            b_normal.y,
            b_normal.z,
            b_texcoord.x,
            b_texcoord.y,
        ]);
        vertices.push([
            -c_position.x,
            c_position.y,
            c_position.z,
            0.0,
            0.0,
            0.0,
            0.0,
            -c_normal.x,
            c_normal.y,
            c_normal.z,
            c_texcoord.x,
            c_texcoord.y,
        ]);
    }

    // Note: Mesh, Face, and the mikktspace implementation are all for tangent/bitangent calculation
    // which is used to properly light the models using lighting/normal mapping techniques

    struct Mesh(Vec<[f32; 12]>);

    trait Face {
        fn vertex(&self, face: usize, vert: usize) -> &[f32];

        fn vertex_mut(&mut self, face: usize, vert: usize) -> &mut [f32];
    }

    impl Face for Mesh {
        fn vertex(&self, face: usize, vert: usize) -> &[f32] {
            &self.0[face * 3 + vert]
        }

        fn vertex_mut(&mut self, face: usize, vert: usize) -> &mut [f32] {
            &mut self.0[face * 3 + vert]
        }
    }

    impl mikktspace::Geometry for Mesh {
        fn num_faces(&self) -> usize {
            self.0.len() / 3
        }

        fn num_vertices_of_face(&self, _face: usize) -> usize {
            3
        }

        fn position(&self, face: usize, vert: usize) -> [f32; 3] {
            let mut res = [0.0; 3];
            res.copy_from_slice(&self.vertex(face, vert)[0..3]);

            res
        }

        fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
            let mut res = [0.0; 3];
            res.copy_from_slice(&self.vertex(face, vert)[7..10]);

            res
        }

        fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
            let mut res = [0.0; 2];
            res.copy_from_slice(&self.vertex(face, vert)[10..12]);

            res
        }

        fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
            self.vertex_mut(face, vert)[3..7].copy_from_slice(&tangent);
        }
    }

    let mut mesh = Mesh(vertices);
    assert!(mikktspace::generate_tangents(&mut mesh));
    let vertices = mesh.0;

    // Re-index and de-dupe the model vertices using meshopt
    let indices = (0u32..vertices.len() as u32).collect::<Vec<_>>();
    let (vertex_count, remap) = generate_vertex_remap(&vertices, Some(&indices));
    let indices = remap_index_buffer(Some(&indices), vertex_count, &remap);
    let vertices = remap_vertex_buffer(&vertices, vertex_count, &remap);

    debug!("Index count: {}", indices.len());
    debug!("Vertex count: {}", vertices.len());

    let index_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::INDEX_BUFFER,
        cast_slice(&indices),
    )?);
    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(&vertices),
    )?);

    Ok(Model {
        index_buf,
        index_count: indices.len() as _,
        vertex_buf,
    })
}

/// Loads a texture from disk and returns the image after waiting for GPU operations to complete.
fn load_texture(
    device: &Arc<Device>,
    path: impl AsRef<Path>,
    fmt: vk::Format,
) -> anyhow::Result<Arc<Image>> {
    trace!("Loading texture {}", path.as_ref().display());

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let image = image::load(reader, image::ImageFormat::Jpeg)?;
    let image = image.into_rgba8();
    let image_rows = image.height() as isize;
    let image_row_size = 4 * image.width() as isize;

    let staging_buf_size = image_rows * image_row_size;
    let staging_buf_info =
        BufferInfo::new_mappable(staging_buf_size as _, vk::BufferUsageFlags::TRANSFER_SRC);
    let mut staging_buf = Buffer::create(device, staging_buf_info)?;
    let staging_data = Buffer::mapped_slice_mut(&mut staging_buf);

    // Copy the rows of the image over but flipped to the correct orientation (bottom up)
    for row in 0..image_rows {
        unsafe {
            copy_nonoverlapping(
                image
                    .as_ptr()
                    .offset(image_rows * image_row_size - row * image_row_size - image_row_size),
                staging_data.as_mut_ptr().offset(row * image_row_size),
                image_row_size as _,
            );
        }
    }

    let texture = Arc::new(Image::create(
        device,
        ImageInfo::new_2d(
            fmt,
            image.width(),
            image.height(),
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        ),
    )?);

    let mut render_graph = RenderGraph::new();
    let staging_buf = render_graph.bind_node(staging_buf);
    let texture_image = render_graph.bind_node(&texture);
    render_graph.copy_buffer_to_image(staging_buf, texture_image);

    let queue_family_index = device_queue_family_index(device, vk::QueueFlags::TRANSFER).unwrap();
    render_graph
        .resolve()
        .submit(&mut LazyPool::new(device), queue_family_index as _, 0)?
        .wait_until_executed()?;

    Ok(texture)
}

fn pose_position(pose: xr::Posef) -> Vec3 {
    Vec3::from(mint::Vector3::from(pose.position))
}

fn pose_transform(pose: xr::Posef) -> Mat4 {
    let position = pose_position(pose);
    let orientation = Quat::from(mint::Quaternion::from(pose.orientation));

    Mat4::from_translation(-position)
        * Mat4::from_quat(orientation)
        * Mat4::from_scale(Vec3::splat(0.1))
        * Mat4::from_translation(Vec3::splat(-0.5))
}

fn projection_transform(view: xr::View) -> Mat4 {
    arbitrary_perspective_rh(
        view.fov.angle_left,
        view.fov.angle_right,
        view.fov.angle_down,
        view.fov.angle_up,
        0.01,
        100.0,
    )
}

fn view_position(view: xr::View) -> Vec3 {
    Vec3::from(mint::Vector3::from(view.pose.position))
}

fn view_transform(view: xr::View) -> Mat4 {
    let orientation = Quat::from(mint::Quaternion::from(view.pose.orientation));
    let basis = Mat3::from_quat(orientation);
    let (dir, up) = (basis.z_axis, basis.y_axis);
    let eye = view_position(view);

    Mat4::look_to_rh(-eye, dir, up)
}

#[derive(Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
struct CameraBuffer {
    projection: Mat4,
    view: Mat4,
    position: Vec3,
    _pad: f32,
}

impl CameraBuffer {
    fn new(view: xr::View) -> Self {
        Self {
            projection: projection_transform(view),
            view: view_transform(view),
            position: view_position(view),
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
struct LightBuffer {
    light_position: Vec3,
    _pad: f32,
}

impl LightBuffer {
    fn new(light_position: Vec3) -> Self {
        Self {
            light_position,
            ..Default::default()
        }
    }
}

struct Model {
    index_buf: Arc<Buffer>,
    index_count: u32,
    vertex_buf: Arc<Buffer>,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct PushConstants {
    model_transform: Mat4,
    model_inv_transpose_transform: Mat4,
}

impl PushConstants {
    fn new(model_transform: Mat4) -> Self {
        Self {
            model_transform,
            model_inv_transpose_transform: model_transform.inverse().transpose(),
        }
    }
}
