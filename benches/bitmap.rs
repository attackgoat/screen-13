use {
    criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion},
    screen_13::prelude_rc::*,
};

const POT_DIMS: [Extent; 15] = [
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
];

/// 💀 Extremely unsafe - You literally get back noise I'm not sure what you'd even want that for.
fn noise(dims: Extent, fmt: BitmapFormat) -> Vec<u8> {
    let mut res = Vec::with_capacity(dims.x as usize * dims.y as usize * fmt.byte_len());
    unsafe {
        res.set_len(res.capacity());
    }

    res
}

fn load_bitmap(c: &mut Criterion) {
    let gpu = Gpu::offscreen();
    let mut group = c.benchmark_group("Load bitmap");
    for (idx, dims) in POT_DIMS.iter().enumerate() {
        let pixels = noise(*dims, BitmapFormat::Rgb);

        group.bench_with_input(
            BenchmarkId::new(
                "Power of two",
                format!("2 ^ {:0>#2}: {}x{}", idx, dims.x, dims.y),
            ),
            dims,
            |b, dims| {
                b.iter(|| {
                    gpu.load_bitmap(
                        black_box(BitmapFormat::Rgb),
                        black_box(dims.x as u16),
                        black_box(pixels.clone()),
                    )
                })
            },
        );
    }
}

criterion_group!(benches, load_bitmap);
criterion_main!(benches);
