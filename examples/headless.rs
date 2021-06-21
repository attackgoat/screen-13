use screen_13::{
    color::CORNFLOWER_BLUE,
    gpu::{write::Write, Gpu},
    math::{Area, Coord, Rect},
    pak::BitmapFormat,
    ptr::RcK,
};

fn main() {
    pretty_env_logger::init();

    // Create a 216x144 pixel render
    let gpu = Gpu::<RcK>::offscreen();
    let mut render = gpu.render((216u32, 144u32));

    // Use the `image` crate to open a .png file and load it into a Screen 13 bitmap
    let tilemap = {
        let pixels = image::open("examples/res/img/kenney_pixel_platformer.png")
            .unwrap()
            .to_rgba8();
        gpu.load_bitmap(
            BitmapFormat::Rgba,
            pixels.width() as _,
            pixels.into_iter().map(|p| *p),
        )
    };

    // Helper stuff to make it clearer which tiles we're picking from the tilemap
    let src = |x: u32, y: u32| Area::new(x * 18, y * 18, 18, 18);
    let dst_coord = |x: i32, y: i32| Coord::new(x * 18, y * 18);
    let dst_rect = |x: i32, y: i32, w: u32, h: u32| Rect::new(x * 18, y * 18, w * 18, h * 18);
    let grass_left = src(1, 1);
    let grass_mid = src(2, 1);
    let grass_right = src(3, 1);
    let dirt_left = src(1, 6);
    let dirt_mid = src(2, 6);
    let dirt_right = src(3, 6);
    let water = src(13, 1);
    let heart_full = src(4, 2);
    let heart_half = src(5, 2);
    let heart_empty = src(6, 2);
    let tree = src(6, 6);
    let sign = src(5, 4);

    // Clear with blue
    render.clear().with(CORNFLOWER_BLUE).record();

    // Draw some tiles
    render.write().with_preserve().record([
        Write::tile_position(&tilemap, grass_left, dst_coord(0, 6)),
        Write::tile_position(&tilemap, grass_mid, dst_coord(1, 6)),
        Write::tile_position(&tilemap, grass_mid, dst_coord(2, 6)),
        Write::tile_position(&tilemap, grass_mid, dst_coord(3, 6)),
        Write::tile_position(&tilemap, grass_right, dst_coord(4, 6)),
        Write::tile_position(&tilemap, dirt_left, dst_coord(0, 7)),
        Write::tile_position(&tilemap, dirt_mid, dst_coord(1, 7)),
        Write::tile_position(&tilemap, dirt_mid, dst_coord(2, 7)),
        Write::tile_position(&tilemap, dirt_mid, dst_coord(3, 7)),
        Write::tile_position(&tilemap, dirt_right, dst_coord(4, 7)),
        Write::tile_position(&tilemap, water, dst_coord(5, 7)),
        Write::tile_position(&tilemap, water, dst_coord(6, 7)),
        Write::tile_position(&tilemap, water, dst_coord(7, 7)),
        Write::tile_position(&tilemap, water, dst_coord(8, 7)),
        Write::tile_position(&tilemap, water, dst_coord(9, 7)),
        Write::tile_position(&tilemap, water, dst_coord(10, 7)),
        Write::tile_position(&tilemap, water, dst_coord(11, 7)),
        Write::tile_position(&tilemap, tree, dst_coord(3, 5)),
        Write::tile_position(&tilemap, sign, dst_coord(4, 5)),
        Write::tile_region(&tilemap, heart_full, dst_rect(4, 0, 2, 2)),
        Write::tile_region(&tilemap, heart_full, dst_rect(6, 0, 2, 2)),
        Write::tile_region(&tilemap, heart_half, dst_rect(8, 0, 2, 2)),
        Write::tile_region(&tilemap, heart_empty, dst_rect(10, 0, 2, 2)),
    ]);

    // Save as png (See documentation on flush function if you want to handle errors!)
    render.encode().record("output.png");
}
