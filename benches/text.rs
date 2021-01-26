// use {
//     criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion},
//     screen_13::prelude_rc::*,
// };

// fn draw_bitmap_font(c: &mut Criterion) {
//     let gpu = Gpu::offscreen();
//     let mut group = c.benchmark_group("Draw bitmap font");
//     for (idx, criteria) in CRITERIASSS.iter().enumerate() {
//         group.bench_with_input(
//             BenchmarkId::new(
//                 "group desc",
//                 format!("WORDS {:0>#2}: {}", idx, criteria),
//             ),
//             criteria,
//             |b, criteria| {
//                 b.iter(|| {
//                     // gpu.load_bitmap(
//                     //     black_box(BitmapFormat::Rgb),
//                     //     black_box(dims.x as u16),
//                     //     black_box(pixels.clone()),
//                     // )
//                 })
//             },
//         );
//     }
// }

// fn draw_scalable_font(c: &mut Criterion) {
//     let gpu = Gpu::offscreen();
//     let mut group = c.benchmark_group("Draw scalable font");
//     for (idx, criteria) in CRITERIASSS.iter().enumerate() {
//         group.bench_with_input(
//             BenchmarkId::new(
//                 "group desc",
//                 format!("WORDS {:0>#2}: {}", idx, criteria),
//             ),
//             criteria,
//             |b, criteria| {
//                 b.iter(|| {
//                     // gpu.load_bitmap(
//                     //     black_box(BitmapFormat::Rgb),
//                     //     black_box(dims.x as u16),
//                     //     black_box(pixels.clone()),
//                     // )
//                 })
//             },
//         );
//     }
// }

// fn load_bitmap_font(c: &mut Criterion) {
//     let gpu = Gpu::offscreen();
//     let mut group = c.benchmark_group("Load bitmap font");
//     for (idx, criteria) in CRITERIASSS.iter().enumerate() {
//         group.bench_with_input(
//             BenchmarkId::new(
//                 "group desc",
//                 format!("WORDS {:0>#2}: {}", idx, criteria),
//             ),
//             criteria,
//             |b, criteria| {
//                 b.iter(|| {
//                     // gpu.load_bitmap(
//                     //     black_box(BitmapFormat::Rgb),
//                     //     black_box(dims.x as u16),
//                     //     black_box(pixels.clone()),
//                     // )
//                 })
//             },
//         );
//     }
// }

// fn load_scalable_font(c: &mut Criterion) {
//     let gpu = Gpu::offscreen();
//     let mut group = c.benchmark_group("Load scalable font");
//     for (idx, criteria) in CRITERIASSS.iter().enumerate() {
//         group.bench_with_input(
//             BenchmarkId::new(
//                 "group desc",
//                 format!("WORDS {:0>#2}: {}", idx, criteria),
//             ),
//             criteria,
//             |b, criteria| {
//                 b.iter(|| {
//                     // gpu.load_bitmap(
//                     //     black_box(BitmapFormat::Rgb),
//                     //     black_box(dims.x as u16),
//                     //     black_box(pixels.clone()),
//                     // )
//                 })
//             },
//         );
//     }
// }

// fn read_bitmap_font(c: &mut Criterion) {
//     let gpu = Gpu::offscreen();
//     let mut group = c.benchmark_group("Read bitmap font");
//     for (idx, criteria) in CRITERIASSS.iter().enumerate() {
//         group.bench_with_input(
//             BenchmarkId::new(
//                 "group desc",
//                 format!("WORDS {:0>#2}: {}", idx, criteria),
//             ),
//             criteria,
//             |b, criteria| {
//                 b.iter(|| {
//                     // gpu.load_bitmap(
//                     //     black_box(BitmapFormat::Rgb),
//                     //     black_box(dims.x as u16),
//                     //     black_box(pixels.clone()),
//                     // )
//                 })
//             },
//         );
//     }
// }

// fn read_scalable_font(c: &mut Criterion) {
//     let gpu = Gpu::offscreen();
//     let mut group = c.benchmark_group("Read scalable font");
//     for (idx, criteria) in CRITERIASSS.iter().enumerate() {
//         group.bench_with_input(
//             BenchmarkId::new(
//                 "group desc",
//                 format!("WORDS {:0>#2}: {}", idx, criteria),
//             ),
//             criteria,
//             |b, criteria| {
//                 b.iter(|| {
//                     // gpu.load_bitmap(
//                     //     black_box(BitmapFormat::Rgb),
//                     //     black_box(dims.x as u16),
//                     //     black_box(pixels.clone()),
//                     // )
//                 })
//             },
//         );
//     }
// }

// criterion_group!(
//     benches,
//     draw_bitmap_font,
//     draw_scalable_font
//     load_bitmap_font,
//     load_scalable_font,
//     read_bitmap_font,
//     read_scalable_font,
// );
// criterion_main!(benches);
