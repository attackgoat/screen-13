mod driver;

use {
    self::driver::Instance,
    openxr::{self as xr, EnvironmentBlendMode, ViewConfigurationType},
    screen_13::{
        driver::{
            ash::vk::{self, Handle as _},
            device::Device,
            image::{Image, ImageInfo},
        },
        graph::RenderGraph,
        pool::lazy::LazyPool,
        prelude::trace,
    },
    std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread::sleep,
        time::Duration,
    },
};

fn main() {
    pretty_env_logger::init();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("setting Ctrl-C handler");

    trace!("Starting");

    let instance = Instance::new().unwrap();
    let system = Instance::system(&instance);
    let device = Instance::device(&instance);
    let vk_instance = Device::instance(device);
    let queue_family_index = device
        .physical_device
        .queue_families
        .iter()
        .enumerate()
        .find(|(_, properties)| properties.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .map(|(index, _)| index as u32)
        .unwrap();

    let (session, mut frame_wait, mut frame_stream) = unsafe {
        instance.create_session::<xr::Vulkan>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: vk_instance.handle().as_raw() as _,
                physical_device: device.physical_device.as_raw() as _,
                device: device.handle().as_raw() as _,
                queue_family_index,
                queue_index: 0,
            },
        )
    }
    .unwrap();

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

    let mut swapchain = {
        let views = instance
            .enumerate_view_configuration_views(system, ViewConfigurationType::PRIMARY_STEREO)
            .unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0], views[1]);

        // Create a swapchain for the viewpoints! A swapchain is a set of texture buffers
        // used for displaying to screen, typically this is a backbuffer and a front buffer,
        // one for rendering data to, and one for displaying on-screen.
        let resolution = vk::Extent2D {
            width: views[0].recommended_image_rect_width,
            height: views[0].recommended_image_rect_height,
        };
        let handle = session
            .create_swapchain(&xr::SwapchainCreateInfo {
                create_flags: xr::SwapchainCreateFlags::EMPTY,
                usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                    | xr::SwapchainUsageFlags::SAMPLED,
                format: vk::Format::R8G8B8A8_SRGB.as_raw() as _,
                // The Vulkan graphics pipeline we create is not set up for multisampling,
                // so we hardcode this to 1. If we used a proper multisampling setup, we
                // could set this to `views[0].recommended_swapchain_sample_count`.
                sample_count: 1,
                width: resolution.width,
                height: resolution.height,
                face_count: 1,
                array_size: 2,
                mip_count: 1,
            })
            .unwrap();

        // We'll want to track our own information about the swapchain, so we can draw stuff
        // onto it! We'll also create a buffer for each generated texture here as well.
        let images = handle.enumerate_images().unwrap();
        Swapchain {
            handle,
            resolution,
            images: images
                .into_iter()
                .map(|color_image| {
                    let color_image = vk::Image::from_raw(color_image);

                    Arc::new(Image::from_raw(
                        device,
                        color_image,
                        ImageInfo::new_2d_array(
                            vk::Format::R8G8B8A8_SRGB,
                            resolution.width,
                            resolution.height,
                            2,
                            vk::ImageUsageFlags::SAMPLED,
                        ),
                    ))

                    // let color = vk_device
                    //     .create_image_view(
                    //         &vk::ImageViewCreateInfo::builder()
                    //             .image(color_image)
                    //             .view_type(vk::ImageViewType::TYPE_2D_ARRAY)
                    //             .format(COLOR_FORMAT)
                    //             .subresource_range(vk::ImageSubresourceRange {
                    //                 aspect_mask: vk::ImageAspectFlags::COLOR,
                    //                 base_mip_level: 0,
                    //                 level_count: 1,
                    //                 base_array_layer: 0,
                    //                 layer_count: VIEW_COUNT,
                    //             }),
                    //         None,
                    //     )
                    //     .unwrap();
                    // let framebuffer = vk_device
                    //     .create_framebuffer(
                    //         &vk::FramebufferCreateInfo::builder()
                    //             .render_pass(render_pass)
                    //             .width(resolution.width)
                    //             .height(resolution.height)
                    //             .attachments(&[color])
                    //             .layers(1), // Multiview handles addressing multiple layers
                    //         None,
                    //     )
                    //     .unwrap();
                    // Framebuffer { framebuffer, color }
                })
                .collect(),
        }
    };

    let mut pool = LazyPool::new(device);
    let mut graphs = Vec::with_capacity(swapchain.images.len());
    for _ in 0..swapchain.images.len() {
        graphs.push(None);
    }

    // Main loop
    let mut event_storage = xr::EventDataBuffer::new();
    let mut session_running = false;
    // Index of the current frame, wrapped by PIPELINE_DEPTH. Not to be confused with the
    // swapchain image index.
    // let mut frame = 0;
    'main_loop: loop {
        if !running.load(Ordering::Relaxed) {
            println!("requesting exit");
            // The OpenXR runtime may want to perform a smooth transition between scenes, so we
            // can't necessarily exit instantly. Instead, we must notify the runtime of our
            // intent and wait for it to tell us when we're actually done.
            match session.request_exit() {
                Ok(()) => {}
                // Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => break,
                Err(e) => panic!("{}", e),
            }
        }

        while let Some(event) = instance.poll_event(&mut event_storage).unwrap() {
            use xr::Event::*;
            match event {
                SessionStateChanged(e) => {
                    // Session state change is where we can begin and end sessions, as well as
                    // find quit messages!
                    println!("entered state {:?}", e.state());
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
                    println!("lost {} events", e.lost_event_count());
                }
                _ => {}
            }
        }

        if !session_running {
            // Don't grind up the CPU
            sleep(Duration::from_millis(100));
            continue;
        }

        // Block until the previous frame is finished displaying, and is ready for another one.
        // Also returns a prediction of when the next frame will be displayed, for use with
        // predicting locations of controllers, viewpoints, etc.
        let xr_frame_state = frame_wait.wait().unwrap();
        // Must be called before any rendering is done!
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

        // We need to ask which swapchain image to use for rendering! Which one will we get?
        // Who knows! It's up to the runtime to decide.
        let image_index = swapchain.handle.acquire_image().unwrap();

        // // Ensure the last use of this frame's resources is 100% done
        // vk_device
        //     .wait_for_fences(&[fences[frame]], true, u64::MAX)
        //     .unwrap();
        // vk_device.reset_fences(&[fences[frame]]).unwrap();

        // let cmd = cmds[frame];
        // vk_device
        //     .begin_command_buffer(
        //         cmd,
        //         &vk::CommandBufferBeginInfo::builder()
        //             .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        //     )
        //     .unwrap();
        // vk_device.cmd_begin_render_pass(
        //     cmd,
        //     &vk::RenderPassBeginInfo::builder()
        //         .render_pass(render_pass)
        //         .framebuffer(swapchain.buffers[image_index as usize].framebuffer)
        //         .render_area(vk::Rect2D {
        //             offset: vk::Offset2D::default(),
        //             extent: swapchain.resolution,
        //         })
        //         .clear_values(&[vk::ClearValue {
        //             color: vk::ClearColorValue {
        //                 float32: [0.0, 0.0, 0.0, 1.0],
        //             },
        //         }]),
        //     vk::SubpassContents::INLINE,
        // );

        // let viewports = [vk::Viewport {
        //     x: 0.0,
        //     y: 0.0,
        //     width: swapchain.resolution.width as f32,
        //     height: swapchain.resolution.height as f32,
        //     min_depth: 0.0,
        //     max_depth: 1.0,
        // }];
        // let scissors = [vk::Rect2D {
        //     offset: vk::Offset2D { x: 0, y: 0 },
        //     extent: swapchain.resolution,
        // }];
        // vk_device.cmd_set_viewport(cmd, 0, &viewports);
        // vk_device.cmd_set_scissor(cmd, 0, &scissors);

        // // Draw the scene. Multiview means we only need to do this once, and the GPU will
        // // automatically broadcast operations to all views. Shaders can use `gl_ViewIndex` to
        // // e.g. select the correct view matrix.
        // vk_device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
        // vk_device.cmd_draw(cmd, 3, 1, 0, 0);

        // vk_device.cmd_end_render_pass(cmd);
        // vk_device.end_command_buffer(cmd).unwrap();

        session.sync_actions(&[(&action_set).into()]).unwrap();

        // Find where our controllers are located in the Stage space
        let right_location = right_space
            .locate(&stage, xr_frame_state.predicted_display_time)
            .unwrap();

        let left_location = left_space
            .locate(&stage, xr_frame_state.predicted_display_time)
            .unwrap();

        let mut printed = false;
        if left_action.is_active(&session, xr::Path::NULL).unwrap() {
            print!(
                "Left Hand: ({:0<12},{:0<12},{:0<12}), ",
                left_location.pose.position.x,
                left_location.pose.position.y,
                left_location.pose.position.z
            );
            printed = true;
        }

        if right_action.is_active(&session, xr::Path::NULL).unwrap() {
            print!(
                "Right Hand: ({:0<12},{:0<12},{:0<12})",
                right_location.pose.position.x,
                right_location.pose.position.y,
                right_location.pose.position.z
            );
            printed = true;
        }
        if printed {
            println!();
        }

        // Fetch the view transforms. To minimize latency, we intentionally do this *after*
        // recording commands to render the scene, i.e. at the last possible moment before
        // rendering begins in earnest on the GPU. Uniforms dependent on this data can be sent
        // to the GPU just-in-time by writing them to per-frame host-visible memory which the
        // GPU will only read once the command buffer is submitted.
        let (_, views) = session
            .locate_views(
                ViewConfigurationType::PRIMARY_STEREO,
                xr_frame_state.predicted_display_time,
                &stage,
            )
            .unwrap();

        // Wait until the image is available to render to before beginning work on the GPU. The
        // compositor could still be reading from it.
        swapchain.handle.wait_image(xr::Duration::INFINITE).unwrap();

        // Submit commands to the GPU, then tell OpenXR we're done with our part.
        // vk_device
        //     .queue_submit(
        //         queue,
        //         &[vk::SubmitInfo::builder().command_buffers(&[cmd]).build()],
        //         fences[frame],
        //     )
        //     .unwrap();
        let mut render_graph = RenderGraph::new();
        let swapchain_image = render_graph.bind_node(&swapchain.images[image_index as usize]);
        render_graph.clear_color_image_value(swapchain_image, [0xff,0x00,0xff,0xff]);
        let cmd_buf = render_graph.resolve().submit(&mut pool, queue_family_index as _, 0).unwrap();
        graphs[image_index as usize] =  Some(cmd_buf);

        swapchain.handle.release_image().unwrap();

        // Tell OpenXR what to present for this frame
        let rect = xr::Rect2Di {
            offset: xr::Offset2Di { x: 0, y: 0 },
            extent: xr::Extent2Di {
                width: swapchain.resolution.width as _,
                height: swapchain.resolution.height as _,
            },
        };
        frame_stream
            .end(
                xr_frame_state.predicted_display_time,
                EnvironmentBlendMode::OPAQUE,
                &[
                    &xr::CompositionLayerProjection::new().space(&stage).views(&[
                        xr::CompositionLayerProjectionView::new()
                            .pose(views[0].pose)
                            .fov(views[0].fov)
                            .sub_image(
                                xr::SwapchainSubImage::new()
                                    .swapchain(&swapchain.handle)
                                    .image_array_index(0)
                                    .image_rect(rect),
                            ),
                        xr::CompositionLayerProjectionView::new()
                            .pose(views[1].pose)
                            .fov(views[1].fov)
                            .sub_image(
                                xr::SwapchainSubImage::new()
                                    .swapchain(&swapchain.handle)
                                    .image_array_index(1)
                                    .image_rect(rect),
                            ),
                    ]),
                ],
            )
            .unwrap();
        // frame = (frame + 1) % PIPELINE_DEPTH as usize;
    }

    trace!("OK");
}

struct Swapchain {
    handle: xr::Swapchain<xr::Vulkan>,
    images: Vec<Arc<Image>>,
    resolution: vk::Extent2D,
}
