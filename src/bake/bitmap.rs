use {
    super::{
        asset::{Asset, Bitmap as BitmapAsset, FontBitmap as FontBitmapAsset},
        get_filename_key, get_path,
        pak_log::{Id, PakLog},
    },
    crate::pak::{Bitmap, BitmapFormat, BitmapId, FontBitmap, FontBitmapId, PakBuf},
    bmfont::{BMFont, OrdinateOrientation},
    image::{buffer::ConvertBuffer, open as image_open, DynamicImage, RgbImage, RgbaImage},
    std::{
        fs::{read, File},
        io::BufReader,
        path::Path,
    },
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
    font_bitmap_asset: &FontBitmapAsset,
    pak: &mut PakBuf,
    log: &mut PakLog,
) -> FontBitmapId {
    let asset = Asset::FontBitmap(font_bitmap_asset.clone());
    if log.contains(&asset) && log.get(&asset).is_some() {
        panic!("unexpected state");
    }

    let mut key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    // Get the fs objects for this asset
    let dir = asset_filename.as_ref().parent().unwrap();
    let def_filename = get_path(&dir, font_bitmap_asset.src());
    let def_file = read(&def_filename).unwrap();
    let def_parent = def_filename.parent().unwrap();
    let def = BMFont::new(def_file.as_slice(), OrdinateOrientation::TopToBottom).unwrap();
    let pages = def
        .pages()
        .iter()
        .map(|page| {
            let page_filename = def_parent.join(page);

            // Bake the pixels
            let (_, width, pixels) = pixels(&page_filename, true);
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

            Bitmap::new(BitmapFormat::Rgb, width as u16, better_pixels)
        })
        .collect();

    // Pak and log this asset
    let font_bitmap_id = pak.push_font_bitmap(key, FontBitmap::new(def_file, pages));
    log.add(&asset, font_bitmap_id);

    font_bitmap_id
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
