use {
    criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion},
    screen_13::prelude_rc::*,
};

const BITMAP_DIMS: [Extent; 14] = [
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
    //c.bench_function("bench_small_dims", load_bitmaps(&SMALL_DIMS[..]));
    let mut group = c.benchmark_group("Image sizes");
    for dims in BITMAP_DIMS.iter() {
        let pixels = noise(*dims, BitmapFormat::R);
        let gpu = Gpu::offscreen();

        group.bench_with_input(BenchmarkId::new("Power of two", dims), dims, |b, dims| {
            b.iter(|| {
                gpu.load_bitmap(
                    black_box(BitmapFormat::R),
                    black_box(dims.x as u16),
                    black_box(pixels.clone()),
                )
            })
        });
    }
}

criterion_group!(benches, load_bitmap);
criterion_main!(benches);
