use {
    super::{
        asset::{Asset, Bitmap as BitmapAsset, FontBitmap},
        get_filename_key, get_path,
        pak_log::{Id, PakLog},
    },
    crate::pak::{Bitmap, BitmapFormat, BitmapId, PakBuf},
    image::{buffer::ConvertBuffer, open as image_open, DynamicImage, RgbImage, RgbaImage},
    std::path::Path,
};

// pub fn bake_atlas<P1: AsRef<Path>, P2: AsRef<Path>>(
//     _project_dir: P1,
//     _asset_filename: P2,
//     _altas_asset: &AtlasAsset,
//     _pak: &mut PakBuf,
//     _log: &mut PakLog,
// ) -> BitmapId {
//     todo!();
// }

pub fn bake_bitmap<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    bitmap_asset: &BitmapAsset,
    pak: &mut PakBuf,
    log: &mut PakLog,
) -> BitmapId {
    let asset = Asset::Bitmap(bitmap_asset.clone());
    if log.contains(&asset) {
        match log.get(&asset).unwrap() {
            Id::Bitmap(id) => return id,
            _ => panic!(),
        }
    }

    let key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    // Get the fs objects for this asset
    let dir = asset_filename.as_ref().parent().unwrap();
    let bitmap_filename = get_path(&dir, bitmap_asset.src());

    // Bake the pixels
    let (fmt, width, pixels) = pixels(&bitmap_filename, bitmap_asset.force_opaque());
    let bitmap = Bitmap::new(fmt, width as u16, pixels);

    // Pak and log this asset
    let bitmap_id = pak.push_bitmap(key, bitmap);
    log.add(&asset, bitmap_id);

    bitmap_id
}

pub fn bake_font_bitmap<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    font_bitmap_asset: &FontBitmap,
    pak: &mut PakBuf,
    log: &mut PakLog,
) {
    let asset = Asset::FontBitmap(font_bitmap_asset.clone());
    if log.contains(&asset) && log.get(&asset).is_some() {
        panic!("unexpected state");
    }

    let mut key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    // Get the fs objects for this asset
    let dir = asset_filename.as_ref().parent().unwrap();
    let bitmap_filename = get_path(&dir, font_bitmap_asset.src());

    // Bake the pixels
    let (_, width, pixels) = pixels(&bitmap_filename, true);
    let mut better_pixels = Vec::with_capacity(pixels.len());
    for y in 0..width as usize {
        for x in 0..width as usize {
            let g = pixels[y * width as usize * 3 + x * 3 + 1];
            let r = pixels[y * width as usize * 3 + x * 3 + 2];
            if 0xff == r {
                better_pixels.push(0xff);
                better_pixels.push(0x00);
                better_pixels.push(0x00);
            } else if 0xff == g {
                better_pixels.push(0x00);
                better_pixels.push(0xff);
                better_pixels.push(0x00);
            } else {
                better_pixels.push(0x00);
                better_pixels.push(0x00);
                better_pixels.push(0x00);
            }
        }
    }
    let bitmap = Bitmap::new(BitmapFormat::Rgb, width as u16, better_pixels);

    // Pak and log this asset
    let key_len = key.len();
    key.truncate(key_len - 5);
    let key = format!("{}.png", key);
    let bitmap_id = pak.push_bitmap(key, bitmap);
    log.add(&asset, bitmap_id);
}

fn pixels<P: AsRef<Path>>(filename: P, force_opaque: bool) -> (BitmapFormat, u32, Vec<u8>) {
    match image_open(&filename).unwrap() {
        DynamicImage::ImageRgb8(image) => (BitmapFormat::Rgb, image.width(), pixels_bgr(&image)),
        DynamicImage::ImageRgba8(image) => {
            if force_opaque {
                (
                    BitmapFormat::Rgb,
                    image.width(),
                    pixels_bgr(&image.convert()),
                )
            } else {
                (BitmapFormat::Rgba, image.width(), pixels_bgra(&image))
            }
        }
        _ => unimplemented!(),
    }
}

fn pixels_bgr(image: &RgbImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[2]);
            buf.push(pixel[1]);
            buf.push(pixel[0]);
        }
    }

    buf
}

fn pixels_bgra(image: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 4);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[2]);
            buf.push(pixel[1]);
            buf.push(pixel[0]);
            buf.push(pixel[3]);
        }
    }

    buf
}
