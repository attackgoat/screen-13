use {
    criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion},
    image::{ImageBuffer, ImageFormat, RgbImage},
    screen_13::{
        bake::{asset::Bitmap, bake_bitmap},
        prelude_rc::*,
    },
    std::{
        collections::HashMap,
        env::temp_dir,
        fs::{read, File},
        io::{BufWriter, Cursor},
    },
};

/// 💀 Extremely unsafe - You literally get back noise I'm not sure what you'd even want that for.
fn noise(dims: Extent, fmt: BitmapFormat) -> Vec<u8> {
    let mut res = Vec::with_capacity(dims.x as usize * dims.y as usize * fmt.byte_len());
    unsafe {
        res.set_len(res.capacity());
    }

    res
}

fn load_bitmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("Load bitmap");
    for (idx, criteria) in [
        Extent::new(1, 1),
        Extent::new(2, 2),
        Extent::new(4, 4),
        Extent::new(8, 8),
        Extent::new(16, 16),
        Extent::new(32, 32),
        Extent::new(64, 64),
        Extent::new(128, 128),
        Extent::new(256, 256),
        Extent::new(512, 512),
        Extent::new(1024, 1024),
        Extent::new(2048, 2048),
        Extent::new(4096, 4096),
        Extent::new(8192, 8192),
        Extent::new(16384, 16384),
    ]
    .iter()
    .enumerate()
    {
        let gpu = Gpu::offscreen();
        let bitmap = noise(*criteria, BitmapFormat::Rgb);

        group.bench_with_input(
            BenchmarkId::new(
                "Power of two dimensions",
                format!("2 ^ {:0>#2}: {}x{}", idx, criteria.x, criteria.y),
            ),
            criteria,
            |b, criteria| {
                b.iter(|| {
                    gpu.load_bitmap(
                        black_box(BitmapFormat::Rgb),
                        black_box(criteria.x as u16),
                        black_box(bitmap.clone()),
                    )
                })
            },
        );
    }
}

fn read_bitmap(c: &mut Criterion) {
    // let mut group = c.benchmark_group("Read bitmap");
    // for (idx, criteria) in [
    //     Extent::new(1, 1),
    //     Extent::new(2, 2),
    //     Extent::new(4, 4),
    //     Extent::new(8, 8),
    //     Extent::new(16, 16),
    //     Extent::new(32, 32),
    //     Extent::new(64, 64),
    //     Extent::new(128, 128),
    //     Extent::new(256, 256),
    //     Extent::new(512, 512),
    //     Extent::new(1024, 1024),
    //     Extent::new(2048, 2048),
    //     Extent::new(4096, 4096),
    //     Extent::new(8192, 8192),
    //     Extent::new(16384, 16384),
    // ]
    // .iter()
    // .enumerate()
    // {
    //     // Some pre-work to get a .pak file ready for this criteria
    //     {
    //         // Create a new image
    //         let bitmap: RgbImage = ImageBuffer::new(criteria.x, criteria.y);
    //         let bitmap_filename = format!("screen-13-bench-read-bitmap-{}.bmp", idx);
    //         let bitmap_path = temp_dir().join(&bitmap_filename);
    //         bitmap
    //             .save_with_format(&bitmap_path, ImageFormat::Bmp)
    //             .unwrap();

    //         // Bake the image into a .pak (no compression in this bench)
    //         let asset = Bitmap::new(bitmap_filename);
    //         let asset_filename = temp_dir().join("my-bitmap.toml");
    //         let mut pak = PakBuf::default();
    //         let pak_filename = temp_dir().join(format!("screen-13-bench-read-bitmap-{}.pak", idx));
    //         let mut context = Default::default();
    //         bake_bitmap(&mut context, temp_dir(), asset_filename, &asset, &mut pak);
    //         pak.write(
    //             &mut BufWriter::new(File::create(&pak_filename).unwrap()),
    //             None,
    //         )
    //         .unwrap();
    //     }

    //     let gpu = Gpu::offscreen();
    //     let mut pak = Pak::read(Cursor::new(
    //         read(temp_dir().join(format!("screen-13-bench-read-bitmap-{}.pak", idx))).unwrap(),
    //     ))
    //     .unwrap();
    //     let id = pak.bitmap_id("my-bitmap").unwrap();

    //     group.bench_with_input(
    //         BenchmarkId::new(
    //             "Power of two dimensions",
    //             format!("2 ^ {:0>#2}: {}", idx, criteria),
    //         ),
    //         criteria,
    //         |b, _criteria| {
    //             b.iter(|| {
    //                 gpu.read_bitmap_with_id(black_box(&mut pak), black_box(id));
    //             })
    //         },
    //     );
    // }
}

fn write_bitmap_onto_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("Write bitmap onto render");
    for (idx, criteria) in [
        1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384,
    ]
    .iter()
    .enumerate()
    {
        let gpu = Gpu::offscreen();

        group.bench_with_input(
            BenchmarkId::new(
                "Power of two write count",
                format!(
                    "2 ^ {:0>#2}: {} write{}",
                    idx,
                    criteria,
                    if *criteria == 1 { "" } else { "s" }
                ),
            ),
            criteria,
            |b, criteria| {
                b.iter(|| {
                    let bitmap = black_box(gpu.load_bitmap(
                        BitmapFormat::Rgb,
                        1024,
                        noise((1024u32, 1024).into(), BitmapFormat::Rgb),
                    ));
                    let mut render = black_box(gpu.render((1024u32, 1024u32)));
                    render.write().record(black_box(
                        (0..*criteria)
                            .map(|_| Write::position(&bitmap, (0.0, 0.0)))
                            .collect::<Vec<_>>(),
                    ));
                })
            },
        );
    }
}

criterion_group!(benches, load_bitmap, read_bitmap, write_bitmap_onto_render);
criterion_main!(benches);
