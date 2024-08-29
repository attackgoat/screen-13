mod profile_with_puffin;

use {
    bmfont::{BMFont, OrdinateOrientation},
    image::ImageReader,
    log::info,
    screen_13::prelude::*,
    screen_13_fx::BitmapFont,
    screen_13_window::Window,
    std::{
        collections::VecDeque,
        io::Cursor,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::channel,
            Arc,
        },
        thread::{available_parallelism, sleep, spawn},
        time::{Duration, Instant},
    },
};

const COLOR_SUBRESOURCE_LAYER: vk::ImageSubresourceLayers = vk::ImageSubresourceLayers {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    mip_level: 0,
    base_array_layer: 0,
    layer_count: 1,
};

// Demonstrates submitting work on multiple hardware queues (of the same family) from multiple
// threads
fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    profile_with_puffin::init();

    let started_at = Instant::now();

    // For this example we don't use V-Sync so that we are able to submit work as often as possible
    let event_loop = Window::builder().v_sync(false).build()?;

    // We want to create one hardware queue for each CPU, or at least two
    let desired_queue_count = available_parallelism()
        .map(|res| res.get() as u32)
        .unwrap_or_default()
        .min(8);

    let secondary_queue_family = event_loop
        .device
        .physical_device
        .queue_families
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, queue_family_properties)| {
            queue_family_properties
                .queue_flags
                .contains(vk::QueueFlags::COMPUTE)
                || queue_family_properties
                    .queue_flags
                    .contains(vk::QueueFlags::GRAPHICS)
        });

    assert!(
        secondary_queue_family.is_some(),
        "GPU does not support secondary queue family"
    );

    let (secondary_queue_family_index, secondary_queue_family_properties) =
        secondary_queue_family.unwrap();
    let queue_count =
        desired_queue_count.min(secondary_queue_family_properties.queue_count) as usize;

    assert!(queue_count > 0, "GPU does not support secondary queues");

    info!("Using {queue_count} queues");

    let running = Arc::new(AtomicBool::new(true));
    let thread_count = queue_count;
    let mut threads = Vec::with_capacity(thread_count);
    let (tx, rx) = channel();

    info!("Launching {thread_count} threads");

    for thread_index in 0..thread_count {
        let running = Arc::clone(&running);
        let device = Arc::clone(&event_loop.device);
        let tx = tx.clone();
        threads.push(spawn(move || {
            let queue_index = thread_index;
            let mut pool = HashPool::new(&device);

            while running.load(Ordering::Relaxed) {
                // Fake some I/O time by sleeping
                sleep(Duration::from_millis(16));

                let t = 12.0 * ((Instant::now() - started_at).as_millis() % 32) as f32;

                // Clear a new image to a cycling color
                let mut render_graph = RenderGraph::new();
                let image = render_graph.bind_node(
                    pool.lease(ImageInfo::image_2d(
                        10,
                        10,
                        vk::Format::R8G8B8A8_UNORM,
                        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC,
                    ))
                    .unwrap(),
                );
                render_graph.clear_color_image_value(
                    image,
                    [
                        (t.sin() * 127.0 + 128.0) as u8,
                        ((t + 2.0).sin() * 127.0 + 128.0) as u8,
                        ((t + 4.0).sin() * 127.0 + 128.0) as u8,
                        0xff,
                    ],
                );

                let image = render_graph.unbind_node(image);

                // Submit on a queue we are reserving for only this thread to use
                render_graph
                    .resolve()
                    .submit(&mut pool, secondary_queue_family_index, queue_index)
                    .unwrap();

                // After submit() is called we can safely use this image on another thread!
                tx.send(image).unwrap();
            }
        }));
    }

    let mut font = load_font(&event_loop.device)?;
    let mut images = VecDeque::new();

    let mut previous_frame = Instant::now();
    event_loop.run(|frame| {
        let current_frame = Instant::now();
        let elapsed = current_frame - previous_frame;
        previous_frame = current_frame;

        if let Ok(image) = rx.recv_timeout(Duration::from_nanos(1)) {
            images.push_front(image);

            while images.len() > 64 {
                images.pop_back();
            }
        }

        frame.render_graph.clear_color_image(frame.swapchain_image);

        for (image_idx, image) in images.iter().enumerate() {
            let image = frame.render_graph.bind_node(image);

            let x = (image_idx % 8) as f32;
            let y = (image_idx / 8) as f32;

            let j = frame.width as f32 / 10.0;
            let k = frame.height as f32 / 10.0;

            frame.render_graph.blit_image_region(
                image,
                frame.swapchain_image,
                vk::Filter::NEAREST,
                vk::ImageBlit {
                    src_subresource: COLOR_SUBRESOURCE_LAYER,
                    src_offsets: [
                        vk::Offset3D { x: 0, y: 0, z: 0 },
                        vk::Offset3D { x: 10, y: 10, z: 1 },
                    ],
                    dst_subresource: COLOR_SUBRESOURCE_LAYER,
                    dst_offsets: [
                        vk::Offset3D {
                            x: ((x * j) + j) as i32,
                            y: ((y * k) + k) as i32,
                            z: 0,
                        },
                        vk::Offset3D {
                            x: ((x * j) + (2.0 * j)) as i32,
                            y: ((y * k) + (2.0 * k)) as i32,
                            z: 1,
                        },
                    ],
                },
            );
        }

        let fps = (1.0 / elapsed.as_secs_f32()).round();
        let message = format!("FPS: {fps}");
        font.print_scale(
            frame.render_graph,
            frame.swapchain_image,
            0.0,
            0.0,
            [0xff, 0xff, 0xff],
            message,
            4.0,
        );
    })?;

    info!("Stopping threads");

    running.store(false, Ordering::Relaxed);
    for thread in threads.drain(..) {
        thread.join().unwrap();
    }

    Ok(())
}

fn load_font(device: &Arc<Device>) -> anyhow::Result<BitmapFont> {
    // Load the font definition file using the bmfont crate
    let font = BMFont::new(
        Cursor::new(include_bytes!("res/font/small/small_10px.fnt")),
        OrdinateOrientation::TopToBottom,
    )?;

    // We happen to know this font only requires a single image, this uses the image crate
    let temp_buf = Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::TRANSFER_SRC,
        ImageReader::new(Cursor::new(
            include_bytes!("res/font/small/small_10px_0.png").as_slice(),
        ))
        .with_guessed_format()?
        .decode()?
        .into_rgba8()
        .to_vec()
        .as_slice(),
    )?;

    // This image will hold the font glyphs
    let page_0 = Image::create(
        device,
        ImageInfo::image_2d(
            64,
            64,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        ),
    )
    .unwrap();

    let mut render_graph = RenderGraph::new();
    let page_0 = render_graph.bind_node(page_0);
    let temp_buf = render_graph.bind_node(temp_buf);
    render_graph.copy_buffer_to_image(temp_buf, page_0);

    // Unbind page_0 to get the Arc<Image> but we could have just bound a reference (with no unbind)
    let page_0 = render_graph.unbind_node(page_0);

    // This copy happens in queue index 0! Notice the unbind above is OK because we already asked
    // for the copy to happen first!
    render_graph
        .resolve()
        .submit(&mut HashPool::new(device), 0, 0)?;

    BitmapFont::new(device, font, [page_0])
}
